//! Compare-and-set over the fold (architecture.md, "Conflicts"; DR-5, DR-7, DR-61, DR-73): every
//! input is built as a value, so these tests do no I/O.

use std::collections::BTreeMap;

use planroom_contract::{
    Actor, Client, Edit, Event, EventBody, EventId, Kind, Layer, LevelId, Node, NodeId, Op, Provenance,
    Rename, ScanStats, Seq, Status, Surface, Target, Tier, Timestamp, Vocabulary,
};
use planroom_kernel::cas::{
    Authority, Check, Proposed, check_edit, check_edits, check_undo, copy_for_review,
};
use planroom_kernel::fold::{Folded, Writer, fold_full};
use planroom_kernel::model::Facts;
use serde_json::{Value, json};

fn node(id: &str, level: LevelId, parent: Option<&str>) -> Node {
    Node {
        id: NodeId(id.into()),
        kind: Kind("k.x".into()),
        level,
        parent: parent.map(|p| NodeId(p.into())),
        name: id.rsplit(':').next().unwrap().into(),
        aliases: vec![],
        props: Default::default(),
        layer: Layer::Actual,
        provenance: Provenance::Extracted { extractor: "p/x".into(), tier: Tier::Universal },
        evidence: vec![],
    }
}

/// A repo with two boxes, `a` and `b`, and an item `x` inside `a`.
fn facts() -> Facts {
    Facts {
        nodes: vec![
            node("r:x:repo", LevelId::from("system"), None),
            node("r:x:a", LevelId::from("component"), Some("r:x:repo")),
            node("r:x:b", LevelId::from("component"), Some("r:x:repo")),
            node("r:x:item", LevelId::from("code"), Some("r:x:a")),
        ],
        ..Facts::default()
    }
}

fn id(s: &str) -> NodeId {
    NodeId(s.into())
}

fn person(surface: Surface, client: &str) -> Writer {
    Writer {
        actor: Actor::Person { id: "person:ann".into(), name: Some("Ann".into()) },
        client: Client { surface, id: client.into() },
    }
}

fn tab(n: u8) -> Writer {
    person(Surface::Canvas, &format!("tab_{n}"))
}

fn press() -> Writer {
    person(Surface::Mod, "mod:s1")
}

fn agent() -> Writer {
    Writer {
        actor: Actor::Agent { session: "1a2b3c4d".into(), role: "chat".into(), key: None },
        client: Client { surface: Surface::Cli, id: "agent:1a2b3c4d:chat".into() },
    }
}

fn set(target: &str, field: &str, value: Value, base: Seq) -> Edit {
    Edit {
        op: Op::Set,
        target: Target::Node(id(target)),
        layer: Layer::Actual,
        fields: BTreeMap::from([(field.to_owned(), value)]),
        base,
        over: BTreeMap::new(),
    }
}

fn rename(target: &str, name: &str, base: Seq) -> Edit {
    set(target, "name", json!(name), base)
}

/// The event log, as the store would hand it to the fold.
#[derive(Default)]
struct Log {
    events: Vec<Event>,
}

impl Log {
    fn push(&mut self, writer: &Writer, status: Status, body: EventBody) -> EventId {
        let seq = self.events.len() as Seq + 1;
        let id = EventId(format!("ev-{seq:04}"));
        self.events.push(Event {
            seq,
            id: id.clone(),
            idem: format!("k{seq}"),
            request_hash: "h".into(),
            at: Timestamp("2026-10-26T12:00:00Z".into()),
            actor: writer.actor.clone(),
            client: writer.client.clone(),
            project: None,
            cause: None,
            status,
            body,
        });
        id
    }

    fn edit(&mut self, writer: &Writer, status: Status, edit: Edit) -> EventId {
        self.push(writer, status, EventBody::Edit(edit))
    }

    fn seq(&self) -> Seq {
        self.events.len() as Seq
    }

