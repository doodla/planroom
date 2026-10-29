//! A map cut to fit: a page after a cursor, and the blocks that fit the leaf's budget.
//!
//! Pages are cut in a stable order (architecture.md, The CLI: Pagination): by lens group, then
//! level, then ID. Group 0 is the blocks of the lens's kinds, when its `kinds` is non-empty, and
//! group 1 every other block, so with no kinds the order is level, then ID. A map over its leaf's
//! budget gives its first page, the blocks that fit taken in that order, so the lens's kinds'
//! blocks reach it first. Each page lists its blocks in the lens's display order (its kinds
//! first, then by lifted edges, then level, name, ID, as `kernel::lens::apply` orders them), so a
//! map that fits on one page reads as it always has; ranking by lifted edges orders blocks within
//! a page only, since lifted-edge counts change between versions and a cursor stays exact.
//! `meta.next` is an opaque cursor over the `seq` the page read, a hash of the request's filters,
//! and the group, level and ID of the page's last block in cut order; `--cursor` continues with
//! the first block after it. Each lifted edge lands on the page that gives the later of its two
//! blocks in cut order, so the pages joined hold every block and every edge exactly once.
//!
//! A cursor from another request's filters is `invalid_argument`. One whose `seq` is behind reads
//! the current map, warns `stale_page` and continues the same way. A block whose ID, level and
//! kind are the same in both versions keeps its place in cut order, whatever names and edges
//! changed and whatever rows came or went, so it is given exactly once across the pages (DR-73);
//! a block whose level or kind changed is a changed row and may move. A cursor with no block
//! after it gives the empty state `nothing_after_cursor`.
//!
//! The cursor carries no name: names are repo-derived text and redacted, and a cursor never is.
//! An ID is never redacted either, so the cursor holds one.

use std::collections::HashSet;

use planroom_contract::{
    CommandName, Cursor, Detail, Empty, EmptyReason, Handle, Kind, LevelId, LiftedEdge, MapNode, MapReq,
    MapView, NodeId, Seq, Warning, WarningCode,
};
use planroom_kernel::keys;
use planroom_kernel::ladder::Ladder;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::app::error::AppError;
use crate::text;

/// What the envelope around a map takes besides the view: its other fields, the summary line and
/// the `truncated` warning. The view gets the rest of the budget.
const ENVELOPE_RESERVE_BYTES: usize = 1_536;

/// One page of a map: the blocks it gives and the edges that land on it, the cursor of the page
/// after it, how many blocks the whole map has, and the warnings that say so.
pub(crate) struct Page {
    pub view: MapView,
    pub next: Option<Cursor>,
    pub total: usize,
    pub warnings: Vec<Warning>,
}

/// What a cursor holds: the `seq` its page read, the filters hash, and the lens group, level and
/// ID of the page's last block in cut order. Never a name.
#[derive(Serialize, Deserialize)]
struct Mark {
    #[serde(rename = "s")]
    seq: Seq,
    #[serde(rename = "f")]
    filters: String,
    #[serde(rename = "g")]
    group: u8,
    #[serde(rename = "l")]
    level: LevelId,
    #[serde(rename = "i")]
    id: NodeId,
}

/// A map's blocks in cut order, by lens group, level and ID, each with its place in the map's own
/// order.
type Ordered = Vec<(usize, MapNode)>;

/// A block's lens group: 0 for a block of one of `about`, the kinds the lens is about, and 1 for
/// every other block. With no kinds, every block is in group 1.
fn group(node: &MapNode, about: &[Kind]) -> u8 {
    u8::from(!about.contains(&node.kind))
}

/// Where `node` sits in cut order: its lens group, its level's rank on the ladder and its ID, all
/// of which a cursor holds.
fn key<'a>(node: &'a MapNode, about: &[Kind], ladder: &Ladder<'_>) -> (u8, u8, &'a NodeId) {
    (group(node, about), ladder.rank(&node.level), &node.id)
}

/// `nodes`, which come in the map's own order, sorted into cut order.
fn in_cut_order(nodes: Vec<MapNode>, about: &[Kind], ladder: &Ladder<'_>) -> Ordered {
    let mut ordered: Ordered = nodes.into_iter().enumerate().collect();
    ordered.sort_by(|(_, a), (_, b)| key(a, about, ladder).cmp(&key(b, about, ladder)));
    ordered
}

/// The page that gives `shown`, blocks in cut order, after the blocks in `given`: `shown` listed
/// in the map's own order, which is the lens's display order, and the lifted edges that land on
/// it.
fn page_of(
    shown: &[(usize, MapNode)],
    given: &HashSet<NodeId>,
    edges: &[LiftedEdge],
) -> (Vec<MapNode>, Vec<LiftedEdge>) {
    let mut listed: Vec<&(usize, MapNode)> = shown.iter().collect();
    listed.sort_by_key(|(place, _)| *place);
    let listed: Vec<MapNode> = listed.into_iter().map(|(_, n)| n.clone()).collect();
    let edges = landing(edges, given, &listed);
    (listed, edges)
}

