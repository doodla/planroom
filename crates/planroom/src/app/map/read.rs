//! The map as the store holds it: the fold of one read transaction, at a level or under one
//! node; its empty states; and its cached layout.
//!
//! Everything a reply shows comes from one [`Snapshot`]: the model, folded at the snapshot's
//! `seq` (the process's kept fold caught up to it, `live::model`), with the overlay beside it
//! (what a person hid, and where they pinned a block), the layout cache, and the `seq` the reply
//! carries as `meta.seq`. So a reply's `meta.seq` never runs ahead of the view it folded, and its
//! repos never come from one scan beside a link from another (DR-73).

use std::collections::{BTreeMap, BTreeSet};
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

use planroom_contract::{
    CommandName, Edge, Empty, EmptyReason, Handle, LayoutKey, LevelId, MapView, Node, NodeId, Position,
    Vocabulary,
};
use planroom_kernel::fold::TargetKey;
use planroom_kernel::ladder::Ladder;
use planroom_kernel::plan;
use planroom_kernel::tree;
use planroom_store::{Snapshot, Store};

use crate::app::error::AppError;
use crate::app::map::{focus, lens};
use crate::app::workspace;
use crate::app::{Reply, empty_view};
use crate::packs::state::ReadSet;
use crate::{live, text};

/// The map of the store's active snapshots at the top of the tree, or at `level`, read in one
/// read transaction. A level with no blocks is an `Empty` whose next action works today.
pub fn top(store: &Store, root: &Path, level: Option<LevelId>) -> Result<MapView, AppError> {
    let snapshot = store.snapshot()?;
    let set = crate::packs::state::for_snapshot(&snapshot, root)?;
    top_in(&snapshot, root, level, set.vocabulary())
}

/// [`top`] inside a read transaction the caller holds, so the rest of its reply, such as the
/// pane's `seq`, comes from the same version of the store, folded with `vocabulary`, the set the
/// caller resolved.
pub fn top_in(
    snapshot: &Snapshot<'_>,
    root: &Path,
    level: Option<LevelId>,
    vocabulary: &Vocabulary,
) -> Result<MapView, AppError> {
    let mut view = view(snapshot, root, level, None, vocabulary)?.0;
    view.lens = lens::default_id(vocabulary);
    let lens = view.lens.clone();
    Ok(lens::filter(view, vocabulary, &lens))
}

/// The map in `snapshot`: the blocks at `level` (the ladder's top when it is `None`),
/// or, with `focus`, the blocks under that one node, at `level` or else at the level below the
/// node's own [DR-85], with the node's name. A level with no blocks is an `Empty` whose next
/// action works today.
pub(super) fn view(
    snapshot: &Snapshot<'_>,
    root: &Path,
    level: Option<LevelId>,
    focus: Option<&str>,
    vocabulary: &Vocabulary,
) -> Result<(MapView, Option<String>), AppError> {
    if snapshot.active_set()?.repos.is_empty() {
        let lens = lens::default_id(vocabulary);
        let plan = plan_held(snapshot, vocabulary)?;
        return Ok((not_scanned(root, lens, level, snapshot.seq(), vocabulary, plan)?.result, None));
    }
    let ladder = Ladder::of(vocabulary);
    let (model, overlay) = live::model_and_overlay(snapshot, vocabulary)?;
    let (nodes, edges) = (&model.nodes, &model.edges);
    let under = focus.map(|target| focus::resolve(nodes, target)).transpose()?;
    // A hidden node drops out with everything under it and its edges, and the view counts the
    // blocks that dropped, so a hide shows as a count, never as a silent loss.
    let hidden_ids = hidden_nodes(nodes, &overlay.hidden);
    let visible: Vec<Node> = nodes.iter().filter(|n| !hidden_ids.contains(&n.id)).cloned().collect();
    let visible_edges: Vec<Edge> =
        edges.iter().filter(|e| !overlay.hidden.contains(&TargetKey::Edge(e.id.clone()))).cloned().collect();
    // `map` and `map --level` at the ladder's top are one view, the top of the map: both say they
    // show the top level, so both show the same blocks.
    let shown = match (level, under) {
        (Some(level), _) => level,
        (None, Some(node)) => ladder.below(&node.level).clone(),
        (None, None) => ladder.top().clone(),
    };
    let blocks = tree::blocks_at(&visible, &ladder, &shown, under.map(|n| &n.id));
    let hidden = if hidden_ids.is_empty() {
        0
    } else {
        let all = tree::blocks_at(nodes, &ladder, &shown, under.map(|n| &n.id));
        all.iter().filter(|b| hidden_ids.contains(&b.id)).count() as u32
    };
    let edges = tree::lift_edges(&visible, &visible_edges, &blocks);
    let empty = if !blocks.is_empty() {
        None
    } else if let Some(node) = under {
        let parent = node.parent.as_ref().and_then(|p| nodes.iter().find(|n| &n.id == p));
        Some(focus::nothing_under(node, &shown, parent, &ladder))
    } else if tree::blocks_at(nodes, &ladder, ladder.top(), None).is_empty() {
        // With nothing at the top there is nothing at any level, so every level shows the top's
        // own empty state, which says why.
        Some(top_empty(root, plan_held(snapshot, vocabulary)?))
    } else {
        Some(level_empty(&shown, &ladder))
    };
    let view = MapView {
        level: shown,
        lens: lens::default_id(vocabulary),
        focus: under.map(|n| n.id.clone()),
        nodes: blocks,
        edges,
        hidden,
        empty,
        layout: None,
        levels: vec![],
        yarn: None,
    };
    Ok((view, under.map(|n| n.name.clone())))
}

