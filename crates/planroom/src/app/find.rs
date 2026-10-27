//! `planroom find TEXT`: fuzzy search over the map's node names and IDs, best match first
//! (architecture.md, "Lenses and questions": the `find` shape; contracts.md, `FindResult`).
//!
//! A reply reads the store in one read transaction, as `map` does, so its matches and `meta.seq`
//! come from one fold. Each match carries handles that run today: the map at the match's level
//! among the blocks beside it, then, when it holds any, the blocks under it. With no scan yet,
//! `find` returns the same empty state `map` does; with no match, one whose next action is the
//! top of the map.
//!
//! A reply keeps to the leaf's budget whatever the matches: when a match doesn't fit, it drops
//! the match's optional handles first, keeping the one that shows it on the map, then the match
//! itself, down to none. A reply with matches but none that fits counts them in its summary and
//! carries a `truncated` warning whose next action is a narrower `find`.

use std::collections::HashSet;
use std::path::Path;

use planroom_contract::{
    Command, CommandName, Empty, EmptyReason, FindItem, FindReq, FindResult, Handle, Node, Seq, Warning,
    WarningCode,
};
use planroom_kernel::{find, tree};
use planroom_store::Store;
use serde_json::json;

use crate::app::error::AppError;
use crate::app::{Reply, map, workspace};
use crate::{live, text};

/// Matches a reply lists at most, best first; the summary counts every match.
pub const MAX_ITEMS: usize = 10;
/// What the envelope around the result takes, in tokens: its fields, `meta` and the summary line
/// it repeats.
const ENVELOPE_TOKENS: usize = 60;
/// Names the summary line lists.
const SUMMARY_NAMES: usize = 5;
/// Characters of the search text a line echoes; a longer text shows this prefix and an ellipsis,
/// so no search text can push a reply past its budget.
const ECHO_CHARS: usize = 40;

pub fn run(req: &FindReq, cwd: &Path) -> Result<Reply<FindResult>, AppError> {
    if req.text.trim().is_empty() {
        return Err(AppError::invalid_argument("text", "argument.find_text"));
    }
    let Some(root) = workspace::find_root(cwd) else {
        return Ok(not_scanned(cwd, 0));
    };
    let Some(store) = Store::open_existing(&workspace::store_path(&root))? else {
        return Ok(not_scanned(&root, 0));
    };
    let snapshot = store.snapshot()?;
    let seq = snapshot.seq();
    if snapshot.active_set()?.repos.is_empty() {
        return Ok(not_scanned(&root, seq));
    }
    let model = live::model(&snapshot, crate::packs::vocabulary()?)?;
    let hits = find::find(&model.nodes, &req.text);
    if hits.is_empty() {
        return Ok(no_match(&req.text, seq));
    }
    let present: HashSet<&str> = model.nodes.iter().map(|n| n.id.0.as_str()).collect();
    let parents: HashSet<&str> =
        model.nodes.iter().filter_map(|n| n.parent.as_ref()).map(|p| p.0.as_str()).collect();
    let items: Vec<FindItem> =
        hits.iter().take(MAX_ITEMS).map(|hit| item(hit.node, &present, &parents)).collect();
    let (result, warnings) = fit(items, hits.len(), &req.text);
    let mut text = result.summary.clone();
    for warning in &warnings {
        text.push('\n');
        text.push_str(&warning.message);
    }
    Ok(Reply { text, result, seq, warnings })
}

/// The search text as a line echoes it: trimmed, and past [`ECHO_CHARS`] characters, its prefix
/// and an ellipsis. `text::get` passes it through the display-sanitizing pass.
fn echo(text: &str) -> String {
    text::short(text, ECHO_CHARS)
}

/// The best matches that keep the reply within the leaf's budget, with the summary that counts
/// every match and names the first few. When a match doesn't fit, its optional handles go first,
/// keeping the one that shows it on the map, then the match itself, down to none; a reply with no
/// match left carries a `truncated` warning whose next action is a narrower search. `asked` is
/// the search text; the lines echo only [`echo`]'s short prefix of it, so no search text can push
/// a reply past the budget.
fn fit(mut items: Vec<FindItem>, count: usize, asked: &str) -> (FindResult, Vec<Warning>) {
    let budget = budget_tokens();
    let text = echo(asked);
    while !items.is_empty() {
        let result = listing(items, count, &text);
        if sent_tokens(&result) <= budget {
            return (result, vec![]);
        }
        items = result.items;
        match items.last_mut() {
            Some(last) if last.handles.len() > 1 => last.handles.truncate(1),
            _ => {
                items.pop();
            }
        }
    }
    let key = if count == 1 { "find.none_fit_one" } else { "find.none_fit" };
    let summary = text::get(key, &[("count", &count), ("text", &text)]);
    let result = FindResult { summary, items: vec![], empty: None };
    let next = narrower(asked);
    let warning = Warning {
        code: WarningCode::Truncated,
        message: text::get("warning.find_none_fit", &[("budget", &budget), ("label", &next.label)]),
        details: json!({"kept": 0, "total": count, "budget_tokens": budget, "next": next}),
    };
    (result, vec![warning])
}

fn budget_tokens() -> usize {
    Command::Find(FindReq::default()).spec().budget_tokens as usize
}

/// `items` with the summary that counts `count` matches of `text` and names the first few.
fn listing(items: Vec<FindItem>, count: usize, text: &str) -> FindResult {
    let names: Vec<&str> = items.iter().take(SUMMARY_NAMES).map(|i| i.name.as_str()).collect();
    let key = if count == 1 { "find.summary_one" } else { "find.summary" };
    let summary = text::get(key, &[("count", &count), ("text", &text), ("names", &names.join(", "))]);
    FindResult { summary, items, empty: None }
}

