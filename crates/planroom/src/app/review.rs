//! `planroom review ID accept|reject`: accept or reject one ghost or one stored conflict, or pick
//! a card a write dealt (architecture.md, Writes and Actor rules; DR-5, DR-7, DR-54, DR-89).
//!
//! - A person reviews any item. An agent may only reject its own ghosts, those written under its
//!   own session and role; every other call of an agent is `person_required`, decided before
//!   anything else about the item, with the exact command a person runs and the card to pick.
//! - An accept writes a new edit that copies the reviewed one, with `base` from the review's view,
//!   the reviewed event as its cause and the reviewing person as its actor; an inbox row's ghost
//!   on the Apply card applies instead as its viewer, `person:artifact:<uid>`, with the card as
//!   its cause (DR-67). Either way the review is the person's. It passes the kernel's
//!   compare-and-set like any write, so an accept over a newer write is stored as a conflict and
//!   returns `stale_write`, never a silent no-op. A stored conflict is accepted the same way.
//! - Reviewing the last open ghost of a card also writes the card's own review in the same
//!   transaction: accept when any of its ghosts was accepted, else reject. That review is keyed
//!   apart from the command ([`cards::card_review`]), so whichever later undo sets one of the
//!   card's ghosts waiting again can reverse it alone and reopen the card.
//! - A card is picked through the same `Review { of }` body; a second pick is `already_resolved`.
//!   The Apply card is picked through its ghosts, so `review` on it is `invalid_argument`, naming
//!   them, while any waits; an agent's call naming it is `person_required`, with the review of its
//!   first waiting ghost as the line a person runs.
//!
//! Every event of one review is planned inside the write's own IMMEDIATE transaction
//! ([`Store::commit_keyed`]), from the fold of the store as the commit finds it. One `edit undo`
//! of the review event reverses the review and an accept's copy, and the undo reopens the card the
//! review closed (`edit`'s undo plan).

use std::cell::RefCell;
use std::path::Path;

use planroom_contract::{
    Actor, Cause, Client, Command, Edit, ErrorCode, Event, EventBody, EventId, ReviewReq, ReviewResult, Seq,
    Status, Surface, Verdict,
};
use planroom_kernel::cas::{self, Authority, Check, Proposed};
use planroom_kernel::fold::{Folded, Writer, actor_key};
use planroom_kernel::keys;
use planroom_store::{NewEvent, Planned as Appended, Snapshot, Store};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::app::actor::{self, Bound, Caller, Refused, Terminal, TerminalPath};
use crate::app::cards::{self, APPLY, BATCH_ITEMS};
use crate::app::edit::{self, Written};
use crate::app::error::AppError;
use crate::app::{Reply, import, new_event_id, workspace};
use crate::{clock, live, text};

/// `planroom review` from the CLI, as the actor rules resolve its caller (`actor::writer`).
pub fn run(
    req: &ReviewReq,
    cwd: &Path,
    caller: &Caller,
    terminal: &mut dyn Terminal,
) -> Result<Written<ReviewResult>, AppError> {
    let root = workspace::find_root(cwd).ok_or_else(|| AppError::workspace_not_found(cwd))?;
    let mut store = Store::open_existing(&workspace::store_path(&root))?
        .ok_or_else(|| AppError::workspace_not_found(&root))?;
    crate::app::hook(&mut store);
    // An agent's call that isn't the reject of its own ghost is refused before anything else, a
    // missing base included, and checked again inside the write, which reads the store as its
    // commit finds it. Its hint reviews at the base the call gave, or else at the head.
    if let Some(agent) = caller.actor.as_deref().and_then(actor::agent) {
        let writer = Writer { actor: agent, client: caller.client() };
        let snapshot = store.snapshot()?;
        let events = snapshot.events_after(0)?;
        let folded = live::folded(&snapshot, crate::packs::vocabulary()?)?;
        let head = snapshot.seq();
        let base = req.common.base.unwrap_or(head).min(head);
        if let Some(refusal) = agent_refusal(req, &events, &folded, &writer, base) {
            return Err(refusal);
        }
    }
    let base =
        req.common.base.ok_or_else(|| AppError::new(ErrorCode::BaseRequired, &[("command", &"review")]))?;
    let writer = cli_writer(req, caller, &mut store, base, terminal)?;
    write(req, &mut store, writer, base)
}

fn is_agent(writer: &Writer) -> bool {
    matches!(writer.actor, Actor::Agent { .. })
}

