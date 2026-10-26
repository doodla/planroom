//! `feature_list_is_complete`: every test `feature_list.json` lists resolves to a test that runs
//! under a CI `cargo xtask` command with no `PLANROOM_LIVE_*` flag and nothing live, or carries a
//! `due` milestone. A test that resolves carries no `due`, and an entry marked `passing` carries
//! none (contracts.md, Every public contract; DR-59, DR-89).
//!
//! A listed test takes one of these forms, each resolved the way its runner finds it:
//!
//! | Form | Resolves when | Run by |
//! |---|---|---|
//! | `crates/C/{tests,src}/…/F.rs::name` | `fn name` carries a test attribute and no `#[ignore]` | `cargo xtask test` |
//! | `crates/C/tests/F.rs` | the file holds at least one such test | `cargo xtask test` |
//! | `packages/P/src/…/F.test.ts::name`, `tests/e2e/src/…/F.test.ts::name` | a `test`, `it` or `describe` title is `name`, or starts with it and a non-word character | vitest, in `cargo xtask test` |
//! | `packages/mod/plugin-tests/F.test.tsx::name` | the same | `claude plugin test`, in `cargo xtask test` |
//! | `tests/e2e/src/…/F.spec.ts[::name]` | the same, or the file holds a test | Playwright, in `cargo xtask e2e` |
//! | `tests/installer/run.sh[::name]` | `name() {` is defined and the runner's `for t in` list names it | `cargo xtask test --installer` |
//! | `cargo xtask ARGS…` | `main.rs` has the arm `["ARG", …]` | itself |
//!
//! A test needs a live flag when its body reads a `PLANROOM_LIVE_*` variable from its own
//! environment (C16): such a test belongs on the entry's `evidence_path`, never in its `tests`.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use serde_json::Value;

pub const FEATURE_LIST: &str = "feature_list.json";
const FORMAT: &str = "planroom.feature-list/v1";
const MILESTONES: &[&str] = &["M1", "M2", "M3", "M4", "M5"];
const LIVE: &str = "PLANROOM_LIVE_";

/// The globs `vitest.config.js` includes; the gate checks the config still names each.
const VITEST_INCLUDE: &[&str] = &["packages/*/src/**/*.test.ts", "tests/e2e/src/**/*.test.ts"];
const VITEST_CONFIG: &str = "vitest.config.js";
const XTASK_MAIN: &str = "crates/xtask/src/main.rs";
const INSTALLER: &str = "tests/installer/run.sh";

/// What resolving one listed test found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Resolution {
    /// A test that a CI `cargo xtask` command runs with nothing live.
    Runs,
    /// A test that exists but reads a `PLANROOM_LIVE_*` flag, so CI never runs it.
    NeedsLive(String),
    /// Nothing CI runs answers to the listed name.
    Missing(String),
}

/// Every problem with the feature list at `root`, one line each; empty when it is complete.
pub fn problems(root: &Path) -> Result<Vec<String>, String> {
    let text = fs::read_to_string(root.join(FEATURE_LIST)).map_err(|e| format!("{FEATURE_LIST}: {e}"))?;
    let doc: Value = serde_json::from_str(&text).map_err(|e| format!("{FEATURE_LIST}: {e}"))?;
    let mut problems = Vec::new();
    if doc["format"] != FORMAT {
        problems.push(format!("{FEATURE_LIST}: format is {}, not {FORMAT}", doc["format"]));
    }
    problems.extend(vitest_drift(root));
    let features = doc["features"].as_array().ok_or(format!("{FEATURE_LIST}: no `features` array"))?;
    let mut ids = Vec::new();
    let mut cache = Sources::default();
    for feature in features {
        let id = feature["id"].as_str().unwrap_or("?").to_owned();
        ids.push(id.clone());
        let tests: Vec<&str> =
            feature["tests"].as_array().into_iter().flatten().filter_map(Value::as_str).collect();
        let due: BTreeMap<&str, &str> = match &feature["due"] {
            Value::Null => BTreeMap::new(),
            Value::Object(map) => map.iter().map(|(k, v)| (k.as_str(), v.as_str().unwrap_or(""))).collect(),
            other => {
                problems
                    .push(format!("{id}: `due` is {other}, not an object from a listed test to a milestone"));
                BTreeMap::new()
            }
        };
        let passing = feature["status"] == "passing";
        for (test, milestone) in &due {
            if !tests.contains(test) {
                problems.push(format!("{id}: `due` names {test}, which the entry doesn't list"));
            }
            if !MILESTONES.contains(milestone) {
                problems.push(format!("{id}: {test} is due `{milestone}`, not one of M1 to M5"));
            }
            if passing {
                problems.push(format!("{id}: a passing entry still has {test} due {milestone}"));
            }
        }
        for test in &tests {
            match (resolve(root, test, &mut cache), due.get(test)) {
                (Resolution::Runs, None) => {}
                (Resolution::Runs, Some(m)) => {
                    problems.push(format!("{id}: {test} resolves, so it carries no `due` (it says {m})"))
                }
                (Resolution::NeedsLive(why), _) => problems.push(format!(
                    "{id}: {test} needs a live flag ({why}); it belongs on the entry's evidence_path"
                )),
                (Resolution::Missing(why), None) => {
                    problems.push(format!("{id}: {test} doesn't resolve ({why}) and carries no `due`"))
                }
                (Resolution::Missing(_), Some(_)) => {}
            }
        }
    }
    for n in 1..=15 {
        let id = format!("S{n}");
        if !ids.contains(&id) {
            problems.push(format!("{FEATURE_LIST}: no entry for {id}"));
        }
    }
    Ok(problems)
}

