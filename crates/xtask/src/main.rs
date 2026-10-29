//! The one task entry point: `cargo xtask check`, `cargo xtask test`, `cargo xtask gen` and
//! `cargo xtask e2e`.
//!
//! Each command runs every step without stopping at the first failure, prints one `ok`, `FAILED`
//! or `skipped (why)` line per step, and names the fix command on failure. Under `CI=true` a
//! `skipped` step fails unless `crates/xtask/ci_skips.toml` declares it (DR-79).

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use xtask::report::{self, Report};
use xtask::testrun::skips;
use xtask::tools::{self, Tool, pin_age};
use xtask::{
    areas, bench, boundaries, canvas, docs, features, fixture, generate, layout, live, modgen, plugin,
    schema_guard, secrets, testrun, util, workflows,
};

const USAGE: &str = "\
usage: cargo xtask <command>

  check                       every check CI runs
  check boundaries [--self-test]
                              the boundary rules alone, or with the planted crossings
  check layout [--commits RANGE]
                              the state check of layout.lock, and the walk over RANGE's commits
  check audit                 cargo deny check advisories, pnpm audit --prod and the pin-age
                              audit (network)
  check guardrails            the two guardrails test files alone
  test [-p CRATE [FILTER]]    the canvas build, the Rust tests (nextest when installed) and vitest;
                              with -p, one crate's tests, those whose name holds FILTER,
                              or else those of its test file tests/FILTER.rs
  test --ts PATH              vitest on one file or folder
  test --installer            ShellCheck, then tests/installer/run.sh under sh, dash and (Linux) BusyBox
  gen                         write every generated file
  gen --mod                   write only the mod bundle, plugin/hooks/planroom-mod.js
  gen --golden                rerun the Rust tests that write tests/golden/ from real engine runs
  gen --canvas                build the canvas into target/canvas/ (not committed)
  gen --schema-baseline       copy both schema views into schema/baseline/, only with a
                              schema_version bump
  gen --layout-lock DR-n      ratify the gated surfaces with record DR-n, in the commit
                              that adds the record (a branch's merge commit when a
                              branch brought the change)
  e2e [--screens]             build the canvas, the engine and the fixture, then the Playwright
                              specs against planroom serve; --screens also writes
                              docs/screenshots/
  fixture OUT [--no-packs]    build the fixture workspace into OUT, with a config loading the Ruby pack
  live record SNN             record SNN's model calls (needs PLANROOM_LIVE_GATEWAY=1 or
                              PLANROOM_LIVE_JEV=1); prints its estimate from the live spend plan
                              and refuses past what docs/evidence/spend.jsonl has left
  bench fold                  cargo bench fold, each median compared with budgets.json's fold.20k
                              and fold.50k

Every command sets PLANROOM_REPLAY=strict and PLANROOM_SPEND_LOG, the main checkout's
docs/evidence/spend.jsonl by its absolute path; test, e2e and gen --golden also set a
temporary PLANROOM_DATA and unset OPENAI_API_KEY, GEMINI_API_KEY, CLAUDE_CODE_REMOTE, CODESPACES
and REMOTE_CONTAINERS; live record unsets the same five for its cargo test.";

fn main() -> ExitCode {
    let root = repo_root();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let testing = matches!(args.as_slice(), ["test", ..] | ["e2e", ..] | ["gen", "--golden"]);
    let data = model_variables(&root, testing);
    let code = run(&root, &args);
    if let Some(data) = data {
        let _ = std::fs::remove_dir_all(data);
    }
    code
}

/// Set the model variables every command passes on (`live::variables`), before any thread or
/// child process starts. Returns the temporary data root a test run gets, to remove after it.
fn model_variables(root: &Path, testing: bool) -> Option<PathBuf> {
    let mut data = None;
    for (name, value) in live::variables(root, testing) {
        if name == "PLANROOM_DATA" {
            data = value.as_ref().map(PathBuf::from);
        }
        // SAFETY: main calls this first, while the process has one thread and no child.
        unsafe {
            match value {
                Some(v) => std::env::set_var(&name, v),
                None => std::env::remove_var(&name),
            }
        }
    }
    data
}

