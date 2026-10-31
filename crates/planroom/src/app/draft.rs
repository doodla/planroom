//! The model side of `planroom intent next` (architecture.md, "Planning: stages, ghosts and lint
//! (R5)" and Model adapters; DR-23, DR-65, DR-99): the Claude call that drafts one plan stage, and
//! the reply read back into a [`PlanProposal`].
//!
//! - **The stable prefix** is the plan pack's one stage prompt, `prompts/plan.md`, then the reply's
//!   schema, [`draft_reply_schema`] over every item type and prop the stages and the packs'
//!   `[[items]]` declare: the same
//!   bytes for every stage and every project, so after a plan's first stage each call reads the
//!   cache the first one wrote. The prompt's `[[prompts]]` entry gives its tier and its 1-hour TTL.
//! - **The tail** is [`StageSlice`], built from the fold and passed through
//!   [`models::request_tail`], which masks it and fences it, then the stage's question:
//!   `intent.next_question`, filled with the stage's method, its checklist as the pack writes it,
//!   each item type it writes with its props, then each `[[items]]` type with its props.
//! - **The slice** holds the project, the stage and every item, live and ghost, in `intent list`'s
//!   order, and for a stage that writes an item type a loaded rule requires a `kernel.touches` edge
//!   of, the blocks on the map its items may touch: those `find` ranks best for the words of the
//!   live items' names and text props, and every block the plan already touches. It is masked,
//!   then cut to [`SLICE_MAX_BYTES`]: targets from the end first, then each item's text props to
//!   [`PROP_CUT_CHARS`], then items from the end, each cut counted. Masking comes first, so a
//!   secret is never cut short of the shape that masks it.
//! - **The reply** must end its turn and parse as a [`DraftReply`]; its prop pairs become each
//!   item's props, a parent the plan doesn't hold is dropped, and a touch target that isn't one
//!   block on the map is dropped and counted.
//!
//! Every byte of a request comes from the pack, the configuration and the fold: no `seq`, event
//! ID, key, time, path or evidence line, so the replay key is fixed by the plan and the map.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use planroom_contract::{
    CommandName, DraftReply, DraftValue, ErrorCode, Handle, Node, NodeId, PlanProposal, PromptDecl,
    PromptRole, ProposedItem, RuleType, SliceItem, SliceTarget, StageDecl, StageSlice, Status, Vocabulary,
    draft_reply_schema,
};
use planroom_kernel::fold::Folded;
use planroom_kernel::ladder::Ladder;
use planroom_kernel::plan::{self, Project};
use planroom_kernel::query::lookup::{self, Lookup};
use planroom_kernel::{find, redact};
use planroom_models::canonical;
use planroom_models::claude::{Call, Reply};
use planroom_store::Store;
use serde_json::{Value, json};

use crate::app::error::AppError;
use crate::app::intent::{self, key_of};
use crate::app::{models, workspace};
use crate::packs::{Loaded, state};
use crate::{live, text};

/// The `[[prompts]]` ID of the plan pack's stage prompt.
pub const STAGE_PROMPT: &str = "stage";
/// A stage call's purpose is this and the stage's ID, such as `plan.stage.outcome`: its
/// recordings' folder and its spend log scenario.
pub const PURPOSE_PREFIX: &str = "plan.stage.";
/// The most one stage call may cost, in dollars, which the spend log reserves before it is sent.
/// From claude-gateway.md §2's Sonnet 5.5 prices, $2.00 per million input tokens and $10.00 per
/// million output tokens, with the 1-hour cache write at 2× input, $4.00, by Anthropic's
/// prompt-caching price rule (platform.claude.com/docs/en/build-with-claude/prompt-caching, its
/// pricing: 1.25× input for a 5-minute write, 2× for a 1-hour one; checked 2026-10-28; §2 lists
/// only the 5-minute write): a 24 KB prefix written cold (about 6,000 tokens, $0.024), an
/// 8 KB slice and its question (about 2,300 tokens, $0.0046) and [`MAX_TOKENS`] of output
/// ($0.060) come to about $0.089, rounded up.
pub const STAGE_CEILING_USD: f64 = 0.10;
/// The most output tokens a stage's reply takes: the prompt's working limits keep a reply of 15
/// items well inside it.
pub const MAX_TOKENS: u32 = 6_000;
/// The uncached tail: the slice and its question, about 2,300 tokens (see [`STAGE_CEILING_USD`]).
pub const TAIL_TOKENS_EST: u32 = 2_300;
/// What one stage call costs at most, as the stage picker states it before a pick: a per-pick
/// token ceiling (the uncached tail and the most output a reply takes), the dollar ceiling, and
/// that the shared prefix is cached.
pub fn estimate() -> planroom_contract::PickEstimate {
    planroom_contract::PickEstimate {
        tokens: TAIL_TOKENS_EST + MAX_TOKENS,
        usd: STAGE_CEILING_USD,
        cached: true,
    }
}
/// The slice's canonical JSON, as the fence holds it, at most.
pub const SLICE_MAX_BYTES: usize = 8_000;
/// Characters of an item's text prop kept when the slice must be cut.
pub const PROP_CUT_CHARS: usize = 200;
/// The words a stage's targets are found by, at most.
const WORDS_MAX: usize = 30;
/// A word shorter than this finds too much to rank by.
const WORD_MIN_CHARS: usize = 4;
/// The blocks `find` ranks into a slice's targets, at most, beside those the plan already touches.
const TARGETS_MAX: usize = 40;

