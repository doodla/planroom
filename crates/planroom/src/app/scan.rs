//! `planroom scan DIR`: discover the workspace, extract every repo at its `HEAD`, verify the
//! evidence, and commit every repo's snapshot and `scan` event in one transaction.
//!
//! Each repo stands alone: one git can't read is left out with a `repo_skipped` warning, one with
//! no commit yet maps nothing until its first commit, with `repo_unborn`, and the rest of the
//! workspace maps.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use planroom_contract::errors::WARNING_SAMPLE;
use planroom_contract::{
    Actor, Client, Command, CommandName, FACTS_FORMAT, FactBatch, Handle, ScanReq, ScanResult, ScanStats,
    Seq, Surface, Tier, Warning, WarningCode,
};
use planroom_extract::git::{Git, GitError};
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
    scan_key: |k: &ScanKey<'_>| keys::scan_key(k.repo, k.commit, k.inputs, k.previous),
    link_key: keys::link_key,
};

/// The resolved pack set's hash. No pack loader yet, so it is the empty set's, under this
/// engine's version: an engine whose built-ins change re-extracts every repo (DR-62).
pub fn packs_hash() -> String {
    keys::sha256_hex(&["planroom.packs/v1", concat!("builtin@", env!("CARGO_PKG_VERSION")), "[]"])
}

/// A scan between extraction and commit: every repo git could read, at the `HEAD` it read.
pub struct Prepared {
    ws: Workspace,
    packs_hash: String,
    /// Every repo's extraction inputs besides its commit and a partial clone's missing blobs.
    inputs: String,
    /// The store's latest `seq` before any repo was read.
    started: Seq,
    repos: Vec<RepoScan>,
    warnings: Vec<Warning>,
}

pub fn run(req: &ScanReq, cwd: &Path) -> Result<Reply<ScanResult>, AppError> {
    commit(prepare(req, cwd)?)
}

/// Discover the workspace and extract each repo at its `HEAD`. Nothing is written yet.
pub fn prepare(req: &ScanReq, cwd: &Path) -> Result<Prepared, AppError> {
    let dir = req.dir.as_deref().map_or_else(|| cwd.to_path_buf(), |d| cwd.join(d));
    let ws = workspace::discover(&dir)?;
    if ws.repos.is_empty() {
        return Err(AppError::workspace_not_found(&ws.root));
    }
    // Read before any repo, so a scan that commits meanwhile is newer than this one.
    let started = match Store::open_existing(&ws.store_path())? {
        Some(store) => store.seq()?,
        None => 0,
    };
    let mut warnings = ws.warnings.clone();
    let names: BTreeSet<&str> = ws.repos.iter().map(|r| r.name.as_str()).collect();
    let options = options(&names);
    let packs_hash = packs_hash();
    let inputs = inputs_hash(&packs_hash, &options);
    let mut repos = Vec::with_capacity(ws.repos.len());
    for repo in &ws.repos {
        match extract(repo, &options, &inputs, &mut warnings) {
            Ok(scan) => repos.push(scan),
            Err(RepoError::Git(e)) => warnings.push(skipped(repo, &e)),
            Err(RepoError::App(e)) => return Err(e),
        }
    }
    Ok(Prepared { ws, packs_hash, inputs, started, repos, warnings })
}

