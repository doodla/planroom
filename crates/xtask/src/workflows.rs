//! `ci_runs_only_xtask`: every `run:` step in `.github/workflows/` is a listed setup step or one
//! `cargo xtask` command, so a green local `cargo xtask` means a green CI (repo-layout.md, CI
//! workflows; DR-59).

use std::fs;
use std::path::Path;

/// The setup steps a job may run besides `cargo xtask`. The ones after the first three set up the
/// oldest-git job's `ubuntu:22.04` container, which has neither git nor Rust (A-21).
pub const SETUP_STEPS: &[&str] = &[
    "rustup toolchain install",
    "corepack enable",
    "pnpm install --frozen-lockfile",
    "apt-get update",
    "apt-get install -y --no-install-recommends git build-essential ca-certificates curl pkg-config python3.11",
    "ln -sf /usr/bin/python3.11 /usr/local/bin/python3",
    "curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- --default-toolchain none -y",
    "echo \"$HOME/.cargo/bin\" >> \"$GITHUB_PATH\"",
];

/// Each `run:` command in a workflow file, with its line: a one-line value, or each line of a
/// block scalar (`run: |`).
pub fn run_steps(text: &str) -> Vec<(usize, String)> {
    let lines: Vec<&str> = text.lines().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < lines.len() {
        let line = lines[i];
        let trimmed = line.trim_start().trim_start_matches("- ").trim_start();
        let Some(value) = trimmed.strip_prefix("run:") else {
            i += 1;
            continue;
        };
        let value = value.trim();
        if value.starts_with('|') || value.starts_with('>') {
            let indent = line.len() - line.trim_start().len();
            i += 1;
            while i < lines.len() {
                let l = lines[i];
                let l_indent = l.len() - l.trim_start().len();
                if !l.trim().is_empty() && l_indent <= indent {
                    break;
                }
                if !l.trim().is_empty() {
                    out.push((i + 1, l.trim().to_owned()));
                }
                i += 1;
            }
        } else {
            out.push((i + 1, unquote(value).to_owned()));
            i += 1;
        }
    }
    out
}

/// A one-line value without the quotes of a quoted scalar: only a pair that wraps the whole value,
/// so a plain value that ends in a quote, such as `echo "$X" >> "$GITHUB_PATH"`, stays whole.
fn unquote(value: &str) -> &str {
    for quote in ['"', '\''] {
        if value.len() >= 2 && value.starts_with(quote) && value.ends_with(quote) {
            return &value[1..value.len() - 1];
        }
    }
    value
}

/// Whether a command is a setup step or one `cargo xtask` command.
pub fn allowed(command: &str) -> bool {
    SETUP_STEPS.contains(&command)
        || command.strip_prefix("cargo xtask").is_some_and(|rest| rest.is_empty() || rest.starts_with(' '))
}

/// Every `run:` step under `.github/workflows/` that isn't allowed.
pub fn problems(root: &Path) -> Result<Vec<String>, String> {
    let dir = root.join(".github/workflows");
    let mut files: Vec<_> = fs::read_dir(&dir)
        .map_err(|e| format!("{}: {e}", dir.display()))?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|e| e == "yml" || e == "yaml"))
        .collect();
    files.sort();
    let mut problems = Vec::new();
    for file in files {
        let text = fs::read_to_string(&file).map_err(|e| format!("{}: {e}", file.display()))?;
        let name = file.file_name().and_then(|n| n.to_str()).unwrap_or_default().to_owned();
        for (line, command) in run_steps(&text) {
            if !allowed(&command) {
                problems.push(format!(
                    ".github/workflows/{name}:{line}: `{command}` is neither a setup step nor one cargo xtask command"
                ));
            }
        }
    }
    Ok(problems)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn run_steps_cover_one_line_values_and_block_scalars() {
        let yml = "jobs:\n  a:\n    steps:\n      - run: corepack enable\n      - name: x\n        run: |\n          set -e\n          curl x\n      - run: cargo xtask check\n";
        let steps: Vec<String> = run_steps(yml).into_iter().map(|(_, c)| c).collect();
        assert_eq!(steps, ["corepack enable", "set -e", "curl x", "cargo xtask check"]);
    }

    #[test]
    fn only_setup_steps_and_xtask_are_allowed() {
        assert!(allowed("cargo xtask test --installer"));
        assert!(allowed("cargo xtask release-dry-run --target ${{ matrix.target }}"));
        assert!(allowed("pnpm install --frozen-lockfile"));
        assert!(!allowed("cargo xtaskfoo"));
        assert!(!allowed("cargo deny check"));
        assert!(!allowed("python3 .claude/guardrails/tests/test_guardrails.py"));
    }

    #[test]
    fn a_quoted_value_loses_its_quotes_and_a_plain_one_ending_in_a_quote_keeps_them() {
        let yml = "      - run: \"cargo xtask check\"\n      - run: echo \"$HOME/.cargo/bin\" >> \"$GITHUB_PATH\"\n";
        let steps: Vec<String> = run_steps(yml).into_iter().map(|(_, c)| c).collect();
        assert_eq!(steps, ["cargo xtask check", "echo \"$HOME/.cargo/bin\" >> \"$GITHUB_PATH\""]);
    }
}