/// One stage's call, resolved: the project and the stage it drafts, the request, and the slice
/// its tail holds.
#[derive(Debug, Clone)]
pub struct Drafting {
    pub project: Project,
    pub stage: StageDecl,
    pub call: Call,
    pub slice: StageSlice,
}

/// The call `intent next` would make for `stage` (or the next one the plan lacks) of `project`
/// (or the only one), in the workspace at `root`, read in one transaction with the pack set it
/// records: the request exactly as the adapter would build it, and its slice.
pub fn stage_call(
    root: &Path,
    stage: Option<&str>,
    project: Option<&str>,
) -> Result<(Call, StageSlice), AppError> {
    let store = Store::open_existing(&workspace::store_path(root))?
        .ok_or_else(|| AppError::workspace_not_found(root))?;
    let snapshot = store.snapshot()?;
    let set = state::for_snapshot(&snapshot, root)?;
    let folded = live::folded(&snapshot, set.vocabulary())?;
    let drafting = drafting(&set, &folded, stage, project)?;
    Ok((drafting.call, drafting.slice))
}

/// Resolve the project and the stage over `folded`, then build the stage's call. An unknown
/// stage, `lint`, or no stage left is `invalid_argument` on `stage`; with `lint` or none left, its
/// hint is the exact `intent lint` for the project.
pub(crate) fn drafting(
    loaded: &Loaded,
    folded: &Folded,
    stage: Option<&str>,
    project: Option<&str>,
) -> Result<Drafting, AppError> {
    let projects = plan::projects(folded);
    let project = intent::current(&projects, project)?
        .ok_or_else(|| AppError::invalid_argument("project", "intent.no_project"))?
        .clone();
    let stages = intent::stages(loaded);
    let mut items = plan::items_of(folded, &project.id);
    plan::order(&mut items, &stages);
    let decl = match stage {
        Some(id) => stages.iter().find(|s| s.id == id).ok_or_else(|| {
            let ids: Vec<&str> = stages.iter().map(|s| s.id.as_str()).collect();
            let vars: &[(&str, &dyn std::fmt::Display)] =
                &[("stage", &crate::app::edit::q(id)), ("stages", &ids.join(", "))];
            AppError::new(ErrorCode::InvalidArgument, &[("what", &text::get("intent.stage_unknown", vars))])
                .details(json!({"field": "stage"}))
        })?,
        None => {
            plan::next_stage(&stages, &items).ok_or_else(|| lint_instead("intent.next_none", &project.id))?
        }
    };
    if decl.items.is_empty() {
        return Err(lint_instead("intent.next_lint", &project.id));
    }
    let slice = slice(loaded, folded, &project, decl, &stages);
    let call = call(loaded, decl, &slice)?;
    Ok(Drafting { project, stage: decl.clone(), call, slice })
}

