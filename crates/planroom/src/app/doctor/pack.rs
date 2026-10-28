//! `planroom doctor --pack DIR`: one pack's conformance kit (architecture.md, "What a pack is";
//! contracts.md, "Pack test layout"; DR-10).
//!
//! The kit validates the pack with the built-in packs, as a scan would load it: as an out-of-tree
//! pack, since it was found on a path. A directory whose files are a built-in pack's own, file for
//! file but its tests, checks that built-in; one that claims a built-in's ID with other files is
//! refused, so no directory takes a built-in's ring by naming itself after one. It then runs each of the pack's extractors over every repo
//! under `tests/inputs/`, compares the facts with `tests/expected.jsonl` (or rewrites it with
//! `--update`), runs them again to prove the same bytes come out, checks every line of evidence
//! against its input file, and reads each eval case file. The first check that fails fails the
//! command with `pack_invalid`.
//!
//! The inputs aren't a git repo, so their facts name a fixed commit, all zeros.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::Path;

use planroom_contract::{Evidence, FactBatch, PackCheck, PackReport};
use planroom_extract::walk::Blobs;
use planroom_extract::{RepoPath, RepoView, select};

use crate::app::Reply;
use crate::app::error::AppError;
use crate::packs::{self, Loaded, Origin, Run, embed, load};
use crate::text;

/// The commit the conformance inputs' facts name.
pub const INPUTS_COMMIT: &str = "0000000000000000000000000000000000000000";
const INPUTS: &str = "tests/inputs";
const EXPECTED: &str = "tests/expected.jsonl";
const EVALS: &str = "evals";

pub fn run(dir: &Path, update: bool) -> Result<Reply<PackReport>, AppError> {
    let source = packs::read_dir(dir)?;
    let id = load::peek_id(&source).unwrap_or_default();
    let mut sources = embed::sources();
    // The ring comes from where a pack was found, never from the ID it claims. A directory
    // holding a built-in pack's own files, file for file (its tests aside), is that built-in, so
    // the embedded one is checked; any other directory under a built-in's ID is refused.
    match sources.iter().find(|s| load::peek_id(s).as_deref() == Some(id.as_str())) {
        Some(embedded) if embedded.files == source.files => {}
        Some(_) => {
            let at = load::KeyPath::root().key("id");
            return Err(source.invalid_at(&at, load::At::Value, &id, "builtin_changed", &[("id", &id)]));
        }
        None => sources.push(source),
    }
    let loaded = packs::resolve(sources)?;
    // Every declarative extractor compiles here, whether or not the pack has inputs to run, so a
    // query that doesn't compile is `pack_invalid` from `doctor --pack` as from a scan.
    loaded.programs()?;
    let pack = loaded.pack(&id).expect("the pack just loaded");
    let file = &pack.file;
    let qualified =
        |names: Vec<&String>| names.into_iter().map(|n| packs::qualify(&id, n)).collect::<Vec<_>>();
    let extractors: Vec<String> =
        loaded.extractors.iter().filter(|e| e.pack == id).map(|e| e.name.clone()).collect();
    let mut checks = vec![PackCheck {
        check: "validate".into(),
        line: text::get(
            "pack.check_validate",
            &[
                ("kinds", &file.kinds.len()),
                ("relations", &file.relations.len()),
                ("extractors", &file.extractors.len()),
                ("lenses", &file.lenses.len()),
            ],
        ),
    }];
    conformance(dir, &id, &loaded, update, &mut checks)?;
    checks.push(evals(dir)?);
    let summary =
        text::get("pack.summary", &[("id", &id), ("version", &file.version), ("count", &checks.len())]);
    let report = PackReport {
        summary: summary.clone(),
        id: id.clone(),
        version: file.version,
        origin: match pack.origin {
            Origin::BuiltIn => "builtin".into(),
            Origin::OutOfTree => "out_of_tree".into(),
        },
        sha256: pack.sha256.clone(),
        kinds: qualified(file.kinds.iter().map(|k| &k.id).collect()),
        relations: qualified(file.relations.iter().map(|r| &r.id).collect()),
        extractors,
        linkers: file.linkers.iter().map(|l| format!("{id}/{}", l.builtin)).collect(),
        lenses: file.lenses.iter().map(|l| l.id.clone()).collect(),
        checks,
    };
    let text = std::iter::once(summary)
        .chain(report.checks.iter().map(|c| format!("  {:<12} {}", c.check, c.line)))
        .collect::<Vec<_>>()
        .join("\n");
    Ok(Reply { result: report, seq: 0, warnings: vec![], text })
}

