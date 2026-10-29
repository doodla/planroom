//! `planroom ask [QUESTION [ARGS]]` (architecture.md, "Lenses and questions"; contracts.md,
//! `AskReq` and `AskResult`; DR-15, DR-21).
//!
//! Alone, or as `ask kinds`, it lists what is loaded: the levels first, in ladder order, each with
//! the pack that declares it and the map at it; then every loaded pack's kinds, lenses and
//! questions, each with its owning pack, a question with its arguments and one-line summary, and
//! the built-in questions this build serves, marked: `kinds`, and `stage` while the plan pack is
//! loaded. Nothing else: a built-in question this build
//! doesn't serve yet, or a pack that isn't loaded, isn't listed. The listing needs no workspace,
//! and pages under the leaf's budget with `--cursor`.
//!
//! With a question, it runs the question's pipeline (`kernel::query`) over the map at the store's
//! latest `seq`, in one read transaction, with each argument resolved as a full ID, an ID suffix
//! or a name at the question's level first. Each item carries one line on why it answers, built
//! from `en.toml`, and handles that run today. A reply keeps to the question's budget (its pack's
//! `budget_tokens`, at most 3,000, else the leaf's 1,500) whatever the repo holds: every name and
//! path a line quotes is cut short, an item that still doesn't fit drops its optional handles,
//! then goes, and a `truncated` warning counts what was left out.
//!
//! `ask stage S [PROJECT]` serves a plan stage: its method checklist, one item per line, the item
//! types it writes with their fields, and, for the named or only project, its items of those
//! types, the stage's lint findings and the blocks the plan touches. An unknown stage is
//! `not_found` before any store is read; with no store or no project, the checklist and the types
//! come with the `no_intent` empty state, so the checklist works before any scan.

use std::collections::{BTreeMap, HashMap};
use std::path::Path;

use planroom_contract::{
    AskItem, AskReq, AskResult, Command, CommandName, Cursor, Empty, EmptyReason, ErrorCode, Evidence,
    Handle, Kind, Node, NodeId, Provenance, QuestionDecl, Seq, Shape, Vocabulary, Warning, WarningCode,
};
use planroom_kernel::ladder::Ladder;
use planroom_kernel::query::lookup::Lookup;
use planroom_kernel::query::{self, Answer, EdgeWhy, Hop, Item, Neighbor, QueryError, Reason, Toward};
use planroom_store::Store;
use serde::{Deserialize, Serialize};
use serde_json::json;

use crate::app::error::AppError;
use crate::app::{Paged, Reply, intent, map, workspace};
use crate::packs::{self, Loaded};
use crate::{live, text};

/// The built-in questions this build serves: the listing itself, and a plan stage while the plan
/// pack is loaded (DR-15). `lint`, `missing` and `drift` join them as their packs and checks land.
const KINDS: &str = "kinds";
const STAGE: &str = "stage";
/// `stage`'s budget: the `intent` leaves' (architecture.md, the Intent row).
const STAGE_BUDGET: u32 = 1_500;
/// The most a pack question's budget may be (DR-21).
const MAX_BUDGET: u32 = 3_000;
/// What the envelope around the result takes, in tokens: its fields, `meta` and the summary line
/// it repeats.
const ENVELOPE_TOKENS: usize = 60;
/// What a `truncated` warning takes, in tokens, with the cursor or counts it names.
const WARNING_TOKENS: usize = 120;
/// The least an item takes, in tokens: its ID, name and line, however short.
const MIN_ITEM_TOKENS: usize = 10;
/// Names the summary line lists.
const SUMMARY_NAMES: usize = 5;
/// Characters a line quotes of a name, a label or an argument.
const NAME_CHARS: usize = 40;
/// Characters an item's name keeps.
const ITEM_NAME_CHARS: usize = 80;
/// Characters a line quotes of an evidence path, and of the evidence line's text.
const PATH_CHARS: usize = 80;
const CONTAINS_CHARS: usize = 60;
/// Characters the IDs an error's details name may take together.
const DETAILS_CHARS: usize = 1_200;
/// Characters of a question's summary the listing quotes.
const SUMMARY_CHARS: usize = 160;
/// Characters of the last entry's ID a listing cursor keeps, beside the entry's place.
const MARK_CHARS: usize = 32;

/// The budget a reply to `req` keeps to: its question's, or the leaf's.
pub fn budget(req: &AskReq) -> Option<u32> {
    let leaf = leaf_budget();
    let question = req.question.as_deref().filter(|q| *q != KINDS);
    let decl = question.and_then(|q| packs::loaded().ok().and_then(|l| find_question(l, q)).map(|(_, d)| d));
    Some(decl.map_or(leaf, question_budget))
}

fn leaf_budget() -> u32 {
    Command::Ask(AskReq::default()).spec().budget_tokens
}

/// A question's budget: its own, at most [`MAX_BUDGET`], or the leaf's.
fn question_budget(decl: &QuestionDecl) -> u32 {
    decl.budget_tokens.map_or(leaf_budget(), |b| b.min(MAX_BUDGET))
}