/// `invalid_argument` on `stage` for a stage that drafts nothing, said by `key`, whose line and
/// hint name the exact `intent lint` for `project`, and whose details carry it as the `next`
/// handle, so the CLI keeps that hint in place of the leaf's schema handle.
fn lint_instead(key: &str, project: &NodeId) -> AppError {
    let lint = format!("planroom intent lint --project {}", text::shell_word(&project.0));
    let what = text::get(key, &[("lint", &lint)]);
    let next = Handle {
        command: CommandName("intent".into()),
        args: vec!["lint".into(), "--project".into(), project.0.clone()],
        label: text::label("intent.lint_label", &[]),
    };
    AppError::new(ErrorCode::InvalidArgument, &[("what", &what)])
        .details(json!({"field": "stage", "next": next}))
        .with_hint(lint)
}

/// The plan pack's stage prompt: its declaration and its text.
fn prompt(loaded: &Loaded) -> Result<(&PromptDecl, &str), AppError> {
    let missing = || AppError::internal("the plan pack has no stage prompt");
    let pack = loaded.packs.iter().find(|p| !p.file.stages.is_empty()).ok_or_else(missing)?;
    let decl = pack
        .file
        .prompts
        .iter()
        .find(|p| p.id == STAGE_PROMPT && p.role == PromptRole::Propose)
        .ok_or_else(missing)?;
    let text = pack
        .source
        .text(&format!("prompts/{}", decl.file))
        .or_else(|| pack.source.text(&decl.file))
        .ok_or_else(missing)?;
    Ok((decl, text))
}

/// The tier the stage prompt declares, which picks the pinned model a stage call takes.
pub(crate) fn stage_tier(loaded: &Loaded) -> Result<planroom_contract::ModelTier, AppError> {
    Ok(prompt(loaded)?.0.tier)
}

/// The reply schema every stage shares: every item type a plan item may take, the stages' and
/// each pack's `[[items]]` (DR-116), and every prop of any of them.
pub fn reply_schema(loaded: &Loaded) -> Value {
    let types = intent::item_types(loaded);
    let props: BTreeSet<String> = types.values().flat_map(|p| p.keys().cloned()).collect();
    let names: Vec<String> = types.keys().cloned().collect();
    draft_reply_schema(&names, &props.into_iter().collect::<Vec<_>>())
}

/// The call that drafts `stage` from `slice`.
fn call(loaded: &Loaded, stage: &StageDecl, slice: &StageSlice) -> Result<Call, AppError> {
    let (decl, prompt) = prompt(loaded)?;
    let tail = models::request_tail(slice, &question(loaded, stage))?;
    let purpose = format!("{PURPOSE_PREFIX}{}", stage.id);
    Ok(Call {
        purpose: purpose.clone(),
        tier: decl.tier,
        prefix: vec![prompt.to_owned()],
        ttl: decl.ttl,
        tail,
        output_schema: Some(reply_schema(loaded)),
        max_tokens: MAX_TOKENS,
        effort: None,
        ceiling_usd: STAGE_CEILING_USD,
        scenario: purpose,
    })
}

/// The question that ends a stage's request: its method, its checklist one step per line as the
/// pack writes it, each item type it writes with its props, then each pack's `[[items]]` types
/// with theirs, which any stage may write (DR-116). A prop with an `enum` names its values, as
/// `stakes (low, medium or high)`, since the reply's schema can't hold one prop's values.
pub fn question(loaded: &Loaded, stage: &StageDecl) -> String {
    let line = |id: &str, props: &Value| {
        let props: Vec<String> = props
            .get("properties")
            .and_then(Value::as_object)
            .map(|p| p.iter().map(|(name, schema)| prop_line(name, schema)).collect())
            .unwrap_or_default();
        let props = if props.is_empty() { "no props".to_owned() } else { props.join(", ") };
        format!("- {id}: {props}")
    };
    let any = loaded.packs.iter().flat_map(|p| &p.file.items).map(|t| line(&t.id, &t.props));
    let types: Vec<String> = stage.items.iter().map(|t| line(&t.id, &t.props)).chain(any).collect();
    text::get(
        "intent.next_question",
        &[
            ("stage", &stage.id),
            ("method", &stage.method),
            ("checklist", &stage.prompt.trim_end()),
            ("types", &types.join("\n")),
        ],
    )
}

/// A prop as the stage question names it: its name, and its values when its schema lists them.
fn prop_line(name: &str, schema: &Value) -> String {
    let values: Vec<&str> = schema
        .get("enum")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect();
    match values.split_last() {
        None => name.to_owned(),
        Some((last, [])) => format!("{name} ({last})"),
        Some((last, rest)) => format!("{name} ({} or {last})", rest.join(", ")),
    }
}

