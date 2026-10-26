//! The one task entry point: `cargo xtask check`, `cargo xtask test` and `cargo xtask gen`.
//!
//! Each command runs every step without stopping at the first failure, prints one `ok`, `FAILED`
//! or `skipped (why)` line per step, and names the fix command on failure.

mod boundaries;
mod generate;
mod plugin;
mod rustlex;
mod util;

use std::path::{Path, PathBuf};
use std::process::ExitCode;

const USAGE: &str = "\
usage: cargo xtask <command>

  check                       every check CI runs
  check boundaries [--self-test]
                              the boundary rules alone, or with the planted crossings
  test [-p CRATE]             the Rust tests (nextest when installed) and vitest
  gen                         write every generated file";

fn main() -> ExitCode {
    let root = repo_root();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let args: Vec<&str> = args.iter().map(String::as_str).collect();
    let mut report = Report::default();
    match args.as_slice() {
        ["check"] => check(&root, &mut report),
        ["check", "boundaries"] => boundary_checks(&root, &mut report, false),
        ["check", "boundaries", "--self-test"] => boundary_checks(&root, &mut report, true),
        ["test"] => test(&root, &mut report, None),
        ["test", "-p", krate] => test(&root, &mut report, Some(krate)),
        ["gen"] => match generate::write_all(&root) {
            Ok(written) => {
                written.iter().for_each(|p| println!("wrote {p}"));
                return ExitCode::SUCCESS;
            }
            Err(e) => {
                eprintln!("cargo xtask gen failed: {e}");
                return ExitCode::FAILURE;
            }
        },
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

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("crates/xtask sits two levels down")
        .to_path_buf()
}

/// One line per step, printed as each finishes, and a summary at the end.
#[derive(Default)]
struct Report {
    lines: Vec<String>,
    failed: bool,
}

impl Report {
    fn ok(&mut self, name: &str) {
        self.line(format!("ok       {name}"));
    }

    fn skipped(&mut self, name: &str, why: &str) {
        self.line(format!("skipped  {name} ({why})"));
    }

    fn failed(&mut self, name: &str, output: &str, fix: &str) {
        let output = output.trim_end();
        if !output.is_empty() {
            let lines: Vec<&str> = output.lines().collect();
            let tail = &lines[lines.len().saturating_sub(80)..];
            println!("---- {name} ----\n{}\n", tail.join("\n"));
        }
        self.failed = true;
        self.line(format!("FAILED   {name} (fix: {fix})"));
    }

    fn line(&mut self, line: String) {
        println!("{line}");
        self.lines.push(line);
    }

    /// Run a tool as one step.
    fn tool(&mut self, root: &Path, name: &str, program: &str, args: &[&str], fix: &str) {
        let (ok, out) = util::run(root, program, args);
        if ok { self.ok(name) } else { self.failed(name, &out, fix) }
    }

    fn finish(self) -> ExitCode {
        println!("\nsummary:");
        self.lines.iter().for_each(|l| println!("  {l}"));
        if self.failed { ExitCode::FAILURE } else { ExitCode::SUCCESS }
    }
}

const PNPM_FIX: &str = "pnpm install --frozen-lockfile, then fix what the output names";

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
    python_checks(root, r);
    plugin_checks(root, r);
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
            Ok(lines) => {
                lines.iter().for_each(|l| println!("    {l}"));
                r.ok("boundaries self-test (each planted crossing failed its check)");
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

fn python_checks(root: &Path, r: &mut Report) {
    let steps = [
        ("guardrails: test_guardrails.py", ".claude/guardrails/tests/test_guardrails.py"),
        ("guardrails: test_context_restore.py", ".claude/guardrails/tests/test_context_restore.py"),
        ("examples/check-fixture", "examples/check-fixture"),
    ];
    let version_ok = "import sys; sys.exit(0 if sys.version_info >= (3, 11) else 1)";
    if !util::available("python3", &["-c", version_ok]) {
        for (name, _) in steps {
            r.failed(name, "python3 3.11 or later is required", "install Python 3.11 or later");
        }
        return;
    }
    for (name, script) in steps {
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
    if util::available("cargo", &["nextest", "--version"]) {
        let mut args = vec!["nextest", "run", "--locked", "--no-tests", "pass"];
        args.extend(&scope);
        r.tool(root, "cargo nextest run", "cargo", &args, "fix the failing tests above");
        let mut args = vec!["test", "--doc", "--locked"];
        args.extend(&scope);
        r.tool(root, "cargo test --doc", "cargo", &args, "fix the failing doctests above");
    } else {
        r.skipped("cargo nextest run", "cargo-nextest is not installed; ran cargo test instead");
        let mut args = vec!["test", "--locked"];
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
}
