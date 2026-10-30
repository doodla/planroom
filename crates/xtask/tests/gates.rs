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

    /// A clone that keeps this repository's `layout.lock`, for the checks that read the tree as it
    /// stands rather than the lock's history.
    fn with_lock(name: &str) -> Self {
        let clone = Self::new(name);
        let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        if let Ok(lock) = fs::read(repo.join(LOCK)) {
            fs::write(clone.path(LOCK), lock).unwrap();
            clone.commit("keep the layout.lock");
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
        // A test file's binary is `crate::file`; a crate's own unit tests are the crate's binary,
        // named by their module path, such as `git::tests::name` in `src/git.rs`.
        let (krate, file, test) = match binary.split_once("::") {
            Some((krate, file)) => (krate, format!("tests/{file}.rs"), format!("#[test]\nfn {name}(")),
            None => {
                let (module, test) = name.rsplit_once("::").unwrap_or_else(|| panic!("{name}"));
                let module = module.strip_suffix("::tests").unwrap_or(module);
                let file = format!("src/{}.rs", module.replace("::", "/"));
                (binary.as_str(), file, format!("    #[test]\n    fn {test}("))
            }
        };
        let dir = krate.strip_prefix("planroom-").unwrap_or(krate);
        let path = repo.join(format!("crates/{dir}/{file}"));
        let text = fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        assert!(text.contains(&test), "{binary} has no test {name}");
    }
}

#[test]
fn every_test_that_asserts_a_wall_clock_budget_runs_alone() {
    use xtask::testrun;
    // What a budget looks like, and a deadline loop that is none.
    let sample = "#[test]\nfn timed() {\n    let took = started.elapsed();\n    assert!(took < BUDGET, \"{took:?}\");\n}\n\
        #[test]\n#[cfg(unix)]\nfn inline() {\n    assert!(started.elapsed() < Duration::from_secs(5));\n}\n\
        #[test]\nfn waits() {\n    assert!(ready);\n    while started.elapsed() < limit {}\n    assert!(done);\n}\n\
        mod m {\n    #[test]\n    fn nested() {\n        let t = s.elapsed();\n        assert!(t < B);\n    }\n}\n";
    assert_eq!(testrun::budgeted(sample), ["timed", "inline", "nested"]);

    // Every test in the workspace's test files that asserts one runs with no other test beside it.
    let repo = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let config = fs::read_to_string(repo.join(testrun::NEXTEST_CONFIG)).unwrap();
    let alone = testrun::alone(&config);
    let mut missing = Vec::new();
    let mut found = 0;
    for dir in fs::read_dir(repo.join("crates")).unwrap() {
        let dir = dir.unwrap().path();
        let krate = dir.file_name().unwrap().to_str().unwrap().to_owned();
        // xtask's own tests time nothing; their text holds samples like the one above.
        if krate == "xtask" {
            continue;
        }
        let krate = if krate == "planroom" { krate } else { format!("planroom-{krate}") };
        let Ok(files) = fs::read_dir(dir.join("tests")) else { continue };
        for file in files {
            let path = file.unwrap().path();
            if path.extension().is_none_or(|e| e != "rs") {
                continue;
            }
            let binary = format!("{krate}::{}", path.file_stem().unwrap().to_str().unwrap());
            for name in testrun::budgeted(&fs::read_to_string(&path).unwrap()) {
                found += 1;
                if !alone.contains(&(binary.clone(), name.clone())) {
                    missing.push(format!("{binary} {name}"));
                }
            }
        }
    }
    assert!(found >= 8, "{found}");
    assert!(missing.is_empty(), "add to {}: {missing:?}", testrun::NEXTEST_CONFIG);
}

