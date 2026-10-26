//! `cargo xtask test -p CRATE FILTER`: a filter that matches no test fails, rather than reporting
//! `ok` over nothing run, so a mistyped name never reads as a pass (repo-layout.md, The one task
//! entry point).

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
