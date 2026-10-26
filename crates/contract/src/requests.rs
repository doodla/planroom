//! Requests and results of the catalogue leaves M1 adds: `edit`, `review`, `intent next`,
//! `intent propose`, `intent lint`, `ask` and `find`.
//!
//! Each request derives `clap::Args`, `serde` and `JsonSchema`, as every catalogue request does;
//! a leaf joins `Command` with its handler.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::command::WriteCommon;
use crate::envelope::{Empty, Handle};
use crate::event::Verdict;
use crate::model::{Kind, Level, NodeId};

/// `planroom edit OP ...`: one hand edit. An agent's edit is a ghost until a person accepts it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, clap::Args)]
pub struct EditReq {
    #[command(subcommand)]
    #[serde(flatten)]
    pub op: EditOp,
    #[serde(flatten)]
    #[command(flatten)]
    pub common: WriteCommon,
}

/// The hand edits. Each target is a node ID, an ID suffix or a name.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema, clap::Subcommand)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum EditOp {
    /// Move a node under another parent: a `set parent` edit.
    Move {
        id: String,
        /// The new parent.
        parent: String,
    },
    /// Make a group by hand and move the nodes into it; undoing it removes the group and restores
    /// every member's parent together.
    Group {
        /// The group's name.
        name: String,
        /// The nodes to move into it.
        #[arg(required = true)]
        ids: Vec<String>,
    },
    /// Rename a node.
    Rename { id: String, name: String },
    /// Hide a node; a view counts it as hidden, never drops it silently.
    Hide { id: String },
    /// Pin a node's position in one view.
    Pin {
        id: String,
        /// The view the position belongs to, such as `map:system:all`.
        #[arg(long)]
        view: String,
        #[arg(long, allow_negative_numbers = true)]
        x: f64,
        #[arg(long, allow_negative_numbers = true)]
        y: f64,
    },
    /// Add an intended relation between two nodes.
    Relate {
        from: String,
        to: String,
        /// The relation kind.
        #[arg(long)]
        kind: String,
    },
    /// Remove an intended relation between two nodes.
    Unrelate {
        from: String,
        to: String,
        /// The relation kind.
        #[arg(long)]
        kind: String,
    },
    /// Add a boundary rule that forbids a relation between two nodes.
    Forbid {
        from: String,
        to: String,
        /// The relation kind the rule forbids; every kind when absent.
        #[arg(long)]
        #[serde(default)]
        kind: Option<String>,
    },
    /// Undo the command that wrote an event: every event it wrote, together.
    Undo {
        /// An event ID or an ID suffix.
        event: String,
    },
}

/// `planroom review ID accept|reject`: accept or reject one ghost, conflict, inferred fact or
/// card.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema, clap::Args)]
pub struct ReviewReq {
    /// An event ID or an ID suffix.
    pub id: String,
    #[arg(value_enum)]
    pub verdict: Verdict,
    #[serde(flatten)]
    #[command(flatten)]
    pub common: WriteCommon,
}

/// `planroom intent next [--stage S]`: draft a plan stage's items as ghosts.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema, clap::Args)]
pub struct IntentNextReq {
    /// The stage to draft; the next one the plan lacks when absent.
    #[arg(long)]
    #[serde(default)]
    pub stage: Option<String>,
    #[serde(flatten)]
    #[command(flatten)]
    pub common: WriteCommon,
}

/// `planroom intent propose --file F|-`: plan items proposed as ghosts, one batch.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema, clap::Args)]
pub struct ProposeReq {
    /// The proposal's path, or `-` for stdin, so a proposal writes no file.
    #[arg(long, value_name = "F")]
    pub file: String,
    #[serde(flatten)]
    #[command(flatten)]
    pub common: WriteCommon,
}

/// `planroom intent lint [--project P]`: the plan's lint findings.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema, clap::Args)]
pub struct LintReq {
    /// The project to lint; the current one when absent.
    #[arg(long)]
    #[serde(default)]
    pub project: Option<String>,
}

