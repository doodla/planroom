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
    Config,
    Pattern,
    Inference,
    Linked,
}

/// Where a node or edge came from.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Provenance {
    Extracted { extractor: String, tier: Tier },
    Linked { linker: String, hints: Vec<String> },
    Inferred { model: String, decision: EventId, confidence: f32 },
    Authored { actor: Actor, event: EventId },
}

/// An edge end: a node ID, or an alias another extractor defines, such as `host:orders`.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Ref {
    Id(NodeId),
    Alias(String),
}

/// A pointer to the line a fact was read from.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub struct Evidence {
    /// `<repo>/<repo-relative path>`.
    pub path: String,
    /// 1-based.
    #[schemars(range(min = 1))]
    pub line: u32,
    /// The exact line, cut to 200 characters; omitted in a default Artifact snapshot.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schemars(length(max = 200))]
    pub contains: Option<String>,
    pub commit: Option<String>,
    pub branch: Option<String>,
}

/// A thing in the system at one level, with exactly one parent.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct Node {
    pub id: NodeId,
    pub kind: Kind,
    pub level: Level,
    /// `None` only at the top.
    pub parent: Option<NodeId>,
    pub name: String,
    /// Such as `host:orders`, `service:orders`, `pkg:@acme/kit`.
    pub aliases: Vec<String>,
    /// Validated against the kind's prop schema.
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
    /// Such as `code.url` or `code.env_read`.
    pub kind: Kind,
    /// The file, or the compose service.
    pub scope: NodeId,
    /// The join key: a host, an environment variable name, a path, a queue key.
    pub key: String,
    pub value: Option<String>,
    /// Scheme, port, path, method, op, driver, default.
    pub attrs: BTreeMap<String, Value>,
    pub evidence: Vec<Evidence>,
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