pub fn run(req: &AskReq, cwd: &Path) -> Result<Paged<AskResult>, AppError> {
    let loaded = packs::loaded()?;
    let Some(question) = req.question.as_deref().filter(|q| *q != KINDS) else {
        if !req.args.is_empty() {
            return Err(AppError::invalid_argument("args", "ask.kinds_args"));
        }
        return listing(loaded, req.cursor.as_ref());
    };
    if req.cursor.is_some() {
        return Err(AppError::invalid_argument("cursor", "ask.cursor_question"));
    }
    if question == STAGE && !intent::stages(loaded).is_empty() {
        return stage(loaded, &req.args, cwd);
    }
    let Some((_, decl)) = find_question(loaded, question) else {
        return Err(unknown(loaded, question));
    };
    let required = decl.args.iter().filter(|a| !a.optional).count();
    if req.args.len() < required || req.args.len() > decl.args.len() {
        let what = text::get(
            "ask.arity",
            &[
                ("question", &text::short(&decl.id, NAME_CHARS)),
                ("usage", &text::short(&usage(decl), SUMMARY_CHARS)),
                ("count", &req.args.len()),
            ],
        );
        return Err(
            AppError::new(ErrorCode::InvalidArgument, &[("what", &what)]).details(json!({"field": "args"}))
        );
    }
    let asked = asked(decl, &req.args);
    let Some(root) = workspace::find_root(cwd) else {
        return Ok(Paged::whole(not_scanned(decl, cwd, 0)));
    };
    let Some(store) = Store::open_existing(&workspace::store_path(&root))? else {
        return Ok(Paged::whole(not_scanned(decl, &root, 0)));
    };
    let snapshot = store.snapshot()?;
    let seq = snapshot.seq();
    if snapshot.active_set()?.repos.is_empty() {
        return Ok(Paged::whole(not_scanned(decl, &root, seq)));
    }
    let vocabulary = &loaded.vocabulary;
    let model = live::model(&snapshot, vocabulary)?;
    let level = query::level_of(&decl.steps, vocabulary);
    let mut args: BTreeMap<String, NodeId> = BTreeMap::new();
    for (arg, given) in decl.args.iter().zip(&req.args) {
        let node = match query::lookup::lookup(&model.nodes, given, &level, &Ladder::of(vocabulary)) {
            Lookup::Found(node) => node,
            Lookup::Missing(closest) => return Err(missing(given, &closest)),
            Lookup::Ambiguous(several) => return Err(ambiguous(given, &several)),
        };
        args.insert(arg.name.clone(), node.id.clone());
    }
    let answer = query::run(&model, vocabulary, &decl.steps, &args).map_err(|e| unanswerable(decl, &e))?;
    if answer.items.is_empty() {
        return Ok(Paged::whole(no_answer(decl, &asked, seq)));
    }
    let lines = Lines::new(&model.nodes, vocabulary, why_question(loaded));
    let items: Vec<AskItem> = answer_items(&answer, &lines);
    let (result, warnings) = fit(&decl.id, question_budget(decl), items, &asked, None);
    let mut text = result.summary.clone();
    for item in &result.items {
        text.push_str(&format!("\n  {} · {}", item.id, item.why));
    }
    Ok(Paged::whole(Reply { text, result, seq, warnings }))
}

/// The question `id` and the pack that declares it, in load order.
fn find_question<'a>(loaded: &'a Loaded, id: &str) -> Option<(&'a str, &'a QuestionDecl)> {
    loaded.packs.iter().find_map(|p| p.file.questions.iter().find(|q| q.id == id).map(|q| (p.id(), q)))
}

/// A loaded question that is one `why` step over two arguments: the drill-down a hop or a
/// neighbor offers, whichever pack declares it.
fn why_question(loaded: &Loaded) -> Option<&str> {
    loaded
        .packs
        .iter()
        .flat_map(|p| &p.file.questions)
        .find(|q| q.steps.len() == 1 && q.steps[0].shape == Shape::Why && q.args.len() == 2)
        .map(|q| q.id.as_str())
}

/// How a question is asked: its ID and its arguments, an optional one in brackets.
fn usage(decl: &QuestionDecl) -> String {
    let mut words = vec![decl.id.clone()];
    words.extend(decl.args.iter().map(|a| if a.optional { format!("[{}]", a.name) } else { a.name.clone() }));
    words.join(" ")
}

/// The question's summary with each `{NAME}` filled with the argument as given, cut short.
fn asked(decl: &QuestionDecl, given: &[String]) -> String {
    let mut line = decl.summary.clone();
    for (arg, value) in decl.args.iter().zip(given) {
        line = line.replace(&format!("{{{}}}", arg.name), &text::short(value, NAME_CHARS));
    }
    text::short(&line, SUMMARY_CHARS)
}

/// The question's summary with each `{NAME}` as its argument's name, as the listing shows it.
fn named(decl: &QuestionDecl) -> String {
    let names: Vec<String> = decl.args.iter().map(|a| a.name.clone()).collect();
    asked(decl, &names)
}

fn unknown(loaded: &Loaded, question: &str) -> AppError {
    let mut ids: Vec<&str> =
        loaded.packs.iter().flat_map(|p| &p.file.questions).map(|q| q.id.as_str()).collect();
    ids.push(KINDS);
    if !intent::stages(loaded).is_empty() {
        ids.push(STAGE);
    }
    let near: Vec<&str> =
        ids.iter().copied().filter(|id| id.contains(question) || question.contains(id)).collect();
    let closest = within(if near.is_empty() { ids } else { near });
    let what = text::get("ask.unknown", &[("question", &text::short(question, NAME_CHARS))]);
    AppError::new(ErrorCode::NotFound, &[("what", &what)]).details(json!({"closest": closest}))
}

fn missing(given: &str, closest: &[&Node]) -> AppError {
    let what = text::get("ask.arg_missing", &[("target", &text::short(given, NAME_CHARS))]);
    let closest = within(closest.iter().map(|n| n.id.0.as_str()));
    AppError::new(ErrorCode::NotFound, &[("what", &what)]).details(json!({"closest": closest}))
}

/// The IDs, in order, while they fit [`DETAILS_CHARS`] together, so an error that names blocks
/// with long paths, or questions with long IDs, stays short; an ID is never cut, since a caller
/// passes it back.
fn within<'n>(ids: impl IntoIterator<Item = &'n str>) -> Vec<&'n str> {
    let mut used = 0;
    ids.into_iter()
        .take_while(|id| {
            used += id.len();
            used <= DETAILS_CHARS
        })
        .collect()
}

