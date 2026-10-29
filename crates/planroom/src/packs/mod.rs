//! Packs: loading, validation and embedding (architecture.md, "What a pack is"; DR-9, DR-46,
//! DR-62).
//!
//! A pack is a directory of data. The built-in packs are every directory under `packs/`, embedded
//! at build time ([`embed`]); an out-of-tree pack is a directory a person added. [`resolve`] reads
//! each pack's files ([`load`]), checks the whole set ([`validate`]) and turns it into what the
//! engine reads: the kernel's [`Vocabulary`], the merged `[[languages]]` and `[[image_roles]]`
//! tables, every declared extractor and linker, and the set's hash.
//!
//! A pack's regex extractors compile at load. Its tree-sitter query extractors compile only when
//! a process extracts: the first time [`Loaded::runs`] or [`Loaded::programs`] is asked, once
//! per loaded set. `scan`, `doctor --pack` and adding a pack ask; a read command, such as `map`,
//! `focus`, `find`, `ask`, `schema` or `version`, or `watch`'s frames, never does, so none of them
//! pays for compiling the code pack's queries. A built-in pack's queries are compiled by a test,
//! so no shipped pack carries a broken one; an out-of-tree pack's broken query is `pack_invalid`
//! when `doctor --pack`, adding it or a scan compiles it.
//!
//! Packs load in a fixed order: the built-ins, `code` first and the rest by ID, then each
//! out-of-tree pack in the order given. That order is the merged tables' order, so an overlapping
//! image glob takes the first match, built-ins first.

pub mod embed;
pub mod load;
pub mod state;
mod validate;

pub use validate::ID_MAX;

use std::collections::BTreeMap;
use std::sync::{Arc, OnceLock};

use planroom_contract::{
    ExtractorDecl, FactBatch, Flow, ImageRole, Kind, KindVocab, LanguageDecl, LensVocab, LevelId, LevelVocab,
    LinkerDecl, PackFile, PackRecord, RelationVocab, Vocabulary,
};
use planroom_extract::declarative::Declarative;
use planroom_extract::link::{Linked, Member, Selection, WorkspaceFacts};
use planroom_extract::{ExtractError, RepoPath, RepoView};
use planroom_kernel::keys;
use planroom_kernel::ladder::Ladder;
use serde_json::{Value, json};

use crate::app::error::AppError;

pub use load::{Source, read_dir};

/// The manifest format this engine reads.
pub const FORMAT: &str = "planroom.pack/v1";
/// The base pack every other built-in pack may require, and the one that loads first.
pub const BASE: &str = "code";
/// The kernel's own namespace: its kinds and relation, which every pack may reference (DR-45).
pub const KERNEL: &str = "kernel";

/// Where a pack came from, which decides what it may require (DR-62).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Origin {
    /// Embedded in the binary from `packs/`: an inner ring, which requires `code` or nothing.
    BuiltIn,
    /// A directory a person added: it may require any built-in pack, never another out-of-tree one.
    OutOfTree,
}

/// One loaded pack: its manifest, where it came from, and its files.
#[derive(Debug, Clone)]
pub struct Pack {
    pub file: PackFile,
    pub origin: Origin,
    pub source: Source,
    /// The sha256 over its files, as its record names it.
    pub sha256: String,
}

impl Pack {
    pub fn id(&self) -> &str {
        &self.file.id
    }

    /// The record the store's `meta.packs` keeps for it.
    pub fn record(&self) -> PackRecord {
        PackRecord {
            id: self.file.id.clone(),
            version: self.file.version,
            source: match self.origin {
                Origin::BuiltIn => "builtin".into(),
                // A copy records the directory it was copied from, so the hash never depends on
                // where the copy sits.
                Origin::OutOfTree => {
                    self.source.recorded.clone().unwrap_or_else(|| self.source.label.clone())
                }
            },
            sha256: self.sha256.clone(),
        }
    }
}

/// One declared extractor: its pack-qualified name, `<pack>/<id>`, and its declaration.
#[derive(Debug, Clone)]
pub struct Declared {
    pub name: String,
    pub pack: String,
    pub decl: ExtractorDecl,
}

