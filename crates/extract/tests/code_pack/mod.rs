//! The code pack's declarative extractors, read from `packs/code` and compiled as the pack loader
//! compiles them, for the tests that run the pack's own queries and regexes: `pack.toml`, then each
//! `languages/*.toml` in path order, and its `queries/*.scm`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use planroom_contract::{ExtractorDecl, Kind, KindVocab, LanguageDecl, PackFile, RelationVocab, Vocabulary};
use planroom_extract::declarative::{Declarative, Spec};
use serde::Deserialize;

/// The code pack, merged.
pub struct CodePack {
    dir: PathBuf,
    vocabulary: Vocabulary,
    pub languages: Vec<LanguageDecl>,
    extractors: Vec<ExtractorDecl>,
    hints: BTreeMap<String, Vec<String>>,
}

/// A `languages/*.toml` file: only these two tables.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct LanguageFile {
    #[serde(default)]
    languages: Vec<LanguageDecl>,
    #[serde(default)]
    extractors: Vec<ExtractorDecl>,
}

pub fn load() -> CodePack {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packs/code");
    let file: PackFile = toml::from_str(&std::fs::read_to_string(dir.join("pack.toml")).unwrap()).unwrap();
    let qualify = |id: &str| Kind(format!("code.{id}"));
    let kinds = file
        .kinds
        .iter()
        .map(|k| KindVocab {
            kind: qualify(&k.id),
            level: k.level.clone(),
            id_prefix: k.id_prefix.clone(),
            label: k.label.clone(),
            props: k.props.clone(),
            tone: None,
        })
        .collect();
    let relations = file
        .relations
        .iter()
        .map(|r| RelationVocab {
            kind: qualify(&r.id),
            label: r.label.clone(),
            from: r.from.iter().map(|k| Kind(k.clone())).collect(),
            to: r.to.iter().map(|k| Kind(k.clone())).collect(),
            props: r.props.clone(),
            inferred: r.inferred,
        })
        .collect();
    let hints = file.hints.iter().map(|h| (format!("code.{}", h.id), h.attrs.clone())).collect();
    let (mut languages, mut extractors) = (file.languages, file.extractors);
    let mut paths: Vec<PathBuf> = std::fs::read_dir(dir.join("languages"))
        .map(|d| d.map(|e| e.unwrap().path()).collect())
        .unwrap_or_default();
    paths.sort();
    for path in paths {
        let more: LanguageFile = toml::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        languages.extend(more.languages);
        extractors.extend(more.extractors);
    }
    let vocabulary = Vocabulary { kinds, relations, lenses: vec![], suggestions: vec![], levels: vec![] };
    CodePack { dir, vocabulary, languages, extractors, hints }
}

impl CodePack {
    /// Every declarative extractor, regexes and queries, in load order.
    pub fn programs(&self) -> Vec<Declarative> {
        self.compiled(|_| true)
    }

    /// Every query extractor over `language`.
    #[allow(dead_code)]
    pub fn queries(&self, language: &str) -> Vec<Declarative> {
        self.compiled(|d| matches!(d, ExtractorDecl::Query { language: l, .. } if l == language))
    }

    /// The `.scm` text of each query extractor, by its pack-qualified name, and the emits'
    /// templates.
    #[allow(dead_code)]
    pub fn queries_and_templates(&self) -> Vec<(String, String, Vec<String>)> {
        self.extractors
            .iter()
            .filter_map(|decl| {
                let ExtractorDecl::Query { id, query, emit, .. } = decl else { return None };
                let text = std::fs::read_to_string(self.dir.join(query)).unwrap();
                let emit = serde_json::to_value(emit).unwrap();
                let mut strings = Vec::new();
                collect(&emit, &mut strings);
                Some((format!("code/{id}"), text, strings))
            })
            .collect()
    }

    fn compiled(&self, keep: impl Fn(&ExtractorDecl) -> bool) -> Vec<Declarative> {
        self.extractors
            .iter()
            .filter(|d| !matches!(d, ExtractorDecl::Builtin { .. }) && keep(d))
            .map(|decl| {
                let text = match decl {
                    ExtractorDecl::Query { query, .. } => Some(
                        std::fs::read_to_string(self.dir.join(query))
                            .unwrap_or_else(|e| panic!("{}: {e}", self.dir.join(query).display())),
                    ),
                    _ => None,
                };
                let spec = Spec {
                    pack: "code",
                    decl,
                    vocabulary: &self.vocabulary,
                    languages: &self.languages,
                    hints: &self.hints,
                    query: text.as_deref(),
                };
                Declarative::compile(&spec).unwrap_or_else(|e| panic!("{e}"))
            })
            .collect()
    }
}

/// Every string in `value`, at any depth.
fn collect(value: &serde_json::Value, out: &mut Vec<String>) {
    match value {
        serde_json::Value::String(s) => out.push(s.clone()),
        serde_json::Value::Array(items) => items.iter().for_each(|v| collect(v, out)),
        serde_json::Value::Object(map) => map.values().for_each(|v| collect(v, out)),
        _ => {}
    }
}