/// Several blocks, all at one level, match `given`: an ambiguous match holds two or more, and
/// the details name their level by its ID.
fn ambiguous(given: &str, several: &[&Node]) -> AppError {
    let level = several.first().map(|n| n.level.as_str());
    let candidates = within(several.iter().take(query::lookup::CLOSEST).map(|n| n.id.0.as_str()));
    let shown: Vec<String> = candidates.iter().take(3).map(|id| text::short(id, ITEM_NAME_CHARS)).collect();
    let vars: &[(&str, &dyn std::fmt::Display)] =
        &[("target", &text::short(given, NAME_CHARS)), ("count", &several.len()), ("ids", &shown.join(", "))];
    AppError::new(ErrorCode::AmbiguousRef, vars).details(json!({"candidates": candidates, "level": level}))
}

fn unanswerable(decl: &QuestionDecl, e: &QueryError) -> AppError {
    let what = match e {
        QueryError::Unserved(shape) => {
            let shape = serde_json::to_value(shape).ok().and_then(|v| v.as_str().map(str::to_owned));
            let question = text::short(&decl.id, NAME_CHARS);
            text::get("ask.unserved", &[("question", &question), ("shape", &shape.unwrap_or_default())])
        }
        QueryError::Param(key) | QueryError::Missing(key) => {
            text::get("ask.param", &[("question", &text::short(&decl.id, NAME_CHARS)), ("key", key)])
        }
    };
    AppError::new(ErrorCode::InvalidArgument, &[("what", &what)]).details(json!({"field": "question"}))
}

/// What the lines of an answer read: each node by ID, each relation's label, and the loaded
/// question that shows an edge's evidence.
struct Lines<'a> {
    nodes: HashMap<&'a str, &'a Node>,
    relations: HashMap<&'a Kind, &'a str>,
    why: Option<&'a str>,
}

