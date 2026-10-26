//! `planroom map`: the map at a level, folded from the active snapshots on read.
//!
//! The leaf honors its catalogue budget (`CommandSpec::budget_tokens`): over it, the map keeps its
//! blocks in their documented order while they fit, keeps the lifted edges between the kept ones,
//! and says so with a `truncated` warning whose `details.next` is a handle that works today. Only
//! the canvas's layout input, `--detail graph` over `serve` and in the `share` snapshot, takes the
//! whole graph, until M2's paging pages it (architecture.md, The CLI: "`graph` pages").
//!
//! A map is read ([`read`]), filtered by its lens ([`lens`]), cut to a page ([`page`]) and fitted
//! to the leaf's budget; [`focus`] centres it on a node.

mod focus;
pub(crate) mod lens;
pub(crate) mod page;
mod read;

use std::path::Path;

use planroom_contract::{Command, Detail, MapReq, MapView, Vocabulary, Warning};
use planroom_store::Store;

use crate::app::Reply;
use crate::app::error::AppError;
use crate::app::workspace;
use crate::text;

pub(crate) use page::fit_beside;
pub use read::top;
pub(crate) use read::{not_scanned_view, scan_empty};

/// The lens a map shows when none is asked for; no pack declares a default lens yet.
const ALL_LENSES: &str = "all";
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
    focus::check(req)?;
    let lens_id = req.lens.clone().unwrap_or_else(|| ALL_LENSES.into());
    // Outside a workspace, the same empty state `focus` shows (DR-57).
    let Some(root) = workspace::find_root(cwd) else {
        return read::not_scanned(cwd, lens_id, req.level);
    };
    let Some(store) = Store::open_existing(&workspace::store_path(&root))? else {
        return read::not_scanned(&root, lens_id, req.level);
    };
    let mut view = top(&store, &root, req.level)?;
    view.lens = lens_id.clone();
    // No pack is loaded yet, so the lens keeps the whole view, and the page is the whole view with
    // no cursor after it.
    let vocabulary = Vocabulary::default();
    let view = lens::filter(view, &vocabulary, &lens_id);
    let (mut view, _next) = page::cut(view, req.cursor.as_ref(), budget);
    if view.empty.is_none() {
        match req.detail {
            Detail::Graph => read::place(&store, &mut view)?,
            // Agents get the counts and the lifted edges; evidence is a `show` away.
            Detail::Summary => view.edges.iter_mut().for_each(|e| e.evidence_sample.clear()),
        }
    }
    let total = view.nodes.len();
    let warnings: Vec<Warning> =
        budget.and_then(|budget| page::fit(&mut view, budget, req.level)).into_iter().collect();
    let seq = store.seq()?;
    let mut text = text_of(&view, total);
    for warning in &warnings {
        text.push('\n');
        text.push_str(&warning.message);
    }
    Ok(Reply { text, result: view, seq, warnings })
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
