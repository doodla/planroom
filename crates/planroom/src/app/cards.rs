//! Cards a write deals before `decide` exists (DR-89): who deals which, how `focus` lists the open
//! ones, and the Apply card `import` deals for unsigned inbox edits.
//!
//! - A card is a `card` event. Only a write deals one, in its own transaction, never a read.
//! - A dealer never deals a second card of one kind and subject while one is open: the open cards
//!   are the `card` events no standing review names and no undo reverses, which the store reads
//!   and the kernel decides ([`open`]).
//! - `scan` asks each of [`SCAN_DEALERS`] for its card inside the scan's commit: the trust card
//!   for a state directory the engine can't vouch for, whose subject is the workspace ID, so a
//!   moved workspace keeps one card. A person's pick of it records or removes the workspace's
//!   trust once `review` commits ([`picked`]), and an undo of an accept removes it again
//!   ([`after_trust_undo`]).
//! - `focus` puts the first open card face up in `PaneModel.decision` and counts the rest.
//! - The Apply card's subject is the workspace ID. Its ghosts name it as their cause, its count is
//!   derived live from them, and it resolves with its last ghost's review.
//! - A card that holds ghosts is open exactly while one of them waits. The card and the review
//!   that closes it are each keyed apart from the write that adds them, so an `edit undo` closes
//!   or reopens it in its own write ([`after_undo`]); a card itself is never undone. An undo that
//!   would reopen a card beside an open one of its kind and subject is refused until that one
//!   resolves ([`undo_beside`]), so a dealer's later rows always join its one open card.

use std::collections::BTreeSet;
use std::path::Path;

use planroom_contract::{
    Actor, BatchInfo, Card, CardKind, Choice, Client, CommandName, DecisionCard, EditResult, ErrorCode,
    Event, EventBody, EventId, Handle, ReviewResult, Seq, Stakes, Status, Surface, Timestamp, Verdict,
    Warning, WarningCode,
};
use planroom_kernel::fold::{Folded, Writer};
use planroom_kernel::keys;
use planroom_store::{Dealt, NewEvent, Snapshot, Store};
use serde_json::json;

use crate::app::data::TrustedBy;
use crate::app::edit::Written;
use crate::app::error::AppError;
use crate::app::new_event_id;
use crate::app::trust::{self, Trust};
use crate::text;

/// The Apply card's kind: unsigned inbox edits, waiting as ghosts (DR-83).
pub const APPLY: &str = "apply";
/// Items a card's batch lists at most; its count says how many there are.
pub const BATCH_ITEMS: usize = 5;
/// The most characters of one of a card's values its lines quote.
const VAR_CHARS: usize = 80;

/// The trust card's kind: a state directory the engine can't vouch for (DR-48, DR-89).
pub const TRUST: &str = "trust";

/// What a scan's dealers know besides the store: the workspace the scan ran in, and how its trust
/// stood as the scan committed.
#[derive(Debug, Clone)]
pub struct ScanCx<'a> {
    pub root: &'a Path,
    pub state_dir: &'a Path,
    pub trust: Trust,
}

/// One dealer of a card a scan deals: what it would deal, read from the store as the scan's
/// commit finds it and from the scan's own context, or none. [`deal_for_scan`] drops a card one
/// already open makes a second.
pub struct Dealer {
    pub kind: &'static str,
    pub deal: fn(&Snapshot<'_>, &ScanCx<'_>) -> Result<Option<Card>, AppError>,
}

/// The dealers `scan` asks, in order. The changed-source and restore cards join here.
pub const SCAN_DEALERS: &[Dealer] = &[Dealer { kind: TRUST, deal: deal_trust }];

/// The trust card, for a workspace whose state directory is untrusted, unless a pick of an earlier
/// one still stands as a reject: the reject keeps the config unloaded until an undo reopens it.
/// After a standing accept, a workspace untrusted again, as when it moved or its state directory
/// was replaced, gets a fresh card.
fn deal_trust(snapshot: &Snapshot<'_>, cx: &ScanCx<'_>) -> Result<Option<Card>, AppError> {
    if cx.trust != Trust::Untrusted {
        return Ok(None);
    }
    let workspace = snapshot.meta("workspace_id")?.unwrap_or_default();
    if standing_pick(snapshot, TRUST, &workspace)? == Some(Verdict::Reject) {
        return Ok(None);
    }
    Ok(Some(Card {
        kind: CardKind(TRUST.into()),
        subject: workspace,
        question: "card.trust_question".into(),
        why: "card.trust_why".into(),
        vars: [("dir".to_owned(), trust::dir_name(cx.root).to_owned())].into(),
        stakes: Stakes::Person,
        choices: vec![
            Choice { id: "accept".into(), label: "card.trust_accept".into(), preselected: false },
            Choice { id: "reject".into(), label: "card.trust_reject".into(), preselected: true },
        ],
        default: "reject".into(),
        dealt_by: CommandName("scan".into()),
    }))
}

