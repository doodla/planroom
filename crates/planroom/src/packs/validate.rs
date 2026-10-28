//! Checking a pack set before anything reads it: each manifest's header, the inward-only rule by
//! origin, every reference (a decision kind's, a question step's, a rule's, a suggestion's and a
//! literal node ID's among them), the ID prefixes, the merged tables, the built-ins each pack
//! names, each declarative extractor's regex, the default lens and the hint kinds no linker joins
//! (DR-9, DR-46, DR-54, DR-62). The first problem fails the load with `pack_invalid`, naming the
//! file, the line of the offending key or value, and the reference.
//!
//! Every place a pack names something goes through a table the contract declares: a question
//! step's and a candidate query's parameters through their shape's ([`Shape::params`]), a rule's
//! through its rule type's ([`RuleType::params`]), a decision effect's through the effect's
//! ([`EffectOp::params`]) and a `when` predicate's through [`PREDICATE_PARAMS`]. A key the table
//! doesn't declare is refused, at any depth, a value form no check reads is refused, and each
//! name a reference parameter holds is checked against the pack's scope. A suggestion's action is
//! parsed by the CLI's own clap tree, and every lens and node ID its arguments name is checked
//! the same way.
//!
//! A `$` placeholder stands in only where the docs allow one: a question's argument for a node
//! ID in a step, `$pick` for a node ID in an effect, and an emit template's captures. A kind,
//! relation or lens is always spelled out, and a literal ID's kind-short is checked even when
//! its namespace or key holds a placeholder.
//!
//! A pack's ring is its origin, where it was found, never the ID its manifest claims: a pack found
//! on a path is out-of-tree, and one that claims a built-in pack's ID is refused.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use planroom_contract::{
    ActionDecl, ArgDecl, Class, Command, DecisionKind, EffectOp, EmitTemplate, ExtractorDecl, Level,
    PREDICATE_PARAMS, Param, ParamNames, Predicate, Question, RuleType, Shape, SubjectKind, Undo,
    WriteCommon,
};
use planroom_extract::declarative::{
    Declarative, Fault, Form, Spec, Step, TEMPLATE_MAX_BYTES, TEMPLATE_MAX_VARS, Why, fault, grammar,
};
use planroom_extract::registry::{self, Role};
use serde_json::Value;

use super::load::{At, KeyPath, Source, extractor_id};
use super::{BASE, FORMAT, KERNEL, Origin, Pack, qualify};
use crate::app::error::AppError;
use crate::text;

/// The kernel's own kinds and its one relation, which every pack may reference (DR-45).
const KERNEL_KINDS: &[&str] = &["group", "external", "project", "item"];
const KERNEL_RELATIONS: &[&str] = &["touches"];
/// The kernel's ID prefix for every Container-level kind, so a reclassified container keeps its
/// ID (DR-46).
const CONTAINER_PREFIX: &str = "container";
/// Prefixes the kernel's own IDs use, which no pack may claim.
const KERNEL_PREFIXES: &[&str] = &["group"];
/// Questions the engine serves itself, which no pack declares (DR-15).
const BUILT_IN_QUESTIONS: &[&str] = &["kinds", "stage", "lint", "missing", "drift"];
/// The one pack that declares stages.
const PLAN: &str = "plan";
/// The namespace of plan items, `plan:<project>:<key>`, whose middle part names a project rather
/// than a kind-short.
const PLAN_NAMESPACE: &str = "plan";
/// The placeholder an effect's node-ID parameter takes for the chosen candidate (DR-63).
const PICK: &str = "$pick";
/// `map --lens all` lifts every relation kind; it names no lens.
const ALL_LENSES: &str = "all";

type Vars<'a> = [(&'a str, &'a dyn std::fmt::Display)];

