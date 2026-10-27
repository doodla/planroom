//! The graph model: the four shapes every pack's facts use (`Node`, `Edge`, `Evidence`, `Hint`).

use std::collections::BTreeMap;
use std::fmt;
use std::str::FromStr;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::event::{Actor, EventId};

/// A store version: the global event order. Always below 2^53.
pub type Seq = u64;

/// The largest integer a JSON reader holds exactly, 2^53 - 1. Every `u64` in the contract carries
/// it as its schema `maximum`, so a JavaScript reader never rounds one (Contracts, Versioning
/// rules: Integers).
pub const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;

/// A node ID, `<namespace>:<kind-short>:<key>`, such as `orders:pkg:internal/http`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(transparent)]
pub struct NodeId(#[schemars(pattern(r"^[^:\s]+:[^:\s]+:.+$"))] pub String);

impl fmt::Display for NodeId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl FromStr for NodeId {
    type Err = String;

    /// Accepts `<namespace>:<kind-short>:<key>` with three non-empty parts.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut parts = s.splitn(3, ':');
        let ok = parts.next().is_some_and(|p| !p.is_empty() && !p.contains(char::is_whitespace))
            && parts.next().is_some_and(|p| !p.is_empty() && !p.contains(char::is_whitespace))
            && parts.next().is_some_and(|p| !p.is_empty());
        if ok {
            Ok(NodeId(s.to_owned()))
        } else {
            Err(format!("`{s}` is not a node ID (namespace:kind:key)"))
        }
    }
}

/// An edge ID: `e:` plus 12 hex characters of `sha256(kind ‖ from ‖ to ‖ via)`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(transparent)]
pub struct EdgeId(#[schemars(pattern(r"^e:[0-9a-f]{12}$"))] pub String);

/// A kind, `<pack>.<name>`, such as `code.service`, `kernel.group` or `kernel.external`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(transparent)]
pub struct Kind(#[schemars(pattern(r"^[a-z][a-z0-9_]*\.[a-z][a-z0-9_]*$"))] pub String);

/// The five fixed levels, from the plan down to the code.
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    Serialize,
    Deserialize,
    JsonSchema,
    clap::ValueEnum,
)]
#[serde(rename_all = "snake_case")]
pub enum Level {
    Intent,
    System,
    Container,
    Component,
    Code,
}

/// Which layer a fact or edit belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Layer {
    Actual,
    Intended,
}

/// How a fact was found, from the least to the most specific tier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Tier {
    Universal,
    Syntactic,
    /// SCIP or LSIF; no v0.1 extractor (DR-11).
    Precise,
    Config,
    Pattern,
    Inference,
    Linked,
}

/// Where a node or edge came from.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Provenance {
    Extracted {
        extractor: String,
        tier: Tier,
    },
    /// The linker is pack-qualified, such as `code/runtime_wiring`.
    Linked {
        linker: String,
        hints: Vec<HintRef>,
    },
    Inferred {
        model: String,
        decision: EventId,
        confidence: f32,
    },
    Authored {
        actor: Actor,
        event: EventId,
    },
}

/// An edge end: a node ID, or an alias another extractor defines, such as `host:orders`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Ref {
    Id(NodeId),
    Alias(String),
}

/// What a fact was read from, verified at ingest (DR-12).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Evidence {
    /// A file fact: verified against the line index of the blob at `commit`.
    Line {
        /// `<repo>/<repo-relative path>`.
        path: String,
        /// 1-based.
        #[schemars(range(min = 1))]
        line: u32,
        /// The exact line, cut to 200 characters; omitted in a default Artifact snapshot.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        #[schemars(length(max = 200))]
        #[schemars(extend("x-redact" = true))]
        contains: Option<String>,
        commit: Option<String>,
        branch: Option<String>,
    },
    /// A git-derived fact (a tag, a commit, co-change, ownership, the tree at a commit): the commit
    /// is verified to exist, and the ref to name it.
    Ref {
        commit: String,
        /// Such as `refs/tags/v1.5.0` or `refs/heads/main`.
        #[serde(rename = "ref")]
        ref_name: Option<String>,
    },
}