/// `budgets.json` with `fold.20k`'s `ms` and the S3 cache-hit floor set, the rest as committed.
fn budgets(ms: serde_json::Value, rate: serde_json::Value) -> String {
    let doc = serde_json::json!({
        "S03.plan": {"claude": {"cache_hit_rate": rate}},
        "fold.20k": {"ms": ms},
        "fold.50k": {},
        "surface": {"skill_bodies": {}},
        "room_text": null,
        "recorded": {"token_ratio": null}
    });
    format!("{}\n", serde_json::to_string_pretty(&doc).unwrap())
}

#[test]
fn budgets_raise_needs_a_record() {
    let c = Clone::new("budgets-raise");
    fs::write(c.path(layout::BUDGETS), budgets(400.into(), 0.6.into())).unwrap();
    let base = c.commit("set the fold and cache-hit budgets");
    let fails = |c: &Clone, what: &str| {
        let walk = c.walk(&format!("{base}..HEAD"));
        assert!(
            walk.failures.iter().any(|f| f.contains("without adding a record") && f.contains(what)),
            "{what}: {walk:?}"
        );
    };
    // A larger ceiling fails without a record.
    fs::write(c.path(layout::BUDGETS), budgets(500.into(), 0.6.into())).unwrap();
    c.commit("raise the fold budget");
    fails(&c, "fold.20k.ms: raised from 400 to 500");
    // A lower floor is a raise too, and so are a number set back to null and a removed key.
    git(&c.dir, &["reset", "--quiet", "--hard", &base]);
    fs::write(c.path(layout::BUDGETS), budgets(400.into(), 0.5.into())).unwrap();
    c.commit("lower the cache-hit floor");
    fails(&c, "S03.plan.claude.cache_hit_rate: a floor lowered from 0.6 to 0.5");
    git(&c.dir, &["reset", "--quiet", "--hard", &base]);
    fs::write(c.path(layout::BUDGETS), budgets(serde_json::Value::Null, 0.6.into())).unwrap();
    c.commit("drop the fold number");
    fails(&c, "fold.20k.ms: 400 set back to null");
    git(&c.dir, &["reset", "--quiet", "--hard", &base]);
    c.replace(layout::BUDGETS, "  \"room_text\": null,\n", "");
    c.commit("drop a gated key");
    fails(&c, "room_text: removed");
    // The same raise with a record in its commit passes, and the walk names the record.
    git(&c.dir, &["reset", "--quiet", "--hard", &base]);
    fs::write(c.path(layout::BUDGETS), budgets(500.into(), 0.6.into())).unwrap();
    let record = c.add_record();
    c.commit("raise the fold budget, with its record");
    let walk = c.walk(&format!("{base}..HEAD"));
    assert!(walk.failures.is_empty(), "{walk:?}");
    let named = format!("{record} records the budgets.json raises");
    assert!(walk.lines.iter().any(|l| l.contains(&named)), "{walk:?}");
    // Lowering a ceiling, or raising a floor, needs none.
    git(&c.dir, &["reset", "--quiet", "--hard", &base]);
    fs::write(c.path(layout::BUDGETS), budgets(300.into(), 0.7.into())).unwrap();
    c.commit("tighten both budgets");
    let walk = c.walk(&format!("{base}..HEAD"));
    assert!(walk.failures.is_empty(), "{walk:?}");
}

#[test]
fn a_new_budgets_key_is_not_a_raise() {
    let c = Clone::new("budgets-new-key");
    fs::write(c.path(layout::BUDGETS), budgets(serde_json::Value::Null, 0.6.into())).unwrap();
    let base = c.commit("the budgets with no fold number yet");
    // A number for a null key, a new key, and a `recorded` change: none is a raise (DR-89).
    let mut doc: serde_json::Value = serde_json::from_str(&budgets(400.into(), 0.6.into())).unwrap();
    doc["S02.q1"] = serde_json::json!({"tool_tokens": 1200});
    doc["room_text"] = 300.into();
    doc["recorded"]["token_ratio"] = 1.3.into();
    fs::write(c.path(layout::BUDGETS), serde_json::to_string_pretty(&doc).unwrap()).unwrap();
    c.commit("first numbers and a new question");
    let walk = c.walk(&format!("{base}..HEAD"));
    assert!(walk.failures.is_empty(), "{walk:?}");
    // A budgets file that first appears raises nothing.
    let first = layout::budget_raises_between(None, Some("{\"fold.20k\": {\"ms\": 9}}")).unwrap();
    assert!(first.is_empty(), "{first:?}");
}