/// One input repo: its name and its files.
struct Input {
    name: String,
    view: RepoView,
    files: Vec<RepoPath>,
}

/// Every extractor of pack `id` over `tests/inputs/`, twice, against `tests/expected.jsonl`.
fn conformance(
    dir: &Path,
    id: &str,
    loaded: &Loaded,
    update: bool,
    checks: &mut Vec<PackCheck>,
) -> Result<(), AppError> {
    let shown = |rel: &str| dir.join(rel).display().to_string();
    let inputs_dir = dir.join(INPUTS);
    if !inputs_dir.is_dir() {
        checks.push(PackCheck { check: "expected".into(), line: text::get("pack.check_no_inputs", &[]) });
        return Ok(());
    }
    let inputs = read_inputs(&inputs_dir)?;
    let names: BTreeSet<&str> = inputs.iter().map(|i| i.name.as_str()).collect();
    let prefix = format!("{id}/");
    let runs: Vec<Run> = loaded.runs(&names)?.into_iter().filter(|r| r.name.starts_with(&prefix)).collect();
    let first = extract(&inputs, &runs)?;
    let second = extract(&inputs, &runs)?;
    for ((repo, name, a), (_, _, b)) in first.iter().zip(&second) {
        if serde_json::to_string(a).ok() != serde_json::to_string(b).ok() {
            let file = shown(&format!("{INPUTS}/{repo}"));
            return Err(AppError::pack_invalid(
                &file,
                1,
                name,
                "not_deterministic",
                &[("name", name), ("repo", repo)],
            ));
        }
    }
    let lines: Vec<String> = first
        .iter()
        .map(|(_, _, batch)| serde_json::to_string(batch).map_err(AppError::internal))
        .collect::<Result<_, _>>()?;
    let expected_file = dir.join(EXPECTED);
    let vars: [(&str, &dyn std::fmt::Display); 2] = [("batches", &lines.len()), ("repos", &inputs.len())];
    if update {
        let mut body = lines.join("\n");
        body.push('\n');
        fs::write(&expected_file, body).map_err(AppError::internal)?;
        checks.push(PackCheck { check: "expected".into(), line: text::get("pack.check_updated", &vars) });
    } else {
        let Ok(expected) = fs::read_to_string(&expected_file) else {
            return Err(AppError::pack_invalid(&shown(EXPECTED), 1, EXPECTED, "expected_missing", &[]));
        };
        let expected: Vec<&str> = expected.lines().collect();
        let differs = (0..lines.len().max(expected.len()))
            .find(|&n| lines.get(n).map(String::as_str) != expected.get(n).copied());
        if let Some(n) = differs {
            let line = n as u32 + 1;
            return Err(AppError::pack_invalid(
                &shown(EXPECTED),
                line,
                EXPECTED,
                "expected_differs",
                &[("line", &line)],
            ));
        }
        checks.push(PackCheck { check: "expected".into(), line: text::get("pack.check_expected", &vars) });
    }
    checks.push(PackCheck { check: "determinism".into(), line: text::get("pack.check_determinism", &[]) });
    let count = evidence(&inputs, &first, &shown)?;
    checks.push(PackCheck {
        check: "evidence".into(),
        line: text::get("pack.check_evidence", &[("count", &count)]),
    });
    Ok(())
}

