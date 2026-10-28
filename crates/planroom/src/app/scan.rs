//! `planroom scan DIR`: discover the workspace, extract every repo at its `HEAD`, verify the
//! evidence, and commit every repo's snapshot and `scan` event in one transaction.
//!
//! Each repo stands alone: one git can't read is left out with a `repo_skipped` warning, one with
//! no commit yet maps nothing until its first commit, with `repo_unborn`, and the rest of the
//! workspace maps.
//!
//! The commit asks each of `app::cards`' scan dealers for a card, in the scan's own transaction.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use planroom_contract::errors::WARNING_SAMPLE;
use planroom_contract::{
    Actor, Client, Command, CommandName, FACTS_FORMAT, FactBatch, Handle, Rename, ScanReq, ScanResult,
    ScanStats, Seq, Surface, Tier, Warning, WarningCode,
};
use planroom_extract::git::{Git, GitError};
use planroom_extract::verify::Verifier;
use planroom_extract::{RepoView, files, select, walk};
use planroom_kernel::{facts, keys, redact};
use planroom_store::{FactRow, Init, RepoOutcome, RepoScan, ScanKey, ScanRules, ScanWrite, Shape, Store};
use serde_json::{Value, json};

use crate::app::dirty::Dirty;
use crate::app::error::AppError;
use crate::app::workspace::{self, Repo, Workspace};
use crate::app::{Paged, Reply, map, models, new_event_id};
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

/// A scan, with the repos whose uncommitted work it didn't map: it reads `HEAD`'s tree, never the
/// working tree, so its reply says what that left out (DR-71, DR-85).
pub fn run(req: &ScanReq, cwd: &Path) -> Result<Paged<ScanResult>, AppError> {
    let prepared = prepare(req, cwd)?;
    let dirty = Dirty::of(&prepared.ws);
    Ok(Paged { dirty, ..Paged::whole(commit(prepared)?) })
}

/// Discover the workspace and extract each repo at its `HEAD` with the built-in packs. Nothing is
/// written yet.
pub fn prepare(req: &ScanReq, cwd: &Path) -> Result<Prepared, AppError> {
    // A workspace's own packs come from its config, which loads only once the workspace is
    // trusted, so none loads yet.
    prepare_with(req, cwd, packs::builtins()?)
}

