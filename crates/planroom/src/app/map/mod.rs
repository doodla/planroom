//! `planroom map`: the map at a level, or under one node, folded from the store on read.
//!
//! A reply reads the store in one read transaction: the model is the fold at that transaction's
//! highest `seq`, and the reply's `meta.seq` is that `seq`, so it never runs ahead of the view
//! (DR-73). A scan, an undo or a review refolds; in `serve` the fold is kept between replies and
//! caught up one event at a time (DR-61, `live::model`).
//!
//! The leaf honors its catalogue budget (`CommandSpec::budget_tokens`) at every detail: over it,
//! the map gives its first page, the blocks that fit taken by level and then ID and listed in
//! their documented order ([`page`]), with the lifted edges that land there, and `meta.next`, the
//! cursor `--cursor` takes for the page after it, with a `truncated` warning whose
//! `details.next` is a drill-down that works today. Only the canvas's layout input, `--detail
//! graph` over `serve` and in the `share` snapshot, takes the whole graph, until M2 pages it
//! (architecture.md, Token budgets).
//!
//! A map is read ([`read`]), filtered by its lens ([`lens`]) and cut to a page within the leaf's
//! budget ([`page`]); [`focus`] finds the node `--focus` names, whose blocks the view shows.

mod focus;
pub(crate) mod lens;
pub(crate) mod page;
mod read;

use std::collections::HashMap;
use std::path::Path;

use planroom_contract::{Command, Detail, Level, MapNode, MapReq, MapView, Vocabulary};
use planroom_kernel::tree;
use planroom_store::{Snapshot, Store};

use crate::app::error::AppError;
use crate::app::workspace;
use crate::app::{Paged, Reply};
use crate::text;

pub(crate) use page::fit_beside;
pub(crate) use read::{not_scanned_view, scan_empty};
pub use read::{top, top_in};

/// The lens a map shows when none is asked for; no pack declares a default lens yet.
const ALL_LENSES: &str = "all";
/// Block names the one-line summary lists before "and N more".
const SUMMARY_NAMES: usize = 12;

/// `planroom map` as an agent reads it, through the CLI or the tools: within the leaf's budget.
pub fn run(req: &MapReq, cwd: &Path) -> Result<Paged<MapView>, AppError> {
    run_within(req, cwd, Some(budget_tokens()))
}

/// `map` for the canvas: `--detail graph` is its layout input and takes every block; a summary
/// stays within the leaf's budget.
pub fn for_canvas(req: &MapReq, cwd: &Path) -> Result<Paged<MapView>, AppError> {
    let budget = (req.detail == Detail::Summary).then(budget_tokens);
    run_within(req, cwd, budget)
}

/// [`for_canvas`] inside a read transaction the caller holds, so the rest of what it builds, such
/// as `share`'s pane and its snapshot's `seq`, comes from the same version of the store as the map.
pub fn for_canvas_in(snapshot: &Snapshot<'_>, root: &Path, req: &MapReq) -> Result<Paged<MapView>, AppError> {
    let budget = (req.detail == Detail::Summary).then(budget_tokens);
    reply_in(snapshot, root, req, budget)
}

fn budget_tokens() -> u32 {
    Command::Map(MapReq::default()).spec().budget_tokens
}

fn run_within(req: &MapReq, cwd: &Path, budget: Option<u32>) -> Result<Paged<MapView>, AppError> {
    let lens_id = req.lens.clone().unwrap_or_else(|| ALL_LENSES.into());
    let whole = |reply: Reply<MapView>| Paged { reply, next: None };
    // Outside a workspace, the same empty state `focus` shows (DR-57).
    let Some(root) = workspace::find_root(cwd) else {
        return read::not_scanned(cwd, lens_id, req.level, 0).map(whole);
    };
    let Some(store) = Store::open_existing(&workspace::store_path(&root))? else {
        return read::not_scanned(&root, lens_id, req.level, 0).map(whole);
    };
    reply_in(&store.snapshot()?, &root, req, budget)
}

