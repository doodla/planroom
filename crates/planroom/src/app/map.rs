//! `planroom map`: the map at a level, folded from the active snapshots on read.
//!
//! The leaf honors its catalogue budget (`CommandSpec::budget_tokens`): over it, the map keeps its
//! blocks in their documented order while they fit, keeps the lifted edges between the kept ones,
//! and says so with a `truncated` warning whose `details.next` is a handle that works today. Only
//! the canvas's layout input, `--detail graph` over `serve` and in the `share` snapshot, takes the
//! whole graph, until M2's paging pages it (architecture.md, The CLI: "`graph` pages").

use std::collections::{BTreeMap, HashSet};
use std::path::Path;

use planroom_contract::{
    Command, CommandName, Detail, Edge, Empty, EmptyReason, Handle, LayoutKey, Level, LiftedEdge, MapNode,
    MapReq, MapView, Node, NodeId, Position, Warning, WarningCode,
};
use planroom_kernel::{facts, tree};
use planroom_store::{Shape, Store};
use serde_json::json;

use crate::app::error::AppError;
use crate::app::workspace;
use crate::app::{Reply, empty_view};
use crate::text;

/// The lens a map shows when none is asked for; no pack declares a default lens yet.
const ALL_LENSES: &str = "all";
/// What the envelope around a map takes besides the view: its other fields, the summary line and
/// the `truncated` warning. The view gets the rest of the budget.
const ENVELOPE_RESERVE_BYTES: usize = 1_536;
/// Block names the one-line summary lists before "and N more".
const SUMMARY_NAMES: usize = 12;

/// `planroom map` as an agent reads it, through the CLI or the tools: within the leaf's budget.
pub fn run(req: &MapReq, cwd: &Path) -> Result<Reply<MapView>, AppError> {
    run_within(req, cwd, Some(budget_tokens()))
}

/// `map` for the canvas: `--detail graph` is its layout input and takes every block; a summary
/// stays within the leaf's budget.
pub fn for_canvas(req: &MapReq, cwd: &Path) -> Result<Reply<MapView>, AppError> {
    let budget = (req.detail == Detail::Summary).then(budget_tokens);
    run_within(req, cwd, budget)
}

fn budget_tokens() -> u32 {
    Command::Map(MapReq::default()).spec().budget_tokens
}

fn run_within(req: &MapReq, cwd: &Path, budget: Option<u32>) -> Result<Reply<MapView>, AppError> {
    if req.focus.is_some() {
        return Err(AppError::invalid_argument("focus", "argument.focus_unsupported"));
    }
    let lens = req.lens.clone().unwrap_or_else(|| ALL_LENSES.into());
    // Outside a workspace, the same empty state `focus` shows (DR-57).
    let Some(root) = workspace::find_root(cwd) else {
        return not_scanned(cwd, lens, req.level);
    };
    let Some(store) = Store::open_existing(&workspace::store_path(&root))? else {
        return not_scanned(&root, lens, req.level);
    };
    let mut view = top(&store, &root, req.level)?;
    view.lens = lens;
    if view.empty.is_none() {
        match req.detail {
            Detail::Graph => place(&store, &mut view)?,
            // Agents get the counts and the lifted edges; evidence is a `show` away.
            Detail::Summary => view.edges.iter_mut().for_each(|e| e.evidence_sample.clear()),
        }
    }
    let total = view.nodes.len();
    let warnings: Vec<Warning> =
        budget.and_then(|budget| fit(&mut view, budget, req.level)).into_iter().collect();
    let seq = store.seq()?;
    let mut text = text_of(&view, total);
    for warning in &warnings {
        text.push('\n');
        text.push_str(&warning.message);
    }
    Ok(Reply { text, result: view, seq, warnings })
}