/// The page of `view`, which the store read at `seq`, after `req`'s cursor, within
/// `budget_tokens`; with no budget, everything after the cursor. `about` is the kinds the view's
/// lens is about, whose blocks come first in cut order, and `ladder` orders the levels.
pub(crate) fn cut(
    view: MapView,
    req: &MapReq,
    seq: Seq,
    budget_tokens: Option<u32>,
    about: &[Kind],
    ladder: &Ladder<'_>,
) -> Result<Page, AppError> {
    let total = view.nodes.len();
    let filters = filters_hash(req, &view.lens);
    let mut warnings = Vec::new();
    let mut view = view;
    let ordered = in_cut_order(std::mem::take(&mut view.nodes), about, ladder);
    let edges = std::mem::take(&mut view.edges);

    // Where this page starts in cut order: the blocks before it are the ones earlier pages gave.
    let start = match req.cursor.as_ref() {
        None => 0,
        Some(cursor) => {
            let mark = decode(cursor)?;
            if mark.filters != filters {
                return Err(AppError::invalid_argument("cursor", "argument.cursor_filters"));
            }
            if mark.seq != seq {
                warnings.push(stale_page());
            }
            let at = (mark.group, ladder.rank(&mark.level), &mark.id);
            ordered.partition_point(|(_, n)| key(n, about, ladder) <= at)
        }
    };
    let given: HashSet<NodeId> = ordered[..start].iter().map(|(_, n)| n.id.clone()).collect();
    let rest = &ordered[start..];
    // A cursor with no block after it, as when the map moved on and dropped every block after
    // it, still gives a page that says what to do next.
    if req.cursor.is_some() && rest.is_empty() && view.empty.is_none() {
        view.empty = Some(past_end(req));
    }

    let page = |count: usize| page_of(&rest[..count], &given, &edges);
    let Some(budget) = budget_tokens else {
        (view.nodes, view.edges) = page(rest.len());
        return Ok(Page { view, next: None, total, warnings });
    };

    // The cursor rides in `meta.next` and in the warning's line; leave room for the longest. A
    // page that ends at the last block has no cursor.
    let cursor_bytes = rest[..rest.len().saturating_sub(1)]
        .iter()
        .map(|(_, n)| 2 * mark_json(n, seq, &filters, about).len())
        .max()
        .unwrap_or(0);
    let limit = limit(budget, &warnings).saturating_sub(2 * cursor_bytes);
    let size = |v: &MapView| serde_json::to_string(v).map_or(usize::MAX, |s| s.len());
    (view.nodes, view.edges) = page(rest.len());
    if size(&view) <= limit {
        return Ok(Page { view, next: None, total, warnings });
    }
    // The size grows with the number of blocks kept, so the largest prefix that fits is a search.
    // A page holds at least one block, so every page moves the cursor on.
    let (mut fits, mut over) = (1, rest.len());
    while fits < over {
        let mid = (fits + over).div_ceil(2);
        (view.nodes, view.edges) = page(mid);
        if size(&view) <= limit { fits = mid } else { over = mid - 1 }
    }
    (view.nodes, view.edges) = page(fits);
    if fits == rest.len() {
        return Ok(Page { view, next: None, total, warnings });
    }
    let next = mark_after(&rest[fits - 1].1, seq, &filters, about);
    let drill = drill_down(req.level.as_ref().unwrap_or(&view.level), req.level.is_none(), ladder);
    let message = text::get(
        "warning.paged",
        &[
            ("total", &total),
            ("kept", &fits),
            ("budget", &budget),
            ("cursor", &next.0),
            ("label", &drill.label),
        ],
    );
    warnings.push(Warning {
        code: WarningCode::Truncated,
        message,
        details: json!({"kept": fits, "total": total, "budget_tokens": budget, "next": drill}),
    });
    Ok(Page { view, next: Some(next), total, warnings })
}

/// The empty state of a page after a cursor with no block after it: the map's first page with
/// the same flags.
fn past_end(req: &MapReq) -> Empty {
    let mut args = Vec::new();
    if let Some(level) = &req.level {
        args.extend(["--level".to_owned(), level.as_str().to_owned()]);
    }
    if let Some(lens) = &req.lens {
        args.extend(["--lens".to_owned(), lens.clone()]);
    }
    if let Some(focus) = &req.focus {
        args.extend(["--focus".to_owned(), focus.clone()]);
    }
    if req.detail == Detail::Graph {
        args.extend(["--detail".to_owned(), "graph".to_owned()]);
    }
    Empty {
        reason: EmptyReason("nothing_after_cursor".into()),
        line: Some(text::get("empty.nothing_after_cursor", &[])),
        next: Handle {
            command: CommandName("map".into()),
            args,
            label: text::label("empty.nothing_after_cursor_next", &[]),
        },
    }
}

/// The lifted edges that land on a page giving `shown` after the blocks in `given`: both ends
/// given by now, and at least one of them on this page.
fn landing(edges: &[LiftedEdge], given: &HashSet<NodeId>, shown: &[MapNode]) -> Vec<LiftedEdge> {
    let here: HashSet<&NodeId> = shown.iter().map(|n| &n.id).collect();
    let by_now = |id: &NodeId| here.contains(id) || given.contains(id);
    edges
        .iter()
        .filter(|e| by_now(&e.from) && by_now(&e.to) && (here.contains(&e.from) || here.contains(&e.to)))
        .cloned()
        .collect()
}

/// A hash of what selects and shapes a map, so a cursor continues only the map it came from.
fn filters_hash(req: &MapReq, lens: &str) -> String {
    let level = req.level.as_ref().map(LevelId::as_str).unwrap_or_default();
    let focus = req.focus.as_deref().unwrap_or_default();
    let detail = match req.detail {
        Detail::Summary => "summary",
        Detail::Graph => "graph",
    };
    let mut hash = keys::sha256_hex(&["map", level, lens, focus, detail]);
    hash.truncate(16);
    hash
}

/// The cursor of the page after one whose last block in cut order is `last`, in the map read at
/// `seq`: its mark as JSON, hex-encoded, so it passes as one shell word and no redaction pass
/// mistakes it for a secret.
fn mark_after(last: &MapNode, seq: Seq, filters: &str, about: &[Kind]) -> Cursor {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let json = mark_json(last, seq, filters, about);
    Cursor(
        json.iter()
            .flat_map(|b| [HEX[usize::from(b >> 4)], HEX[usize::from(b & 15)]])
            .map(char::from)
            .collect(),
    )
}

/// The mark of the page after one whose last block in cut order is `last`, as JSON.
fn mark_json(last: &MapNode, seq: Seq, filters: &str, about: &[Kind]) -> Vec<u8> {
    let mark = Mark {
        seq,
        filters: filters.into(),
        group: group(last, about),
        level: last.level.clone(),
        id: last.id.clone(),
    };
    serde_json::to_vec(&mark).expect("a mark serializes")
}