/// How [`project_without_items`] matches its target.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum ProjectBy {
    /// The project's ID alone, which no block shares.
    Id,
    /// The ID, else the key the ID is built from (`plan:<key>:project`), else the name, whichever
    /// names exactly one project. A caller asks this only where no block answers to the target.
    Any,
}

/// The view of the plan project `target` names when it holds no items: a ghost project is no node
/// of the model, and a live one has nothing under it, so `view` has no block to focus on. The view
/// is empty at `level`, else at the intent level, and carries the project's own empty state, with
/// the project's name; `None` for a project with items, or no single project.
pub(super) fn project_without_items(
    snapshot: &Snapshot<'_>,
    set: &ReadSet,
    target: &str,
    level: Option<&LevelId>,
    by: ProjectBy,
) -> Result<Option<(MapView, Option<String>)>, AppError> {
    let vocabulary = set.vocabulary();
    let folded = live::folded(snapshot, vocabulary)?;
    let projects = plan::projects(&folded);
    let one = |found: Vec<&plan::Project>| match found.as_slice() {
        [only] => Some((*only).clone()),
        _ => None,
    };
    let by_id = projects.iter().find(|p| p.id.0 == target).cloned();
    let chosen = match by {
        ProjectBy::Id => by_id,
        ProjectBy::Any => by_id
            .or_else(|| {
                let id = plan::project_id(target);
                projects.iter().find(|p| p.id == id).cloned()
            })
            .or_else(|| one(projects.iter().filter(|p| p.name == target).collect())),
    };
    let Some(project) = chosen else { return Ok(None) };
    if !plan::items_of(&folded, &project.id).is_empty() {
        return Ok(None);
    }
    let shown = level.cloned().unwrap_or_else(LevelId::intent);
    let mut view = empty_view(shown, lens::default_id(vocabulary));
    view.focus = Some(project.id.clone());
    view.empty = Some(crate::app::intent::project_empty(&project, &crate::app::intent::stages(&set.loaded)));
    Ok(Some((view, Some(project.name.clone()))))
}

/// The nodes a hide takes off the map: each hidden node and every node under it.
fn hidden_nodes(nodes: &[Node], hidden: &BTreeSet<TargetKey>) -> BTreeSet<NodeId> {
    let mut out: BTreeSet<NodeId> = nodes
        .iter()
        .filter(|n| hidden.contains(&TargetKey::Node(n.id.clone())))
        .map(|n| n.id.clone())
        .collect();
    if out.is_empty() {
        return out;
    }
    // Parents before children is not an order the model keeps, so sweep until nothing joins.
    loop {
        let before = out.len();
        for node in nodes {
            if node.parent.as_ref().is_some_and(|p| out.contains(p)) {
                out.insert(node.id.clone());
            }
        }
        if out.len() == before {
            return out;
        }
    }
}