/// [`prepare`] with the pack set `packs`: a scan runs every extractor the set declares, built-in
/// and declarative alike.
pub fn prepare_with(req: &ScanReq, cwd: &Path, packs: Loaded) -> Result<Prepared, AppError> {
    // A scan's decision run may call a model, so the live gate is read first: a relative spend
    // log, or a live flag with none, stops the scan before it reads anything (DR-65).
    models::live_env()?;
    let dir = req.dir.as_deref().map_or_else(|| cwd.to_path_buf(), |d| cwd.join(d));
    let ws = workspace::discover(&dir)?;
    if ws.repos.is_empty() {
        return Err(AppError::workspace_not_found(&ws.root));
    }
    // Read before any repo, so a scan that commits meanwhile is newer than this one; with it, the
    // commit each repo's active snapshot was read at, which its renames are reported from.
    let (started, mapped) = match Store::open_existing(&ws.store_path())? {
        Some(store) => {
            let snapshot = store.snapshot()?;
            (snapshot.seq(), mapped_commits(&snapshot)?)
        }
        None => (0, BTreeMap::new()),
    };
    let mut warnings = ws.warnings.clone();
    // A workspace's own config loads only from a state directory the data root records as
    // trusted (DR-48); otherwise only the user-level file and the variables apply.
    let trusted = crate::app::trust::check(&ws) == crate::app::trust::Trust::Trusted;
    let loaded = config::for_workspace(&ws.state_dir(), trusted)?;
    warnings.extend(loaded.warnings);
    let exclude = loaded.config.scan.exclude;
    let globs = walk::Exclude::new(&exclude)?;
    let names: BTreeSet<&str> = ws.repos.iter().map(|r| r.name.as_str()).collect();
    let runs = packs.runs(&names);
    let inputs = inputs_hash(&packs.hash, &runs, &exclude);
    let prefixes = id_prefixes(&runs);
    let mut repos = Vec::with_capacity(ws.repos.len());
    for repo in &ws.repos {
        let since = mapped.get(&repo.name).map(String::as_str);
        match extract(repo, &runs, &globs, &inputs, since, prefixes.as_ref(), &mut warnings) {
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
    // With nothing to write, a workspace with no store stays without one: a repo with no commit
    // yet writes nothing (DR-86).
    let store = if repos.iter().all(|r| r.commit.is_empty()) {
        Store::open_existing(&ws.store_path())?.map(|mut store| {
            crate::app::hook(&mut store);
            store
        })
    } else {
        Some(open_store(&ws)?)
    };
    // Each mapped root's namespace, which the commit fixes at the root's first scan (DR-74).
    let namespaces: BTreeMap<String, String> = ws
        .repos
        .iter()
        .filter(|r| repos.iter().any(|s| s.repo == r.name && !s.commit.is_empty()))
        .map(|r| (r.rel.clone(), r.name.clone()))
        .collect();
    let write = ScanWrite {
        at: clock::now(),
        actor: Actor::System { part: "scan".into() },
        client: client(),
        packs_hash: packs.hash.clone(),
        repos,
        started,
        namespaces,
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
    // The cards a scan deals go in its own transaction, read from the store as it commits (DR-89).
    let dealt_at = write.at.clone();
    // The trust dealer reads how trust stands once this scan made the state directory, and the
    // reply says while the workspace's trust card is open (DR-89).
    let state_dir = ws.state_dir();
    let mut deal = |snapshot: &planroom_store::Snapshot<'_>| {
        let trust = crate::app::trust::check(&ws);
        let cx = crate::app::cards::ScanCx { root: &ws.root, state_dir: &state_dir, trust };
        let dealt = crate::app::cards::deal_for_scan(snapshot, &cx, &dealt_at)?;
        warnings.retain(|w| w.code != WarningCode::WorkspaceUntrusted);
        let card = dealt.iter().find_map(|d| match &d.event.body {
            planroom_contract::EventBody::Card(card) if card.kind.0 == crate::app::cards::TRUST => {
                Some((d.event.id.clone(), card.clone()))
            }
            _ => None,
        });
        let open = crate::app::cards::open_trust(snapshot)?.map(|o| (o.event.id, o.card));
        if let Some((id, card)) = card.or(open) {
            warnings.push(crate::app::cards::untrusted_warning(&id, &card));
        }
        Ok::<_, AppError>(dealt)
    };
    let done = store.commit_scan_dealing(
        &write,
        RULES,
        &mut |repo, newer_scan| {
            if !paths.get(repo.repo.as_str()).is_some_and(|p| is_head(p, &repo.commit)) {
                why.insert(repo.repo.clone(), "head_moved");
                return false;
            }
            if newer_scan
                && !*same_repos.get_or_insert_with(|| {
                    current_inputs(&ws.root, &packs, &exclude).as_deref() == Some(&inputs)
                })
            {
                why.insert(repo.repo.clone(), "repos_changed");
                return false;
            }
            true
        },
        &mut deal,
    )?;
    let superseded =
        |o: &RepoOutcome| matches!(o, RepoOutcome::Superseded { .. } | RepoOutcome::NamespaceTaken { .. });
    // The commit each superseded repo's map stays at: what the newer scan mapped.
    let mapped = if done.repos.iter().any(|(_, o)| superseded(o)) {
        mapped_commits(&store.snapshot()?)?
    } else {
        BTreeMap::new()
    };
    for ((name, outcome), repo) in done.repos.iter().zip(&write.repos) {
        if superseded(outcome) {
            // A namespace another scan fixed otherwise means that scan read another repo set.
            let reason = match outcome {
                RepoOutcome::NamespaceTaken { .. } => "repos_changed",
                _ => why.get(name).copied().unwrap_or("head_moved"),
            };
            let held = mapped.get(name).cloned().unwrap_or_default();
            // When HEAD moved and a newer scan already mapped it, there's nothing to scan again;
            // when the repo set changed and no scan mapped this name, nothing holds it.
            let key = match reason {
                "head_moved" if paths.get(name.as_str()).is_some_and(|p| is_head(p, &held)) => {
                    "head_moved_mapped"
                }
                "repos_changed" if held.is_empty() => "repos_changed_unmapped",
                other => other,
            };
            warnings.push(Warning {
                code: WarningCode::ScanSuperseded,
                message: text::get(
                    &format!("warning.scan_superseded_{key}"),
                    &[("repo", name), ("commit", &short(&repo.commit)), ("mapped", &short(&held))],
                ),
                details: json!({"repo": name, "commit": repo.commit, "reason": reason, "mapped": held}),
            });
        }
    }
    let recorded = done.repos.iter().filter(|(_, o)| matches!(o, RepoOutcome::Recorded { .. })).count();
    // The reply's map, its container count and its seq come from one read transaction, as map's
    // do (DR-73): another writer that commits after this scan shows in all of them or none, so
    // the map never holds an event past the reply's `meta.seq`.
    let snapshot = store.snapshot()?;
    let seq = snapshot.seq();
    let mut view = map::top_in(&snapshot, &ws.root, None)?;
    let containers = map::top_in(&snapshot, &ws.root, Some(planroom_contract::Level::Container))?.nodes.len();
    drop(snapshot);
    // The top-level map an agent reads in scan's reply keeps to scan's budget, as map's does.
    let budget = Command::Scan(ScanReq::default()).spec().budget_tokens;
    let about = map::lens::kinds(crate::packs::vocabulary()?, &view.lens).to_vec();
    let cut = map::fit_beside(&mut view, budget, None, &warnings, &about);
    warnings.extend(cut);
    let summary = summary(&write, containers, recorded);
    Ok(Reply { result: ScanResult { map: view, next: next() }, seq, warnings, text: summary })
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
    Handle { command: CommandName("map".into()), args: vec![], label: text::label("scan.next_label", &[]) }
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
    // A declarative extractor's inputs are its pack's files, which the packs hash already holds.
    let options: Vec<String> = runs
        .iter()
        .filter_map(|run| Some(format!("{}={}", run.name, keys::canonical(run.builtin()?.1))))
        .collect();
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
        renames: Vec::new(),
        stats,
        facts,
        event_id: new_event_id(),
    })
}

