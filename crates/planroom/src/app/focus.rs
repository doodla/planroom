//! `planroom focus`: the `PaneModel` the pane, the band and the status line draw.
//!
//! In a directory with no store it returns the not-yet-a-workspace empty state and creates
//! nothing (DR-57). In a scanned one it lists the open cards a write dealt, the first face up in
//! `decision` and the rest counted in `more_decisions`, and deals none itself (DR-89); while the
//! workspace's trust card is open it carries `workspace_untrusted`. Every string
//! comes from `text/en.toml` (DR-21).

use std::path::Path;

use planroom_contract::EventBody;
use planroom_contract::{
    CommandName, Crumb, DriftLine, Focus, FocusDetail, FocusReq, Handle, MiniBlock, MiniMap, NodeId,
    PaneModel, Progress, ScanState, Seq, Vocabulary, Warning, WarningCode,
};
use planroom_kernel::ladder::Ladder;
use planroom_store::{Snapshot, Store};
use serde_json::json;

use crate::app::dirty::Dirty;
use crate::app::error::AppError;
use crate::app::{Paged, Reply, cards, map, workspace};
use crate::packs::state::{self, ReadSet};
use crate::{clock, live, text};

/// Blocks the mini-map holds at most: the pane's cap. On the CLI the reply then keeps to `focus`'s
/// catalogue budget, as sent, by dropping blocks with [`drop_block`]; over HTTP and in a share's
/// snapshot the pane is whole.
pub const MINIMAP_BLOCKS: usize = 60;
/// How long a presence row counts as present after its last write.
const PRESENT_FOR_MICROS: u64 = 30_000_000;
/// Presence rows the pane carries.
const WHO_ROWS: usize = 3;

/// The pane, and in a scanned workspace the repos with uncommitted work, which `meta.dirty_repos`
/// lists and one line of the text names, with no rescan (DR-71, DR-85). `--detail summary` builds
/// the same pane and text, then leaves the mini-map out of the reply on every path (DR-82), so
/// text mode still names the blocks.
pub fn run(req: &FocusReq, cwd: &Path) -> Result<Paged<PaneModel>, AppError> {
    let mut paged = whole_detail(cwd)?;
    if req.detail == FocusDetail::Summary {
        paged.reply.result.minimap = None;
    }
    Ok(paged)
}

/// [`run`] at full detail.
fn whole_detail(cwd: &Path) -> Result<Paged<PaneModel>, AppError> {
    let Some(root) = workspace::find_root(cwd) else {
        return not_a_workspace(cwd).map(Paged::whole);
    };
    let Some(store) = Store::open_existing(&workspace::store_path(&root))? else {
        return not_a_workspace(&root).map(Paged::whole);
    };
    let snapshot = store.snapshot()?;
    let set = state::for_snapshot(&snapshot, &root)?;
    let mut reply = reply_with(&store, &snapshot, &root, &set)?;
    if reply.result.empty.is_some() {
        return Ok(Paged::whole(reply));
    }
    let dirty = Dirty::of_root(&root);
    reply.text = text_of(&reply.result, dirty.line(), set.vocabulary());
    Ok(Paged { dirty, ..Paged::whole(reply) })
}

/// `focus` inside a read transaction the caller holds on `store`, so the rest of what it builds,
/// such as `share`'s map, comes from the same version of the store as the pane. A repo whose last
/// scan didn't cover every file keeps its `partial_scan` warning here, so the pane never shows a
/// partly mapped repo as whole.
pub fn reply_in(store: &Store, snapshot: &Snapshot<'_>, root: &Path) -> Result<Reply<PaneModel>, AppError> {
    reply_with(store, snapshot, root, &state::for_snapshot(snapshot, root)?)
}

/// [`reply_in`] with the pack set the caller resolved from `snapshot`: the pane is folded with it,
/// and a set that didn't load says so in the reply's warnings.
fn reply_with(
    store: &Store,
    snapshot: &Snapshot<'_>,
    root: &Path,
    set: &ReadSet,
) -> Result<Reply<PaneModel>, AppError> {
    if snapshot.active_set()?.repos.is_empty() {
        return not_a_workspace(root);
    }
    let pane = pane_in(store, snapshot, root, set.vocabulary())?;
    let seq = pane.focus.seq;
    let mut warnings = partly_mapped(snapshot)?;
    // While the trust card is open, the workspace's config stays unloaded, and the pane says so
    // from the store alone (DR-89).
    if let Some(open) = cards::open_trust(snapshot)? {
        warnings.push(cards::untrusted_warning(&open.event.id, &open.card));
    }
    set.warn(&mut warnings);
    Ok(Reply { text: text_of(&pane, None, set.vocabulary()), result: pane, seq, warnings })
}