#[test]
fn an_in_test_skip_fails_under_ci() {
    use xtask::report::{CiSkips, Report, Skip};
    use xtask::testrun::skips;
    let vitest = r#"{"testResults": [{"name": "/repo/packages/mod/src/watch-engine.test.ts", "assertionResults": [
        {"fullName": "runs", "status": "passed"},
        {"fullName": "the engine's watch run (skipped: no engine)", "status": "skipped"},
        {"fullName": "later", "status": "todo"}]}]}"#;
    let playwright = r#"{"suites": [{"title": "screens.spec.ts", "specs": [], "suites": [{"title": "views",
        "specs": [{"title": "main-1440x900-light.png", "file": "screens.spec.ts",
                   "tests": [{"projectName": "specs", "status": "skipped"}, {"projectName": "screens", "status": "expected"}]}]}]}]}"#;
    let nextest = r#"{"rust-suites": {"planroom::scan": {"testcases": {
        "a_slow_scan": {"ignored": true, "filter-match": {"status": "mismatch", "reason": "ignored"}},
        "a_fast_scan": {"ignored": false, "filter-match": {"status": "matches"}},
        "a_filtered_scan": {"ignored": true, "filter-match": {"status": "mismatch", "reason": "string"}}}}}}"#;
    let cargo = "test scan::a_slow_scan ... ignored\ntest scan::a_fast_scan ... ok\n";
    let found = [
        ("vitest", skips::vitest(vitest, "/repo").unwrap()),
        ("playwright", skips::playwright(playwright).unwrap()),
        ("cargo nextest", skips::nextest_ignored(nextest).unwrap()),
        ("cargo test", skips::libtest_ignored(cargo)),
    ];
    assert_eq!(
        found[0].1,
        [
            "packages/mod/src/watch-engine.test.ts > the engine's watch run (skipped: no engine)",
            "packages/mod/src/watch-engine.test.ts > later"
        ]
    );
    assert_eq!(found[1].1, ["screens.spec.ts > main-1440x900-light.png [specs]"]);
    assert_eq!(found[2].1, ["planroom::scan a_slow_scan"]);
    assert_eq!(found[3].1, ["scan::a_slow_scan"]);
    for (runner, tests) in &found {
        // Under CI each one fails; locally it is a skipped line; ci_skips.toml can declare one.
        let mut ci = Report::with(true, CiSkips::default());
        skips::report(&mut ci, runner, tests);
        assert!(ci.has_failed(), "{runner}: {:?}", ci.lines());
        assert!(ci.lines()[0].starts_with(&format!("FAILED   {runner}: ")), "{:?}", ci.lines());
        let mut local = Report::with(false, CiSkips::default());
        skips::report(&mut local, runner, tests);
        assert!(!local.has_failed() && local.lines()[0].starts_with("skipped  "), "{:?}", local.lines());
        let declared = CiSkips::from_entries(tests.iter().map(|t| Skip {
            check: format!("{runner}: {t}"),
            reason: "no runner has it".into(),
            record: "DR-79".into(),
        }));
        let mut ci = Report::with(true, declared);
        skips::report(&mut ci, runner, tests);
        assert!(!ci.has_failed(), "{:?}", ci.lines());
    }
}

