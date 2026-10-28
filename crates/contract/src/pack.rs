//! The pack manifest, `pack.toml`, in the deserialize view: what a pack declares (DR-9, DR-46).
//!
//! A pack is data, never code. The loader parses its TOML, validates it against `PackFile` in the
//! input schema and checks its references, failing with `pack_invalid`. Kind and relation IDs here
//! are the pack's own short names; the loader qualifies them as `<pack>.<name>`.

use std::collections::BTreeMap;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::decision::{DecisionKind, EffectOp, Predicate};
use crate::envelope::CommandName;
use crate::model::Level;
use crate::view::Tone;

/// One pack's `pack.toml`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PackFile {
    /// `planroom.pack/v1`.
    pub format: String,
    /// Lowercase, `[a-z0-9_]`.
    pub id: String,
    pub version: u32,
    pub title: String,
    /// A built-in pack: `[]` or `["code"]`; an out-of-tree pack: any built-in packs (DR-62).
    #[serde(default)]
    pub requires: Vec<String>,
    /// Merged across packs; a duplicate extension is `pack_invalid`.
    #[serde(default)]
    pub languages: Vec<LanguageDecl>,
    /// Merged across packs; a duplicate glob is `pack_invalid`, and overlapping globs take the
    /// first match in load order.
    #[serde(default)]
    pub image_roles: Vec<ImageRole>,
    #[serde(default)]
    pub kinds: Vec<KindDecl>,
    #[serde(default)]
    pub relations: Vec<RelationDecl>,
    /// A hint kind no loaded linker consumes is `pack_invalid` (DR-9).
    #[serde(default)]
    pub hints: Vec<HintDecl>,
    #[serde(default)]
    pub extractors: Vec<ExtractorDecl>,
    #[serde(default)]
    pub linkers: Vec<LinkerDecl>,
    #[serde(default)]
    pub lenses: Vec<LensDecl>,
    #[serde(default)]
    pub questions: Vec<QuestionDecl>,
    #[serde(default)]
    pub rules: Vec<RuleDecl>,
    #[serde(default)]
    pub decisions: Vec<DecisionKind>,
    /// Each action names a leaf that can be undone, isn't a hand-off and never spends (DR-54).
    #[serde(default)]
    pub suggestions: Vec<SuggestionDecl>,
    #[serde(default)]
    pub prompts: Vec<PromptDecl>,
    /// The plan pack only.
    #[serde(default)]
    pub stages: Vec<StageDecl>,
}

/// `[[languages]]`: a language's file extensions and comment forms.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LanguageDecl {
    pub id: String,
    pub extensions: Vec<String>,
    #[serde(default)]
    pub line_comment: Option<String>,
    /// The opening and closing marks.
    #[serde(default)]
    pub block_comment: Option<[String; 2]>,
}

/// `[[image_roles]]`: a compose image glob's kind and role, such as `redis*` to a queue kind with
/// role `queue`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ImageRole {
    /// A glob over the image name.
    pub image: String,
    pub role: String,
    pub kind: String,
}

/// `[[kinds]]`: a node kind.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct KindDecl {
    pub id: String,
    /// The ID's kind-short. One pack owns it, except the kernel's `container`, which every
    /// Container-level kind uses (DR-46).
    pub id_prefix: String,
    pub level: Level,
    pub label: String,
    #[serde(default)]
    pub parents: Vec<String>,
    /// A JSON Schema fragment the kind's props validate against.
    #[serde(default)]
    pub props: Value,
    #[serde(default)]
    pub icon: Option<String>,
    /// A Container-level kind's block color; surfaces style from `MapNode.tone`, never from a kind
    /// ID (DR-46).
    #[serde(default)]
    pub tone: Option<Tone>,
    #[serde(default)]
    pub empty_text: Option<String>,
    #[serde(default)]
    pub empty_action: Option<String>,
}