    fn fold(&self, facts: &Facts) -> Folded {
        fold_full(facts, &self.events, &Vocabulary::default(), self.seq())
    }
}

fn proposed(writer: Writer, base: Seq) -> Proposed {
    let authority = Authority::of(&writer.actor);
    Proposed { writer, authority, base }
}

fn name_of(folded: &Folded, node: &str) -> String {
    folded.model.nodes.iter().find(|n| n.id.0 == node).unwrap().name.clone()
}

#[test]
fn two_tabs_of_one_person_conflict() {
    let mut log = Log::default();
    log.edit(&tab(1), Status::Applied, rename("r:x:item", "Bag", 0));
    let folded = log.fold(&facts());

    let stale = check_edit(&folded, &proposed(tab(2), 0), &rename("r:x:item", "Basket", 0));
    let Check::Conflict { clashes, card } = stale else { panic!("{stale:?}") };
    assert!(card, "a person's stale write deals a card");
    let theirs = clashes[0].theirs.as_ref().unwrap();
    assert_eq!((theirs.seq, &theirs.writer), (1, &tab(1)));
    assert_eq!(clashes[0].field, "name");

    // The same tab never conflicts with its own write, and a tab that saw the write doesn't either.
    assert_eq!(check_edit(&folded, &proposed(tab(1), 0), &rename("r:x:item", "Basket", 0)), Check::Applied);
    assert_eq!(check_edit(&folded, &proposed(tab(2), 1), &rename("r:x:item", "Basket", 1)), Check::Applied);
    // Another field of the same node is no conflict: revisions are per field.
    assert_eq!(
        check_edit(&folded, &proposed(tab(2), 0), &set("r:x:item", "parent", json!("r:x:b"), 0)),
        Check::Applied
    );
}

#[test]
fn a_tab_and_a_mod_press_conflict() {
    let mut log = Log::default();
    log.edit(&press(), Status::Applied, rename("r:x:item", "Bag 2", 0));
    let folded = log.fold(&facts());

    let tab_write = check_edit(&folded, &proposed(tab(1), 0), &rename("r:x:item", "Basket", 0));
    assert!(
        matches!(&tab_write, Check::Conflict { card: true, clashes } if clashes[0].theirs.as_ref().unwrap().writer == press())
    );
    assert_eq!(tab_write.status(), Some(Status::Conflict));

    // An agent's stale write is stored as a conflict too, and deals no card.
    let agent_write = check_edit(&folded, &proposed(agent(), 0), &rename("r:x:item", "Sack", 0));
    assert!(matches!(agent_write, Check::Conflict { card: false, .. }), "{agent_write:?}");
}

#[test]
fn a_person_edit_supersedes_a_pending_ghost() {
    let mut log = Log::default();
    let ghost = log.edit(&agent(), Status::Ghost, rename("r:x:item", "Sack", 0));
    let other = log.edit(&agent(), Status::Ghost, set("r:x:b", "name", json!("Bin"), 0));
    let folded = log.fold(&facts());
    assert_eq!(folded.overlay.ghosts.len(), 2);
    assert_eq!(name_of(&folded, "r:x:item"), "item", "a ghost stays out of the model");
    assert_eq!(folded.revision(&Target::Node(id("r:x:item")), "name"), None, "a ghost writes no revision");

    // The person's edit over the ghost's field is no conflict, whatever its base.
    let check = check_edit(&folded, &proposed(tab(1), 0), &rename("r:x:item", "Bag", 0));
    assert_eq!(check, Check::Applied);
    let by = log.edit(&tab(1), Status::Applied, rename("r:x:item", "Bag", 0));
    let folded = log.fold(&facts());
    assert_eq!(name_of(&folded, "r:x:item"), "Bag");
    let pending: Vec<&EventId> = folded.overlay.ghosts.iter().map(|g| &g.event).collect();
    assert_eq!(pending, [&other], "only the ghost on another field stays pending");
    assert!(folded.diffs.contains(&planroom_kernel::fold::FoldDiff::GhostSuperseded { ghost, by }));
}

