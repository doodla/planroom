//! `planroom scan DIR`: discover the workspace, extract every repo at its `HEAD`, verify the
//! evidence, and commit every repo's snapshot and `scan` event in one transaction.

use std::collections::BTreeSet;
use std::path::Path;

use planroom_contract::{
    Actor, Client, CommandName, FactBatch, Handle, ScanReq, ScanResult, ScanStats, Surface, Warning,
    WarningCode,
};
use planroom_extract::git::Git;
use planroom_extract::verify::Verifier;
use planroom_extract::{RepoView, compose, files, select, walk};
use planroom_kernel::{facts, keys};
use planroom_store::{FactRow, Init, RepoOutcome, RepoScan, ScanKey, ScanRules, ScanWrite, Shape, Store};
use serde_json::{Value, json};

use crate::app::error::AppError;
use crate::app::workspace::{self, Repo, Workspace};
use crate::app::{Reply, map, new_event_id};
use crate::{clock, text};

/// The built-in extractors this build runs, as the code pack will declare them: each one's name
/// and its `files` globs. The pack loader replaces this table.
const BUILTINS: &[(&str, &[&str])] = &[("files", &["**"]), ("compose", compose::DEFAULT_FILES)];

const RULES: ScanRules = ScanRules {
    scan_key: |k: &ScanKey<'_>| keys::scan_key(k.repo, k.commit, k.packs_hash, k.previous),
    link_key: keys::link_key,
};

/// The resolved pack set's hash. No pack loader yet, so it is the empty set's, under this
/// engine's version: an engine whose built-ins change re-extracts every repo (DR-62).
pub fn packs_hash() -> String {
    keys::sha256_hex(&["planroom.packs/v1", concat!("builtin@", env!("CARGO_PKG_VERSION")), "[]"])
}

pub fn run(req: &ScanReq, cwd: &Path) -> Result<Reply<ScanResult>, AppError> {
    let dir = req.dir.as_deref().map_or_else(|| cwd.to_path_buf(), |d| cwd.join(d));
    let ws = workspace::discover(&dir)?;
    if ws.repos.is_empty() {
        return Err(AppError::workspace_not_found(&ws.root));
    }
    let mut warnings = ws.warnings.clone();
    let names: Vec<String> = ws.repos.iter().map(|r| r.name.clone()).collect();
    let mut repos = Vec::with_capacity(ws.repos.len());
    for repo in &ws.repos {
        repos.push(extract(repo, &names, &mut warnings)?);
    }
    let mut store = open_store(&ws)?;
    let packs_hash = packs_hash();
    store.meta_insert_once("packs", "[]")?;
    store.meta_insert_once("packs_hash", &packs_hash)?;
    let write = ScanWrite {
        at: clock::now(),
        actor: Actor::System { part: "scan".into() },
        client: client(),
        packs_hash,
        repos,
    };
    let done = store.commit_scan(&write, RULES)?;
    let recorded = done.repos.iter().filter(|(_, o)| matches!(o, RepoOutcome::Recorded { .. })).count();
    let view = map::top(&store, &ws.root, None)?;
    let containers = map::top(&store, &ws.root, Some(planroom_contract::Level::Container))?.nodes.len();
    let summary = text::get(
        "scan.summary",
        &[
            ("repos", &ws.repos.len()),
            ("containers", &containers),
            ("nodes", &write.repos.iter().map(|r| r.stats.nodes).sum::<u32>()),
            ("hints", &write.repos.iter().map(|r| r.stats.hints).sum::<u32>()),
            ("moved", &recorded),
        ],
    );
    let next =
        Handle { command: CommandName("map".into()), args: vec![], label: text::get("scan.next_label", &[]) };
    Ok(Reply { result: ScanResult { map: view, next }, seq: done.seq, warnings, text: summary })
}

fn client() -> Client {
    let id = std::env::var("PLANROOM_CLIENT").unwrap_or_else(|_| format!("cli:{}", std::process::id()));
    Client { surface: Surface::Cli, id }
}

/// Open the workspace's store, creating `.planroom/` and the store on the first scan.
pub fn open_store(ws: &Workspace) -> Result<Store, AppError> {
    workspace::ensure_dir(&ws.root)?;
    let init = Init {
        workspace_id: new_event_id().0,
        engine_version: env!("CARGO_PKG_VERSION").into(),
        created_at: clock::now().0,
    };
    let mut store = Store::open(&ws.store_path(), &init)?;
    crate::app::hook(&mut store);
    Ok(store)
}