/// An extractor as one scan runs it: its name, its `files` globs and what runs it.
#[derive(Debug, Clone)]
pub struct Run {
    /// Pack-qualified: what its facts' provenance names.
    pub name: String,
    pub files: Vec<String>,
    pub runner: Runner,
}

/// What runs one extractor.
#[derive(Debug, Clone)]
pub enum Runner {
    /// A built-in, by the name the pack gives with `builtin = NAME`, with its options: the kinds
    /// it emits resolved against the vocabulary and the engine's inputs filled in.
    Builtin { builtin: String, options: Value },
    /// A declarative extractor, compiled the first time its loaded set ran ([`Loaded::programs`]).
    Declarative(Arc<Declarative>),
}

impl Run {
    /// The built-in's name and options, or `None` for a declarative extractor, whose every input
    /// is its pack's files, which the packs hash holds.
    pub fn builtin(&self) -> Option<(&str, &Value)> {
        match &self.runner {
            Runner::Builtin { builtin, options } => Some((builtin, options)),
            Runner::Declarative(_) => None,
        }
    }

    /// Its facts over `files` of `repo`.
    pub fn extract(&self, repo: &RepoView, files: &[RepoPath]) -> Result<FactBatch, ExtractError> {
        match &self.runner {
            Runner::Builtin { builtin, options } => {
                // The loader refused a pack naming a built-in this build doesn't have.
                let extractor = planroom_extract::builtin(builtin).expect("a built-in this build has");
                extractor.run(repo, files, options)
            }
            Runner::Declarative(program) => Ok(program.run(repo, files)),
        }
    }

    /// Whether it reads the repo's history rather than the files of one commit, as the `git`
    /// built-in does. A declarative extractor reads only files.
    pub fn reads_history(&self) -> bool {
        self.builtin()
            .and_then(|(builtin, _)| planroom_extract::builtin(builtin))
            .is_some_and(|e| e.reads_history())
    }

    /// What its own bound left out of `repo`, such as the `git` built-in's `history before <sha>`
    /// (DR-47). Empty for a declarative extractor, whose bounds are per file.
    pub fn uncovered(&self, repo: &RepoView) -> Result<Vec<String>, ExtractError> {
        match self.builtin().and_then(|(builtin, _)| planroom_extract::builtin(builtin)) {
            Some(extractor) => extractor.uncovered(repo),
            None => Ok(Vec::new()),
        }
    }
}

/// A `[[linkers]]` entry of a loaded pack.
#[derive(Debug, Clone)]
pub struct DeclaredLinker {
    /// Pack-qualified, such as `code/runtime_wiring`: what linked facts' provenance names.
    pub name: String,
    pub decl: LinkerDecl,
}

/// A linker as one scan runs it: its name, the built-in, and its options with the kinds and
/// relations they name resolved against the vocabulary.
#[derive(Debug, Clone)]
pub struct LinkRun {
    pub name: String,
    pub builtin: String,
    pub options: Value,
}

impl LinkRun {
    /// Its facts over the active set's `facts`.
    pub fn link(&self, facts: &WorkspaceFacts) -> Result<Linked, ExtractError> {
        // The loader refused a pack naming a linker this build doesn't have.
        let linker =
            planroom_extract::link::builtin(&self.builtin).expect("a built-in linker this build has");
        linker.link(facts, &self.options)
    }
}

/// Compiled declarative extractors, by pack-qualified name.
pub type Programs = BTreeMap<String, Arc<Declarative>>;

