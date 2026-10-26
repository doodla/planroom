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

/// How many tests a `--list --format terse` listing names, across every test binary.
pub fn listed(listing: &str) -> usize {
    listing.lines().filter(|l| l.trim_end().ends_with(": test")).count()
}

/// The failure for a filter whose listing names no test, or `None` when it names one.
pub fn unmatched(krate: &str, filter: &str, listing: &str) -> Option<String> {
    (listed(listing) == 0).then(|| format!("no test in {krate} has a name that holds `{filter}`"))
}