impl<'a> Lines<'a> {
    fn new(nodes: &'a [Node], vocabulary: &'a Vocabulary, why: Option<&'a str>) -> Lines<'a> {
        Lines {
            nodes: nodes.iter().map(|n| (n.id.0.as_str(), n)).collect(),
            relations: vocabulary.relations.iter().map(|r| (&r.kind, r.label.as_str())).collect(),
            why,
        }
    }

    /// A node's name as a line quotes it, cut short; its ID when the model doesn't hold it.
    fn name(&self, id: &NodeId) -> String {
        let name = self.nodes.get(id.0.as_str()).map_or(id.0.as_str(), |n| n.name.as_str());
        text::short(name, NAME_CHARS)
    }

    fn relation(&self, kind: &Kind) -> String {
        text::short(self.relations.get(kind).copied().unwrap_or(&kind.0), NAME_CHARS)
    }

    /// The map at the node's level among the blocks beside it, as `find` shows a match.
    fn show(&self, id: &NodeId) -> Option<Handle> {
        let node = self.nodes.get(id.0.as_str())?;
        let mut args = vec!["--level".to_owned(), node.level.as_str().to_owned()];
        if let Some(parent) = node.parent.as_ref().filter(|p| self.nodes.contains_key(p.0.as_str())) {
            args.extend(["--focus".to_owned(), parent.0.clone()]);
        }
        Some(Handle { command: CommandName("map".into()), args, label: text::label("ask.show", &[]) })
    }

    /// The question that shows the evidence of the edges from `from` to `to`.
    fn evidence(&self, from: &NodeId, to: &NodeId) -> Option<Handle> {
        Some(Handle {
            command: CommandName("ask".into()),
            args: vec![self.why?.to_owned(), from.0.clone(), to.0.clone()],
            label: text::label("ask.why", &[]),
        })
    }
}

fn answer_items(answer: &Answer, lines: &Lines<'_>) -> Vec<AskItem> {
    answer.items.iter().map(|item| answer_item(item, lines)).collect()
}

fn answer_item(item: &Item, lines: &Lines<'_>) -> AskItem {
    let name = lines
        .nodes
        .get(item.node.0.as_str())
        .map_or_else(|| item.node.0.clone(), |n| text::short(&n.name, ITEM_NAME_CHARS));
    let (why, drill) = match &item.reason {
        Reason::Named => (text::get("ask.named", &[]), None),
        Reason::Start => (text::get("ask.start", &[]), None),
        Reason::Hop(hop) => (hop_line(hop, lines), lines.evidence(&hop.from, &item.node)),
        Reason::Neighbor(n) => {
            let (from, to) = match n.toward {
                Toward::Forward => (&n.via, &item.node),
                Toward::Backward => (&item.node, &n.via),
            };
            (neighbor_line(n, lines), lines.evidence(from, to))
        }
        Reason::Edge(e) => (edge_line(e, lines), None),
    };
    let handles = drill.into_iter().chain(lines.show(&item.node)).collect();
    AskItem { id: item.node.0.clone(), name, why, handles }
}

fn hop_line(hop: &Hop, lines: &Lines<'_>) -> String {
    let key = if hop.backward { "ask.hop_back" } else { "ask.hop" };
    text::get(
        key,
        &[
            ("from", &lines.name(&hop.from)),
            ("relation", &lines.relation(&hop.relation)),
            ("count", &hop.edges.len()),
            ("confidence", &format!("{:.2}", hop.confidence)),
            ("at", &at(&hop.evidence)),
        ],
    )
}

fn neighbor_line(n: &Neighbor, lines: &Lines<'_>) -> String {
    let through = if n.depth > 1 {
        text::get("ask.through", &[("depth", &n.depth), ("via", &lines.name(&n.via))])
    } else {
        String::new()
    };
    let key = match n.toward {
        Toward::Forward => "ask.reaches",
        Toward::Backward => "ask.reached",
    };
    text::get(
        key,
        &[
            ("start", &lines.name(&n.start)),
            ("relation", &lines.relation(&n.relation)),
            ("through", &through),
        ],
    )
}

fn edge_line(e: &EdgeWhy, lines: &Lines<'_>) -> String {
    let via = e
        .via
        .as_deref()
        .map(|v| text::get("ask.edge_via", &[("via", &text::short(v, NAME_CHARS))]))
        .unwrap_or_default();
    let by = match &e.provenance {
        Provenance::Extracted { extractor, .. } => text::short(extractor, NAME_CHARS),
        Provenance::Linked { linker, .. } => text::short(linker, NAME_CHARS),
        Provenance::Inferred { model, .. } => text::short(model, NAME_CHARS),
        Provenance::Authored { .. } => text::get("ask.by_hand", &[]),
    };
    text::get(
        "ask.edge",
        &[
            ("relation", &lines.relation(&e.relation)),
            ("from", &lines.name(&e.from)),
            ("to", &lines.name(&e.to)),
            ("via", &via),
            ("confidence", &format!("{:.2}", e.confidence)),
            ("by", &by),
            ("at", &at(&e.evidence)),
        ],
    )
}

/// Where the first piece of evidence is, as a line ends: a file's path and line, with the line's
/// text, or a commit; each cut short.
fn at(evidence: &[Evidence]) -> String {
    match evidence.first() {
        Some(Evidence::Line { path, line, contains: Some(contains), .. }) => text::get(
            "ask.at_line",
            &[
                ("path", &text::short(path, PATH_CHARS)),
                ("line", line),
                ("contains", &text::short(contains, CONTAINS_CHARS)),
            ],
        ),
        Some(Evidence::Line { path, line, contains: None, .. }) => {
            text::get("ask.at", &[("path", &text::short(path, PATH_CHARS)), ("line", line)])
        }
        Some(Evidence::Ref { commit, .. }) => {
            text::get("ask.at_commit", &[("commit", &commit.chars().take(12).collect::<String>())])
        }
        None => String::new(),
    }
}

/// The answer's items that keep the reply within the question's budget, with the summary that
/// counts every item and names the first few. When an item doesn't fit, its optional handles go
/// first, keeping its first, then the item itself; a reply that left items out carries a
/// `truncated` warning that counts them.
fn fit(
    question: &str,
    budget: u32,
    mut items: Vec<AskItem>,
    asked: &str,
    summary: Option<&str>,
) -> (AskResult, Vec<Warning>) {
    let budget = budget as usize;
    let total = items.len();
    let result = |items: Vec<AskItem>| answer_result(question, items, total, asked, summary);
    let tokens = |items: &[AskItem]| sent_tokens(&result(items.to_vec()));
    if tokens(&items) <= budget {
        return (result(items), vec![]);
    }
    // A cut reply carries a warning, so the items get the budget less its room. An item takes at
    // least [`MIN_ITEM_TOKENS`], which caps how many can fit; the longest prefix that fits is
    // found by halving, so a large answer costs a logarithmic number of measures.
    let room = budget.saturating_sub(WARNING_TOKENS);
    items.truncate(room / MIN_ITEM_TOKENS);
    let (mut fits, mut over) = (0, items.len());
    while fits < over {
        let mid = (fits + over).div_ceil(2);
        if tokens(&items[..mid]) <= room { fits = mid } else { over = mid - 1 }
    }
    // The next item may still fit without its optional handles.
    if let Some(next) = items.get_mut(fits).filter(|i| i.handles.len() > 1) {
        next.handles.truncate(1);
        if tokens(&items[..=fits]) <= room {
            fits += 1;
        }
    }
    items.truncate(fits);
    let result = result(items);
    let kept = result.items.len();
    let warning = Warning {
        code: WarningCode::Truncated,
        message: text::get("ask.cut", &[("kept", &kept), ("total", &total), ("budget", &budget)]),
        details: json!({"kept": kept, "total": total, "budget_tokens": budget}),
    };
    (result, vec![warning])
}

/// The result that carries `items`, `total` of them in all: its summary is `summary` when given,
/// else one that counts them and names the first few.
fn answer_result(
    question: &str,
    items: Vec<AskItem>,
    total: usize,
    asked: &str,
    summary: Option<&str>,
) -> AskResult {
    let summary = if let Some(summary) = summary {
        summary.to_owned()
    } else if items.is_empty() {
        text::get("ask.none_fit", &[("asked", &asked), ("count", &total)])
    } else {
        let names: Vec<String> =
            items.iter().take(SUMMARY_NAMES).map(|i| text::short(&i.name, NAME_CHARS)).collect();
        let key = if total == 1 { "ask.summary_one" } else { "ask.summary" };
        text::get(key, &[("asked", &asked), ("count", &total), ("names", &names.join(", "))])
    };
    AskResult { question: question.to_owned(), summary, items, empty: None }
}

/// The tokens a reply carrying `result` takes, measured as it is sent: the result redacted or
/// raw (`PLANROOM_RAW`), whichever is longer, the summary again, which the envelope repeats, and
/// the envelope's own fields.
fn sent_tokens(result: &AskResult) -> usize {
    let mut redacted = result.clone();
    planroom_kernel::redact::output(&mut redacted);
    let bytes = |r: &AskResult| serde_json::to_string(r).map_or(usize::MAX, |s| s.len() + r.summary.len());
    bytes(result).max(bytes(&redacted)).div_ceil(4) + ENVELOPE_TOKENS
}

/// A directory with no scan: the empty state `map` shows there.
fn not_scanned(decl: &QuestionDecl, dir: &Path, seq: Seq) -> Reply<AskResult> {
    empty_reply(decl, map::scan_empty(dir, &workspace::scannable(dir)), seq)
}

/// Nothing on the map answers the question: the next action lists what can be asked.
fn no_answer(decl: &QuestionDecl, asked: &str, seq: Seq) -> Reply<AskResult> {
    let empty = Empty {
        reason: EmptyReason("no_answer".into()),
        line: Some(text::get("empty.no_answer", &[("asked", &asked)])),
        next: Handle {
            command: CommandName("ask".into()),
            args: vec![],
            label: text::label("empty.no_answer_next", &[]),
        },
    };
    empty_reply(decl, empty, seq)
}

fn empty_reply(decl: &QuestionDecl, empty: Empty, seq: Seq) -> Reply<AskResult> {
    let line = empty.line.clone().unwrap_or_else(|| empty.next.label.clone());
    let result =
        AskResult { question: decl.id.clone(), summary: line.clone(), items: vec![], empty: Some(empty) };
    Reply { text: line, result, seq, warnings: vec![] }
}

/// `ask stage S [PROJECT]`: the stage's checklist and item types, then, for the named or only
/// project, its items of those types with their status, the stage's lint findings and the blocks
/// the plan touches, within `stage`'s budget.
fn stage(loaded: &Loaded, args: &[String], cwd: &Path) -> Result<Paged<AskResult>, AppError> {
    let stages = intent::stages(loaded);
    let arity = || {
        let what = text::get(
            "ask.arity",
            &[("question", &STAGE), ("usage", &"stage S [PROJECT]"), ("count", &args.len())],
        );
        AppError::new(ErrorCode::InvalidArgument, &[("what", &what)]).details(json!({"field": "args"}))
    };
    let Some(given) = args.first() else { return Err(arity()) };
    let Some(stage) = stages.iter().find(|s| &s.id == given) else {
        let ids: Vec<&str> = stages.iter().map(|s| s.id.as_str()).collect();
        let what = text::get(
            "ask.stage_unknown",
            &[("stage", &text::short(given, NAME_CHARS)), ("stages", &ids.join(", "))],
        );
        return Err(AppError::new(ErrorCode::NotFound, &[("what", &what)]).details(json!({"closest": ids})));
    };
    if args.len() > 2 {
        return Err(arity());
    }
    let mut items: Vec<AskItem> = stage
        .prompt
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .enumerate()
        .map(|(n, line)| AskItem {
            id: format!("{}:{}", stage.id, n + 1),
            name: text::short(line, ITEM_NAME_CHARS),
            why: line.to_owned(),
            handles: vec![],
        })
        .collect();
    let types = intent::item_types(&stages);
    for item in &stage.items {
        let fields: Vec<String> = types
            .get(&item.id)
            .into_iter()
            .flatten()
            .map(|(name, schema)| match schema.get("type").and_then(serde_json::Value::as_str) {
                Some(kind) => format!("{name} ({kind})"),
                None => name.clone(),
            })
            .collect();
        let why = if fields.is_empty() {
            text::get("ask.stage_type_bare", &[("type", &item.id)])
        } else {
            text::get("ask.stage_type", &[("type", &item.id), ("fields", &fields.join(", "))])
        };
        items.push(AskItem { id: item.id.clone(), name: item.id.clone(), why, handles: vec![] });
    }
    let (mut count, mut waiting, mut findings) = (0, 0, 0);
    let mut empty = Some(intent::no_intent());
    let mut seq = 0;
    let store = workspace::find_root(cwd)
        .map(|root| Store::open_existing(&workspace::store_path(&root)))
        .transpose()?
        .flatten();
    if let Some(store) = store {
        let snapshot = store.snapshot()?;
        seq = snapshot.seq();
        let folded = live::folded(&snapshot, &loaded.vocabulary)?;
        let projects = planroom_kernel::plan::projects(&folded);
        if let Some(project) = intent::current(&projects, args.get(1).map(String::as_str))? {
            empty = None;
            let mut all = planroom_kernel::plan::items_of(&folded, &project.id);
            planroom_kernel::plan::order(&mut all, &stages);
            let written: Vec<&str> = stage.items.iter().map(|t| t.id.as_str()).collect();
            for item in all.iter().filter(|i| written.contains(&i.item_type.as_str())) {
                count += 1;
                let status = if item.live {
                    waiting += usize::from(!item.pending.is_empty());
                    text::get("ask.stage_accepted", &[])
                } else {
                    waiting += 1;
                    text::get("ask.stage_waiting", &[])
                };
                let shown = intent::plan_item(item, &stages, seq);
                items.push(AskItem {
                    id: item.id.0.clone(),
                    name: shown.name,
                    why: text::get("ask.stage_item", &[("type", &item.item_type), ("status", &status)]),
                    handles: shown.handles.into_iter().take(1).collect(),
                });
            }
            let found = intent::findings(loaded, &folded, &project.id, |r| stage.lint.contains(&r.id));
            findings = found.len();
            for finding in found {
                items.push(AskItem {
                    id: finding.subject.0,
                    name: finding.name,
                    why: finding.message,
                    handles: vec![],
                });
            }
            let lines = Lines::new(&folded.model.nodes, &loaded.vocabulary, None);
            let mut attached: Vec<&NodeId> = all.iter().flat_map(|i| &i.touches).collect();
            attached.sort();
            attached.dedup();
            for id in attached {
                let Some(node) = lines.nodes.get(id.0.as_str()) else { continue };
                items.push(AskItem {
                    id: id.0.clone(),
                    name: text::short(&node.name, ITEM_NAME_CHARS),
                    why: text::get("ask.stage_attached", &[]),
                    handles: lines.show(id).into_iter().collect(),
                });
            }
        }
    }
    let summary = text::get(
        "ask.stage_summary",
        &[
            ("stage", &stage.id),
            ("method", &text::short(&stage.method, NAME_CHARS)),
            ("items", &count),
            ("waiting", &waiting),
            ("findings", &findings),
        ],
    );
    let (mut result, warnings) = fit(STAGE, STAGE_BUDGET, items, &summary, Some(&summary));
    result.empty = empty;
    let mut text = result.summary.clone();
    for item in &result.items {
        text.push_str(&format!("\n  {} · {}", item.id, item.why));
    }
    if let Some(line) = result.empty.as_ref().and_then(|e| e.line.as_ref()) {
        text.push_str(&format!("\n{line}"));
    }
    Ok(Paged::whole(Reply { text, result, seq, warnings }))
}

/// Where a listing page ends: its last entry's section, its place in the listing and the start of
/// its ID, so a cursor stays short however long the ID.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Mark {
    ask: u8,
    #[serde(default)]
    at: usize,
    id: String,
}