/// The pick that stands on the closed cards of `kind` about `subject`: the last review naming one
/// of them. A second pick of a closed card is `already_resolved`, so a later review exists only
/// after the earlier one was undone, and a card an undo reopened is open, not closed. `None` when
/// no card of that kind and subject is closed.
pub fn standing_pick(
    snapshot: &Snapshot<'_>,
    kind: &str,
    subject: &str,
) -> Result<Option<Verdict>, AppError> {
    let events = snapshot.card_events()?;
    let open: BTreeSet<EventId> = planroom_kernel::fold::open_cards(&events).into_iter().collect();
    let closed: BTreeSet<&EventId> = events
        .iter()
        .filter(|e| !open.contains(&e.id))
        .filter(|e| matches!(&e.body, EventBody::Card(c) if c.kind.0 == kind && c.subject == subject))
        .map(|e| &e.id)
        .collect();
    let mut last = None;
    for event in &events {
        if let EventBody::Review { of, verdict } = &event.body
            && closed.contains(of)
            && event.status != Status::Conflict
        {
            last = Some(*verdict);
        }
    }
    Ok(last)
}

/// The open trust card of the store's workspace, if one is.
pub fn open_trust(snapshot: &Snapshot<'_>) -> Result<Option<OpenCard>, AppError> {
    let workspace = snapshot.meta("workspace_id")?.unwrap_or_default();
    open_of(snapshot, TRUST, &workspace)
}

/// `workspace_untrusted`, naming the open trust card and the state directory it asks about.
pub fn untrusted_warning(id: &EventId, card: &Card) -> Warning {
    let dir = card.vars.get("dir").cloned().unwrap_or_else(|| ".planroom/".into());
    Warning {
        code: WarningCode::WorkspaceUntrusted,
        message: text::get("warning.workspace_untrusted", &[("dir", &dir), ("card", &id.0)]),
        details: json!({"card": id.0}),
    }
}

/// What a person's pick of a trust card does once `review` committed it (DR-89), acting on the
/// workspace at `root`, where the review ran: an accept records its state directory as trusted by
/// a person, with its current identity, and a reject removes any record. A replayed pick acts only
/// while that review still stands, so a retry repairs a record write that failed, and a pick an
/// undo reversed never trusts the workspace again. The reply gains its line, or, when the record
/// can't be written, `trust_not_recorded`: the pick stands, and the next scan deals a fresh card.
pub fn picked(store: &Store, written: &mut Written<ReviewResult>, root: &Path) -> Result<(), AppError> {
    let result = &written.reply.result;
    let Some(card) = result.card_resolved.clone() else { return Ok(()) };
    if result.of != card {
        return Ok(());
    }
    let snapshot = store.snapshot()?;
    let events = snapshot.card_events()?;
    let is_trust =
        events.iter().any(|e| e.id == card && matches!(&e.body, EventBody::Card(c) if c.kind.0 == TRUST));
    if !is_trust {
        return Ok(());
    }
    let open: BTreeSet<EventId> = planroom_kernel::fold::open_cards(&events).into_iter().collect();
    let last = events.iter().rev().find(|e| {
        matches!(&e.body, EventBody::Review { of, .. } if *of == card) && e.status != Status::Conflict
    });
    let stands = !open.contains(&card) && last.is_some_and(|e| e.id == result.event);
    drop(snapshot);
    if !stands {
        return Ok(());
    }
    let verdict = result.verdict;
    let done = match verdict {
        Verdict::Accept => trust::record(root, TrustedBy::Person),
        Verdict::Reject => trust::revoke(root),
    };
    let reply = &mut written.reply;
    match done {
        Ok(()) => {
            let key = if verdict == Verdict::Accept { "review.trusted" } else { "review.kept" };
            reply.text.push(' ');
            reply.text.push_str(&text::get(key, &[]));
        }
        Err(_) => reply.warnings.push(Warning {
            code: WarningCode::TrustNotRecorded,
            message: text::get("warning.trust_not_recorded", &[("card", &card.0)]),
            details: json!({"card": card.0}),
        }),
    }
    Ok(())
}