fn run(root: &Path, args: &[&str]) -> ExitCode {
    let root = root.to_path_buf();
    let mut report = Report::new(&root);
    match args {
        ["check"] => check(&root, &mut report),
        ["check", "boundaries"] => boundary_checks(&root, &mut report, false),
        ["check", "boundaries", "--self-test"] => boundary_checks(&root, &mut report, true),
        ["check", "layout"] => layout_checks(&root, &mut report, None),
        ["check", "layout", "--commits", range] => layout_checks(&root, &mut report, Some(range)),
        ["check", "audit"] => audit(&root, &mut report),
        ["check", "guardrails"] => guardrails(&root, &mut report),
        ["test"] => test(&root, &mut report, None, None),
        ["test", "-p", krate] => test(&root, &mut report, Some(krate), None),
        ["test", "-p", krate, filter] => test(&root, &mut report, Some(krate), Some(filter)),
        ["test", "--ts", path] => ts_test(&root, &mut report, path),
        ["test", "--installer"] => installer(&root, &mut report),
        ["gen"] | ["gen", "--mod"] => {
            match if args.len() == 1 { generate::write_all(&root) } else { generate::write_mod(&root) } {
                Ok(written) => {
                    written.iter().for_each(|p| println!("wrote {p}"));
                    return ExitCode::SUCCESS;
                }
                Err(e) => {
                    eprintln!("cargo xtask gen failed: {e}");
                    return ExitCode::FAILURE;
                }
            }
        }
        ["gen", "--layout-lock", record] => {
            return match layout::write_lock(&root, record) {
                Ok(ratified) if ratified.is_empty() => {
                    println!("{} already matches every gated surface; nothing to ratify", layout::LOCK);
                    ExitCode::SUCCESS
                }
                Ok(ratified) => {
                    println!("wrote {}: {record} ratifies {}", layout::LOCK, ratified.join(", "));
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("cargo xtask gen --layout-lock refused: {e}");
                    ExitCode::FAILURE
                }
            };
        }
        ["gen", "--golden"] => {
            return match golden(&root) {
                Ok(written) => {
                    written.iter().for_each(|p| println!("wrote {p}"));
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("cargo xtask gen --golden failed: {e}");
                    ExitCode::FAILURE
                }
            };
        }
        ["gen", "--schema-baseline"] => {
            return match schema_guard::write_baseline(&root) {
                Ok(written) => {
                    written.iter().for_each(|p| println!("wrote {p}"));
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("cargo xtask gen --schema-baseline refused: {e}");
                    ExitCode::FAILURE
                }
            };
        }
        ["gen", "--canvas"] => {
            return match canvas::build(&root) {
                Ok(files) => {
                    println!("wrote {}/ ({} files)", canvas::OUT, files.len());
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("cargo xtask gen --canvas failed: {e}");
                    ExitCode::FAILURE
                }
            };
        }
        ["bench", "fold"] => bench_fold(&root, &mut report),
        ["e2e"] => e2e(&root, &mut report, false),
        ["e2e", "--screens"] => e2e(&root, &mut report, true),
        ["fixture", out] | ["fixture", out, "--no-packs"] => {
            return match fixture::build(&root, Path::new(out), args.len() == 2) {
                Ok(heads) => {
                    print!("{heads}");
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("cargo xtask fixture failed: {e}");
                    ExitCode::FAILURE
                }
            };
        }
        ["live", "record", scenario] => {
            return match live::record(&root, scenario) {
                Ok(line) => {
                    println!("{line}");
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("cargo xtask live record refused: {e}");
                    ExitCode::FAILURE
                }
            };
        }
        ["help" | "--help" | "-h"] => {
            println!("{USAGE}");
            return ExitCode::SUCCESS;
        }
        _ => {
            eprintln!("{USAGE}");
            return ExitCode::from(2);
        }
    }
    report.finish()
}

fn golden(root: &Path) -> Result<Vec<String>, String> {
    let mut written = Vec::new();
    for (krate, test, files) in generate::GOLDEN_TESTS {
        let out = std::process::Command::new("cargo")
            .args(["test", "--locked", "-p", krate, "--test", test])
            .current_dir(root)
            .env("PLANROOM_GOLDEN", "write")
            .output()
            .map_err(|e| format!("could not run cargo test: {e}"))?;
        if !out.status.success() {
            return Err(format!(
                "cargo test -p {krate} --test {test} failed:\n{}{}",
                String::from_utf8_lossy(&out.stdout),
                String::from_utf8_lossy(&out.stderr)
            ));
        }
        written.extend(files.iter().map(|f| (*f).to_owned()));
    }
    Ok(written)
}

/// `cargo xtask bench fold`: one line per fold case, failing past its `budgets.json` budget.
fn bench_fold(root: &Path, r: &mut Report) {
    const FIX: &str =
        "make the fold faster; a raise of budgets.json's number needs a decision record (DR-59)";
    let budgets = match bench::budgets(root) {
        Ok(b) => b,
        Err(e) => return r.failed("cargo bench fold", &e, "fix budgets.json"),
    };
    let output = match bench::run_fold(root) {
        Ok(out) => out,
        Err(e) => return r.failed("cargo bench fold", &e, "fix the benchmark's build or run"),
    };
    print!("{output}");
    for outcome in bench::compare(&output, &budgets, &bench::FOLD_CASES) {
        match &outcome.failure {
            None => r.ok(&outcome.case),
            Some(why) => r.failed(&outcome.case, why, FIX),
        }
    }
}

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("crates/xtask sits two levels down")
        .to_path_buf()
}