/// The loaded pack set, in load order, and everything the engine reads from it.
#[derive(Debug, Clone)]
pub struct Loaded {
    pub packs: Vec<Pack>,
    pub vocabulary: Vocabulary,
    /// Merged across packs, in load order.
    pub languages: Vec<LanguageDecl>,
    /// Merged across packs, in load order: the first glob that matches an image wins.
    pub image_roles: Vec<ImageRole>,
    /// Every pack's extractors, in load order.
    pub extractors: Vec<Declared>,
    /// Every declared built-in linker, in load order.
    pub linkers: Vec<DeclaredLinker>,
    /// Every regex extractor, compiled against the merged set at load, by its pack-qualified name.
    patterns: BTreeMap<String, Arc<Declarative>>,
    /// Every declarative extractor, the query ones compiled against the merged set, by its
    /// pack-qualified name: compiled once, the first time [`Loaded::programs`] is asked. A clone
    /// shares them, so a process whose scans each clone its one loaded set compiles them once.
    programs: Arc<OnceLock<Result<Programs, AppError>>>,
    /// The set's hash: every pack's record under this engine's identity ([`engine`]), so an
    /// engine whose built-ins change re-extracts every repo (DR-62).
    pub hash: String,
}

/// A stored fact of a member snapshot, as [`Selection::facts`] reads it.
pub fn link_member(row: &planroom_store::FactRow) -> Member<'_> {
    match row.shape {
        planroom_store::Shape::Node => Member::Node { kind: &row.kind, body: &row.body },
        planroom_store::Shape::Edge => Member::Edge { kind: &row.kind, body: &row.body },
        planroom_store::Shape::Hint => Member::Hint { body: &row.body },
    }
}

/// The built-in packs alone: what every process loads before a workspace's own packs.
pub fn builtins() -> Result<Loaded, AppError> {
    resolve(embed::sources())
}

/// The vocabulary the views read: the built-in packs', loaded once per process. A workspace's
/// own packs come from its config, which loads only once the workspace is trusted, so none
/// loads yet, as in `scan`.
pub fn vocabulary() -> Result<&'static Vocabulary, AppError> {
    Ok(&loaded()?.vocabulary)
}

/// The pack set the views and questions read: the built-in packs, loaded once per process, as
/// [`vocabulary`] reads them.
pub fn loaded() -> Result<&'static Loaded, AppError> {
    static LOADED: std::sync::OnceLock<Loaded> = std::sync::OnceLock::new();
    if let Some(loaded) = LOADED.get() {
        return Ok(loaded);
    }
    let loaded = builtins()?;
    Ok(LOADED.get_or_init(|| loaded))
}

/// Load, check and merge `sources`, the built-ins first.
pub fn resolve(sources: Vec<Source>) -> Result<Loaded, AppError> {
    let mut packs = sources.into_iter().map(load::parse).collect::<Result<Vec<Pack>, AppError>>()?;
    // The built-ins load first, `code` before the rest; out-of-tree packs keep their order.
    packs.sort_by_key(|p| match p.origin {
        Origin::BuiltIn => (0, p.id() != BASE, p.id().to_owned()),
        Origin::OutOfTree => (1, false, String::new()),
    });
    validate::check(&packs)?;
    let mut loaded = merge(packs);
    // Each regex extractor compiles against the merged set now, and each query extractor when a
    // process first runs the set; each runs as it compiled.
    loaded.patterns = validate::runnable(&loaded, false)?;
    Ok(loaded)
}

impl Loaded {
    /// Every loaded pack's record, in load order.
    pub fn records(&self) -> Vec<PackRecord> {
        self.packs.iter().map(Pack::record).collect()
    }

    /// The pack with ID `id`.
    pub fn pack(&self, id: &str) -> Option<&Pack> {
        self.packs.iter().find(|p| p.id() == id)
    }

    /// Every declarative extractor of the set, compiled against the whole set as the runner runs
    /// it, by pack-qualified name: compiled the first time this is asked, and kept, so a process
    /// compiles a set's queries once. A declaration that can't run, such as a query that doesn't
    /// compile against its grammar, is `pack_invalid` at its own line, every time it is asked.
    pub fn programs(&self) -> Result<&BTreeMap<String, Arc<Declarative>>, AppError> {
        let compile = || {
            let mut programs = validate::runnable(self, true)?;
            programs.extend(self.patterns.iter().map(|(name, p)| (name.clone(), Arc::clone(p))));
            Ok(programs)
        };
        self.programs.get_or_init(compile).as_ref().map_err(Clone::clone)
    }

    /// Whether this set's query extractors have compiled: whether some run asked for them.
    pub fn compiled(&self) -> bool {
        self.programs.get().is_some()
    }