/// `person_required` for an agent's call, unless it rejects a ghost written under its own session
/// and role; anything else it names is a person's to review, whatever else is true of it. The
/// refusal names the card a person picks when the item is one or sits on one, and its hint the
/// review at `base`, no later than the head, so the line runs as it is offered: for the Apply
/// card, which is picked through its ghosts, the review of its first waiting ghost.
fn agent_refusal(
    req: &ReviewReq,
    events: &[Event],
    folded: &Folded,
    writer: &Writer,
    base: Seq,
) -> Option<AppError> {
    let found = edit::find_event(events, &req.id).ok();
    let own = found.is_some_and(|e| {
        e.status == Status::Ghost
            && matches!(e.body, EventBody::Edit(_))
            && actor_key(&e.actor) == actor_key(&writer.actor)
    });
    if own && req.verdict == Verdict::Reject {
        return None;
    }
    let card = found.and_then(|e| card_of(e, events));
    let first = match found.map(|e| &e.body) {
        Some(EventBody::Card(named)) if named.kind.0 == APPLY => {
            card.as_ref().and_then(|c| cards::open_ghosts(folded, c).into_iter().next())
        }
        _ => None,
    };
    let id = match (&first, found) {
        (Some(ghost), _) => ghost.0.as_str(),
        (None, Some(e)) => e.id.0.as_str(),
        (None, None) => req.id.as_str(),
    };
    Some(refused(req, id, base, card.as_ref()))
}

/// The exact command line that runs this review of `id` at `base` from a terminal.
fn terminal_line(req: &ReviewReq, id: &str, base: Seq) -> String {
    let mut words = vec!["planroom".to_owned(), "review".to_owned(), "--base".to_owned(), base.to_string()];
    if let Some(project) = &req.common.project {
        words.extend(["--project".to_owned(), text::shell_word(&project.0)]);
    }
    if id.starts_with('-') {
        words.push("--".to_owned());
    }
    words.push(text::shell_word(id));
    words.push(verdict_word(req.verdict).to_owned());
    words.join(" ")
}

fn verdict_word(verdict: Verdict) -> &'static str {
    match verdict {
        Verdict::Accept => "accept",
        Verdict::Reject => "reject",
    }
}

/// `person_required` for an agent's review of `id`: its hint is the exact review a person runs,
/// and its details name the card a person can pick, when the item is one or sits on one.
fn refused(req: &ReviewReq, id: &str, base: Seq, card: Option<&EventId>) -> AppError {
    let line = terminal_line(req, id, base);
    actor::person_required_with("review", Refused::NotAPerson, TerminalPath::Takes, &line, card)
}

/// The CLI's writer instance: the agent `PLANROOM_ACTOR` names, a person's token, or a person who
/// confirms on the terminal (DR-4, DR-64).
fn cli_writer(
    req: &ReviewReq,
    caller: &Caller,
    store: &mut Store,
    base: Seq,
    terminal: &mut dyn Terminal,
) -> Result<Writer, AppError> {
    let spec = Command::Review(req.clone()).spec();
    let registered = store.meta("person_pubkey")?;
    let keys: Vec<_> = registered.as_deref().and_then(actor::parse_key).into_iter().collect();
    let now = clock::unix_micros() / 1_000_000;
    let bound = Bound { idem: req.common.idem.as_deref(), base: Some(base) };
    let (name, path) = (spec.name(), actor::terminal_path(&spec.name(), !keys.is_empty()));
    let line = terminal_line(req, &req.id, base);
    let acting = actor::writer(caller, spec, &bound, &keys.iter().collect::<Vec<_>>(), now, terminal)
        .map_err(|e| actor::naming_line(e, &name, path, &line))?;
    if let Some(digest) = &acting.digest {
        actor::spend(store, digest, now, &name, path)
            .map_err(|e| actor::naming_line(e, &name, path, &line))?;
    }
    let default = match (&acting.actor, caller.actor.as_deref()) {
        (Actor::Agent { .. }, Some(named)) => named.to_owned(),
        _ => format!("cli:{}", std::process::id()),
    };
    let id = req.common.client.clone().or_else(|| caller.client.clone()).unwrap_or(default);
    let surface = if id.starts_with("mod:") { Surface::Mod } else { Surface::Cli };
    Ok(Writer { actor: acting.actor, client: Client { surface, id } })
}

/// The reply a commit stores on its first event, so a retry with the same key returns the same.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Stored {
    Done { result: ReviewResult, text: String },
    Conflict { name: String, seq: Seq, details: Value },
}