/// A `partial_scan` warning for each repo whose active snapshot's scan left files uncovered: the
/// count its scan event kept. The paths are in that scan's reply alone, so `uncovered` is empty.
fn partly_mapped(snapshot: &Snapshot<'_>) -> Result<Vec<Warning>, AppError> {
    let mut warnings = Vec::new();
    for event in snapshot.active_scans()? {
        let EventBody::Scan { repo, stats, .. } = event.body else { continue };
        if stats.uncovered == 0 {
            continue;
        }
        let extractor = planroom_extract::files::NAME;
        warnings.push(Warning {
            code: WarningCode::PartialScan,
            message: text::get(
                "warning.partial_scan",
                &[("extractor", &extractor), ("repo", &repo), ("count", &stats.uncovered)],
            ),
            details: json!({"extractor": extractor, "repo": repo, "count": stats.uncovered, "uncovered": []}),
        });
    }
    Ok(warnings)
}

/// The pane of a scanned workspace.
pub fn pane(store: &Store, root: &Path) -> Result<PaneModel, AppError> {
    let snapshot = store.snapshot()?;
    let set = state::for_snapshot(&snapshot, root)?;
    pane_in(store, &snapshot, root, set.vocabulary())
}

/// [`pane`] inside a read transaction on `store`: the pane's seq and its mini-map come from that
/// one version of the store (DR-73), folded with `vocabulary`.
fn pane_in(
    store: &Store,
    snapshot: &Snapshot<'_>,
    root: &Path,
    vocabulary: &Vocabulary,
) -> Result<PaneModel, AppError> {
    let seq = snapshot.seq();
    let view = map::top_in(snapshot, root, None, vocabulary)?;
    let next = next_in(snapshot, vocabulary)?;
    let top = Ladder::of(vocabulary).top().clone();
    let folded = live::folded(snapshot, vocabulary)?;
    let (decision, more_decisions) = cards::face_up(snapshot, &folded)?;
    let since = clock::rfc3339(clock::unix_micros().saturating_sub(PRESENT_FOR_MICROS));
    let mut who = store.presence_since(&planroom_contract::Timestamp(since))?;
    who.truncate(WHO_ROWS);
    let blocks: Vec<MiniBlock> = view
        .nodes
        .iter()
        .take(MINIMAP_BLOCKS)
        .map(|n| MiniBlock { id: n.id.clone(), name: n.name.clone(), focused: false })
        .collect();
    let edges = view
        .edges
        .iter()
        .filter(|e| blocks.iter().any(|b| b.id == e.from) && blocks.iter().any(|b| b.id == e.to))
        .map(|e| planroom_contract::MiniEdge { from: e.from.clone(), to: e.to.clone(), count: e.count })
        .collect();
    let mut pane = PaneModel {
        focus: Focus {
            breadcrumb: vec![workspace_crumb(root)],
            level: top.clone(),
            lens: view.lens.clone(),
            project: None,
            seq,
        },
        decision,
        more_decisions,
        progress: Progress { stage: None, done: 0, total: 0, handoffs_running: 0, scan: ScanState::Done },
        diffs: vec![],
        drift: no_drift(),
        who,
        next,
        minimap: Some(MiniMap { level: top, blocks, edges }),
        empty: None,
    };
    // A top with no blocks is the map's own empty state, never "0 at the top".
    if let Some(empty) = view.empty {
        pane.next = empty.next.clone();
        pane.empty = Some(empty);
    }
    Ok(pane)
}

/// The one next action at the top: the loaded packs' highest-priority suggestion whose `when`
/// holds on the model this pane reads (DR-54), its text the button's label. The loader refused
/// every suggestion that names a leaf the band may not, so this one can be undone, hands nothing
/// off and never spends. With none that holds, the top of the map.
fn next_in(snapshot: &Snapshot<'_>, vocabulary: &Vocabulary) -> Result<Handle, AppError> {
    let model = live::model(snapshot, vocabulary)?;
    Ok(match planroom_kernel::next::next(&model, &vocabulary.suggestions) {
        Some(suggestion) => Handle {
            command: suggestion.action.command.clone(),
            args: suggestion.action.args.clone(),
            label: suggestion.text.clone(),
        },
        None => Handle {
            command: CommandName("map".into()),
            args: vec![],
            label: text::label("empty.top_next", &[]),
        },
    })
}

/// Drop the mini-map's last block and every edge whose end it was; false when there was none to
/// drop, as with no mini-map. The CLI calls it until the reply it sends fits `focus`'s budget.
pub fn drop_block(pane: &mut PaneModel) -> bool {
    let Some(MiniMap { blocks, edges, .. }) = pane.minimap.as_mut() else { return false };
    if blocks.pop().is_none() {
        return false;
    }
    edges.retain(|e| blocks.iter().any(|b| b.id == e.from) && blocks.iter().any(|b| b.id == e.to));
    true
}

