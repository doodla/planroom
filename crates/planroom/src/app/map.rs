//! `planroom map`: the map at a level, folded from the active snapshots on read.

use std::path::Path;

use planroom_contract::{CommandName, Empty, EmptyReason, Handle, Level, MapReq, MapView, Node};
use planroom_kernel::{facts, tree};
use planroom_store::{Shape, Store};

use crate::app::error::AppError;
use crate::app::workspace;
use crate::app::{Reply, empty_view};
use crate::text;

/// The lens a map shows when none is asked for; no pack declares a default lens yet.
const ALL_LENSES: &str = "all";

pub fn run(req: &MapReq, cwd: &Path) -> Result<Reply<MapView>, AppError> {
    if req.focus.is_some() {
        return Err(AppError::invalid_argument("focus", "argument.focus_unsupported"));
    }
    let root = workspace::find_root(cwd).ok_or_else(|| AppError::workspace_not_found(cwd))?;
    let lens = req.lens.clone().unwrap_or_else(|| ALL_LENSES.into());
    let Some(store) = Store::open_existing(&workspace::store_path(&root))? else {
        return not_scanned(&root, lens, req.level);
    };
    let mut view = top(&store, &root, req.level)?;
    view.lens = lens;
    let seq = store.seq()?;
    Ok(Reply { text: text_of(&view), result: view, seq, warnings: vec![] })
}

/// The map of the store's active snapshots: the top of the tree, or every node at `level`.
pub fn top(store: &Store, root: &Path, level: Option<Level>) -> Result<MapView, AppError> {
    let active = store.active_set()?;
    if active.repos.is_empty() {
        return Ok(not_scanned(root, ALL_LENSES.into(), level)?.result);
    }
    let mut nodes: Vec<Node> = Vec::new();
    for row in store.facts(&active.snapshots())? {
        if row.shape == Shape::Node {
            nodes.push(serde_json::from_str(&row.body).map_err(AppError::internal)?);
        }
    }
    let nodes = facts::merge_nodes(nodes);
    let (level, blocks) = match level {
        None => (Level::System, tree::top_level(&nodes)),
        Some(level) => (level, tree::at_level(&nodes, level)),
    };
    Ok(MapView {
        level,
        lens: ALL_LENSES.into(),
        focus: None,
        nodes: blocks,
        edges: vec![],
        hidden: 0,
        empty: None,
    })
}

/// A workspace with no scan yet: an empty map whose next action is the scan.
fn not_scanned(root: &Path, lens: String, level: Option<Level>) -> Result<Reply<MapView>, AppError> {
    let repos = workspace::discover(root).map(|ws| ws.repos.len()).unwrap_or(0);
    let mut view = empty_view(level.unwrap_or(Level::System), lens);
    view.empty = Some(scan_empty(root, repos));
    Ok(Reply { text: text_of(&view), result: view, seq: 0, warnings: vec![] })
}

/// The empty state of a directory with no scan: map its repos, or find some first.
pub(crate) fn scan_empty(root: &Path, repos: usize) -> Empty {
    let (reason, label) = if repos == 0 {
        ("no_repos", text::get("empty.no_repos", &[]))
    } else {
        ("not_scanned", text::get("empty.not_scanned", &[("count", &repos)]))
    };
    Empty {
        reason: EmptyReason(reason.into()),
        next: Handle { command: CommandName("scan".into()), args: vec![root.display().to_string()], label },
    }
}

fn text_of(view: &MapView) -> String {
    if let Some(empty) = &view.empty {
        return empty.next.label.clone();
    }
    let names: Vec<String> = view
        .nodes
        .iter()
        .map(|n| {
            let counts: Vec<String> =
                n.counts.iter().map(|(level, count)| format!("{count} {level}")).collect();
            if counts.is_empty() { n.name.clone() } else { format!("{} ({})", n.name, counts.join(", ")) }
        })
        .collect();
    text::get("map.summary", &[("count", &view.nodes.len()), ("names", &names.join("; "))])
}