#[test]
fn a_write_over_an_undo_since_base_conflicts() {
    let mut log = Log::default();
    let edit = log.edit(&tab(1), Status::Applied, rename("r:x:item", "Bag", 0));
    // Tab 2 reads the view at 1; then tab 1 undoes its rename.
    let base = log.seq();
    let undo = log.push(&tab(1), Status::Applied, EventBody::Undo { of: edit });
    let folded = log.fold(&facts());
    assert_eq!(name_of(&folded, "r:x:item"), "item", "the undo changed the name back");
    let rev = folded.revision(&Target::Node(id("r:x:item")), "name").unwrap();
    assert_eq!((rev.seq, &rev.event, &rev.writer), (2, &undo, &tab(1)), "an undo is a write");

    let stale = check_edit(&folded, &proposed(tab(2), base), &rename("r:x:item", "Basket", base));
    assert!(
        matches!(&stale, Check::Conflict { card: true, clashes } if clashes[0].theirs.as_ref().unwrap().seq == 2),
        "{stale:?}"
    );
    assert_eq!(check_edit(&folded, &proposed(tab(2), 2), &rename("r:x:item", "Basket", 2)), Check::Applied);

    // A stale undo conflicts like any write: tab 2, at base 0, undoing a rename it never saw.
    let mut log = Log::default();
    let edit = log.edit(&tab(1), Status::Applied, rename("r:x:item", "Bag", 0));
    let folded = log.fold(&facts());
    assert!(matches!(check_undo(&folded, &proposed(tab(2), 0), &edit), Check::Conflict { .. }));
    assert_eq!(check_undo(&folded, &proposed(tab(2), 1), &edit), Check::Applied);
}

#[test]
fn rescan_never_stales_an_edit() {
    let mut log = Log::default();
    let mut over = rename("r:x:item", "Bag", 0);
    over.over = BTreeMap::from([("name".to_owned(), json!("item"))]);
    log.edit(&tab(1), Status::Applied, over);
    let base = log.seq();
    // A rescan renames the fact under the override and moves another node.
    log.push(
        &Writer {
            actor: Actor::System { part: "scan".into() },
            client: Client { surface: Surface::Cli, id: "cli:1".into() },
        },
        Status::Applied,
        EventBody::Scan {
            repo: "r".into(),
            commit: "c2".into(),
            branch: "main".into(),
            snapshot: 2,
            link: 2,
            renames: Vec::<Rename>::new(),
            stats: ScanStats::default(),
        },
    );
    let mut rescanned = facts();
    rescanned.nodes[3].name = "item-v2".into();
    rescanned.nodes[2].parent = None;
    let folded = log.fold(&rescanned);

    assert_eq!(name_of(&folded, "r:x:item"), "Bag", "the override holds over the new fact");
    assert_eq!(
        folded.revision(&Target::Node(id("r:x:item")), "name").unwrap().seq,
        1,
        "a scan writes no revision"
    );
    assert_eq!(folded.revisions(&Target::Node(id("r:x:b"))), vec![], "a fact change is no revision");
    // A write from the view before the scan is current on every field the scan changed.
    assert_eq!(check_edit(&folded, &proposed(press(), base), &rename("r:x:b", "Bin", base)), Check::Applied);
    assert_eq!(
        check_edit(&folded, &proposed(press(), base), &set("r:x:b", "parent", json!("r:x:repo"), base)),
        Check::Applied
    );
    assert_eq!(
        check_edit(&folded, &proposed(tab(2), base), &rename("r:x:item", "Basket", base)),
        Check::Applied
    );
}