/// The breadcrumb's first step: the workspace itself, named by its directory.
fn workspace_crumb(root: &Path) -> Crumb {
    let name = root.file_name().and_then(|n| n.to_str()).unwrap_or("workspace").to_owned();
    Crumb { id: NodeId(format!("ws:system:{name}")), name }
}

fn no_drift() -> DriftLine {
    // No drift is computed yet, so there is no top item for a row key to select.
    DriftLine { summary: text::get("focus.no_drift", &[]), rules_broken: 0, machine_drift: 0, top: None }
}

/// Not yet a workspace: no store, or a store with no scan. Its next action is the scan of the
/// repos git can read, or, when it can read none, a check again: never a scan that can't map.
fn not_a_workspace(dir: &Path) -> Result<Reply<PaneModel>, AppError> {
    let empty = map::scan_empty(dir, &workspace::scannable(dir));
    let vocabulary = crate::packs::vocabulary()?;
    let top = Ladder::of(vocabulary).top().clone();
    let pane = PaneModel {
        focus: Focus {
            breadcrumb: vec![],
            level: top.clone(),
            lens: map::lens::default_id(vocabulary),
            project: None,
            seq: 0,
        },
        decision: None,
        more_decisions: 0,
        progress: Progress { stage: None, done: 0, total: 0, handoffs_running: 0, scan: ScanState::Never },
        diffs: vec![],
        drift: no_drift(),
        who: vec![],
        next: empty.next.clone(),
        minimap: Some(MiniMap { level: top, blocks: vec![], edges: vec![] }),
        empty: Some(empty),
    };
    Ok(Reply { text: text_of(&pane, None, vocabulary), result: pane, seq: 0, warnings: vec![] })
}

/// The pane as text: where you are, the top's blocks, the repos with uncommitted work when there
/// are any, the drift line and the next action. The pane shows the ladder's top, by its label.
fn text_of(pane: &PaneModel, dirty: Option<String>, vocabulary: &Vocabulary) -> String {
    if let Some(empty) = &pane.empty {
        return empty.line.clone().unwrap_or_else(|| empty.next.label.clone());
    }
    let name = pane.focus.breadcrumb.iter().map(|c| c.name.as_str()).collect::<Vec<_>>().join(" › ");
    let seq: Seq = pane.focus.seq;
    let blocks: Vec<&str> = pane.minimap.iter().flat_map(|m| &m.blocks).map(|b| b.name.as_str()).collect();
    [
        text::get(
            "focus.summary",
            &[
                ("name", &name),
                ("level", &map::label_of(&Ladder::of(vocabulary), &pane.focus.level)),
                ("lens", &pane.focus.lens),
                ("seq", &seq),
            ],
        ),
        text::get("focus.blocks", &[("count", &blocks.len()), ("names", &blocks.join(", "))]),
    ]
    .into_iter()
    .chain(dirty)
    .chain(pane.decision.as_ref().map(|d| text::get("card.focus_line", &[("question", &d.question)])))
    .chain([pane.drift.summary.clone(), text::get("focus.next", &[("label", &pane.next.label)])])
    .collect::<Vec<_>>()
    .join("\n")
}

#[cfg(test)]
mod tests {
    use planroom_contract::MiniEdge;

    use super::*;

    fn block(i: usize) -> MiniBlock {
        MiniBlock { id: NodeId(format!("r{i}:repo:r{i}")), name: format!("block {i}"), focused: false }
    }

    fn edge(from: usize, to: usize) -> MiniEdge {
        MiniEdge { from: block(from).id, to: block(to).id, count: 1 }
    }

    #[test]
    fn drop_block_pops_the_last_block_and_its_edges() {
        let mut pane = not_a_workspace(Path::new("/nonexistent/for/the/focus/test")).unwrap().result;
        pane.minimap = Some(MiniMap {
            level: "system".into(),
            blocks: (0..3).map(block).collect(),
            edges: vec![edge(0, 1), edge(1, 2), edge(2, 0)],
        });
        assert!(drop_block(&mut pane));
        let map = pane.minimap.as_ref().unwrap();
        assert_eq!(map.blocks, vec![block(0), block(1)]);
        assert_eq!(map.edges, vec![edge(0, 1)]);
        assert!(drop_block(&mut pane) && drop_block(&mut pane));
        assert!(
            pane.minimap.as_ref().unwrap().blocks.is_empty()
                && pane.minimap.as_ref().unwrap().edges.is_empty()
        );
        // Nothing left to drop, or no mini-map at all: it says so, and the CLI stops.
        assert!(!drop_block(&mut pane));
        pane.minimap = None;
        assert!(!drop_block(&mut pane));
    }
}