/// `[[relations]]`: an edge kind.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RelationDecl {
    pub id: String,
    pub from: Vec<String>,
    pub to: Vec<String>,
    pub label: String,
    /// A JSON Schema fragment the relation's props validate against.
    #[serde(default)]
    pub props: Value,
    #[serde(default)]
    pub inferred: bool,
}

/// `[[hints]]`: a hint kind a linker consumes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct HintDecl {
    pub id: String,
    /// The attribute names the hint carries, such as `scheme`, `port` and `path`.
    #[serde(default)]
    pub attrs: Vec<String>,
}

/// `[[extractors]]`: a built-in, a regex or a tree-sitter query, told apart by which of
/// `builtin`, `regex` or `query` it sets.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum ExtractorDecl {
    Builtin {
        id: String,
        builtin: String,
        #[serde(default)]
        files: Vec<String>,
        #[serde(default)]
        options: Value,
    },
    Regex {
        id: String,
        files: Vec<String>,
        regex: String,
        emit: Vec<EmitTemplate>,
    },
    Query {
        id: String,
        files: Vec<String>,
        /// A `[[languages]]` ID with a built-in grammar.
        language: String,
        /// The path of the `.scm` file, under `queries/`.
        query: String,
        emit: Vec<EmitTemplate>,
    },
}

/// What a declarative extractor emits per match. Values may use `$file`, `$stem`, `$repo`,
/// `$line`, `$1`..`$9` and `@capture`. A reference is `@alias:...`, an ID, `@id:KIND:KEY` or
/// `@ext:NAME`; a bare path such as `$file` is `pack_invalid`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum EmitTemplate {
    Node {
        node: String,
        key: String,
        #[serde(default)]
        name: Option<String>,
        #[serde(default)]
        parent: Option<String>,
        /// TOML values; captures are coerced to the prop schema's type.
        #[serde(default)]
        props: BTreeMap<String, Value>,
        #[serde(default)]
        aliases: Vec<String>,
    },
    Edge {
        edge: String,
        from: String,
        to: String,
        #[serde(default)]
        via: Option<String>,
        /// 1.0 when absent, as for every deterministic tier.
        #[serde(default)]
        confidence: Option<f32>,
        #[serde(default)]
        props: BTreeMap<String, Value>,
    },
    Hint {
        hint: String,
        scope: String,
        key: String,
        #[serde(default)]
        value: Option<String>,
        #[serde(default)]
        attrs: BTreeMap<String, String>,
    },
}

/// `[[linkers]]`: a built-in linker and its options.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LinkerDecl {
    pub builtin: String,
    #[serde(default)]
    pub options: Value,
}

/// `[[lenses]]`: which relationships a view shows (DR-15).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LensDecl {
    pub id: String,
    pub label: String,
    pub relations: Vec<String>,
    /// The direction each relation flows in; forward when absent.
    #[serde(default)]
    pub flow: BTreeMap<String, Flow>,
    /// The node kinds the lens is about: its map shows their nodes first, and `map --lens` alone
    /// takes the lens's level.
    #[serde(default)]
    pub kinds: Vec<String>,
    pub level: Level,
    /// Two defaults are `pack_invalid`, unless one is the code pack's, which wins.
    #[serde(default)]
    pub default: bool,
    #[serde(default)]
    pub empty_text: Option<String>,
    #[serde(default)]
    pub empty_action: Option<String>,
}

/// The direction a lens follows a relation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Flow {
    #[default]
    Forward,
    Backward,
}

/// `[[questions]]`: a named pipeline of one to three query shapes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct QuestionDecl {
    pub id: String,
    #[serde(default)]
    pub args: Vec<ArgDecl>,
    /// 1 to 3; each step's input is the previous step's node set.
    pub steps: Vec<ShapeStep>,
    /// The summary line's template.
    pub summary: String,
    /// Clamped to 3,000; absent takes `ask`'s 1,500 (DR-21).
    #[serde(default)]
    pub budget_tokens: Option<u32>,
}