const PNPM_FIX: &str = "pnpm install --frozen-lockfile, then fix what the output names";

/// The unscanned fixture workspace `cargo xtask e2e` builds for the specs to copy.
const E2E_FIXTURE: &str = "target/e2e/fixture/shop";

fn check(root: &Path, r: &mut Report) {
    r.tool(root, "cargo fmt --check", "cargo", &["fmt", "--all", "--check"], "cargo fmt --all");
    r.tool(
        root,
        "clippy -D warnings",
        "cargo",
        &["clippy", "--workspace", "--all-targets", "--locked", "--", "-D", "warnings"],
        "fix the warnings above",
    );
    let js = root.join("node_modules").is_dir();
    let js_step = |r: &mut Report, name: &str, args: &[&str], fix: &str| {
        if js {
            r.tool(root, name, "pnpm", args, fix)
        } else {
            r.failed(name, "node_modules is missing", PNPM_FIX)
        }
    };
    js_step(r, "prettier --check", &["exec", "prettier", "--check", "."], "pnpm exec prettier --write .");
    js_step(r, "eslint", &["exec", "eslint", "--max-warnings", "0", "."], "pnpm exec eslint --fix .");
    js_step(r, "tsc -b", &["exec", "tsc", "-b"], "fix the type errors above");
    js_step(
        r,
        "depcruise",
        boundaries::DEPCRUISE_ARGS,
        "keep imports inside boundaries.toml's [ts.packages]",
    );
    boundary_checks(root, r, true);
    drift_checks(root, r);
    area_checks(root, r);
    docs_checks(root, r);
    layout_checks(root, r, None);
    match layout::cargo_lock_is_current(root, report::under_ci()) {
        Ok(note) => r.ok(&format!("{}{note}", layout::CARGO_LOCK_STEP)),
        Err(e) => r.failed(layout::CARGO_LOCK_STEP, &e, layout::CARGO_LOCK_FIX),
    }
    python_checks(root, r);
    plugin_checks(root, r);
    deny(root, r, &["licenses", "bans", "sources"]);
    match workflows::problems(root) {
        Ok(p) if p.is_empty() => r.ok("ci_runs_only_xtask"),
        Ok(p) => r.failed(
            "ci_runs_only_xtask",
            &p.join("\n"),
            "run the step through cargo xtask, or install the tool from xtask (crates/xtask/src/tools.rs)",
        ),
        Err(e) => r.failed("ci_runs_only_xtask", &e, "fix the error above"),
    }
    match features::problems(root) {
        Ok(p) if p.is_empty() => r.ok("feature_list_is_complete"),
        Ok(p) => r.failed(
            "feature_list_is_complete",
            &p.join("\n"),
            "point each listed test at the test that runs it, or tag it in the entry's `due` with the milestone that builds it",
        ),
        Err(e) => r.failed("feature_list_is_complete", &e, "fix feature_list.json"),
    }
    schema_checks(root, r);
    secret_scan(root, r);
    r.tool(
        root,
        "release_pins",
        "cargo",
        &["test", "--locked", "-p", "planroom", "--test", "release_pins"],
        "make the file the failure names agree with the pin's home (repo-layout.md, Toolchain pins)",
    );
    r.not_built_in("check");
}

/// The schema lint and the breaking-change guard (contracts.md, Versioning rules).
fn schema_checks(root: &Path, r: &mut Report) {
    let name = "schema lint: every 64-bit integer has its 2^53 - 1 maximum";
    let found: Vec<String> = planroom_contract::schema::View::ALL
        .iter()
        .flat_map(|&v| {
            let file = v.file_name();
            schema_guard::lint(&planroom_contract::schema::generate(v))
                .into_iter()
                .map(move |p| format!("{file}{p}"))
        })
        .collect();
    if found.is_empty() {
        r.ok(name)
    } else {
        r.failed(
            name,
            &found.join("\n"),
            "annotate each field with #[schemars(range(max = crate::model::MAX_SAFE_INTEGER))]",
        )
    }
    let name = "breaking-change guard over schema/baseline/";
    match schema_guard::guard(root) {
        Ok(g) if g.problems.is_empty() => r.ok(&format!("{name} ({})", g.compared.join("; "))),
        Ok(g) => r.failed(
            name,
            &g.problems.join("\n"),
            "restore what the lines above name, or bump schema_version with a decision record and run cargo xtask gen --schema-baseline",
        ),
        Err(e) => r.failed(name, &e, "fix the error above"),
    }
}

fn secret_scan(root: &Path, r: &mut Report) {
    let name = "secret scan over tests/replay/ and docs/evidence/";
    match secrets::scan(root) {
        Ok(found) if found.is_empty() => r.ok(name),
        Ok(found) => r.failed(
            name,
            &found.join("\n"),
            "remove the secret from the file and its history, rotate it, and record again with a fake key",
        ),
        Err(e) => r.failed(name, &e, "fix the error above"),
    }
}