/// The map reply read in one read transaction: the fold, the layout and `meta.seq` (DR-73).
fn reply_in(
    snapshot: &Snapshot<'_>,
    root: &Path,
    req: &MapReq,
    budget: Option<u32>,
) -> Result<Paged<MapView>, AppError> {
    let lens_id = req.lens.clone().unwrap_or_else(|| ALL_LENSES.into());
    let seq = snapshot.seq();
    // No pack is loaded yet, so the lens keeps the whole view.
    let vocabulary = Vocabulary::default();
    let (mut view, under) = read::view(snapshot, root, req.level, req.focus.as_deref(), &vocabulary)?;
    view.lens = lens_id.clone();
    let mut view = lens::filter(view, &vocabulary, &lens_id);
    if view.empty.is_none() {
        match req.detail {
            // Positions come from the whole map's layout, whichever page shows them.
            Detail::Graph => read::place(snapshot, &mut view)?,
            // Agents get the counts and the lifted edges; evidence is a `show` away.
            Detail::Summary => view.edges.iter_mut().for_each(|e| e.evidence_sample.clear()),
        }
    }
    // The page is cut from the view as the reply carries it, so it is measured as it is sent.
    let page = page::cut(view, req, seq, budget)?;
    let mut text = text_of(&page.view, page.total, req.level, under.as_deref());
    for warning in &page.warnings {
        text.push('\n');
        text.push_str(&warning.message);
    }
    Ok(Paged { reply: Reply { text, result: page.view, seq, warnings: page.warnings }, next: page.next })
}

/// The text mode's first line, which the envelope also carries as `summary`: the empty state's
/// line, or what the map shows (the top, one level, or the blocks under one node) with the count
/// and the first block names with their counts. `level` is the level the request named, and
/// `under` the name of the node it focused on.
fn text_of(view: &MapView, total: usize, level: Option<Level>, under: Option<&str>) -> String {
    if let Some(empty) = &view.empty {
        return empty.line.clone().unwrap_or_else(|| empty.next.label.clone());
    }
    let shown = names(&view.nodes);
    let listed: Vec<String> = view
        .nodes
        .iter()
        .zip(shown)
        .take(SUMMARY_NAMES)
        .map(|(n, name)| {
            let counts: Vec<String> =
                n.counts.iter().map(|(level, count)| format!("{count} {level}")).collect();
            if counts.is_empty() { name } else { format!("{name} ({})", counts.join(", ")) }
        })
        .collect();
    let more = total.saturating_sub(listed.len());
    let names = if more == 0 {
        listed.join("; ")
    } else {
        text::get("map.more", &[("names", &listed.join("; ")), ("more", &more)])
    };
    let at = tree::level_name(view.level);
    match (under, level) {
        (Some(name), _) => text::get(
            "map.summary_under",
            &[("count", &total), ("name", &name), ("level", &at), ("names", &names)],
        ),
        (None, Some(_)) => {
            text::get("map.summary_level", &[("count", &total), ("level", &at), ("names", &names)])
        }
        (None, None) => text::get("map.summary", &[("count", &total), ("names", &names)]),
    }
}

/// Each block's name as the text lists it: its own name, or, when another block in the view has
/// the same one, its repo and key from its ID (`orders/.gitignore`), so the line tells them apart.
fn names(nodes: &[MapNode]) -> Vec<String> {
    let mut seen: HashMap<&str, usize> = HashMap::new();
    for node in nodes {
        *seen.entry(node.name.as_str()).or_default() += 1;
    }
    nodes
        .iter()
        .map(|n| {
            if seen[n.name.as_str()] < 2 {
                return n.name.clone();
            }
            match n.id.0.splitn(3, ':').collect::<Vec<_>>().as_slice() {
                [namespace, _, key] => format!("{namespace}/{key}"),
                _ => n.id.0.clone(),
            }
        })
        .collect()
}