impl Mark {
    fn of(entries: &[Entry], at: usize) -> Mark {
        let entry = &entries[at];
        let id = entry.key.chars().take(MARK_CHARS).collect();
        Mark { ask: entry.section, at, id }
    }

    /// The place in `entries` after this mark: after the entry it names, or, when the listing
    /// changed since, after every entry that sorts before it.
    fn after(&self, entries: &[Entry]) -> usize {
        match entries.get(self.at) {
            Some(e) if e.section == self.ask && e.key.starts_with(&self.id) => self.at + 1,
            _ => entries.partition_point(|e| (e.section, e.key.as_str()) <= (self.ask, self.id.as_str())),
        }
    }
}

/// One entry of the listing, in listing order: levels in ladder order, then kinds, lenses and
/// questions, each by ID. `key` is what the entry sorts by within its section: a level's rank as
/// two digits, else the item's ID, which the item shows whole, since a caller passes it back.
struct Entry {
    section: u8,
    key: String,
    item: AskItem,
}

impl Entry {
    fn new(section: u8, item: AskItem) -> Entry {
        Entry { section, key: item.id.clone(), item }
    }
}

/// The listing's sections, in order.
const LEVELS: u8 = 0;
const KINDS_SECTION: u8 = 1;
const LENSES: u8 = 2;
const QUESTIONS: u8 = 3;