/// What the plan decided, inside the write's transaction.
struct Planned {
    events: Vec<NewEvent>,
    /// The card this review closes, with its verdict: the review of its last waiting ghost writes
    /// the card's own review too, keyed apart from this command ([`cards::card_review`]).
    closes: Option<(EventId, Verdict)>,
    result: ReviewResult,
    line: String,
    /// The accept's copy was stored as a conflict: the clashes, and the copy as written.
    conflict: Option<(Check, Edit, String)>,
}

fn write(
    req: &ReviewReq,
    store: &mut Store,
    writer: Writer,
    base: Seq,
) -> Result<Written<ReviewResult>, AppError> {
    let vocabulary = crate::packs::vocabulary()?;
    let args = json!({"id": req.id, "verdict": req.verdict});
    let project = req.common.project.clone();
    let who = actor_key(&writer.actor);
    let request =
        json!({"args": args, "base": base, "project": project, "actor": who, "client": writer.client});
    let request_hash = keys::request_hash("review", &request);
    let spec = Command::Review(req.clone()).spec();
    let keyed = json!({"args": args, "project": project});
    let (idem, derived) = edit::write_key(spec, req.common.idem.as_deref(), &who, &keyed, Some(base))?;
    let at = clock::now();
    let planned: RefCell<Option<Planned>> = RefCell::new(None);
    let committed = store.commit_keyed(
        &idem,
        &request_hash,
        |snapshot| {
            let folded = live::folded(snapshot, vocabulary)?;
            let mut plan = plan(req, snapshot, &folded, &writer, base)?;
            for event in &mut plan.events {
                event.at = at.clone();
                event.project = project.clone();
            }
            let mut events: Vec<Appended> = plan.events.iter().cloned().map(Appended::Command).collect();
            if let Some((card, verdict)) = &plan.closes {
                events.push(Appended::Own(cards::card_review(
                    card,
                    *verdict,
                    &writer,
                    &idem,
                    &request_hash,
                    &at,
                )));
            }
            *planned.borrow_mut() = Some(plan);
            Ok::<_, AppError>(events)
        },
        |_| {
            let planned = planned.borrow();
            let plan = planned.as_ref().expect("the plan ran before the reply");
            serde_json::to_value(stored(plan, base)).expect("a stored reply serializes")
        },
    )?;
    let first = *committed.seqs.first().ok_or_else(|| AppError::internal("a commit wrote no event"))?;
    let last = committed.seqs.iter().chain(&committed.own).copied().max().expect("one event at least");
    match serde_json::from_value(committed.reply).map_err(AppError::internal)? {
        Stored::Conflict { name, seq, details } => {
            let event = details["event"].as_str().unwrap_or_default().to_owned();
            Err(AppError::new(ErrorCode::StaleWrite, &[("name", &name), ("seq", &seq)])
                .details(details)
                .with_hint(text::get("hint.stale_write", &[("event", &event)])))
        }
        Stored::Done { result, mut text } => {
            let replayed_from = (committed.replayed && derived).then_some(first);
            if replayed_from.is_some() {
                text.push('\n');
                text.push_str(&text::get("review.replayed", &[]));
            }
            Ok(Written {
                reply: Reply { result, seq: last, warnings: vec![], text },
                replayed: committed.replayed,
                replayed_from,
            })
        }
    }
}

/// The reply a commit stores for its plan: the result, or the conflict the accept was stored as.
fn stored(plan: &Planned, base: Seq) -> Stored {
    if let Some((Check::Conflict { clashes, .. }, copy, name)) = &plan.conflict {
        let theirs = edit::within(
            clashes.iter().map(|c| {
                json!({
                    "target": edit::target_id(&c.target),
                    "field": c.field,
                    "seq": c.theirs.as_ref().map(|r| r.seq),
                    "event": c.theirs.as_ref().map(|r| r.event.0.clone()),
                })
            }),
            edit::THEIRS_MAX_BYTES,
        );
        let seq = clashes.first().and_then(|c| c.theirs.as_ref()).map_or(base, |r| r.seq);
        let event = plan.result.edit.clone().map(|e| e.0).unwrap_or_default();
        let details = json!({
            "event": event,
            "base": base,
            "theirs": theirs,
            "yours": edit::yours(&[(None, EventBody::Edit(copy.clone()))]),
            "card": null,
        });
        return Stored::Conflict { name: name.clone(), seq, details };
    }
    Stored::Done { result: plan.result.clone(), text: plan.line.clone() }
}