/// One argument a question takes, such as `X` in `fires-if X`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ArgDecl {
    pub name: String,
    #[serde(default)]
    pub optional: bool,
}

/// One step of a question: a shape and its parameters.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ShapeStep {
    pub shape: Shape,
    #[serde(default)]
    pub params: Value,
}

/// The kernel's twelve query shapes. `kinds`, `stage`, `lint`, `missing` and `drift` are built-in
/// questions the engine serves, not shapes (DR-15).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Shape {
    Node,
    Find,
    Neighbors,
    Path,
    Why,
    Filter,
    Compare,
    Order,
    Subgraph,
    Tree,
    Machine,
    Checks,
}

/// What one parameter of a query shape, a lint rule type, a decision effect or a `when`
/// predicate names. The pack loader checks each name a reference parameter holds against the
/// pack's scope, and refuses a key the table doesn't declare, so no parameter goes unread (DR-62).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParamNames {
    /// A number, a direction, a prop name or text: no reference.
    Nothing,
    /// A field test: a value, or `{ lt = N }`, `{ gt = N }` or `{ in = [...] }`.
    Test,
    /// Node kinds, never relations.
    NodeKind,
    /// The kinds of the subject a `when` predicate selects: node kinds when the subject is a node
    /// kind, relations when it is a relation, and either for a role's subjects or a suggestion.
    SubjectKind,
    /// Relations, the edge kinds.
    Relation,
    /// Lenses, by ID.
    Lens,
    /// Node IDs. Where the docs allow one, a placeholder stands in: a question's argument such as
    /// `$X` in a step, and `$pick` in an effect.
    NodeId,
    /// A table with its own declared parameters, such as `without_edge`'s.
    Table(&'static [Param]),
}

/// One declared parameter. A key that ends in `.*` stands for every key with that prefix and a
/// name after it, such as `props.*` for `"props.role"`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Param {
    pub key: &'static str,
    pub names: ParamNames,
}

/// A [`Param`] as a constant expression, so a table of them is a `&'static` slice.
macro_rules! param {
    ($key:literal, $names:expr) => {
        Param { key: $key, names: $names }
    };
}

/// A filter's `with_edge` and `without_edge`: a relation, a direction and an optional layer.
const EDGE_TEST: &[Param] = &[
    param!("relation", ParamNames::Relation),
    param!("direction", ParamNames::Nothing),
    param!("layer", ParamNames::Nothing),
];

/// Prop tests in the filter shape's predicate form, such as `"props.role" = { in = [...] }`.
const PROP_TESTS: Param = param!("props.*", ParamNames::Test);

/// The keys of a `when` predicate, a decision kind's or a suggestion's: the filter shape's
/// predicate form, with the subject's kind, its ID and parent, its `status` and `confidence`, prop
/// tests and edge tests. architecture.md's predicate table lists the same keys.
pub const PREDICATE_PARAMS: &[Param] = &[
    param!("kind", ParamNames::SubjectKind),
    param!("kinds", ParamNames::SubjectKind),
    param!("id", ParamNames::NodeId),
    param!("parent", ParamNames::NodeId),
    param!("status", ParamNames::Test),
    param!("confidence", ParamNames::Test),
    PROP_TESTS,
    param!("with_edge", ParamNames::Table(EDGE_TEST)),
    param!("without_edge", ParamNames::Table(EDGE_TEST)),
];

