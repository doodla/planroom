//! The `compose` built-in: each service is a container node with `host:` and `service:` aliases,
//! its image and role as props, its environment as `code.env_set` hints and its `depends_on` as
//! `code.depends_on` hints. The compose file's own repo gets a tooling container for the
//! deployment definition itself [TRUTH containers].
//!
//! A service joins a workspace repo by its `build:` context or its image name (`acme/orders` to
//! repo `orders`), and then takes that repo's namespace; any other service sits under the
//! workspace, `ws:container:<name>`. Its kind and role come from the merged `[[image_roles]]`
//! table, first match in load order; any other service takes the pack's service kind, with role
//! `service`. The pack that declares this built-in names its kinds in `options.kinds`, and every
//! container's ID takes the service kind's prefix, the kernel's `container`, so a reclassified
//! container keeps its ID (DR-46).

use std::collections::BTreeMap;

use globset::{Glob, GlobMatcher};
use planroom_contract::{
    Evidence, FACTS_FORMAT, FactBatch, Hint, Kind, Layer, Node, NodeId, PartialFile, Provenance, Tier,
};
use serde::Deserialize;
use serde_json::Value;

use crate::files::repo_id;
use crate::ids::{hint_id, node_id};
use crate::yaml::{self, Yaml};
use crate::{ExtractError, Extractor, KindRef, RepoPath, RepoView, lines};

pub const NAME: &str = "code/compose";
/// The globs a pack declares this built-in with when it names none.
pub const DEFAULT_FILES: &[&str] =
    &["**/docker-compose.yml", "**/docker-compose.yaml", "**/compose.yml", "**/compose.yaml"];

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
    /// A service no image role classifies.
    pub service: KindRef,
    /// The compose file's own repo, as the deployment definition.
    pub tooling: KindRef,
    /// A repo's own node, which a joined service sits under.
    pub repo: KindRef,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Options {
    /// The workspace's repo names, for the join by build context or image name.
    #[serde(default)]
    repos: Vec<String>,
    #[serde(default)]
    image_roles: Vec<ImageRole>,
    kinds: Kinds,
}

pub struct Compose;

struct Ctx<'a> {
    repo: &'a RepoView,
    path: String,
    text: &'a str,
    kinds: &'a Kinds,
}

impl Ctx<'_> {
    fn evidence(&self, line: u32) -> Vec<Evidence> {
        vec![Evidence::Line {
            path: self.path.clone(),
            line,
            contains: lines::line(self.text, line).map(lines::cut),
            commit: Some(self.repo.commit.clone()),
            // A snapshot is keyed by its commit, never a branch, which can move or go (DR-85).
            branch: None,
        }]
    }
}

impl Extractor for Compose {
    fn name(&self) -> &'static str {
        NAME
    }

    fn run(&self, repo: &RepoView, files: &[RepoPath], opts: &Value) -> Result<FactBatch, ExtractError> {
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
        let mut batch = FactBatch {
            format: FACTS_FORMAT.into(),
            extractor: NAME.into(),
            tier: Tier::Config,
            nodes: vec![],
            edges: vec![],
            hints: vec![],
            partial: vec![],
        };
        let mut tooling_done = false;
        for file in files {
            let full = format!("{}/{}", repo.name, file.0);
            let Some(text) = repo.read(file).and_then(|bytes| std::str::from_utf8(bytes).ok()) else {
                batch.partial.push(PartialFile { path: full, errors: 1, reason: None });
                continue;
            };
            let doc = match yaml::parse(text) {
                Ok(mut docs) if !docs.is_empty() => docs.swap_remove(0),
                _ => {
                    batch.partial.push(PartialFile { path: full, errors: 1, reason: None });
                    continue;
                }
            };
            let Some(services) = doc.get("services").and_then(Yaml::entries) else {
                continue;
            };
            let ctx = Ctx { repo, path: full, text, kinds: &opts.kinds };
            if !tooling_done {
                tooling_done = true;
                let at =
                    doc.get("name").map_or_else(|| doc.get("services").map_or(1, Yaml::line), Yaml::line);
                let mut props = BTreeMap::new();
                props.insert("role".into(), Value::from("tooling"));
                if let Some(project) = doc.get("name").and_then(Yaml::as_str) {
                    props.insert("compose_project".into(), Value::from(project));
                }
                let kinds = &opts.kinds;
                batch.nodes.push(container(
                    node_id(&repo.name, &kinds.service.id_prefix, &repo.name),
                    &kinds.tooling,
                    &repo.name,
                    Some(repo_id(&repo.name, &kinds.repo)),
                    vec![],
                    props,
                    ctx.evidence(at),
                ));
            }
            for (key, service) in services {
                let Some(name) = key.as_str() else { continue };
                service_facts(&ctx, name, key.line(), service, &opts.repos, &roles, &mut batch);
            }
        }
        batch.nodes.sort_by(|a, b| a.id.cmp(&b.id));
        batch.hints.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(batch)
    }
}

