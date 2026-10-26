//! The decision-record gate, run on a temporary clone of this repository that is no linked
//! worktree: the first lock names DR-79, a changed surface fails until a newer record ratifies it,
//! and the per-commit walk holds a lock change to the record its commit adds (repo-layout.md,
//! Decision-record gate; DR-59, DR-70, DR-79, DR-81).

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use xtask::layout::{self, CiWalk, LOCK};

/// A clone of this repository's HEAD in a temporary directory, removed on drop.
struct Clone {
    dir: PathBuf,
}

impl Clone {
    fn new(name: &str) -> Self {
        let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let dir = std::env::temp_dir().join(format!("planroom-gates-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let dir_str = dir.to_str().unwrap().to_owned();
        git(&repo, &["clone", "--quiet", "--shared", ".", &dir_str]);
        // Each test starts from a repository with no lock yet. Once this repository holds one, the
        // clone's removal is committed, so a test that writes the same lock again has a change.
        let clone = Self { dir };
        if clone.path(LOCK).exists() {
            fs::remove_file(clone.path(LOCK)).unwrap();
            clone.commit("start with no layout.lock");
        }
        clone
    }

    fn path(&self, rel: &str) -> PathBuf {
        self.dir.join(rel)
    }

    fn append(&self, rel: &str, text: &str) {
        let mut s = fs::read_to_string(self.path(rel)).unwrap();
        s.push_str(text);
        fs::write(self.path(rel), s).unwrap();
    }

    fn replace(&self, rel: &str, from: &str, to: &str) {
        let s = fs::read_to_string(self.path(rel)).unwrap();
        assert!(s.contains(from), "{rel} holds no {from:?}");
        fs::write(self.path(rel), s.replacen(from, to, 1)).unwrap();
    }

    /// The next free record number, and its heading appended to decisions.md.
    fn add_record(&self) -> String {
        let n = layout::record_headings(&self.dir).unwrap().iter().map(|(n, _)| *n).max().unwrap() + 1;
        self.append(layout::DECISIONS, &format!("\n## DR-{n}: A test record\n\n- **Status:** test.\n"));
        format!("DR-{n}")
    }

    fn commit(&self, message: &str) -> String {
        git(&self.dir, &["add", "-A"]);
        git(
            &self.dir,
            &[
                "-c",
                "user.name=gates-test",
                "-c",
                "user.email=gates-test@example.invalid",
                "commit",
                "--quiet",
                "-m",
                message,
            ],
        );
        self.head()
    }

    fn head(&self) -> String {
        git(&self.dir, &["rev-parse", "HEAD"]).trim().to_owned()
    }

    fn state(&self) -> layout::State {
        layout::state(&self.dir).unwrap()
    }

    fn lock(&self) -> layout::Lock {
        layout::parse_lock(&fs::read_to_string(self.path(LOCK)).unwrap()).unwrap()
    }

    fn walk(&self, range: &str) -> layout::Walk {
        layout::walk(&self.dir, &[range.to_owned()]).unwrap()
    }
}

impl Drop for Clone {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

fn git(dir: &Path, args: &[&str]) -> String {
    let out = Command::new("git").args(args).current_dir(dir).output().expect("git runs");
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8_lossy(&out.stdout).into_owned()
}

#[test]
fn the_first_lock_names_dr_79() {
    let c = Clone::new("first");
    assert!(!layout::linked_worktree(&c.dir).unwrap());
    let st = c.state();
    assert!(!st.has_lock && !st.changed.is_empty(), "with no lock every surface is unratified: {st:?}");
    let err = layout::write_lock(&c.dir, "DR-80").unwrap_err();
    assert!(err.contains("the first lock names DR-79"), "{err}");
    let ratified = layout::write_lock(&c.dir, "DR-79").unwrap();
    assert!(ratified.contains(&"boundaries.toml".to_owned()), "{ratified:?}");
    let lock = c.lock();
    for surface in [
        "boundaries.toml",
        "crate dependencies",
        "top-level directories",
        "area contracts table",
        "AGENTS.md Boundaries",
        "AGENTS.md Conventions",
        "package packages/mod",
    ] {
        assert_eq!(lock.get(surface).map(|e| e.record), Some(79), "{surface}: {lock:?}");
    }
    let st = c.state();
    assert!(st.errors.is_empty() && st.changed.is_empty(), "{st:?}");
}

#[test]
fn a_changed_surface_fails_until_a_newer_record_ratifies_it() {
    let c = Clone::new("ratify");
    layout::write_lock(&c.dir, "DR-79").unwrap();
    c.append("boundaries.toml", "\n# a changed rule\n");
    assert_eq!(c.state().changed, ["boundaries.toml"]);
    let err = layout::write_lock(&c.dir, "DR-79").unwrap_err();
    assert!(err.contains("isn't newer than DR-79"), "{err}");
    let err = layout::write_lock(&c.dir, "DR-9999").unwrap_err();
    assert!(err.contains("no `## DR-9999` heading"), "{err}");
    let record = c.add_record();
    let ratified = layout::write_lock(&c.dir, &record).unwrap();
    assert_eq!(ratified, ["boundaries.toml"]);
    let n = layout::record_number(&record).unwrap();
    assert_eq!(c.lock()["boundaries.toml"].record, n);
    assert_eq!(c.lock()["crate dependencies"].record, 79, "an unchanged surface keeps its record");
    let st = c.state();
    assert!(st.errors.is_empty() && st.changed.is_empty(), "{st:?}");
}

#[test]
fn a_duplicate_record_heading_fails() {
    let c = Clone::new("duplicate");
    layout::write_lock(&c.dir, "DR-79").unwrap();
    c.append(layout::DECISIONS, "\n## DR-79: The same number again\n");
    let st = c.state();
    assert!(st.errors.iter().any(|e| e.contains("`## DR-79` appears more than once")), "{st:?}");
    assert!(layout::write_lock(&c.dir, "DR-79").is_err());
}

#[test]
fn a_lock_naming_a_missing_record_fails() {
    let c = Clone::new("missing");
    layout::write_lock(&c.dir, "DR-79").unwrap();
    let text = fs::read_to_string(c.path(LOCK)).unwrap();
    fs::write(c.path(LOCK), text.replacen("record = \"DR-79\"", "record = \"DR-9999\"", 1)).unwrap();
    let st = c.state();
    assert!(st.errors.iter().any(|e| e.contains("names DR-9999, which has no heading")), "{st:?}");
}

#[test]
fn a_dependency_version_bump_needs_none_and_a_new_dependency_needs_a_record() {
    let c = Clone::new("deps");
    layout::write_lock(&c.dir, "DR-79").unwrap();
    c.replace("packages/canvas/package.json", "\"zustand\": \"", "\"zustand\": \"0.0.1-");
    assert!(c.state().changed.is_empty(), "a version bump changed a surface");
    c.replace(
        "packages/canvas/package.json",
        "\"dependencies\": {",
        "\"dependencies\": {\n    \"left-pad\": \"1.3.0\",",
    );
    assert_eq!(c.state().changed, ["package packages/canvas"]);
}

#[test]
fn an_edited_area_row_needs_a_record_and_its_formatting_does_not() {
    let c = Clone::new("area");
    layout::write_lock(&c.dir, "DR-79").unwrap();
    c.replace(
        layout::REPO_LAYOUT,
        "| `crates/xtask` | task commands |",
        "|   `crates/xtask`   |   task   commands   |",
    );
    assert!(c.state().changed.is_empty(), "reformatting a row changed a surface");
    c.replace(layout::REPO_LAYOUT, "task   commands", "task commands and releases");
    assert_eq!(c.state().changed, ["area contracts table"]);
    c.replace("AGENTS.md", "## Conventions\n", "## Conventions\n\nA new convention.\n");
    assert_eq!(c.state().changed, ["AGENTS.md Conventions", "area contracts table"]);
}

#[test]
fn a_worktree_change_is_pending_not_failed_and_gen_layout_lock_refuses_there() {
    let c = Clone::new("worktree");
    layout::write_lock(&c.dir, "DR-79").unwrap();
    c.commit("the first lock");
    let wt = c.dir.with_extension("wt");
    let _ = fs::remove_dir_all(&wt);
    git(&c.dir, &["worktree", "add", "--quiet", "--detach", wt.to_str().unwrap(), "HEAD"]);
    assert!(layout::linked_worktree(&wt).unwrap());
    let mut s = fs::read_to_string(wt.join("boundaries.toml")).unwrap();
    s.push_str("\n# a branch's change\n");
    fs::write(wt.join("boundaries.toml"), s).unwrap();
    let st = layout::state(&wt).unwrap();
    assert!(st.errors.is_empty(), "{st:?}");
    assert_eq!(st.changed, ["boundaries.toml"], "the change shows, as pending ratification");
    let err = layout::write_lock(&wt, "DR-79").unwrap_err();
    assert!(err.contains("refuses in a linked worktree"), "{err}");
    let _ = fs::remove_dir_all(&wt);
}

#[test]
fn a_lock_change_must_add_its_record_in_the_same_commit() {
    let c = Clone::new("walk");
    let base = c.head();
    layout::write_lock(&c.dir, "DR-79").unwrap();
    let first = c.commit("write the first lock");
    let walk = c.walk(&format!("{base}..{first}"));
    assert!(walk.failures.is_empty(), "{walk:?}");
    assert!(walk.lines.iter().any(|l| l.contains("writes the first layout.lock")), "{walk:?}");

    // The record lands in one commit and the lock that names it in the next: that is a bad commit.
    let record = c.add_record();
    c.commit("a record alone");
    c.append("boundaries.toml", "\n# changed\n");
    layout::write_lock(&c.dir, &record).unwrap();
    let bad = c.commit("a lock naming an older commit's record");
    let walk = c.walk(&format!("{first}..{bad}"));
    assert_eq!(walk.failures.len(), 1, "{walk:?}");
    assert!(walk.failures[0].contains(&format!("doesn't add `## {record}`")), "{walk:?}");
    assert!(walk.lines.iter().any(|l| l.contains("gated surfaces changed: boundaries.toml")), "{walk:?}");

    // The right way: the record and the lock in one commit.
    c.append("boundaries.toml", "\n# changed again\n");
    let record = c.add_record();
    layout::write_lock(&c.dir, &record).unwrap();
    let good = c.commit("a record and the lock that names it");
    let walk = c.walk(&format!("{bad}..{good}"));
    assert!(walk.failures.is_empty(), "{walk:?}");
    assert!(walk.lines.iter().any(|l| l.contains(&format!("{record} ratifies boundaries.toml"))), "{walk:?}");
}

#[test]
fn a_bad_commit_already_pushed_fails_no_later_run() {
    let c = Clone::new("pushed");
    layout::write_lock(&c.dir, "DR-79").unwrap();
    c.commit("write the first lock");
    let record = c.add_record();
    c.commit("a record alone");
    c.append("boundaries.toml", "\n# changed\n");
    layout::write_lock(&c.dir, &record).unwrap();
    let bad = c.commit("a bad commit, already pushed");
    c.append("README.md", "\nA later change touching no gated surface.\n");
    let later = c.commit("a later push");
    let walk = c.walk(&format!("{bad}..{later}"));
    assert!(walk.failures.is_empty(), "{walk:?}");
    assert_eq!(walk.commits, 1);
    assert!(c.state().errors.is_empty() && c.state().changed.is_empty());
}

#[test]
fn a_range_that_cant_be_resolved_fails() {
    let c = Clone::new("range");
    let err = layout::walk(&c.dir, &["no-such-ref..HEAD".to_owned()]).unwrap_err();
    assert!(err.contains("can't be resolved"), "{err}");
}

#[test]
fn ci_walks_branch_pushes_and_pull_requests_only() {
    let yes = |_: &str, _: &str| true;
    let ev = serde_json::json!({"before": "b4"});
    assert_eq!(
        layout::ci_walk("push", "refs/heads/main", "tip", &ev, &yes).unwrap(),
        CiWalk::Range(vec!["b4..tip".into()])
    );
    assert!(matches!(
        layout::ci_walk("workflow_dispatch", "refs/heads/main", "tip", &ev, &yes).unwrap(),
        CiWalk::StateOnly(_)
    ));
    assert!(matches!(
        layout::ci_walk("push", "refs/tags/v0.1.0", "tip", &ev, &yes).unwrap(),
        CiWalk::StateOnly(_)
    ));
}

// --- feature_list_is_complete (contracts.md, Every public contract; DR-59, DR-89) ----------------

/// A temporary repository holding a feature list and the test files it names, removed on drop.
struct Listed {
    dir: PathBuf,
}

impl Listed {
    /// S1 to S15 with no tests, then `extra` as entry `C1`, beside `files`.
    fn new(name: &str, extra: serde_json::Value, files: &[(&str, &str)]) -> Self {
        let dir = std::env::temp_dir().join(format!("planroom-features-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let mut features: Vec<serde_json::Value> = (1..=15)
            .map(|n| serde_json::json!({"id": format!("S{n}"), "status": "failing", "tests": []}))
            .collect();
        let mut extra = extra;
        extra["id"] = "C1".into();
        features.push(extra);
        let list = serde_json::json!({"format": "planroom.feature-list/v1", "features": features});
        let mut all = vec![
            ("feature_list.json", serde_json::to_string_pretty(&list).unwrap()),
            (
                "vitest.config.js",
                r#"include: ["packages/*/src/**/*.test.ts", "tests/e2e/src/**/*.test.ts"]"#.to_owned(),
            ),
        ];
        all.extend(files.iter().map(|(p, t)| (*p, (*t).to_owned())));
        for (rel, text) in all {
            let path = dir.join(rel);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, text).unwrap();
        }
        Self { dir }
    }

    fn problems(&self) -> Vec<String> {
        xtask::features::problems(&self.dir).unwrap()
    }
}

impl Drop for Listed {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

const BUILT: &str = "#[test]\nfn built() {\n    assert_eq!(1 + 1, 2);\n}\n";
const BUILT_TEST: &str = "crates/k/tests/t.rs::built";
const UNBUILT: &str = "crates/k/tests/t.rs::not_written_yet";

#[test]
fn feature_list_is_complete() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let problems = xtask::features::problems(&root).unwrap();
    assert!(problems.is_empty(), "feature_list.json is incomplete:\n{}", problems.join("\n"));
}

#[test]
fn a_listed_test_that_resolves_carries_no_due() {
    let ok = Listed::new(
        "due-ok",
        serde_json::json!({"status": "failing", "tests": [BUILT_TEST, UNBUILT], "due": {UNBUILT: "M2"}}),
        &[("crates/k/tests/t.rs", BUILT)],
    );
    assert!(ok.problems().is_empty(), "{:?}", ok.problems());

    let stale = Listed::new(
        "due-stale",
        serde_json::json!({"status": "failing", "tests": [BUILT_TEST], "due": {BUILT_TEST: "M1"}}),
        &[("crates/k/tests/t.rs", BUILT)],
    );
    let p = stale.problems();
    assert!(p.iter().any(|l| l.contains(BUILT_TEST) && l.contains("carries no `due`")), "{p:?}");

    // An unbuilt test with no `due` can't be told from a mistyped one, so it fails too.
    let untagged = Listed::new(
        "due-missing",
        serde_json::json!({"status": "failing", "tests": [UNBUILT]}),
        &[("crates/k/tests/t.rs", BUILT)],
    );
    let p = untagged.problems();
    assert!(p.iter().any(|l| l.contains(UNBUILT) && l.contains("no test `fn not_written_yet`")), "{p:?}");

    // A `due` names a milestone, and only a test the entry lists.
    let odd = Listed::new(
        "due-odd",
        serde_json::json!({
            "status": "failing",
            "tests": [UNBUILT],
            "due": {UNBUILT: "M9", "crates/k/tests/t.rs::other": "M2"},
        }),
        &[("crates/k/tests/t.rs", BUILT)],
    );
    let p = odd.problems();
    assert!(p.iter().any(|l| l.contains("not one of M1 to M5")), "{p:?}");
    assert!(p.iter().any(|l| l.contains("which the entry doesn't list")), "{p:?}");
}

#[test]
fn a_passing_entry_has_no_due() {
    let listed = Listed::new(
        "passing",
        serde_json::json!({"status": "passing", "tests": [BUILT_TEST, UNBUILT], "due": {UNBUILT: "M2"}}),
        &[("crates/k/tests/t.rs", BUILT)],
    );
    let p = listed.problems();
    assert!(p.iter().any(|l| l.contains("a passing entry still has") && l.contains(UNBUILT)), "{p:?}");
}

#[test]
fn a_feature_test_that_needs_a_live_flag_fails() {
    let rust = "\
#[test]
fn calls_the_gateway() {
    if std::env::var(\"PLANROOM_LIVE_GATEWAY\").is_err() {
        return;
    }
}

#[test]
fn sets_the_flag_on_a_child() {
    let _ = std::process::Command::new(\"planroom\").env(\"PLANROOM_LIVE_GATEWAY\", \"1\");
}
";
    let ts = "test(\"asks_claude\", () => {\n  if (!process.env.PLANROOM_LIVE_CLAUDE) return;\n});\n";
    let sh = "asks_live() {\n  [ -n \"${PLANROOM_LIVE_GATEWAY:-}\" ] || return 0\n}\n\nfor t in asks_live; do\n  $t\ndone\n";
    let listed = Listed::new(
        "live",
        serde_json::json!({
            "status": "failing",
            "tests": [
                "crates/k/tests/live.rs::calls_the_gateway",
                "crates/k/tests/live.rs::sets_the_flag_on_a_child",
                "packages/p/src/live.test.ts::asks_claude",
                "tests/installer/run.sh::asks_live",
            ],
            // A `due` tag doesn't excuse a test that exists but needs a live flag.
            "due": {"packages/p/src/live.test.ts::asks_claude": "M2"},
        }),
        &[
            ("crates/k/tests/live.rs", rust),
            ("packages/p/src/live.test.ts", ts),
            ("tests/installer/run.sh", sh),
        ],
    );
    let p = listed.problems();
    let live = |test: &str| p.iter().any(|l| l.contains(test) && l.contains("needs a live flag"));
    assert!(live("::calls_the_gateway"), "{p:?}");
    assert!(live("::asks_claude"), "{p:?}");
    assert!(live("::asks_live"), "{p:?}");
    // Setting the flag on a child process, to test that the engine refuses it, needs nothing live.
    assert!(!p.iter().any(|l| l.contains("sets_the_flag_on_a_child")), "{p:?}");
}

#[test]
fn a_listed_test_resolves_only_where_ci_runs_it() {
    let listed = Listed::new(
        "runners",
        serde_json::json!({
            "status": "failing",
            "tests": [
                "packages/p/src/a.test.ts::rows_fit",
                "packages/p/src/a.test.tsx::rows_fit",
                "tests/plugin/surface.sh",
                "tests/e2e/src/s.spec.ts",
                "cargo xtask check layout",
                "cargo xtask release-dry-run",
                "crates/k/tests/t.rs::ignored",
            ],
        }),
        &[
            ("packages/p/src/a.test.ts", "test(\"rows_fit: every row fits\", () => {});\n"),
            ("packages/p/src/a.test.tsx", "test(\"rows_fit\", () => {});\n"),
            ("tests/plugin/surface.sh", "echo ok\n"),
            ("tests/e2e/src/s.spec.ts", "test.describe(\"at 390x844\", () => { test(\"x\", () => {}); });\n"),
            ("crates/xtask/src/main.rs", "match args { [\"check\", \"layout\"] => {} }\n"),
            ("crates/k/tests/t.rs", "#[test]\n#[ignore = \"slow\"]\nfn ignored() {\n}\n"),
        ],
    );
    let p = listed.problems();
    let fails = |test: &str| p.iter().any(|l| l.contains(&format!(" {test} ")));
    assert!(!fails("packages/p/src/a.test.ts::rows_fit"), "{p:?}");
    assert!(!fails("tests/e2e/src/s.spec.ts"), "{p:?}");
    assert!(!fails("cargo xtask check layout"), "{p:?}");
    // vitest includes only *.test.ts under src/, and no cargo xtask command runs tests/plugin/.
    assert!(fails("packages/p/src/a.test.tsx::rows_fit"), "{p:?}");
    assert!(fails("tests/plugin/surface.sh"), "{p:?}");
    assert!(fails("cargo xtask release-dry-run"), "{p:?}");
    assert!(fails("crates/k/tests/t.rs::ignored"), "{p:?}");
}

#[test]
fn a_mistyped_filter_fails() {
    use xtask::testrun;
    let listing = "t::a_mistyped_filter_fails: test\nsrc/lib.rs - parse (line 4): test\n";
    assert_eq!(testrun::listed(listing), 2);
    assert_eq!(testrun::unmatched("xtask", "mistyped", listing), None);
    let why = testrun::unmatched("xtask", "a_mistypd_filter", "").expect("an empty listing fails");
    assert!(why.contains("no test in xtask") && why.contains("`a_mistypd_filter`"), "{why}");
    assert_eq!(
        testrun::list_args("xtask", "f"),
        ["test", "--locked", "-p", "xtask", "f", "--", "--list", "--format", "terse"]
    );
    // nextest fails a filter that matches nothing, and passes a whole run with no tests.
    assert_eq!(testrun::no_tests(Some("f")), "fail");
    assert_eq!(testrun::no_tests(None), "pass");
}

#[test]
fn every_test_that_runs_alone_exists() {
    use xtask::testrun;
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let config = fs::read_to_string(repo.join(testrun::NEXTEST_CONFIG)).unwrap();
    let alone = testrun::alone(&config);
    assert!(alone.len() >= 8, "{alone:?}");
    // A renamed test would drop out of the group silently, and run beside the rest again.
    for (binary, name) in alone {
        let (krate, file) = binary.split_once("::").unwrap_or_else(|| panic!("{binary}"));
        let dir = krate.strip_prefix("planroom-").unwrap_or(krate);
        let path = repo.join(format!("crates/{dir}/tests/{file}.rs"));
        let text = fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        assert!(text.contains(&format!("#[test]\nfn {name}(")), "{binary} has no test {name}");
    }
}