/// Commit a prepared scan. A repo stays where it is when its `HEAD` moved on since it was read, or
/// when another scan moved it meanwhile and the workspace's repos changed since this scan read
/// them: a late scan never moves a repo back, whatever input changed.
pub fn commit(prepared: Prepared) -> Result<Reply<ScanResult>, AppError> {
    let Prepared { ws, packs_hash, inputs, started, repos, mut warnings } = prepared;
    // With nothing to write, a workspace with no store stays without one.
    let store = if repos.is_empty() {
        Store::open_existing(&ws.store_path())?.map(|mut store| {
            crate::app::hook(&mut store);
            store
        })
    } else {
        Some(open_store(&ws)?)
    };
    let write = ScanWrite {
        at: clock::now(),
        actor: Actor::System { part: "scan".into() },
        client: client(),
        packs_hash,
        repos,
        started,
    };
    let Some(mut store) = store else {
        let view = map::not_scanned_view(&ws.root)?;
        let summary = summary(&write, 0, 0);
        return Ok(Reply { result: ScanResult { map: view, next: next() }, seq: 0, warnings, text: summary });
    };
    store.meta_insert_once("packs", "[]")?;
    store.meta_insert_once("packs_hash", &write.packs_hash)?;
    let paths: BTreeMap<&str, &PathBuf> = ws.repos.iter().map(|r| (r.name.as_str(), &r.path)).collect();
    // Why each superseded repo stays, and whether this scan's repo set is still the workspace's,
    // which is asked once, and only when another scan moved a repo after this one began.
    let mut why: BTreeMap<String, &str> = BTreeMap::new();
    let mut same_repos: Option<bool> = None;
    let done = store.commit_scan(&write, RULES, &mut |repo, newer_scan| {
        if !paths.get(repo.repo.as_str()).is_some_and(|p| is_head(p, &repo.commit)) {
            why.insert(repo.repo.clone(), "head_moved");
            return false;
        }
        if newer_scan
            && !*same_repos.get_or_insert_with(|| {
                current_inputs(&ws.root, &write.packs_hash).as_deref() == Some(&inputs)
            })
        {
            why.insert(repo.repo.clone(), "repos_changed");
            return false;
        }
        true
    })?;
    for ((name, outcome), repo) in done.repos.iter().zip(&write.repos) {
        if let RepoOutcome::Superseded { .. } = outcome {
            let reason = why.get(name).copied().unwrap_or("head_moved");
            warnings.push(Warning {
                code: WarningCode::ScanSuperseded,
                message: text::get(
                    &format!("warning.scan_superseded_{reason}"),
                    &[("repo", name), ("commit", &short(&repo.commit))],
                ),
                details: json!({"repo": name, "commit": repo.commit, "reason": reason}),
            });
        }
    }
    // An unborn repo's scan event records its branch, but it maps nothing, so it isn't counted.
    let recorded = done
        .repos
        .iter()
        .zip(&write.repos)
        .filter(|((_, o), repo)| matches!(o, RepoOutcome::Recorded { .. }) && !repo.commit.is_empty())
        .count();
    // The top-level map an agent reads in scan's reply keeps to scan's budget, as map's does.
    let mut view = map::top(&store, &ws.root, None)?;
    let budget = Command::Scan(ScanReq::default()).spec().budget_tokens;
    let cut = map::fit_beside(&mut view, budget, None, &warnings);
    warnings.extend(cut);
    let containers = map::top(&store, &ws.root, Some(planroom_contract::Level::Container))?.nodes.len();
    let summary = summary(&write, containers, recorded);
    Ok(Reply { result: ScanResult { map: view, next: next() }, seq: done.seq, warnings, text: summary })
}

/// The scan's one line: the repos it mapped, those with a commit, and how many moved; then the
/// repos with no commit yet, which map after their first. A skipped repo isn't in `write`.
fn summary(write: &ScanWrite, containers: usize, moved: usize) -> String {
    let (mapped, unborn): (Vec<&RepoScan>, Vec<&RepoScan>) =
        write.repos.iter().partition(|r| !r.commit.is_empty());
    let mut line = text::get(
        "scan.summary",
        &[
            ("repos", &mapped.len()),
            ("containers", &containers),
            ("nodes", &mapped.iter().map(|r| r.stats.nodes).sum::<u32>()),
            ("hints", &mapped.iter().map(|r| r.stats.hints).sum::<u32>()),
            ("moved", &moved),
        ],
    );
    if !unborn.is_empty() {
        let names: Vec<&str> = unborn.iter().map(|r| r.repo.as_str()).collect();
        line.push(' ');
        line.push_str(&text::get("scan.unborn", &[("names", &names.join(", "))]));
    }
    line
}

