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
use planroom_extract::{RepoView, files, select, walk};
use planroom_kernel::{facts, keys};
use planroom_store::{FactRow, Init, RepoOutcome, RepoScan, ScanKey, ScanRules, ScanWrite, Shape, Store};
use serde_json::{Value, json};

use crate::app::error::AppError;
use crate::app::workspace::{self, Repo, Workspace};
use crate::app::{Reply, map, new_event_id};
use crate::packs::{self, Loaded, Run};
use crate::{clock, config, text};

const RULES: ScanRules = ScanRules {
    scan_key: |k: &ScanKey<'_>| keys::scan_key(k.repo, k.commit, k.inputs, k.previous),
    link_key: keys::link_key,
};

/// A scan between extraction and commit: every repo git could read, at the `HEAD` it read.
pub struct Prepared {
    ws: Workspace,
    /// The pack set this scan extracted with.
    packs: Loaded,
    /// The `[scan] exclude` globs this scan walked with.
    exclude: Vec<String>,
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
    // A workspace's own config loads only once the workspace is trusted, and no workspace is
    // recorded as trusted yet, so only the user-level file and the variables apply.
    let loaded = config::for_workspace(&ws.state_dir(), false)?;
    warnings.extend(loaded.warnings);
    let exclude = loaded.config.scan.exclude;
    let globs = walk::Exclude::new(&exclude)?;
    // The built-in packs. A workspace's own packs come from its config, which loads only once
    // the workspace is trusted, so none loads yet.
    let packs = packs::builtins()?;
    let names: BTreeSet<&str> = ws.repos.iter().map(|r| r.name.as_str()).collect();
    let runs = packs.runs(&names);
    let inputs = inputs_hash(&packs.hash, &runs, &exclude);
    let mut repos = Vec::with_capacity(ws.repos.len());
    for repo in &ws.repos {
        match extract(repo, &runs, &globs, &inputs, &mut warnings) {
            Ok(scan) => repos.push(scan),
            Err(RepoError::Git(e)) => warnings.push(skipped(repo, &e)),
            Err(RepoError::App(e)) => return Err(e),
        }
    }
    Ok(Prepared { ws, packs, exclude, inputs, started, repos, warnings })
}

/// Commit a prepared scan. A repo stays where it is when its `HEAD` moved on since it was read, or
/// when another scan moved it meanwhile and the workspace's repos changed since this scan read
/// them: a late scan never moves a repo back, whatever input changed.
pub fn commit(prepared: Prepared) -> Result<Reply<ScanResult>, AppError> {
    let Prepared { ws, packs, exclude, inputs, started, repos, mut warnings } = prepared;
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
        packs_hash: packs.hash.clone(),
        repos,
        started,
    };
    let Some(mut store) = store else {
        let view = map::not_scanned_view(&ws.root)?;
        let summary = summary(&write, 0, 0);
        return Ok(Reply { result: ScanResult { map: view, next: next() }, seq: 0, warnings, text: summary });
    };
    let records = serde_json::to_string(&packs.records()).map_err(AppError::internal)?;
    store.meta_insert_once("packs", &records)?;
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
            && !*same_repos
                .get_or_insert_with(|| current_inputs(&ws.root, &packs, &exclude).as_deref() == Some(&inputs))
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

/// Open the workspace's store, creating the state directory and the store on the first scan.
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

/// The hash of every extraction input besides a repo's commit: the packs hash, each built-in's
/// options and the `[scan] exclude` globs. It keys the snapshot with the repo and the commit, as
/// the packs hash alone did, so a snapshot's facts are a function of its key (DR-85, DR-89):
/// moving a repo out of the workspace and back gives the map a fresh store gives, and so does
/// changing the excludes and back.
fn inputs_hash(packs_hash: &str, runs: &[Run], exclude: &[String]) -> String {
    let options: Vec<String> =
        runs.iter().map(|run| format!("{}={}", run.name, keys::canonical(&run.options))).collect();
    let exclude = keys::canonical(&json!(exclude));
    keys::sha256_hex(&["planroom.extract-inputs/v1", packs_hash, &options.join("\n"), "exclude", &exclude])
}

/// The extraction inputs the workspace at `root` gives now, with the excludes this scan read, as
/// [`prepare`] derives them: `None` when its repos can't be listed.
fn current_inputs(root: &Path, packs: &Loaded, exclude: &[String]) -> Option<String> {
    let ws = workspace::discover(root).ok()?;
    let names: BTreeSet<&str> = ws.repos.iter().map(|r| r.name.as_str()).collect();
    Some(inputs_hash(&packs.hash, &packs.runs(&names), exclude))
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
    runs: &[Run],
    exclude: &walk::Exclude,
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
    let walked = walk::walk(&git, &head.commit, exclude)?;
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
    for run in runs {
        // The loader refused a pack naming a built-in this build doesn't have.
        let extractor = planroom_extract::builtin(&run.builtin).expect("a built-in this build has");
        let globs: Vec<&str> = run.files.iter().map(String::as_str).collect();
        let selected = select(&walked.files, &globs).map_err(|e| RepoError::App(e.into()))?;
        let mut batch =
            extractor.run(&view, &selected, &run.options).map_err(|e| RepoError::App(e.into()))?;
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