/// Each repo directory under `tests/inputs/`, in name order, with every file under it.
fn read_inputs(dir: &Path) -> Result<Vec<Input>, AppError> {
    let mut repos: Vec<_> = fs::read_dir(dir)
        .map_err(AppError::internal)?
        .flatten()
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    repos.sort();
    let mut inputs = Vec::new();
    for root in repos {
        let name = root.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let mut blobs = BTreeMap::new();
        let mut todo = vec![root.clone()];
        while let Some(at) = todo.pop() {
            for entry in fs::read_dir(&at).map_err(AppError::internal)?.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    todo.push(path);
                } else if path.is_file() {
                    let rel = path.strip_prefix(&root).unwrap_or(&path).to_string_lossy().replace('\\', "/");
                    blobs.insert(rel, fs::read(&path).map_err(AppError::internal)?);
                }
            }
        }
        let files = blobs.keys().cloned().map(RepoPath).collect();
        let view =
            RepoView { name: name.clone(), root, commit: INPUTS_COMMIT.into(), blobs: Blobs::new(blobs) };
        inputs.push(Input { name, view, files });
    }
    Ok(inputs)
}

/// Every run over every input repo: `(repo, extractor, facts)`, repos in order, then extractors.
fn extract(inputs: &[Input], runs: &[Run]) -> Result<Vec<(String, String, FactBatch)>, AppError> {
    let mut out = Vec::new();
    for input in inputs {
        for run in runs {
            let globs: Vec<&str> = run.files.iter().map(String::as_str).collect();
            let selected = select(&input.files, &globs)?;
            let batch = run.extract(&input.view, &selected).map_err(|e| {
                let why = e.to_string();
                AppError::pack_invalid(
                    &input.view.root.display().to_string(),
                    1,
                    &run.name,
                    "extract_failed",
                    &[("name", &run.name), ("why", &why)],
                )
            })?;
            out.push((input.name.clone(), run.name.clone(), batch));
        }
    }
    Ok(out)
}

/// Every `Line` evidence item against its input file, as ingest checks it against a blob.
fn evidence(
    inputs: &[Input],
    batches: &[(String, String, FactBatch)],
    shown: &dyn Fn(&str) -> String,
) -> Result<usize, AppError> {
    let mut count = 0;
    for (_, _, batch) in batches {
        let facts = batch
            .nodes
            .iter()
            .map(|n| (n.id.0.as_str(), &n.evidence))
            .chain(batch.edges.iter().map(|e| (e.id.0.as_str(), &e.evidence)))
            .chain(batch.hints.iter().map(|h| (h.id.as_str(), &h.evidence)));
        for (fact, evidence) in facts {
            for item in evidence {
                let Evidence::Line { path, line, contains: Some(contains), .. } = item else { continue };
                let holds = path.split_once('/').is_some_and(|(repo, rel)| {
                    inputs
                        .iter()
                        .find(|i| i.name == repo)
                        .and_then(|i| i.view.blobs.get(rel))
                        .and_then(|b| std::str::from_utf8(b).ok())
                        .is_some_and(|t| planroom_extract::verify::line_holds(t, *line, contains))
                });
                if !holds {
                    let file = shown(&format!("{INPUTS}/{path}"));
                    return Err(AppError::pack_invalid(
                        &file,
                        *line,
                        fact,
                        "evidence",
                        &[("fact", &fact), ("line", line), ("path", path)],
                    ));
                }
                count += 1;
            }
        }
    }
    Ok(count)
}

/// Each `evals/*.cases.jsonl` file: every line a JSON object.
fn evals(dir: &Path) -> Result<PackCheck, AppError> {
    let mut files: Vec<_> = fs::read_dir(dir.join(EVALS))
        .map(|entries| entries.flatten().map(|e| e.path()).collect())
        .unwrap_or_default();
    files.retain(|p| p.to_string_lossy().ends_with(".cases.jsonl"));
    files.sort();
    for path in &files {
        let body = fs::read_to_string(path).map_err(AppError::internal)?;
        for (n, line) in body.lines().enumerate().filter(|(_, l)| !l.trim().is_empty()) {
            if !serde_json::from_str::<serde_json::Value>(line).is_ok_and(|v| v.is_object()) {
                let at = n as u32 + 1;
                let file = path.display().to_string();
                return Err(AppError::pack_invalid(&file, at, &file, "eval_line", &[("line", &at)]));
            }
        }
    }
    Ok(PackCheck { check: "evals".into(), line: text::get("pack.check_evals", &[("count", &files.len())]) })
}