/// The map of the store's active snapshots: the top of the tree, or every node at `level`. A
/// level with no blocks is an `Empty` whose next action works today.
pub fn top(store: &Store, root: &Path, level: Option<Level>) -> Result<MapView, AppError> {
    let active = store.active_set()?;
    if active.repos.is_empty() {
        return Ok(not_scanned(root, ALL_LENSES.into(), level)?.result);
    }
    let mut nodes: Vec<Node> = Vec::new();
    let mut edges: Vec<Edge> = Vec::new();
    for row in store.facts(&active.snapshots())? {
        match row.shape {
            Shape::Node => nodes.push(serde_json::from_str(&row.body).map_err(AppError::internal)?),
            Shape::Edge => edges.push(serde_json::from_str(&row.body).map_err(AppError::internal)?),
            Shape::Hint => {}
        }
    }
    let nodes = facts::merge_nodes(nodes);
    let (shown, blocks) = match level {
        None => (Level::System, tree::top_level(&nodes)),
        Some(level) => (level, tree::at_level(&nodes, level)),
    };
    let edges = tree::lift_edges(&nodes, &edges, &blocks);
    let empty = blocks.is_empty().then(|| level_empty(root, level));
    Ok(MapView {
        level: shown,
        lens: ALL_LENSES.into(),
        focus: None,
        nodes: blocks,
        edges,
        hidden: 0,
        empty,
        layout: None,
    })
}

/// The empty state of a scanned workspace's level with no blocks: the intent level before any
/// plan, or a level this workspace has nothing at. Its next action is the top of the map, which a
/// scan always fills; an empty top offers the scan again.
fn level_empty(root: &Path, level: Option<Level>) -> Empty {
    let map = |label: String| Handle { command: CommandName("map".into()), args: vec![], label };
    let (reason, next) = match level {
        Some(Level::Intent) => ("no_intent", map(text::get("empty.no_intent", &[]))),
        Some(level) => {
            ("empty_level", map(text::get("empty.empty_level", &[("level", &tree::level_name(level))])))
        }
        None => (
            "nothing_mapped",
            Handle {
                command: CommandName("scan".into()),
                args: vec![root.display().to_string()],
                label: text::get("empty.nothing_mapped", &[]),
            },
        ),
    };
    Empty { reason: EmptyReason(reason.into()), next }
}

/// Keep the blocks that fit `budget_tokens`, in their order, and the lifted edges between them.
/// Returns the `truncated` warning when it cut anything. `scan`'s reply carries a map too.
pub(crate) fn fit(view: &mut MapView, budget_tokens: u32, level: Option<Level>) -> Option<Warning> {
    let limit = (budget_tokens as usize * 4).saturating_sub(ENVELOPE_RESERVE_BYTES);
    let size = |v: &MapView| serde_json::to_string(v).map_or(usize::MAX, |s| s.len());
    if size(view) <= limit {
        return None;
    }
    let nodes = std::mem::take(&mut view.nodes);
    let edges = std::mem::take(&mut view.edges);
    let total = nodes.len();
    // The size grows with the number of blocks kept, so the largest prefix that fits is a search.
    let (mut fits, mut over) = (0, total);
    while fits < over {
        let mid = (fits + over).div_ceil(2);
        (view.nodes, view.edges) = prefix(&nodes, &edges, mid);
        if size(view) <= limit { fits = mid } else { over = mid - 1 }
    }
    (view.nodes, view.edges) = prefix(&nodes, &edges, fits);
    let next = drill_down(level.unwrap_or(view.level), level.is_none());
    let message = text::get(
        "warning.truncated",
        &[("total", &total), ("kept", &fits), ("budget", &budget_tokens), ("label", &next.label)],
    );
    Some(Warning {
        code: WarningCode::Truncated,
        message,
        details: json!({"kept": fits, "total": total, "budget_tokens": budget_tokens, "next": next}),
    })
}

/// The first `count` blocks and the lifted edges between them.
fn prefix(nodes: &[MapNode], edges: &[LiftedEdge], count: usize) -> (Vec<MapNode>, Vec<LiftedEdge>) {
    let kept = nodes[..count].to_vec();
    let ids: HashSet<&NodeId> = kept.iter().map(|n| &n.id).collect();
    let edges = edges.iter().filter(|e| ids.contains(&e.from) && ids.contains(&e.to)).cloned().collect();
    (kept, edges)
}