/// Every loaded level, kind, lens and question, with the built-in questions this build serves.
/// A line names a level by its label, and a handle by its ID.
fn entries(loaded: &Loaded) -> Vec<Entry> {
    let mut out = Vec::new();
    let ladder = Ladder::of(&loaded.vocabulary);
    let chain = loaded.vocabulary.levels.iter().filter(|l| !ladder.is_apart(&l.id)).count();
    for level in &loaded.vocabulary.levels {
        let rank = ladder.rank(&level.id);
        let why = if ladder.is_apart(&level.id) {
            text::get("ask.level_apart", &[])
        } else {
            let pack = text::short(&level.pack, NAME_CHARS);
            text::get("ask.level", &[("n", &rank), ("count", &chain), ("pack", &pack)])
        };
        let item = AskItem {
            id: level.id.as_str().to_owned(),
            name: text::short(&level.label, NAME_CHARS),
            why,
            handles: vec![Handle {
                command: CommandName("map".into()),
                args: vec!["--level".into(), level.id.as_str().into()],
                label: text::label("ask.show_level", &[]),
            }],
        };
        out.push(Entry { section: LEVELS, key: format!("{rank:02}"), item });
    }
    for pack in &loaded.packs {
        let id = pack.id();
        let pack_name = text::short(id, NAME_CHARS);
        for kind in &pack.file.kinds {
            let level = kind.level.as_str();
            let label = text::short(&map::label_of(&ladder, &kind.level), NAME_CHARS);
            out.push(Entry::new(
                KINDS_SECTION,
                AskItem {
                    id: packs::qualify(id, &kind.id),
                    name: text::short(&kind.label, NAME_CHARS),
                    why: text::get("ask.kind", &[("level", &label), ("pack", &pack_name)]),
                    handles: vec![Handle {
                        command: CommandName("map".into()),
                        args: vec!["--level".into(), level.into()],
                        label: text::label("ask.show_level", &[]),
                    }],
                },
            ));
        }
        for lens in &pack.file.lenses {
            let label = text::short(&map::label_of(&ladder, &lens.level), NAME_CHARS);
            out.push(Entry::new(
                LENSES,
                AskItem {
                    id: lens.id.clone(),
                    name: text::short(&lens.label, NAME_CHARS),
                    why: text::get(
                        "ask.lens",
                        &[("count", &lens.relations.len()), ("level", &label), ("pack", &pack_name)],
                    ),
                    handles: vec![Handle {
                        command: CommandName("map".into()),
                        args: vec!["--lens".into(), lens.id.clone()],
                        label: text::label("ask.show_lens", &[]),
                    }],
                },
            ));
        }
        for question in &pack.file.questions {
            let ready = question.args.iter().all(|a| a.optional);
            out.push(Entry::new(
                QUESTIONS,
                AskItem {
                    id: question.id.clone(),
                    name: text::short(&usage(question), NAME_CHARS),
                    why: text::get(
                        "ask.question",
                        &[("summary", &text::short(&named(question), SUMMARY_CHARS)), ("pack", &pack_name)],
                    ),
                    handles: ready
                        .then(|| Handle {
                            command: CommandName("ask".into()),
                            args: vec![question.id.clone()],
                            label: text::label("ask.run", &[]),
                        })
                        .into_iter()
                        .collect(),
                },
            ));
        }
    }
    out.push(Entry::new(
        QUESTIONS,
        AskItem {
            id: KINDS.into(),
            name: KINDS.into(),
            why: text::get("ask.builtin_kinds", &[]),
            handles: vec![],
        },
    ));
    if !intent::stages(loaded).is_empty() {
        out.push(Entry::new(
            QUESTIONS,
            AskItem {
                id: STAGE.into(),
                name: "stage S [PROJECT]".into(),
                why: text::get("ask.builtin_stage", &[]),
                handles: vec![],
            },
        ));
    }
    out.sort_by(|a, b| (a.section, &a.key).cmp(&(b.section, &b.key)));
    out
}