fn next() -> Handle {
    Handle { command: CommandName("map".into()), args: vec![], label: text::get("scan.next_label", &[]) }
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

/// Each built-in's options, as this workspace gives them: `compose` joins a service to a repo of
/// the workspace by name, so it takes the sorted repo names.
fn options(names: &BTreeSet<&str>) -> Vec<(&'static str, Value)> {
    BUILTINS
        .iter()
        .map(|(name, _)| {
            let opts = match *name {
                "compose" => json!({"repos": names}),
                _ => Value::Null,
            };
            (*name, opts)
        })
        .collect()
}

/// The hash of every extraction input besides a repo's commit: the packs hash and each built-in's
/// options. It keys the snapshot with the repo and the commit, as the packs hash alone did, so a
/// snapshot's facts are a function of its key (DR-85): moving a repo out of the workspace and
/// back gives the map a fresh store gives.
fn inputs_hash(packs_hash: &str, options: &[(&str, Value)]) -> String {
    let options: Vec<String> =
        options.iter().map(|(name, opts)| format!("{name}={}", keys::canonical(opts))).collect();
    keys::sha256_hex(&["planroom.extract-inputs/v1", packs_hash, &options.join("\n")])
}

/// The extraction inputs the workspace at `root` gives now, as [`prepare`] derives them: `None`
/// when its repos can't be listed.
fn current_inputs(root: &Path, packs_hash: &str) -> Option<String> {
    let ws = workspace::discover(root).ok()?;
    let names: BTreeSet<&str> = ws.repos.iter().map(|r| r.name.as_str()).collect();
    Some(inputs_hash(packs_hash, &options(&names)))
}

/// Whether `HEAD` in the repo at `path` still names `commit` (empty: still no commit).
fn is_head(path: &Path, commit: &str) -> bool {
    let git = Git::new(path);
    match git.head() {
        Ok(head) => head.commit == commit,
        Err(_) => commit.is_empty() && git.unborn_branch().is_some(),
    }
}

/// Why one repo couldn't be extracted: git refused it (that repo is skipped), or a bug.
enum RepoError {
    Git(GitError),
    App(AppError),
}

impl From<GitError> for RepoError {
    fn from(e: GitError) -> RepoError {
        RepoError::Git(e)
    }
}

/// The `repo_skipped` warning for a repo git can't read: its reason and the fix.
fn skipped(repo: &Repo, e: &GitError) -> Warning {
    let path = repo.path.display().to_string();
    let reason = if e.is_other_owner() { "other_owner" } else { "git_failed" };
    let fix = text::get(&format!("fix.{reason}"), &[("path", &path)]);
    Warning {
        code: WarningCode::RepoSkipped,
        message: text::get(&format!("warning.repo_skipped_{reason}"), &[("repo", &repo.name), ("fix", &fix)]),
        details: json!({"repo": repo.name, "reason": reason, "fix": fix}),
    }
}

/// A repo with no commit yet: the map holds only committed content and every fact cites its
/// evidence, so it maps nothing until its first commit (DR-86); the warning says so.
fn unborn(
    repo: &Repo,
    branch: String,
    inputs: &str,
    warnings: &mut Vec<Warning>,
) -> Result<RepoScan, RepoError> {
    warnings.push(Warning {
        code: WarningCode::RepoUnborn,
        message: text::get("warning.repo_unborn", &[("repo", &repo.name)]),
        details: json!({"repo": repo.name, "branch": branch}),
    });
    let batch = FactBatch {
        format: FACTS_FORMAT.into(),
        extractor: files::NAME.into(),
        tier: Tier::Universal,
        nodes: vec![],
        edges: vec![],
        hints: vec![],
        partial: vec![],
    };
    let (facts, stats) = rows(vec![batch], ScanStats::default()).map_err(RepoError::App)?;
    Ok(RepoScan {
        repo: repo.name.clone(),
        commit: String::new(),
        inputs: inputs.to_owned(),
        branch,
        stats,
        facts,
        event_id: new_event_id(),
    })
}

/// One repo at its `HEAD`: every built-in's facts, verified, merged and ready to store. The files
/// come from `HEAD`'s tree, never the working tree, and every other input is in `inputs`, so the
/// facts are a function of the snapshot's key alone: a dirty or untracked file never reaches a
/// snapshot, and neither does the branch.
fn extract(
    repo: &Repo,
    options: &[(&'static str, Value)],
    inputs: &str,
    warnings: &mut Vec<Warning>,
) -> Result<RepoScan, RepoError> {
    let git = Git::new(&repo.path);
    let head = match git.head() {
        Ok(head) => head,
        Err(e) => {
            return match git.unborn_branch() {
                Some(branch) => unborn(repo, branch, inputs, warnings),
                None => Err(RepoError::Git(e)),
            };
        }
    };
    let walked = walk::walk(&git, &head.commit)?;
    // Which blobs a partial clone holds changes what the walk read, so it keys the snapshot too.
    let inputs = if walked.missing.is_empty() {
        inputs.to_owned()
    } else {
        keys::sha256_hex(&[inputs, "missing", &walked.missing.join("\n")])
    };
    let view = RepoView {
        name: repo.name.clone(),
        root: repo.path.clone(),
        commit: head.commit.clone(),
        blobs: walked.blobs.clone(),
    };
    if !walked.uncovered.is_empty() {
        warnings.push(Warning {
            code: WarningCode::PartialScan,
            message: text::get(
                "warning.partial_scan",
                &[("extractor", &files::NAME), ("repo", &repo.name), ("count", &walked.uncovered.len())],
            ),
            details: json!({
                "extractor": files::NAME,
                "repo": repo.name,
                "count": walked.uncovered.len(),
                "uncovered": walked.uncovered.iter().take(WARNING_SAMPLE).collect::<Vec<_>>(),
            }),
        });
    }
    let refs = git.refs()?;
    let objects = git.objects()?;
    let mut verifier = Verifier::new(&repo.name, &head.commit, &refs, objects);
    let mut batches: Vec<FactBatch> = Vec::new();
    let mut dropped = Vec::new();
    for ((name, globs), (_, opts)) in BUILTINS.iter().zip(options) {
        let extractor = planroom_extract::builtin(name).expect("a built-in this build has");
        let selected = select(&walked.files, globs).map_err(|e| RepoError::App(e.into()))?;
        let mut batch = extractor.run(&view, &selected, opts).map_err(|e| RepoError::App(e.into()))?;
        dropped.extend(verifier.verify(&mut batch)?);
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
            details: json!({
                "repo": repo.name,
                "count": partial.len(),
                "paths": partial.iter().take(WARNING_SAMPLE).collect::<Vec<_>>(),
            }),
        });
    }
    let stats = ScanStats {
        files: walked.files.len() as u32,
        nodes: 0,
        edges: 0,
        hints: 0,
        partial: partial.len() as u32,
    };
    let (facts, stats) = rows(batches, stats).map_err(RepoError::App)?;
    Ok(RepoScan {
        repo: repo.name.clone(),
        commit: head.commit,
        inputs,
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
