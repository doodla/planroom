//! `cargo xtask test -p CRATE FILTER`: a filter that matches no test fails, rather than reporting
//! `ok` over nothing run, so a mistyped name never reads as a pass (repo-layout.md, The one task
//! entry point).

/// nextest's configuration, passed with `--config-file`. It runs each test that asserts a
/// wall-clock budget with no other test beside it.
pub const NEXTEST_CONFIG: &str = "crates/xtask/nextest.toml";

/// The tests `config` runs alone, as `(binary_id, test name)`: each `test(=NAME)` with the
/// `binary_id(...)` that opens its group.
pub fn alone(config: &str) -> Vec<(String, String)> {
    let mut out = Vec::new();
    let mut binary = String::new();
    let mut rest = config;
    while let Some(at) = rest.find("(") {
        let word = rest[..at].rsplit(|c: char| !(c.is_ascii_alphanumeric() || c == '_')).next().unwrap_or("");
        let Some(end) = rest[at..].find(')') else { break };
        let arg = &rest[at + 1..at + end];
        match word {
            "binary_id" => binary = arg.to_owned(),
            "test" => {
                if let Some(name) = arg.strip_prefix('=') {
                    out.push((binary.clone(), name.to_owned()));
                }
            }
            _ => {}
        }
        rest = &rest[at + 1..];
    }
    out
}

/// The tests in a test file's `text` that assert a wall-clock budget, by name: each `#[test]`
/// function whose body asserts `X < ...` of an `X` it took from `.elapsed()`, or
/// `... .elapsed() < ...` itself. A loop that waits until a deadline asserts no budget.
pub fn budgeted(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    for part in text.split("#[test]\n").skip(1) {
        let Some(at) = part.find("fn ") else { continue };
        let head = &part[..at];
        if !head.lines().all(|l| l.trim().is_empty() || l.trim_start().starts_with("#[")) {
            continue;
        }
        let indent = &head[head.rfind('\n').map_or(0, |n| n + 1)..];
        let rest = &part[at + 3..];
        let Some(open) = rest.find('(') else { continue };
        let name = &rest[..open];
        let body = rest.find(&format!("\n{indent}}}\n")).map_or(rest, |end| &rest[..end]);
        let timers: Vec<&str> = body
            .lines()
            .filter_map(|l| {
                let (var, value) = l.trim().strip_prefix("let ")?.split_once(" = ")?;
                value.ends_with(".elapsed();").then_some(var.trim())
            })
            .collect();
        let asserts = body.split("assert!(").skip(1).any(|a| {
            let cond = a.trim_start().split([',', ';']).next().unwrap_or("");
            cond.contains(".elapsed() <") || timers.iter().any(|t| cond.starts_with(&format!("{t} <")))
        });
        if asserts {
            out.push(name.to_owned());
        }
    }
    out
}

/// What nextest does when no test matches: with a filter, a match was asked for, so none fails;
/// a whole crate or the workspace with no test passes.
pub fn no_tests(filter: Option<&str>) -> &'static str {
    if filter.is_some() { "fail" } else { "pass" }
}

/// The `cargo test` arguments that list, without running, the tests in `krate` whose name holds
/// `filter`.
pub fn list_args<'a>(krate: &'a str, filter: &'a str) -> Vec<&'a str> {
    vec!["test", "--locked", "-p", krate, filter, "--", "--list", "--format", "terse"]
}

/// The `cargo test` arguments that list, without running, every test in `krate`'s test file
/// `tests/FILE.rs`.
pub fn list_file_args<'a>(krate: &'a str, file: &'a str) -> Vec<&'a str> {
    vec!["test", "--locked", "-p", krate, "--test", file, "--", "--list", "--format", "terse"]
}

/// How many tests a `--list --format terse` listing names, across every test binary.
pub fn listed(listing: &str) -> usize {
    listing.lines().filter(|l| l.trim_end().ends_with(": test")).count()
}

/// The failure for a filter whose listing names no test, or `None` when it names one.
pub fn unmatched(krate: &str, filter: &str, listing: &str) -> Option<String> {
    (listed(listing) == 0).then(|| format!("no test in {krate} has a name that holds `{filter}`"))
}

/// What each test runner reported as skipped, each test named so a reader can find it.
///
/// A test that skips itself (`#[ignore]`, `test.skip`, `test.skipIf`, a `todo`) runs nowhere, so
/// under `CI=true` each one fails unless `crates/xtask/ci_skips.toml` declares it under the name
/// [`skips::report`] gives it (repo-layout.md, The one task entry point; DR-79).
pub mod skips {
    use serde_json::Value;

    use crate::report::Report;

