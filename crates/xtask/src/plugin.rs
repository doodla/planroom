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

    let resolver = plugin.join("scripts/planroom");
    match fs::metadata(&resolver) {
        Ok(meta) if std::os::unix::fs::PermissionsExt::mode(&meta.permissions()) & 0o111 != 0 => {}
        Ok(_) => problems.push("plugin/scripts/planroom is not executable".into()),
        Err(_) => problems.push("plugin/scripts/planroom is missing".into()),
    }
    // The resolver sources these two generated files from its own directory (DR-69).
    for generated in ["scripts/engine.env", "scripts/contract.sh"] {
        if !plugin.join(generated).is_file() {
            problems.push(format!("plugin/{generated} is missing; run cargo xtask gen"));
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
    let stamped = manifest["version"].as_str().unwrap_or_default();
    let matches = if version.ends_with("-dev") {
        stamped.strip_prefix(&format!("{version}.g")).is_some_and(|h| h.len() == 8)
    } else {
        stamped == version
    };
    // `claude plugin validate --strict` accepts any string here, so the semver rule is ours.
    if !is_semver(stamped) {
        problems.push(format!("plugin/.claude-plugin/plugin.json: version {stamped:?} is not semver"));
    }
    if !matches {
        problems.push(format!(
            "plugin/.claude-plugin/plugin.json: version {stamped:?} is not the workspace version {version} as cargo xtask gen stamps it"
        ));
    }

    for skill in SKILLS {
        let path = plugin.join("skills").join(skill).join("SKILL.md");
        match frontmatter(&path) {
            Some(front) if front.lines().any(|l| l.trim() == "user-invocable: false") => {}
            Some(_) => problems.push(format!("{}: needs user-invocable: false", rel(root, &path))),
            None => problems.push(format!("{}: missing, or has no frontmatter", rel(root, &path))),
        }
    }
    if let Ok(entries) = fs::read_dir(plugin.join("skills")) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if crate::modgen::MOD_COMMANDS.contains(&name.as_str()) {
                problems.push(format!(
                    "plugin/skills/{name}: a skill can't share its name with the mod's /{name} command; the host refuses the command and skips the mod's start"
                ));
            }
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

pub const MANIFEST: &str = "plugin/.claude-plugin/plugin.json";

/// The plugin's skills, each `user-invocable: false`.
const SKILLS: [&str; 3] = ["planroom", "room-plan", "room-handoff"];

/// sha256 over every file under `plugin/` but the manifest, in sorted path order: each file's path
/// from `plugin/`, a NUL, its length in decimal, a NUL, then its bytes.
pub fn plugin_hash(root: &Path) -> Result<String, String> {
    use sha2::{Digest, Sha256};
    let listed = crate::util::capture(
        root,
        "git",
        &["ls-files", "-z", "--cached", "--others", "--exclude-standard", "--", "plugin"],
    )?;
    let mut files: Vec<&str> =
        listed.split('\0').filter(|p| !p.is_empty() && *p != MANIFEST && root.join(p).is_file()).collect();
    files.sort_unstable();
    files.dedup();
    let mut hash = Sha256::new();
    for file in files {
        let bytes = fs::read(root.join(file)).map_err(|e| format!("{file}: {e}"))?;
        let name = file.strip_prefix("plugin/").unwrap_or(file);
        hash.update(name.as_bytes());
        hash.update([0]);
        hash.update(bytes.len().to_string().as_bytes());
        hash.update([0]);
        hash.update(&bytes);
    }
    Ok(hash.finalize().iter().map(|b| format!("{b:02x}")).collect())
}

/// The plugin's version: `X.Y.Z-dev.g<first 8 hex of plugin_hash>` for a `-dev` workspace, so
/// `/plugin update` sees every change between releases (A-23); a release version as is. The `g`
/// keeps the prerelease identifier alphanumeric, as semver needs.
pub fn stamped_version(root: &Path) -> Result<String, String> {
    let version = workspace_version(root)?;
    if version.ends_with("-dev") {
        Ok(format!("{version}.g{}", &plugin_hash(root)?[..8]))
    } else {
        Ok(version)
    }
}

/// Semver 2.0.0: `MAJOR.MINOR.PATCH`, numbers without leading zeros, then an optional
/// `-prerelease` of dot-separated identifiers (a numeric one without leading zeros) and `+build`.
pub fn is_semver(v: &str) -> bool {
    let ident = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-');
    let number =
        |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit()) && (s == "0" || !s.starts_with('0'));
    let (rest, build) = v.split_once('+').map_or((v, None), |(r, b)| (r, Some(b)));
    let (core, pre) = rest.split_once('-').map_or((rest, None), |(c, p)| (c, Some(p)));
    let core_ok = {
        let parts: Vec<&str> = core.split('.').collect();
        parts.len() == 3 && parts.iter().all(|p| number(p))
    };
    let pre_ok = pre.is_none_or(|p| {
        p.split('.').all(|i| ident(i) && (!i.bytes().all(|b| b.is_ascii_digit()) || number(i)))
    });
    let build_ok = build.is_none_or(|b| b.split('.').all(ident));
    core_ok && pre_ok && build_ok
}

/// `plugin.json` with its `"version"` value stamped and every other byte as written.
pub fn stamped_manifest(root: &Path) -> Result<crate::generate::Generated, String> {
    let text = fs::read_to_string(root.join(MANIFEST)).map_err(|e| format!("{MANIFEST}: {e}"))?;
    let key = "\"version\": \"";
    let start = text.find(key).ok_or_else(|| format!("{MANIFEST} has no \"version\""))? + key.len();
    let end = start + text[start..].find('"').ok_or_else(|| format!("{MANIFEST}: unterminated version"))?;
    let contents = format!("{}{}{}", &text[..start], stamped_version(root)?, &text[end..]);
    Ok(crate::generate::Generated { path: MANIFEST.into(), contents })
}

/// `[workspace.package] version` from the root `Cargo.toml`.
pub fn workspace_version(root: &Path) -> Result<String, String> {
    workspace_package(root, "version")
}

/// `owner/repo` from `[workspace.package] repository`, a GitHub URL.
pub fn workspace_repo(root: &Path) -> Result<String, String> {
    let url = workspace_package(root, "repository")?;
    url.strip_prefix("https://github.com/")
        .map(|r| r.trim_end_matches('/').to_owned())
        .ok_or_else(|| format!("Cargo.toml: repository {url} is not a https://github.com/ URL"))
}

fn workspace_package(root: &Path, key: &str) -> Result<String, String> {
    let text = fs::read_to_string(root.join("Cargo.toml")).map_err(|e| format!("Cargo.toml: {e}"))?;
    let doc: toml::Table = toml::from_str(&text).map_err(|e| format!("Cargo.toml: {e}"))?;
    doc.get("workspace")
        .and_then(|w| w.get("package"))
        .and_then(|p| p.get(key))
        .and_then(|v| v.as_str())
        .map(str::to_owned)
        .ok_or_else(|| format!("Cargo.toml has no [workspace.package] {key}"))
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_dev_stamp_is_semver_and_a_bare_hash_may_not_be() {
        assert!(is_semver("0.1.0"));
        assert!(is_semver("0.1.0-dev.gf7573900"));
        assert!(is_semver("0.1.0-dev.g00000000"));
        assert!(!is_semver("0.1.0-dev.00012345"), "a numeric identifier with a leading zero");
        assert!(!is_semver("0.1"));
        assert!(!is_semver("not a version!"));
        assert!(!is_semver("01.1.0"));
    }
}