/// `ask` alone: the page of the listing after `cursor`, within the leaf's budget.
pub fn listing(loaded: &Loaded, cursor: Option<&Cursor>) -> Result<Paged<AskResult>, AppError> {
    let entries = entries(loaded);
    let total = entries.len();
    let count = |section: u8| entries.iter().filter(|e| e.section == section).count();
    let packs: Vec<&str> = loaded.packs.iter().map(|p| p.id()).collect();
    let summary = text::get(
        "ask.listing",
        &[
            ("levels", &count(LEVELS)),
            ("kinds", &count(KINDS_SECTION)),
            ("lenses", &count(LENSES)),
            ("questions", &count(QUESTIONS)),
            ("packs", &text::short(&packs.join(", "), SUMMARY_CHARS)),
        ],
    );
    let after = cursor.map(decode).transpose()?;
    let from = after.map_or(0, |m| m.after(&entries)).min(entries.len());
    let rest = &entries[from..];
    let budget = leaf_budget() as usize;
    let page = |items: Vec<AskItem>| AskResult {
        question: KINDS.into(),
        summary: summary.clone(),
        items,
        empty: None,
    };
    // A page whose last entry is `rest[last]` carries that entry's cursor when entries follow, in
    // `meta.next` and in the paging warning and its handle; a page that left out an entry too long
    // for any page counts it in that warning.
    let paging = |last: usize, kept: usize, left_out: usize| -> (Option<Cursor>, Vec<Warning>) {
        let next = (last + 1 < rest.len()).then(|| encode(&Mark::of(&entries, from + last)));
        if next.is_none() && left_out == 0 {
            return (None, vec![]);
        }
        let mut details = json!({"kept": kept, "total": total, "budget_tokens": budget});
        if left_out > 0 {
            details["entries_left_out"] = json!(left_out);
        }
        let message = match &next {
            Some(cursor) => {
                let handle = Handle {
                    command: CommandName("ask".into()),
                    args: vec!["--cursor".into(), cursor.0.clone()],
                    label: text::label("ask.next_page", &[]),
                };
                details["next"] = json!(handle);
                let key = if left_out > 0 { "ask.paged_left_out" } else { "ask.paged" };
                text::get(
                    key,
                    &[
                        ("kept", &kept),
                        ("total", &total),
                        ("budget", &budget),
                        ("left_out", &left_out),
                        ("cursor", &cursor.0),
                    ],
                )
            }
            None => text::get(
                "ask.left_out",
                &[("kept", &kept), ("total", &total), ("budget", &budget), ("left_out", &left_out)],
            ),
        };
        (next, vec![Warning { code: WarningCode::Truncated, message, details }])
    };
    // What a page takes as sent: the result, and the cursor and warning it carries.
    let cost = |items: Vec<AskItem>, last: usize, left_out: usize| {
        let (next, warnings) = paging(last, items.len(), left_out);
        let extra =
            serde_json::to_string(&warnings).map_or(usize::MAX, |w| w.len()) + next.map_or(0, |c| c.0.len());
        sent_tokens(&page(items)).saturating_add(extra.div_ceil(4))
    };
    // Each entry as a page can show it: whole, or without its handles when whole it fits no page,
    // or not at all when even so it doesn't, since an ID is never cut: a caller passes it back.
    // Measured as the only item of a page that ends at it and counts every other entry as left
    // out, the most a page holding it takes. No more entries are weighed than one page can hold.
    let mut shown: Vec<Option<AskItem>> = Vec::new();
    let mut held = 0;
    for (at, entry) in rest.iter().enumerate() {
        if held > budget / MIN_ITEM_TOKENS {
            break;
        }
        let mut item = entry.item.clone();
        let fits = |item: &AskItem| cost(vec![item.clone()], at, rest.len()) <= budget;
        if !fits(&item) {
            item.handles.clear();
        }
        let item = fits(&item).then_some(item);
        held += usize::from(item.is_some());
        shown.push(item);
    }
    let taken = |n: usize| -> (Vec<AskItem>, usize) {
        let items: Vec<AskItem> = shown[..n].iter().flatten().cloned().collect();
        let left_out = n - items.len();
        (items, left_out)
    };
    let fits = |n: usize| {
        let (items, left_out) = taken(n);
        cost(items, n - 1, left_out) <= budget
    };
    // The longest run of entries that fits, found by halving; at least one, so a page moves on,
    // and one always fits as it was weighed.
    let n = if shown.is_empty() {
        0
    } else if fits(shown.len()) {
        shown.len()
    } else {
        let (mut fit, mut over) = (1, shown.len());
        while fit < over {
            let mid = (fit + over).div_ceil(2);
            if fits(mid) { fit = mid } else { over = mid - 1 }
        }
        fit
    };
    let (items, left_out) = taken(n);
    let kept = items.len();
    let result = page(items);
    let mut text = result.summary.clone();
    for item in &result.items {
        text.push_str(&format!("\n  {} · {} · {}", item.id, item.name, item.why));
    }
    let (next, warnings) = if n == 0 { (None, vec![]) } else { paging(n - 1, kept, left_out) };
    let reply = Reply { text, result, seq: 0, warnings };
    Ok(Paged { reply, next, dirty: Default::default() })
}