/// The item types a loaded rule requires a `kernel.touches` edge of, such as next actions.
fn touching_types(loaded: &Loaded) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let rules = loaded.packs.iter().flat_map(|p| &p.file.rules);
    for rule in rules.filter(|r| r.kind == RuleType::RequiresEdge && r.subject == plan::ITEM_KIND) {
        if rule.params.get("relation").and_then(Value::as_str) != Some(plan::TOUCHES) {
            continue;
        }
        match rule.params.get("props.type") {
            Some(Value::String(t)) => {
                out.insert(t.clone());
            }
            Some(Value::Object(m)) => {
                out.extend(
                    m.get("in")
                        .and_then(Value::as_array)
                        .into_iter()
                        .flatten()
                        .filter_map(|v| v.as_str().map(str::to_owned)),
                );
            }
            _ => {}
        }
    }
    out
}

/// The props an item holds now: its node's in the model, else its last pending `put`'s, less its
/// `type`.
fn props_of(folded: &Folded, id: &NodeId) -> BTreeMap<String, Value> {
    let node = folded.model.nodes.iter().find(|n| &n.id == id).cloned().or_else(|| {
        folded
            .overlay
            .ghosts
            .iter()
            .rev()
            .filter(|g| matches!(&g.edit.target, planroom_contract::Target::Node(t) if t == id))
            .find_map(plan::ghost_node)
    });
    let mut props: BTreeMap<String, Value> = node.map(|n| n.props.into_iter().collect()).unwrap_or_default();
    props.remove(plan::TYPE);
    props
}

/// The slice that drafts `stage` of `project`, masked and cut to [`SLICE_MAX_BYTES`].
fn slice(
    loaded: &Loaded,
    folded: &Folded,
    project: &Project,
    stage: &StageDecl,
    stages: &[StageDecl],
) -> StageSlice {
    let mut items = plan::items_of(folded, &project.id);
    plan::order(&mut items, stages);
    let slice_items: Vec<SliceItem> = items
        .iter()
        .map(|item| SliceItem {
            key: key_of(&item.id).to_owned(),
            item_type: item.item_type.clone(),
            name: item.name.clone(),
            parent: item.parent.as_ref().and_then(|p| {
                if *p == project.id {
                    Some(plan::PROJECT_KEY.to_owned())
                } else {
                    plan::in_project(p, &project.id).then(|| key_of(p).to_owned())
                }
            }),
            status: if item.live { Status::Applied } else { Status::Ghost },
            props: props_of(folded, &item.id),
            touches: item.touches.clone(),
        })
        .collect();
    let touching = touching_types(loaded);
    let targets = if stage.items.iter().any(|t| touching.contains(&t.id)) {
        let words = words(slice_items.iter().filter(|i| i.status == Status::Applied));
        let touched: BTreeSet<&NodeId> = items.iter().flat_map(|i| &i.touches).collect();
        targets(folded, &Ladder::of(&loaded.vocabulary), &words, &touched)
    } else {
        vec![]
    };
    let slice = StageSlice {
        project: project.id.clone(),
        name: project.name.clone(),
        stage: stage.id.clone(),
        items: slice_items,
        items_left_out: 0,
        targets,
        targets_left_out: 0,
    };
    fit(slice)
}

/// The words of `items`' names and text props, the text in their lists included: lowercase, split
/// on every character that isn't a letter or a digit, at least [`WORD_MIN_CHARS`] long, each once,
/// sorted, at most [`WORDS_MAX`].
fn words<'a>(items: impl Iterator<Item = &'a SliceItem>) -> Vec<String> {
    fn strings<'v>(value: &'v Value, out: &mut Vec<&'v str>) {
        match value {
            Value::String(s) => out.push(s),
            Value::Array(items) => items.iter().for_each(|v| strings(v, out)),
            _ => {}
        }
    }
    let mut found = BTreeSet::new();
    for item in items {
        let mut texts = vec![item.name.as_str()];
        item.props.values().for_each(|v| strings(v, &mut texts));
        for text in texts {
            for word in text.split(|c: char| !c.is_alphanumeric()) {
                if word.chars().count() >= WORD_MIN_CHARS {
                    found.insert(word.to_lowercase());
                }
            }
        }
    }
    found.into_iter().take(WORDS_MAX).collect()
}

