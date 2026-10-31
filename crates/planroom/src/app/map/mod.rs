//! `planroom map`: the map at a level, or under one node, folded from the store on read.
//!
//! A reply reads the store in one read transaction: the model is the fold at that transaction's
//! highest `seq`, and the reply's `meta.seq` is that `seq`, so it never runs ahead of the view
//! (DR-73). A scan, an undo or a review refolds; in `serve` the fold is kept between replies and
//! caught up one event at a time (DR-61, `live::model`).
//!
//! The leaf honors its catalogue budget (`CommandSpec::budget_tokens`) at every detail: over it,
//! the map gives its first page, the blocks that fit taken in cut order (the lens's kinds' blocks,
//! then the rest, each by level and then ID) and listed in the lens's display order ([`page`]),
//! with the lifted edges that land there, and `meta.next`, the cursor `--cursor` takes for the
//! page after it, with a `truncated` warning whose `details.next` is a drill-down that works
//! today. Only the canvas's layout input, `--detail
//! graph` over `serve` and in the `share` snapshot, takes the whole graph, until M2 pages it
//! (architecture.md, Token budgets).
//!
//! A map is read ([`read`]), filtered by its lens ([`lens`]) and cut to a page within the leaf's
//! budget ([`page`]); [`focus`] finds the node `--focus` names, whose blocks the view shows.

mod focus;
pub mod lens;
pub(crate) mod page;
mod read;

use std::collections::HashMap;
use std::path::Path;

use planroom_contract::{Command, Detail, ErrorCode, LevelId, MapNode, MapReq, MapView, Vocabulary};
use planroom_kernel::ladder::Ladder;
use planroom_store::{Snapshot, Store};
use serde_json::json;

use crate::app::error::AppError;
use crate::app::workspace;
use crate::app::{Paged, Reply};
use crate::packs::state::ReadSet;
use crate::text;

pub(crate) use page::fit_beside;
pub(crate) use read::{not_scanned_view, plan_held, scan_empty};
pub use read::{top, top_in};

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
    // With no workspace or no store yet there is no recorded set: the built-in packs check the
    // request, and the empty state carries their ladder, so a surface can name the levels before
    // a scan.
    let not_scanned = |dir: &Path| {
        let vocabulary = crate::packs::vocabulary()?;
        let lens_id = lens::asked(vocabulary, req.lens.as_deref())?;
        asked_level(vocabulary, req.level.as_ref())?;
        let level = level_of(req, vocabulary);
        let mut reply = read::not_scanned(dir, lens_id, level, 0, vocabulary, false)?;
        with_ladder(&mut reply.result, req.detail, vocabulary);
        Ok(Paged::whole(reply))
    };
    // Outside a workspace, the same empty state `focus` shows (DR-57).
    let Some(root) = workspace::find_root(cwd) else {
        return not_scanned(cwd);
    };
    let Some(store) = Store::open_existing(&workspace::store_path(&root))? else {
        return not_scanned(&root);
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
    // The set the workspace records, read in this transaction, checks the request and folds it.
    let set = crate::packs::state::for_snapshot(snapshot, root)?;
    let vocabulary = set.vocabulary();
    let lens_id = lens::asked(vocabulary, req.lens.as_deref())?;
    asked_level(vocabulary, req.level.as_ref())?;
    let level = level_of(req, vocabulary);
    let seq = snapshot.seq();
    let (mut view, under) = match req.focus.as_deref() {
        Some(target) => focused(snapshot, root, &set, level.as_ref(), target)?,
        None => read::view(snapshot, root, level.clone(), None, vocabulary)?,
    };
    view.lens = lens_id.clone();
    let mut view = lens::filter(view, vocabulary, &lens_id);
    if view.empty.is_none() {
        match req.detail {
            // Positions come from the whole map's layout, whichever page shows them.
            Detail::Graph => read::place(snapshot, &mut view, vocabulary)?,
            // Agents get the counts and the lifted edges; evidence is a `show` away.
            Detail::Summary => view.edges.iter_mut().for_each(|e| e.evidence_sample.clear()),
        }
    }
    with_ladder(&mut view, req.detail, vocabulary);
    // The page is cut from the view as the reply carries it, so it is measured as it is sent. The
    // level the lens defaults to is the level the page, its cursor and its drill-down name.
    let req = MapReq { level: level.clone(), ..req.clone() };
    let ladder = Ladder::of(vocabulary);
    let mut page = page::cut(view, &req, seq, budget, lens::kinds(vocabulary, &lens_id), &ladder)?;
    set.warn(&mut page.warnings);
    let mut text = text_of(&page.view, page.total, level.as_ref(), under.as_deref(), &ladder);
    for warning in &page.warnings {
        text.push('\n');
        text.push_str(&warning.message);
    }
    let reply = Reply { text, result: page.view, seq, warnings: page.warnings };
    Ok(Paged { next: page.next, ..Paged::whole(reply) })
}