    /// Every extractor as a scan runs it, in load order, the declarative ones compiled
    /// ([`Self::programs`]). For a built-in, `options.kinds` names become the kinds' levels and
    /// prefixes from the vocabulary; `files` takes the merged `[[languages]]` table, and
    /// `compose` the merged `[[image_roles]]` table. No extractor reads which repos the workspace
    /// holds: the join across repos is the linkers' ([`Self::linker_runs`]), so a repo's snapshot
    /// is a function of its own inputs (DR-86).
    pub fn runs(&self) -> Result<Vec<Run>, AppError> {
        Ok(self.runs_with(Some(self.programs()?)))
    }

    /// [`Self::runs`] with the declarative extractors `programs`, or the built-ins alone without
    /// them, which compiles nothing.
    fn runs_with(&self, programs: Option<&BTreeMap<String, Arc<Declarative>>>) -> Vec<Run> {
        let mut runs = Vec::new();
        for declared in &self.extractors {
            let (builtin, files, options) = match &declared.decl {
                ExtractorDecl::Builtin { builtin, files, options, .. } => (builtin, files, options),
                ExtractorDecl::Regex { files, .. } | ExtractorDecl::Query { files, .. } => {
                    let Some(programs) = programs else { continue };
                    let program = programs.get(&declared.name).expect("compiled with its set");
                    runs.push(Run {
                        name: declared.name.clone(),
                        files: files.clone(),
                        runner: Runner::Declarative(Arc::clone(program)),
                    });
                    continue;
                }
            };
            let mut options = self.resolved(options);
            match builtin.as_str() {
                "files" => {
                    options.insert("languages".into(), json!(self.languages));
                }
                "compose" => {
                    options.insert("image_roles".into(), json!(self.image_roles));
                }
                _ => {}
            }
            runs.push(Run {
                name: declared.name.clone(),
                files: files.clone(),
                runner: Runner::Builtin { builtin: builtin.clone(), options: Value::Object(options) },
            });
        }
        runs
    }

    /// What the loaded linkers read of an active set: every hint, the nodes of the kinds at the
    /// ladder's top level and of the kinds that use the container prefix, and the nodes and edges
    /// their runs' input roles name. The scan that builds the link and anything that reruns it
    /// over stored facts select through this alone.
    pub fn link_selection(&self) -> Selection {
        let kinds = &self.vocabulary.kinds;
        let top = Ladder::of(&self.vocabulary).top().clone();
        let tops = kinds.iter().filter(|k| k.level == top).map(|k| k.kind.0.clone());
        let containers =
            kinds.iter().filter(|k| k.id_prefix == validate::CONTAINER_PREFIX).map(|k| k.kind.0.clone());
        let runs = self.linker_runs();
        Selection::new(tops, containers, runs.iter().map(|r| (r.builtin.as_str(), &r.options)))
    }

    /// Every linker as a scan runs it, in load order: `options.kinds` names become the kinds'
    /// levels and prefixes, and `options.relations` names the relations with the kinds each end
    /// may be, from the vocabulary, so no linker spells a kind or a relation of its own.
    /// Every linker also takes the merged `[[languages]]` table, whose test globs name the files
    /// its joins never read.
    pub fn linker_runs(&self) -> Vec<LinkRun> {
        let relations: BTreeMap<&str, &RelationVocab> =
            self.vocabulary.relations.iter().map(|r| (r.kind.0.as_str(), r)).collect();
        self.linkers
            .iter()
            .map(|declared| {
                let mut options = self.resolved(&declared.decl.options);
                if let Some(Value::Object(named)) = options.get("relations").cloned() {
                    let resolved: serde_json::Map<String, Value> = named
                        .iter()
                        .filter_map(|(role, relation)| {
                            let vocab = relations.get(relation.as_str()?)?;
                            Some((
                                role.clone(),
                                json!({"kind": vocab.kind.0, "from": vocab.from, "to": vocab.to}),
                            ))
                        })
                        .collect();
                    options.insert("relations".into(), Value::Object(resolved));
                }
                // The merged languages, whose test globs name the files no join reads.
                options.insert("languages".into(), json!(self.languages));
                LinkRun {
                    name: declared.name.clone(),
                    builtin: declared.decl.builtin.clone(),
                    options: Value::Object(options),
                }
            })
            .collect()
    }

