//! The `compose` built-in: each service as a `code.compose_service` hint, its environment as
//! `code.env_set` hints and its `depends_on` as `code.depends_on` hints, all scoped to the
//! compose file; and the compose file's own repo as a tooling container for the deployment
//! definition itself [TRUTH containers].
//!
//! A service's hint is keyed by its name and cites its `image:` line, or its key's line when it
//! has no image. Its attributes are the `image`, the `build` context's last segment, and the
//! `kind` and `role` the merged `[[image_roles]]` table gives its image, first match in load
//! order; any other service takes the pack's service kind, with role `service`. Joining a
//! service to a workspace repo, by its build context or its image name, and minting its
//! container are the `runtime_wiring` linker's, in the link snapshot ([`crate::link::runtime`]).
//! So this built-in reads one repo's files and nothing else, and a repo's snapshot holds no other
//! repo's name (DR-86). The pack that declares it names its kinds in `options.kinds`.
//!
//! A service whose name holds a newline or a backslash, which no node ID can hold, gives no hint,
//! and its file is partial. A file that isn't UTF-8 or doesn't parse is partial and gives
//! nothing; a byte order mark before the document is read past. Each file's lines are indexed
//! once, and its facts are bounded as the declarative runner's are
//! ([`crate::declarative::charge`]): a file whose facts would hold more heap than its size allows
//! is partial with `work_limit`, and in sorted path order the first file that would take the
//! repo's total past its bound, and every one after it, with `repo_limit`; each gives no fact.

use std::collections::BTreeMap;

use globset::{Glob, GlobMatcher};
use planroom_contract::{
    Evidence, FACTS_FORMAT, FactBatch, Hint, Kind, Layer, Node, NodeId, PartialFile, PartialReason,
    Provenance, Tier,
};
use serde::Deserialize;
use serde_json::Value;

use crate::declarative::REPO_CAPTURE_MAX;
use crate::declarative::charge::{self, Charges};
use crate::files::repo_id;
use crate::ids::{hint_id, node_id};
use crate::link::nameable;
use crate::yaml::{self, Yaml};
use crate::{ExtractError, Extractor, KindRef, RepoPath, RepoView, lines};

pub const NAME: &str = "code/compose";
/// The globs a pack declares this built-in with when it names none.
pub const DEFAULT_FILES: &[&str] =
    &["**/docker-compose.yml", "**/docker-compose.yaml", "**/compose.yml", "**/compose.yaml"];

pub const COMPOSE_SERVICE_HINT: &str = "code.compose_service";
pub const ENV_SET_HINT: &str = "code.env_set";
pub const DEPENDS_ON_HINT: &str = "code.depends_on";

/// One `[[image_roles]]` entry, as the merged table passes it in `options.image_roles`.
#[derive(Debug, Clone, Deserialize)]
pub struct ImageRole {
    pub image: String,
    pub role: String,
    pub kind: String,
}

/// The kinds this built-in emits, which the pack that declares it names in `options.kinds`.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Kinds {
    /// A service no image role classifies: its kind, and the ID prefix every container takes.
    pub service: KindRef,
    /// The compose file's own repo, as the deployment definition.
    pub tooling: KindRef,
    /// A repo's own node, which the tooling container sits under.
    pub repo: KindRef,
    /// The compose file's node, which every hint is scoped to.
    pub file: KindRef,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Options {
    #[serde(default)]
    image_roles: Vec<ImageRole>,
    kinds: Kinds,
}

pub struct Compose;

/// One compose file as it is read.
struct Ctx<'a> {
    repo: &'a RepoView,
    path: String,
    text: &'a str,
    /// The text's line starts, read once, so each fact finds its line by a binary search.
    index: lines::Index,
    /// The file's node, which its hints are scoped to.
    scope: NodeId,
}

impl Ctx<'_> {
    fn evidence(&self, line: u32) -> Vec<Evidence> {
        vec![Evidence::Line {
            path: self.path.clone(),
            line,
            contains: self.index.line(self.text, line).map(lines::cut),
            commit: Some(self.repo.commit.clone()),
            // A snapshot is keyed by its commit, never a branch, which can move or go (DR-85).
            branch: None,
        }]
    }
}

/// One file's facts, before they join the batch.
#[derive(Default)]
struct Read {
    tooling: Option<Node>,
    hints: Vec<Hint>,
    /// Services whose name no node ID can hold.
    errors: u32,
}

impl Extractor for Compose {
    fn name(&self) -> &'static str {
        NAME
    }

    fn run(&self, repo: &RepoView, files: &[RepoPath], opts: &Value) -> Result<FactBatch, ExtractError> {
        self.run_within(repo, files, opts, REPO_CAPTURE_MAX)
    }
}

