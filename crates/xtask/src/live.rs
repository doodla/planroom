//! The model variables every `cargo xtask` command sets, and `cargo xtask live record SNN`
//! (repo-layout.md, The one task entry point; architecture.md, One build-wide spend record and
//! Live spend plan; DR-49, DR-51, DR-65).
//!
//! - `PLANROOM_REPLAY=strict` for every command, so no test or check makes a live call;
//!   `live record` alone switches to `record`.
//! - `PLANROOM_SPEND_LOG`, the absolute path of the main checkout's `docs/evidence/spend.jsonl`,
//!   found through `git rev-parse --git-common-dir`, so every worktree, folder and nested process
//!   adds to the one file.
//! - For `test`, a temporary `PLANROOM_DATA` under `target/`, so no test writes your data root,
//!   and no `OPENAI_API_KEY` or `GEMINI_API_KEY`, which Planroom never reads.
//!
//! `live record SNN` needs `PLANROOM_LIVE_GATEWAY=1` or `PLANROOM_LIVE_JEV=1`. It prints its
//! estimate from the live spend plan's rows for SNN and refuses when that is more than the spend
//! log has left under `live_tests_total_usd` less the $0.25 margin; then it runs SNN's scenario
//! test in record mode, where the engine checks and appends every call under the log's lock.

use std::path::{Path, PathBuf};
use std::process::Command;

use planroom_models::spend::{MARGIN_USD, SpendLog};

/// The spend log, from the repo root of any checkout.
pub const SPEND_LOG: &str = "docs/evidence/spend.jsonl";
/// Where the live spend plan lives.
const ARCHITECTURE: &str = "docs/contributing/architecture.md";
const PLAN_HEADING: &str = "#### Live spend plan";
/// The engine's defaults, which hold the cap.
const DEFAULTS: &str = "crates/planroom/src/config/defaults.toml";
/// The variables a test run never passes on.
pub const UNSET_FOR_TESTS: &[&str] = &["OPENAI_API_KEY", "GEMINI_API_KEY"];

