//! The map as the store holds it: the fold of one read transaction, at a level or under one
//! node; its empty states; and its cached layout.
//!
//! Everything a reply shows comes from one [`Snapshot`]: the model, folded at the snapshot's
//! `seq` (the process's kept fold caught up to it, `live::model`), the layout cache, and the
//! `seq` the reply carries as `meta.seq`. So a reply's `meta.seq` never runs ahead of the view it
//! folded, and its repos never come from one scan beside a link from another (DR-73).

use std::collections::BTreeMap;
use std::path::Path;

use planroom_contract::{
    CommandName, Empty, EmptyReason, Handle, LayoutKey, Level, MapView, NodeId, Position, Vocabulary,
};
use planroom_kernel::tree;
use planroom_store::{Snapshot, Store};

use crate::app::error::AppError;
use crate::app::map::{ALL_LENSES, focus};
use crate::app::workspace;
use crate::app::{Reply, empty_view};
use crate::{live, text};

/// The map of the store's active snapshots at the top of the tree, or at `level`, read in one
/// read transaction. A level with no blocks is an `Empty` whose next action works today.
pub fn top(store: &Store, root: &Path, level: Option<Level>) -> Result<MapView, AppError> {
    top_in(&store.snapshot()?, root, level)
}

/// [`top`] inside a read transaction the caller holds, so the rest of its reply, such as the
/// pane's `seq`, comes from the same version of the store.
pub fn top_in(snapshot: &Snapshot<'_>, root: &Path, level: Option<Level>) -> Result<MapView, AppError> {
    Ok(view(snapshot, root, level, None, &Vocabulary::default())?.0)
}

/// The map in `snapshot`: the blocks at `level` (the top, the system level, when it is `None`),
/// or, with `focus`, the blocks under that one node, at `level` or else at the level below the
/// node's own [DR-85], with the node's name. A level with no blocks is an `Empty` whose next
/// action works today.
pub(super) fn view(
    snapshot: &Snapshot<'_>,
    root: &Path,
    level: Option<Level>,
    focus: Option<&str>,
    vocabulary: &Vocabulary,
) -> Result<(MapView, Option<String>), AppError> {
    if snapshot.active_set()?.repos.is_empty() {
        return Ok((not_scanned(root, ALL_LENSES.into(), level, snapshot.seq())?.result, None));
    }
    let model = live::model(snapshot, vocabulary)?;
    let (nodes, edges) = (&model.nodes, &model.edges);
    let under = focus.map(|target| focus::resolve(nodes, target)).transpose()?;
    // `map` and `map --level system` are one view, the top of the map: both say they show the
    // system level, so both show the same blocks.
    let shown = match (level, under) {
        (Some(level), _) => level,
        (None, Some(node)) => focus::level_below(node.level),
        (None, None) => Level::System,
    };
    let blocks = tree::blocks_at(nodes, shown, under.map(|n| &n.id));
    let edges = tree::lift_edges(nodes, edges, &blocks);
    let empty = if !blocks.is_empty() {
        None
    } else if let Some(node) = under {
        let parent = node.parent.as_ref().and_then(|p| nodes.iter().find(|n| &n.id == p));
        Some(focus::nothing_under(node, shown, parent))
    } else if tree::blocks_at(nodes, Level::System, None).is_empty() {
        // With nothing at the top there is nothing at any level, so every level shows the top's
        // own empty state, which says why.
        Some(top_empty(root))
    } else {
        Some(level_empty(shown))
    };
    let view = MapView {
        level: shown,
        lens: ALL_LENSES.into(),
        focus: under.map(|n| n.id.clone()),
        nodes: blocks,
        edges,
        hidden: 0,
        empty,
        layout: None,
    };
    Ok((view, under.map(|n| n.name.clone())))
}

/// The empty state of a scanned workspace's level with no blocks, below a top that has some: the
/// intent level before any plan, or a level this workspace has nothing at. Its next action is the
/// top of the map.
fn level_empty(level: Level) -> Empty {
    let (reason, line) = match level {
        Level::Intent => ("no_intent", text::get("empty.no_intent", &[])),
        level => ("empty_level", text::get("empty.empty_level", &[("level", &tree::level_name(level))])),
    };
    let next = Handle {
        command: CommandName("map".into()),
        args: vec![],
        label: text::label("empty.top_next", &[]),
    };
    Empty { reason: EmptyReason(reason.into()), line: Some(line), next }
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
        line: Some(text::get("empty.nothing_mapped", &[])),
        next: Handle {
            command: CommandName("scan".into()),
            args: vec![root.display().to_string()],
            label: text::label("empty.nothing_mapped_next", &[]),
        },
    }
}

/// `--detail graph`: the view's layout key, and each block's position from the layout cache
/// when one is cached for this structure, read in the same snapshot as the view.
pub(super) fn place(snapshot: &Snapshot<'_>, view: &mut MapView) -> Result<(), AppError> {
    let key = LayoutKey {
        view_key: view_key(view),
        structure_hash: tree::structure_hash(&view.nodes, &view.edges),
    };
    if let Some(cached) = snapshot.layout(&key.view_key, &key.structure_hash)? {
        let positions: BTreeMap<NodeId, Position> =
            serde_json::from_str(&cached).map_err(AppError::internal)?;
        for node in &mut view.nodes {
            node.position = positions.get(&node.id).copied();
        }
    }
    view.layout = Some(key);
    Ok(())
}

/// The layout cache's key for a view: its level and lens, and the node it is focused on, so a
/// view under one node keeps its own layout.
fn view_key(view: &MapView) -> String {
    let base = format!("map:{}:{}", tree::level_name(view.level), view.lens);
    match &view.focus {
        Some(focus) => format!("{base}:{}", focus.0),
        None => base,
    }
}

/// A directory with no scan yet, a workspace or not: an empty map whose next action is the scan,
/// or, with no repos to scan, the same empty state `focus` returns. `seq` is the store's, 0
/// where there is none.
pub(super) fn not_scanned(
    dir: &Path,
    lens: String,
    level: Option<Level>,
    seq: planroom_contract::Seq,
) -> Result<Reply<MapView>, AppError> {
    let mut view = empty_view(level.unwrap_or(Level::System), lens);
    view.empty = Some(scan_empty(dir, &workspace::scannable(dir)));
    Ok(Reply { text: super::text_of(&view, 0, level, None), result: view, seq, warnings: vec![] })
}

/// The top of the map of a workspace a scan wrote nothing for, since git could read none of its
/// repos: the same empty state `map` returns there.
pub(crate) fn not_scanned_view(root: &Path) -> Result<MapView, AppError> {
    Ok(not_scanned(root, ALL_LENSES.into(), None, 0)?.result)
}

/// The empty state of a directory with no scan: map the repos a scan can map, those with a commit.
/// With none, no scan can map anything, so the line says what to do (make a first commit, or see
/// why git can't read them) and the action checks again.
pub(crate) fn scan_empty(root: &Path, repos: &workspace::Scannable) -> Empty {
    let check_again = |reason: &str, line: String| Empty {
        reason: EmptyReason(reason.into()),
        line: Some(line),
        next: Handle {
            command: CommandName("focus".into()),
            args: vec![],
            label: text::label("empty.check_again_next", &[]),
        },
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
        line: Some(text::get("empty.not_scanned", &[("count", &repos.readable)])),
        next: Handle {
            command: CommandName("scan".into()),
            args: vec![root.display().to_string()],
            label: text::label("empty.not_scanned_next", &[("count", &repos.readable)]),
        },
    }
}