/// The mark a cursor holds, or `invalid_argument` for one no map gave.
fn decode(cursor: &Cursor) -> Result<Mark, AppError> {
    let unreadable = || AppError::invalid_argument("cursor", "argument.cursor_unreadable");
    let hex = cursor.0.as_bytes();
    if !hex.len().is_multiple_of(2) {
        return Err(unreadable());
    }
    let digit = |c: u8| (c as char).to_digit(16);
    let bytes: Option<Vec<u8>> =
        hex.chunks(2).map(|pair| Some((digit(pair[0])? * 16 + digit(pair[1])?) as u8)).collect();
    let bytes = bytes.ok_or_else(unreadable)?;
    serde_json::from_slice(&bytes).map_err(|_| unreadable())
}

fn stale_page() -> Warning {
    Warning {
        code: WarningCode::StalePage,
        message: text::get("warning.stale_page", &[]),
        details: json!({}),
    }
}

/// The bytes a view may take within `budget_tokens`, beside the warnings the reply carries: at
/// most half the budget goes to them, so the map keeps a first page however many there are.
fn limit(budget_tokens: u32, beside: &[Warning]) -> usize {
    let whole = (budget_tokens as usize * 4).saturating_sub(ENVELOPE_RESERVE_BYTES);
    let warnings = serde_json::to_string(beside).map_or(0, |s| s.len());
    whole - warnings.min(whole / 2)
}

