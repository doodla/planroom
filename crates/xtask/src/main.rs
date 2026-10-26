//! The one task entry point: `cargo xtask check`, `cargo xtask test`, `cargo xtask gen` and
//! `cargo xtask e2e`.
//!
//! Each command runs every step without stopping at the first failure, prints one `ok`, `FAILED`
//! or `skipped (why)` line per step, and names the fix command on failure. Under `CI=true` a
//! `skipped` step fails unless `crates/xtask/ci_skips.toml` declares it (DR-79).

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use xtask::report::Report;
use xtask::tools::{self, Tool};
use xtask::{areas, boundaries, canvas, docs, fixture, generate, layout, modgen, plugin, util, workflows};

const USAGE: &str = "\
usage: cargo xtask <command>

  check                       every check CI runs
  check boundaries [--self-test]
                              the boundary rules alone, or with the planted crossings
  check layout [--commits RANGE]
                              the state check of layout.lock, and the walk over RANGE's commits
  check audit                 cargo deny check advisories and pnpm audit --prod (network)
  check guardrails            the two guardrails test files alone
  test [-p CRATE]             the canvas build, the Rust tests (nextest when installed) and vitest
  test --installer            ShellCheck, then tests/installer/run.sh under sh, dash and (Linux) BusyBox
  gen                         write every generated file
  gen --mod                   write only the mod bundle, plugin/hooks/planroom-mod.js
  gen --golden                rerun the Rust tests that write tests/golden/ from real engine runs
  gen --canvas                build the canvas into target/canvas/ (not committed)
  gen --layout-lock DR-n      ratify the gated surfaces with record DR-n (the main agent only)
  e2e [--screens]             build the canvas, the engine and the fixture, then the Playwright
                              specs against planroom serve; --screens also writes
                              docs/screenshots/
  fixture OUT [--no-packs]    build the fixture workspace into OUT, with a config loading the Ruby pack";