/// Where to go from a map cut at its budget, with what `map` and `focus` serve today: the next
/// coarser level, whose blocks hold these; from the top, the pane's summary.
fn drill_down(level: Level, at_top: bool) -> Handle {
    let coarser = match level {
        Level::Code => Some(Level::Component),
        Level::Component => Some(Level::Container),
        Level::Container => Some(Level::System),
        Level::System | Level::Intent => None,
    };
    match coarser.filter(|_| !at_top) {
        Some(level) => {
            let name = tree::level_name(level);
            Handle {
                command: CommandName("map".into()),
                args: vec!["--level".into(), name.into()],
                label: text::get("map.coarser", &[("level", &name)]),
            }
        }
        None => {
            Handle { command: CommandName("focus".into()), args: vec![], label: text::get("map.pane", &[]) }
        }
    }
}

/// `--detail graph`: the view's layout key, and each block's position from the layout cache
/// when one is cached for this structure.
fn place(store: &Store, view: &mut MapView) -> Result<(), AppError> {
    let key = LayoutKey {
        view_key: format!("map:{}:{}", tree::level_name(view.level), view.lens),
        structure_hash: tree::structure_hash(&view.nodes, &view.edges),
    };
    if let Some(cached) = store.layout(&key.view_key, &key.structure_hash)? {
        let positions: BTreeMap<NodeId, Position> =
            serde_json::from_str(&cached).map_err(AppError::internal)?;
        for node in &mut view.nodes {
            node.position = positions.get(&node.id).copied();
        }
    }
    view.layout = Some(key);
    Ok(())
}

/// A directory with no scan yet, a workspace or not: an empty map whose next action is the scan,
/// or, with no repos to scan, the same empty state `focus` returns.
fn not_scanned(dir: &Path, lens: String, level: Option<Level>) -> Result<Reply<MapView>, AppError> {
    let mut view = empty_view(level.unwrap_or(Level::System), lens);
    view.empty = Some(scan_empty(dir, &workspace::scannable(dir)));
    Ok(Reply { text: text_of(&view, 0), result: view, seq: 0, warnings: vec![] })
}

/// The top of the map of a workspace a scan wrote nothing for, since git could read none of its
/// repos: the same empty state `map` returns there.
pub(crate) fn not_scanned_view(root: &Path) -> Result<MapView, AppError> {
    Ok(not_scanned(root, ALL_LENSES.into(), None)?.result)
}

/// The empty state of a directory with no scan: map the repos git can read. With none, no scan
/// can map anything, so the line says what to do and the action checks again.
pub(crate) fn scan_empty(root: &Path, repos: &workspace::Scannable) -> Empty {
    let check_again = |reason: &str, label: String| Empty {
        reason: EmptyReason(reason.into()),
        next: Handle { command: CommandName("focus".into()), args: vec![], label },
    };
    if repos.readable == 0 && !repos.unreadable.is_empty() {
        let names = repos.unreadable.join(", ");
        return check_again("repos_unreadable", text::get("empty.repos_unreadable", &[("names", &names)]));
    }
    if repos.readable == 0 {
        return check_again("no_repos", text::get("empty.no_repos", &[]));
    }
    Empty {
        reason: EmptyReason("not_scanned".into()),
        next: Handle {
            command: CommandName("scan".into()),
            args: vec![root.display().to_string()],
            label: text::get("empty.not_scanned", &[("count", &repos.readable)]),
        },
    }
}

/// The text mode's first line, which the envelope also carries as `summary`: the empty state's
/// line, or the count and the first block names with their counts.
fn text_of(view: &MapView, total: usize) -> String {
    if let Some(empty) = &view.empty {
        return empty.next.label.clone();
    }
    let names: Vec<String> = view
        .nodes
        .iter()
        .take(SUMMARY_NAMES)
        .map(|n| {
            let counts: Vec<String> =
                n.counts.iter().map(|(level, count)| format!("{count} {level}")).collect();
            if counts.is_empty() { n.name.clone() } else { format!("{} ({})", n.name, counts.join(", ")) }
        })
        .collect();
    let more = total.saturating_sub(names.len());
    let names = if more == 0 {
        names.join("; ")
    } else {
        text::get("map.more", &[("names", &names.join("; ")), ("more", &more)])
    };
    text::get("map.summary", &[("count", &total), ("names", &names)])
}