/// `cargo deny check` with the given checks, through the pinned cargo-deny.
fn deny(root: &Path, r: &mut Report, checks: &[&str]) {
    let name = format!("cargo deny check {}", checks.join(" "));
    let found = match tools::ensure(root, Tool::Deny) {
        Ok(found) => found,
        Err(why) => return r.skipped(&name, &why),
    };
    let mut command = std::process::Command::new("cargo");
    command.args(["deny", "--locked", "check"]).args(checks).current_dir(root);
    if let Some(path) = found.path_env() {
        command.env("PATH", path);
    }
    match command.stdin(std::process::Stdio::null()).output() {
        Ok(out) if out.status.success() => r.ok(&name),
        Ok(out) => {
            let mut text = String::from_utf8_lossy(&out.stdout).into_owned();
            text.push_str(&String::from_utf8_lossy(&out.stderr));
            r.failed(&name, &text, "fix what cargo deny names, or deny.toml with a decision record")
        }
        Err(e) => r.failed(&name, &format!("could not run cargo deny: {e}"), "fix the error above"),
    }
}

/// `cargo xtask check audit`: the advisory checks, which need the network. Locally they're
/// `skipped (offline)` when it's down; in CI a skip fails [DR-70].
fn audit(root: &Path, r: &mut Report) {
    let online = util::run(root, "pnpm", &["ping"]).0;
    if online {
        deny(root, r, &["advisories"]);
        r.tool(
            root,
            "pnpm audit --prod",
            "pnpm",
            &["audit", "--prod"],
            "update the package the advisory names",
        );
        pin_ages(root, r);
    } else {
        r.skipped("cargo deny check advisories", "offline: the npm registry didn't answer");
        r.skipped("pnpm audit --prod", "offline: the npm registry didn't answer");
        r.skipped(PIN_AGE, "offline: the npm registry didn't answer");
    }
    r.not_built_in("check audit");
}

const PIN_AGE: &str = "pin ages: each direct pin's release at least 14 days older than its commit";

/// Each direct pin's release date from its registry against the commit that set it, with DR-83's
/// advisory exceptions (AGENTS.md, Dependencies; DR-70, DR-83).
fn pin_ages(root: &Path, r: &mut Report) {
    let decisions = match std::fs::read_to_string(root.join(layout::DECISIONS)) {
        Ok(t) => t,
        Err(e) => return r.failed(PIN_AGE, &format!("{}: {e}", layout::DECISIONS), "fix the error above"),
    };
    let exceptions = pin_age::exceptions(&decisions);
    let pins = match pin_age::direct_pins(root) {
        Ok(p) => p,
        Err(e) => return r.failed(PIN_AGE, &e, "fix the error above"),
    };
    let mut problems = Vec::new();
    for pin in &pins {
        let dates = pin_age::set_at(root, pin).and_then(|set| Ok((set, pin_age::released_at(root, pin)?)));
        match dates {
            Ok((set, released)) => problems.extend(pin_age::verdict(pin, set, released, &exceptions)),
            Err(e) => problems.push(format!("{} {}: {e}", pin.name, pin.version)),
        }
    }
    if problems.is_empty() {
        r.ok(&format!("{PIN_AGE} ({} pins, {} advisory exceptions)", pins.len(), exceptions.len()))
    } else {
        r.failed(
            PIN_AGE,
            &problems.join("\n"),
            "pin an older release, or, for a release that fixes an advisory in the tree, name it in DR-83's list and the commit",
        )
    }
}

fn guardrails(root: &Path, r: &mut Report) {
    python_steps(root, r, &GUARDRAILS);
}

/// The area contracts: rows, area files and allow-lists (the blocks are a drift check).
fn area_checks(root: &Path, r: &mut Report) {
    let name = "area_contracts_match_repo_layout";
    match boundaries::load(root).and_then(|rules| areas::problems(root, &rules)) {
        Ok(p) if p.is_empty() => r.ok(name),
        Ok(p) => {
            r.failed(name, &p.join("\n"), "add the row, area files or allow-list entry the lines above name")
        }
        Err(e) => r.failed(name, &e, "fix the error above"),
    }
    let name = "every_directory_has_a_purpose";
    match areas::purpose_problems(root) {
        Ok(p) if p.is_empty() => r.ok(name),
        Ok(p) => r.failed(
            name,
            &p.join("\n"),
            "state each directory's purpose in the Directory map or its area's ## Layout list, and keep each (Mn) tag on exactly the lines whose paths don't exist yet (repo-layout.md, Area contracts)",
        ),
        Err(e) => r.failed(name, &e, "fix the error above"),
    }
}