/// A narrower search than `asked`: `find` with the search text, at most [`ECHO_CHARS`]
/// characters of it, for the caller to add to.
fn narrower(asked: &str) -> Handle {
    let text: String = asked.trim().chars().take(ECHO_CHARS).collect();
    Handle {
        command: CommandName("find".into()),
        args: vec![text.trim_end().to_owned()],
        label: text::label("find.narrower", &[]),
    }
}

/// The tokens a reply carrying `result` takes, measured as it is sent: the result redacted or
/// raw (`PLANROOM_RAW`), whichever is longer, the summary again, which the envelope repeats, and
/// the envelope's own fields.
fn sent_tokens(result: &FindResult) -> usize {
    let mut redacted = result.clone();
    planroom_kernel::redact::output(&mut redacted);
    let bytes = |r: &FindResult| serde_json::to_string(r).map_or(usize::MAX, |s| s.len() + r.summary.len());
    bytes(result).max(bytes(&redacted)).div_ceil(4) + ENVELOPE_TOKENS
}

/// One match with its handles: where it sits on the map, then what lies under it.
fn item(node: &Node, present: &HashSet<&str>, parents: &HashSet<&str>) -> FindItem {
    let level = tree::level_name(node.level).to_owned();
    let mut show = vec!["--level".to_owned(), level];
    if let Some(parent) = node.parent.as_ref().filter(|p| present.contains(p.0.as_str())) {
        show.extend(["--focus".to_owned(), parent.0.clone()]);
    }
    let mut handles =
        vec![Handle { command: CommandName("map".into()), args: show, label: text::label("find.show", &[]) }];
    if parents.contains(node.id.0.as_str()) {
        handles.push(Handle {
            command: CommandName("map".into()),
            args: vec!["--focus".into(), node.id.0.clone()],
            label: text::label("find.inside", &[]),
        });
    }
    FindItem {
        id: node.id.clone(),
        name: node.name.clone(),
        level: node.level,
        kind: node.kind.clone(),
        handles,
    }
}

/// A directory with no scan: the empty state `map` shows there.
fn not_scanned(dir: &Path, seq: Seq) -> Reply<FindResult> {
    let empty = map::scan_empty(dir, &workspace::scannable(dir));
    empty_reply(empty, seq)
}

/// Nothing matches `text`: the line says so, and the next action is the top of the map.
fn no_match(text: &str, seq: Seq) -> Reply<FindResult> {
    let empty = Empty {
        reason: EmptyReason("no_match".into()),
        line: Some(text::get("empty.no_match", &[("text", &echo(text))])),
        next: Handle {
            command: CommandName("map".into()),
            args: vec![],
            label: text::label("empty.top_next", &[]),
        },
    };
    empty_reply(empty, seq)
}

fn empty_reply(empty: Empty, seq: Seq) -> Reply<FindResult> {
    let line = empty.line.clone().unwrap_or_else(|| empty.next.label.clone());
    let result = FindResult { summary: line.clone(), items: vec![], empty: Some(empty) };
    Reply { text: line, result, seq, warnings: vec![] }
}

#[cfg(test)]
mod tests {
    use planroom_contract::{Kind, Level, NodeId};

    use super::*;

    fn found(id: &str, name: &str, handles: &[(&str, &str)]) -> FindItem {
        FindItem {
            id: NodeId(id.into()),
            name: name.into(),
            level: Level::Code,
            kind: Kind("test.file".into()),
            handles: handles
                .iter()
                .map(|(flag, arg)| Handle {
                    command: CommandName("map".into()),
                    args: vec![(*flag).into(), (*arg).into()],
                    label: "label".into(),
                })
                .collect(),
        }
    }

    #[test]
    fn a_match_that_does_not_fit_drops_its_optional_handles_before_it_goes() {
        let deep = format!("a:dir:{}", "d".repeat(2_000));
        let items = vec![
            found("a:file:one", "one", &[("--level", "code"), ("--focus", "a:file:one")]),
            found("a:file:two", "two", &[("--level", "code"), ("--focus", &deep)]),
        ];
        let (result, warnings) = fit(items, 2, "o");
        assert!(warnings.is_empty(), "{warnings:?}");
        assert!(sent_tokens(&result) <= budget_tokens());
        let handles: Vec<usize> = result.items.iter().map(|i| i.handles.len()).collect();
        assert_eq!(handles, [2, 1], "the second match keeps the handle that shows it");
        assert_eq!(result.items[1].handles[0].args, ["--level", "code"]);
    }

    #[test]
    fn a_match_that_does_not_fit_without_its_optional_handles_goes() {
        let long = "n".repeat(3_000);
        let items = vec![
            found("a:file:one", "one", &[("--level", "code")]),
            found("a:file:two", &long, &[("--level", "code")]),
        ];
        let (result, warnings) = fit(items, 2, "o");
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(result.items.len(), 1);
        assert_eq!(result.items[0].name, "one");
        assert!(result.summary.starts_with("2 blocks match"), "{}", result.summary);
    }

    #[test]
    fn with_no_match_that_fits_the_reply_counts_them_and_points_to_a_narrower_search() {
        let long = format!("a:file:{}", "x".repeat(3_000));
        let (result, warnings) = fit(vec![found(&long, "x", &[("--level", "code")])], 1, "x");
        assert!(result.items.is_empty());
        assert!(result.summary.starts_with("1 block matches x"), "{}", result.summary);
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].code, WarningCode::Truncated);
        assert_eq!(warnings[0].details["next"]["command"], "find");
        assert_eq!(warnings[0].details["next"]["args"], json!(["x"]));
        assert_eq!(warnings[0].details["total"], 1);
    }
}