#[test]
fn a_uint64_without_its_annotation_fails() {
    use planroom_contract::schema::{View, generate};
    use xtask::schema_guard::lint;
    // The committed contract's every 64-bit integer carries its maximum.
    for view in View::ALL {
        assert_eq!(lint(&generate(view)), Vec::<String>::new(), "{view:?}");
    }
    // One without it fails, by its path; one above 2^53 - 1 fails too; a signed one needs a minimum.
    let planted = serde_json::json!({"$defs": {
        "Meta": {"properties": {
            "seq": {"type": "integer", "format": "uint64", "minimum": 0},
            "big": {"type": "integer", "format": "uint64", "minimum": 0, "maximum": 18446744073709551615u64},
            "delta": {"type": "integer", "format": "int64", "maximum": 9007199254740991u64},
            "count": {"type": "integer", "format": "uint32", "minimum": 0},
            "ok": {"type": ["integer", "null"], "format": "uint64", "minimum": 0, "maximum": 9007199254740991u64}
        }}
    }});
    let found = lint(&planted);
    assert_eq!(found.len(), 3, "{found:?}");
    for p in ["/$defs/Meta/properties/seq:", "/$defs/Meta/properties/big:", "/$defs/Meta/properties/delta:"] {
        assert!(found.iter().any(|f| f.starts_with(p)), "{p}: {found:?}");
    }
}

#[test]
fn check_names_no_m1_gap() {
    use xtask::report::{CiSkips, NOT_BUILT, Report};
    // `check` names nothing M1 builds as not built; the gates M1 built run instead.
    let m1: Vec<_> = NOT_BUILT.iter().filter(|(cmd, _, m)| *cmd == "check" && *m == "M1").collect();
    assert!(m1.is_empty(), "{m1:?}");
    for built in [
        "breaking-change guard",
        "secret scan",
        "release_pins",
        "feature_list_is_complete",
        "empty-state audit",
    ] {
        assert!(!NOT_BUILT.iter().any(|(_, name, _)| name.contains(built)), "{built} is built");
    }
    let mut r = Report::with(true, CiSkips::default());
    r.not_built_in("check");
    assert!(!r.has_failed());
    assert!(r.lines().iter().all(|l| l.starts_with("not built ") && l.ends_with("(M5)")), "{:?}", r.lines());
    // test and e2e name theirs the same way, each keyed to its own subcommand.
    for subcommand in ["test", "e2e"] {
        let mut r = Report::with(true, CiSkips::default());
        r.not_built_in(subcommand);
        assert!(!r.lines().is_empty() && !r.has_failed(), "{subcommand}: {:?}", r.lines());
    }
    // Each key is a command that prints its lines.
    let main = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("src/main.rs")).unwrap();
    for (cmd, _, _) in NOT_BUILT {
        assert!(main.contains(&format!("r.not_built_in(\"{cmd}\")")), "{cmd} never prints its lines");
    }
}

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[test]
fn every_directory_has_a_purpose() {
    use xtask::areas::purpose_problems;
    let c = Clone::with_lock("purpose");
    let dirs = xtask::util::directories_of(&xtask::util::listed_files(&c.dir).unwrap());
    for dir in [
        ".claude/guardrails/rules",
        ".claude/skills/review-round/scripts",
        "tests/replay",
        "packs/code/tests/inputs/web/src",
    ] {
        assert!(dirs.contains(dir), "the clone holds {dir}");
    }
    assert_eq!(purpose_problems(&c.dir).unwrap(), Vec::<String>::new());

    // A directory nothing names fails, naming it.
    fs::create_dir_all(c.path("crates/store/src/planted")).unwrap();
    fs::write(c.path("crates/store/src/planted/x.rs"), "").unwrap();
    c.commit("a directory with no stated purpose");
    let found = purpose_problems(&c.dir).unwrap();
    assert!(
        found.len() == 1 && found[0].contains("`crates/store/src/planted/` has no stated purpose"),
        "{found:?}"
    );
    fs::remove_dir_all(c.path("crates/store/src/planted")).unwrap();
    assert!(purpose_problems(&c.dir).unwrap().is_empty());

    // A Layout line for a path that doesn't exist fails until it carries the milestone that builds it.
    c.append("crates/store/AGENTS.md", "- `src/later/`: a module a later change adds.\n");
    let found = purpose_problems(&c.dir).unwrap();
    assert!(
        found.len() == 1
            && found[0].contains("crates/store/AGENTS.md:")
            && found[0].contains("`crates/store/src/later/`, which doesn't exist"),
        "{found:?}"
    );
    c.replace("crates/store/AGENTS.md", "- `src/later/`: a module", "- `src/later/`: (M2) a module");
    assert_eq!(purpose_problems(&c.dir).unwrap(), Vec::<String>::new());

    // A tag on a line whose paths all exist fails, so the change that creates a path drops it.
    c.replace("crates/store/AGENTS.md", "- `migrations/`: the", "- `migrations/`: (M2) the");
    let found = purpose_problems(&c.dir).unwrap();
    assert!(
        found.len() == 1 && found[0].contains("carries (M2), but every path it names exists"),
        "{found:?}"
    );
}

