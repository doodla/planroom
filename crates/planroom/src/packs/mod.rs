//! Packs: loading, validation and embedding (architecture.md, "What a pack is"; DR-9, DR-46,
//! DR-62).
//!
//! A pack is a directory of data. The built-in packs are every directory under `packs/`, embedded
//! at build time ([`embed`]); an out-of-tree pack is a directory a person added. [`resolve`] reads
//! each pack's files ([`load`]), checks the whole set ([`validate`]) and turns it into what the
//! engine reads: the kernel's [`Vocabulary`], the merged `[[languages]]` and `[[image_roles]]`
//! tables, every declared extractor and linker, and the set's hash.
//!
//! Packs load in a fixed order: the built-ins, `code` first and the rest by ID, then each
//! out-of-tree pack in the order given. That order is the merged tables' order, so an overlapping
//! image glob takes the first match, built-ins first.

pub mod embed;
pub mod load;
mod validate;

use std::collections::{BTreeMap, BTreeSet};

use planroom_contract::{
    ExtractorDecl, Flow, ImageRole, Kind, KindVocab, LanguageDecl, LensVocab, PackFile, PackRecord,
    RelationVocab, Vocabulary,
};
use planroom_kernel::keys;
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
                Origin::OutOfTree => self.source.label.clone(),
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

/// A built-in extractor as one scan runs it: its name, its `files` globs and its options, with
/// the kinds it emits resolved against the vocabulary and the engine's inputs filled in.
#[derive(Debug, Clone, PartialEq)]
pub struct Run {
    /// Pack-qualified: what its facts' provenance names.
    pub name: String,
    /// What the pack names with `builtin = NAME`.
    pub builtin: String,
    pub files: Vec<String>,
    pub options: Value,
}

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
    /// Every declared built-in linker, pack-qualified.
    pub linkers: Vec<String>,
    /// The set's hash: every pack's record under this engine's version, so an engine whose
    /// built-ins change re-extracts every repo (DR-62).
    pub hash: String,
}

/// The built-in packs alone: what every process loads before a workspace's own packs.
pub fn builtins() -> Result<Loaded, AppError> {
    resolve(embed::sources())
}

/// The vocabulary the views read: the built-in packs', loaded once per process. A workspace's
/// own packs come from its config, which loads only once the workspace is trusted, so none
/// loads yet, as in `scan`.
pub fn vocabulary() -> Result<&'static Vocabulary, AppError> {
    static LOADED: std::sync::OnceLock<Vocabulary> = std::sync::OnceLock::new();
    if let Some(vocabulary) = LOADED.get() {
        return Ok(vocabulary);
    }
    let vocabulary = builtins()?.vocabulary;
    Ok(LOADED.get_or_init(|| vocabulary))
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
    Ok(merge(packs))
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

    /// Every built-in extractor as a scan of a workspace whose repos are `repos` runs it, in load
    /// order: `options.kinds` names become the kinds' levels and prefixes from the vocabulary;
    /// `files` takes the merged `[[languages]]` table, and `compose` the merged `[[image_roles]]`
    /// table and the sorted repo names, since it joins a service to a repo by name.
    pub fn runs(&self, repos: &BTreeSet<&str>) -> Vec<Run> {
        let kinds: BTreeMap<&str, &KindVocab> =
            self.vocabulary.kinds.iter().map(|k| (k.kind.0.as_str(), k)).collect();
        let mut runs = Vec::new();
        for declared in &self.extractors {
            let ExtractorDecl::Builtin { builtin, files, options, .. } = &declared.decl else { continue };
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
            match builtin.as_str() {
                "files" => {
                    options.insert("languages".into(), json!(self.languages));
                }
                "compose" => {
                    options.insert("repos".into(), json!(repos));
                    options.insert("image_roles".into(), json!(self.image_roles));
                }
                _ => {}
            }
            runs.push(Run {
                name: declared.name.clone(),
                builtin: builtin.clone(),
                files: files.clone(),
                options: Value::Object(options),
            });
        }
        runs
    }

    /// The kind the `files` extractor gives a repo's node, with the level and ID prefix the
    /// vocabulary gives it, as a scan resolves it: what a repo's node ID is made from.
    pub fn repo_kind(&self) -> Option<planroom_extract::KindRef> {
        let run = self.runs(&BTreeSet::new()).into_iter().find(|r| r.builtin == "files")?;
        serde_json::from_value(run.options.get("kinds")?.get("repo")?.clone()).ok()
    }
}

/// `<pack>.<name>`.
pub fn qualify(pack: &str, name: &str) -> String {
    format!("{pack}.{name}")
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
    for pack in &packs {
        let id = pack.id();
        let file = &pack.file;
        vocabulary.kinds.extend(file.kinds.iter().map(|k| KindVocab {
            kind: Kind(qualify(id, &k.id)),
            level: k.level,
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
            level: l.level,
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
        linkers.extend(file.linkers.iter().map(|l| format!("{id}/{}", l.builtin)));
    }
    let records: Vec<PackRecord> = packs.iter().map(Pack::record).collect();
    let hash = keys::sha256_hex(&[
        "planroom.packs/v1",
        concat!("engine@", env!("CARGO_PKG_VERSION")),
        &keys::canonical(&json!(records)),
    ]);
    Loaded { packs, vocabulary, languages, image_roles, extractors, linkers, hash }
}
