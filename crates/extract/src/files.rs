//! The universal `files` built-in: a `System` node per repo, a `Component` node per directory and
//! a `Code` node per file, with the `language` the merged `[[languages]]` table gives its extension.
//!
//! Evidence: a file's first line (`Evidence::Line`, line 1); an empty or non-UTF-8 file, a
//! directory and the repo carry the commit they were read at (`Evidence::Ref`). Every file and
//! directory is one of that commit's paths, because the walk reads the commit's tree. Evidence
//! names no branch: a branch can move or be deleted while the commit stays (DR-85).

use std::collections::{BTreeMap, BTreeSet};

use planroom_contract::{
    Evidence, FACTS_FORMAT, FactBatch, Kind, Layer, Level, Node, NodeId, PartialFile, Provenance, Tier,
};
use serde::Deserialize;
use serde_json::Value;

use crate::ids::node_id;
use crate::{ExtractError, Extractor, RepoPath, RepoView, lines};

pub const NAME: &str = "code/files";

/// The node kinds and ID prefixes this extractor emits; the code pack declares them.
pub const REPO_KIND: &str = "code.repo";
pub const REPO_PREFIX: &str = "repo";
pub const DIR_KIND: &str = "code.directory";
pub const DIR_PREFIX: &str = "dir";
pub const FILE_KIND: &str = "code.file";
pub const FILE_PREFIX: &str = "file";

/// One `[[languages]]` entry, as the merged table passes it in `options.languages`.
#[derive(Debug, Clone, Deserialize)]
pub struct Language {
    pub id: String,
    pub extensions: Vec<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct Options {
    #[serde(default)]
    languages: Vec<Language>,
}

/// The repo node's ID.
pub fn repo_id(repo: &str) -> NodeId {
    node_id(repo, REPO_PREFIX, repo)
}

pub struct Files;

impl Extractor for Files {
    fn name(&self) -> &'static str {
        NAME
    }

    fn run(&self, repo: &RepoView, files: &[RepoPath], opts: &Value) -> Result<FactBatch, ExtractError> {
        let opts: Options = if opts.is_null() {
            Options::default()
        } else {
            serde_json::from_value(opts.clone())
                .map_err(|e| ExtractError::InvalidOptions(format!("{NAME}: {e}")))?
        };
        let by_ext: BTreeMap<&str, &str> = opts
            .languages
            .iter()
            .flat_map(|l| l.extensions.iter().map(move |e| (e.trim_start_matches('.'), l.id.as_str())))
            .collect();
        let tree_evidence = || vec![Evidence::Ref { commit: repo.commit.clone(), ref_name: None }];
        let provenance = Provenance::Extracted { extractor: NAME.into(), tier: Tier::Universal };
        let node = |id: NodeId, kind: &str, level, parent, name: &str, props, evidence| Node {
            id,
            kind: Kind(kind.into()),
            level,
            parent,
            name: name.into(),
            aliases: vec![],
            props,
            layer: Layer::Actual,
            provenance: provenance.clone(),
            evidence,
        };
        let parent_of = |path: &str| match path.rsplit_once('/') {
            Some((dir, _)) => node_id(&repo.name, DIR_PREFIX, dir),
            None => repo_id(&repo.name),
        };

        let mut nodes = Vec::new();
        let mut partial = Vec::new();
        let mut dirs = BTreeSet::new();
        let mut languages = BTreeSet::new();
        for file in files {
            let path = file.0.as_str();
            let full = format!("{}/{path}", repo.name);
            let evidence = match repo.read(file) {
                Some(bytes) => match std::str::from_utf8(bytes) {
                    Ok(text) if !text.is_empty() => vec![Evidence::Line {
                        path: full.clone(),
                        line: 1,
                        contains: lines::line(text, 1).map(lines::cut),
                        commit: Some(repo.commit.clone()),
                        branch: None,
                    }],
                    Ok(_) => tree_evidence(),
                    Err(_) => {
                        partial.push(PartialFile { path: full.clone(), errors: 1 });
                        tree_evidence()
                    }
                },
                None => {
                    partial.push(PartialFile { path: full.clone(), errors: 1 });
                    continue;
                }
            };
            // A directory exists only for the commit's files: the ones the walk read.
            let mut dir = path;
            while let Some((up, _)) = dir.rsplit_once('/') {
                dirs.insert(up.to_owned());
                dir = up;
            }
            let name = path.rsplit('/').next().unwrap_or(path);
            let mut props = BTreeMap::new();
            if let Some(language) = name.rsplit_once('.').and_then(|(_, ext)| by_ext.get(ext)) {
                languages.insert(*language);
                props.insert("language".to_owned(), Value::String((*language).to_owned()));
            }
            let id = node_id(&repo.name, FILE_PREFIX, path);
            nodes.push(node(id, FILE_KIND, Level::Code, Some(parent_of(path)), name, props, evidence));
        }
        for dir in &dirs {
            let name = dir.rsplit('/').next().unwrap_or(dir);
            let id = node_id(&repo.name, DIR_PREFIX, dir);
            nodes.push(node(
                id,
                DIR_KIND,
                Level::Component,
                Some(parent_of(dir)),
                name,
                BTreeMap::new(),
                tree_evidence(),
            ));
        }
        let mut props = BTreeMap::new();
        if !languages.is_empty() {
            props.insert("languages".to_owned(), Value::from(languages.into_iter().collect::<Vec<_>>()));
        }
        nodes.push(node(
            repo_id(&repo.name),
            REPO_KIND,
            Level::System,
            None,
            &repo.name,
            props,
            tree_evidence(),
        ));
        nodes.sort_by(|a, b| a.id.cmp(&b.id));
        partial.sort_by(|a, b| a.path.cmp(&b.path));
        Ok(FactBatch {
            format: FACTS_FORMAT.into(),
            extractor: NAME.into(),
            tier: Tier::Universal,
            nodes,
            edges: vec![],
            hints: vec![],
            partial,
        })
    }
}