/// The most characters of a line that `Evidence::Line.contains` keeps.
pub const CONTAINS_MAX_CHARS: usize = 200;

/// A thing in the system at one level, with exactly one parent.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Node {
    pub id: NodeId,
    pub kind: Kind,
    pub level: Level,
    /// `None` only at the top.
    pub parent: Option<NodeId>,
    #[schemars(extend("x-redact" = true))]
    pub name: String,
    /// Such as `host:orders`, `service:orders`, `pkg:@acme/kit`.
    pub aliases: Vec<String>,
    /// Validated against the kind's prop schema.
    #[schemars(extend("x-redact" = true))]
    pub props: BTreeMap<String, Value>,
    pub layer: Layer,
    pub provenance: Provenance,
    pub evidence: Vec<Evidence>,
}

/// A relation between two nodes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Edge {
    pub id: EdgeId,
    /// Such as `code.http` or `observability.watches`.
    pub kind: Kind,
    pub from: Ref,
    pub to: Ref,
    /// Such as `POST /orders`, `order-jobs`, `@acme/kit@1.1.0`.
    pub via: Option<String>,
    /// A node whose evidence also counts for this edge, such as an endpoint.
    pub via_ref: Option<Ref>,
    #[schemars(extend("x-redact" = true))]
    pub props: BTreeMap<String, Value>,
    pub evidence: Vec<Evidence>,
    /// 1.0 for deterministic tiers.
    #[schemars(range(min = 0.0, max = 1.0))]
    pub confidence: f32,
    pub layer: Layer,
    pub provenance: Provenance,
}

/// A partial observation a linker joins with others, such as a URL literal or an environment read.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Hint {
    /// 12 hex characters of `sha256(kind ‖ scope ‖ key ‖ evidence)`.
    #[schemars(pattern(r"^[0-9a-f]{12}$"))]
    pub id: String,
    /// Such as `code.url` or `code.env_read`.
    pub kind: Kind,
    /// The file, or the compose service.
    pub scope: NodeId,
    /// The join key: a host, an environment variable name, a path, a queue key.
    pub key: String,
    #[schemars(extend("x-redact" = true))]
    pub value: Option<String>,
    /// Scheme, port, path, method, op, driver, default.
    #[schemars(extend("x-redact" = true))]
    pub attrs: BTreeMap<String, Value>,
    pub evidence: Vec<Evidence>,
    /// Always `Extracted`.
    pub provenance: Provenance,
}

/// A hint a linked edge joined: its ID and the pack-qualified extractor that emitted it.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub struct HintRef {
    pub id: String,
    pub extractor: String,
}

/// The format tag of a `FactBatch`.
pub const FACTS_FORMAT: &str = "planroom.facts/v1";

/// What every extractor and linker returns, sorted by ID.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct FactBatch {
    /// `planroom.facts/v1`.
    pub format: String,
    /// Pack-qualified, such as `code/files`.
    pub extractor: String,
    pub tier: Tier,
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    pub hints: Vec<Hint>,
    /// Files read with errors; their facts are weak.
    pub partial: Vec<PartialFile>,
}

/// A file an extractor read with errors, such as one that isn't UTF-8.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub struct PartialFile {
    /// `<repo>/<repo-relative path>`.
    pub path: String,
    pub errors: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_node_id_needs_three_non_empty_parts() {
        assert!("orders:pkg:internal/http".parse::<NodeId>().is_ok());
        assert!("plan:checkout:retries".parse::<NodeId>().is_ok());
        assert!("orders:pkg".parse::<NodeId>().is_err());
        assert!("orders::x".parse::<NodeId>().is_err());
        assert!("orders:pkg:".parse::<NodeId>().is_err());
    }
}
