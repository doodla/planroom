//! Running tools and walking files.

use std::collections::BTreeSet;
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

/// The files `git ls-files --cached --others --exclude-standard` holds that exist on disk, so
/// `target/`, `node_modules/` and other ignored paths never count, sorted.
pub fn listed_files(root: &Path) -> Result<Vec<String>, String> {
    let listed = capture(root, "git", &["ls-files", "-z", "--cached", "--others", "--exclude-standard"])?;
    let mut files: Vec<String> =
        listed.split('\0').filter(|p| !p.is_empty() && root.join(p).is_file()).map(str::to_owned).collect();
    files.sort();
    files.dedup();
    Ok(files)
}

/// Every directory that holds one of `files`, at any depth, the root left out.
pub fn directories_of(files: &[String]) -> BTreeSet<String> {
    let mut dirs = BTreeSet::new();
    for file in files {
        let mut at = file.as_str();
        while let Some((parent, _)) = at.rsplit_once('/') {
            if !dirs.insert(parent.to_owned()) {
                break;
            }
            at = parent;
        }
    }
    dirs
}

/// `a/{b,c}/d` as `a/b/d` and `a/c/d`; a path with no brace list as itself.
pub fn expand_braces(path: &str) -> Vec<String> {
    let (Some(open), Some(close)) = (path.find('{'), path.find('}')) else { return vec![path.to_owned()] };
    if close < open {
        return vec![path.to_owned()];
    }
    let (head, tail) = (&path[..open], &path[close + 1..]);
    path[open + 1..close]
        .split(',')
        .flat_map(|alt| expand_braces(&format!("{head}{}{tail}", alt.trim())))
        .collect()
}

/// Whether a path pattern holds a wildcard or a placeholder: `*`, `<name>` or `NN`.
pub fn is_pattern(pattern: &str) -> bool {
    pattern.contains('*') || pattern.contains('<') || pattern.contains("NN")
}

/// Whether `path` matches `pattern`, segment by segment: `**` matches any number of segments,
/// `*` any run of characters within one, `<name>` a run of one or more, and `NN` two digits, so
/// `docs/evidence/sNN/` matches `docs/evidence/s15/`. A trailing `/` on either side is ignored.
pub fn path_matches(pattern: &str, path: &str) -> bool {
    let pattern: Vec<&str> = pattern.trim_end_matches('/').split('/').collect();
    let path: Vec<&str> = path.trim_end_matches('/').split('/').collect();
    segments_match(&pattern, &path)
}

fn segments_match(pattern: &[&str], path: &[&str]) -> bool {
    match pattern.split_first() {
        None => path.is_empty(),
        Some((&"**", rest)) => (0..=path.len()).any(|skip| segments_match(rest, &path[skip..])),
        Some((first, rest)) => path
            .split_first()
            .is_some_and(|(p, prest)| segment_matches(first, p) && segments_match(rest, prest)),
    }
}

fn segment_matches(pattern: &str, text: &str) -> bool {
    let cuts = |from: usize| (from..=text.len()).filter(|i| text.is_char_boundary(*i));
    if let Some(rest) = pattern.strip_prefix('*') {
        return cuts(0).any(|i| segment_matches(rest, &text[i..]));
    }
    if pattern.starts_with('<')
        && let Some(close) = pattern.find('>')
    {
        let rest = &pattern[close + 1..];
        return cuts(1).any(|i| segment_matches(rest, &text[i..]));
    }
    if let Some(rest) = pattern.strip_prefix("NN") {
        let digits = text.bytes().take(2).filter(u8::is_ascii_digit).count();
        return digits == 2 && segment_matches(rest, &text[2..]);
    }
    match (pattern.chars().next(), text.chars().next()) {
        (None, None) => true,
        (Some(p), Some(t)) if p == t => segment_matches(&pattern[p.len_utf8()..], &text[t.len_utf8()..]),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn path_patterns_match_by_segment() {
        assert!(path_matches("docs/evidence/sNN/", "docs/evidence/s15"));
        assert!(!path_matches("docs/evidence/sNN/", "docs/evidence/s1x"));
        assert!(path_matches("plugin/skills/<name>/", "plugin/skills/room-plan"));
        assert!(!path_matches("plugin/skills/<name>/", "plugin/skills"));
        assert!(path_matches("packs/*/evals/*.result.json", "packs/code/evals/a.result.json"));
        assert!(!path_matches("packs/*/evals/*.result.json", "packs/code/x/evals/a.result.json"));
        assert!(path_matches("research/**", "research/a/b"));
        assert!(path_matches("research/**", "research"));
        assert!(path_matches("**/AGENTS.md", "crates/kernel/AGENTS.md"));
        assert!(path_matches("**/AGENTS.md", "AGENTS.md"));
        assert!(path_matches("claude/<purpose>/<key>.json", "claude/plan.stage/ab12.json"));
        assert!(!path_matches("claude/<purpose>/<key>.json", "claude/plan.stage/.json"));
    }

    #[test]
    fn brace_lists_expand() {
        assert_eq!(
            expand_braces("crates/{kernel,store}/clippy.toml"),
            ["crates/kernel/clippy.toml", "crates/store/clippy.toml"]
        );
        assert_eq!(expand_braces("a/b"), ["a/b"]);
    }

    #[test]
    fn directories_are_every_ancestor() {
        let files = vec!["a/b/c.rs".to_owned(), "a/d.rs".to_owned(), "e.rs".to_owned()];
        assert_eq!(directories_of(&files).into_iter().collect::<Vec<_>>(), ["a", "a/b"]);
    }
}