impl Shape {
    /// Every parameter the shape takes: architecture.md's "Lenses and questions" table.
    pub const fn params(self) -> &'static [Param] {
        use ParamNames::{Lens, NodeId, NodeKind, Nothing, Relation, Table};
        match self {
            Shape::Node | Shape::Tree => &[param!("id", NodeId)],
            Shape::Find => &[param!("text", Nothing)],
            Shape::Neighbors => &[
                param!("ids", NodeId),
                param!("lens", Lens),
                param!("relations", Relation),
                param!("depth", Nothing),
                param!("direction", Nothing),
            ],
            Shape::Path => &[param!("from", NodeId), param!("to", NodeId), param!("lens", Lens)],
            Shape::Why => &[param!("from", NodeId), param!("to", NodeId)],
            Shape::Filter => &[
                param!("kinds", NodeKind),
                PROP_TESTS,
                param!("with_edge", Table(EDGE_TEST)),
                param!("without_edge", Table(EDGE_TEST)),
            ],
            Shape::Compare => &[
                param!("relations", Relation),
                param!("edge_prop", Nothing),
                param!("end", Nothing),
                param!("end_prop", Nothing),
            ],
            Shape::Order => {
                &[param!("from", NodeId), param!("relations", Relation), param!("direction", Nothing)]
            }
            Shape::Subgraph => &[param!("lens", Lens), param!("props", Nothing)],
            Shape::Machine => &[
                param!("id", NodeId),
                param!("steps", Nothing),
                param!("event", Nothing),
                param!("guard", Nothing),
                param!("action", Nothing),
                param!("initial", Nothing),
                param!("terminal", Nothing),
            ],
            Shape::Checks => &[
                param!("kind", NodeKind),
                param!("relation", Relation),
                param!("event", Nothing),
                param!("guard", Nothing),
                param!("initial", Nothing),
                param!("terminal", Nothing),
            ],
        }
    }
}

/// `[[rules]]`: a structural lint rule over one kind.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RuleDecl {
    pub id: String,
    pub kind: RuleType,
    /// The kind the rule checks.
    pub subject: String,
    #[serde(default)]
    pub params: Value,
    pub message: String,
}

/// The five lint rule types. `covers` finds a subject whose counted children don't cover its own
/// `prop` values (the WBS 100% rule), and `disjoint` one two of whose counted children share a
/// value of `prop` (MECE).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RuleType {
    RequiresProp,
    RequiresEdge,
    MinChildren,
    Covers,
    Disjoint,
}

impl RuleType {
    /// Every parameter the rule type takes: architecture.md's rule types table. Each also takes
    /// prop tests that narrow the subjects it checks, such as `"props.type" = "next_action"`.
    pub const fn params(self) -> &'static [Param] {
        use ParamNames::{NodeKind, Nothing, Relation};
        match self {
            RuleType::RequiresProp => &[param!("prop", Nothing), PROP_TESTS],
            RuleType::RequiresEdge => &[
                param!("relation", Relation),
                param!("direction", Nothing),
                param!("kinds", NodeKind),
                PROP_TESTS,
            ],
            RuleType::MinChildren => &[param!("min", Nothing), param!("kinds", NodeKind), PROP_TESTS],
            RuleType::Covers | RuleType::Disjoint => {
                &[param!("prop", Nothing), param!("kinds", NodeKind), PROP_TESTS]
            }
        }
    }
}

impl EffectOp {
    /// Every parameter the effect takes besides `on` and `do`: architecture.md's effects table.
    /// The subject is one end of a `put_edge` or `retract_edge` edge, `$pick` names the chosen
    /// candidate in a node-ID parameter, and `retarget` always moves onto the pick.
    pub const fn params(self) -> &'static [Param] {
        use ParamNames::{NodeId, Nothing, Relation};
        match self {
            EffectOp::None | EffectOp::Retarget => &[],
            EffectOp::Lint => &[param!("rule", Nothing)],
            EffectOp::SetProp => {
                &[param!("prop", Nothing), param!("value", Nothing), param!("parent", NodeId)]
            }
            EffectOp::PutEdge | EffectOp::RetractEdge => {
                &[param!("relation", Relation), param!("from", NodeId), param!("to", NodeId)]
            }
        }
    }
}

/// `[[suggestions]]`: a candidate for the one next action.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SuggestionDecl {
    /// When the suggestion applies, in the filter shape's predicate form.
    #[serde(default)]
    pub when: Predicate,
    pub text: String,
    pub action: ActionDecl,
    /// Higher ranks first.
    #[serde(default)]
    pub priority: u32,
}