/// The blocks `words` find on the map, outside the plan: ranked by how many words hit each, then
/// its best score, then its ID, at most [`TARGETS_MAX`]; then each block in `touched` the map
/// holds, by ID.
fn targets(
    folded: &Folded,
    ladder: &Ladder<'_>,
    words: &[String],
    touched: &BTreeSet<&NodeId>,
) -> Vec<SliceTarget> {
    let owned: Vec<Node> =
        folded.model.nodes.iter().filter(|n| !ladder.is_apart(&n.level)).cloned().collect();
    let mut hits: BTreeMap<&NodeId, (usize, u32)> = BTreeMap::new();
    for word in words {
        for hit in find::find(&owned, ladder, word) {
            let entry = hits.entry(&hit.node.id).or_insert((0, 0));
            entry.0 += 1;
            entry.1 = entry.1.max(hit.score);
        }
    }
    let mut ranked: Vec<(&NodeId, (usize, u32))> = hits.into_iter().collect();
    ranked.sort_by(|a, b| b.1.0.cmp(&a.1.0).then(b.1.1.cmp(&a.1.1)).then_with(|| a.0.cmp(b.0)));
    let name = |id: &NodeId| owned.iter().find(|n| &n.id == id).map(|n| n.name.clone());
    let mut out: Vec<SliceTarget> = ranked
        .into_iter()
        .take(TARGETS_MAX)
        .filter_map(|(id, _)| Some(SliceTarget { id: id.clone(), name: name(id)? }))
        .collect();
    for id in touched {
        if !out.iter().any(|t| &t.id == *id)
            && let Some(name) = name(id)
        {
            out.push(SliceTarget { id: (*id).clone(), name });
        }
    }
    out
}

/// The bytes the slice's JSON takes inside its fence, with every `<` escaped.
fn fenced_len(slice: &StageSlice) -> usize {
    let json = serde_json::to_value(slice).unwrap_or(Value::Null);
    let bytes = canonical::bytes(&json);
    bytes.len() + 5 * bytes.iter().filter(|b| **b == b'<').count()
}

/// `slice` masked by its marks, then cut to [`SLICE_MAX_BYTES`]: targets from the end, then each
/// item's text props to [`PROP_CUT_CHARS`], then items from the end, each cut counted.
pub fn fit(slice: StageSlice) -> StageSlice {
    let mut json = serde_json::to_value(&slice).unwrap_or(Value::Null);
    redact::value_of::<StageSlice>(&mut json);
    let mut slice: StageSlice = serde_json::from_value(json).unwrap_or(slice);
    while fenced_len(&slice) > SLICE_MAX_BYTES && slice.targets.pop().is_some() {
        slice.targets_left_out += 1;
    }
    if fenced_len(&slice) > SLICE_MAX_BYTES {
        for item in &mut slice.items {
            item.props.values_mut().for_each(cut);
        }
    }
    while fenced_len(&slice) > SLICE_MAX_BYTES && slice.items.pop().is_some() {
        slice.items_left_out += 1;
    }
    slice
}

/// Every string in `value` cut to [`PROP_CUT_CHARS`].
fn cut(value: &mut Value) {
    match value {
        Value::String(s) if s.chars().count() > PROP_CUT_CHARS => {
            *s = s.chars().take(PROP_CUT_CHARS).collect()
        }
        Value::Array(items) => items.iter_mut().for_each(cut),
        Value::Object(map) => map.values_mut().for_each(cut),
        _ => {}
    }
}

/// Why a reply couldn't be used, as `model_reply_invalid`'s `reason`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unusable {
    StopReason,
    Unparsable,
    InvalidItems,
    NothingNew,
}

impl Unusable {
    pub fn as_str(self) -> &'static str {
        match self {
            Unusable::StopReason => "stop_reason",
            Unusable::Unparsable => "unparsable",
            Unusable::InvalidItems => "invalid_items",
            Unusable::NothingNew => "nothing_new",
        }
    }
}

/// `model_reply_invalid` for the call `purpose` keyed `key`: nothing was written, and the record
/// is kept.
pub fn unusable(purpose: &str, key: &str, why: Unusable, field: Option<&str>) -> AppError {
    AppError::new(ErrorCode::ModelReplyInvalid, &[("purpose", &purpose), ("reason", &why.as_str())])
        .details(json!({"purpose": purpose, "key": key, "reason": why.as_str(), "field": field}))
}

