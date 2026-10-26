//! `planroom focus`: the `PaneModel` the pane, the band and the status line draw.
//!
//! In a directory with no store it returns the not-yet-a-workspace empty state and creates
//! nothing (DR-57). Every string comes from `text/en.toml` (DR-21).

use std::path::Path;

use planroom_contract::{
    CommandName, Crumb, DriftLine, Focus, FocusReq, Handle, Level, MiniBlock, MiniMap, NodeId, PaneModel,
    Progress, ScanState, Seq,
};
use planroom_store::Store;

use crate::app::error::AppError;
use crate::app::{Reply, map, workspace};
use crate::{clock, text};

/// The lens the pane shows until lenses load from packs.
const LENS: &str = "all";
/// Blocks the mini-map holds at most.
pub const MINIMAP_BLOCKS: usize = 60;
/// The `PaneModel` stays within this many bytes of JSON.
pub const BUDGET_BYTES: usize = 4_096;
/// How long a presence row counts as present after its last write.
const PRESENT_FOR_MICROS: u64 = 30_000_000;
/// Presence rows the pane carries.
const WHO_ROWS: usize = 3;

pub fn run(_req: &FocusReq, cwd: &Path) -> Result<Reply<PaneModel>, AppError> {
    let Some(root) = workspace::find_root(cwd) else {
        return not_a_workspace(cwd);
    };
    let Some(store) = Store::open_existing(&workspace::store_path(&root))? else {
        return not_a_workspace(&root);
    };
    if store.active_set()?.repos.is_empty() {
        return not_a_workspace(&root);
    }
    let pane = pane(&store, &root)?;
    let seq = pane.focus.seq;
    Ok(Reply { text: text_of(&pane), result: pane, seq, warnings: vec![] })
}

/// The pane of a scanned workspace.
pub fn pane(store: &Store, root: &Path) -> Result<PaneModel, AppError> {
    let seq = store.seq()?;
    let view = map::top(store, root, None)?;
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
            level: Level::System,
            lens: LENS.into(),
            project: None,
            seq,
        },
        decision: None,
        more_decisions: 0,
        progress: Progress { stage: None, done: 0, total: 0, handoffs_running: 0, scan: ScanState::Done },
        diffs: vec![],
        drift: no_drift(),
        who,
        next: Handle {
            command: CommandName("map".into()),
            args: vec!["--level".into(), "container".into()],
            label: text::get("focus.next_containers", &[]),
        },
        minimap: MiniMap { level: Level::System, blocks, edges },
        empty: None,
    };
    // A top with no blocks is the map's own empty state, never "0 at the top".
    if let Some(empty) = view.empty {
        pane.next = empty.next.clone();
        pane.empty = Some(empty);
    }
    fit(&mut pane);
    Ok(pane)
}

/// Drop mini-map blocks, last first, until the pane fits its byte budget.
fn fit(pane: &mut PaneModel) {
    let size = |p: &PaneModel| serde_json::to_string(p).map_or(usize::MAX, |s| s.len());
    while size(pane) > BUDGET_BYTES && pane.minimap.blocks.pop().is_some() {
        let MiniMap { blocks, edges, .. } = &mut pane.minimap;
        edges.retain(|e| blocks.iter().any(|b| b.id == e.from) && blocks.iter().any(|b| b.id == e.to));
    }
}

/// The breadcrumb's first step: the workspace itself, named by its directory.
fn workspace_crumb(root: &Path) -> Crumb {
    let name = root.file_name().and_then(|n| n.to_str()).unwrap_or("workspace").to_owned();
    Crumb { id: NodeId(format!("ws:system:{name}")), name }
}

fn no_drift() -> DriftLine {
    DriftLine { summary: text::get("focus.no_drift", &[]), rules_broken: 0, machine_drift: 0 }
}

/// Not yet a workspace: no store, or a store with no scan. Its next action is the scan of the
/// repos git can read, or, when it can read none, a check again: never a scan that can't map.
fn not_a_workspace(dir: &Path) -> Result<Reply<PaneModel>, AppError> {
    let empty = map::scan_empty(dir, &workspace::scannable(dir));
    let pane = PaneModel {
        focus: Focus { breadcrumb: vec![], level: Level::System, lens: LENS.into(), project: None, seq: 0 },
        decision: None,
        more_decisions: 0,
        progress: Progress { stage: None, done: 0, total: 0, handoffs_running: 0, scan: ScanState::Never },
        diffs: vec![],
        drift: no_drift(),
        who: vec![],
        next: empty.next.clone(),
        minimap: MiniMap { level: Level::System, blocks: vec![], edges: vec![] },
        empty: Some(empty),
    };
    Ok(Reply { text: text_of(&pane), result: pane, seq: 0, warnings: vec![] })
}

fn text_of(pane: &PaneModel) -> String {
    if let Some(empty) = &pane.empty {
        return empty.next.label.clone();
    }
    let name = pane.focus.breadcrumb.iter().map(|c| c.name.as_str()).collect::<Vec<_>>().join(" › ");
    let seq: Seq = pane.focus.seq;
    let blocks: Vec<&str> = pane.minimap.blocks.iter().map(|b| b.name.as_str()).collect();
    [
        text::get(
            "focus.summary",
            &[("name", &name), ("level", &"system"), ("lens", &pane.focus.lens), ("seq", &seq)],
        ),
        text::get("focus.blocks", &[("count", &blocks.len()), ("names", &blocks.join(", "))]),
        pane.drift.summary.clone(),
        text::get("focus.next", &[("label", &pane.next.label)]),
    ]
    .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_large_minimap_is_cut_to_the_budget() {
        let mut pane = not_a_workspace(Path::new("/nonexistent/for/the/focus/test")).unwrap().result;
        pane.focus.breadcrumb = vec![workspace_crumb(Path::new("/w/shop"))];
        pane.minimap.blocks = (0..MINIMAP_BLOCKS)
            .map(|i| MiniBlock {
                id: NodeId(format!("r{i}:repo:{}", "x".repeat(60))),
                name: format!("block {i} {}", "y".repeat(40)),
                focused: false,
            })
            .collect();
        fit(&mut pane);
        assert!(serde_json::to_string(&pane).unwrap().len() <= BUDGET_BYTES);
        assert!(!pane.minimap.blocks.is_empty() && pane.minimap.blocks.len() < MINIMAP_BLOCKS);
        assert_eq!(pane.minimap.blocks[0].name, format!("block 0 {}", "y".repeat(40)));
    }
}
