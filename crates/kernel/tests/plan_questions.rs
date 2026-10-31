//! A question's state as the plan derives it (DR-116): what rests on it, its status, whether its
//! answer kept the assumption, its score, its rank and the sticky pick, all over the draft view
//! (the model plus each pending ghost) and never stored. Events are built as values and folded in
//! memory, with no I/O and no pack file.

use std::collections::{BTreeMap, BTreeSet};

use planroom_contract::{
    Actor, Client, Edit, Event, EventBody, EventId, Layer, NodeId, Op, QuestionState, QuestionStatus,
    RuleDecl, RuleType, Seq, Status, Surface, Target, Timestamp, Verdict, Vocabulary,
};
use planroom_kernel::fold::fold_full;
use planroom_kernel::model::Facts;
use planroom_kernel::plan::{ITEM_KIND, PROJECT_KIND, in_project, items_of, questions};
use serde_json::{Value, json};

const PROJECT: &str = "plan:retries:project";

fn id(s: &str) -> NodeId {
    NodeId(s.into())
}

fn qid(key: &str) -> NodeId {
    id(&format!("plan:retries:{key}"))
}

fn person() -> Actor {
    Actor::Person { id: "ann".into(), name: None }
}

fn agent() -> Actor {
    Actor::Agent { session: "1a2b3c4d".into(), role: "chat".into(), key: None }
}

/// A plan being written event by event; `seq` and `ev-<seq>` are assigned in order.
struct Plan {
    events: Vec<Event>,
}

impl Plan {
    /// A plan whose project a person has accepted.
    fn new() -> Self {
        let mut plan = Plan { events: vec![] };
        plan.push(
            person(),
            Status::Applied,
            EventBody::Edit(put(
                Target::Node(id(PROJECT)),
                json!({"kind": PROJECT_KIND, "level": "intent", "name": "Add retries"}),
            )),
        );
        plan
    }

    fn push(&mut self, actor: Actor, status: Status, body: EventBody) -> EventId {
        let seq = self.events.len() as Seq + 1;
        let event = Event {
            seq,
            id: EventId(format!("ev-{seq}")),
            idem: format!("cmd{seq}"),
            request_hash: "h".into(),
            at: Timestamp("2026-10-28T12:00:00Z".into()),
            actor,
            client: Client { surface: Surface::Cli, id: "cli:1".into() },
            project: Some(id(PROJECT)),
            cause: None,
            status,
            body,
        };
        let at = event.id.clone();
        self.events.push(event);
        at
    }

    /// A person's applied `put` of an item under the project.
    fn item(&mut self, key: &str, props: Value) -> EventId {
        self.item_under(key, PROJECT, props)
    }

    fn item_under(&mut self, key: &str, parent: &str, props: Value) -> EventId {
        self.push(person(), Status::Applied, EventBody::Edit(put_item(key, parent, props)))
    }

    /// An agent's pending ghost `put` of an item under the project.
    fn ghost(&mut self, key: &str, props: Value) -> EventId {
        self.push(agent(), Status::Ghost, EventBody::Edit(put_item(key, PROJECT, props)))
    }

    fn set(&mut self, key: &str, field: &str, value: Value) {
        let fields = BTreeMap::from([(field.to_owned(), value)]);
        let edit = Edit {
            op: Op::Set,
            target: Target::Node(qid(key)),
            layer: Layer::Intended,
            fields,
            base: 0,
            over: BTreeMap::new(),
        };
        self.push(person(), Status::Applied, EventBody::Edit(edit));
    }

    fn review(&mut self, of: &EventId, verdict: Verdict) {
        self.push(person(), Status::Applied, EventBody::Review { of: of.clone(), verdict });
    }

    fn folded(&self) -> planroom_kernel::fold::Folded {
        fold_full(&Facts::default(), &self.events, &Vocabulary::default(), self.events.len() as Seq)
    }

    /// The questions in rank order, as the engine reads them.
    fn ranked(&self) -> Vec<(String, QuestionState)> {
        let items = items_of(&self.folded(), &id(PROJECT));
        let types = BTreeSet::from(["question".to_owned()]);
        questions(&items, &types).into_iter().map(|(id, state)| (id.0, state)).collect()
    }
}

fn put(target: Target, fields: Value) -> Edit {
    let fields: BTreeMap<String, Value> = serde_json::from_value(fields).unwrap();
    Edit { op: Op::Put, target, layer: Layer::Intended, fields, base: 0, over: BTreeMap::new() }
}

fn put_item(key: &str, parent: &str, props: Value) -> Edit {
    put(
        Target::Node(qid(key)),
        json!({"kind": ITEM_KIND, "level": "intent", "name": key, "parent": parent, "props": props}),
    )
}