#[test]
fn an_accept_after_an_intervening_write_is_a_card() {
    let mut log = Log::default();
    let ghost = log.edit(&agent(), Status::Ghost, rename("r:x:item", "Sack", 0));
    let EventBody::Edit(ghost_edit) = log.events[0].body.clone() else { unreachable!() };
    let view = log.seq();

    // With nothing in between, the accept's copy applies at the accepting view's base.
    let quiet = log.fold(&facts());
    let accept = copy_for_review(&ghost_edit, view);
    assert_eq!(accept.base, view);
    assert_eq!(
        (accept.fields.clone(), accept.target.clone()),
        (ghost_edit.fields.clone(), ghost_edit.target.clone())
    );
    assert_eq!(check_edit(&quiet, &proposed(tab(1), view), &accept), Check::Applied);

    // A mod press renames the node before the tab's accept lands.
    log.edit(&press(), Status::Applied, rename("r:x:item", "Bag 2", view));
    let busy = log.fold(&facts());
    let check = check_edit(&busy, &proposed(tab(1), view), &accept);
    let Check::Conflict { clashes, card } = &check else { panic!("{check:?}") };
    assert!(card, "an accept over a newer write is a three-way card, never a silent no-op");
    assert_eq!(clashes[0].theirs.as_ref().unwrap().seq, 2);
    assert!(busy.overlay.ghosts.iter().all(|g| g.event != ghost));
}

#[test]
fn a_resolution_after_an_intervening_write_is_a_card() {
    let mut log = Log::default();
    log.edit(&tab(1), Status::Applied, rename("r:x:item", "Bag", 0));
    let yours = rename("r:x:item", "Basket", 0);
    let conflict = log.edit(&tab(2), Status::Conflict, yours.clone());
    let card_view = log.seq();
    let folded = log.fold(&facts());
    assert_eq!(name_of(&folded, "r:x:item"), "Bag", "a stored conflict is never merged");
    assert_eq!(folded.conflicts.len(), 1);
    assert_eq!((&folded.conflicts[0].event, folded.conflicts[0].resolved), (&conflict, None));

    // Choosing "yours" from the card writes a copy at the card's view, which applies...
    let choice = copy_for_review(&yours, card_view);
    assert_eq!(check_edit(&folded, &proposed(tab(2), card_view), &choice), Check::Applied);

    // ...unless a write landed after the card, which makes it a new card.
    log.edit(&press(), Status::Applied, rename("r:x:item", "Bag 3", card_view));
    let folded = log.fold(&facts());
    let check = check_edit(&folded, &proposed(tab(2), card_view), &choice);
    assert!(
        matches!(&check, Check::Conflict { card: true, clashes } if clashes[0].theirs.as_ref().unwrap().seq == 3),
        "{check:?}"
    );
}

#[test]
fn a_parent_change_that_makes_a_cycle_is_refused() {
    let folded = Log::default().fold(&facts());
    let under_child = set("r:x:a", "parent", json!("r:x:item"), 0);
    assert_eq!(
        check_edit(&folded, &proposed(tab(1), 0), &under_child),
        Check::Cycle { cycle: vec![id("r:x:a"), id("r:x:item"), id("r:x:a")] }
    );
    assert_eq!(
        check_edit(&folded, &proposed(tab(1), 0), &set("r:x:a", "parent", json!("r:x:a"), 0)).status(),
        None
    );
    assert_eq!(
        check_edit(&folded, &proposed(tab(1), 0), &set("r:x:a", "parent", json!("r:x:b"), 0)),
        Check::Applied
    );

    // A regroup is one write: a group put under the item, then the item's own box moved into it.
    let group = Edit {
        op: Op::Put,
        target: Target::Node(id("r:group:g")),
        layer: Layer::Actual,
        fields: BTreeMap::from([
            ("kind".to_owned(), json!("k.g")),
            ("level".to_owned(), json!("component")),
            ("name".to_owned(), json!("g")),
            ("parent".to_owned(), json!("r:x:item")),
        ]),
        base: 0,
        over: BTreeMap::new(),
    };
    let check = check_edits(
        &folded,
        &proposed(tab(1), 0),
        &[group.clone(), set("r:x:a", "parent", json!("r:group:g"), 0)],
    );
    assert_eq!(
        check,
        Check::Cycle { cycle: vec![id("r:x:a"), id("r:group:g"), id("r:x:item"), id("r:x:a")] }
    );
    assert_eq!(
        check_edits(&folded, &proposed(tab(1), 0), &[group, set("r:x:b", "parent", json!("r:group:g"), 0)]),
        Check::Applied
    );
}