/// Check the whole set, in load order.
pub fn check(packs: &[Pack]) -> Result<(), AppError> {
    let ids = headers(packs)?;
    let names = Names::of(packs)?;
    let root = KeyPath::root();
    let mut prefixes: BTreeMap<&str, &str> = KERNEL_PREFIXES.iter().map(|p| (*p, KERNEL)).collect();
    let mut extensions: BTreeMap<String, &str> = BTreeMap::new();
    let mut language_ids: BTreeMap<&str, &str> = BTreeMap::new();
    let mut globs: BTreeMap<&str, &str> = BTreeMap::new();
    let mut lenses: BTreeMap<&str, &str> = BTreeMap::new();
    let mut questions: BTreeMap<&str, &str> = BTreeMap::new();
    // Each default lens: its pack, its ID, its index, and whether that pack is the built-in code
    // pack.
    let mut defaults: Vec<(&str, &str, usize, bool)> = Vec::new();
    let linkers: BTreeSet<&str> =
        packs.iter().flat_map(|p| p.file.linkers.iter().map(|l| l.builtin.as_str())).collect();
    for pack in packs {
        let ctx = Ctx { pack, names: &names, scope: scope(pack) };
        let src = &pack.source;
        let id = pack.id();
        requires(pack, &ids)?;

        for (i, kind) in pack.file.kinds.iter().enumerate() {
            let at = root.key("kinds").at(i);
            let q = qualify(id, &kind.id);
            let container = kind.level == Level::Container;
            let prefix_at = at.key("id_prefix");
            if container != (kind.id_prefix == CONTAINER_PREFIX) {
                let why = if container { "prefix_container" } else { "prefix_reserved" };
                return Err(ctx.bad(&prefix_at, &kind.id_prefix, why, &[("kind", &q)]));
            }
            if !container {
                syntax(src, &prefix_at, &kind.id_prefix)?;
                match prefixes.get(kind.id_prefix.as_str()) {
                    Some(owner) if *owner != id => {
                        let why = if *owner == KERNEL { "prefix_kernel" } else { "prefix_owned" };
                        let vars: &Vars<'_> = &[("kind", &q), ("prefix", &kind.id_prefix), ("owner", owner)];
                        return Err(ctx.bad(&prefix_at, &kind.id_prefix, why, vars));
                    }
                    _ => {
                        prefixes.insert(&kind.id_prefix, id);
                    }
                }
            }
            for (j, parent) in kind.parents.iter().enumerate() {
                ctx.kind(parent, &at.key("parents").at(j))?;
            }
            prop_schema(&ctx, &at.key("props"), &q, &kind.props)?;
        }
        for (i, relation) in pack.file.relations.iter().enumerate() {
            let at = root.key("relations").at(i);
            for (end, ends) in [("from", &relation.from), ("to", &relation.to)] {
                for (j, kind) in ends.iter().enumerate() {
                    ctx.kind(kind, &at.key(end).at(j))?;
                }
            }
            prop_schema(&ctx, &at.key("props"), &qualify(id, &relation.id), &relation.props)?;
        }
        for (i, language) in pack.file.languages.iter().enumerate() {
            let at = root.key("languages").at(i);
            syntax(src, &at.key("id"), &language.id)?;
            if let Some(other) = language_ids.insert(&language.id, id) {
                return Err(claimed(&ctx, &at.key("id"), &language.id, "languages", other));
            }
            for (j, ext) in language.extensions.iter().enumerate() {
                let ext = ext.trim_start_matches('.').to_owned();
                if let Some(other) = extensions.insert(ext.clone(), id) {
                    return Err(claimed(&ctx, &at.key("extensions").at(j), &ext, "languages", other));
                }
            }
        }
        for (i, role) in pack.file.image_roles.iter().enumerate() {
            let at = root.key("image_roles").at(i);
            glob(&ctx, &at.key("image"), &role.image)?;
            if let Some(other) = globs.insert(&role.image, id) {
                return Err(claimed(&ctx, &at.key("image"), &role.image, "image_roles", other));
            }
            let level = ctx.kind(&role.kind, &at.key("kind"))?;
            if level != Some(Level::Container) {
                let vars: &Vars<'_> = &[("image", &role.image), ("kind", &role.kind)];
                return Err(ctx.bad(&at.key("kind"), &role.kind, "image_kind", vars));
            }
        }
        for (i, hint) in pack.file.hints.iter().enumerate() {
            let q = qualify(id, &hint.id);
            // A built-in pack's hint kinds are joined by the built-in linkers that land with
            // them; an out-of-tree pack can add no linker, so a hint kind of its own that no
            // loaded linker joins would join nothing (DR-9).
            if pack.origin == Origin::OutOfTree
                && !registry::LINKERS
                    .iter()
                    .any(|l| linkers.contains(l.builtin) && l.consumes.contains(&q.as_str()))
            {
                let at = root.key("hints").at(i).key("id");
                return Err(ctx.bad(&at, &hint.id, "hint_unconsumed", &[("hint", &q)]));
            }
        }
        for (i, decl) in pack.file.extractors.iter().enumerate() {
            extractor(&ctx, decl, &root.key("extractors").at(i), &language_ids)?;
        }
        for (i, linker) in pack.file.linkers.iter().enumerate() {
            let at = root.key("linkers").at(i).key("builtin");
            let name = format!("{id}/{}", linker.builtin);
            match registry::find(&linker.builtin) {
                Some(b) if b.role == Role::Linker && b.name == name => {}
                Some(b) if b.role == Role::Linker => {
                    let vars: &Vars<'_> = &[("builtin", &linker.builtin), ("name", &b.name)];
                    return Err(ctx.bad(&at, &linker.builtin, "builtin_name", vars));
                }
                _ => {
                    let vars: &Vars<'_> = &[("builtin", &linker.builtin)];
                    return Err(ctx.bad(&at, &linker.builtin, "unknown_linker", vars));
                }
            }
        }
        for (i, lens) in pack.file.lenses.iter().enumerate() {
            let at = root.key("lenses").at(i);
            syntax(src, &at.key("id"), &lens.id)?;
            // `--lens all` means every relation of every loaded pack, so no lens may take its name.
            if lens.id == ALL_LENSES {
                return Err(src.invalid_at(&at.key("id"), At::Value, &lens.id, "lens_all", &[]));
            }
            if let Some(other) = lenses.insert(&lens.id, id) {
                return Err(claimed(&ctx, &at.key("id"), &lens.id, "lenses", other));
            }
            for (j, relation) in lens.relations.iter().enumerate() {
                ctx.relation(relation, &at.key("relations").at(j))?;
            }
            for relation in lens.flow.keys() {
                if !lens.relations.contains(relation) {
                    let vars: &Vars<'_> = &[("lens", &lens.id), ("relation", relation)];
                    let flow = at.key("flow").key(relation);
                    return Err(src.invalid_at(&flow, At::Key, relation, "lens_flow", vars));
                }
            }
            for (j, kind) in lens.kinds.iter().enumerate() {
                ctx.kind(kind, &at.key("kinds").at(j))?;
            }
            if lens.default {
                defaults.push((id, &lens.id, i, is_base(pack)));
            }
        }
        for (i, question) in pack.file.questions.iter().enumerate() {
            let at = root.key("questions").at(i);
            bounded(&pack.source, &at.key("id"), &question.id)?;
            if BUILT_IN_QUESTIONS.contains(&question.id.as_str()) {
                return Err(ctx.bad(
                    &at.key("id"),
                    &question.id,
                    "question_builtin",
                    &[("id", &question.id)],
                ));
            }
            if let Some(other) = questions.insert(&question.id, id) {
                return Err(claimed(&ctx, &at.key("id"), &question.id, "questions", other));
            }
            if !(1..=3).contains(&question.steps.len()) {
                let vars: &Vars<'_> = &[("id", &question.id), ("count", &question.steps.len())];
                return Err(ctx.bad(&at.key("steps"), &question.id, "steps", vars));
            }
            for (j, step) in question.steps.iter().enumerate() {
                let params = at.key("steps").at(j).key("params");
                ctx.shape(step.shape, &step.params, &params, Use::step(&question.args))?;
            }
        }
        for (i, rule) in pack.file.rules.iter().enumerate() {
            let at = root.key("rules").at(i);
            ctx.kind(&rule.subject, &at.key("subject"))?;
            let owner = text::get("pack.owner_rule", &[("name", &serde_name(&rule.kind))]);
            ctx.params(&owner, RuleType::params(rule.kind), &rule.params, &at.key("params"), Use::plain())?;
        }
        for (i, decision) in pack.file.decisions.iter().enumerate() {
            ctx.decision(decision, &root.key("decisions").at(i))?;
        }
        for (i, suggestion) in pack.file.suggestions.iter().enumerate() {
            let at = root.key("suggestions").at(i);
            // A suggestion has no subject kind of its own, so its kinds may be either kind.
            ctx.predicate(&suggestion.when, &at.key("when"), Subject::Either)?;
            ctx.action(&suggestion.action, &at.key("action"))?;
        }
        if !pack.file.stages.is_empty() && !(id == PLAN && pack.origin == Origin::BuiltIn) {
            let first = &pack.file.stages[0].id;
            return Err(ctx.bad(&root.key("stages").at(0).key("id"), first, "stages", &[]));
        }
        for (i, prompt) in pack.file.prompts.iter().enumerate() {
            if !holds(src, "prompts", &prompt.file) {
                let at = root.key("prompts").at(i).key("file");
                return Err(ctx.bad(&at, &prompt.file, "file_missing", &[("reference", &prompt.file)]));
            }
        }
    }
    // Two defaults load only when one of them is the code pack's, which stays the default.
    let of = |base: bool| -> Vec<(&str, &str, usize)> {
        defaults.iter().filter(|(.., b)| *b == base).map(|(pack, lens, i, _)| (*pack, *lens, *i)).collect()
    };
    let (base, others) = (of(true), of(false));
    let clash =
        if base.len() > 1 { Some((base[0], base[1])) } else { others.get(1).map(|o| (others[0], *o)) };
    if let Some(((_, first, _), (pack, second, i))) = clash {
        let src = &packs.iter().find(|p| p.id() == pack).expect("a pack that declared a lens").source;
        let at = root.key("lenses").at(i).key("id");
        let vars: &Vars<'_> = &[("first", &first), ("second", &second)];
        return Err(src.invalid_at(&at, At::Value, second, "default_lens", vars));
    }
    Ok(())
}