/// After an `edit undo` commits: when it reversed a person's accept of a trust card, which is open
/// again, the workspace's trust record goes with it. An undo of that undo records nothing, so the
/// next scan deals a fresh card, and an agent's undo, a ghost that applies nothing, leaves the card
/// closed and changes nothing. An undo never trusts a workspace.
pub fn after_trust_undo(store: &Store, result: &EditResult, root: &Path) -> Result<(), AppError> {
    if result.status != Status::Applied {
        return Ok(());
    }
    let snapshot = store.snapshot()?;
    let events = snapshot.card_events()?;
    // The command's first event: for an undo, the `undo` event itself.
    let Some(written) = events.iter().find(|e| e.id == result.event) else { return Ok(()) };
    if !matches!(written.body, EventBody::Undo { .. }) {
        return Ok(());
    }
    // Follow the undos back to what the first one reversed.
    let mut at = written;
    for _ in 0..=events.len() {
        let EventBody::Undo { of } = &at.body else { break };
        let Some(next) = events.iter().find(|e| &e.id == of) else { return Ok(()) };
        at = next;
    }
    let EventBody::Review { of: card, verdict: Verdict::Accept } = &at.body else { return Ok(()) };
    let is_trust =
        events.iter().any(|e| &e.id == card && matches!(&e.body, EventBody::Card(c) if c.kind.0 == TRUST));
    let open = planroom_kernel::fold::open_cards(&events);
    drop(snapshot);
    if is_trust && open.contains(card) {
        trust::revoke(root)?;
    }
    Ok(())
}

/// An open card: its event and its body.
#[derive(Debug, Clone)]
pub struct OpenCard {
    pub event: Event,
    pub card: Card,
}

/// The open cards, in `seq` order: the store reads the events that bear on cards, and the kernel
/// decides which are open, as the fold does.
pub fn open(snapshot: &Snapshot<'_>) -> Result<Vec<OpenCard>, AppError> {
    let events = snapshot.card_events()?;
    let open: BTreeSet<EventId> = planroom_kernel::fold::open_cards(&events).into_iter().collect();
    Ok(events
        .into_iter()
        .filter(|e| open.contains(&e.id))
        .filter_map(|event| match &event.body {
            EventBody::Card(card) => Some(OpenCard { card: card.clone(), event }),
            _ => None,
        })
        .collect())
}

/// The open card of `kind` about `subject`, if one is.
pub fn open_of(snapshot: &Snapshot<'_>, kind: &str, subject: &str) -> Result<Option<OpenCard>, AppError> {
    Ok(open(snapshot)?.into_iter().find(|c| c.card.kind.0 == kind && c.card.subject == subject))
}

/// The cards `scan`'s commit deals: each dealer's, unless one of its kind and subject is open.
pub fn deal_for_scan(
    snapshot: &Snapshot<'_>,
    cx: &ScanCx<'_>,
    at: &Timestamp,
) -> Result<Vec<Dealt>, AppError> {
    let mut dealt = Vec::new();
    for dealer in SCAN_DEALERS {
        let Some(card) = (dealer.deal)(snapshot, cx)? else { continue };
        debug_assert_eq!(card.kind.0, dealer.kind);
        if open_of(snapshot, &card.kind.0, &card.subject)?.is_some() {
            continue;
        }
        dealt.push(dealt_card(card, snapshot.seq(), at));
    }
    Ok(dealt)
}

/// A card as a write deals it: the system part that dealt it is its actor, and it is keyed by its
/// kind, its subject and the head it was dealt at, apart from the write that deals it. So a scan
/// that retries under the write lock joins the first one, and an undo of the write that dealt it
/// leaves it standing for the rest of what waits on it.
fn dealt_card(card: Card, head: Seq, at: &Timestamp) -> Dealt {
    let request_hash = keys::request_hash("card", &json!(card));
    let idem = format!("card:{}:{}", card.kind.0, keys::sha256_hex(&[&card.subject, &head.to_string()]));
    let part = card.dealt_by.0.clone();
    let event = card_event(card, at, Actor::System { part });
    Dealt { idem, request_hash, event }
}