/// The source files read so far, by path.
#[derive(Default)]
struct Sources(BTreeMap<String, Option<String>>);

impl Sources {
    fn get(&mut self, root: &Path, rel: &str) -> Option<&str> {
        self.0.entry(rel.to_owned()).or_insert_with(|| fs::read_to_string(root.join(rel)).ok()).as_deref()
    }
}

/// Resolve one listed test at `root`.
pub fn resolve_one(root: &Path, test: &str) -> Resolution {
    resolve(root, test, &mut Sources::default())
}

fn resolve(root: &Path, test: &str, cache: &mut Sources) -> Resolution {
    if let Some(args) = test.strip_prefix("cargo xtask ") {
        return command(root, args, cache);
    }
    // A TypeScript test's name is its title, which may hold spaces; a path never does.
    let (path, name) = match test.split_once("::") {
        Some((p, n)) => (p, Some(n)),
        None => (test, None),
    };
    if path.contains(char::is_whitespace) || name.is_some_and(str::is_empty) {
        return Resolution::Missing("not a `path::name`, a path or a `cargo xtask` command".into());
    }
    let Some(runner) = runner(path) else {
        return Resolution::Missing(format!("no CI `cargo xtask` command runs {path}"));
    };
    let Some(src) = cache.get(root, path) else {
        return Resolution::Missing(format!("{path} doesn't exist"));
    };
    match runner {
        Runner::Rust => rust(src, name),
        Runner::Ts => ts(src, name),
        Runner::Installer => installer(src, name),
    }
}

enum Runner {
    Rust,
    Ts,
    Installer,
}

/// Which CI runner takes a file, by its path.
fn runner(path: &str) -> Option<Runner> {
    let parts: Vec<&str> = path.split('/').collect();
    match parts.as_slice() {
        ["crates", _, "tests" | "src", .., f] if f.ends_with(".rs") => Some(Runner::Rust),
        ["packages", _, "src", .., f] if f.ends_with(".test.ts") => Some(Runner::Ts),
        ["packages", "mod", "plugin-tests", f] if f.ends_with(".test.tsx") || f.ends_with(".test.ts") => {
            Some(Runner::Ts)
        }
        ["tests", "e2e", "src", .., f] if f.ends_with(".test.ts") || f.ends_with(".spec.ts") => {
            Some(Runner::Ts)
        }
        _ if path == INSTALLER => Some(Runner::Installer),
        _ => None,
    }
}

/// `cargo xtask ARGS` resolves when `main.rs` dispatches exactly those arguments.
fn command(root: &Path, args: &str, cache: &mut Sources) -> Resolution {
    let arm =
        format!("[{}]", args.split_whitespace().map(|a| format!("\"{a}\"")).collect::<Vec<_>>().join(", "));
    match cache.get(root, XTASK_MAIN) {
        Some(main) if main.contains(&arm) => Resolution::Runs,
        Some(_) => Resolution::Missing(format!("{XTASK_MAIN} has no `{arm}` arm")),
        None => Resolution::Missing(format!("{XTASK_MAIN} doesn't exist")),
    }
}

/// A Rust test: `fn name` under a test attribute, not `#[ignore]`d.
fn rust(src: &str, name: Option<&str>) -> Resolution {
    let lines: Vec<&str> = src.lines().collect();
    let mut found = false;
    for (i, line) in lines.iter().enumerate() {
        let Some(fn_name) = fn_name(line) else { continue };
        let attrs = attributes_above(&lines, i);
        if !is_test_attr(&attrs) {
            continue;
        }
        if name.is_some_and(|n| n != fn_name) {
            continue;
        }
        if attrs.contains("#[ignore") {
            if name.is_some() {
                return Resolution::Missing(format!("`{fn_name}` is #[ignore]d, so CI never runs it"));
            }
            continue;
        }
        let body = rust_body(&lines, i);
        if let Some(why) = reads_live_rust(&body) {
            return Resolution::NeedsLive(format!("`{fn_name}` {why}"));
        }
        found = true;
        if name.is_some() {
            break;
        }
    }
    match (found, name) {
        (true, _) => Resolution::Runs,
        (false, Some(n)) => Resolution::Missing(format!("no test `fn {n}`")),
        (false, None) => Resolution::Missing("the file holds no test".into()),
    }
}