/// Whether `pack` is the built-in code pack, the base: by its origin as well as its ID, so a pack
/// found on a path never takes the base's place.
pub(super) fn is_base(pack: &Pack) -> bool {
    pack.id() == BASE && pack.origin == Origin::BuiltIn
}

/// Each manifest's format and ID, no ID twice, and no out-of-tree pack under a built-in pack's
/// ID: every loaded pack's ID and its origin.
fn headers(packs: &[Pack]) -> Result<BTreeMap<&str, Origin>, AppError> {
    let builtins: BTreeSet<&str> =
        packs.iter().filter(|p| p.origin == Origin::BuiltIn).map(Pack::id).collect();
    let mut ids = BTreeMap::new();
    let root = KeyPath::root();
    for pack in packs {
        let src = &pack.source;
        let format = &pack.file.format;
        if format != FORMAT {
            return Err(src.invalid_at(
                &root.key("format"),
                At::Value,
                format,
                "format",
                &[("found", format)],
            ));
        }
        let at = root.key("id");
        syntax(src, &at, pack.id())?;
        if pack.origin == Origin::OutOfTree && (builtins.contains(pack.id()) || pack.id() == BASE) {
            return Err(src.invalid_at(&at, At::Value, pack.id(), "builtin_id", &[("id", &pack.id())]));
        }
        if pack.id() == KERNEL || ids.insert(pack.id(), pack.origin).is_some() {
            return Err(src.invalid_at(&at, At::Value, pack.id(), "duplicate_pack", &[("id", &pack.id())]));
        }
    }
    Ok(ids)
}

/// Requires, inward only, by origin: a built-in pack requires `code` or nothing, and an
/// out-of-tree pack any built-in pack but no out-of-tree one (DR-9, DR-62).
fn requires(pack: &Pack, ids: &BTreeMap<&str, Origin>) -> Result<(), AppError> {
    let src = &pack.source;
    for (j, required) in pack.file.requires.iter().enumerate() {
        let why = match (pack.origin, ids.get(required.as_str())) {
            (Origin::BuiltIn, _) if required != BASE || pack.id() == BASE => "requires_builtin",
            (_, None) => "requires_missing",
            (Origin::OutOfTree, Some(Origin::OutOfTree)) => "requires_out_of_tree",
            _ => continue,
        };
        let at = KeyPath::root().key("requires").at(j);
        return Err(src.invalid_at(&at, At::Value, required, why, &[("required", required)]));
    }
    Ok(())
}

/// Every kind, relation and hint kind the set declares, qualified, with a kind's level, each
/// lens ID with the pack that first declares it, and each ID prefix but the kernel's with the
/// packs whose kinds declare it.
struct Names {
    kinds: BTreeMap<String, Level>,
    relations: BTreeSet<String>,
    hints: BTreeSet<String>,
    lenses: BTreeMap<String, String>,
    prefixes: BTreeMap<String, BTreeSet<String>>,
}

impl Names {
    fn of(packs: &[Pack]) -> Result<Names, AppError> {
        let mut names = Names {
            kinds: BTreeMap::new(),
            relations: BTreeSet::new(),
            hints: BTreeSet::new(),
            lenses: BTreeMap::new(),
            prefixes: BTreeMap::new(),
        };
        let root = KeyPath::root();
        for pack in packs {
            let src = &pack.source;
            let id = pack.id();
            let twice = |at: KeyPath, table: &str, decl: &str| {
                let vars: &Vars<'_> = &[("table", &table), ("id", &decl)];
                src.invalid_at(&at, At::Value, decl, "duplicate_decl", vars)
            };
            for (i, kind) in pack.file.kinds.iter().enumerate() {
                let at = root.key("kinds").at(i).key("id");
                syntax(src, &at, &kind.id)?;
                if names.kinds.insert(qualify(id, &kind.id), kind.level).is_some() {
                    return Err(twice(at, "kinds", &kind.id));
                }
                if kind.id_prefix != CONTAINER_PREFIX {
                    names.prefixes.entry(kind.id_prefix.clone()).or_default().insert(id.to_owned());
                }
            }
            for (i, relation) in pack.file.relations.iter().enumerate() {
                let at = root.key("relations").at(i).key("id");
                syntax(src, &at, &relation.id)?;
                if !names.relations.insert(qualify(id, &relation.id)) {
                    return Err(twice(at, "relations", &relation.id));
                }
            }
            for (i, hint) in pack.file.hints.iter().enumerate() {
                let at = root.key("hints").at(i).key("id");
                syntax(src, &at, &hint.id)?;
                if !names.hints.insert(qualify(id, &hint.id)) {
                    return Err(twice(at, "hints", &hint.id));
                }
            }
            // A lens ID is unqualified; a second pack that declares one is refused where the
            // merged tables are checked.
            for lens in &pack.file.lenses {
                names.lenses.entry(lens.id.clone()).or_insert_with(|| id.to_owned());
            }
            let mut seen = BTreeSet::new();
            for (i, decl) in pack.file.extractors.iter().enumerate() {
                let at = root.key("extractors").at(i).key("id");
                let eid = extractor_id(decl);
                syntax(src, &at, eid)?;
                if !seen.insert(eid) {
                    return Err(twice(at, "extractors", eid));
                }
            }
        }
        Ok(names)
    }
}

/// The packs one pack may reference: its own, those it requires, and the kernel.
fn scope(pack: &Pack) -> BTreeSet<String> {
    let mut scope: BTreeSet<String> = pack.file.requires.iter().cloned().collect();
    scope.insert(pack.id().to_owned());
    scope.insert(KERNEL.to_owned());
    scope
}

/// Which placeholder a node-ID parameter takes where it stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Placeholder<'a> {
    /// None: an ID is spelled out.
    Never,
    /// One of the question's declared arguments, such as `$X`, in a step.
    Argument(&'a [ArgDecl]),
    /// `$pick`, the chosen candidate, in an effect.
    Pick,
    /// An emit template's captures, such as `$repo` or `$1`, around a literal kind-short.
    Capture,
}

/// What a `when` predicate selects, which decides what its kinds name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Subject {
    /// A node kind's subjects: its kinds are node kinds.
    Node,
    /// A relation's subjects: its kinds are relations.
    Edge,
    /// A role's subjects, or a suggestion's: either.
    Either,
}

/// How one table of parameters reads where it stands.
#[derive(Debug, Clone, Copy)]
struct Use<'a> {
    ids: Placeholder<'a>,
    subject: Subject,
}

impl<'a> Use<'a> {
    fn plain() -> Use<'a> {
        Use { ids: Placeholder::Never, subject: Subject::Either }
    }

    fn step(args: &'a [ArgDecl]) -> Use<'a> {
        Use { ids: Placeholder::Argument(args), subject: Subject::Either }
    }
}