/// The empty state of a scanned workspace's level with no blocks, below a top that has some: the
/// intent level before any plan, or a level this workspace has nothing at. Its next action is the
/// top of the map.
fn level_empty(level: &LevelId, ladder: &Ladder<'_>) -> Empty {
    let (reason, line) = if ladder.is_apart(level) {
        ("no_intent", text::get("empty.no_intent", &[]))
    } else {
        ("empty_level", text::get("empty.empty_level", &[("level", &super::label_of(ladder, level))]))
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
/// says why (the plan is all there is, no commits yet, or git can't read the repos) and the action
/// checks again. `plan` is whether the store holds a project.
fn top_empty(root: &Path, plan: bool) -> Empty {
    let repos = workspace::scannable(root);
    if repos.readable == 0 {
        return scan_empty(root, &repos, plan);
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
pub(super) fn place(
    snapshot: &Snapshot<'_>,
    view: &mut MapView,
    vocabulary: &Vocabulary,
) -> Result<(), AppError> {
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
    // A position a person pinned in this view (`edit pin`) wins over the cached auto-layout.
    let overlay = live::overlay(snapshot, vocabulary)?;
    for node in &mut view.nodes {
        if let Some(pinned) = overlay.positions.get(&(key.view_key.clone(), node.id.clone())) {
            node.position = serde_json::from_value(pinned.clone()).ok().or(node.position);
        }
    }
    view.layout = Some(key);
    Ok(())
}

/// The layout cache's key for a view: its level and lens, and the node it is focused on, so a
/// view under one node keeps its own layout.
fn view_key(view: &MapView) -> String {
    let base = format!("map:{}:{}", view.level.as_str(), view.lens);
    match &view.focus {
        Some(focus) => format!("{base}:{}", focus.0),
        None => base,
    }
}

/// A directory with no scan yet, a workspace or not: an empty map whose next action is the scan,
/// or, with no repos to scan, the same empty state `focus` returns. `seq` is the store's, 0
/// where there is none; `plan` is whether the store holds a project.
pub(super) fn not_scanned(
    dir: &Path,
    lens: String,
    level: Option<LevelId>,
    seq: planroom_contract::Seq,
    vocabulary: &Vocabulary,
    plan: bool,
) -> Result<Reply<MapView>, AppError> {
    let ladder = Ladder::of(vocabulary);
    let shown = level.clone().unwrap_or_else(|| ladder.top().clone());
    let mut view = empty_view(shown, lens);
    view.empty = Some(scan_empty(dir, &workspace::scannable(dir), plan));
    let text = super::text_of(&view, 0, level.as_ref(), None, &ladder);
    Ok(Reply { text, result: view, seq, warnings: vec![] })
}

/// The top of the map of a workspace a scan wrote nothing for, since git could read none of its
/// repos: the same empty state `map` returns there.
pub(crate) fn not_scanned_view(root: &Path, vocabulary: &Vocabulary) -> Result<MapView, AppError> {
    Ok(not_scanned(root, lens::default_id(vocabulary), None, 0, vocabulary, false)?.result)
}

/// The repo names an empty state's line lists: the first [`LINE_NAMES`], each cut to
/// [`NAME_CHARS`] characters, then how many more, so the line stays short whatever the repos are
/// called and every reply that carries it keeps to its budget.
fn listed(names: &[String]) -> String {
    let shown: Vec<String> = names.iter().take(LINE_NAMES).map(|n| text::short(n, NAME_CHARS)).collect();
    let more = names.len() - shown.len();
    if more == 0 {
        shown.join(", ")
    } else {
        text::get("map.more", &[("names", &shown.join(", ")), ("more", &more)])
    }
}

/// Repo names an empty state's line lists before "and N more".
const LINE_NAMES: usize = 3;
/// Characters of a repo's name an empty state's line quotes.
const NAME_CHARS: usize = 40;

/// Whether the store at `snapshot` holds a plan: a project, live or waiting as a proposal.
pub(crate) fn plan_held(snapshot: &Snapshot<'_>, vocabulary: &Vocabulary) -> Result<bool, AppError> {
    Ok(!planroom_kernel::plan::projects(&crate::live::folded(snapshot, vocabulary)?).is_empty())
}

/// The empty state of a directory with no scan: map the repos a scan can map, those with a commit.
/// With none, no scan can map anything, so the state says why, in this order: repos with no commit
/// when no plan is held (`repos_unborn`: a plan's own state is the better answer when one is
/// held); repos git can't read or another user owns (`repos_unreadable`, plan or not, since a plan
/// can't make them mappable); the plan alone (`plan_only`, whose action shows the plan); no repos
/// at all (`no_repos`, whose action starts a plan, or, in a folder below a repo's root, where
/// `intent new` is refused as `scan` is, the scan of that repo, by its root, so it runs from
/// here, when that scan would map it; when it wouldn't, the state that repo's root gives). The
/// first three check again; with a readable repo the state is `not_scanned` and the action is the
/// scan. `plan` is whether the store holds a project.
pub(crate) fn scan_empty(root: &Path, repos: &workspace::Scannable, plan: bool) -> Empty {
    let check_again = |reason: &str, line: String| Empty {
        reason: EmptyReason(reason.into()),
        line: Some(line),
        next: Handle {
            command: CommandName("focus".into()),
            args: vec![],
            label: text::label("empty.check_again_next", &[]),
        },
    };
    if repos.readable == 0 && !plan && !repos.unborn.is_empty() {
        let names = listed(&repos.unborn);
        return check_again("repos_unborn", text::get("empty.repos_unborn", &[("names", &names)]));
    }
    // A repo another user owns is one Planroom never runs git in, so its line names no command to
    // run there; only a repo git itself refuses gets `git status`. A repo the person owns that
    // reads another user's objects is theirs, so its line never says its owner can scan it.
    let refused = [&repos.unreadable, &repos.others, &repos.borrowers];
    if repos.readable == 0 && refused.iter().any(|names| !names.is_empty()) {
        let (names, others) = (listed(&repos.unreadable), listed(&repos.others));
        let borrowers = listed(&repos.borrowers);
        let line = match (repos.unreadable.is_empty(), repos.others.is_empty(), repos.borrowers.is_empty()) {
            (false, true, true) => text::get("empty.repos_unreadable", &[("names", &names)]),
            (true, false, true) => text::get("empty.repos_other_owner", &[("others", &others)]),
            (false, false, true) => {
                text::get("empty.repos_unreadable_other_owner", &[("names", &names), ("others", &others)])
            }
            (true, true, false) => text::get("empty.repos_borrowed", &[("borrowers", &borrowers)]),
            (no_unreadable, no_others, _) => {
                let mut reasons = Vec::new();
                if !no_unreadable {
                    reasons.push(text::get("empty.reason_unreadable", &[("names", &names)]));
                }
                if !no_others {
                    reasons.push(text::get("empty.reason_other_owner", &[("others", &others)]));
                }
                reasons.push(text::get("empty.reason_borrowed", &[("borrowers", &borrowers)]));
                text::get("empty.repos_mixed", &[("reasons", &reasons.join(" "))])
            }
        };
        return check_again("repos_unreadable", line);
    }
    if repos.readable == 0 && plan {
        return Empty {
            reason: EmptyReason("plan_only".into()),
            line: Some(text::get("empty.plan_only", &[])),
            next: Handle {
                command: CommandName("intent list".into()),
                args: vec![],
                label: text::label("empty.plan_only_next", &[]),
            },
        };
    }
    if let Some(repo) = (repos.readable == 0).then(|| workspace::enclosing_repo(root)).flatten() {
        // A scan of that repo maps nothing while it has no commit or git can't read it, and makes
        // no store, so the folder says what the repo's root says; that repo has no store either,
        // or the folder would lie in its workspace, so it holds no plan.
        let own = workspace::scannable_at(&repo);
        if own.readable == 0 {
            return scan_empty(&repo, &own, false);
        }
        let name = repo.file_name().map_or_else(String::new, |n| workspace::escape(n.as_bytes()));
        return Empty {
            reason: EmptyReason("no_repos".into()),
            line: Some(text::get("empty.in_repo", &[("name", &listed(&[name]))])),
            next: Handle {
                command: CommandName("scan".into()),
                args: vec![repo.display().to_string()],
                label: text::label("empty.in_repo_next", &[]),
            },
        };
    }
    if repos.readable == 0 {
        return Empty {
            reason: EmptyReason("no_repos".into()),
            line: Some(text::get("empty.no_repos", &[])),
            next: Handle {
                command: CommandName("schema".into()),
                args: vec!["--command".into(), "intent new".into()],
                label: text::label("empty.no_repos_next", &[]),
            },
        };
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