fn main() -> ExitCode {
    let root = repo_root();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let mut report = Report::new(&root);
    match args.as_slice() {
        ["check"] => check(&root, &mut report),
        ["check", "boundaries"] => boundary_checks(&root, &mut report, false),
        ["check", "boundaries", "--self-test"] => boundary_checks(&root, &mut report, true),
        ["check", "layout"] => layout_checks(&root, &mut report, None),
        ["check", "layout", "--commits", range] => layout_checks(&root, &mut report, Some(range)),
        ["check", "audit"] => audit(&root, &mut report),
        ["check", "guardrails"] => guardrails(&root, &mut report),
        ["test"] => test(&root, &mut report, None),
        ["test", "-p", krate] => test(&root, &mut report, Some(krate)),
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

/// The Rust tests that own a file under `tests/golden/`, each run once with
/// `PLANROOM_GOLDEN=write`; a plain test run only compares (DR-79).
const GOLDEN_TESTS: &[(&str, &str, &[&str])] = &[(
    "planroom",
    "golden",
    &[
        "tests/golden/cli/focus.json",
        "tests/golden/cli/map-graph.json",
        "tests/golden/cli/map-empty.json",
        "tests/golden/cli/watch-frames.json",
    ],
)];

fn golden(root: &Path) -> Result<Vec<String>, String> {
    let mut written = Vec::new();
    for (krate, test, files) in GOLDEN_TESTS {
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

/// The steps repo-layout.md puts in `check` that nobody has built yet, with the milestone that
/// builds each (architecture.md, Milestones; DR-85).
const CHECK_NOT_BUILT: [(&str, &str); 6] = [
    ("breaking-change guard over schema/baseline/", "M1"),
    ("feature_list_is_complete", "M1"),
    ("secret scan over tests/replay/ and docs/evidence/", "M1"),
    ("release_pins", "M1"),
    ("docs tests (links, prose rules, CLI reference)", "M5"),
    ("scripts/check-tag-version.sh", "M5"),
];

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
    for (name, milestone) in CHECK_NOT_BUILT {
        r.not_built(name, milestone);
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
    } else {
        r.skipped("cargo deny check advisories", "offline: the npm registry didn't answer");
        r.skipped("pnpm audit --prod", "offline: the npm registry didn't answer");
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
            "fix decisions.md or layout.lock as the lines above say (the main agent only)",
        ),
        Ok(st) if st.changed.is_empty() => r.ok(name),
        Ok(st) if linked => r.ok(&format!(
            "{name}: pending ratification: gated surfaces {} changed; the main agent records this at merge",
            st.changed.join(", ")
        )),
        Ok(st) if !st.has_lock => r.failed(
            name,
            &format!("there is no {} yet", layout::LOCK),
            &format!(
                "cargo xtask gen --layout-lock DR-{}, the first lock (the main agent only)",
                layout::FIRST_RECORD
            ),
        ),
        Ok(st) => r.failed(
            name,
            &format!("gated surfaces changed with no record: {}", st.changed.join(", ")),
            "write the decision record, then cargo xtask gen --layout-lock DR-n (the main agent only)",
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
            }
            Err(e) => r.failed("drift", &e, "cargo xtask gen"),
        }
    }
    let globs = boundaries::load(root).map(|b| b.ts.rules.generated_read_only);
    match globs.and_then(|g| generate::missing_headers(root, &g)) {
        Ok(m) if m.is_empty() => r.ok("generated headers"),
        Ok(m) => r.failed(
            "generated headers",
            &format!("no GENERATED header: {}", m.join(", ")),
            "delete the file or generate it with cargo xtask gen",
        ),
        Err(e) => r.failed("generated headers", &e, "fix the error above"),
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

fn test(root: &Path, r: &mut Report, krate: Option<&str>) {
    let scope: Vec<&str> = match krate {
        Some(k) => vec!["-p", k],
        None => vec!["--workspace"],
    };
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
    let nextest = tools::ensure(root, Tool::Nextest);
    if nextest.is_ok() {
        let mut args = vec!["nextest", "run", "--locked", "--no-fail-fast", "--no-tests", "pass"];
        args.extend(&scope);
        r.tool(root, "cargo nextest run", "cargo", &args, "fix the failing tests above");
        let mut args = vec!["test", "--doc", "--locked"];
        args.extend(&scope);
        r.tool(root, "cargo test --doc", "cargo", &args, "fix the failing doctests above");
    } else if let Err(why) = nextest {
        r.skipped("cargo nextest run", &format!("{why}; ran cargo test instead"));
        // Every test binary runs, so one failure doesn't hide the next (a CI run shows them all).
        let mut args = vec!["test", "--locked", "--no-fail-fast"];
        args.extend(&scope);
        r.tool(root, "cargo test", "cargo", &args, "fix the failing tests above");
    }
    match krate {
        Some(k) => r.skipped("vitest run", &format!("only {k} was asked for")),
        None if root.join("node_modules").is_dir() => {
            r.tool(root, "vitest run", "pnpm", &["exec", "vitest", "run"], "fix the failing tests above")
        }
        None => r.failed("vitest run", "node_modules is missing", PNPM_FIX),
    }
    match krate {
        Some(k) => r.skipped("claude plugin test target/mod-test", &format!("only {k} was asked for")),
        None => mod_tests(root, r),
    }
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
    match fixture::build(root, &fixture, true) {
        Ok(_) => r.ok(&format!("fixture ({E2E_FIXTURE})")),
        Err(e) => return r.failed("fixture", &e, &format!("cargo xtask fixture {E2E_FIXTURE}")),
    }
    let name = if screens { "playwright test (with screenshots)" } else { "playwright test" };
    let mut command = std::process::Command::new("pnpm");
    command
        .args(["--dir", "tests/e2e", "exec", "playwright", "test"])
        .current_dir(root)
        .env("PLANROOM_E2E_BIN", root.join("target/debug/planroom"))
        .env("PLANROOM_E2E_FIXTURE", &fixture);
    if screens {
        command.env("PLANROOM_SCREENS", "1");
    }
    match command.stdin(std::process::Stdio::null()).output() {
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
}