fn container(
    id: NodeId,
    kind: &KindRef,
    name: &str,
    parent: Option<NodeId>,
    aliases: Vec<String>,
    props: BTreeMap<String, Value>,
    evidence: Vec<Evidence>,
) -> Node {
    Node {
        id,
        kind: Kind(kind.kind.clone()),
        level: kind.level,
        parent,
        name: name.into(),
        aliases,
        props,
        layer: Layer::Actual,
        provenance: Provenance::Extracted { extractor: NAME.into(), tier: Tier::Config },
        evidence,
    }
}

fn service_facts(
    ctx: &Ctx<'_>,
    name: &str,
    line: u32,
    service: &Yaml,
    repos: &[String],
    roles: &[(GlobMatcher, &ImageRole)],
    batch: &mut FactBatch,
) {
    let image = service.get("image");
    let image_name = image.and_then(Yaml::as_str);
    let build =
        service.get("build").and_then(|b| b.as_str().or_else(|| b.get("context").and_then(Yaml::as_str)));
    let joined = build
        .and_then(|b| b.trim_end_matches('/').rsplit('/').next())
        .filter(|b| repos.iter().any(|r| r == b))
        .or_else(|| image_name.map(image_repo).filter(|i| repos.iter().any(|r| r == i)));
    let kinds = ctx.kinds;
    let id = node_id(joined.unwrap_or("ws"), &kinds.service.id_prefix, name);
    let role = image_name.and_then(|i| roles.iter().find(|(m, _)| m.is_match(i)).map(|(_, r)| *r));
    let mut props = BTreeMap::new();
    props.insert("role".into(), Value::from(role.map_or("service", |r| r.role.as_str())));
    if let Some(image) = image_name {
        props.insert("image".into(), Value::from(image));
    }
    let aliases = vec![format!("host:{name}"), format!("service:{name}")];
    let evidence = ctx.evidence(image.map_or(line, Yaml::line));
    // An image role's kind is Container-level, as the pack loader checks, so it takes the level
    // and prefix of the service kind.
    let kind = role
        .map_or_else(|| kinds.service.clone(), |r| KindRef { kind: r.kind.clone(), ..kinds.service.clone() });
    let parent = joined.map(|r| repo_id(r, &kinds.repo));
    batch.nodes.push(container(id.clone(), &kind, name, parent, aliases, props, evidence));

    let mut hint = |kind: &str, key: &str, value: Option<&str>, attrs: BTreeMap<String, Value>, line: u32| {
        let kind = Kind(kind.into());
        let evidence = ctx.evidence(line);
        let provenance = Provenance::Extracted { extractor: NAME.into(), tier: Tier::Config };
        let hint_id = hint_id(&kind, &id, key, &evidence);
        batch.hints.push(Hint {
            id: hint_id,
            kind,
            scope: id.clone(),
            key: key.into(),
            value: value.map(str::to_owned),
            attrs,
            evidence,
            provenance,
        });
    };
    for (var, value, line) in environment(service.get("environment")) {
        let mut attrs = BTreeMap::new();
        attrs.insert("service".into(), Value::from(name));
        attrs.insert("name".into(), Value::from(var.as_str()));
        attrs.insert("value".into(), value.as_deref().map_or(Value::Null, Value::from));
        hint(ENV_SET_HINT, &var, value.as_deref(), attrs, line);
    }
    for (target, line) in depends_on(service.get("depends_on")) {
        let mut attrs = BTreeMap::new();
        attrs.insert("service".into(), Value::from(name));
        hint(DEPENDS_ON_HINT, &target, None, attrs, line);
    }
}

/// `acme/orders:1.4.0` to `orders`; `registry:5000/x/y@sha256:..` to `y`.
fn image_repo(image: &str) -> &str {
    let image = image.split('@').next().unwrap_or(image);
    let last = image.rsplit('/').next().unwrap_or(image);
    last.split(':').next().unwrap_or(last)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn image_names_join_by_their_last_path_segment() {
        assert_eq!(image_repo("acme/orders:1.4.0"), "orders");
        assert_eq!(image_repo("redis:7.4"), "redis");
        assert_eq!(image_repo("registry:5000/x/y@sha256:abc"), "y");
    }
}