/// One pack's view of the set while its references are checked.
struct Ctx<'a> {
    pack: &'a Pack,
    names: &'a Names,
    scope: BTreeSet<String>,
}

impl Ctx<'_> {
    /// `pack_invalid` at the value `at` names.
    fn bad(&self, at: &KeyPath, reference: &str, why: &str, vars: &Vars<'_>) -> AppError {
        self.pack.source.invalid_at(at, At::Value, reference, why, vars)
    }

    /// The namespace of a qualified reference this pack may make, or why it may not. A kind,
    /// relation or lens is always spelled out, never a placeholder.
    fn visible<'r>(&self, reference: &'r str, at: &KeyPath) -> Result<(&'r str, &'r str), AppError> {
        if reference.contains('$') {
            return Err(self.bad(at, reference, "placeholder", &[("reference", &reference)]));
        }
        let Some((ns, name)) = reference.split_once('.') else {
            return Err(self.bad(at, reference, "not_qualified", &[("reference", &reference)]));
        };
        if !self.scope.contains(ns) {
            return Err(self.bad(at, reference, "out_of_scope", &[("reference", &reference), ("pack", &ns)]));
        }
        Ok((ns, name))
    }

    /// A kind reference's level; the kernel's kinds have none of their own.
    fn kind(&self, reference: &str, at: &KeyPath) -> Result<Option<Level>, AppError> {
        let (ns, name) = self.visible(reference, at)?;
        if ns == KERNEL && KERNEL_KINDS.contains(&name) {
            return Ok(None);
        }
        match self.names.kinds.get(reference) {
            Some(level) => Ok(Some(*level)),
            None => Err(self.bad(at, reference, "unknown_kind", &[("reference", &reference)])),
        }
    }

    fn relation(&self, reference: &str, at: &KeyPath) -> Result<(), AppError> {
        let (ns, name) = self.visible(reference, at)?;
        if (ns == KERNEL && KERNEL_RELATIONS.contains(&name)) || self.names.relations.contains(reference) {
            return Ok(());
        }
        Err(self.bad(at, reference, "unknown_relation", &[("reference", &reference)]))
    }

    /// A lens, by its unqualified ID: one a pack in this pack's scope declares.
    fn lens(&self, reference: &str, at: &KeyPath) -> Result<(), AppError> {
        if reference.contains('$') {
            return Err(self.bad(at, reference, "placeholder", &[("reference", &reference)]));
        }
        match self.names.lenses.get(reference) {
            Some(owner) if self.scope.contains(owner) => Ok(()),
            Some(owner) => {
                Err(self.bad(at, reference, "out_of_scope", &[("reference", &reference), ("pack", owner)]))
            }
            None => Err(self.bad(at, reference, "unknown_lens", &[("reference", &reference)])),
        }
    }

    /// A decision kind's subject, a node kind or an edge kind: which of the two it is.
    fn subject(&self, reference: &str, at: &KeyPath) -> Result<Subject, AppError> {
        let (ns, name) = self.visible(reference, at)?;
        if (ns == KERNEL && KERNEL_KINDS.contains(&name)) || self.names.kinds.contains_key(reference) {
            return Ok(Subject::Node);
        }
        if (ns == KERNEL && KERNEL_RELATIONS.contains(&name)) || self.names.relations.contains(reference) {
            return Ok(Subject::Edge);
        }
        Err(self.bad(at, reference, "unknown_subject", &[("reference", &reference)]))
    }

    /// Everything a decision kind names: its subject, its `when` precheck, its slice's relations,
    /// its candidates' query and its effect, each through its own table (DR-62, DR-63).
    fn decision(&self, decision: &DecisionKind, at: &KeyPath) -> Result<(), AppError> {
        let subject = match &decision.subject {
            SubjectKind::Kind(subject) => self.subject(subject, &at.key("subject"))?,
            SubjectKind::Role { .. } => Subject::Either,
        };
        self.predicate(&decision.when, &at.key("when"), subject)?;
        for (j, relation) in decision.slice.relations.iter().enumerate() {
            self.relation(relation, &at.key("slice").key("relations").at(j))?;
        }
        if let Question::Choice { options_from: Some(from), .. } = &decision.question {
            let params = at.key("question").key("options_from").key("params");
            self.shape(from.shape, &from.params, &params, Use::plain())?;
        }
        let effect = &decision.effect;
        let owner = text::get("pack.owner_effect", &[("name", &serde_name(&effect.action))]);
        let params = Value::Object(effect.params.clone());
        let cx = Use { ids: Placeholder::Pick, subject };
        self.params(&owner, EffectOp::params(effect.action), &params, &at.key("effect"), cx)
    }

    /// A `when` predicate, a decision kind's or a suggestion's, through the predicate's table.
    fn predicate(&self, when: &Predicate, at: &KeyPath, subject: Subject) -> Result<(), AppError> {
        let owner = text::get("pack.owner_predicate", &[]);
        let when = Value::Object(when.iter().map(|(k, v)| (k.clone(), v.clone())).collect());
        self.params(&owner, PREDICATE_PARAMS, &when, at, Use { ids: Placeholder::Never, subject })
    }

    /// A suggestion's action: a leaf that can be undone, doesn't hand off and never spends
    /// (DR-54), whose arguments parse as the CLI parses them, and every lens and node ID they name
    /// in scope.
    fn action(&self, action: &ActionDecl, at: &KeyPath) -> Result<(), AppError> {
        let command = &action.command.0;
        let allowed = Command::SPECS.iter().find(|s| s.name() == *command).is_some_and(|s| {
            !matches!(s.undo, Undo::NotUndoable(_)) && s.class != Class::Handoff && !s.spends
        });
        if !allowed {
            return Err(self.bad(&at.key("command"), command, "suggestion", &[("command", command)]));
        }
        let args = at.key("args");
        // The argument a parsed value came from: `--lens X`'s X, or `--lens=X`.
        let arg_at = |value: &str| {
            let given = action.args.iter().position(|a| a == value || a.ends_with(&format!("={value}")));
            given.map_or_else(|| args.clone(), |j| args.at(j))
        };
        let leaf = crate::cli::parse_leaf(command, &action.args).map_err(|e| {
            let rendered = e.render().to_string();
            let why = rendered.lines().next().unwrap_or_default();
            let why = why.strip_prefix("error: ").unwrap_or(why);
            let named = match e.get(clap::error::ContextKind::InvalidArg) {
                Some(clap::error::ContextValue::String(arg)) => {
                    arg.split([' ', '=']).next().map(str::to_owned)
                }
                _ => None,
            };
            let at = named
                .and_then(|flag| {
                    action.args.iter().position(|a| a == &flag || a.starts_with(&format!("{flag}=")))
                })
                .map_or_else(|| args.clone(), |j| args.at(j));
            let shown = std::iter::once(command.as_str()).chain(action.args.iter().map(String::as_str));
            let shown = shown.collect::<Vec<_>>().join(" ");
            self.bad(&at, command, "action_args", &[("command", &shown), ("why", &why)])
        })?;
        let lens = |lens: &Option<String>| match lens.as_deref() {
            Some(lens) if lens != ALL_LENSES => self.lens(lens, &arg_at(lens)),
            _ => Ok(()),
        };
        let project = |common: &WriteCommon| match &common.project {
            Some(id) => self.node_id(&id.0, &arg_at(&id.0), Placeholder::Never),
            None => Ok(()),
        };
        // Each leaf says what its arguments name, so a new leaf must decide.
        match &leaf {
            Command::Map(req) => {
                lens(&req.lens)?;
                // A target is an ID, an ID suffix or a name: only a whole ID names a kind-short.
                if let Some(focus) = &req.focus
                    && (focus.starts_with('$') || focus.matches(':').count() >= 2)
                {
                    self.node_id(focus, &arg_at(focus), Placeholder::Never)?;
                }
            }
            Command::Scan(req) => project(&req.common)?,
            Command::PersonInit(req) => project(&req.common)?,
            // Its targets are IDs, ID suffixes or names, resolved when it runs, as `map`'s focus is.
            Command::Edit(req) => project(&req.common)?,
            // Its item is an event ID, which no pack can name ahead of the event.
            Command::Review(req) => project(&req.common)?,
            // The text `find` searches for names no kind, relation, lens or ID.
            Command::Find(_)
            | Command::Ask(_)
            | Command::Focus(_)
            | Command::Watch(_)
            | Command::Schema(_)
            | Command::Version(_)
            | Command::Serve(_)
            | Command::Share(_)
            | Command::Import(_)
            | Command::Doctor(_)
            | Command::Open(_) => {}
        }
        Ok(())
    }

    fn hint(&self, reference: &str, at: &KeyPath) -> Result<(), AppError> {
        self.visible(reference, at)?;
        if self.names.hints.contains(reference) {
            return Ok(());
        }
        Err(self.bad(at, reference, "unknown_hint", &[("reference", &reference)]))
    }

    /// A reference in `from`, `to`, `parent` or `scope`: `@alias:...`, `@ext:NAME`, `@id:KIND:KEY`
    /// or an ID, whose namespace and key may hold captures. A bare value such as `$file` is none
    /// of them.
    fn reference(&self, reference: &str, at: &KeyPath) -> Result<(), AppError> {
        if reference.starts_with("@alias:") || reference.starts_with("@ext:") {
            return Ok(());
        }
        if let Some(rest) = reference.strip_prefix("@id:") {
            let kind = rest.split_once(':').map_or(rest, |(k, _)| k);
            return self.kind(kind, at).map(drop);
        }
        if !reference.starts_with('@') && reference.split(':').count() >= 3 {
            return self.node_id(reference, at, Placeholder::Capture);
        }
        Err(self.bad(at, reference, "bare_ref", &[("reference", &reference)]))
    }

    /// `name`, an argument the ID `id` names, is one its question declares in `args`.
    fn declared(&self, id: &str, name: &str, at: &KeyPath, args: &[ArgDecl]) -> Result<(), AppError> {
        if args.iter().any(|a| a.name == name) {
            return Ok(());
        }
        let declared: Vec<&str> = args.iter().map(|a| a.name.as_str()).collect();
        let declared = if declared.is_empty() { text::get("pack.no_args", &[]) } else { declared.join(", ") };
        let vars: &Vars<'_> = &[("reference", &id), ("name", &name), ("args", &declared)];
        Err(self.bad(at, id, "argument_undeclared", vars))
    }

    /// A node ID, `<namespace>:<kind-short>:<key>`: its kind-short is the `id_prefix` of a kind in
    /// this pack's scope, or the kernel's own, and is spelled out even where the namespace or the
    /// key holds a placeholder. A plan item's ID names a project there. A whole placeholder stands
    /// in only where `ids` allows one.
    fn node_id(&self, id: &str, at: &KeyPath, ids: Placeholder<'_>) -> Result<(), AppError> {
        if !id.contains(':') && id.starts_with('$') {
            let name = &id[1..];
            let argument = !name.is_empty() && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_');
            return match ids {
                Placeholder::Argument(args) if argument => self.declared(id, name, at, args),
                Placeholder::Pick if id == PICK => Ok(()),
                _ => Err(self.bad(at, id, "placeholder_id", &[("reference", &id)])),
            };
        }
        let mut parts = id.splitn(3, ':');
        let (Some(ns), Some(prefix), Some(key)) = (parts.next(), parts.next(), parts.next()) else {
            return Err(self.bad(at, id, "node_id", &[("reference", &id)]));
        };
        if ns.is_empty() || prefix.is_empty() || key.is_empty() {
            return Err(self.bad(at, id, "node_id", &[("reference", &id)]));
        }
        if prefix.contains(['$', '@']) {
            return Err(self.bad(at, id, "prefix_placeholder", &[("reference", &id)]));
        }
        let placed = |part: &str| match ids {
            Placeholder::Capture => false,
            Placeholder::Argument(_) => part.contains('@'),
            Placeholder::Never | Placeholder::Pick => part.contains(['$', '@']),
        };
        if placed(ns) || placed(key) {
            return Err(self.bad(at, id, "placeholder_id", &[("reference", &id)]));
        }
        if let Placeholder::Argument(args) = ids {
            for name in [ns, key].into_iter().flat_map(arguments) {
                self.declared(id, name, at, args)?;
            }
        }
        if ns == PLAN_NAMESPACE || prefix == CONTAINER_PREFIX || KERNEL_PREFIXES.contains(&prefix) {
            return Ok(());
        }
        match self.names.prefixes.get(prefix) {
            Some(owners) if owners.iter().any(|o| self.scope.contains(o)) => Ok(()),
            Some(owners) => {
                let owner = owners.iter().next().expect("a prefix some kind declares");
                let vars: &Vars<'_> = &[("reference", &id), ("prefix", &prefix), ("pack", owner)];
                Err(self.bad(at, id, "prefix_scope", vars))
            }
            None => Err(self.bad(at, id, "unknown_prefix", &[("reference", &id), ("prefix", &prefix)])),
        }
    }

    /// A query shape's parameters, through the table the shape declares.
    fn shape(&self, shape: Shape, params: &Value, at: &KeyPath, cx: Use<'_>) -> Result<(), AppError> {
        let owner = text::get("pack.owner_shape", &[("name", &serde_name(&shape))]);
        self.params(&owner, shape.params(), params, at, cx)
    }

    /// `params`, a table of `owner`'s parameters at `at`: each key one `table` declares, each
    /// value in a form the check reads, and each name a reference parameter holds in this pack's
    /// scope.
    fn params(
        &self,
        owner: &str,
        table: &[Param],
        params: &Value,
        at: &KeyPath,
        cx: Use<'_>,
    ) -> Result<(), AppError> {
        let src = &self.pack.source;
        let map = match params {
            Value::Null => return Ok(()),
            Value::Object(map) => map,
            _ => return Err(src.invalid_at(at, At::Value, owner, "params_table", &[("owner", &owner)])),
        };
        for (key, value) in map {
            let here = at.key(key);
            let Some(param) = table.iter().find(|p| declares(p.key, key)) else {
                let keys = table.iter().map(|p| p.key).collect::<Vec<_>>().join(", ");
                let keys = if keys.is_empty() { text::get("pack.no_params", &[]) } else { keys };
                let vars: &Vars<'_> = &[("key", key), ("owner", &owner), ("keys", &keys)];
                return Err(src.invalid_at(&here, At::Key, key, "param_unknown", vars));
            };
            match param.names {
                ParamNames::Nothing => plain(self, key, value, &here)?,
                ParamNames::Test => test(self, key, value, &here)?,
                ParamNames::Table(inner) => {
                    if !value.is_object() {
                        return Err(self.bad(&here, key, "params_table", &[("owner", key)]));
                    }
                    self.params(key, inner, value, &here, cx)?;
                }
                names => {
                    let Some(items) = names_of(value, &here) else {
                        return Err(self.bad(&here, key, "param_value", &[("key", key)]));
                    };
                    for (name, item) in items {
                        self.named(names, key, name, &item, cx)?;
                    }
                }
            }
        }
        Ok(())
    }

    /// One name a reference parameter `key` holds, by what the parameter names.
    fn named(
        &self,
        names: ParamNames,
        key: &str,
        name: &str,
        at: &KeyPath,
        cx: Use<'_>,
    ) -> Result<(), AppError> {
        match (names, cx.subject) {
            (ParamNames::NodeId, _) => self.node_id(name, at, cx.ids),
            (ParamNames::NodeKind, _) | (ParamNames::SubjectKind, Subject::Node) => {
                self.node_kind(key, name, at)
            }
            (ParamNames::Relation, _) | (ParamNames::SubjectKind, Subject::Edge) => self.relation(name, at),
            (ParamNames::SubjectKind, Subject::Either) => self.subject(name, at).map(drop),
            (ParamNames::Lens, _) => self.lens(name, at),
            (ParamNames::Nothing | ParamNames::Test | ParamNames::Table(_), _) => Ok(()),
        }
    }

    /// A node kind where parameter `key` takes one: a relation there is refused by name, since
    /// the shape would read it as a kind no node has.
    fn node_kind(&self, key: &str, reference: &str, at: &KeyPath) -> Result<(), AppError> {
        let (ns, name) = self.visible(reference, at)?;
        if (ns == KERNEL && KERNEL_RELATIONS.contains(&name)) || self.names.relations.contains(reference) {
            let vars: &Vars<'_> = &[("reference", &reference), ("key", &key)];
            return Err(self.bad(at, reference, "kind_is_relation", vars));
        }
        self.kind(reference, at).map(drop)
    }
}