    /// Where `cargo xtask test` has vitest write its JSON report.
    pub const VITEST_REPORT: &str = "target/test-reports/vitest.json";
    /// Where tests/e2e/playwright.config.ts has Playwright write its JSON report.
    pub const PLAYWRIGHT_REPORT: &str = "target/e2e/report.json";

    /// The `cargo nextest list` arguments that list `scope`'s tests as JSON, with each test's
    /// `ignored` flag.
    pub fn nextest_list_args<'a>(scope: &[&'a str]) -> Vec<&'a str> {
        let mut args = vec!["nextest", "list", "--locked", "--message-format", "json"];
        args.extend(scope);
        args
    }

    /// The tests a default nextest run skips because they're `#[ignore]`d, as `binary test`,
    /// from `cargo nextest list --message-format json`. A test a name filter left out isn't one.
    pub fn nextest_ignored(json: &str) -> Result<Vec<String>, String> {
        let doc: Value = serde_json::from_str(json).map_err(|e| format!("cargo nextest list: {e}"))?;
        let suites =
            doc.get("rust-suites").and_then(Value::as_object).ok_or("cargo nextest list: no rust-suites")?;
        let mut out = Vec::new();
        for (binary, suite) in suites {
            let Some(cases) = suite.get("testcases").and_then(Value::as_object) else { continue };
            for (name, case) in cases {
                let ignored = case.get("ignored").and_then(Value::as_bool) == Some(true);
                let filter = case.get("filter-match");
                let status = filter.and_then(|f| f.get("status")).and_then(Value::as_str);
                let reason = filter.and_then(|f| f.get("reason")).and_then(Value::as_str);
                if ignored && (status == Some("matches") || reason == Some("ignored")) {
                    out.push(format!("{binary} {name}"));
                }
            }
        }
        out.sort();
        Ok(out)
    }

    /// The tests `cargo test`'s own output reports `ignored`.
    pub fn libtest_ignored(output: &str) -> Vec<String> {
        output
            .lines()
            .filter_map(|l| l.strip_prefix("test ")?.strip_suffix(" ... ignored"))
            .map(|name| name.split(", ").next().unwrap_or(name).to_owned())
            .collect()
    }

    /// The skipped and todo tests in vitest's JSON report, as `file > name`, the file relative
    /// to `root` when it is under it.
    pub fn vitest(json: &str, root: &str) -> Result<Vec<String>, String> {
        let doc: Value = serde_json::from_str(json).map_err(|e| format!("vitest's JSON report: {e}"))?;
        let files =
            doc.get("testResults").and_then(Value::as_array).ok_or("vitest's JSON report: no testResults")?;
        let mut out = Vec::new();
        for file in files {
            let path = file.get("name").and_then(Value::as_str).unwrap_or("?");
            let path = path.strip_prefix(root).map(|p| p.trim_start_matches('/')).unwrap_or(path);
            for test in file.get("assertionResults").and_then(Value::as_array).into_iter().flatten() {
                let status = test.get("status").and_then(Value::as_str).unwrap_or("");
                if matches!(status, "skipped" | "pending" | "todo" | "disabled") {
                    let name = test
                        .get("fullName")
                        .or_else(|| test.get("title"))
                        .and_then(Value::as_str)
                        .unwrap_or("?");
                    out.push(format!("{path} > {name}"));
                }
            }
        }
        Ok(out)
    }

    /// The skipped tests in Playwright's JSON report, as `file > title [project]`.
    pub fn playwright(json: &str) -> Result<Vec<String>, String> {
        let doc: Value = serde_json::from_str(json).map_err(|e| format!("Playwright's JSON report: {e}"))?;
        let mut out = Vec::new();
        let mut stack: Vec<&Value> =
            doc.get("suites").and_then(Value::as_array).into_iter().flatten().collect();
        while let Some(suite) = stack.pop() {
            stack.extend(suite.get("suites").and_then(Value::as_array).into_iter().flatten());
            for spec in suite.get("specs").and_then(Value::as_array).into_iter().flatten() {
                let file = spec.get("file").and_then(Value::as_str).unwrap_or("?");
                let title = spec.get("title").and_then(Value::as_str).unwrap_or("?");
                for test in spec.get("tests").and_then(Value::as_array).into_iter().flatten() {
                    if test.get("status").and_then(Value::as_str) == Some("skipped") {
                        let project = test.get("projectName").and_then(Value::as_str).unwrap_or("");
                        out.push(format!("{file} > {title} [{project}]"));
                    }
                }
            }
        }
        out.sort();
        Ok(out)
    }

    /// One `skipped` line per test that skipped itself, named `RUNNER: TEST`; under `CI=true`
    /// each fails unless `ci_skips.toml` declares that name.
    pub fn report(r: &mut Report, runner: &str, tests: &[String]) {
        for test in tests {
            r.skipped(&format!("{runner}: {test}"), "the test skipped itself");
        }
    }
}