/// A card's own review that a write adds on its own, such as the one its last waiting ghost's
/// review writes, keyed apart from that write: `IDEM#card:CARD`, which no caller's key can be,
/// since a caller's key holds no `#`. So a later undo that sets one of its ghosts waiting again
/// can reverse this review alone, and the card reopens.
pub fn card_review(
    card: &EventId,
    verdict: Verdict,
    writer: &Writer,
    idem: &str,
    request_hash: &str,
    at: &Timestamp,
) -> Dealt {
    let event = NewEvent {
        id: new_event_id(),
        at: at.clone(),
        actor: writer.actor.clone(),
        client: writer.client.clone(),
        project: None,
        cause: None,
        status: Status::Applied,
        target: Some(card.0.clone()),
        body: EventBody::Review { of: card.clone(), verdict },
    };
    Dealt { idem: format!("{idem}#card:{}", card.0), request_hash: request_hash.to_owned(), event }
}

fn card_event(card: Card, at: &Timestamp, actor: Actor) -> NewEvent {
    let part = card.dealt_by.0.clone();
    NewEvent {
        id: new_event_id(),
        at: at.clone(),
        actor,
        client: Client { surface: Surface::Cli, id: format!("{part}:{}", std::process::id()) },
        project: None,
        cause: None,
        status: Status::Open,
        target: Some(card.subject.clone()),
        body: EventBody::Card(card),
    }
}

/// The Apply card import's rows join: the open one, or a new one, the first event of the write
/// that deals it, so its ghosts can name it as their cause, under a key of its own, so an undo of
/// that row's ghosts leaves it for the rows that joined it.
pub fn apply_card(snapshot: &Snapshot<'_>, at: &Timestamp) -> Result<(EventId, Option<Dealt>), AppError> {
    let workspace = snapshot.meta("workspace_id")?.unwrap_or_default();
    if let Some(open) = open_of(snapshot, APPLY, &workspace)? {
        return Ok((open.event.id, None));
    }
    let card = Card {
        kind: CardKind(APPLY.into()),
        subject: workspace,
        question: "card.apply_question".into(),
        why: "card.apply_why".into(),
        vars: Default::default(),
        stakes: Stakes::Person,
        choices: vec![
            Choice { id: "accept".into(), label: "card.apply_accept".into(), preselected: true },
            Choice { id: "reject".into(), label: "card.apply_reject".into(), preselected: false },
        ],
        default: "accept".into(),
        dealt_by: CommandName("import".into()),
    };
    let dealt = dealt_card(card, snapshot.seq(), at);
    Ok((dealt.event.id.clone(), Some(dealt)))
}

/// The ghosts on `card` that wait for a review, in `seq` order.
pub fn open_ghosts(folded: &Folded, card: &EventId) -> Vec<EventId> {
    folded.ghosts_of(card).into_iter().filter(|g| folded.verdict(g).is_none()).collect()
}

/// The handle that accepts one ghost from the view at `head`.
pub fn accept_handle(ghost: &EventId, head: Seq) -> Handle {
    Handle {
        command: CommandName("review".into()),
        args: vec!["--base".into(), head.to_string(), ghost.0.clone(), "accept".into()],
        label: text::label("card.apply_item", &[]),
    }
}

/// The exact command that accepts one ghost from the view at `head`.
pub fn accept_line(ghost: &EventId, head: Seq) -> String {
    format!("planroom review --base {head} {} accept", text::shell_word(&ghost.0))
}

/// What `focus` shows of the open cards: the first, face up, and how many more there are.
pub fn face_up(snapshot: &Snapshot<'_>, folded: &Folded) -> Result<(Option<DecisionCard>, u32), AppError> {
    let open = open(snapshot)?;
    let more = open.len().saturating_sub(1) as u32;
    Ok((open.first().map(|c| decision(c, folded, snapshot.seq())), more))
}

/// One open card as a person reads it: its lines filled from its `vars` and, for the Apply card,
/// the count of its waiting ghosts, which is derived live, with the first few as its batch.
pub fn decision(open: &OpenCard, folded: &Folded, head: Seq) -> DecisionCard {
    let card = &open.card;
    let ghosts = (card.kind.0 == APPLY).then(|| open_ghosts(folded, &open.event.id));
    let count = ghosts.as_ref().map_or(0, Vec::len);
    // A value is repo text, such as a repo's or a pack's name: quoted short, so the pane keeps to
    // its budget whatever it names.
    let quoted: Vec<(&str, String)> =
        card.vars.iter().map(|(k, v)| (k.as_str(), text::short(v, VAR_CHARS))).collect();
    let mut vars: Vec<(&str, &dyn std::fmt::Display)> =
        quoted.iter().map(|(k, v)| (*k, v as &dyn std::fmt::Display)).collect();
    vars.push(("count", &count));
    let line = |key: &str| match text::try_get(key) {
        Some(_) => text::get(key, &vars),
        // A card of a kind a later engine dealt, whose line this one doesn't have.
        None => text::get("card.unknown", &[("kind", &card.kind.0)]),
    };
    let choices = card
        .choices
        .iter()
        .map(|c| Choice {
            id: c.id.clone(),
            label: if text::LABELS.contains(&c.label.as_str()) {
                text::label(&c.label, &[])
            } else {
                c.id.clone()
            },
            preselected: c.preselected,
        })
        .collect();
    let batch = ghosts.map(|ghosts| BatchInfo {
        count: count as u32,
        items: ghosts.iter().take(BATCH_ITEMS).map(|g| accept_handle(g, head)).collect(),
    });
    DecisionCard {
        id: open.event.id.0.clone(),
        question: line(&card.question),
        why: line(&card.why),
        choices,
        stakes: card.stakes,
        batch,
        person_only: true,
    }
}