/// A field test: a value, or a table of one `lt` or `gt` with a number, or of `in` with a list
/// of values. Any other form, such as `{ eq = 1 }` or an `in` that isn't a list, is refused, never
/// skipped.
fn test(ctx: &Ctx<'_>, key: &str, value: &Value, at: &KeyPath) -> Result<(), AppError> {
    let scalar = |v: &Value| matches!(v, Value::String(_) | Value::Number(_) | Value::Bool(_));
    let ok = match value {
        Value::Object(map) if map.len() == 1 => match map.iter().next() {
            Some((op, Value::Number(_))) => op == "lt" || op == "gt",
            Some((op, Value::Array(items))) => op == "in" && items.iter().all(scalar),
            _ => false,
        },
        other => scalar(other),
    };
    if ok { Ok(()) } else { Err(ctx.bad(at, key, "param_test", &[("key", &key)])) }
}

/// A parameter that names nothing: a number, a word or text, or a list of them, which is all such
/// a key reads. A table, or a list that holds one, is refused, never skipped, since nothing would
/// read its keys.
fn plain(ctx: &Ctx<'_>, key: &str, value: &Value, at: &KeyPath) -> Result<(), AppError> {
    let scalar = |v: &Value| matches!(v, Value::String(_) | Value::Number(_) | Value::Bool(_));
    let ok = match value {
        Value::Array(items) => items.iter().all(scalar),
        other => scalar(other),
    };
    if ok { Ok(()) } else { Err(ctx.bad(at, key, "param_plain", &[("key", &key)])) }
}