/// One repo at its `HEAD`: every built-in's facts, verified, merged and ready to store. The files
/// come from `HEAD`'s tree, never the working tree, so the facts are a function of the snapshot's
/// key alone: a dirty or untracked file never reaches a snapshot.
fn extract(repo: &Repo, names: &[String], warnings: &mut Vec<Warning>) -> Result<RepoScan, AppError> {
    let git = Git::new(&repo.path);
    let head = git.head().map_err(|e| AppError::git(&repo.name, &e))?;
    let walked = walk::walk(&git, &head.commit).map_err(|e| AppError::git(&repo.name, &e))?;
    let view = RepoView {
        name: repo.name.clone(),
        root: repo.path.clone(),
        commit: head.commit.clone(),
        branch: head.branch.clone().unwrap_or_default(),
        blobs: walked.blobs.clone(),
    };
    if !walked.uncovered.is_empty() {
        warnings.push(Warning {
            code: WarningCode::PartialScan,
            message: text::get(
                "warning.partial_scan",
                &[("extractor", &files::NAME), ("repo", &repo.name), ("count", &walked.uncovered.len())],
            ),
            details: json!({"extractor": files::NAME, "repo": repo.name, "uncovered": walked.uncovered}),
        });
    }
    let refs = git.refs().map_err(|e| AppError::git(&repo.name, &e))?;
    let objects = git.objects().map_err(|e| AppError::git(&repo.name, &e))?;
    let mut verifier = Verifier::new(&repo.name, &head.commit, &refs, objects);
    let mut batches: Vec<FactBatch> = Vec::new();
    let mut dropped = Vec::new();
    for (name, globs) in BUILTINS {
        let extractor = planroom_extract::builtin(name).expect("a built-in this build has");
        let selected = select(&walked.files, globs)?;
        let opts = match *name {
            "compose" => json!({"repos": names}),
            _ => Value::Null,
        };
        let mut batch = extractor.run(&view, &selected, &opts)?;
        dropped.extend(verifier.verify(&mut batch).map_err(|e| AppError::git(&repo.name, &e))?);
        batches.push(batch);
    }
    if !dropped.is_empty() {
        let sample: Vec<Value> =
            dropped.iter().take(5).map(|d| json!({"fact": d.fact, "reason": d.reason})).collect();
        warnings.push(Warning {
            code: WarningCode::EvidenceDropped,
            message: text::get(
                "warning.evidence_dropped",
                &[("count", &dropped.len()), ("repo", &repo.name), ("commit", &short(&head.commit))],
            ),
            details: json!({"repo": repo.name, "count": dropped.len(), "items": sample}),
        });
    }
    let partial: BTreeSet<&str> =
        batches.iter().flat_map(|b| b.partial.iter().map(|p| p.path.as_str())).collect();
    if !partial.is_empty() {
        warnings.push(Warning {
            code: WarningCode::PartialParse,
            message: text::get("warning.partial_parse", &[("count", &partial.len()), ("repo", &repo.name)]),
            details: json!({"repo": repo.name, "paths": partial}),
        });
    }
    let stats = ScanStats {
        files: walked.files.len() as u32,
        nodes: 0,
        edges: 0,
        hints: 0,
        partial: partial.len() as u32,
    };
    let (facts, stats) = rows(batches, stats)?;
    Ok(RepoScan {
        repo: repo.name.clone(),
        commit: head.commit,
        branch: head.branch.unwrap_or_default(),
        stats,
        facts,
        event_id: new_event_id(),
    })
}

/// Every batch's facts as stored rows: nodes merged by ID (DR-42), one row per ID, sorted.
fn rows(batches: Vec<FactBatch>, mut stats: ScanStats) -> Result<(Vec<FactRow>, ScanStats), AppError> {
    let mut nodes = Vec::new();
    let mut edges = Vec::new();
    let mut hints = Vec::new();
    for batch in batches {
        nodes.extend(batch.nodes);
        edges.extend(batch.edges);
        hints.extend(batch.hints);
    }
    let nodes = facts::merge_nodes(nodes);
    edges.sort_by(|a, b| a.id.cmp(&b.id));
    edges.dedup_by(|a, b| a.id == b.id);
    hints.sort_by(|a, b| a.id.cmp(&b.id));
    hints.dedup_by(|a, b| a.id == b.id);
    stats.nodes = nodes.len() as u32;
    stats.edges = edges.len() as u32;
    stats.hints = hints.len() as u32;
    let row = |id: &str, shape: Shape, kind: &str, body: String| FactRow {
        id: id.to_owned(),
        shape,
        kind: kind.to_owned(),
        hash: keys::sha256_hex(&[&body]),
        body,
    };
    fn json<T: serde::Serialize>(v: &T) -> Result<String, AppError> {
        serde_json::to_string(v).map_err(AppError::internal)
    }
    let mut out = Vec::with_capacity(nodes.len() + edges.len() + hints.len());
    for n in &nodes {
        out.push(row(&n.id.0, Shape::Node, &n.kind.0, json(n)?));
    }
    for e in &edges {
        out.push(row(&e.id.0, Shape::Edge, &e.kind.0, json(e)?));
    }
    for h in &hints {
        out.push(row(&h.id, Shape::Hint, &h.kind.0, json(h)?));
    }
    out.sort_by(|a, b| a.id.cmp(&b.id));
    Ok((out, stats))
}

fn short(commit: &str) -> &str {
    &commit[..commit.len().min(7)]
}