/// One repo at its `HEAD`: every extractor's facts, verified, merged and ready to store. The files
/// come from `HEAD`'s tree, never the working tree, and every other input is in `inputs`, so the
/// facts are a function of the snapshot's key alone: a dirty or untracked file never reaches a
/// snapshot, and neither does the branch.
///
/// `since` is the commit the repo's active snapshot was read at: the renames git reports from
/// there to `HEAD` go on the scan event, as IDs of the `files` built-in's kinds (DR-74).
fn extract(
    repo: &Repo,
    runs: &[Run],
    exclude: &walk::Exclude,
    inputs: &str,
    since: Option<&str>,
    prefixes: Option<&files::Kinds>,
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
    if !walked.unnamed.is_empty() {
        warnings.push(Warning {
            code: WarningCode::UnnamedPaths,
            message: text::get(
                "warning.unnamed_paths",
                &[("count", &walked.unnamed.len()), ("repo", &repo.name)],
            ),
            details: json!({
                "repo": repo.name,
                "count": walked.unnamed.len(),
                "paths": walked.unnamed.iter().take(WARNING_SAMPLE).collect::<Vec<_>>(),
            }),
        });
    }
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
        let globs: Vec<&str> = run.files.iter().map(String::as_str).collect();
        let selected = select(&walked.files, &globs).map_err(|e| RepoError::App(e.into()))?;
        let mut batch = run.extract(&view, &selected).map_err(|e| RepoError::App(e.into()))?;
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
    // A file read with errors is `partial_parse`; one a bound refused gave no facts and wasn't
    // parsed or queried in full, so it is `unparsed_files`, one warning per bound.
    let mut read_with_errors: BTreeSet<&str> = BTreeSet::new();
    let mut refused: BTreeMap<String, BTreeSet<&str>> = BTreeMap::new();
    for p in batches.iter().flat_map(|b| &b.partial) {
        match p.reason {
            None => {
                read_with_errors.insert(p.path.as_str());
            }
            Some(reason) => {
                let bound = serde_json::to_value(reason).ok().and_then(|v| v.as_str().map(str::to_owned));
                refused.entry(bound.unwrap_or_default()).or_default().insert(p.path.as_str());
            }
        }
    }
    if !read_with_errors.is_empty() {
        warnings.push(Warning {
            code: WarningCode::PartialParse,
            message: text::get(
                "warning.partial_parse",
                &[("count", &read_with_errors.len()), ("repo", &repo.name)],
            ),
            details: json!({
                "repo": repo.name,
                "count": read_with_errors.len(),
                "paths": read_with_errors.iter().take(WARNING_SAMPLE).collect::<Vec<_>>(),
            }),
        });
    }
    for (bound, paths) in &refused {
        // Repo names and paths come from the repo, so they pass redaction.
        let shown = redact::text(&repo.name);
        warnings.push(Warning {
            code: WarningCode::UnparsedFiles,
            message: text::get(
                &format!("warning.unparsed_files_{bound}"),
                &[("count", &paths.len()), ("repo", &shown)],
            ),
            details: json!({
                "repo": shown,
                "count": paths.len(),
                "bound": bound,
                "paths": paths.iter().take(WARNING_SAMPLE).map(|p| redact::text(p)).collect::<Vec<_>>(),
            }),
        });
    }
    let stats = ScanStats {
        files: walked.files.len() as u32,
        nodes: 0,
        edges: 0,
        hints: 0,
        partial: partial.len() as u32,
        uncovered: walked.uncovered.len() as u32,
    };
    let (facts, stats) = rows(batches, stats).map_err(RepoError::App)?;
    // Only a rename to a node this snapshot holds is recorded: an edit can follow nothing else,
    // and so the event stays within the walk's bounds however many files git saw move.
    let mut renames = match (since, prefixes) {
        (Some(old), Some(kinds)) if !old.is_empty() && old != head.commit => {
            renames(&git, &repo.name, old, &head.commit, kinds)
        }
        _ => Vec::new(),
    };
    renames.retain(|r| facts.binary_search_by(|f| f.id.as_str().cmp(&r.to.0)).is_ok());
    Ok(RepoScan {
        repo: repo.name.clone(),
        commit: head.commit,
        inputs,
        branch: head.branch.unwrap_or_default(),
        renames,
        stats,
        facts,
        event_id: new_event_id(),
    })
}

