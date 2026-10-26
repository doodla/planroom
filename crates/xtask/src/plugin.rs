//! The shipped plugin: the pinned `claude` CLI, and the checks on `plugin/` that
//! `claude plugin validate` doesn't make (repo-layout.md, Plugin layout).

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

use crate::util::rel;

/// The Claude Code version every plugin check runs against (repo-layout.md, Toolchain pins).
pub const CLAUDE_CODE_VERSION: &str = "2.1.310";

/// The `claude` binary at the pinned version: the one on `PATH` when it matches, else a copy
/// installed under `target/tools/`.
pub fn claude(root: &Path) -> Result<PathBuf, String> {
    let on_path = PathBuf::from("claude");
    if version_of(&on_path).as_deref() == Some(CLAUDE_CODE_VERSION) {
        return Ok(on_path);
    }
    let prefix = root.join("target/tools/claude-code");
    let local = prefix.join("node_modules/.bin/claude");
    if version_of(&local).as_deref() != Some(CLAUDE_CODE_VERSION) {
        let package = format!("@anthropic-ai/claude-code@{CLAUDE_CODE_VERSION}");
        let prefix_arg = prefix.display().to_string();
        let out = Command::new("npm")
            .args(["install", "--no-save", "--no-audit", "--no-fund", "--prefix", &prefix_arg, &package])
            .env("DISABLE_AUTOUPDATER", "1")
            .output()
            .map_err(|e| format!("could not run npm to install {package}: {e}"))?;
        if !out.status.success() {
            return Err(format!("npm install {package} failed:\n{}", String::from_utf8_lossy(&out.stderr)));
        }
    }
    match version_of(&local) {
        Some(v) if v == CLAUDE_CODE_VERSION => Ok(local),
        found => Err(format!("{} reports {found:?}, not {CLAUDE_CODE_VERSION}", local.display())),
    }
}

/// `2.1.310` from `claude --version`'s `2.1.310 (Claude Code)`.
fn version_of(claude: &Path) -> Option<String> {
    let out = Command::new(claude).arg("--version").env("DISABLE_AUTOUPDATER", "1").output().ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    out.status.success().then(|| text.split_whitespace().next().unwrap_or_default().to_owned())
}

/// Run `claude` with the auto-updater off.
pub fn run_claude(root: &Path, claude: &Path, args: &[&str]) -> (bool, String) {
    match Command::new(claude).args(args).current_dir(root).env("DISABLE_AUTOUPDATER", "1").output() {
        Ok(out) => {
            let mut text = String::from_utf8_lossy(&out.stdout).into_owned();
            text.push_str(&String::from_utf8_lossy(&out.stderr));
            (out.status.success(), text)
        }
        Err(e) => (false, format!("could not run {}: {e}", claude.display())),
    }
}