#[test]
fn an_automatic_write_over_a_person_field_is_superseded_whatever_its_base() {
    let mut log = Log::default();
    let edit = log.edit(&tab(1), Status::Applied, rename("r:x:item", "Bag", 0));
    let folded = log.fold(&facts());
    let model = Writer {
        actor: Actor::Model { id: "m".into() },
        client: Client { surface: Surface::Cli, id: "duty:decide".into() },
    };
    let automatic = Proposed { writer: model.clone(), authority: Authority::Automatic, base: 5 };
    assert_eq!(
        check_edit(&folded, &automatic, &rename("r:x:item", "Sack", 5)),
        Check::Superseded { held: vec![(Target::Node(id("r:x:item")), "name".into())] }
    );
    assert_eq!(check_edit(&folded, &automatic, &rename("r:x:b", "Bin", 5)), Check::Applied);

    // An undo of the person's edit releases the field.
    log.push(&tab(1), Status::Applied, EventBody::Undo { of: edit });
    let folded = log.fold(&facts());
    assert!(!folded.held_by_person(&Target::Node(id("r:x:item")), "name"));
    let automatic = Proposed { writer: model, authority: Authority::Automatic, base: 2 };
    assert_eq!(check_edit(&folded, &automatic, &rename("r:x:item", "Sack", 2)), Check::Applied);
}

#[test]
fn only_a_field_the_fold_set_has_a_revision_or_a_hold() {
    // An older store holds two moves that close a loop: the later one loses in the fold, so the
    // person's move of `a` sets nothing, and `a` keeps its fact parent.
    let mut log = Log::default();
    log.edit(&tab(1), Status::Applied, set("r:x:b", "parent", json!("r:x:a"), 0));
    log.edit(&tab(1), Status::Applied, set("r:x:a", "parent", json!("r:x:b"), 0));
    // A rename of a node that doesn't exist, and a name that isn't text, set nothing either.
    log.edit(&tab(1), Status::Applied, rename("r:x:gone", "Bag", 0));
    log.edit(&tab(1), Status::Applied, set("r:x:item", "name", json!(5), 0));
    let folded = log.fold(&facts());
    let a = Target::Node(id("r:x:a"));
    let parent_of_a = folded.model.nodes.iter().find(|n| n.id == id("r:x:a")).unwrap().parent.clone();
    assert_eq!(parent_of_a, Some(id("r:x:repo")), "the move that closed the loop lost");
    assert_eq!(folded.revision(&a, "parent"), None);
    assert!(!folded.held_by_person(&a, "parent"));
    assert!(!folded.held_by_person(&a, "*"));
    assert_eq!(folded.revision(&Target::Node(id("r:x:gone")), "name"), None);
    assert_eq!(folded.revision(&Target::Node(id("r:x:item")), "name"), None);
    assert!(!folded.held_by_person(&Target::Node(id("r:x:item")), "name"));
    // The move of `b` did apply, so it keeps its revision.
    assert_eq!(folded.revision(&Target::Node(id("r:x:b")), "parent").map(|r| r.seq), Some(1));

    // So an automatic write to a field nobody set applies, and no base is stale on one.
    let model = Writer {
        actor: Actor::Model { id: "m".into() },
        client: Client { surface: Surface::Cli, id: "duty:decide".into() },
    };
    let automatic = Proposed { writer: model, authority: Authority::Automatic, base: 0 };
    assert_eq!(
        check_edit(&folded, &automatic, &set("r:x:a", "parent", json!("r:x:repo"), 0)),
        Check::Applied
    );
    assert_eq!(check_edit(&folded, &proposed(tab(2), 0), &rename("r:x:item", "Sack", 0)), Check::Applied);
}

