//! The map as the store holds it: the active snapshots' facts, folded, at a level; its empty
//! states; and its cached layout.

use std::collections::BTreeMap;
use std::path::Path;

use planroom_contract::{
    CommandName, Edge, Empty, EmptyReason, Handle, LayoutKey, Level, MapView, Node, NodeId, Position,
    Vocabulary,
};
use planroom_kernel::{Facts, fold, tree};
use planroom_store::{Shape, Store};

use crate::app::error::AppError;
use crate::app::map::ALL_LENSES;
use crate::app::workspace;
use crate::app::{Reply, empty_view};
use crate::text;

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
            // No linker joins hints into edges yet, so the fold doesn't read them.
            Shape::Hint => {}
        }
    }
    // No pack is loaded and no event changes the graph yet: the model is the resolved facts.
    let model = fold(&Facts { nodes, edges, hints: vec![] }, &[], &Vocabulary::default(), store.seq()?);
    let (nodes, edges) = (model.nodes, model.edges);
    let (shown, blocks) = match level {
        None => (Level::System, tree::top_level(&nodes)),
        Some(level) => (level, tree::at_level(&nodes, level)),
    };
    let edges = tree::lift_edges(&nodes, &edges, &blocks);
    // With nothing at the top there is nothing at any level, so every level shows the top's own
    // empty state, which says why.
    let empty = if !blocks.is_empty() {
        None
    } else if level.is_none() || tree::top_level(&nodes).is_empty() {
        Some(top_empty(root))
    } else {
        Some(level_empty(level))
    };
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

/// The empty state of a scanned workspace's level with no blocks, below a top that has some: the
/// intent level before any plan, or a level this workspace has nothing at. Its next action is the
/// top of the map.
fn level_empty(level: Option<Level>) -> Empty {
    let map = |label: String| Handle { command: CommandName("map".into()), args: vec![], label };
    let (reason, next) = match level {
        Some(Level::Intent) => ("no_intent", map(text::get("empty.no_intent", &[]))),
        Some(level) => {
            ("empty_level", map(text::get("empty.empty_level", &[("level", &tree::level_name(level))])))
        }
        None => ("empty_level", map(text::get("empty.empty_level", &[("level", &"system")]))),
    };
    Empty { reason: EmptyReason(reason.into()), line: None, next }
}

/// The empty state of a scanned workspace with nothing at the top. When a scan can now map a repo,
/// one with a commit, the action is that scan; otherwise no scan could map anything, so the state
/// says why (no commits yet, or git can't read the repos) and the action checks again.
fn top_empty(root: &Path) -> Empty {
    let repos = workspace::scannable(root);
    if repos.readable == 0 {
        return scan_empty(root, &repos);
    }
    Empty {
        reason: EmptyReason("nothing_mapped".into()),
        line: None,
        next: Handle {
            command: CommandName("scan".into()),
            args: vec![root.display().to_string()],
            label: text::get("empty.nothing_mapped", &[]),
        },
    }
}

/// `--detail graph`: the view's layout key, and each block's position from the layout cache
/// when one is cached for this structure.
pub(super) fn place(store: &Store, view: &mut MapView) -> Result<(), AppError> {
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
pub(super) fn not_scanned(
    dir: &Path,
    lens: String,
    level: Option<Level>,
) -> Result<Reply<MapView>, AppError> {
    let mut view = empty_view(level.unwrap_or(Level::System), lens);
    view.empty = Some(scan_empty(dir, &workspace::scannable(dir)));
    Ok(Reply { text: super::text_of(&view, 0), result: view, seq: 0, warnings: vec![] })
}

/// The top of the map of a workspace a scan wrote nothing for, since git could read none of its
/// repos: the same empty state `map` returns there.
pub(crate) fn not_scanned_view(root: &Path) -> Result<MapView, AppError> {
    Ok(not_scanned(root, ALL_LENSES.into(), None)?.result)
}

/// The empty state of a directory with no scan: map the repos a scan can map, those with a commit.
/// With none, no scan can map anything, so the line says what to do (make a first commit, or see
/// why git can't read them) and the action checks again.
pub(crate) fn scan_empty(root: &Path, repos: &workspace::Scannable) -> Empty {
    let check_again = |reason: &str, label: String| Empty {
        reason: EmptyReason(reason.into()),
        line: None,
        next: Handle { command: CommandName("focus".into()), args: vec![], label },
    };
    if repos.readable == 0 && !repos.unborn.is_empty() {
        let names = repos.unborn.join(", ");
        return check_again("repos_unborn", text::get("empty.repos_unborn", &[("names", &names)]));
    }
    if repos.readable == 0 && !repos.unreadable.is_empty() {
        let names = repos.unreadable.join(", ");
        return check_again("repos_unreadable", text::get("empty.repos_unreadable", &[("names", &names)]));
    }
    if repos.readable == 0 {
        return check_again("no_repos", text::get("empty.no_repos", &[]));
    }
    Empty {
        reason: EmptyReason("not_scanned".into()),
        line: None,
        next: Handle {
            command: CommandName("scan".into()),
            args: vec![root.display().to_string()],
            label: text::get("empty.not_scanned", &[("count", &repos.readable)]),
        },
    }
}