/// A new event of this review, written by `writer`; the commit sets its time and project.
fn event(
    writer: &Writer,
    status: Status,
    target: &EventId,
    cause: Option<Cause>,
    body: EventBody,
) -> NewEvent {
    NewEvent {
        id: new_event_id(),
        at: planroom_contract::Timestamp(String::new()),
        actor: writer.actor.clone(),
        client: writer.client.clone(),
        project: None,
        cause,
        status,
        target: Some(target.0.clone()),
        body,
    }
}

/// Plan the review over the fold of the store as this write's commit finds it.
fn plan(
    req: &ReviewReq,
    snapshot: &Snapshot<'_>,
    folded: &Folded,
    writer: &Writer,
    base: Seq,
) -> Result<Planned, AppError> {
    let events = snapshot.events_after(0)?;
    if is_agent(writer)
        && let Some(refusal) = agent_refusal(req, &events, folded, writer, base.min(snapshot.seq()))
    {
        return Err(refusal);
    }
    let found = edit::find_event(&events, &req.id);
    // A base past the head names a version this writer can't have read (DR-73).
    if base > snapshot.seq() {
        return Err(AppError::invalid_argument("base", "edit.base_ahead"));
    }
    let of = found?;
    match &of.body {
        EventBody::Card(card) => pick_card(req, of, card, &events, folded, snapshot.seq(), writer),
        EventBody::Edit(reviewed) if matches!(of.status, Status::Ghost | Status::Conflict) => {
            review_edit(req, of, reviewed, &events, folded, writer, base)
        }
        _ => {
            let what = text::get("review.not_reviewable", &[("event", &of.id.0)]);
            Err(AppError::new(ErrorCode::InvalidArgument, &[("what", &what)]).details(json!({"field": "id"})))
        }
    }
}

/// The card a person picks for `event`: itself when it is a card, else the card it sits on.
fn card_of(event: &Event, events: &[Event]) -> Option<EventId> {
    if let EventBody::Card(_) = event.body {
        return Some(event.id.clone());
    }
    let Some(Cause::Event(cause)) = &event.cause else { return None };
    events.iter().find(|e| &e.id == cause && matches!(e.body, EventBody::Card(_))).map(|e| e.id.clone())
}

/// The Apply card `event` waits on as a ghost, when it is one of an inbox row's.
fn apply_card_of(event: &Event, events: &[Event]) -> Option<EventId> {
    let card = card_of(event, events).filter(|c| c != &event.id)?;
    events
        .iter()
        .any(|e| e.id == card && matches!(&e.body, EventBody::Card(named) if named.kind.0 == APPLY))
        .then_some(card)
}

/// Pick a card: one `review` naming it.
fn pick_card(
    req: &ReviewReq,
    of: &Event,
    card: &planroom_contract::Card,
    events: &[Event],
    folded: &Folded,
    head: Seq,
    writer: &Writer,
) -> Result<Planned, AppError> {
    if !folded.open_cards.contains(&of.id) {
        return Err(closed(of, events, folded));
    }
    if card.kind.0 == APPLY {
        let ghosts = cards::open_ghosts(folded, &of.id);
        if let Some(first) = ghosts.first() {
            let shown: Vec<&str> = ghosts.iter().take(BATCH_ITEMS).map(|g| g.0.as_str()).collect();
            let what = text::get(
                "review.apply_card",
                &[("card", &of.id.0), ("count", &ghosts.len()), ("ghosts", &shown.join(", "))],
            );
            return Err(AppError::new(ErrorCode::InvalidArgument, &[("what", &what)])
                .details(json!({"field": "id", "ghosts": shown, "count": ghosts.len()}))
                .with_hint(cards::accept_line(first, head)));
        }
    }
    let review = event(
        writer,
        Status::Applied,
        &of.id,
        None,
        EventBody::Review { of: of.id.clone(), verdict: req.verdict },
    );
    let result = ReviewResult {
        event: review.id.clone(),
        of: of.id.clone(),
        verdict: req.verdict,
        edit: None,
        card_resolved: Some(of.id.clone()),
    };
    let line = text::get("review.picked", &[("verdict", &verdict_word(req.verdict)), ("card", &of.id.0)]);
    Ok(Planned { events: vec![review], closes: None, result, line, conflict: None })
}

/// Why a card that isn't open can't be picked: picked before (`already_resolved`, naming the pick
/// that stands), or withdrawn by an undo.
fn closed(of: &Event, events: &[Event], folded: &Folded) -> AppError {
    let standing = events.iter().rev().find(|e| {
        matches!(&e.body, EventBody::Review { of: named, .. } if named == &of.id)
            && e.status != Status::Conflict
            && !folded.is_undone(&e.id)
    });
    match (folded.verdict(&of.id), standing) {
        (Some(verdict), Some(review)) => {
            let by = match &review.actor {
                Actor::Person { id, .. } => id.clone(),
                other => actor_key(other).replace('\0', ":"),
            };
            let pick = verdict_word(verdict);
            AppError::new(ErrorCode::AlreadyResolved, &[("pick", &pick), ("by", &by), ("at", &review.at.0)])
                .details(json!({"by": by, "at": review.at.0, "pick": pick}))
        }
        _ => undone(of),
    }
}