fn docs_checks(root: &Path, r: &mut Report) {
    let name = "docs: every_image_has_its_prompt_and_spend_row";
    match docs::every_image_has_its_prompt_and_spend_row(root) {
        Ok(found) if found.problems.is_empty() => {
            r.ok(&format!("{name} (Iris spend ${:.2} of ${:.0})", found.total_usd, docs::ART_CAP_USD))
        }
        Ok(found) => r.failed(name, &found.problems.join("\n"), "add the image's prompt and spend row"),
        Err(e) => r.failed(name, &e, "fix the error above"),
    }
}

/// The decision-record gate: the state check, then the walk over `range`, or in CI over what the
/// push or pull request adds [DR-59, DR-70, DR-79].
fn layout_checks(root: &Path, r: &mut Report, range: Option<&str>) {
    let name = "layout gate: state of layout.lock";
    let linked = match layout::linked_worktree(root) {
        Ok(l) => l,
        Err(e) => return r.failed(name, &e, "fix the error above"),
    };
    match layout::state(root) {
        Ok(st) if !st.errors.is_empty() => r.failed(
            name,
            &st.errors.join("\n"),
            "fix decisions.md or layout.lock as the lines above say, in the commit that adds the record; when a branch brought the change, that is its merge commit",
        ),
        Ok(st) if st.changed.is_empty() => r.ok(name),
        Ok(st) if linked => r.ok(&format!(
            "{name}: pending ratification: gated surfaces {} changed; record them in the merge commit",
            st.changed.join(", ")
        )),
        Ok(st) if !st.has_lock => r.failed(
            name,
            &format!("there is no {} yet", layout::LOCK),
            &format!(
                "cargo xtask gen --layout-lock DR-{}, the first lock; when a branch brought the change, run it in its merge commit",
                layout::FIRST_RECORD
            ),
        ),
        Ok(st) => r.failed(
            name,
            &format!("gated surfaces changed with no record: {}", st.changed.join(", ")),
            "write the decision record and run cargo xtask gen --layout-lock DR-n in the same commit; when a branch brought the change, that commit is its merge commit",
        ),
        Err(e) => r.failed(name, &e, "fix the error above"),
    }
    let rev_list = match range {
        Some(range) => vec![range.to_owned()],
        None => match layout::ci_walk_from_env(root) {
            Ok(layout::CiWalk::Range(args)) => args,
            Ok(layout::CiWalk::StateOnly(_)) => return,
            Err(e) => return r.failed("layout gate: the commits CI walks", &e, "fix the error above"),
        },
    };
    let name = format!("layout gate: the commits in {}", rev_list.join(" "));
    match layout::walk(root, &rev_list) {
        Ok(walk) => {
            walk.lines.iter().for_each(|l| println!("    {l}"));
            if walk.failures.is_empty() {
                r.ok(&format!("{name} ({} commits)", walk.commits))
            } else {
                r.failed(&name, &walk.failures.join("\n"), "add the record the lock names in the same commit")
            }
        }
        Err(e) => {
            r.failed(&name, &e, "fetch the full history (fetch-depth: 0), or name a range git can resolve")
        }
    }
}

fn boundary_checks(root: &Path, r: &mut Report, self_test: bool) {
    match boundaries::check(root) {
        Ok(v) if v.is_empty() => r.ok("boundaries"),
        Ok(v) => r.failed(
            "boundaries",
            &v.join("\n"),
            "remove the crossing, or change boundaries.toml with a decision record",
        ),
        Err(e) => r.failed("boundaries", &e, "fix the error above"),
    }
    if self_test {
        match boundaries::self_test(root) {
            Ok(done) => {
                done.lines.iter().for_each(|l| println!("    {l}"));
                r.ok("boundaries self-test (each planted crossing failed its check)");
                for (name, why) in &done.not_built {
                    println!("    not built: {name} ({why})");
                }
            }
            Err(e) => {
                r.failed("boundaries self-test", &e, "make the check that missed its crossing catch it")
            }
        }
    }
}

fn drift_checks(root: &Path, r: &mut Report) {
    let mut groups = Vec::new();
    let mut all_built = true;
    for group in generate::all(root) {
        match group {
            Ok(group) => {
                let name = format!("drift: {}", group.name);
                match generate::drift(root, &group).as_slice() {
                    [] => r.ok(&name),
                    stale => r.failed(
                        &name,
                        &format!("differs from a fresh generation: {}", stale.join(", ")),
                        "cargo xtask gen",
                    ),
                }
                groups.push(group);
            }
            Err(e) => {
                all_built = false;
                r.failed("drift", &e, "cargo xtask gen")
            }
        }
    }
    let written = docs::generator_outputs(&groups);
    let name = "generated_headers: every path in repo-layout.md's Generated directories table is marked";
    match docs::generated_headers(root, all_built.then_some(&written)) {
        Ok(p) if p.is_empty() => r.ok(name),
        Ok(p) => r.failed(
            name,
            &p.join("\n"),
            "run cargo xtask gen, or fix the Generated directories table in docs/contributing/repo-layout.md as the lines above say",
        ),
        Err(e) => r.failed(name, &e, "fix the error above"),
    }
}