fn work(plan: &mut Plan, keys: &[&str]) {
    for key in keys {
        plan.item(key, json!({"type": "work"}));
    }
}

fn question(rests_on: &[&str], extra: Value) -> Value {
    let mut props = json!({
        "type": "question",
        "assumption": "Redis",
        "rests_on": rests_on.iter().map(|k| format!("plan:retries:{k}")).collect::<Vec<_>>(),
    });
    for (name, value) in extra.as_object().cloned().unwrap_or_default() {
        props[name] = value;
    }
    props
}

fn order(ranked: &[(String, QuestionState)]) -> Vec<&str> {
    ranked.iter().map(|(id, _)| id.strip_prefix("plan:retries:").unwrap()).collect()
}

fn state<'a>(ranked: &'a [(String, QuestionState)], key: &str) -> &'a QuestionState {
    &ranked.iter().find(|(id, _)| id == &format!("plan:retries:{key}")).unwrap().1
}

#[test]
fn a_question_with_nothing_live_resting_on_it_is_dropped() {
    let mut plan = Plan::new();
    work(&mut plan, &["w1"]);
    plan.item("bare", json!({"type": "question", "assumption": "Redis"}));
    plan.item("lost", question(&["nowhere"], json!({})));
    let ranked = plan.ranked();
    assert_eq!(ranked.len(), 2);
    for (_, state) in &ranked {
        assert_eq!(state.status, QuestionStatus::Dropped);
        assert_eq!((state.rests, state.score), (0, 0));
        assert!(!state.sticky);
    }
}

#[test]
fn the_score_is_the_stakes_weight_times_what_rests() {
    let mut plan = Plan::new();
    let keys: Vec<String> = (1..=10).map(|n| format!("w{n}")).collect();
    let keys: Vec<&str> = keys.iter().map(String::as_str).collect();
    work(&mut plan, &keys);
    plan.item("q-none", question(&keys[..2], json!({})));
    plan.item("q-medium", question(&keys[..1], json!({"stakes": "medium"})));
    plan.item("q-low", question(&keys, json!({"stakes": "low"})));
    plan.item("q-high", question(&keys[..2], json!({"stakes": "high"})));
    let ranked = plan.ranked();
    assert_eq!(order(&ranked), ["q-high", "q-low", "q-medium", "q-none"]);
    let scores: Vec<u32> = ranked.iter().map(|(_, s)| s.score).collect();
    assert_eq!(scores, [18, 10, 3, 2]);
    assert_eq!(state(&ranked, "q-low").rests, 10);

    // Equal scores order by higher stakes, whatever the IDs (the mediums on 1 beat the low on 3),
    // and equal stakes then by the lower ID.
    plan.item("q-a-low", question(&keys[..3], json!({"stakes": "low"})));
    plan.item("q-z-medium", question(&keys[..1], json!({"stakes": "medium"})));
    let ranked = plan.ranked();
    assert_eq!(order(&ranked), ["q-high", "q-low", "q-medium", "q-z-medium", "q-a-low", "q-none"]);
}

#[test]
fn a_descendant_counts_once_however_many_named_ancestors() {
    let mut plan = Plan::new();
    plan.item("a", json!({"type": "work"}));
    plan.item_under("b", "plan:retries:a", json!({"type": "work"}));
    plan.item_under("c", "plan:retries:b", json!({"type": "work"}));
    plan.item("all", question(&["a", "b", "c"], json!({})));
    plan.item("top", question(&["a"], json!({})));
    let ranked = plan.ranked();
    assert_eq!(state(&ranked, "all").rests, 3);
    assert_eq!(state(&ranked, "top").rests, 3);

    // A question under `a` that names `a` never counts itself.
    let mut own = Plan::new();
    own.item("a", json!({"type": "work"}));
    own.item_under("b", "plan:retries:a", json!({"type": "work"}));
    own.item_under("c", "plan:retries:b", json!({"type": "work"}));
    own.item_under("inner", "plan:retries:a", question(&["a"], json!({})));
    assert_eq!(state(&own.ranked(), "inner").rests, 3);
}

#[test]
fn a_rejected_resting_item_no_longer_counts() {
    let mut plan = Plan::new();
    let proposed = plan.ghost("g", json!({"type": "work"}));
    plan.item("q", question(&["g"], json!({})));
    let ranked = plan.ranked();
    assert_eq!(state(&ranked, "q").rests, 1);
    assert_eq!(state(&ranked, "q").status, QuestionStatus::Open);

    plan.review(&proposed, Verdict::Reject);
    let ranked = plan.ranked();
    assert_eq!(state(&ranked, "q").rests, 0);
    assert_eq!(state(&ranked, "q").status, QuestionStatus::Dropped);
}