#[test]
fn a_hide_or_a_pin_of_a_node_that_does_not_exist_sets_nothing() {
    let pin = "pos@map:system:all";
    let mut log = Log::default();
    log.edit(&tab(1), Status::Applied, set("r:x:gone", "hidden", json!(true), 0));
    log.edit(&tab(1), Status::Applied, set("r:x:gone", pin, json!({"x": 1.0, "y": 2.0}), 0));
    let folded = log.fold(&facts());
    let gone = Target::Node(id("r:x:gone"));
    assert_eq!(folded.revision(&gone, "hidden"), None);
    assert_eq!(folded.revision(&gone, pin), None);
    assert!(folded.revisions(&gone).is_empty());
    assert!(!folded.held_by_person(&gone, "*"));
    assert!(folded.overlay.hidden.is_empty(), "the overlay hides nothing");
    assert!(folded.overlay.positions.is_empty(), "the overlay pins nothing");
    // So another tab's write from the older base meets no revision.
    assert_eq!(
        check_edit(&folded, &proposed(tab(2), 0), &set("r:x:gone", "hidden", json!(false), 0)),
        Check::Applied
    );
    assert_eq!(
        check_edit(&folded, &proposed(tab(2), 0), &set("r:x:gone", pin, json!(null), 0)),
        Check::Applied
    );

    // The same hide and pin of a node that exists set both fields.
    log.edit(&tab(1), Status::Applied, set("r:x:item", "hidden", json!(true), 0));
    log.edit(&tab(1), Status::Applied, set("r:x:item", pin, json!({"x": 1.0, "y": 2.0}), 0));
    let folded = log.fold(&facts());
    let item = Target::Node(id("r:x:item"));
    assert_eq!(folded.revision(&item, "hidden").map(|r| r.seq), Some(3));
    assert_eq!(folded.revision(&item, pin).map(|r| r.seq), Some(4));
    assert_eq!(folded.overlay.hidden.len(), 1);
    assert_eq!(folded.overlay.positions.len(), 1);
}

#[test]
fn an_undo_writes_a_revision_only_on_a_field_its_edit_set() {
    let mut log = Log::default();
    // An older store holds two moves that close a loop: the later one loses in the fold.
    log.edit(&tab(1), Status::Applied, set("r:x:b", "parent", json!("r:x:a"), 0));
    let lost_move = log.edit(&tab(1), Status::Applied, set("r:x:a", "parent", json!("r:x:b"), 0));
    // A rename of a node that doesn't exist, and a name that isn't text, set nothing either.
    let missing = log.edit(&tab(1), Status::Applied, rename("r:x:gone", "Bag", 0));
    let not_text = log.edit(&tab(1), Status::Applied, set("r:x:item", "name", json!(5), 0));
    // One edit that sets a name, beside a hide the field can't take.
    let mut mixed = rename("r:x:b", "Basket", 0);
    mixed.fields.insert("hidden".into(), json!("yes"));
    let mixed = log.edit(&tab(1), Status::Applied, mixed);
    for of in [lost_move, missing, not_text, mixed] {
        log.push(&tab(1), Status::Applied, EventBody::Undo { of });
    }
    let folded = log.fold(&facts());

    assert_eq!(folded.revision(&Target::Node(id("r:x:a")), "parent"), None);
    assert_eq!(folded.revision(&Target::Node(id("r:x:gone")), "name"), None);
    assert!(folded.revisions(&Target::Node(id("r:x:gone"))).is_empty());
    assert_eq!(folded.revision(&Target::Node(id("r:x:item")), "name"), None);
    assert_eq!(folded.revision(&Target::Node(id("r:x:b")), "hidden"), None);
    // The name the mixed edit did set is changed back, so it carries that undo's revision.
    assert_eq!(folded.revision(&Target::Node(id("r:x:b")), "name").map(|r| r.seq), Some(9));

    // So a write from an older base meets no revision on a field nothing set.
    assert_eq!(check_edit(&folded, &proposed(tab(2), 0), &rename("r:x:item", "Sack", 0)), Check::Applied);
    assert_eq!(
        check_edit(&folded, &proposed(tab(2), 0), &set("r:x:a", "parent", json!("r:x:repo"), 0)),
        Check::Applied
    );
    assert_eq!(
        check_edit(&folded, &proposed(tab(2), 0), &set("r:x:b", "hidden", json!(true), 0)),
        Check::Applied
    );
    let stale = check_edit(&folded, &proposed(tab(2), 5), &rename("r:x:b", "Bin", 5));
    assert!(matches!(stale, Check::Conflict { .. }), "{stale:?}");
}