const GUARDRAILS: [(&str, &str); 2] = [
    ("guardrails: test_guardrails.py", ".claude/guardrails/tests/test_guardrails.py"),
    ("guardrails: test_context_restore.py", ".claude/guardrails/tests/test_context_restore.py"),
];

fn python_checks(root: &Path, r: &mut Report) {
    let mut steps = GUARDRAILS.to_vec();
    steps.push(("examples/check-fixture", "examples/check-fixture"));
    python_steps(root, r, &steps);
}

fn python_steps(root: &Path, r: &mut Report, steps: &[(&str, &str)]) {
    let version_ok = "import sys; sys.exit(0 if sys.version_info >= (3, 11) else 1)";
    if !util::available("python3", &["-c", version_ok]) {
        for (name, _) in steps {
            r.failed(name, "python3 3.11 or later is required", "install Python 3.11 or later");
        }
        return;
    }
    for &(name, script) in steps {
        r.tool(root, name, "python3", &[script], &format!("python3 {script}"));
    }
}

fn plugin_checks(root: &Path, r: &mut Report) {
    match plugin::layout_problems(root) {
        Ok(p) if p.is_empty() => {
            r.ok("plugin layout (no bin/, no symlinks, names, version, skills, agents, icon)")
        }
        Ok(p) => r.failed("plugin layout", &p.join("\n"), "fix plugin/ as the lines above say"),
        Err(e) => r.failed("plugin layout", &e, "fix the error above"),
    }
    let names = ["claude plugin validate --strict .", "claude plugin validate --strict plugin"];
    match plugin::claude(root) {
        Ok(claude) => {
            for (name, dir) in names.iter().zip([".", "plugin"]) {
                let (ok, out) = plugin::run_claude(root, &claude, &["plugin", "validate", "--strict", dir]);
                if ok { r.ok(name) } else { r.failed(name, &out, "fix the manifest errors above") }
            }
        }
        Err(e) => {
            for name in names {
                r.failed(name, &e, &format!("install Claude Code {}", plugin::CLAUDE_CODE_VERSION));
            }
        }
    }
}

fn test(root: &Path, r: &mut Report, krate: Option<&str>, filter: Option<&str>) {
    let mut scope: Vec<&str> = match krate {
        Some(k) => vec!["-p", k],
        None => vec!["--workspace"],
    };
    // cargo test and nextest both take a name filter after the package.
    scope.extend(filter);
    // The engine embeds target/canvas/ when it builds (build.rs), and `share`'s tests publish it, so
    // the canvas builds first wherever the engine's tests run.
    if matches!(krate, None | Some("planroom")) {
        if !root.join("node_modules").is_dir() {
            r.failed("canvas build", "node_modules is missing", PNPM_FIX);
        } else {
            match canvas::build(root) {
                Ok(files) => r.ok(&format!("canvas build ({} files in {}/)", files.len(), canvas::OUT)),
                Err(e) => r.failed("canvas build", &e, "fix the build errors above"),
            }
        }
    }
    // A filter that matches nothing fails here, so a mistyped name never reports `ok`. One that
    // names no test but names one of the crate's test files, `tests/FILTER.rs`, runs that file.
    if let (Some(k), Some(f)) = (krate, filter) {
        let name = format!("test filter `{f}` in {k}");
        match util::capture(root, "cargo", &testrun::list_args(k, f)) {
            Ok(listing) if testrun::listed(&listing) > 0 => {
                r.ok(&format!("{name} ({} tests)", testrun::listed(&listing)))
            }
            Ok(listing) => match util::capture(root, "cargo", &testrun::list_file_args(k, f)) {
                Ok(file) if testrun::listed(&file) > 0 => {
                    scope.truncate(2);
                    scope.extend(["--test", f]);
                    r.ok(&format!("{name}: tests/{f}.rs ({} tests)", testrun::listed(&file)))
                }
                _ => {
                    let why = testrun::unmatched(k, f, &listing).unwrap_or_default();
                    return r.failed(&name, &why, "check the name, or run the crate's tests with no filter");
                }
            },
            Err(e) => return r.failed(&name, &e, "fix the build errors above"),
        }
    }
    let nextest = tools::ensure(root, Tool::Nextest);
    if nextest.is_ok() {
        // A failing test's output comes at the end, so a CI log's tail shows why it failed.
        let mut args = vec![
            "nextest",
            "run",
            "--locked",
            "--no-fail-fast",
            "--no-tests",
            testrun::no_tests(filter),
            "--failure-output",
            "final",
            "--config-file",
            testrun::NEXTEST_CONFIG,
        ];
        args.extend(&scope);
        r.tool(root, "cargo nextest run", "cargo", &args, "fix the failing tests above");
        // nextest's list names each #[ignore]d test the run skipped.
        let name = "in-test skips (cargo)";
        match util::capture(root, "cargo", &skips::nextest_list_args(&scope))
            .and_then(|j| skips::nextest_ignored(&j))
        {
            Ok(ignored) => skips::report(r, "cargo nextest", &ignored),
            Err(e) => r.failed(name, &e, "fix the error above"),
        }
        if scope.contains(&"--test") {
            r.not_asked("cargo test --doc", "only one test file was asked for");
        } else {
            let mut args = vec!["test", "--doc", "--locked"];
            args.extend(&scope);
            r.tool(root, "cargo test --doc", "cargo", &args, "fix the failing doctests above");
        }
    } else if let Err(why) = nextest {
        r.skipped("cargo nextest run", &format!("{why}; ran cargo test instead"));
        // Every test binary runs, so one failure doesn't hide the next (a CI run shows them all).
        let mut args = vec!["test", "--locked", "--no-fail-fast"];
        args.extend(&scope);
        let (ok, out) = util::run(root, "cargo", &args);
        if ok {
            r.ok("cargo test")
        } else {
            r.failed("cargo test", &out, "fix the failing tests above")
        }
        skips::report(r, "cargo test", &skips::libtest_ignored(&out));
    }
    match krate {
        Some(k) => r.not_asked("vitest run", &format!("only {k} was asked for")),
        None if root.join("node_modules").is_dir() => vitest(root, r),
        None => r.failed("vitest run", "node_modules is missing", PNPM_FIX),
    }
    match krate {
        Some(k) => r.not_asked("claude plugin test target/mod-test", &format!("only {k} was asked for")),
        None => mod_tests(root, r),
    }
    if krate.is_none() {
        r.not_built_in("test");
    }
}