/// The argument names a part of a node ID holds, each after a `$`, such as `X` in `web:file:$X`.
fn arguments(part: &str) -> Vec<&str> {
    part.split('$')
        .skip(1)
        .map(|after| {
            let len = after.find(|c: char| !(c.is_ascii_alphanumeric() || c == '_')).unwrap_or(after.len());
            &after[..len]
        })
        .collect()
}

/// Whether the declared key `declared` covers `key`: the same key, or, for one that ends in
/// `.*`, a key with that prefix and a name after it.
fn declares(declared: &str, key: &str) -> bool {
    match declared.strip_suffix('*') {
        Some(prefix) => key.strip_prefix(prefix).is_some_and(|name| !name.is_empty()),
        None => declared == key,
    }
}

/// The names a reference parameter at `at` holds, each with its own path: a string, a list of
/// strings, or an `in` list of strings, as the filter shape's predicate form writes a set. Any
/// other form holds no names a check reads, so it's refused.
fn names_of<'v>(value: &'v Value, at: &KeyPath) -> Option<Vec<(&'v str, KeyPath)>> {
    fn list<'v>(items: &'v [Value], at: &KeyPath) -> Option<Vec<(&'v str, KeyPath)>> {
        items.iter().enumerate().map(|(i, v)| Some((v.as_str()?, at.at(i)))).collect()
    }
    match value {
        Value::String(s) => Some(vec![(s.as_str(), at.clone())]),
        Value::Array(items) => list(items, at),
        Value::Object(map) if map.len() == 1 => list(map.get("in")?.as_array()?, &at.key("in")),
        _ => None,
    }
}

/// A shape's, a rule type's or an effect's name as a pack spells it.
fn serde_name<T: serde::Serialize>(value: &T) -> String {
    serde_json::to_value(value).ok().and_then(|v| v.as_str().map(str::to_owned)).unwrap_or_default()
}