#[test]
fn ci_runs_only_xtask() {
    assert_eq!(xtask::workflows::problems(&repo()).unwrap(), Vec::<String>::new());
    let c = Clone::new("ci-steps");
    c.replace(
        ".github/workflows/ci.yml",
        "      - run: cargo xtask check\n",
        "      - run: cargo test\n      - run: cargo xtask check\n",
    );
    let found = xtask::workflows::problems(&c.dir).unwrap();
    assert!(
        found.len() == 1
            && found[0].contains("`cargo test` is neither a setup step nor one cargo xtask command"),
        "{found:?}"
    );

    // The oldest-git job: its container's setup lines are setup steps, exactly as the job spells
    // them, and it runs the git adapter's tests through cargo xtask.
    use xtask::workflows::{allowed, run_steps};
    let text = fs::read_to_string(repo().join(".github/workflows/ci.yml")).unwrap();
    let job = text.split("\n  oldest-git:\n").nth(1).expect("the oldest-git job");
    let job = job.split("\n\n").next().unwrap();
    let steps: Vec<String> = run_steps(job).into_iter().map(|(_, c)| c).collect();
    for xtask in ["cargo xtask test -p planroom-extract", "cargo xtask test -p planroom git_hardening"] {
        assert!(steps.iter().any(|s| s == xtask), "{xtask}: {steps:?}");
    }
    assert!(steps.iter().all(|s| allowed(s)), "{steps:?}");
    // A step of that job that is neither fails, named with its file.
    let c = Clone::new("ci-oldest-git");
    fs::write(c.path(".github/workflows/ci.yml"), &text).unwrap();
    c.replace(
        ".github/workflows/ci.yml",
        "      - run: cargo xtask test -p planroom-extract\n",
        "      - run: cargo test -p planroom-extract\n",
    );
    let found = xtask::workflows::problems(&c.dir).unwrap();
    assert!(
        found.len() == 1
            && found[0].contains("ci.yml")
            && found[0].contains("`cargo test -p planroom-extract`"),
        "{found:?}"
    );
}