    /// A built-in's `options`, with the kind each `kinds` role names resolved to its kind, ID
    /// prefix and level from the vocabulary.
    fn resolved(&self, options: &Value) -> serde_json::Map<String, Value> {
        let kinds: BTreeMap<&str, &KindVocab> =
            self.vocabulary.kinds.iter().map(|k| (k.kind.0.as_str(), k)).collect();
        let mut options = match options {
            Value::Object(map) => map.clone(),
            _ => serde_json::Map::new(),
        };
        if let Some(Value::Object(named)) = options.get("kinds").cloned() {
            let resolved: serde_json::Map<String, Value> = named
                .iter()
                .filter_map(|(role, kind)| {
                    let vocab = kinds.get(kind.as_str()?)?;
                    Some((
                        role.clone(),
                        json!({"kind": vocab.kind.0, "id_prefix": vocab.id_prefix, "level": vocab.level}),
                    ))
                })
                .collect();
            options.insert("kinds".into(), Value::Object(resolved));
        }
        options
    }

    /// The kind the `files` extractor gives a repo's node, with the level and ID prefix the
    /// vocabulary gives it, as a scan resolves it: what a repo's node ID is made from. It reads the
    /// built-ins alone, so it compiles no declarative extractor.
    pub fn repo_kind(&self) -> Option<planroom_extract::KindRef> {
        let runs = self.runs_with(None);
        let (_, options) = runs.iter().filter_map(Run::builtin).find(|(b, _)| *b == "files")?;
        serde_json::from_value(options.get("kinds")?.get("repo")?.clone()).ok()
    }
}

/// This engine's identity, as a pack record names the engine that recorded it: its version for a
/// release, and for a prerelease its version and the commit it was built from, when the build
/// recorded one. Every `main` commit shares one `-dev` version, so the version alone would keep a
/// store's snapshots across a change in what the built-ins extract.
pub fn engine() -> String {
    identity(env!("CARGO_PKG_VERSION"), option_env!("PLANROOM_GIT_COMMIT"))
}

/// The identity of an engine at `version`, built from `commit`: `VERSION` for a release,
/// `VERSION+COMMIT` for a prerelease whose build recorded its commit.
pub fn identity(version: &str, commit: Option<&str>) -> String {
    match commit.map(str::trim).filter(|c| !c.is_empty()) {
        Some(commit) if version.contains('-') => format!("{version}+{commit}"),
        _ => version.to_owned(),
    }
}

/// The hash of a pack set's records, in load order, under this engine's identity, so an engine
/// whose built-ins could extract otherwise re-extracts every repo (DR-62).
pub fn set_hash(records: &[PackRecord]) -> String {
    keys::sha256_hex(&[
        "planroom.packs/v1",
        &format!("engine@{}", engine()),
        &keys::canonical(&json!(records)),
    ])
}

/// `<pack>.<name>`.
pub fn qualify(pack: &str, name: &str) -> String {
    format!("{pack}.{name}")
}

/// The ladder the set declares: the kernel's `intent` first, apart, in en.toml's words, then the
/// base pack's levels top to finest, with each other pack's level directly below its `below`, in
/// load order and, within a pack, in declaration order. The set is checked, so each `below` names
/// a level already placed, and no two levels sit directly below the same one.
fn levels(packs: &[Pack]) -> Vec<LevelVocab> {
    let intent = LevelVocab {
        id: LevelId::intent(),
        label: crate::text::get("levels.intent", &[]),
        one: crate::text::get("levels.intent_one", &[]),
        many: crate::text::get("levels.intent_many", &[]),
        apart: true,
        pack: KERNEL.to_owned(),
    };
    let mut ladder = vec![intent];
    for pack in packs {
        for l in &pack.file.levels {
            let level = LevelVocab {
                id: LevelId::from(l.id.as_str()),
                label: l.label.clone(),
                one: l.one.clone(),
                many: l.many.clone(),
                apart: false,
                pack: pack.id().to_owned(),
            };
            let at = l.below.as_deref().and_then(|below| ladder.iter().position(|v| v.id.as_str() == below));
            match at {
                Some(k) => ladder.insert(k + 1, level),
                None => ladder.push(level),
            }
        }
    }
    ladder
}