/// The name a line declares with `fn`, if it does.
fn fn_name(line: &str) -> Option<&str> {
    let t = line.trim_start();
    let t = t.strip_prefix("pub ").unwrap_or(t);
    let t = t.strip_prefix("async ").unwrap_or(t);
    let rest = t.strip_prefix("fn ")?;
    let end = rest.find(|c: char| !(c.is_alphanumeric() || c == '_'))?;
    rest[end..].starts_with(['(', '<']).then_some(&rest[..end])
}

/// The attribute and comment lines directly above line `i`, joined.
fn attributes_above(lines: &[&str], i: usize) -> String {
    let mut out = Vec::new();
    for line in lines[..i].iter().rev() {
        let t = line.trim();
        if t.is_empty() || t.ends_with('{') || t.ends_with('}') || t.ends_with(';') && !t.starts_with("//") {
            break;
        }
        out.push(t);
    }
    out.reverse();
    out.join("\n")
}

fn is_test_attr(attrs: &str) -> bool {
    attrs.lines().any(|l| {
        l.starts_with("#[") && {
            let inner = l.trim_start_matches("#[");
            let path = inner.split(['(', ']']).next().unwrap_or("");
            path == "test" || path.ends_with("::test")
        }
    })
}

/// A Rust function's body: from its `fn` line to the `}` at its own indentation.
fn rust_body(lines: &[&str], i: usize) -> String {
    let indent = &lines[i][..lines[i].len() - lines[i].trim_start().len()];
    let close = format!("{indent}}}");
    let end = lines[i..].iter().position(|l| l.trim_end() == close).map_or(lines.len(), |p| i + p + 1);
    lines[i..end].join("\n")
}

fn reads_live_rust(body: &str) -> Option<String> {
    let reads = [format!("var(\"{LIVE}"), format!("var_os(\"{LIVE}"), format!("env!(\"{LIVE}")];
    body.lines()
        .find(|l| reads.iter().any(|r| l.contains(r.as_str())))
        .map(|l| format!("reads its own environment: {}", l.trim()))
}

/// A TypeScript test: a `test`, `it` or `describe` title that is the name, or starts with it.
fn ts(src: &str, name: Option<&str>) -> Resolution {
    let titles = ts_titles(src);
    let pick: Vec<&Title> = match name {
        Some(n) => titles.iter().filter(|t| title_matches(&t.title, n)).collect(),
        None => titles.iter().collect(),
    };
    let Some(first) = pick.iter().find(|t| !t.skipped) else {
        return match (pick.first(), name) {
            (Some(t), _) => Resolution::Missing(format!("`{}` is skipped, so CI never runs it", t.title)),
            (None, Some(n)) => Resolution::Missing(format!("no test titled `{n}`")),
            (None, None) => Resolution::Missing("the file holds no test".into()),
        };
    };
    let next = titles.iter().map(|t| t.at).find(|&at| at > first.at).unwrap_or(src.len());
    let body = &src[first.at..next];
    let reads =
        [format!("process.env.{LIVE}"), format!("process.env[\"{LIVE}"), format!("process.env['{LIVE}")];
    if let Some(line) = body.lines().find(|l| reads.iter().any(|r| l.contains(r.as_str()))) {
        return Resolution::NeedsLive(format!(
            "`{}` reads its own environment: {}",
            first.title,
            line.trim()
        ));
    }
    Resolution::Runs
}

struct Title {
    title: String,
    at: usize,
    skipped: bool,
}