#[test]
fn an_answer_equal_to_the_assumption_reads_as_kept() {
    let mut plan = Plan::new();
    work(&mut plan, &["w"]);
    plan.item("kept", question(&["w"], json!({"answer": " Redis "})));
    plan.item("changed", question(&["w"], json!({"answer": "Postgres"})));
    plan.item("blank", question(&["w"], json!({"answer": ""})));
    let ranked = plan.ranked();
    let kept = state(&ranked, "kept");
    assert_eq!((kept.status, kept.kept), (QuestionStatus::Answered, true));
    let changed = state(&ranked, "changed");
    assert_eq!((changed.status, changed.kept), (QuestionStatus::Answered, false));
    let blank = state(&ranked, "blank");
    assert_eq!((blank.status, blank.kept), (QuestionStatus::Open, false));
}

#[test]
fn the_sticky_slot_holds_the_top_two_open_questions() {
    let mut plan = Plan::new();
    work(&mut plan, &["w1", "w2", "w3", "w4"]);
    plan.item("qa", question(&["w1", "w2", "w3", "w4"], json!({"answer": "Postgres"})));
    plan.item("qb", question(&["w1", "w2", "w3"], json!({})));
    plan.item("qc", question(&["w1", "w2"], json!({})));
    plan.item("qd", question(&["w1"], json!({})));
    plan.item("qe", question(&[], json!({})));
    let ranked = plan.ranked();
    assert_eq!(order(&ranked), ["qa", "qb", "qc", "qd", "qe"]);
    let sticky: Vec<bool> = ranked.iter().map(|(_, s)| s.sticky).collect();
    assert_eq!(sticky, [false, true, true, false, false]);
    assert_eq!(state(&ranked, "qa").status, QuestionStatus::Answered);
    assert_eq!(state(&ranked, "qe").status, QuestionStatus::Dropped);
}

#[test]
fn a_reproposal_that_moves_the_resting_items_moves_the_sticky_slot_on() {
    let mut plan = Plan::new();
    work(&mut plan, &["w1", "w2", "w3", "w4"]);
    plan.item("qa", question(&["w1", "w2", "w3"], json!({})));
    plan.item("qb", question(&["w1", "w2"], json!({})));
    plan.item("qc", question(&["w1"], json!({})));
    let sticky = |plan: &Plan| -> Vec<String> {
        let ranked = plan.ranked();
        ranked.iter().filter(|(_, s)| s.sticky).map(|(id, _)| id.clone()).collect()
    };
    assert_eq!(sticky(&plan), ["plan:retries:qa", "plan:retries:qb"]);

    let all: Vec<&str> = vec!["w1", "w2", "w3", "w4"];
    plan.set(
        "qc",
        "props.rests_on",
        json!(all.iter().map(|k| format!("plan:retries:{k}")).collect::<Vec<_>>()),
    );
    assert_eq!(sticky(&plan), ["plan:retries:qc", "plan:retries:qa"]);
}

#[test]
fn pending_ghosts_count_toward_the_rank_and_not_toward_lint() {
    let rules = [
        rule("question_assumption", json!({"prop": "assumption", "props.type": "question"})),
        rule("question_rests_on", json!({"prop": "rests_on", "props.type": "question"})),
    ];
    let lint_of = |plan: &Plan| -> Vec<(usize, String)> {
        let folded = plan.folded();
        let rules: Vec<&RuleDecl> = rules.iter().collect();
        planroom_kernel::lint::lint(&folded.model, &rules, |n| in_project(&n.id, &id(PROJECT)))
            .into_iter()
            .map(|f| (f.rule, f.subject.0))
            .collect()
    };

    let mut plan = Plan::new();
    plan.ghost("g", json!({"type": "work"}));
    plan.push(
        agent(),
        Status::Ghost,
        EventBody::Edit(put_item("q", PROJECT, json!({"type": "question", "rests_on": ["plan:retries:g"]}))),
    );
    let ranked = plan.ranked();
    let q = state(&ranked, "q");
    assert_eq!((q.rests, q.status, q.sticky), (1, QuestionStatus::Open, true));
    assert!(lint_of(&plan).is_empty());

    // A person's own put of the same question supersedes the ghost, and lint now reads it.
    plan.item("q", json!({"type": "question", "rests_on": ["plan:retries:g"]}));
    assert_eq!(lint_of(&plan), [(0, "plan:retries:q".to_owned())]);
    assert_eq!(state(&plan.ranked(), "q").rests, 1);
}

fn rule(rule_id: &str, params: Value) -> RuleDecl {
    RuleDecl {
        id: rule_id.into(),
        kind: RuleType::RequiresProp,
        subject: ITEM_KIND.into(),
        params,
        message: format!("{rule_id} fails."),
    }
}
