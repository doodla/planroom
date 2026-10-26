//! One `ok`, `FAILED` or `skipped (why)` line per step, and a summary at the end.
//!
//! Under `CI=true` a `skipped` step counts as `FAILED`, unless `crates/xtask/ci_skips.toml` lists
//! it with its reason and the decision record that approved it (repo-layout.md, The one task entry
//! point; DR-79).

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;
use std::process::ExitCode;

use serde::Deserialize;

use crate::util;

/// Where the declared CI skips live, from the repo root.
pub const CI_SKIPS: &str = "crates/xtask/ci_skips.toml";

const SKIP_FIX: &str =
    "install what the step needs in CI, or list it in crates/xtask/ci_skips.toml with its reason and record";

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SkipsFile {
    #[serde(default)]
    skip: Vec<Skip>,
}

/// One declared skip: the step's name as its report line prints it, why, and the record.
#[derive(Deserialize, Clone, Debug)]
#[serde(deny_unknown_fields)]
pub struct Skip {
    pub check: String,
    pub reason: String,
    pub record: String,
}

/// The declared CI skips, by step name.
#[derive(Default, Clone, Debug)]
pub struct CiSkips(BTreeMap<String, Skip>);

impl CiSkips {
    /// Read `ci_skips.toml`, refusing an entry without a reason or whose record has no heading in
    /// `decisions.md`.
    pub fn load(root: &Path) -> Result<Self, String> {
        let path = root.join(CI_SKIPS);
        let text = match fs::read_to_string(&path) {
            Ok(t) => t,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(e) => return Err(format!("{CI_SKIPS}: {e}")),
        };
        let file: SkipsFile = toml::from_str(&text).map_err(|e| format!("{CI_SKIPS}: {e}"))?;
        let records = crate::layout::record_headings(root)?;
        let mut skips = BTreeMap::new();
        for skip in file.skip {
            if skip.reason.trim().is_empty() {
                return Err(format!("{CI_SKIPS}: `{}` gives no reason", skip.check));
            }
            let number = crate::layout::record_number(&skip.record)
                .ok_or_else(|| format!("{CI_SKIPS}: `{}` names `{}`, not DR-n", skip.check, skip.record))?;
            if !records.iter().any(|(n, _)| *n == number) {
                return Err(format!(
                    "{CI_SKIPS}: `{}` names {}, which has no heading in docs/contributing/decisions.md",
                    skip.check, skip.record
                ));
            }
            skips.insert(skip.check.clone(), skip);
        }
        Ok(Self(skips))
    }

    pub fn from_entries(entries: impl IntoIterator<Item = Skip>) -> Self {
        Self(entries.into_iter().map(|s| (s.check.clone(), s)).collect())
    }

    pub fn get(&self, check: &str) -> Option<&Skip> {
        self.0.get(check)
    }
}

/// Whether this run is CI's: `CI=true`, as GitHub Actions sets it.
pub fn under_ci() -> bool {
    std::env::var("CI").is_ok_and(|v| v == "true")
}

/// One line per step, printed as each finishes, and a summary at the end.
#[derive(Default)]
pub struct Report {
    lines: Vec<String>,
    failed: bool,
    ci: bool,
    skips: CiSkips,
}

impl Report {
    /// A report for this run: CI's rules when `CI=true`, with the declared skips from `root`.
    pub fn new(root: &Path) -> Self {
        let mut report = Self { ci: under_ci(), ..Self::default() };
        match CiSkips::load(root) {
            Ok(skips) => report.skips = skips,
            Err(e) => report.failed("ci skips", &e, "fix crates/xtask/ci_skips.toml"),
        }
        report
    }

    /// A report with the CI rule set explicitly, for tests.
    pub fn with(ci: bool, skips: CiSkips) -> Self {
        Self { ci, skips, ..Self::default() }
    }

    pub fn ok(&mut self, name: &str) {
        self.line(format!("ok       {name}"));
    }

    /// A step that didn't run. In CI that fails, unless `ci_skips.toml` declares it.
    pub fn skipped(&mut self, name: &str, why: &str) {
        if !self.ci {
            return self.line(format!("skipped  {name} ({why})"));
        }
        match self.skips.get(name) {
            Some(skip) => {
                let line =
                    format!("skipped  {name} ({why}; declared in CI: {}, {})", skip.reason, skip.record);
                self.line(line)
            }
            None => self.failed(name, &format!("skipped under CI=true: {why}"), SKIP_FIX),
        }
    }

    /// A documented step that no code runs yet, with the milestone that builds it. It neither
    /// passes nor fails, so a green run never reads as one that ran it.
    pub fn not_built(&mut self, name: &str, milestone: &str) {
        self.line(format!("not built {name} ({milestone})"));
    }

    pub fn failed(&mut self, name: &str, output: &str, fix: &str) {
        let output = output.trim_end();
        if !output.is_empty() {
            let lines: Vec<&str> = output.lines().collect();
            let tail = &lines[lines.len().saturating_sub(400)..];
            println!("---- {name} ----\n{}\n", tail.join("\n"));
        }
        self.failed = true;
        self.line(format!("FAILED   {name} (fix: {fix})"));
    }

    fn line(&mut self, line: String) {
        println!("{line}");
        self.lines.push(line);
    }

    /// Run a tool as one step.
    pub fn tool(&mut self, root: &Path, name: &str, program: &str, args: &[&str], fix: &str) {
        let (ok, out) = util::run(root, program, args);
        if ok { self.ok(name) } else { self.failed(name, &out, fix) }
    }

    /// Whether any step failed so far.
    pub fn has_failed(&self) -> bool {
        self.failed
    }

    /// The lines printed so far.
    pub fn lines(&self) -> &[String] {
        &self.lines
    }

    pub fn finish(self) -> ExitCode {
        println!("\nsummary:");
        self.lines.iter().for_each(|l| println!("  {l}"));
        if self.failed { ExitCode::FAILURE } else { ExitCode::SUCCESS }
    }
}
