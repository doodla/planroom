//! Running tools and walking files.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Run a tool, returning whether it succeeded and its stdout and stderr together.
pub fn run(dir: &Path, program: &str, args: &[&str]) -> (bool, String) {
    match Command::new(program).args(args).current_dir(dir).stdin(Stdio::null()).output() {
        Ok(out) => {
            let mut text = String::from_utf8_lossy(&out.stdout).into_owned();
            text.push_str(&String::from_utf8_lossy(&out.stderr));
            (out.status.success(), text)
        }
        Err(e) => (false, format!("could not run {program}: {e}")),
    }
}

/// Run a tool and return its stdout, or an error with its output.
pub fn capture(dir: &Path, program: &str, args: &[&str]) -> Result<String, String> {
    capture_with_input(dir, program, args, None)
}

/// Like `capture`, writing `input` to the tool's stdin.
pub fn capture_with_input(
    dir: &Path,
    program: &str,
    args: &[&str],
    input: Option<&str>,
) -> Result<String, String> {
    let shown = format!("{program} {}", args.join(" "));
    let mut child = Command::new(program)
        .args(args)
        .current_dir(dir)
        .stdin(if input.is_some() { Stdio::piped() } else { Stdio::null() })
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("could not run {shown}: {e}"))?;
    // Write stdin on a thread so a tool that answers while reading can't deadlock on a full pipe.
    let writer = match (input, child.stdin.take()) {
        (Some(text), Some(mut stdin)) => {
            let text = text.to_owned();
            Some(std::thread::spawn(move || stdin.write_all(text.as_bytes())))
        }
        _ => None,
    };
    let out = child.wait_with_output().map_err(|e| format!("{shown}: {e}"))?;
    if let Some(writer) = writer {
        writer
            .join()
            .map_err(|_| format!("{shown}: stdin writer panicked"))?
            .map_err(|e| format!("{shown}: {e}"))?;
    }
    if out.status.success() {
        Ok(String::from_utf8_lossy(&out.stdout).into_owned())
    } else {
        Err(format!(
            "{shown} failed:\n{}{}",
            String::from_utf8_lossy(&out.stdout),
            String::from_utf8_lossy(&out.stderr)
        ))
    }
}

/// Whether a tool can be started at all.
pub fn available(program: &str, args: &[&str]) -> bool {
    Command::new(program)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .is_ok_and(|s| s.success())
}

pub fn canonical(path: &Path) -> Result<PathBuf, String> {
    fs::canonicalize(path).map_err(|e| format!("{}: {e}", path.display()))
}

/// `path` relative to `root`, for messages.
pub fn rel(root: &Path, path: &Path) -> String {
    let root = fs::canonicalize(root).unwrap_or_else(|_| root.to_path_buf());
    path.strip_prefix(&root).unwrap_or(path).display().to_string()
}

/// Every `.rs` file under `dir`, skipping `target/` and hidden directories, sorted.
pub fn rust_files(dir: &Path) -> Result<Vec<PathBuf>, String> {
    let mut files = Vec::new();
    walk(&canonical(dir)?, &mut |p| {
        if p.extension().is_some_and(|e| e == "rs") {
            files.push(p.to_path_buf());
        }
    })?;
    files.sort();
    Ok(files)
}

/// Every file under `dir`, skipping `target/`, `node_modules/` and hidden directories.
pub fn walk(dir: &Path, f: &mut dyn FnMut(&Path)) -> Result<(), String> {
    let entries = fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    for entry in entries {
        let path = entry.map_err(|e| e.to_string())?.path();
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or_default();
        if path.is_dir() {
            if !(name.starts_with('.') || name == "target" || name == "node_modules") {
                walk(&path, f)?;
            }
        } else {
            f(&path);
        }
    }
    Ok(())
}
