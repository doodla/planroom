//! A map cut to fit: a page after a cursor, and the blocks that fit the leaf's budget.

use std::collections::HashSet;

use planroom_contract::{
    CommandName, Cursor, Handle, Level, LiftedEdge, MapNode, MapView, NodeId, Warning, WarningCode,
};
use serde_json::json;

use crate::text;
use planroom_kernel::tree;

/// What the envelope around a map takes besides the view: its other fields, the summary line and
/// the `truncated` warning. The view gets the rest of the budget.
const ENVELOPE_RESERVE_BYTES: usize = 1_536;

/// The page of `view` after `cursor`, within `budget_tokens`, and the cursor of the page after it.
/// The map doesn't page yet, so the page is the whole view and no page follows it.
pub fn cut(view: MapView, cursor: Option<&Cursor>, budget_tokens: Option<u32>) -> (MapView, Option<Cursor>) {
    let _ = (cursor, budget_tokens);
    (view, None)
}

/// Keep the blocks that fit `budget_tokens`, in their order, and the lifted edges between them.
/// Returns the `truncated` warning when it cut anything. `scan`'s reply carries a map too.
pub(crate) fn fit(view: &mut MapView, budget_tokens: u32, level: Option<Level>) -> Option<Warning> {
    fit_beside(view, budget_tokens, level, &[])
}

/// [`fit`], leaving room for the warnings the reply carries beside the map, such as `scan`'s
/// `partial_scan`: at most half the budget, so the map keeps a first page however many there are.
/// The envelope's own check summarises warnings past the rest (`cli::fit_warnings`).
pub(crate) fn fit_beside(
    view: &mut MapView,
    budget_tokens: u32,
    level: Option<Level>,
    beside: &[Warning],
) -> Option<Warning> {
    let whole = (budget_tokens as usize * 4).saturating_sub(ENVELOPE_RESERVE_BYTES);
    let warnings = serde_json::to_string(beside).map_or(0, |s| s.len());
    let limit = whole - warnings.min(whole / 2);
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