/// A mark as JSON, hex-encoded, so it passes as one shell word and no redaction pass mistakes it
/// for a secret, as `map`'s cursor is.
fn encode(mark: &Mark) -> Cursor {
    let json = serde_json::to_vec(mark).expect("a mark serializes");
    Cursor(json.iter().map(|b| format!("{b:02x}")).collect())
}

fn decode(cursor: &Cursor) -> Result<Mark, AppError> {
    let unreadable = || AppError::invalid_argument("cursor", "ask.cursor_unreadable");
    let hex = cursor.0.as_bytes();
    if !hex.len().is_multiple_of(2) {
        return Err(unreadable());
    }
    let bytes: Option<Vec<u8>> =
        hex.chunks(2).map(|pair| u8::from_str_radix(std::str::from_utf8(pair).ok()?, 16).ok()).collect();
    serde_json::from_slice(&bytes.ok_or_else(unreadable)?).map_err(|_| unreadable())
}

#[cfg(test)]
mod tests {
    use planroom_contract::ArgDecl;

    use super::*;

    fn decl(budget: Option<u32>) -> QuestionDecl {
        QuestionDecl {
            id: "asked".into(),
            args: vec![ArgDecl { name: "A".into(), optional: false }],
            steps: vec![],
            summary: "What {A} touches".into(),
            budget_tokens: budget,
        }
    }

    fn item(id: &str, name: &str, handles: usize) -> AskItem {
        AskItem {
            id: id.into(),
            name: name.into(),
            why: "It reaches web over calls.".into(),
            handles: (0..handles)
                .map(|i| Handle {
                    command: CommandName("map".into()),
                    args: vec!["--focus".into(), format!("{id}{i}")],
                    label: "label".into(),
                })
                .collect(),
        }
    }

    #[test]
    fn many_items_keep_to_the_budget_and_count_what_was_left_out() {
        let items: Vec<AskItem> = (0..20_000).map(|i| item(&format!("ws:box:b{i:05}"), "b", 2)).collect();
        let (result, warnings) = fit("asked", question_budget(&decl(None)), items, "What web touches", None);
        assert!(sent_tokens(&result) <= 1_500);
        assert!(!result.items.is_empty() && result.items.len() < 100);
        assert_eq!(warnings[0].code, WarningCode::Truncated);
        assert_eq!(warnings[0].details["total"], 20_000);
        assert!(result.summary.starts_with("What web touches: 20000 found"), "{}", result.summary);
    }

    #[test]
    fn a_pack_budget_is_clamped_and_an_item_too_long_for_any_budget_goes() {
        assert_eq!(question_budget(&decl(Some(9_000))), 3_000);
        assert_eq!(question_budget(&decl(None)), 1_500);
        let long = format!("ws:box:{}", "x".repeat(20_000));
        let budget = question_budget(&decl(Some(9_000)));
        let (result, warnings) = fit("asked", budget, vec![item(&long, "x", 1)], "What x touches", None);
        assert!(result.items.is_empty());
        assert!(result.summary.contains("none fits"), "{}", result.summary);
        assert_eq!(warnings[0].details["kept"], 0);
        // A question whose own ID is as long as the loader allows answers within its budget, the
        // ID whole.
        let mut asked = decl(None);
        asked.id = "q".repeat(crate::packs::ID_MAX);
        let (result, _) =
            fit(&asked.id, question_budget(&asked), vec![item("ws:box:a", "a", 1)], "What a touches", None);
        assert!(sent_tokens(&result) <= 1_500 && result.items.len() == 1, "{}", sent_tokens(&result));
    }

    #[test]
    fn an_error_names_long_question_ids_only_while_they_fit() {
        // Question IDs as long as the loader allows, more of them than any reply can name.
        let long = "q".repeat(crate::packs::ID_MAX - 3);
        let mut body = String::new();
        for i in 0..200 {
            body.push_str(&format!(
                "[[questions]]\nid = \"{long}{i:03}\"\nsummary = \"What uses {{W}}\"\nargs = [{{ name = \"W\" }}]\n\n\
                 [[questions.steps]]\nshape = \"neighbors\"\nparams = {{ ids = [\"$W\"], relations = [\"code.imports\"] }}\n\n"
            ));
        }
        let manifest = format!(
            "format = \"planroom.pack/v1\"\nid = \"asks\"\nversion = 1\ntitle = \"Asks\"\nrequires = [\"code\"]\n\n{body}"
        );
        let mut sources = packs::embed::sources();
        sources.push(packs::Source {
            origin: packs::Origin::OutOfTree,
            label: "mem/asks".into(),
            dir: None,
            recorded: None,
            files: BTreeMap::from([("pack.toml".to_owned(), manifest.into_bytes())]),
        });
        let loaded = packs::resolve(sources).unwrap();
        let e = unknown(&loaded, "nothing-like-it");
        let named: usize =
            e.details["closest"].as_array().unwrap().iter().map(|v| v.as_str().unwrap().len()).sum();
        assert!(named <= DETAILS_CHARS, "{named} characters of IDs");
        let decl = &loaded.packs.iter().find(|p| p.id() == "asks").unwrap().file.questions[0];
        let e = unanswerable(decl, &QueryError::Param("depth".into()));
        assert!(e.message.chars().count() < 400, "{}", e.message);
    }

    #[test]
    fn a_cursor_round_trips_and_a_foreign_one_is_refused() {
        let mark = Mark { ask: 1, at: 7, id: "flow".into() };
        assert_eq!(decode(&encode(&mark)).unwrap(), mark);
        assert!(decode(&Cursor("zz".into())).is_err());
        assert!(decode(&Cursor("7b7d".into())).is_err());
    }
}