/// What the plugin must hold that validate doesn't check: no `bin/`, no symlinks, one marketplace
/// entry naming the plugin, the workspace version, hidden skills, pinned agents and the icon.
pub fn layout_problems(root: &Path) -> Result<Vec<String>, String> {
    let plugin = root.join("plugin");
    let mut problems = Vec::new();
    if plugin.join("bin").exists() {
        problems.push("plugin/bin exists; a plugin ships no bin/ (scripts go in plugin/scripts/)".into());
    }
    let mut links = Vec::new();
    find_symlinks(&plugin, &mut links)?;
    problems.extend(links.iter().map(|p| format!("{} is a symlink; plugin/ holds none", rel(root, p))));
    if plugin.join(".mcp.json").exists() {
        problems.push("plugin/.mcp.json exists; the plugin has no MCP server".into());
    }
    let hooks = plugin.join("hooks/hooks.json");
    if hooks.exists() {
        let keys: Vec<String> =
            read_json(&hooks)?.as_object().map(|o| o.keys().cloned().collect()).unwrap_or_default();
        if keys != ["modules"] {
            problems.push(
                "plugin/hooks/hooks.json must hold only \"modules\"; the plugin has no classic hooks".into(),
            );
        }
    }

    let market = read_json(&root.join(".claude-plugin/marketplace.json"))?;
    let entries = market["plugins"].as_array().cloned().unwrap_or_default();
    match entries.as_slice() {
        [one] if one["name"] == "planroom" && one["source"] == "./plugin" => {}
        _ => problems.push(
            ".claude-plugin/marketplace.json must list one plugin, \"planroom\", with source \"./plugin\""
                .into(),
        ),
    }
    let manifest = read_json(&plugin.join(".claude-plugin/plugin.json"))?;
    if manifest["name"] != "planroom" {
        problems.push("plugin/.claude-plugin/plugin.json: name must be \"planroom\"".into());
    }
    let version = workspace_version(root)?;
    if manifest["version"].as_str() != Some(version.as_str()) {
        problems.push(format!(
            "plugin/.claude-plugin/plugin.json: version {} is not the workspace version {version}",
            manifest["version"]
        ));
    }

    for skill in ["room", "room-plan", "room-handoff"] {
        let path = plugin.join("skills").join(skill).join("SKILL.md");
        match frontmatter(&path) {
            Some(front) if front.lines().any(|l| l.trim() == "user-invocable: false") => {}
            Some(_) => problems.push(format!("{}: needs user-invocable: false", rel(root, &path))),
            None => problems.push(format!("{}: missing, or has no frontmatter", rel(root, &path))),
        }
    }
    for (agent, model, effort) in [("implementer", "sonnet", "high"), ("curator", "sonnet", "medium")] {
        let path = plugin.join("agents").join(format!("{agent}.md"));
        let front = frontmatter(&path).unwrap_or_default();
        for want in [format!("model: {model}"), format!("effort: {effort}")] {
            if !front.lines().any(|l| l.trim() == want) {
                problems.push(format!("{}: needs {want}", rel(root, &path)));
            }
        }
    }

    let icon = plugin.join(".claude-plugin/icon.png");
    match png_size(&icon) {
        Some((w, h, bytes)) if w == h && (512..=2048).contains(&w) && bytes < 2 * 1024 * 1024 => {}
        Some((w, h, bytes)) => problems.push(format!(
            "plugin/.claude-plugin/icon.png is {w}x{h}, {bytes} bytes; it must be square, 512 to 2048 px, under 2 MB"
        )),
        None => problems.push("plugin/.claude-plugin/icon.png is missing or not a PNG".into()),
    }
    Ok(problems)
}

/// `[workspace.package] version` from the root `Cargo.toml`.
pub fn workspace_version(root: &Path) -> Result<String, String> {
    let text = fs::read_to_string(root.join("Cargo.toml")).map_err(|e| format!("Cargo.toml: {e}"))?;
    let doc: toml::Table = toml::from_str(&text).map_err(|e| format!("Cargo.toml: {e}"))?;
    doc.get("workspace")
        .and_then(|w| w.get("package"))
        .and_then(|p| p.get("version"))
        .and_then(|v| v.as_str())
        .map(str::to_owned)
        .ok_or_else(|| "Cargo.toml has no [workspace.package] version".into())
}

fn read_json(path: &Path) -> Result<Value, String> {
    let text = fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))
}

/// The text between a Markdown file's leading `---` lines.
fn frontmatter(path: &Path) -> Option<String> {
    let text = fs::read_to_string(path).ok()?;
    let rest = text.strip_prefix("---\n")?;
    let end = rest.find("\n---")?;
    Some(rest[..end].to_owned())
}

/// Width, height and file size of a PNG, from its IHDR chunk.
fn png_size(path: &Path) -> Option<(u32, u32, u64)> {
    let bytes = fs::read(path).ok()?;
    if bytes.len() < 24 || &bytes[..8] != b"\x89PNG\r\n\x1a\n" || &bytes[12..16] != b"IHDR" {
        return None;
    }
    let word = |at: usize| u32::from_be_bytes([bytes[at], bytes[at + 1], bytes[at + 2], bytes[at + 3]]);
    Some((word(16), word(20), bytes.len() as u64))
}

fn find_symlinks(dir: &Path, out: &mut Vec<PathBuf>) -> Result<(), String> {
    let entries = fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    for entry in entries {
        let path = entry.map_err(|e| e.to_string())?.path();
        let meta = fs::symlink_metadata(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        if meta.file_type().is_symlink() {
            out.push(path);
        } else if meta.is_dir() {
            find_symlinks(&path, out)?;
        }
    }
    out.sort();
    Ok(())
}