#[test]
fn area_contracts_match_repo_layout() {
    use xtask::areas;
    let root = repo();
    let rules = xtask::boundaries::load(&root).unwrap();
    assert_eq!(areas::problems(&root, &rules).unwrap(), Vec::<String>::new());
    for block in areas::generated(&root).unwrap() {
        let committed = fs::read_to_string(root.join(&block.path)).unwrap();
        assert!(committed == block.contents, "{} differs from a fresh generation", block.path);
    }

    let c = Clone::with_lock("areas");
    let problems = || areas::problems(&c.dir, &rules).unwrap();
    // A new top-level directory has no row.
    fs::create_dir_all(c.path("newtop")).unwrap();
    fs::write(c.path("newtop/x.txt"), "x").unwrap();
    let found = problems();
    assert!(found.iter().any(|p| p.contains("`newtop/` has no row")), "{found:?}");
    fs::remove_dir_all(c.path("newtop")).unwrap();
    assert_eq!(problems(), Vec::<String>::new());
    // A How to test it cell naming a file that resolves nowhere.
    let cell = "`cargo xtask test -p planroom-testkit` (its own tests, `truth.rs`)";
    c.replace("docs/contributing/repo-layout.md", cell, &cell.replace("`truth.rs`", "`nowhere.rs`"));
    let found = problems();
    assert!(
        found.len() == 1
            && found[0].contains(
                "names `nowhere.rs`, which resolves neither from the repo root nor under `crates/testkit/`"
            ),
        "{found:?}"
    );
    // A cell naming a test function, whose home is traceability.md and feature_list.json.
    c.replace("docs/contributing/repo-layout.md", "`nowhere.rs`", "`truth_compares_by_name`");
    let found = problems();
    assert!(
        found.len() == 1 && found[0].contains("names `truth_compares_by_name`, a test function"),
        "{found:?}"
    );
}

#[test]
fn a_branch_merge_lists_its_gated_changes() {
    let c = Clone::new("branch");
    let fork = c.head();
    git(&c.dir, &["checkout", "--quiet", "-b", "a-branch"]);
    c.append("boundaries.toml", "\n# a branch's change\n");
    c.commit("a branch changes a boundary");
    c.append("README.md", "\nA change touching no gated surface.\n");
    let tip = c.commit("a branch changes the readme");
    let walk = c.walk(&format!("{fork}..{tip}"));
    assert!(walk.failures.is_empty(), "{walk:?}");
    assert_eq!(walk.commits, 2);
    assert_eq!(walk.lines.len(), 1, "{walk:?}");
    assert!(
        walk.lines[0].ends_with("a branch changes a boundary: gated surfaces changed: boundaries.toml"),
        "{walk:?}"
    );
}

#[test]
fn a_changed_cargo_lock_fails_under_ci() {
    use xtask::layout::cargo_lock_is_current;
    let c = Clone::new("cargo-lock");
    assert_eq!(cargo_lock_is_current(&c.dir, true).unwrap(), "");
    assert_eq!(cargo_lock_is_current(&c.dir, false).unwrap(), "");
    c.append("Cargo.lock", "\n");
    let err = cargo_lock_is_current(&c.dir, true).unwrap_err();
    assert!(err.contains("Cargo.lock differs from the commit"), "{err}");
    let note = cargo_lock_is_current(&c.dir, false).unwrap();
    assert!(note.contains("note: Cargo.lock differs from the commit"), "{note}");
}

#[test]
fn a_golden_read_by_any_literal_path_fails() {
    use xtask::boundaries::{GOLDEN_RULE, check_ts_literals, load};
    let c = Clone::new("golden-reads");
    let rules = load(&c.dir).unwrap();
    assert_eq!(check_ts_literals(&c.dir, &rules).unwrap(), Vec::<String>::new());
    let plant = |rel: &str, text: &str| {
        fs::create_dir_all(c.path(rel).parent().unwrap()).unwrap();
        fs::write(c.path(rel), text).unwrap();
    };
    // A relative path from the file that resolves under tests/golden, each way a script writes one.
    plant(
        "tests/e2e/src/probe.ts",
        "import { readFileSync } from \"node:fs\";\nexport const F = readFileSync(new URL(\"../../golden/cli/focus.json\", import.meta.url));\n",
    );
    plant(
        "tests/probe.ts",
        "export const F = \"golden/cli/focus.json\";\nexport const G = join(here, \"golden\");\n",
    );
    plant("tests/e2e/src/probe2.ts", "export const F = (here: string) => `${here}/../../golden/cli`;\n");
    // The path itself, in the script extensions beyond .ts.
    plant("packages/canvas/src/probe3.mts", "export const F = \"tests/golden/cli/focus.json\";\n");
    plant("packages/canvas/src/probe4.jsx", "export const F = \"tests/golden/cli/focus.json\";\n");
    plant("packages/canvas/src/probe5.cts", "export const F = \"tests/golden/cli/focus.json\";\n");
    // A relative path that stays out of tests/golden passes.
    plant(
        "tests/e2e/src/fine.ts",
        "export const F = \"../../replay/x.json\";\nexport const G = \"./golden.ts\";\n",
    );
    let found = check_ts_literals(&c.dir, &rules).unwrap();
    for at in [
        "tests/e2e/src/probe.ts:2",
        "tests/probe.ts:1",
        "tests/probe.ts:2",
        "tests/e2e/src/probe2.ts:1",
        "packages/canvas/src/probe3.mts:1",
        "packages/canvas/src/probe4.jsx:1",
        "packages/canvas/src/probe5.cts:1",
    ] {
        assert!(
            found.iter().any(|v| v.starts_with(&format!("{at}: {GOLDEN_RULE}"))),
            "{at} isn't caught: {found:#?}"
        );
    }
    assert_eq!(found.len(), 7, "{found:#?}");
}

