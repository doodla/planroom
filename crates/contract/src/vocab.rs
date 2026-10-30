//! The vocabulary the loaded packs give the kernel: kinds, relations, lenses and suggestions,
//! with every ID qualified as `<pack>.<name>`.
//!
//! The kernel is domain-free: it learns what a kind's level, ID prefix, props and tone are only
//! from this, which the engine builds from the loaded packs' `PackFile`s in load order.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::model::{Kind, LevelId};
use crate::pack::{Flow, SuggestionDecl};
use crate::view::{Tone, Yarn};

/// Everything the loaded packs declare that the fold, resolution and the views read.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize, JsonSchema)]
pub struct Vocabulary {
    /// In load order: built-in packs first, then out-of-tree packs.
    pub kinds: Vec<KindVocab>,
    pub relations: Vec<RelationVocab>,
    pub lenses: Vec<LensVocab>,
    pub suggestions: Vec<SuggestionDecl>,
    /// The levels: `intent` first, apart from the ladder, then the ladder top to finest.
    #[serde(default)]
    pub levels: Vec<LevelVocab>,
}

/// One level as the views read it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct LevelVocab {
    pub id: LevelId,
    /// The noun "the {level} level" reads, such as `container`.
    pub label: String,
    /// The count of one, such as "{count} container".
    pub one: String,
    /// The count of many, such as "{count} containers".
    pub many: String,
    /// Apart from the ladder: the kernel's `intent`, which no chain holds.
    pub apart: bool,
    /// The pack that declares the level: `kernel` for `intent`.
    pub pack: String,
}

/// One node kind.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct KindVocab {
    pub kind: Kind,
    pub level: LevelId,
    /// The ID's kind-short: `<namespace>:<id_prefix>:<key>`.
    pub id_prefix: String,
    pub label: String,
    /// The JSON Schema fragment the kind's props validate against.
    pub props: Value,
    /// A kind's block color.
    pub tone: Option<Tone>,
}

/// One edge kind.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct RelationVocab {
    pub kind: Kind,
    pub label: String,
    pub from: Vec<Kind>,
    pub to: Vec<Kind>,
    /// The JSON Schema fragment the relation's props validate against.
    pub props: Value,
    pub inferred: bool,
}

/// One lens: which relations a view lifts, which way each flows, and what it is about.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct LensVocab {
    pub id: String,
    pub label: String,
    pub relations: Vec<Kind>,
    /// A relation absent here flows forward.
    pub flow: BTreeMap<Kind, Flow>,
    /// The node kinds the lens is about; its map shows their nodes first (DR-15).
    pub kinds: Vec<Kind>,
    /// The level `map --lens` alone takes.
    pub level: LevelId,
    /// The one default lens.
    pub default: bool,
    /// The lens's edge color, from its pack.
    pub yarn: Option<Yarn>,
}
