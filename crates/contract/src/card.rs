//! Cards a write deals before `decide` exists: the body of a `card` event (DR-89).
//!
//! The open cards are the `card` events no `review` names; `review CARD accept|reject` picks one
//! through the existing `Review { of }` body. A card fills a `DecisionCard`: its question, its why
//! line and its stakes come from the event.

use std::borrow::Cow;
use std::collections::BTreeMap;

use schemars::{JsonSchema, Schema, SchemaGenerator};
use serde::{Deserialize, Serialize};

use crate::envelope::CommandName;
use crate::open_set::{self, CODE_PATTERN};
use crate::view::{Choice, Stakes};

/// One card a write dealt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Card {
    /// Sent as `card_kind`, not `kind`, because `kind` is the event body's tag, as with
    /// `DecisionRecord.decision_kind`.
    #[serde(rename = "card_kind")]
    pub kind: CardKind,
    /// What the card is about, such as the workspace root or a pack's ID; no second card of one
    /// kind and subject is dealt while one is open.
    pub subject: String,
    /// The `en.toml` key of the card's one-line question.
    pub question: String,
    /// The `en.toml` key of the card's one-line why.
    pub why: String,
    /// The values the question's and why's `{name}`s take, such as a pack's name or an edit count.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    #[schemars(extend("x-redact" = true))]
    pub vars: BTreeMap<String, String>,
    pub stakes: Stakes,
    /// 2 to 4; each label is an `en.toml` key, as the question is.
    pub choices: Vec<Choice>,
    /// The ID of the choice that holds while nobody picks.
    pub default: String,
    /// The leaf whose write dealt the card, such as `scan` or `import`.
    pub dealt_by: CommandName,
}

/// A card's kind: an open set, so an older reader keeps a card of a kind it doesn't know.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CardKind(pub String);

impl CardKind {
    /// The kinds M1 deals: the Apply card for unsigned inbox edits, workspace trust, a changed
    /// pack source and a restore from backup.
    pub const KNOWN: &'static [&'static str] = &["apply", "trust", "pack_source", "restore"];
}

impl JsonSchema for CardKind {
    fn schema_name() -> Cow<'static, str> {
        "CardKind".into()
    }

    fn json_schema(_: &mut SchemaGenerator) -> Schema {
        open_set::schema("A card's kind; an open set.", CardKind::KNOWN.iter().copied(), CODE_PATTERN)
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::event::{EventBody, EventKind};

    #[test]
    fn a_card_event_round_trips_as_its_own_kind() {
        let raw = json!({
            "kind": "card",
            "card_kind": "trust",
            "subject": "/work/shop",
            "question": "card.trust_question",
            "why": "card.trust_why",
            "stakes": "person",
            "choices": [
                {"id": "accept", "label": "card.trust_accept", "preselected": false},
                {"id": "reject", "label": "card.trust_reject", "preselected": true}
            ],
            "default": "reject",
            "dealt_by": "scan"
        });
        let body: EventBody = serde_json::from_value(raw.clone()).unwrap();
        assert_eq!(body.kind(), EventKind::Card);
        assert!(matches!(&body, EventBody::Card(card) if card.kind.0 == "trust"));
        assert_eq!(serde_json::to_value(&body).unwrap(), raw);
    }

    #[test]
    fn a_card_of_a_later_kind_still_reads_and_writes_back_unchanged() {
        let raw = json!({
            "kind": "card", "card_kind": "budget_raise", "subject": "fold.20k",
            "question": "card.q", "why": "card.w", "vars": {"value": "60"}, "stakes": "person",
            "choices": [], "default": "keep", "dealt_by": "scan"
        });
        let body: EventBody = serde_json::from_value(raw.clone()).unwrap();
        assert_eq!(body.kind(), EventKind::Card);
        assert_eq!(serde_json::to_value(&body).unwrap(), raw);
    }
}
