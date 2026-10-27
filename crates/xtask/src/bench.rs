//! `cargo xtask bench fold`: run the kernel's fold benchmark and compare each case's median with
//! its budget in `budgets.json` (architecture.md, Performance budgets; DR-47, DR-89).
//!
//! The benchmark prints one JSON line per case, `{"bench": NAME, "median_ms": N}`. A case fails
//! when its median passes `budgets.json`'s `NAME.ms`, when the file holds no number there, and when
//! the benchmark printed no line for it, so a missing budget or a renamed case never passes.

use std::path::Path;
use std::process::Command;

use serde_json::Value;

/// The budgets file, at the repo root.
pub const BUDGETS: &str = "budgets.json";

/// The fold cases the benchmark measures, each the `budgets.json` key that gates it.
pub const FOLD_CASES: [&str; 2] = ["fold.20k", "fold.50k"];

/// One case's outcome: its name and, when it fails, why.
#[derive(Debug, Clone, PartialEq)]
pub struct Outcome {
    pub case: String,
    pub median_ms: Option<f64>,
    pub budget_ms: Option<f64>,
    pub failure: Option<String>,
}

/// Compare the benchmark's `output` with `budgets` for each of `cases`.
pub fn compare(output: &str, budgets: &Value, cases: &[&str]) -> Vec<Outcome> {
    let mut medians: Vec<(String, f64)> = Vec::new();
    for line in output.lines() {
        let Ok(value) = serde_json::from_str::<Value>(line.trim()) else { continue };
        if let (Some(bench), Some(ms)) = (value["bench"].as_str(), value["median_ms"].as_f64()) {
            medians.push((bench.to_owned(), ms));
        }
    }
    cases
        .iter()
        .map(|case| {
            let median_ms = medians.iter().rev().find(|(b, _)| b == case).map(|(_, ms)| *ms);
            let budget_ms = budgets.get(*case).and_then(|b| b.get("ms")).and_then(Value::as_f64);
            let failure = match (median_ms, budget_ms) {
                (None, _) => Some(format!("the benchmark printed no result for {case}")),
                (_, None) => Some(format!("{BUDGETS} holds no number at {case}.ms")),
                (Some(m), Some(b)) if !m.is_finite() || m > b => {
                    Some(format!("{case}: median {m} ms is over its {b} ms budget"))
                }
                _ => None,
            };
            Outcome { case: (*case).to_owned(), median_ms, budget_ms, failure }
        })
        .collect()
}

/// Run `cargo bench fold` in the kernel and return what it printed on stdout.
pub fn run_fold(root: &Path) -> Result<String, String> {
    let out = Command::new("cargo")
        .args(["bench", "--locked", "-p", "planroom-kernel", "--bench", "fold"])
        .current_dir(root)
        .output()
        .map_err(|e| format!("could not run cargo bench: {e}"))?;
    if !out.status.success() {
        return Err(format!(
            "cargo bench -p planroom-kernel --bench fold failed:\n{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).into_owned())
}

/// Read `budgets.json` at `root`.
pub fn budgets(root: &Path) -> Result<Value, String> {
    let text = std::fs::read_to_string(root.join(BUDGETS)).map_err(|e| format!("{BUDGETS}: {e}"))?;
    serde_json::from_str(&text).map_err(|e| format!("{BUDGETS}: {e}"))
}