/// What an applied undo writes for the cards that hold ghosts, so that once it applies a card is
/// open exactly while a ghost waits on it (DR-89).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UndoCards {
    /// The cards it would leave open with no ghost waiting, each with the review that closes it,
    /// which the undo writes under a key of its own ([`card_review`]): accept when any ghost left
    /// on it was accepted, else reject.
    pub close: Vec<(EventId, Verdict)>,
    /// The card reviews that would stand while one of their card's ghosts waits again, such as
    /// after an undo of an earlier ghost's review: the undo also undoes each, in its own command,
    /// so the card reopens, and an undo of the undo closes it again.
    pub reopen: Vec<EventId>,
    /// A card it would open beside one of the same kind and subject that is open now, such as an
    /// Apply card a later row dealt once the first closed, with that open card. The undo is
    /// refused while that card is open, since a dealer has one open card at a time and its later
    /// rows join only that one ([`undo_beside`]).
    pub beside: Option<(EventId, EventId)>,
}

/// [`UndoCards`] for an applied undo of the command that wrote `event`, from the fold as that
/// undo's commit finds it; `events` holds the cards, for their kind and subject.
pub fn after_undo(folded: &Folded, event: &EventId, events: &[Event]) -> UndoCards {
    let after = folded.after_undo(event);
    let mut out = UndoCards::default();
    // Each card that would be open once the undo and what it writes for the cards apply.
    let mut opened = Vec::new();
    for card in folded.cards() {
        if after.is_undone(card) {
            continue;
        }
        let left = after.ghosts_of(card);
        let holds_ghosts = !(left.is_empty() && folded.ghosts_of(card).is_empty());
        let waiting = left.iter().any(|g| after.verdict(g).is_none());
        let open = match (after.card_open(card), waiting) {
            // A card no ghost waits on, now or after, such as one a person picks whole.
            (open, _) if !holds_ghosts => open,
            (true, false) => {
                let accepted = left.iter().any(|g| after.verdict(g) == Some(Verdict::Accept));
                out.close.push((card.clone(), if accepted { Verdict::Accept } else { Verdict::Reject }));
                false
            }
            (false, true) => {
                out.reopen.extend(after.reviews_of(card));
                true
            }
            (open, _) => open,
        };
        if open && !folded.open_cards.contains(card) {
            opened.push(card.clone());
        }
    }
    let subject = |id: &EventId| {
        events.iter().find(|e| &e.id == id).and_then(|e| match &e.body {
            EventBody::Card(card) => Some((card.kind.0.clone(), card.subject.clone())),
            _ => None,
        })
    };
    out.beside = opened.into_iter().find_map(|card| {
        let about = subject(&card)?;
        let closing: Vec<&EventId> = out.close.iter().map(|(c, _)| c).collect();
        folded
            .open_cards
            .iter()
            .find(|open| !closing.contains(open) && subject(open).as_ref() == Some(&about))
            .map(|open| (card.clone(), open.clone()))
    });
    out
}

/// `invalid_argument` for an undo that would open `card` beside `open`, the open card of its kind
/// and subject: it names that card, and its hint is the review a person runs to resolve it, from
/// the view at `head`, the review of its first waiting ghost when it holds any.
pub fn undo_beside(folded: &Folded, card: &EventId, open: &EventId, head: Seq) -> AppError {
    let item = open_ghosts(folded, open).into_iter().next().unwrap_or_else(|| open.clone());
    let what = text::get("edit.undo_beside_card", &[("card", &card.0), ("open", &open.0)]);
    AppError::new(ErrorCode::InvalidArgument, &[("what", &what)])
        .details(json!({"field": "event", "card": open.0}))
        .with_hint(accept_line(&item, head))
}