/// `planroom ask [QUESTION [ARGS]]`: a pack or built-in question; alone, it lists every loaded
/// kind, lens and question.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema, clap::Args)]
pub struct AskReq {
    #[serde(default)]
    pub question: Option<String>,
    /// The question's arguments, such as the two ends of `path A B`.
    #[serde(default)]
    pub args: Vec<String>,
}

/// What `ask` returns: the answer's summary line and its items, each with the handles that drill
/// down.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AskResult {
    /// The question's ID, such as `unmonitored`.
    pub question: String,
    pub summary: String,
    pub items: Vec<AskItem>,
    pub empty: Option<Empty>,
}

/// One item of an answer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AskItem {
    /// A node ID, or with `ask` alone a kind, lens or question ID.
    pub id: String,
    #[schemars(extend("x-redact" = true))]
    pub name: String,
    /// One line on why the item answers the question.
    pub why: String,
    pub handles: Vec<Handle>,
}

/// `planroom find TEXT`: fuzzy search over node names and IDs.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema, clap::Args)]
pub struct FindReq {
    pub text: String,
}

/// What `find` returns: the matches, best first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct FindResult {
    pub summary: String,
    pub items: Vec<FindItem>,
    pub empty: Option<Empty>,
}

/// One match, with its level and kind.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct FindItem {
    pub id: NodeId,
    #[schemars(extend("x-redact" = true))]
    pub name: String,
    pub level: Level,
    pub kind: Kind,
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::{Args, FromArgMatches};
    use serde_json::json;

    fn parse<T: Args + FromArgMatches>(args: &[&str]) -> T {
        let cmd = T::augment_args(clap::Command::new("t").no_binary_name(true));
        cmd.clone().debug_assert();
        T::from_arg_matches(&cmd.try_get_matches_from(args).unwrap()).unwrap()
    }

    #[test]
    fn an_edit_parses_from_its_op_and_travels_with_it_flattened() {
        let edit: EditReq = parse(&["--base", "41", "group", "purchase", "src/cart", "src/checkout"]);
        assert_eq!(
            edit.op,
            EditOp::Group { name: "purchase".into(), ids: vec!["src/cart".into(), "src/checkout".into()] }
        );
        assert_eq!(edit.common.base, Some(41));
        let json = serde_json::to_value(&edit).unwrap();
        assert_eq!(json["op"], "group");
        assert_eq!(json["base"], 41);
        let back: EditReq = serde_json::from_value(json).unwrap();
        assert_eq!(back, edit);
        let undo: EditReq = serde_json::from_value(json!({"op": "undo", "event": "01JB"})).unwrap();
        assert_eq!(undo.op, EditOp::Undo { event: "01JB".into() });
    }

    #[test]
    fn review_takes_an_id_and_a_verdict() {
        let review: ReviewReq = parse(&["01JB", "accept"]);
        assert_eq!(review.verdict, Verdict::Accept);
        assert_eq!(
            serde_json::to_value(&review).unwrap(),
            json!({"id": "01JB", "verdict": "accept", "base": null, "idem": null, "project": null})
        );
    }

    #[test]
    fn ask_and_the_intent_leaves_parse_their_flags() {
        let ask: AskReq = parse(&["path", "storefront", "worker"]);
        assert_eq!(ask.question.as_deref(), Some("path"));
        assert_eq!(ask.args, ["storefront", "worker"]);
        let next: IntentNextReq = parse(&["--stage", "outcome"]);
        assert_eq!(next.stage.as_deref(), Some("outcome"));
        let propose: ProposeReq = parse(&["--file", "-"]);
        assert_eq!(propose.file, "-");
        let lint: LintReq = parse(&[]);
        assert_eq!(lint.project, None);
        let find: FindReq = parse(&["worker"]);
        assert_eq!(find.text, "worker");
    }
}