/// The commit each repo's active snapshot was read at, by repo, from its last `scan` event. A
/// repo whose last event names no commit, as an older store's unborn repo does, is left out.
fn mapped_commits(snapshot: &planroom_store::Snapshot<'_>) -> Result<BTreeMap<String, String>, AppError> {
    Ok(snapshot
        .active_scans()?
        .into_iter()
        .filter_map(|e| match e.body {
            planroom_contract::EventBody::Scan { repo, commit, .. } if !commit.is_empty() => {
                Some((repo, commit))
            }
            _ => None,
        })
        .collect())
}

/// The kinds the `files` built-in emits, with their ID prefixes, as this scan's packs name them:
/// what a renamed file's or directory's IDs are made of. `None` when no pack declares `files`.
fn id_prefixes(runs: &[Run]) -> Option<files::Kinds> {
    let (_, options) = runs.iter().filter_map(Run::builtin).find(|(name, _)| *name == "files")?;
    serde_json::from_value(options.get("kinds")?.clone()).ok()
}

/// The renames git reports in `repo` from `old` to `new`, as old and new node IDs: each file, then
/// each wholly moved directory. Renames are a convenience for edits that follow their node, so
/// when git can't say, as when `old` is gone from a rewritten history or a partial clone lacks a
/// blob it would compare, the scan records none and such an edit is listed as vanished.
fn renames(git: &Git, ns: &str, old: &str, new: &str, kinds: &files::Kinds) -> Vec<Rename> {
    let Ok(moved) = git.renames(old, new) else { return Vec::new() };
    let id = |prefix: &str, path: &str| planroom_extract::node_id(ns, prefix, path);
    let files = moved.files.iter().map(|(from, to)| (&kinds.file.id_prefix, from, to));
    let dirs = moved.dirs.iter().map(|(from, to)| (&kinds.directory.id_prefix, from, to));
    files
        .chain(dirs)
        .map(|(prefix, from, to)| Rename { from: id(prefix, from), to: id(prefix, to) })
        .collect()
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