/// One extractor declaration at `at`: a built-in this build has, under its own pack-qualified
/// name, or a declarative one over a loaded language, with globs that parse and emits that
/// resolve.
fn extractor(
    ctx: &Ctx<'_>,
    decl: &ExtractorDecl,
    at: &KeyPath,
    languages: &BTreeMap<&str, &str>,
) -> Result<(), AppError> {
    let src = &ctx.pack.source;
    let id = ctx.pack.id();
    let (files, emit) = match decl {
        ExtractorDecl::Builtin { id: eid, builtin, files, options } => {
            let name = format!("{id}/{eid}");
            let builtin_at = at.key("builtin");
            let entry = match registry::find(builtin) {
                Some(b) if b.role == Role::Extractor && b.name == name => b,
                Some(b) if b.role == Role::Extractor => {
                    let vars: &Vars<'_> = &[("builtin", builtin), ("name", &b.name)];
                    return Err(ctx.bad(&builtin_at, builtin, "builtin_name", vars));
                }
                _ => return Err(ctx.bad(&builtin_at, builtin, "unknown_extractor", &[("builtin", builtin)])),
            };
            builtin_options(ctx, entry.options, eid, &name, options, &at.key("options"))?;
            (files, &[][..])
        }
        ExtractorDecl::Regex { id: eid, files, regex, emit } => {
            // Compiled here as the declarative runner compiles it, so a pattern that loads runs.
            if let Err(why) = planroom_extract::check_regex(regex) {
                let name = format!("{id}/{eid}");
                let vars: &Vars<'_> = &[("name", &name), ("why", &why)];
                return Err(ctx.bad(&at.key("regex"), regex, "regex", vars));
            }
            (files, emit.as_slice())
        }
        ExtractorDecl::Query { files, language, query, emit, .. } => {
            if !languages.contains_key(language.as_str()) {
                return Err(ctx.bad(
                    &at.key("language"),
                    language,
                    "unknown_language",
                    &[("reference", language)],
                ));
            }
            if !holds(src, "queries", query) {
                return Err(ctx.bad(&at.key("query"), query, "file_missing", &[("reference", query)]));
            }
            (files, emit.as_slice())
        }
    };
    for (j, g) in files.iter().enumerate() {
        glob(ctx, &at.key("files").at(j), g)?;
    }
    for (k, template) in emit.iter().enumerate() {
        let at = at.key("emit").at(k);
        match template {
            EmitTemplate::Node { node, parent, .. } => {
                ctx.kind(node, &at.key("node"))?;
                if let Some(parent) = parent {
                    ctx.reference(parent, &at.key("parent"))?;
                }
            }
            EmitTemplate::Edge { edge, from, to, .. } => {
                ctx.relation(edge, &at.key("edge"))?;
                ctx.reference(from, &at.key("from"))?;
                ctx.reference(to, &at.key("to"))?;
            }
            EmitTemplate::Hint { hint, scope, .. } => {
                ctx.hint(hint, &at.key("hint"))?;
                ctx.reference(scope, &at.key("scope"))?;
            }
        }
    }
    Ok(())
}

/// The declarative extractors of the merged set, its tree-sitter query ones when `queries` and
/// its regex ones otherwise, compiled as the runner will run them: against the whole vocabulary,
/// the merged `[[languages]]` table and every loaded `[[hints]]` kind. So what only the runner
/// reads, such as a hint's `attrs` keys, a template's variables, a query's grammar and its `.scm`
/// text, is `pack_invalid` at its own line. The regex ones compile at load, so a pack whose regex
/// extractors can't run doesn't load. The query ones compile once in a process that extracts,
/// before it runs any ([`super::Loaded::programs`]): `scan`, `doctor --pack` and adding a pack,
/// before any file is read. The compiled extractors, by pack-qualified name, are what a scan
/// runs.
pub(super) fn runnable(
    loaded: &super::Loaded,
    queries: bool,
) -> Result<BTreeMap<String, Arc<Declarative>>, AppError> {
    let hints: BTreeMap<String, Vec<String>> = loaded
        .packs
        .iter()
        .flat_map(|p| p.file.hints.iter().map(move |h| (qualify(p.id(), &h.id), h.attrs.clone())))
        .collect();
    let mut programs = BTreeMap::new();
    for pack in &loaded.packs {
        for (i, decl) in pack.file.extractors.iter().enumerate() {
            let query = match decl {
                ExtractorDecl::Builtin { .. } => continue,
                ExtractorDecl::Regex { .. } if queries => continue,
                ExtractorDecl::Query { .. } if !queries => continue,
                ExtractorDecl::Regex { .. } => None,
                ExtractorDecl::Query { query, .. } => {
                    pack.source.text(query).or_else(|| pack.source.text(&format!("queries/{query}")))
                }
            };
            let spec = Spec {
                pack: pack.id(),
                decl,
                vocabulary: &loaded.vocabulary,
                languages: &loaded.languages,
                hints: &hints,
                query,
            };
            let invalid = match Declarative::compile(&spec) {
                Ok(program) => {
                    programs.insert(program.name().to_owned(), Arc::new(program));
                    continue;
                }
                Err(invalid) => invalid,
            };
            let mut path = KeyPath::root().key("extractors").at(i);
            for step in &invalid.at {
                path = match step {
                    Step::Key(k) => path.key(k),
                    Step::Index(n) => path.at(*n),
                };
            }
            let name = format!("{}/{}", pack.id(), extractor_id(decl));
            let reference = invalid.reference.as_str();
            let (why, vars): (&str, Vec<(&str, String)>) = match &invalid.why {
                Why::NotDeclarative => continue,
                Why::BareRef => ("bare_ref", vec![("reference", reference.into())]),
                Why::NotAnId => ("not_an_id", vec![("reference", reference.into())]),
                Why::UnknownKind(k) => ("unknown_kind", vec![("reference", k.clone())]),
                Why::UnknownRelation(r) => ("unknown_relation", vec![("reference", r.clone())]),
                Why::UnknownHint(h) => ("unknown_hint", vec![("reference", h.clone())]),
                Why::HintAttr { hint, declared } => (
                    "hint_attr",
                    vec![("attr", reference.into()), ("hint", hint.clone()), ("attrs", declared.join(", "))],
                ),
                Why::UnknownVariable(v) => ("unknown_variable", vec![("reference", v.clone())]),
                Why::TemplateTooLong => (
                    "template_too_long",
                    vec![
                        ("name", name.clone()),
                        ("bytes", TEMPLATE_MAX_BYTES.to_string()),
                        ("vars", TEMPLATE_MAX_VARS.to_string()),
                    ],
                ),
                Why::UnknownGroup { group, groups } => (
                    "unknown_group",
                    vec![
                        ("name", name.clone()),
                        ("groups", groups.to_string()),
                        ("group", group.to_string()),
                    ],
                ),
                Why::Regex(why) => ("regex", vec![("name", name.clone()), ("why", why.clone())]),
                Why::Query(why) => ("query", vec![("name", name.clone()), ("why", why.clone())]),
                Why::QueryMissing => ("file_missing", vec![("reference", reference.into())]),
                Why::UnknownLanguage(l) => ("unknown_language", vec![("reference", l.clone())]),
                Why::NoGrammar(l) => (
                    "no_grammar",
                    vec![
                        ("reference", l.clone()),
                        ("grammars", grammar::ids().collect::<Vec<_>>().join(", ")),
                    ],
                ),
                Why::Confidence => ("confidence", vec![("reference", reference.into())]),
                Why::PropType { kind, prop, types } => (
                    "prop_type",
                    vec![("kind", kind.clone()), ("prop", prop.clone()), ("types", types.clone())],
                ),
                Why::PropValue { kind, prop, keyword } => (
                    "prop_value",
                    vec![
                        ("reference", reference.into()),
                        ("name", name.clone()),
                        ("kind", kind.clone()),
                        ("prop", prop.clone()),
                        ("keyword", (*keyword).to_owned()),
                    ],
                ),
            };
            // An attribute is refused by its key, as a key no table declares is.
            let at = if matches!(invalid.why, Why::HintAttr { .. }) { At::Key } else { At::Value };
            let vars: Vec<(&str, &dyn std::fmt::Display)> =
                vars.iter().map(|(k, v)| (*k, v as &dyn std::fmt::Display)).collect();
            return Err(pack.source.invalid_at(&path, at, reference, why, &vars));
        }
    }
    Ok(programs)
}