impl Compose {
    /// [`Extractor::run`] with the repo's bound at `max` in place of [`REPO_CAPTURE_MAX`], so a
    /// test can reach the cut without a repo whose facts hold 256 MiB. Builds without debug
    /// assertions leave it out, so no release can lower the bound.
    #[cfg(debug_assertions)]
    #[doc(hidden)]
    pub fn run_with_repo_capture_max(
        &self,
        repo: &RepoView,
        files: &[RepoPath],
        opts: &Value,
        max: u64,
    ) -> Result<FactBatch, ExtractError> {
        self.run_within(repo, files, opts, max)
    }

    fn run_within(
        &self,
        repo: &RepoView,
        files: &[RepoPath],
        opts: &Value,
        max: u64,
    ) -> Result<FactBatch, ExtractError> {
        let opts: Options = serde_json::from_value(opts.clone())
            .map_err(|e| ExtractError::InvalidOptions(format!("{NAME}: {e}")))?;
        let roles: Vec<(GlobMatcher, &ImageRole)> = opts
            .image_roles
            .iter()
            .map(|r| {
                Glob::new(&r.image).map(|g| (g.compile_matcher(), r)).map_err(|e| {
                    ExtractError::InvalidOptions(format!("{NAME}: image role `{}`: {e}", r.image))
                })
            })
            .collect::<Result<_, _>>()?;
        let mut files: Vec<&RepoPath> = files.iter().collect();
        files.sort();
        files.dedup();
        let mut batch = FactBatch {
            format: FACTS_FORMAT.into(),
            extractor: NAME.into(),
            tier: Tier::Config,
            nodes: vec![],
            edges: vec![],
            hints: vec![],
            partial: vec![],
        };
        let mut charges = Charges::new(max);
        for file in files {
            let full = format!("{}/{}", repo.name, file.0);
            let bytes = repo.read(file);
            if let Err(reason) = charges.start(bytes.map_or(0, <[u8]>::len)) {
                // Past the repo's cut: not read at all.
                batch.partial.push(PartialFile { path: full, errors: 1, reason: Some(reason) });
                continue;
            }
            let Some(text) = bytes.and_then(|bytes| std::str::from_utf8(bytes).ok()) else {
                batch.partial.push(PartialFile { path: full, errors: 1, reason: None });
                continue;
            };
            let doc = match yaml::parse(lines::strip_bom(text)) {
                Ok(mut docs) if !docs.is_empty() => docs.swap_remove(0),
                _ => {
                    batch.partial.push(PartialFile { path: full, errors: 1, reason: None });
                    continue;
                }
            };
            let Some(services) = doc.get("services").and_then(Yaml::entries) else {
                continue;
            };
            let ctx = Ctx {
                repo,
                path: full.clone(),
                text,
                index: lines::Index::new(text),
                scope: node_id(&repo.name, &opts.kinds.file.id_prefix, &file.0),
            };
            // The first compose file in path order that runs services gives the tooling container.
            let tooling = batch.nodes.is_empty();
            let read = match read(&ctx, &doc, services, &opts.kinds, &roles, tooling, &mut charges) {
                Ok(read) => read,
                Err(reason) => {
                    batch.partial.push(PartialFile { path: full, errors: 1, reason: Some(reason) });
                    continue;
                }
            };
            if let Err(reason) = charges.finish() {
                batch.partial.push(PartialFile { path: full, errors: 1, reason: Some(reason) });
                continue;
            }
            if read.errors > 0 {
                batch.partial.push(PartialFile { path: full, errors: read.errors, reason: None });
            }
            batch.nodes.extend(read.tooling);
            batch.hints.extend(read.hints);
        }
        batch.nodes.sort_by(|a, b| a.id.cmp(&b.id));
        batch.hints.sort_by(|a, b| a.id.cmp(&b.id));
        batch.hints.dedup_by(|a, b| a.id == b.id);
        Ok(batch)
    }
}

/// One compose file's facts, each charged to `charges` as it is made ([`charge`]); `Err` with a
/// reason when they would charge more than the file's budget.
fn read(
    ctx: &Ctx<'_>,
    doc: &Yaml,
    services: &[(Yaml, Yaml)],
    kinds: &Kinds,
    roles: &[(GlobMatcher, &ImageRole)],
    tooling: bool,
    charges: &mut Charges,
) -> Result<Read, PartialReason> {
    let mut read = Read::default();
    let repo = &ctx.repo.name;
    if tooling {
        let at = doc.get("name").map_or_else(|| doc.get("services").map_or(1, Yaml::line), Yaml::line);
        let mut props = BTreeMap::new();
        props.insert("role".into(), Value::from("tooling"));
        if let Some(project) = doc.get("name").and_then(Yaml::as_str) {
            props.insert("compose_project".into(), Value::from(project));
        }
        let node = Node {
            id: node_id(repo, &kinds.service.id_prefix, repo),
            kind: Kind(kinds.tooling.kind.clone()),
            level: kinds.tooling.level,
            parent: Some(repo_id(repo, &kinds.repo)),
            name: repo.clone(),
            aliases: vec![],
            props,
            layer: Layer::Actual,
            provenance: Provenance::Extracted { extractor: NAME.into(), tier: Tier::Config },
            evidence: ctx.evidence(at),
        };
        charges.add(charge::node(&node))?;
        read.tooling = Some(node);
    }
    for (key, service) in services {
        let Some(name) = key.as_str() else { continue };
        if !nameable(name) {
            read.errors += 1;
            continue;
        }
        let service = Service { name, line: key.line(), yaml: service };
        service_hints(ctx, &service, kinds, roles, charges, &mut read.hints)?;
    }
    Ok(read)
}

