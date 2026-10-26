//! Under `CI=true` a `skipped` step fails unless `crates/xtask/ci_skips.toml` declares it (DR-79).

use std::path::Path;

use xtask::report::{CiSkips, Report, Skip};

fn declared(check: &str) -> CiSkips {
    CiSkips::from_entries([Skip {
        check: check.into(),
        reason: "no runner has it".into(),
        record: "DR-79".into(),
    }])
}

#[test]
fn a_skip_fails_under_ci() {
    let mut r = Report::with(true, CiSkips::default());
    r.skipped("shellcheck", "shellcheck is not installed");
    assert!(r.has_failed());
    assert!(r.lines()[0].starts_with("FAILED   shellcheck"), "{:?}", r.lines());
}

#[test]
fn a_declared_skip_passes() {
    let mut r = Report::with(true, declared("shellcheck"));
    r.skipped("shellcheck", "shellcheck is not installed");
    assert!(!r.has_failed(), "{:?}", r.lines());
    assert!(r.lines()[0].contains("declared in CI: no runner has it, DR-79"), "{:?}", r.lines());
    // A declaration covers only the check it names.
    r.skipped("busybox", "busybox is not installed");
    assert!(r.has_failed());
}

#[test]
fn a_local_skip_is_only_a_line() {
    let mut r = Report::with(false, CiSkips::default());
    r.skipped("shellcheck", "shellcheck is not installed");
    assert!(!r.has_failed());
    assert_eq!(r.lines()[0], "skipped  shellcheck (shellcheck is not installed)");
}

#[test]
fn a_gate_not_built_is_a_line_that_neither_passes_nor_fails() {
    let mut r = Report::with(true, CiSkips::default());
    r.not_built("release_pins", "M1");
    assert!(!r.has_failed());
    assert_eq!(r.lines()[0], "not built release_pins (M1)");
}

#[test]
fn the_committed_declarations_each_name_a_record_that_exists() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    CiSkips::load(&root).expect("crates/xtask/ci_skips.toml loads, and each record it names has a heading");
}