/// A stage's reply read back as a proposal into `project`, and how many touch targets it named
/// that aren't one block each on the map. A parent that is no key of the reply, no item of the
/// project and not `project` is left out, so the item goes under the project or keeps its place.
/// A target is looked up on the ladder of `vocabulary`, the set `folded` was folded with, so a
/// block at a pack's own level is found by its name too.
pub fn proposal(
    reply: &Reply,
    purpose: &str,
    key: &str,
    folded: &Folded,
    project: &Project,
    vocabulary: &Vocabulary,
) -> Result<(PlanProposal, usize), AppError> {
    if reply.stop_reason.as_deref() != Some("end_turn") {
        return Err(unusable(purpose, key, Unusable::StopReason, None));
    }
    let draft: DraftReply =
        serde_json::from_str(&reply.text).map_err(|_| unusable(purpose, key, Unusable::Unparsable, None))?;
    let existing: Vec<NodeId> = plan::items_of(folded, &project.id).into_iter().map(|i| i.id).collect();
    let batch: BTreeSet<String> = draft.items.iter().map(|i| i.key.clone()).collect();
    let known = |given: &str| {
        batch.contains(given)
            || given == plan::PROJECT_KEY
            || given == project.id.0
            || existing.iter().any(|id| id.0 == given || key_of(id) == given)
    };
    let ladder = Ladder::of(vocabulary);
    let pool: Vec<Node> = folded.model.nodes.iter().filter(|n| !ladder.is_apart(&n.level)).cloned().collect();
    let mut dropped = 0;
    let mut items = Vec::with_capacity(draft.items.len());
    for (i, item) in draft.items.into_iter().enumerate() {
        let mut props = BTreeMap::new();
        for prop in item.props {
            let value = match prop.value {
                DraftValue::Text(s) => Value::String(s),
                DraftValue::List(list) => Value::Array(list.into_iter().map(Value::String).collect()),
            };
            if props.insert(prop.name, value).is_some() {
                let field = format!("items[{i}].props");
                return Err(unusable(purpose, key, Unusable::InvalidItems, Some(&field)));
            }
        }
        let mut touches = Vec::new();
        for given in item.touches {
            match lookup::lookup(&pool, &given, ladder.finest(), &ladder) {
                Lookup::Found(node) => touches.push(node.id.0.clone()),
                Lookup::Missing(_) | Lookup::Ambiguous(_) => dropped += 1,
            }
        }
        items.push(ProposedItem {
            key: item.key,
            item_type: item.item_type,
            name: item.name,
            parent: item.parent.filter(|p| known(p)),
            props,
            touches,
        });
    }
    Ok((PlanProposal { project: Some(project.id.0.clone()), stage: None, items }, dropped))
}

/// Whether a reply's usage shows its marked prefix neither written nor read: below the model's
/// minimum, so the cache never applied.
pub fn cache_not_applied(reply: &Reply) -> bool {
    reply.usage.cache_write == 0 && reply.usage.cache_read == 0
}

#[cfg(test)]
mod tests {
    use planroom_contract::{
        CacheTtl, Command, EditResult, EventId, Handle, IntentNextReq, Warning, WarningCode,
    };
    use planroom_models::claude;

    use super::*;
    use crate::app::edit::{TARGETS_MAX_BYTES, q, within};
    use crate::packs;

    fn config() -> planroom_contract::config::Config {
        let env = |_: &str| None;
        crate::config::load(&crate::config::Sources { user: None, workspace: None, env: &env })
            .unwrap()
            .config
    }

    fn empty(stage: &str) -> StageSlice {
        StageSlice {
            project: plan::project_id("add-retries-to-the-worker"),
            name: "Add retries to the worker".into(),
            stage: stage.into(),
            items: vec![],
            items_left_out: 0,
            targets: vec![],
            targets_left_out: 0,
        }
    }