/// vitest over the workspace, with its JSON report read for each test that skipped itself.
fn vitest(root: &Path, r: &mut Report) {
    let report = root.join(skips::VITEST_REPORT);
    let _ = std::fs::remove_file(&report);
    let out_file = format!("--outputFile.json={}", report.display());
    let args = ["exec", "vitest", "run", "--reporter=default", "--reporter=json", &out_file];
    r.tool(root, "vitest run", "pnpm", &args, "fix the failing tests above");
    let found = std::fs::read_to_string(&report)
        .map_err(|e| format!("{}: {e}", skips::VITEST_REPORT))
        .and_then(|json| skips::vitest(&json, &root.display().to_string()));
    match found {
        Ok(skipped) => skips::report(r, "vitest", &skipped),
        Err(e) => r.failed("in-test skips (vitest)", &e, "fix the error above"),
    }
}

/// vitest on one file or folder, while iterating on a TypeScript package.
fn ts_test(root: &Path, r: &mut Report, path: &str) {
    if !root.join("node_modules").is_dir() {
        return r.failed("vitest run", "node_modules is missing", PNPM_FIX);
    }
    r.tool(
        root,
        &format!("vitest run {path}"),
        "pnpm",
        &["exec", "vitest", "run", path],
        "fix the failing tests above",
    )
}

/// Build the `target/mod-test/` harness and run `claude plugin test` on it (DR-59).
fn mod_tests(root: &Path, r: &mut Report) {
    let name = "claude plugin test target/mod-test";
    let harness = match modgen::harness(root) {
        Ok(dir) => dir,
        Err(e) => return r.failed(name, &e, "fix the mod build above"),
    };
    match plugin::claude(root) {
        Ok(claude) => {
            let dir = harness.display().to_string();
            let (ok, out) = plugin::run_claude(root, &claude, &["plugin", "test", &dir]);
            if ok {
                println!("{}", out.trim_end());
                r.ok(name)
            } else {
                r.failed(name, &out, "fix the failing mod tests above")
            }
        }
        Err(e) => r.failed(name, &e, &format!("install Claude Code {}", plugin::CLAUDE_CODE_VERSION)),
    }
}