/// Keep the map's first page within `budget_tokens`: the blocks that fit, taken in cut order and
/// listed in the lens's display order, as `map`'s first page takes them, and the lifted edges between
/// them, leaving room for the warnings the reply carries beside the map, such as `scan`'s
/// `partial_scan`. Returns the `truncated` warning when it cut anything. `scan`'s reply carries
/// the top-level map this way, with no cursor; the envelope's own check summarises warnings past
/// the rest (`cli::fit_warnings`). `about` is the kinds the view's lens is about, as for [`cut`].
pub(crate) fn fit_beside(
    view: &mut MapView,
    budget_tokens: u32,
    level: Option<&LevelId>,
    beside: &[Warning],
    about: &[Kind],
    ladder: &Ladder<'_>,
) -> Option<Warning> {
    let limit = limit(budget_tokens, beside);
    let size = |v: &MapView| serde_json::to_string(v).map_or(usize::MAX, |s| s.len());
    if size(view) <= limit {
        return None;
    }
    let ordered = in_cut_order(std::mem::take(&mut view.nodes), about, ladder);
    let edges = std::mem::take(&mut view.edges);
    let total = ordered.len();
    let first = |count: usize| page_of(&ordered[..count], &HashSet::new(), &edges);
    // The size grows with the number of blocks kept, so the largest prefix that fits is a search.
    let (mut fits, mut over) = (0, total);
    while fits < over {
        let mid = (fits + over).div_ceil(2);
        (view.nodes, view.edges) = first(mid);
        if size(view) <= limit { fits = mid } else { over = mid - 1 }
    }
    (view.nodes, view.edges) = first(fits);
    let next = drill_down(level.unwrap_or(&view.level), level.is_none(), ladder);
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

/// Where to go from a map cut at its budget, with what `map` and `focus` serve today: the next
/// coarser level on the ladder, whose blocks hold these; from the top, the pane's summary.
fn drill_down(level: &LevelId, at_top: bool, ladder: &Ladder<'_>) -> Handle {
    match ladder.above(level).filter(|_| !at_top) {
        Some(coarser) => Handle {
            command: CommandName("map".into()),
            args: vec!["--level".into(), coarser.as_str().into()],
            label: text::label("map.coarser", &[("level", &super::label_of(ladder, coarser))]),
        },
        None => {
            Handle { command: CommandName("focus".into()), args: vec![], label: text::label("map.pane", &[]) }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use std::sync::LazyLock;

    use planroom_contract::Vocabulary;

    use super::*;

    /// No vocabulary: every block here sits at one level the ladder doesn't hold, so cut order is
    /// the ID's.
    static VOCABULARY: LazyLock<Vocabulary> = LazyLock::new(Vocabulary::default);

    fn ladder() -> Ladder<'static> {
        Ladder::of(&VOCABULARY)
    }

    fn node(id: &str, name: &str) -> MapNode {
        MapNode {
            id: NodeId(id.into()),
            kind: planroom_contract::Kind("test.block".into()),
            tone: None,
            name: name.into(),
            level: LevelId::from("code"),
            parent: None,
            counts: Default::default(),
            position: None,
            ghost: false,
            claimed_by: None,
        }
    }

    fn edge(from: usize, to: usize) -> LiftedEdge {
        LiftedEdge {
            kind: planroom_contract::Kind("test.uses".into()),
            from: NodeId(format!("a:file:{from:03}")),
            to: NodeId(format!("a:file:{to:03}")),
            count: 1,
            evidence_sample: vec![],
            children: vec![],
        }
    }

    /// A code-level map of `names`, sorted as `kernel::tree` sorts blocks, with `edges`.
    fn view(names: &[&str], edges: Vec<LiftedEdge>) -> MapView {
        let mut view = crate::app::empty_view(LevelId::from("code"), "all".into());
        view.nodes = names.iter().map(|n| node(&format!("a:file:{n}"), &format!("block {n}"))).collect();
        view.nodes.sort_by(|a, b| {
            (ladder().rank(&a.level), &a.name, &a.id).cmp(&(ladder().rank(&b.level), &b.name, &b.id))
        });
        view.edges = edges;
        view
    }

    fn numbered(count: usize) -> Vec<String> {
        (0..count).map(|i| format!("{i:03}")).collect()
    }

    /// Every page from `req`'s cursor on, each read with `read`.
    fn follow(req: MapReq, read: impl Fn() -> (MapView, Seq)) -> Vec<Page> {
        follow_through(req, &[], read)
    }

    /// [`follow`] through a lens about the kinds `about`.
    fn follow_through(mut req: MapReq, about: &[Kind], read: impl Fn() -> (MapView, Seq)) -> Vec<Page> {
        let mut pages = Vec::new();
        loop {
            let (view, seq) = read();
            let page = cut(view, &req, seq, Some(1_500), about, &ladder()).unwrap();
            req.cursor = page.next.clone();
            pages.push(page);
            if req.cursor.is_none() {
                return pages;
            }
            assert!(pages.len() < 100, "the cursor moves on");
        }
    }

    #[test]
    fn the_pages_joined_hold_every_block_and_every_edge_once() {
        let names = numbered(120);
        let names: Vec<&str> = names.iter().map(String::as_str).collect();
        // A chain, and a hub every block points back to, so edges cross every page.
        let edges: Vec<LiftedEdge> =
            (0..119).map(|i| edge(i, i + 1)).chain((1..120).map(|i| edge(i, 0))).collect();
        let whole = view(&names, edges.clone());
        let pages = follow(MapReq::default(), || (whole.clone(), 4));
        assert!(pages.len() > 2, "{} pages", pages.len());
        let nodes: Vec<NodeId> =
            pages.iter().flat_map(|p| p.view.nodes.iter().map(|n| n.id.clone())).collect();
        let expected: Vec<NodeId> = whole.nodes.iter().map(|n| n.id.clone()).collect();
        assert_eq!(nodes, expected, "every block once, in order");
        let mut given: Vec<String> = pages
            .iter()
            .flat_map(|p| p.view.edges.iter().map(|e| format!("{}>{}", e.from.0, e.to.0)))
            .collect();
        let mut want: Vec<String> = edges.iter().map(|e| format!("{}>{}", e.from.0, e.to.0)).collect();
        given.sort();
        want.sort();
        assert_eq!(given, want, "every edge once");
        for page in &pages[..pages.len() - 1] {
            let warning = page.warnings.iter().find(|w| w.code == WarningCode::Truncated).unwrap();
            assert_eq!(warning.details["total"], 120);
            assert_eq!(warning.details["kept"], page.view.nodes.len());
            let size = serde_json::to_string(&page.view).unwrap().len();
            assert!(size <= 1_500 * 4 - ENVELOPE_RESERVE_BYTES, "{size}");
        }
        assert!(pages.last().unwrap().warnings.is_empty(), "the last page is whole and says nothing");
    }

    #[test]
    fn a_stale_cursor_neither_repeats_nor_skips_an_unchanged_row() {
        let before = numbered(120);
        let names: Vec<&str> = before.iter().map(String::as_str).collect();
        let old = view(&names, vec![]);
        let first = cut(old.clone(), &MapReq::default(), 4, Some(1_500), &[], &ladder()).unwrap();
        let last = first.view.nodes.last().unwrap().id.clone();
        // The map moves on: a block before the cursor and one after it go, a block lands on each
        // side of it, a block after it is renamed, and so is the cursor's own block.
        let mut after: Vec<String> = before.clone();
        after.retain(|n| n != "002" && n != "100");
        after.extend(["0005".to_owned(), "9999".to_owned()]);
        let mut new = view(&after.iter().map(String::as_str).collect::<Vec<_>>(), vec![]);
        for n in &mut new.nodes {
            if n.id.0 == "a:file:110" || n.id == last {
                n.name.push_str(" renamed");
            }
        }
        new.nodes.sort_by(|a, b| {
            (ladder().rank(&a.level), &a.name, &a.id).cmp(&(ladder().rank(&b.level), &b.name, &b.id))
        });
        let req = MapReq { cursor: first.next.clone(), ..MapReq::default() };
        let rest = follow(req, || (new.clone(), 9));
        assert!(rest[0].warnings.iter().any(|w| w.code == WarningCode::StalePage), "{:?}", rest[0].warnings);
        let mut seen: Vec<NodeId> = first.view.nodes.iter().map(|n| n.id.clone()).collect();
        seen.extend(rest.iter().flat_map(|p| p.view.nodes.iter().map(|n| n.id.clone())));
        let unchanged: Vec<&MapNode> = new.nodes.iter().filter(|n| old.nodes.contains(n)).collect();
        assert!(unchanged.len() > 100);
        for row in unchanged {
            assert_eq!(seen.iter().filter(|id| **id == row.id).count(), 1, "{}", row.id.0);
        }
        let after_cursor: Vec<&NodeId> =
            rest.iter().flat_map(|p| p.view.nodes.iter().map(|n| &n.id)).collect();
        let unique: HashSet<&NodeId> = after_cursor.iter().copied().collect();
        assert_eq!(unique.len(), after_cursor.len(), "no row twice after the cursor");
        assert!(after_cursor.contains(&&NodeId("a:file:9999".into())), "a new row after the cursor is given");
    }

    #[test]
    fn a_cursor_from_other_flags_is_invalid_argument() {
        let names = numbered(120);
        let whole = view(&names.iter().map(String::as_str).collect::<Vec<_>>(), vec![]);
        let summary = MapReq { level: Some(LevelId::from("code")), ..MapReq::default() };
        let next = cut(whole.clone(), &summary, 4, Some(1_500), &[], &ladder()).unwrap().next;
        assert!(next.is_some());
        for other in [
            MapReq { detail: Detail::Graph, ..summary.clone() },
            MapReq { level: Some(LevelId::from("component")), ..summary.clone() },
            MapReq { level: None, ..summary.clone() },
            MapReq { focus: Some("a:file:001".into()), ..summary.clone() },
        ] {
            let req = MapReq { cursor: next.clone(), ..other };
            let e = cut(whole.clone(), &req, 4, Some(1_500), &[], &ladder()).err().expect("refused");
            assert_eq!(e.code, planroom_contract::ErrorCode::InvalidArgument);
            assert_eq!(e.details["field"], "cursor");
        }
        let mut lens = whole.clone();
        lens.lens = "runtime".into();
        let req = MapReq { cursor: next.clone(), ..summary.clone() };
        assert!(cut(lens, &req, 4, Some(1_500), &[], &ladder()).is_err(), "another lens");
        assert!(cut(whole, &req, 4, Some(1_500), &[], &ladder()).is_ok(), "the same flags continue");
    }

    #[test]
    fn a_cursor_round_trips_and_a_foreign_one_is_refused() {
        let cursor = mark_after(&node("a:file:x", "x"), 7, "0123456789abcdef", &[]);
        let mark = decode(&cursor).unwrap();
        assert_eq!((mark.seq, mark.filters.as_str()), (7, "0123456789abcdef"));
        assert_eq!((mark.level, mark.id), (LevelId::from("code"), NodeId("a:file:x".into())));
        for bad in ["zz", "abc", "7b7d"] {
            let e = decode(&Cursor(bad.into())).err().expect("refused");
            assert_eq!(e.code, planroom_contract::ErrorCode::InvalidArgument, "{bad}");
        }
    }

    /// A code-level map of `(ID key, name)` rows, sorted as `kernel::tree` sorts blocks.
    fn named(rows: &[(String, String)]) -> MapView {
        let mut view = crate::app::empty_view(LevelId::from("code"), "all".into());
        view.nodes = rows.iter().map(|(key, name)| node(&format!("a:file:{key}"), name)).collect();
        view.nodes.sort_by(|a, b| {
            (ladder().rank(&a.level), &a.name, &a.id).cmp(&(ladder().rank(&b.level), &b.name, &b.id))
        });
        view
    }

    /// 120 blocks whose names all redact to `x_TOKEN=[redacted]`, and whose names sort against
    /// their IDs: by redacted name and ID they would sort in the reverse of the map's order.
    fn alike() -> Vec<(String, String)> {
        (0..120).map(|i| (format!("{i:03}"), format!("x_TOKEN={:03}", 119 - i))).collect()
    }

    /// The key of the block the first page of `old` gives last in cut order, by level then ID.
    fn last_key(old: &MapView) -> String {
        let first = cut(old.clone(), &MapReq::default(), 4, Some(1_500), &[], &ladder()).unwrap();
        let last = first.view.nodes.iter().map(|n| (ladder().rank(&n.level), &n.id)).max().unwrap().1;
        last.0.trim_start_matches("a:file:").to_owned()
    }

    /// Reads `old`'s first page, then the rest from `new` at a later seq, and checks that every
    /// row the two maps share by ID and level, renamed or not, is given exactly once.
    fn resumes_exactly(old: &MapView, new: &MapView) {
        let first = cut(old.clone(), &MapReq::default(), 4, Some(1_500), &[], &ladder()).unwrap();
        assert!(first.next.is_some(), "the map pages");
        let req = MapReq { cursor: first.next.clone(), ..MapReq::default() };
        let rest = follow(req, || (new.clone(), 9));
        assert!(rest[0].warnings.iter().any(|w| w.code == WarningCode::StalePage), "{:?}", rest[0].warnings);
        let mut seen: Vec<NodeId> = first.view.nodes.iter().map(|n| n.id.clone()).collect();
        seen.extend(rest.iter().flat_map(|p| p.view.nodes.iter().map(|n| n.id.clone())));
        let unchanged: Vec<&MapNode> = new
            .nodes
            .iter()
            .filter(|n| old.nodes.iter().any(|o| o.id == n.id && o.level == n.level))
            .collect();
        assert!(unchanged.len() >= 115, "{}", unchanged.len());
        for row in unchanged {
            assert_eq!(seen.iter().filter(|id| **id == row.id).count(), 1, "{} ({})", row.id.0, row.name);
        }
    }

    #[test]
    fn a_stale_cursor_places_its_last_block_in_the_maps_own_order_when_names_redact_alike() {
        let rows = alike();
        let old = named(&rows);
        // The last block stays; a block before it goes and one lands after it.
        let last = last_key(&old);
        let mut moved: Vec<(String, String)> = rows.iter().filter(|(k, _)| k != "000").cloned().collect();
        moved.push(("new".into(), "x_TOKEN=500".into()));
        assert!(last != "000" && moved.iter().any(|(k, _)| *k == last));
        resumes_exactly(&old, &named(&moved));
    }

    #[test]
    fn a_stale_cursor_whose_last_block_is_gone_continues_between_the_blocks_around_it() {
        let rows = alike();
        let old = named(&rows);
        let last = last_key(&old);
        let gone: Vec<(String, String)> = rows.iter().filter(|(k, _)| *k != last).cloned().collect();
        resumes_exactly(&old, &named(&gone));
    }

    #[test]
    fn a_stale_cursor_whose_last_block_moved_continues_between_the_blocks_around_it() {
        let rows = alike();
        let old = named(&rows);
        let last = last_key(&old);
        // The last block's name now sorts after every other, and still redacts alike.
        let renamed: Vec<(String, String)> = rows
            .iter()
            .map(|(k, n)| if *k == last { (k.clone(), "x_TOKEN=999".into()) } else { (k.clone(), n.clone()) })
            .collect();
        resumes_exactly(&old, &named(&renamed));
    }

    #[test]
    fn a_cursor_carries_no_name() {
        let rows: Vec<(String, String)> =
            (0..120).map(|i| (format!("{i:03}"), format!("db_PASSWORD=hunter{i:03}"))).collect();
        let first = cut(named(&rows), &MapReq::default(), 4, Some(1_500), &[], &ladder()).unwrap();
        let cursor = first.next.unwrap();
        let bytes: Vec<u8> = (0..cursor.0.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&cursor.0[i..i + 2], 16).unwrap())
            .collect();
        let held = String::from_utf8(bytes).unwrap();
        assert!(!held.contains("hunter") && !held.contains("PASSWORD"), "{held}");
    }

    #[test]
    fn a_cursor_past_every_block_gives_an_empty_state() {
        let names = numbered(120);
        let old = view(&names.iter().map(String::as_str).collect::<Vec<_>>(), vec![]);
        let req =
            MapReq { level: Some(LevelId::from("code")), lens: Some("all".into()), ..MapReq::default() };
        let first = cut(old.clone(), &req, 4, Some(1_500), &[], &ladder()).unwrap();
        // The map moves on, and every block after the first page goes.
        let mut new = old.clone();
        new.nodes.truncate(first.view.nodes.len());
        let req = MapReq { cursor: first.next.clone(), ..req };
        let page = cut(new, &req, 9, Some(1_500), &[], &ladder()).unwrap();
        assert!(page.view.nodes.is_empty());
        assert!(page.next.is_none());
        assert!(page.warnings.iter().any(|w| w.code == WarningCode::StalePage), "{:?}", page.warnings);
        let empty = page.view.empty.expect("an empty page says what to do next");
        assert_eq!(empty.reason.0, "nothing_after_cursor");
        assert_eq!(empty.next.command.0, "map");
        assert_eq!(empty.next.args, ["--level", "code", "--lens", "all"]);
        assert_eq!(empty.line.as_deref(), Some(text::get("empty.nothing_after_cursor", &[]).as_str()));
        assert_eq!(empty.next.label, text::get("empty.nothing_after_cursor_next", &[]));
    }

    /// The JSON a cursor holds, decoded from its hex.
    fn decoded(cursor: &Cursor) -> serde_json::Value {
        let bytes: Vec<u8> = (0..cursor.0.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&cursor.0[i..i + 2], 16).unwrap())
            .collect();
        serde_json::from_slice(&bytes).unwrap()
    }

    #[test]
    fn a_cursor_holds_its_seq_filters_and_last_block_and_no_name() {
        let rows: Vec<(String, String)> =
            (0..120).map(|i| (format!("{i:03}"), format!("db_PASSWORD=hunter{:03}", 119 - i))).collect();
        let req = MapReq::default();
        let first = cut(named(&rows), &req, 4, Some(1_500), &[], &ladder()).unwrap();
        let held = decoded(first.next.as_ref().expect("the map pages"));
        // The last block in the order pages are cut in, level then ID.
        let last = first.view.nodes.iter().map(|n| (ladder().rank(&n.level), &n.id)).max().unwrap();
        assert_eq!(held, json!({"s": 4, "f": filters_hash(&req, "all"), "g": 1, "l": "code", "i": last.1.0}));
        let text = held.to_string();
        assert!(!text.contains("hunter") && !text.contains("PASSWORD"), "{text}");
        assert!(first.view.nodes.iter().all(|n| !text.contains(&n.name)), "{text}");
    }

    /// One change to a map of `(ID key, name)` rows, all of whose names redact alike: a block
    /// goes, its name now sorts before or after every other, or a new block lands.
    #[derive(Clone, Debug)]
    enum Change {
        Remove(String),
        First(String),
        Last(String),
        Add(String, String),
    }

    impl Change {
        /// The key of the row it changes or adds.
        fn row(&self) -> &str {
            match self {
                Change::Remove(k) | Change::First(k) | Change::Last(k) | Change::Add(k, _) => k,
            }
        }

        fn apply(&self, rows: &mut Vec<(String, String)>) {
            let rename = |rows: &mut Vec<(String, String)>, key: &str, name: &str| {
                rows.iter_mut().filter(|(k, _)| k == key).for_each(|(_, n)| *n = name.into())
            };
            match self {
                Change::Remove(key) => rows.retain(|(k, _)| k != key),
                Change::First(key) => rename(rows, key, "x_TOKEN=-1"),
                Change::Last(key) => rename(rows, key, "x_TOKEN=999"),
                Change::Add(key, name) => rows.push((key.clone(), name.clone())),
            }
        }
    }

    /// A code-level map of `rows`, as [`named`] gives it, with the lifted edges of `links` whose
    /// two ends are both in it.
    fn linked(rows: &[(String, String)], links: &[(String, String)]) -> MapView {
        let mut view = named(rows);
        let keys: HashSet<&str> = rows.iter().map(|(k, _)| k.as_str()).collect();
        view.edges = links
            .iter()
            .filter(|(from, to)| keys.contains(from.as_str()) && keys.contains(to.as_str()))
            .map(|(from, to)| LiftedEdge {
                kind: planroom_contract::Kind("test.uses".into()),
                from: NodeId(format!("a:file:{from}")),
                to: NodeId(format!("a:file:{to}")),
                count: 1,
                evidence_sample: vec![],
                children: vec![],
            })
            .collect();
        view
    }

    /// The pages of `old`, read at seq 4 through a lens about `about`, up to the first that takes
    /// the blocks given past half the map, and the cursor of the page after it.
    fn to_the_middle(old: &MapView, about: &[Kind]) -> (Vec<Page>, Cursor) {
        let mut req = MapReq::default();
        let mut pages: Vec<Page> = Vec::new();
        loop {
            let page = cut(old.clone(), &req, 4, Some(1_500), about, &ladder()).unwrap();
            let next = page.next.clone().expect("the map pages past its middle");
            pages.push(page);
            if pages.iter().map(|p| p.view.nodes.len()).sum::<usize>() >= old.nodes.len() / 2 {
                return (pages, next);
            }
            req.cursor = Some(next);
        }
    }

    /// Pages the rest of `new` from `cursor` at a later seq, through a lens about `about`, and
    /// checks that every block `old` and `new` share by ID, level and kind is given exactly once
    /// across `given` and those pages, and so is every edge between two such blocks.
    fn gives_every_unchanged_row_once(
        old: &MapView,
        given: &[Page],
        cursor: &Cursor,
        new: &MapView,
        about: &[Kind],
        case: &str,
    ) {
        let req = MapReq { cursor: Some(cursor.clone()), ..MapReq::default() };
        let rest = follow_through(req, about, || (new.clone(), 9));
        assert!(rest[0].warnings.iter().any(|w| w.code == WarningCode::StalePage), "{case}");
        let pages: Vec<&Page> = given.iter().chain(&rest).collect();
        let mut blocks: HashMap<&NodeId, usize> = HashMap::new();
        let mut edges: HashMap<(&NodeId, &NodeId), usize> = HashMap::new();
        for page in &pages {
            page.view.nodes.iter().for_each(|n| *blocks.entry(&n.id).or_default() += 1);
            page.view.edges.iter().for_each(|e| *edges.entry((&e.from, &e.to)).or_default() += 1);
        }
        let before: HashSet<(&LevelId, &NodeId, &Kind)> =
            old.nodes.iter().map(|n| (&n.level, &n.id, &n.kind)).collect();
        let kept: HashSet<&NodeId> = new
            .nodes
            .iter()
            .filter(|n| before.contains(&(&n.level, &n.id, &n.kind)))
            .map(|n| &n.id)
            .collect();
        for id in &kept {
            let times = blocks.get(id).copied().unwrap_or(0);
            assert_eq!(times, 1, "{case}: {} given {times} times", id.0);
        }
        for edge in new.edges.iter().filter(|e| kept.contains(&e.from) && kept.contains(&e.to)) {
            let times = edges.get(&(&edge.from, &edge.to)).copied().unwrap_or(0);
            assert_eq!(times, 1, "{case}: {} > {} given {times} times", edge.from.0, edge.to.0);
        }
    }

    #[test]
    fn a_stale_cursor_gives_every_unchanged_row_once_whatever_changed_around_it() {
        every_change_around_the_cursor(&|_| false);
    }

    #[test]
    fn through_a_lens_with_kinds_a_stale_cursor_gives_every_unchanged_row_once() {
        // Most blocks are of the lens's kind, so the cursor sits among them, and the rest follow.
        every_change_around_the_cursor(&|i| i % 4 != 0);
        // A few blocks with high IDs are, so they come first and the cursor sits past them.
        every_change_around_the_cursor(&|i| i >= 40 && i % 2 == 0);
    }

    /// The kind of the blocks a test lens is about.
    const FLAG: &str = "test.flag";

    /// `view` through a lens about [`FLAG`], with the blocks whose key `flagged` names of that
    /// kind: listed first, as `kernel::lens::apply` lists them, and otherwise in the view's order.
    fn flag(mut view: MapView, flagged: &dyn Fn(usize) -> bool) -> MapView {
        for node in &mut view.nodes {
            let key = node.id.0.trim_start_matches("a:file:");
            if key.len() == 3 && key.parse().is_ok_and(flagged) {
                node.kind = Kind(FLAG.into());
            }
        }
        view.nodes.sort_by_key(|n| n.kind.0 != FLAG);
        view
    }

    /// Pages a map of 120 blocks to its middle, then, for each of many changes to the blocks
    /// around the cursor, pages the rest of the changed map and checks that every unchanged block
    /// and edge is given once. The blocks whose key `flagged` names are of the kind the lens is
    /// about, so they come first in cut order.
    fn every_change_around_the_cursor(flagged: &dyn Fn(usize) -> bool) {
        let rows = alike();
        // A chain, and a jump from every block, so edges cross every page.
        let links: Vec<(String, String)> = (0..120)
            .flat_map(|i| {
                [(i, (i + 1) % 120), (i, i * 7 % 120)].map(|(a, b)| (format!("{a:03}"), format!("{b:03}")))
            })
            .filter(|(a, b)| a != b)
            .collect();
        let about: Vec<Kind> = if (0..120).any(flagged) { vec![Kind(FLAG.into())] } else { vec![] };
        let build = |rows: &[(String, String)]| flag(linked(rows, &links), flagged);
        let old = build(&rows);
        let (given, cursor) = to_the_middle(&old, &about);
        // In cut order, the lens's kinds then the rest, each by ID, the given blocks and the rest
        // meet once; the cursor sits there.
        let mut keys: Vec<String> = (0..120).map(|i| format!("{i:03}")).collect();
        keys.sort_by_key(|k| !flagged(k.parse().unwrap()));
        let seen: HashSet<String> = given
            .iter()
            .flat_map(|p| p.view.nodes.iter().map(|n| n.id.0.trim_start_matches("a:file:").to_owned()))
            .collect();
        let meets: Vec<usize> =
            (0..119).filter(|&i| seen.contains(&keys[i]) != seen.contains(&keys[i + 1])).collect();
        assert_eq!(meets.len(), 1, "{meets:?}");
        let at = meets[0];
        assert!((10..110).contains(&at), "the cursor is in the middle: {at}");
        // Every change to the 10 rows around the cursor, and a new block in every gap among them,
        // its name next to its neighbour's, before every other or after every other.
        let mut changes: Vec<Change> = Vec::new();
        for key in &keys[at - 4..=at + 5] {
            changes.extend([
                Change::Remove(key.clone()),
                Change::First(key.clone()),
                Change::Last(key.clone()),
            ]);
        }
        let mut added: Vec<[Change; 3]> = Vec::new();
        for (i, key) in keys[at - 5..=at + 5].iter().enumerate() {
            let near = rows.iter().find(|(k, _)| k == key).unwrap().1.clone();
            added.push([
                Change::Add(format!("{key}+a"), format!("{near}+")),
                Change::Add(format!("{key}+b"), "x_TOKEN=-0".into()),
                Change::Add(format!("{key}+c"), format!("x_TOKEN=9999{i}")),
            ]);
        }
        // Each change alone, and every pair of changes on different rows. In a pair, the new block
        // in each gap takes one of its three names, in turn from gap to gap, which keeps the pairs
        // few enough to run in seconds.
        let mut cases: Vec<Vec<Change>> =
            changes.iter().chain(added.iter().flatten()).map(|c| vec![c.clone()]).collect();
        changes.extend(added.iter().enumerate().map(|(i, three)| three[i % 3].clone()));
        for (i, one) in changes.iter().enumerate() {
            for two in &changes[i + 1..] {
                if one.row() != two.row() {
                    cases.push(vec![one.clone(), two.clone()]);
                }
            }
        }
        // Every block within 2 rows of the cursor goes, with the map's first 5 blocks as it lists
        // them or as it pages them; and 5 blocks land that sort first by name, by ID or by both.
        let near: Vec<Change> = keys[at - 2..=at + 2].iter().map(|k| Change::Remove(k.clone())).collect();
        let listed_first = old.nodes[..5].iter().map(|n| n.id.0.trim_start_matches("a:file:").to_owned());
        cases.push(near.iter().cloned().chain(listed_first.map(Change::Remove)).collect());
        cases.push(near.iter().cloned().chain(keys[..5].iter().cloned().map(Change::Remove)).collect());
        cases.push((0..5).map(|i| Change::Add(format!("!{i}"), format!("x_TOKEN=-0{i}"))).collect());
        cases.push((0..5).map(|i| Change::Add(format!("~{i}"), format!("x_TOKEN=-0{i}"))).collect());
        cases.push((0..5).map(|i| Change::Add(format!("!{i}"), format!("x_TOKEN=9999{i}"))).collect());
        assert!(cases.len() > 800, "{}", cases.len());
        for case in &cases {
            let mut changed = rows.clone();
            case.iter().for_each(|c| c.apply(&mut changed));
            let new = build(&changed);
            gives_every_unchanged_row_once(&old, &given, &cursor, &new, &about, &format!("{case:?}"));
        }
    }

    /// A vocabulary with one lens, `flags`, that lifts `test.uses` and is about `kinds`.
    fn flags_lens(kinds: &[&str]) -> planroom_contract::Vocabulary {
        planroom_contract::Vocabulary {
            kinds: vec![],
            relations: vec![],
            lenses: vec![planroom_contract::LensVocab {
                id: "flags".into(),
                label: "Flags".into(),
                relations: vec![Kind("test.uses".into())],
                flow: Default::default(),
                kinds: kinds.iter().map(|k| Kind((*k).into())).collect(),
                level: LevelId::from("code"),
                default: false,
            }],
            suggestions: vec![],
            levels: vec![],
        }
    }

    #[test]
    fn a_block_of_the_lenss_kind_with_a_high_id_reaches_the_first_page() {
        let names = numbered(400);
        let names: Vec<&str> = names.iter().map(String::as_str).collect();
        let mut whole = view(&names, (0..399).map(|i| edge(i, i + 1)).collect());
        whole.lens = "flags".into();
        whole.nodes.iter_mut().filter(|n| n.id.0 == "a:file:399").for_each(|n| n.kind = Kind(FLAG.into()));
        let vocabulary = flags_lens(&[FLAG]);
        let seen = planroom_kernel::lens::apply(whole, &vocabulary, vocabulary.lenses.first());
        assert_eq!(seen.nodes[0].id.0, "a:file:399", "the lens lists its kind first");
        let about = [Kind(FLAG.into())];
        let first = cut(seen.clone(), &MapReq::default(), 4, Some(1_500), &about, &ladder()).unwrap();
        assert!(first.next.is_some(), "the map pages");
        assert_eq!(first.view.nodes[0].id.0, "a:file:399", "{:?}", first.view.nodes.first());
        // The pages joined still give every block once.
        let pages = follow_through(MapReq::default(), &about, || (seen.clone(), 4));
        let mut given: Vec<&str> =
            pages.iter().flat_map(|p| p.view.nodes.iter().map(|n| n.id.0.as_str())).collect();
        given.sort_unstable();
        let mut want: Vec<&str> = seen.nodes.iter().map(|n| n.id.0.as_str()).collect();
        want.sort_unstable();
        assert_eq!(given, want);
    }

    #[test]
    fn within_a_page_a_block_with_more_lifted_edges_is_listed_first() {
        let names = numbered(120);
        let names: Vec<&str> = names.iter().map(String::as_str).collect();
        // Block 119 carries 119 lifted edges, block 060 three, every other at most two.
        let edges: Vec<LiftedEdge> =
            (0..119).map(|i| edge(i, 119)).chain([edge(60, 61), edge(60, 59)]).collect();
        let mut whole = view(&names, edges);
        whole.lens = "flags".into();
        let vocabulary = flags_lens(&[]);
        let seen = planroom_kernel::lens::apply(whole, &vocabulary, vocabulary.lenses.first());
        let carried = |id: &NodeId| seen.edges.iter().filter(|e| &e.from == id || &e.to == id).count();
        let pages = follow(MapReq::default(), || (seen.clone(), 4));
        assert!(pages.len() > 1, "the map pages");
        for page in &pages {
            let counts: Vec<usize> = page.view.nodes.iter().map(|n| carried(&n.id)).collect();
            assert!(counts.windows(2).all(|w| w[0] >= w[1]), "most lifted edges first: {counts:?}");
        }
        // With no kinds, the pages are cut by level and ID, so block 119 is on the last page,
        // listed first there.
        assert_eq!(pages.last().unwrap().view.nodes[0].id.0, "a:file:119");
        let holds_060 = pages.iter().find(|p| p.view.nodes.iter().any(|n| n.id.0 == "a:file:060")).unwrap();
        assert_eq!(holds_060.view.nodes[0].id.0, "a:file:060");
    }
}