/// The level a request names, checked: one the ladder holds, or none. A level no loaded pack
/// declares is `invalid_argument` naming the levels by ID in ladder order, as an unknown lens is
/// ([`lens::asked`]), before anything is read.
pub(crate) fn asked_level(vocabulary: &Vocabulary, asked: Option<&LevelId>) -> Result<(), AppError> {
    let Some(level) = asked else { return Ok(()) };
    if Ladder::of(vocabulary).holds(level) {
        return Ok(());
    }
    let known: Vec<&str> = vocabulary.levels.iter().map(|l| l.id.as_str()).collect();
    let what =
        text::get("argument.unknown_level", &[("level", &level.as_str()), ("known", &known.join(", "))]);
    Err(AppError::new(ErrorCode::InvalidArgument, &[("what", &what)])
        .details(json!({"field": "level", "levels": known})))
}

/// A graph reply carries the vocabulary's levels, `intent` first and then the ladder top to
/// finest, so a surface names and steps through levels without naming one itself; a summary
/// carries none.
pub(crate) fn with_ladder(view: &mut MapView, detail: Detail, vocabulary: &Vocabulary) {
    if detail == Detail::Graph {
        view.levels = vocabulary.levels.clone();
    }
}

/// `count` blocks at `level` in the ladder's words, its `one` or its `many`; a level the ladder
/// doesn't hold reads `{count} {level}`.
pub(crate) fn count_words(ladder: &Ladder<'_>, level: &str, count: u64) -> String {
    match ladder.levels().iter().find(|l| l.id.as_str() == level) {
        Some(l) => {
            let words = if count == 1 { &l.one } else { &l.many };
            text::fill(words, &[("count", &count)])
        }
        None => format!("{count} {}", planroom_kernel::display::sanitize(level)),
    }
}

/// A block's counts as a line lists them: in ladder order, each in the ladder's words, and the
/// keys the ladder doesn't hold last.
fn counts_line(ladder: &Ladder<'_>, counts: &std::collections::BTreeMap<String, u32>) -> Vec<String> {
    let mut keys: Vec<(&String, &u32)> = counts.iter().collect();
    // A level the ladder doesn't hold ranks below its finest; the sort keeps those in key order.
    keys.sort_by_key(|(level, _)| ladder.rank(&LevelId::from(level.as_str())));
    keys.into_iter().map(|(level, count)| count_words(ladder, level, u64::from(*count))).collect()
}

/// The noun a level reads as in an engine line: its `label` on the ladder, else its ID.
pub(crate) fn label_of(ladder: &Ladder<'_>, level: &LevelId) -> String {
    ladder
        .levels()
        .iter()
        .find(|l| l.id == *level)
        .map_or_else(|| level.as_str().to_owned(), |l| l.label.clone())
}

/// The view `--focus target` asks for. A project that holds no items has no block to resolve, so
/// its own view answers: always for its ID, which no block shares, and for its key or name only
/// where no block answers to the target, or no repo is mapped, so a project, an agent's proposal
/// among them, never takes a block's focus or hides an ambiguity.
fn focused(
    snapshot: &Snapshot<'_>,
    root: &Path,
    set: &ReadSet,
    level: Option<&LevelId>,
    target: &str,
) -> Result<(MapView, Option<String>), AppError> {
    let vocabulary = set.vocabulary();
    if let Some(found) = read::project_without_items(snapshot, set, target, level, read::ProjectBy::Id)? {
        return Ok(found);
    }
    // With no repo mapped there is no block, and `view` answers with the top's empty state.
    let missing = if snapshot.active_set()?.repos.is_empty() {
        None
    } else {
        match read::view(snapshot, root, level.cloned(), Some(target), vocabulary) {
            Err(e) if e.code == ErrorCode::NotFound => Some(e),
            answered => return answered,
        }
    };
    match (read::project_without_items(snapshot, set, target, level, read::ProjectBy::Any)?, missing) {
        (Some(found), _) => Ok(found),
        (None, Some(e)) => Err(e),
        (None, None) => read::view(snapshot, root, level.cloned(), Some(target), vocabulary),
    }
}

/// The level a request shows: the one it names, or with `--lens X` alone and no `--focus`, the
/// lens's own (DR-15). With neither, the top of the map.
fn level_of(req: &MapReq, vocabulary: &Vocabulary) -> Option<LevelId> {
    req.level
        .clone()
        .or_else(|| req.focus.is_none().then(|| lens::level_of(vocabulary, req.lens.as_deref())).flatten())
}