fn undone(of: &Event) -> AppError {
    let what = text::get("review.undone", &[("event", &of.id.0)]);
    AppError::new(ErrorCode::InvalidArgument, &[("what", &what)]).details(json!({"field": "id"}))
}

/// Review one ghost or stored conflict: a `review`, an accept's copy, and the card's own review
/// when this was its last open ghost.
fn review_edit(
    req: &ReviewReq,
    of: &Event,
    reviewed: &Edit,
    events: &[Event],
    folded: &Folded,
    writer: &Writer,
    base: Seq,
) -> Result<Planned, AppError> {
    if folded.is_undone(&of.id) {
        return Err(undone(of));
    }
    if let Some(verdict) = folded.verdict(&of.id) {
        let what =
            text::get("review.already_reviewed", &[("event", &of.id.0), ("pick", &verdict_word(verdict))]);
        return Err(
            AppError::new(ErrorCode::InvalidArgument, &[("what", &what)]).details(json!({"field": "id"}))
        );
    }
    let review = event(
        writer,
        Status::Applied,
        &of.id,
        None,
        EventBody::Review { of: of.id.clone(), verdict: req.verdict },
    );
    let mut result = ReviewResult {
        event: review.id.clone(),
        of: of.id.clone(),
        verdict: req.verdict,
        edit: None,
        card_resolved: None,
    };
    let mut planned = Vec::from([review]);
    let mut conflict = None;
    let mut line = text::get("review.rejected", &[("event", &of.id.0)]);
    if req.verdict == Verdict::Accept {
        // An inbox row the person confirms applies as its viewer (DR-67): with the card as its
        // cause when it waits on the Apply card, or with itself when it went stale at import and
        // was held as a conflict on no card. Any other accept is the reviewing person's, caused
        // by the item it copies (DR-5). The person's authority is the review's either way.
        let (author, cause) = match apply_card_of(of, events) {
            Some(card) => (Writer::of(of), card),
            None if import::is_viewer_row(of) => (Writer::of(of), of.id.clone()),
            None => (writer.clone(), of.id.clone()),
        };
        let copy = cas::copy_for_review(reviewed, base);
        let proposed = Proposed { writer: author.clone(), authority: Authority::Person, base };
        let check = cas::check_edit(folded, &proposed, &copy);
        let status = match &check {
            Check::Cycle { cycle } => return Err(edit::cycle_refused(cycle)),
            check => {
                check.status().ok_or_else(|| AppError::internal("a person's accept is never automatic"))?
            }
        };
        let target = edit::target_id(&copy.target);
        let mut written =
            event(&author, status, &of.id, Some(Cause::Event(cause)), EventBody::Edit(copy.clone()));
        written.target = Some(target.clone());
        result.edit = Some(written.id.clone());
        line = text::get("review.accepted", &[("event", &of.id.0), ("edit", &written.id.0)]);
        if status == Status::Conflict {
            let name = folded
                .model
                .nodes
                .iter()
                .find(|n| n.id.0 == target)
                .map_or(target.clone(), |n| n.name.clone());
            conflict = Some((check, copy, edit::q(&name)));
        }
        planned.push(written);
    }
    // The card this ghost sits on resolves with its last open ghost's review, in this write, under
    // a key of its own, so a later undo that sets one of its ghosts waiting again reopens it.
    let mut closes = None;
    if let Some(card) = card_of(of, events).filter(|c| folded.open_cards.contains(c)) {
        let waiting = cards::open_ghosts(folded, &card);
        if waiting.iter().all(|g| g == &of.id) {
            let accepted = req.verdict == Verdict::Accept
                || folded.ghosts_of(&card).iter().any(|g| folded.verdict(g) == Some(Verdict::Accept));
            let verdict = if accepted { Verdict::Accept } else { Verdict::Reject };
            line.push(' ');
            line.push_str(&text::get(
                "review.card_resolved",
                &[("card", &card.0), ("verdict", &verdict_word(verdict))],
            ));
            result.card_resolved = Some(card.clone());
            closes = Some((card, verdict));
        }
    }
    Ok(Planned { events: planned, closes, result, line, conflict })
}