/// The resolver's offline suite: ShellCheck, then `tests/installer/run.sh` under the system `sh`
/// and dash, and under BusyBox `sh` on Linux, which alone has a BusyBox build, after a debug build
/// of the engine for its checkout test [DR-78, DR-79].
fn installer(root: &Path, r: &mut Report) {
    let scripts = ["plugin/scripts/planroom", "plugin/scripts/contract.sh", "tests/installer/run.sh"];
    match tools::ensure(root, Tool::Shellcheck) {
        Ok(_) => {
            // -x follows the resolver into the contract.sh it sources, from its own directory.
            let mut args = vec!["--shell=sh", "--severity=style", "-x", "--source-path=SCRIPTDIR"];
            args.extend(scripts);
            r.tool(root, "shellcheck", "shellcheck", &args, "fix the findings above");
        }
        Err(why) => r.skipped("shellcheck", &why),
    }
    r.tool(
        root,
        "cargo build -p planroom",
        "cargo",
        &["build", "--locked", "-p", "planroom"],
        "fix the build",
    );
    let suite = "tests/installer/run.sh";
    r.tool(root, "tests/installer/run.sh (sh)", "sh", &[suite], "fix the failing tests above");
    match tools::ensure(root, Tool::Dash) {
        Ok(_) => {
            r.tool(root, "tests/installer/run.sh (dash)", "dash", &[suite], "fix the failing tests above")
        }
        Err(why) => r.skipped("tests/installer/run.sh (dash)", &why),
    }
    if cfg!(target_os = "linux") {
        match tools::ensure(root, Tool::Busybox) {
            Ok(_) => r.tool(
                root,
                "tests/installer/run.sh (busybox sh)",
                "busybox",
                &["sh", suite],
                "fix the failing tests above",
            ),
            Err(why) => r.skipped("tests/installer/run.sh (busybox sh)", &why),
        }
    }
}

/// The canvas's end-to-end specs: build the canvas, the engine with it embedded and the fixture,
/// then run Playwright, whose specs copy the fixture, scan it and drive the real `planroom serve`
/// on it; only the states the engine can't produce yet use `tests/e2e`'s stand-in.
fn e2e(root: &Path, r: &mut Report, screens: bool) {
    if !root.join("node_modules").is_dir() {
        return r.failed("e2e", "node_modules is missing", PNPM_FIX);
    }
    let install = "playwright install chromium";
    if std::env::var_os("CI").is_some() {
        r.tool(
            root,
            install,
            "pnpm",
            &["--dir", "tests/e2e", "exec", "playwright", "install", "--with-deps", "chromium"],
            "pnpm --dir tests/e2e exec playwright install --with-deps chromium",
        );
    } else {
        r.skipped(install, "local runs use the Chromium already installed for the pinned Playwright");
    }
    match canvas::build(root) {
        Ok(files) => r.ok(&format!("canvas build ({} files in {}/)", files.len(), canvas::OUT)),
        Err(e) => return r.failed("canvas build", &e, "fix the build errors above"),
    }
    // The engine embeds target/canvas/ at build time (build.rs), so it builds after the canvas.
    let engine = "cargo build -p planroom (canvas embedded)";
    match util::run(root, "cargo", &["build", "--locked", "-p", "planroom"]) {
        (true, _) => r.ok(engine),
        (false, out) => return r.failed(engine, &out, "fix the build"),
    }
    // One unscanned fixture; each spec copies it, scans the copy and serves it.
    let fixture = root.join(E2E_FIXTURE);
    if let Some(parent) = fixture.parent()
        && parent.exists()
        && let Err(e) = std::fs::remove_dir_all(parent)
    {
        return r.failed("fixture", &format!("{}: {e}", parent.display()), "remove target/e2e/fixture/");
    }
    match fixture::build(root, &fixture, false) {
        Ok(_) => r.ok(&format!("fixture ({E2E_FIXTURE})")),
        Err(e) => return r.failed("fixture", &e, &format!("cargo xtask fixture {E2E_FIXTURE}")),
    }
    let name = if screens { "playwright test (with screenshots)" } else { "playwright test" };
    // The screenshots are a project of their own (tests/e2e/playwright.config.ts), which only
    // --screens runs, so no spec skips itself on a plain run.
    let mut args = vec!["--dir", "tests/e2e", "exec", "playwright", "test"];
    if !screens {
        args.extend(["--project", "specs"]);
    }
    let report = root.join(skips::PLAYWRIGHT_REPORT);
    let _ = std::fs::remove_file(&report);
    let mut command = std::process::Command::new("pnpm");
    command
        .args(&args)
        .current_dir(root)
        .env("PLANROOM_E2E_BIN", root.join("target/debug/planroom"))
        .env("PLANROOM_E2E_FIXTURE", &fixture);
    let ran = command.stdin(std::process::Stdio::null()).output();
    match ran {
        Ok(out) => {
            let mut text = String::from_utf8_lossy(&out.stdout).into_owned();
            text.push_str(&String::from_utf8_lossy(&out.stderr));
            if out.status.success() {
                println!("{}", text.trim_end());
                r.ok(name)
            } else {
                r.failed(name, &text, "fix the failing specs above")
            }
        }
        Err(e) => r.failed(name, &format!("could not run pnpm: {e}"), PNPM_FIX),
    }
    let found = std::fs::read_to_string(&report)
        .map_err(|e| format!("{}: {e}", skips::PLAYWRIGHT_REPORT))
        .and_then(|json| skips::playwright(&json));
    match found {
        Ok(skipped) => skips::report(r, "playwright", &skipped),
        Err(e) => r.failed("in-test skips (playwright)", &e, "fix the error above"),
    }
    r.not_built_in("e2e");
}