/// One service as its hints read it: its name, its key's line and its definition.
struct Service<'a> {
    name: &'a str,
    line: u32,
    yaml: &'a Yaml,
}

/// One service's hints, onto `out`: what it runs, the environment it sets and the services it
/// depends on. Each is charged as it is made, so a service whose long name each of its many
/// variables copies is refused before its facts pass the file's budget, never once they are all
/// made.
fn service_hints(
    ctx: &Ctx<'_>,
    service: &Service<'_>,
    kinds: &Kinds,
    roles: &[(GlobMatcher, &ImageRole)],
    charges: &mut Charges,
    out: &mut Vec<Hint>,
) -> Result<(), PartialReason> {
    let Service { name, line, yaml: service } = *service;
    let mut hint = |kind: &str, key: &str, value: Option<&str>, attrs: BTreeMap<String, Value>, line: u32| {
        let kind = Kind(kind.into());
        let evidence = ctx.evidence(line);
        let hint = Hint {
            id: hint_id(&kind, &ctx.scope, key, &evidence),
            kind,
            scope: ctx.scope.clone(),
            key: key.into(),
            value: value.map(str::to_owned),
            attrs,
            evidence,
            provenance: Provenance::Extracted { extractor: NAME.into(), tier: Tier::Config },
        };
        charges.add(charge::hint(&hint))?;
        out.push(hint);
        Ok(())
    };
    let image = service.get("image");
    let image_name = image.and_then(Yaml::as_str);
    let build =
        service.get("build").and_then(|b| b.as_str().or_else(|| b.get("context").and_then(Yaml::as_str)));
    let role = image_name.and_then(|i| roles.iter().find(|(m, _)| m.is_match(i)).map(|(_, r)| *r));
    let mut attrs = BTreeMap::new();
    attrs.insert("kind".into(), Value::from(role.map_or(kinds.service.kind.as_str(), |r| r.kind.as_str())));
    attrs.insert("role".into(), Value::from(role.map_or("service", |r| r.role.as_str())));
    if let Some(image) = image_name {
        attrs.insert("image".into(), Value::from(image));
    }
    let context = build.and_then(|b| b.trim_end_matches('/').rsplit('/').next()).filter(|b| !b.is_empty());
    if let Some(context) = context {
        attrs.insert("build".into(), Value::from(context));
    }
    hint(COMPOSE_SERVICE_HINT, name, None, attrs, image.map_or(line, Yaml::line))?;
    for (var, value, line) in environment(service.get("environment")) {
        let mut attrs = BTreeMap::new();
        attrs.insert("service".into(), Value::from(name));
        attrs.insert("name".into(), Value::from(var.as_str()));
        attrs.insert("value".into(), value.as_deref().map_or(Value::Null, Value::from));
        hint(ENV_SET_HINT, &var, value.as_deref(), attrs, line)?;
    }
    for (target, line) in depends_on(service.get("depends_on")) {
        let mut attrs = BTreeMap::new();
        attrs.insert("service".into(), Value::from(name));
        hint(DEPENDS_ON_HINT, &target, None, attrs, line)?;
    }
    Ok(())
}

/// `environment` as a list of `NAME=VALUE` or a mapping: `(name, value, line)`.
fn environment(env: Option<&Yaml>) -> Vec<(String, Option<String>, u32)> {
    match env {
        Some(Yaml::Seq { items, .. }) => items
            .iter()
            .filter_map(|item| {
                let text = item.as_str()?;
                let (name, value) = match text.split_once('=') {
                    Some((n, v)) => (n, Some(v.to_owned())),
                    None => (text, None),
                };
                Some((name.to_owned(), value, item.line()))
            })
            .collect(),
        Some(Yaml::Map { entries, .. }) => entries
            .iter()
            .filter_map(|(k, v)| Some((k.as_str()?.to_owned(), v.as_str().map(str::to_owned), k.line())))
            .collect(),
        _ => vec![],
    }
}

/// `depends_on` as a list or a mapping of service names: `(name, line)`.
fn depends_on(deps: Option<&Yaml>) -> Vec<(String, u32)> {
    match deps {
        Some(Yaml::Seq { items, .. }) => {
            items.iter().filter_map(|i| Some((i.as_str()?.to_owned(), i.line()))).collect()
        }
        Some(Yaml::Map { entries, .. }) => {
            entries.iter().filter_map(|(k, _)| Some((k.as_str()?.to_owned(), k.line()))).collect()
        }
        _ => vec![],
    }
}