/// The checked set as the engine reads it.
fn merge(packs: Vec<Pack>) -> Loaded {
    let mut vocabulary = Vocabulary::default();
    let mut languages = Vec::new();
    let mut image_roles = Vec::new();
    let mut extractors = Vec::new();
    let mut linkers = Vec::new();
    // Two defaults load only when one is the code pack's, which stays the default.
    let base_default = packs.iter().any(|p| validate::is_base(p) && p.file.lenses.iter().any(|l| l.default));
    vocabulary.levels = levels(&packs);
    for pack in &packs {
        let id = pack.id();
        let file = &pack.file;
        vocabulary.kinds.extend(file.kinds.iter().map(|k| KindVocab {
            kind: Kind(qualify(id, &k.id)),
            level: k.level.clone(),
            id_prefix: k.id_prefix.clone(),
            label: k.label.clone(),
            props: k.props.clone(),
            tone: k.tone,
        }));
        vocabulary.relations.extend(file.relations.iter().map(|r| RelationVocab {
            kind: Kind(qualify(id, &r.id)),
            label: r.label.clone(),
            from: r.from.iter().cloned().map(Kind).collect(),
            to: r.to.iter().cloned().map(Kind).collect(),
            props: r.props.clone(),
            inferred: r.inferred,
        }));
        vocabulary.lenses.extend(file.lenses.iter().map(|l| LensVocab {
            id: l.id.clone(),
            label: l.label.clone(),
            relations: l.relations.iter().cloned().map(Kind).collect(),
            flow: l.flow.iter().map(|(k, f)| (Kind(k.clone()), *f)).collect::<BTreeMap<Kind, Flow>>(),
            kinds: l.kinds.iter().cloned().map(Kind).collect(),
            level: l.level.clone(),
            default: l.default && (validate::is_base(pack) || !base_default),
        }));
        vocabulary.suggestions.extend(file.suggestions.iter().cloned());
        languages.extend(file.languages.iter().cloned());
        image_roles.extend(file.image_roles.iter().cloned());
        extractors.extend(file.extractors.iter().map(|decl| Declared {
            name: format!("{id}/{}", load::extractor_id(decl)),
            pack: id.to_owned(),
            decl: decl.clone(),
        }));
        linkers.extend(
            file.linkers
                .iter()
                .map(|l| DeclaredLinker { name: format!("{id}/{}", l.builtin), decl: l.clone() }),
        );
    }
    let records: Vec<PackRecord> = packs.iter().map(Pack::record).collect();
    let hash = set_hash(&records);
    Loaded {
        packs,
        vocabulary,
        languages,
        image_roles,
        extractors,
        linkers,
        patterns: BTreeMap::new(),
        programs: Arc::new(OnceLock::new()),
        hash,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_dev_builds_engine_identity_names_its_commit() {
        assert_eq!(identity("0.1.0-dev", Some("abc123")), "0.1.0-dev+abc123");
        assert_eq!(identity("0.1.0-dev", None), "0.1.0-dev", "a build that recorded no commit");
        assert_eq!(identity("0.1.0-dev", Some("  ")), "0.1.0-dev");
        assert_eq!(identity("0.1.0", Some("abc123")), "0.1.0", "a release is its version alone");
        // Two prerelease builds at different commits hash one set apart, so a store's snapshots
        // never outlive a change in what the built-ins extract.
        let records = loaded().unwrap().records();
        let at = |commit: &str| {
            keys::sha256_hex(&[
                "planroom.packs/v1",
                &format!("engine@{}", identity("0.1.0-dev", Some(commit))),
                &keys::canonical(&json!(records)),
            ])
        };
        assert_ne!(at("aaa"), at("bbb"));
        assert_eq!(set_hash(&records), loaded().unwrap().hash);
    }
}