/// The leaf a suggestion runs, with its arguments.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ActionDecl {
    pub command: CommandName,
    #[serde(default)]
    pub args: Vec<String>,
}

/// `[[prompts]]`: a Claude stable prefix the engine sends.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PromptDecl {
    pub id: String,
    /// The path of the prompt, under `prompts/`.
    pub file: String,
    pub tier: ModelTier,
    pub role: PromptRole,
    /// The contract type name the reply validates against.
    pub output: String,
    #[serde(default)]
    pub ttl: CacheTtl,
}

/// Which Claude model a call takes: the mod maps `cheap` to Haiku and `capable` to Sonnet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ModelTier {
    Cheap,
    #[default]
    Capable,
}

/// What a prompt's output does: an inferred fact, or a proposal that lands as ghosts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum PromptRole {
    Infer,
    Propose,
}

/// How long Claude caches a prompt's prefix: 5 minutes, or an hour for person-paced purposes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, Serialize, Deserialize, JsonSchema)]
pub enum CacheTtl {
    #[default]
    #[serde(rename = "5m")]
    M5,
    #[serde(rename = "1h")]
    H1,
}

/// `[[stages]]`: one plan stage (R5).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StageDecl {
    pub id: String,
    pub method: String,
    /// The `kernel.item` types the stage writes.
    pub items: Vec<ItemType>,
    /// The stage's instructions, sent in the uncached tail after the plan pack's shared prefix.
    pub prompt: String,
    /// The IDs of the lint rules the stage runs.
    #[serde(default)]
    pub lint: Vec<String>,
}

/// One plan item type a stage writes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ItemType {
    /// The item's `type` prop, such as `next_action`.
    pub id: String,
    /// A JSON Schema fragment.
    #[serde(default)]
    pub props: Value,
}

/// One loaded pack as the store's `meta.packs` records it, in load order (DR-62).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct PackRecord {
    pub id: String,
    pub version: u32,
    /// `builtin` for a pack embedded in the binary; else the directory it was loaded from.
    pub source: String,
    /// The lowercase hex sha256 over the pack's files, each path and its bytes in path order.
    pub sha256: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_pack_with_only_its_header_reads_with_empty_tables() {
        let pack: PackFile = serde_json::from_value(json!({
            "format": "planroom.pack/v1", "id": "toy", "version": 1, "title": "Toy"
        }))
        .unwrap();
        assert!(pack.requires.is_empty() && pack.kinds.is_empty() && pack.decisions.is_empty());
    }

    #[test]
    fn extractors_are_told_apart_by_builtin_regex_or_query() {
        let decls: Vec<ExtractorDecl> = serde_json::from_value(json!([
            {"id": "files", "builtin": "files"},
            {"id": "calls", "files": ["**/*.rb"], "regex": "get\\(\"(.*)\"\\)",
             "emit": [{"hint": "toy.url", "scope": "@id:toy.file:$file", "key": "$1"}]},
            {"id": "go_imports", "files": ["**/*.go"], "language": "go", "query": "queries/imports.scm",
             "emit": [{"edge": "toy.imports", "from": "@id:toy.file:$file", "to": "@alias:@path"}]}
        ]))
        .unwrap();
        assert!(matches!(decls[0], ExtractorDecl::Builtin { .. }));
        assert!(
            matches!(&decls[1], ExtractorDecl::Regex { emit, .. } if matches!(emit[0], EmitTemplate::Hint { .. }))
        );
        assert!(
            matches!(&decls[2], ExtractorDecl::Query { emit, .. } if matches!(emit[0], EmitTemplate::Edge { .. }))
        );
    }

    #[test]
    fn a_prompt_ttl_reads_as_written() {
        let ttl: CacheTtl = serde_json::from_value(json!("1h")).unwrap();
        assert_eq!(ttl, CacheTtl::H1);
        assert_eq!(CacheTtl::default(), CacheTtl::M5);
    }
}