/// The absolute path of the main checkout's spend log, from `root`, any checkout of the repo.
pub fn spend_log_path(root: &Path) -> Result<PathBuf, String> {
    let out = Command::new("git")
        .args(["rev-parse", "--path-format=absolute", "--git-common-dir"])
        .current_dir(root)
        .output()
        .map_err(|e| format!("could not run git: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "git rev-parse --git-common-dir failed: {}",
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    let common = PathBuf::from(String::from_utf8_lossy(&out.stdout).trim());
    let main = common.parent().ok_or_else(|| format!("{} has no parent", common.display()))?;
    Ok(main.join(SPEND_LOG))
}

/// The variables every command sets, as `(name, value)`; `None` removes the variable.
pub fn variables(root: &Path, testing: bool) -> Vec<(String, Option<String>)> {
    let mut vars = vec![("PLANROOM_REPLAY".to_owned(), Some("strict".to_owned()))];
    match spend_log_path(root) {
        Ok(path) => vars.push(("PLANROOM_SPEND_LOG".to_owned(), Some(path.display().to_string()))),
        // Outside a git checkout there's no build-wide log, and live flags are refused.
        Err(_) => vars.push(("PLANROOM_SPEND_LOG".to_owned(), None)),
    }
    if testing {
        let data = root.join("target/tmp").join(format!("xtask-data-{}", std::process::id()));
        vars.push(("PLANROOM_DATA".to_owned(), Some(data.display().to_string())));
        vars.extend(UNSET_FOR_TESTS.iter().map(|n| ((*n).to_owned(), None)));
    }
    vars
}

/// One row of the live spend plan.
#[derive(Debug, Clone, PartialEq)]
pub struct PlanRow {
    pub run: String,
    /// The most one run costs: the largest dollar amount in its Each cell, `$0` when none.
    pub each_usd: f64,
}

/// The live spend plan's rows, from architecture.md.
pub fn plan(root: &Path) -> Result<Vec<PlanRow>, String> {
    let text =
        std::fs::read_to_string(root.join(ARCHITECTURE)).map_err(|e| format!("{ARCHITECTURE}: {e}"))?;
    let rows = crate::areas::table_rows(&text, PLAN_HEADING);
    let header = rows.first().ok_or_else(|| format!("{ARCHITECTURE} has no live spend plan table"))?;
    let run = header.iter().position(|c| c == "Run").ok_or("the live spend plan has no Run column")?;
    let each = header.iter().position(|c| c == "Each").ok_or("the live spend plan has no Each column")?;
    Ok(rows[1..]
        .iter()
        .filter(|r| r.len() > each.max(run))
        .map(|r| PlanRow { run: r[run].clone(), each_usd: dollars(&r[each]).into_iter().fold(0.0, f64::max) })
        .collect())
}

/// Every `$N` amount in `cell`.
fn dollars(cell: &str) -> Vec<f64> {
    cell.split('$')
        .skip(1)
        .filter_map(|rest| {
            let n: String = rest.chars().take_while(|c| c.is_ascii_digit() || *c == '.').collect();
            n.trim_end_matches('.').parse().ok()
        })
        .collect()
}

/// What one recording of `scenario` (such as `S03`) costs at most: the sum, over the plan's
/// rows that name it, of each row's per-run cost. `None` when no row names it.
pub fn estimate(rows: &[PlanRow], scenario: &str) -> Option<f64> {
    let named: Vec<&PlanRow> = rows
        .iter()
        .filter(|r| r.run.split(|c: char| !c.is_ascii_alphanumeric()).any(|word| word == scenario))
        .collect();
    (!named.is_empty()).then(|| named.iter().map(|r| r.each_usd).sum())
}

/// `live_tests_total_usd`, as the engine reads it: the default, then your user-level file, then
/// `PLANROOM_BUDGET_LIVE_TESTS_TOTAL_USD`.
pub fn cap_usd(root: &Path) -> Result<f64, String> {
    let read = |text: &str| -> Option<f64> {
        let table: toml::Table = text.parse().ok()?;
        let v = table.get("budget")?.get("live_tests_total_usd")?;
        v.as_float().or_else(|| v.as_integer().map(|i| i as f64))
    };
    let defaults = std::fs::read_to_string(root.join(DEFAULTS)).map_err(|e| format!("{DEFAULTS}: {e}"))?;
    let mut cap =
        read(&defaults).ok_or_else(|| format!("{DEFAULTS} has no [budget] live_tests_total_usd"))?;
    let absolute = |name: &str| std::env::var_os(name).map(PathBuf::from).filter(|p| p.is_absolute());
    let home = absolute("XDG_CONFIG_HOME").or_else(|| absolute("HOME").map(|h| h.join(".config")));
    if let Some(text) = home.and_then(|h| std::fs::read_to_string(h.join("planroom/config.toml")).ok())
        && let Some(v) = read(&text)
    {
        cap = v;
    }
    if let Some(v) = std::env::var("PLANROOM_BUDGET_LIVE_TESTS_TOTAL_USD").ok().and_then(|v| v.parse().ok()) {
        cap = v;
    }
    Ok(cap)
}

/// The scenario test that records `scenario`: `crates/planroom/tests/sNN_*.rs`.
fn scenario_test(root: &Path, scenario: &str) -> Result<String, String> {
    let prefix = format!("s{}_", scenario.trim_start_matches(['S', 's']));
    let dir = root.join("crates/planroom/tests");
    let mut found: Vec<String> = std::fs::read_dir(&dir)
        .map_err(|e| format!("{}: {e}", dir.display()))?
        .filter_map(|e| e.ok()?.file_name().into_string().ok())
        .filter(|n| n.starts_with(&prefix) && n.ends_with(".rs"))
        .map(|n| n.trim_end_matches(".rs").to_owned())
        .collect();
    found.sort();
    match found.as_slice() {
        [one] => Ok(one.clone()),
        [] => Err(format!("no scenario test records {scenario} yet (crates/planroom/tests/{prefix}*.rs)")),
        _ => Err(format!("more than one scenario test for {scenario}: {}", found.join(", "))),
    }
}

/// `cargo xtask live record SNN`.
pub fn record(root: &Path, scenario: &str) -> Result<String, String> {
    let scenario = scenario.to_ascii_uppercase();
    if !scenario.starts_with('S') || scenario.len() != 3 || !scenario[1..].bytes().all(|b| b.is_ascii_digit())
    {
        return Err(format!("live record takes a scenario such as S03, not {scenario}"));
    }
    let flagged =
        ["PLANROOM_LIVE_GATEWAY", "PLANROOM_LIVE_JEV"].iter().any(|f| std::env::var(f).as_deref() == Ok("1"));
    if !flagged {
        return Err("live record makes paid calls: set PLANROOM_LIVE_GATEWAY=1 (or PLANROOM_LIVE_JEV=1) to allow them".into());
    }
    let log_path = std::env::var_os("PLANROOM_SPEND_LOG")
        .map(PathBuf::from)
        .ok_or("PLANROOM_SPEND_LOG isn't set: run this from a git checkout of Planroom")?;
    let log = SpendLog::open(&log_path).map_err(|e| format!("{}: {e}", log_path.display()))?;
    let cap = cap_usd(root)?;
    let spent = log.total().map_err(|e| format!("{}: {e}", log_path.display()))?;
    let left = cap - MARGIN_USD - spent;
    let estimate = estimate(&plan(root)?, &scenario)
        .ok_or_else(|| format!("the live spend plan in {ARCHITECTURE} has no row for {scenario}"))?;
    println!(
        "live record {scenario}: estimate ${estimate:.2}; spent ${spent:.4} of ${cap:.2} less the ${MARGIN_USD:.2} margin, so ${left:.4} left"
    );
    if estimate > left {
        return Err(format!(
            "the estimate ${estimate:.2} is more than the ${left:.4} the spend log has left"
        ));
    }
    let test = scenario_test(root, &scenario)?;
    let status = Command::new("cargo")
        .args(["test", "--locked", "-p", "planroom", "--test", &test])
        .current_dir(root)
        .env("PLANROOM_REPLAY", "record")
        .status()
        .map_err(|e| format!("could not run cargo test: {e}"))?;
    let after = log.total().map_err(|e| format!("{}: {e}", log_path.display()))?;
    let line = format!("{scenario} spent ${:.4}; the spend log now totals ${after:.4}", after - spent);
    if status.success() { Ok(line) } else { Err(format!("cargo test --test {test} failed; {line}")) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_plan_estimates_each_scenario_it_names() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).ancestors().nth(2).unwrap();
        let rows = plan(root).unwrap();
        assert!(rows.iter().any(|r| r.run.starts_with("S03")), "{rows:?}");
        let s03 = estimate(&rows, "S03").unwrap();
        assert!((s03 - 0.15).abs() < 1e-9, "the gateway run's $0.15 and the session run's $0: {s03}");
        assert!((estimate(&rows, "S12").unwrap() - 0.01).abs() < 1e-9);
        assert_eq!(estimate(&rows, "S99"), None);
        assert_eq!(dollars("$0.02 to $0.05 per pass"), vec![0.02, 0.05]);
    }

    /// Runs git in `dir` with a fixed identity, failing the test when git does.
    fn git(dir: &Path, args: &[&str]) {
        let out = Command::new("git")
            .args(args)
            .current_dir(dir)
            .env("GIT_AUTHOR_NAME", "Test")
            .env("GIT_AUTHOR_EMAIL", "test@example.com")
            .env("GIT_COMMITTER_NAME", "Test")
            .env("GIT_COMMITTER_EMAIL", "test@example.com")
            .output()
            .unwrap();
        assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    }

    #[test]
    fn every_worktree_names_the_main_checkouts_spend_log() {
        // A main checkout and a linked worktree of it, built here so the check runs in any clone.
        let unique = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let base = std::env::temp_dir().join(format!("xtask-spend-log-{}-{unique}", std::process::id()));
        let main = base.join("main");
        std::fs::create_dir_all(main.join("docs/evidence")).unwrap();
        std::fs::write(main.join(SPEND_LOG), "").unwrap();
        git(&main, &["init", "-q", "-b", "main"]);
        git(&main, &["add", "-A"]);
        git(&main, &["commit", "-q", "-m", "init"]);
        let linked = base.join("linked");
        git(&main, &["worktree", "add", "-q", "-b", "other", linked.to_str().unwrap()]);

        let expected = main.canonicalize().unwrap().join(SPEND_LOG);
        let named = [&main, &main.join("docs/evidence"), &linked, &linked.join("docs/evidence")]
            .map(|dir| spend_log_path(dir).unwrap());
        let resolved = named.clone().map(|p| p.canonicalize().unwrap_or(p));
        let _ = std::fs::remove_dir_all(&base);
        for (path, resolved) in named.iter().zip(&resolved) {
            assert!(path.is_absolute(), "{}", path.display());
            assert_eq!(resolved, &expected, "every folder of every worktree names the main checkout's log");
        }
    }
}