    #[test]
    fn the_shared_prefix_is_at_least_16_kb_and_the_same_for_every_stage() {
        let loaded = packs::loaded().unwrap();
        let config = config();
        let mut prefixes = BTreeSet::new();
        for stage in intent::stages(loaded).iter().filter(|s| !s.items.is_empty()) {
            let call = call(loaded, stage, &empty(&stage.id)).unwrap();
            assert_eq!(call.ttl, CacheTtl::H1, "{}", stage.id);
            assert_eq!(call.purpose, format!("plan.stage.{}", stage.id));
            let built = claude::build(&config.claude, &call);
            assert!(built.prefix.len() >= 16_000, "{}: the prefix is {} bytes", stage.id, built.prefix.len());
            prefixes.insert((built.prefix_hash.clone(), built.prefix.len()));
        }
        assert_eq!(prefixes.len(), 1, "one prefix for every stage: {prefixes:?}");
        let (hash, bytes) = prefixes.into_iter().next().unwrap();
        println!("the stage prefix: {bytes} bytes, prefix_hash {hash}");
    }

    #[test]
    fn the_reply_schema_offers_every_items_type() {
        let loaded = packs::loaded().unwrap();
        let schema = reply_schema(loaded).to_string();
        let any = intent::any_stage_types(loaded);
        assert!(!any.is_empty(), "the plan pack declares an [[items]] type");
        let types = intent::item_types(loaded);
        for id in &any {
            assert!(schema.contains(&format!("\"{id}\"")), "the type enum holds {id}: {schema}");
            for prop in types[id].keys() {
                assert!(
                    schema.contains(&format!("\"{prop}\"")),
                    "the prop enum holds {id}'s {prop}: {schema}"
                );
            }
        }
        // Every drafting stage's question lists each [[items]] type with its props, after its own.
        for stage in intent::stages(loaded).iter().filter(|s| !s.items.is_empty()) {
            let asked = question(loaded, stage);
            let own = asked.find(&format!("- {}: ", stage.items[0].id)).unwrap();
            for id in &any {
                let props: Vec<String> =
                    types[id].iter().map(|(name, schema)| prop_line(name, schema)).collect();
                let line = format!("- {id}: {}", props.join(", "));
                let at = asked.find(&line).unwrap_or_else(|| panic!("{}: no line {line}: {asked}", stage.id));
                assert!(at > own, "{}: the stage's own types come first: {asked}", stage.id);
            }
            // A prop with values names them, so a drafted `stakes` is one the schema takes.
            assert!(asked.contains("stakes (low, medium or high)"), "{}: {asked}", stage.id);
        }
    }

    #[test]
    fn a_drafted_touch_names_a_block_at_a_packs_own_level() {
        // A set that adds a level below `container`, and one block at it, which the reply names
        // by its name alone.
        let builtins = packs::vocabulary().unwrap();
        let mut vocabulary = builtins.clone();
        let at = vocabulary.levels.iter().position(|l| l.id.as_str() == "container").unwrap();
        let zone = planroom_contract::LevelId::from("zone");
        vocabulary.levels.insert(
            at + 1,
            planroom_contract::LevelVocab {
                id: zone.clone(),
                label: "zone".into(),
                one: "{count} zone".into(),
                many: "{count} zones".into(),
                apart: false,
                pack: "zoned".into(),
            },
        );
        vocabulary.kinds.push(planroom_contract::KindVocab {
            kind: planroom_contract::Kind("zoned.zone".into()),
            level: zone.clone(),
            id_prefix: "zone".into(),
            label: "Zone".into(),
            props: json!({}),
            tone: None,
        });
        let block = Node {
            id: NodeId("app:zone:alpha".into()),
            kind: planroom_contract::Kind("zoned.zone".into()),
            level: zone,
            parent: None,
            name: "zalpha".into(),
            aliases: vec![],
            props: Default::default(),
            layer: planroom_contract::Layer::Actual,
            provenance: planroom_contract::Provenance::Extracted {
                extractor: "zoned/zones".into(),
                tier: planroom_contract::Tier::Universal,
            },
            evidence: vec![],
        };
        let facts = planroom_kernel::model::Facts { nodes: vec![block], edges: vec![], hints: vec![] };
        let folded = planroom_kernel::fold::fold_full(&facts, &[], &vocabulary, 0);
        assert!(folded.model.nodes.iter().any(|n| n.name == "zalpha"), "the fold keeps the block");
        let project =
            Project { id: plan::project_id("zones"), name: "Zones".into(), live: true, pending: vec![] };
        let draft = json!({"items": [{
            "key": "k1", "type": "task", "name": "Split the zone", "parent": null,
            "touches": ["zalpha"], "props": [],
        }]});
        let reply = Reply {
            text: draft.to_string(),
            stop_reason: Some("end_turn".into()),
            usage: Default::default(),
            cost_usd: None,
            generation_id: None,
            model_reported: None,
            replayed: false,
        };
        // On the ladder of the set the map was folded with, the name finds the block.
        let (drafted, dropped) =
            proposal(&reply, "plan.stage.next_actions", "k", &folded, &project, &vocabulary).unwrap();
        assert_eq!(dropped, 0);
        assert_eq!(drafted.items[0].touches, vec!["app:zone:alpha".to_owned()]);
        // On the built-ins' ladder, which lacks the level, the same name finds nothing.
        let (drafted, dropped) =
            proposal(&reply, "plan.stage.next_actions", "k", &folded, &project, builtins).unwrap();
        assert_eq!(dropped, 1);
        assert!(drafted.items[0].touches.is_empty());
    }