/// A built-in extractor's `options`, read through the table its registry entry declares, as a
/// query shape's parameters are: a key the table doesn't declare is refused at its own line, a
/// `kinds` role the built-in doesn't emit too, and every role it emits names a kind in scope,
/// since it emits each (DR-46).
fn builtin_options(
    ctx: &Ctx<'_>,
    table: &[Param],
    eid: &str,
    name: &str,
    options: &Value,
    at: &KeyPath,
) -> Result<(), AppError> {
    let src = &ctx.pack.source;
    let empty = serde_json::Map::new();
    let map = match options {
        Value::Null => &empty,
        Value::Object(map) => map,
        _ => return Err(ctx.bad(at, eid, "options", &[("name", &name)])),
    };
    let listed = |params: &[Param]| params.iter().map(|p| p.key).collect::<Vec<_>>().join(", ");
    for key in map.keys() {
        if !table.iter().any(|p| p.key == key) {
            let vars: &Vars<'_> = &[("key", key), ("name", &name), ("keys", &listed(table))];
            return Err(src.invalid_at(&at.key(key), At::Key, key, "option_unknown", vars));
        }
    }
    for param in table {
        let ParamNames::Table(roles) = param.names else { continue };
        let here = at.key(param.key);
        let kinds = match map.get(param.key) {
            None => &empty,
            Some(Value::Object(kinds)) => kinds,
            Some(_) => return Err(ctx.bad(&here, eid, "options", &[("name", &name)])),
        };
        for (role, kind) in kinds {
            if !roles.iter().any(|r| r.key == role) {
                let vars: &Vars<'_> = &[("role", role), ("name", &name), ("roles", &listed(roles))];
                return Err(src.invalid_at(&here.key(role), At::Key, role, "role_unknown", vars));
            }
            let Some(kind) = kind.as_str() else {
                return Err(ctx.bad(&here.key(role), eid, "options", &[("name", &name)]));
            };
            ctx.kind(kind, &here.key(role))?;
        }
        if let Some(missing) = roles.iter().find(|r| !kinds.contains_key(r.key)) {
            // Named at the kinds table, or at the options or the entry when those aren't written.
            let vars: &Vars<'_> = &[("role", &missing.key), ("name", &name)];
            return Err(src.invalid_at(&here, At::Key, missing.key, "role_missing", vars));
        }
    }
    Ok(())
}

/// Whether the pack holds `path`, named from the pack's root or from `dir` (`prompts/` for a
/// `[[prompts]]` file, `queries/` for a query extractor's `.scm`).
fn holds(src: &Source, dir: &str, path: &str) -> bool {
    src.files.contains_key(path) || src.files.contains_key(&format!("{dir}/{path}"))
}

/// The longest declared ID, in bytes: every reply can then echo an ID whole within its budget, so
/// none is ever cut short (DR-94).
pub const ID_MAX: usize = 64;

/// A declared ID or prefix at `at`: lowercase letters, digits and `_`, at most [`ID_MAX`] long.
fn syntax(src: &Source, at: &KeyPath, id: &str) -> Result<(), AppError> {
    let ok = !id.is_empty() && id.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_');
    if !ok {
        return Err(src.invalid_at(at, At::Value, id, "id_syntax", &[("id", &id)]));
    }
    bounded(src, at, id)
}

/// A declared ID at `at` at most [`ID_MAX`] bytes long.
fn bounded(src: &Source, at: &KeyPath, id: &str) -> Result<(), AppError> {
    if id.len() <= ID_MAX {
        return Ok(());
    }
    let vars: &Vars<'_> = &[("id", &text::short(id, 40)), ("max", &ID_MAX)];
    Err(src.invalid_at(at, At::Value, id, "id_length", vars))
}

fn glob(ctx: &Ctx<'_>, at: &KeyPath, glob: &str) -> Result<(), AppError> {
    planroom_extract::check_glob(glob)
        .map_err(|why| ctx.bad(at, glob, "glob", &[("glob", &glob), ("why", &why)]))
}

/// A kind's or a relation's prop schema, at `at`, takes only the keywords coercion enforces, each
/// in a form it reads, and each prop's schema is a table, so a value the runner emits never breaks
/// it; any other keyword, an enforced one it can't read, or a prop whose schema isn't a table is
/// `pack_invalid` at its own key, naming the kind, the prop and the keyword.
fn prop_schema(ctx: &Ctx<'_>, at: &KeyPath, kind: &str, schema: &Value) -> Result<(), AppError> {
    let Some(Fault { prop, keyword, form }) = fault(schema) else { return Ok(()) };
    let Some(prop) = prop else {
        let vars: &Vars<'_> = &[("kind", &kind), ("keyword", &keyword)];
        let (path, why) = match (form, keyword.as_str()) {
            // `props` itself is what is wrong.
            (Form::NotATable, _) => (at.clone(), "schema_not_a_table"),
            (Form::Malformed, "type") => (at.key(&keyword), "schema_type"),
            (Form::Malformed, _) => (at.key(&keyword), "schema_malformed"),
            (Form::Unenforced, _) => (at.key(&keyword), "schema_keyword"),
        };
        return Err(ctx.pack.source.invalid_at(&path, At::Key, &keyword, why, vars));
    };
    let vars: &Vars<'_> = &[("kind", &kind), ("prop", &prop), ("keyword", &keyword)];
    let (path, why) = match form {
        // The prop's own entry under `properties` is what is wrong.
        Form::NotATable => (at.key("properties").key(&prop), "prop_not_a_table"),
        Form::Malformed | Form::Unenforced => {
            let path = keyword.split('.').fold(at.key("properties").key(&prop), |path, k| path.key(k));
            (path, if form == Form::Malformed { "prop_malformed" } else { "prop_keyword" })
        }
    };
    Err(ctx.pack.source.invalid_at(&path, At::Key, &keyword, why, vars))
}

/// A merged table's key that an earlier pack, or an earlier entry, already claims (DR-46).
fn claimed(ctx: &Ctx<'_>, at: &KeyPath, key: &str, table: &str, other: &str) -> AppError {
    let vars: &Vars<'_> = &[("key", &key), ("table", &table), ("other", &other)];
    ctx.bad(at, key, "duplicate_key", vars)
}