/// The text mode's first line, which the envelope also carries as `summary`: the empty state's
/// line, or what the map shows (the top, one level, or the blocks under one node) with the count
/// and the first block names with their counts in the ladder's words. `level` is the level the
/// request named, and `under` the name of the node it focused on; the line names the shown level
/// by its label.
fn text_of(
    view: &MapView,
    total: usize,
    level: Option<&LevelId>,
    under: Option<&str>,
    ladder: &Ladder<'_>,
) -> String {
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
            let counts = counts_line(ladder, &n.counts);
            if counts.is_empty() { name } else { format!("{name} ({})", counts.join(", ")) }
        })
        .collect();
    let more = total.saturating_sub(listed.len());
    let names = if more == 0 {
        listed.join("; ")
    } else {
        text::get("map.more", &[("names", &listed.join("; ")), ("more", &more)])
    };
    let at = label_of(ladder, &view.level);
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

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use planroom_contract::{Kind, LevelVocab, NodeId};

    use super::*;

    /// An invented ladder whose inserted level's label differs from its ID: `zone`, read as
    /// "service area".
    fn vocabulary() -> Vocabulary {
        let level = |id: &str, label: &str, apart: bool, pack: &str| LevelVocab {
            id: LevelId::from(id),
            label: label.into(),
            one: format!("{{count}} {label}"),
            many: format!("{{count}} {label}s"),
            apart,
            pack: pack.into(),
        };
        Vocabulary {
            levels: vec![
                level("intent", "intent", true, "kernel"),
                level("realm", "realm", false, "base"),
                level("zone", "service area", false, "zones"),
                level("room", "room", false, "base"),
            ],
            ..Vocabulary::default()
        }
    }

    fn block(name: &str, level: &str, counts: &[(&str, u32)]) -> MapNode {
        MapNode {
            id: NodeId(format!("ns:{level}:{name}")),
            kind: Kind("base.block".into()),
            tone: None,
            name: name.into(),
            level: LevelId::from(level),
            parent: None,
            counts: counts.iter().map(|(k, n)| ((*k).to_owned(), *n)).collect::<BTreeMap<_, _>>(),
            position: None,
            ghost: false,
            claimed_by: None,
        }
    }

    fn view(level: &str, nodes: Vec<MapNode>) -> MapView {
        let mut view = crate::app::empty_view(LevelId::from(level), "all".into());
        view.nodes = nodes;
        view
    }

    #[test]
    fn level_strings_take_labels_and_handles_keep_ids() {
        let vocabulary = vocabulary();
        let ladder = Ladder::of(&vocabulary);

        // A level's line names its label; the view keeps its ID.
        let zones = view("zone", vec![block("north", "zone", &[("room", 1)])]);
        let zone = LevelId::from("zone");
        let line = text_of(&zones, 1, Some(&zone), None, &ladder);
        assert_eq!(line, "1 at the service area level: north (1 room).");
        assert_eq!(zones.level.as_str(), "zone");

        // Counts are keyed by ID and read in the ladder's words, in ladder order, with a key the
        // ladder doesn't hold last.
        let realm = view("realm", vec![block("east", "realm", &[("room", 5), ("galaxy", 2), ("zone", 3)])]);
        let line = text_of(&realm, 1, None, None, &ladder);
        assert_eq!(line, "1 at the top: east (3 service areas, 5 rooms, 2 galaxy).");
        assert_eq!(count_words(&ladder, "zone", 1), "1 service area");

        // A handle names a level by ID in its args and by label in its label: a map cut at its
        // budget at `room` drills to the next coarser level, the inserted `zone`.
        let room = LevelId::from("room");
        let mut rooms = view("room", vec![block("kitchen", "room", &[]), block("hall", "room", &[])]);
        let cut = fit_beside(&mut rooms, 1, Some(&room), &[], &[], &ladder).expect("a cut past its budget");
        let next: planroom_contract::Handle = serde_json::from_value(cut.details["next"].clone()).unwrap();
        assert_eq!(next.command.0, "map");
        assert_eq!(next.args, ["--level", "zone"]);
        assert_eq!(next.label, "Show the service area level");

        // An unknown level is refused naming the levels by ID, in ladder order.
        let err = asked_level(&vocabulary, Some(&LevelId::from("foo"))).unwrap_err();
        assert_eq!(err.code, planroom_contract::ErrorCode::InvalidArgument);
        assert_eq!(err.details["field"], "level");
        assert_eq!(err.details["levels"], serde_json::json!(["intent", "realm", "zone", "room"]));
        assert!(asked_level(&vocabulary, Some(&zone)).is_ok());
        assert!(asked_level(&vocabulary, Some(&LevelId::intent())).is_ok());
        assert!(asked_level(&vocabulary, None).is_ok());

        // Only a graph carries the ladder, IDs and labels both.
        let mut graph = zones.clone();
        with_ladder(&mut graph, Detail::Graph, &vocabulary);
        assert_eq!(graph.levels, vocabulary.levels);
        let mut summary = zones;
        with_ladder(&mut summary, Detail::Summary, &vocabulary);
        assert!(summary.levels.is_empty());
    }
}