    #[test]
    fn a_secret_across_the_prop_cut_is_masked_whole() {
        let mut slice = empty("next_actions");
        // The key starts at character 190, so a cut to 200 before masking would leave its first
        // 10 characters, short of the shape that masks it.
        let planted = format!("{} AKIAZ7PLANTEDEXMPL01 and the rest of the method", "m".repeat(189));
        assert_eq!(planted.find("AKIA"), Some(190));
        for i in 0..40 {
            slice.items.push(SliceItem {
                key: format!("work-{i:02}"),
                item_type: "work".into(),
                name: format!("Work item {i}"),
                parent: Some("project".into()),
                status: Status::Applied,
                props: BTreeMap::from([("method".to_owned(), json!(planted))]),
                touches: vec![],
            });
        }
        assert!(fenced_len(&slice) > SLICE_MAX_BYTES);
        let fitted = fit(slice);
        assert!(fenced_len(&fitted) <= SLICE_MAX_BYTES);
        let tail = models::request_tail(&fitted, "Draft it.").unwrap();
        assert!(!tail.join("\n").contains("AKIAZ7PLAN"), "{}", tail[0]);
        assert!(tail[0].contains("[redacted]"), "{}", tail[0]);
    }

    #[test]
    fn a_stage_slice_is_masked_by_its_marks() {
        assert!(redact::is_listed::<StageSlice>());
    }

    #[test]
    fn a_fifty_item_draft_keeps_its_reply_to_its_budget() {
        let project = "p".repeat(200);
        let slug = "p".repeat(48);
        let ids: Vec<String> =
            (0..50).map(|i| plan::item_id(&slug, &format!("k{i:02}{}", "x".repeat(61))).0).collect();
        let targets = within(ids.iter().cloned(), TARGETS_MAX_BYTES);
        let shown: Vec<String> = ids.iter().take(3).map(|id| q(id)).collect();
        let vars: &[(&str, &dyn std::fmt::Display)] = &[
            ("count", &50),
            ("stage", &"next_actions"),
            ("project", &q(&project)),
            ("ids", &shown.join(", ")),
        ];
        let line = format!(
            "{} {}",
            text::get("intent.drafted", vars),
            text::get("intent.targets_dropped", &[("count", &150)])
        );
        let summary = text::get("edit.ghost", &[("line", &line)]);
        let event = EventId("0193a0b4-7c3e-7d2a-9f10-2b3c4d5e6f70".into());
        let result = EditResult {
            event: event.clone(),
            count: 200,
            status: Status::Ghost,
            more_targets: (ids.len() - targets.len()) as u32,
            targets,
            undo: Handle {
                command: planroom_contract::CommandName("edit".into()),
                args: vec!["--base".into(), "999999".into(), "undo".into(), event.0.clone()],
                label: text::label("edit.undo_label", &[]),
            },
        };
        let warning = Warning {
            code: WarningCode::CacheNotApplied,
            message: text::get("warning.cache_not_applied", &[]),
            details: json!({}),
        };
        let tokens = intent::sent_tokens(&result, &summary)
            + serde_json::to_string(&warning).unwrap().len().div_ceil(4)
            + 40;
        let budget = Command::IntentNext(IntentNextReq::default()).spec().budget_tokens as usize;
        assert!(tokens <= budget, "{tokens} tokens over intent next's {budget}");
    }
}