/// Every `test(`, `it(` or `describe(` call with a string title, with its modifiers.
fn ts_titles(src: &str) -> Vec<Title> {
    let bytes = src.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        let word_start =
            i == 0 || !(bytes[i - 1].is_ascii_alphanumeric() || bytes[i - 1] == b'_' || bytes[i - 1] == b'.');
        let head = ["test", "it", "describe"].iter().find(|w| bytes[i..].starts_with(w.as_bytes()));
        let (Some(head), true) = (head, word_start) else {
            i += 1;
            continue;
        };
        let mut j = i + head.len();
        // `.skip`, `.only`, `.describe`, `.serial`… chained before the call.
        let mut modifiers = Vec::new();
        while bytes.get(j) == Some(&b'.') {
            let start = j + 1;
            let mut k = start;
            while k < bytes.len() && (bytes[k].is_ascii_alphanumeric() || bytes[k] == b'_') {
                k += 1;
            }
            modifiers.push(&src[start..k]);
            j = k;
        }
        if bytes.get(j) != Some(&b'(') {
            i += 1;
            continue;
        }
        j += 1;
        while bytes.get(j).is_some_and(u8::is_ascii_whitespace) {
            j += 1;
        }
        let Some(&quote) = bytes.get(j).filter(|q| matches!(q, b'"' | b'\'' | b'`')) else {
            i = j;
            continue;
        };
        let start = j + 1;
        let mut k = start;
        while k < bytes.len() && bytes[k] != quote && !(quote == b'`' && bytes[k..].starts_with(b"${")) {
            k += if bytes[k] == b'\\' { 2 } else { 1 };
        }
        let skipped = modifiers.iter().any(|m| matches!(*m, "skip" | "todo" | "fixme"));
        let title = String::from_utf8_lossy(&bytes[start..k.min(bytes.len())]).into_owned();
        out.push(Title { title, at: i, skipped });
        i = k + 1;
    }
    out
}

fn title_matches(title: &str, name: &str) -> bool {
    title == name
        || title
            .strip_prefix(name)
            .and_then(|rest| rest.chars().next())
            .is_some_and(|c| !(c.is_alphanumeric() || c == '_'))
}

/// An installer test: `name() {` defined, and named in the runner's `for t in` list.
fn installer(src: &str, name: Option<&str>) -> Resolution {
    let listed: Vec<&str> = match (src.rfind("for t in "), src.rfind("; do")) {
        (Some(a), Some(b)) if a < b => {
            src[a + "for t in ".len()..b].split_whitespace().filter(|w| *w != "\\").collect()
        }
        _ => Vec::new(),
    };
    let Some(name) = name else {
        return if listed.is_empty() {
            Resolution::Missing("no runner list".into())
        } else {
            Resolution::Runs
        };
    };
    let def = format!("{name}() {{");
    let lines: Vec<&str> = src.lines().collect();
    let Some(at) = lines.iter().position(|l| *l == def) else {
        return Resolution::Missing(format!("no function `{name}`"));
    };
    if !listed.contains(&name) {
        return Resolution::Missing(format!("`{name}` is defined but the runner's `for t in` list skips it"));
    }
    let end = lines[at..].iter().position(|l| *l == "}").map_or(lines.len(), |p| at + p);
    if let Some(line) =
        lines[at..end].iter().find(|l| l.contains(&format!("${LIVE}")) || l.contains(&format!("${{{LIVE}")))
    {
        return Resolution::NeedsLive(format!("`{name}` reads its own environment: {}", line.trim()));
    }
    Resolution::Runs
}

/// A line for each glob the gate takes as vitest's that `vitest.config.js` no longer includes.
fn vitest_drift(root: &Path) -> Vec<String> {
    let config = fs::read_to_string(root.join(VITEST_CONFIG)).unwrap_or_default();
    VITEST_INCLUDE
        .iter()
        .filter(|g| !config.contains(&format!("\"{g}\"")))
        .map(|g| format!("{VITEST_CONFIG} no longer includes {g}; update VITEST_INCLUDE in crates/xtask/src/features.rs"))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fn_line_names_its_function() {
        assert_eq!(fn_name("fn a_b() {"), Some("a_b"));
        assert_eq!(fn_name("    pub async fn c() -> X {"), Some("c"));
        assert_eq!(fn_name("let fn_x = 1;"), None);
    }

    #[test]
    fn a_rust_test_resolves_only_under_a_test_attribute() {
        let src = "#[test]\nfn listed() {\n    assert!(true);\n}\n\nfn helper() {}\n\n#[tokio::test(flavor = \"multi_thread\")]\nasync fn later() {\n}\n";
        assert_eq!(rust(src, Some("listed")), Resolution::Runs);
        assert_eq!(rust(src, Some("later")), Resolution::Runs);
        assert!(matches!(rust(src, Some("helper")), Resolution::Missing(_)));
        assert_eq!(rust(src, None), Resolution::Runs);
        let ignored = "#[test]\n#[ignore = \"slow\"]\nfn listed() {\n}\n";
        assert!(matches!(rust(ignored, Some("listed")), Resolution::Missing(_)));
    }

    #[test]
    fn a_ts_title_matches_its_name_and_a_described_name() {
        let src = "test(\"rows_fit: every row fits\", () => {});\ndescribe(\"group\", () => {\n  test.skip(\"later\", () => {});\n  it(`at ${w}`, () => {});\n});\n";
        assert_eq!(ts(src, Some("rows_fit")), Resolution::Runs);
        assert_eq!(ts(src, Some("group")), Resolution::Runs);
        assert!(matches!(ts(src, Some("rows")), Resolution::Missing(_)));
        assert!(matches!(ts(src, Some("later")), Resolution::Missing(_)));
    }
}