#[test]
fn an_undo_that_would_close_a_parent_loop_is_refused() {
    // Tab 1 moves `a` under `b`, moves it back, then moves `b` under `a`. Undoing the move back
    // would put `a` under `b` again, which is under `a`: a loop.
    let mut log = Log::default();
    let first = log.edit(&tab(1), Status::Applied, set("r:x:a", "parent", json!("r:x:b"), 0));
    let back = log.edit(&tab(1), Status::Applied, set("r:x:a", "parent", json!("r:x:repo"), 1));
    log.edit(&tab(1), Status::Applied, set("r:x:b", "parent", json!("r:x:a"), 2));
    let folded = log.fold(&facts());
    assert_eq!(
        check_undo(&folded, &proposed(tab(1), 3), &back),
        Check::Cycle { cycle: vec![id("r:x:a"), id("r:x:b"), id("r:x:a")] },
        "refused as the edit would be, naming the loop"
    );
    // Undoing the first move leaves `a` where the move back put it, which closes nothing.
    assert_eq!(check_undo(&folded, &proposed(tab(1), 3), &first), Check::Applied);

    // An undo of an undo restores what the first undo took back, and is checked the same way.
    let mut log = Log::default();
    let moved = log.edit(&tab(1), Status::Applied, set("r:x:a", "parent", json!("r:x:b"), 0));
    let undo = log.push(&tab(1), Status::Applied, EventBody::Undo { of: moved });
    log.edit(&tab(1), Status::Applied, set("r:x:b", "parent", json!("r:x:a"), 2));
    let folded = log.fold(&facts());
    assert_eq!(
        check_undo(&folded, &proposed(tab(1), 3), &undo),
        Check::Cycle { cycle: vec![id("r:x:a"), id("r:x:b"), id("r:x:a")] }
    );

    // An undo of a retract brings the node back under the parent it had, and is checked too:
    // `a` goes under `b`, is retracted, and `b` goes under `item`, whose parent is `a`.
    let mut log = Log::default();
    log.edit(&tab(1), Status::Applied, set("r:x:a", "parent", json!("r:x:b"), 0));
    let retract = log.edit(
        &tab(1),
        Status::Applied,
        Edit {
            op: Op::Retract,
            target: Target::Node(id("r:x:a")),
            layer: Layer::Actual,
            fields: BTreeMap::new(),
            base: 1,
            over: BTreeMap::new(),
        },
    );
    log.edit(&tab(1), Status::Applied, set("r:x:b", "parent", json!("r:x:item"), 2));
    let folded = log.fold(&facts());
    assert!(folded.model.nodes.iter().all(|n| n.id != id("r:x:a")));
    assert_eq!(
        check_undo(&folded, &proposed(tab(1), 3), &retract),
        Check::Cycle { cycle: vec![id("r:x:a"), id("r:x:b"), id("r:x:item"), id("r:x:a")] }
    );
}