#[test]
fn a_subtree_wildcard_names_only_a_data_tree() {
    use xtask::areas::purpose_problems;
    let c = Clone::with_lock("stars");
    fs::create_dir_all(c.path("crates/kernel/src/newdir/deeper")).unwrap();
    fs::write(c.path("crates/kernel/src/newdir/deeper/x.rs"), "").unwrap();
    c.append("crates/kernel/AGENTS.md", "- `src/**`: anything.\n");
    let found = purpose_problems(&c.dir).unwrap();
    assert!(
        found.len() == 1
            && found[0].contains("crates/kernel/AGENTS.md:")
            && found[0].contains("`crates/kernel/src/**`")
            && found[0].contains("only where a tree's layout is data"),
        "{found:#?}"
    );
    // So does one in the middle of a path, which names every directory at its depth.
    c.replace("crates/kernel/AGENTS.md", "- `src/**`: anything.", "- `src/**/views/`: anything.");
    let found = purpose_problems(&c.dir).unwrap();
    assert!(
        found.iter().any(|p| p.contains("`crates/kernel/src/**/views`, but `**` stands only")),
        "{found:#?}"
    );
    // A single `*` names nothing, so it can't silence the check for a directory's children.
    c.replace("crates/kernel/AGENTS.md", "- `src/**/views/`: anything.", "- `src/*/`: anything.");
    let found = purpose_problems(&c.dir).unwrap();
    assert!(
        found.iter().any(|p| p.contains("`crates/kernel/src/*`, but a single `*` names no directory")),
        "{found:#?}"
    );
}

#[test]
fn a_test_cell_naming_a_test_function_with_its_file_fails() {
    use xtask::areas;
    let c = Clone::with_lock("cell-fn");
    let rules = xtask::boundaries::load(&c.dir).unwrap();
    let cell = "`cargo xtask test -p planroom-testkit` (its own tests, `truth.rs`)";
    c.replace(
        "docs/contributing/repo-layout.md",
        cell,
        &cell.replace("`truth.rs`", "`truth.rs::truth_compares_by_name`"),
    );
    let found = areas::problems(&c.dir, &rules).unwrap();
    assert!(
        found.len() == 1 && found[0].contains("names `truth.rs::truth_compares_by_name`, a test function"),
        "{found:#?}"
    );
    for written in ["truth::truth_compares_by_name", "truth_compares_by_name()"] {
        c.replace(
            "docs/contributing/repo-layout.md",
            "`truth.rs::truth_compares_by_name`",
            &format!("`{written}`"),
        );
        let found = areas::problems(&c.dir, &rules).unwrap();
        assert!(
            found.len() == 1 && found[0].contains(&format!("names `{written}`, a test function")),
            "{found:#?}"
        );
        c.replace(
            "docs/contributing/repo-layout.md",
            &format!("`{written}`"),
            "`truth.rs::truth_compares_by_name`",
        );
    }
}
