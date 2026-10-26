//! The boundary rules from `boundaries.toml`: the Rust crate graph and allow-lists, the kernel's
//! literal scan, path escapes, the dependency-cruiser config for TypeScript, and the self-test.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Component, Path, PathBuf};

use serde::Deserialize;
use serde_json::{Value, json};

use crate::rustlex;
use crate::util::{self, rel};

/// The built-in packs (repo-layout.md, `packs/`). The kernel may not name their kinds.
const BUILTIN_PACKS: &[&str] = &["code", "plan", "observability", "release", "artifacts", "machines"];

#[derive(Deserialize)]
pub struct Boundaries {
    pub rust: RustRules,
    pub ts: TsRules,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RustRules {
    pub crates: BTreeMap<String, CrateRule>,
    pub kernel: KernelRules,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CrateRule {
    pub may_depend: Vec<String>,
    #[serde(default)]
    pub external_allow: Option<Vec<String>>,
    #[serde(default)]
    pub dev_only: bool,
    #[serde(default = "yes")]
    pub ships: bool,
}

fn yes() -> bool {
    true
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KernelRules {
    pub domain_free: bool,
    pub no_path_escapes: bool,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TsRules {
    pub packages: BTreeMap<String, PackageRule>,
    pub rules: TsGlobalRules,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PackageRule {
    pub may_import: Vec<String>,
    #[serde(default)]
    pub forbid: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TsGlobalRules {
    pub entry_only: bool,
    pub no_circular: bool,
    pub not_to_unresolvable: bool,
    pub generated_read_only: Vec<String>,
}

pub fn load(root: &Path) -> Result<Boundaries, String> {
    let path = root.join("boundaries.toml");
    let text = fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    toml::from_str(&text).map_err(|e| format!("boundaries.toml: {e}"))
}

/// Every boundary violation in the tree at `root`: the crate graph, the kernel scan and path escapes.
pub fn check(root: &Path) -> Result<Vec<String>, String> {
    let rules = load(root)?;
    let mut violations = check_crate_graph(root, &rules)?;
    if rules.rust.kernel.domain_free {
        violations.extend(check_kernel_literals(root)?);
    }
    if rules.rust.kernel.no_path_escapes {
        violations.extend(check_path_escapes(root)?);
    }
    Ok(violations)
}

/// The crate graph from `cargo metadata` against `[rust.crates]`.
pub fn check_crate_graph(root: &Path, rules: &Boundaries) -> Result<Vec<String>, String> {
    let out = util::capture(
        root,
        "cargo",
        &["metadata", "--format-version", "1", "--no-deps", "--offline", "--manifest-path", "Cargo.toml"],
    )?;
    let meta: Value = serde_json::from_str(&out).map_err(|e| format!("cargo metadata: {e}"))?;
    let crates_dir = util::canonical(&root.join("crates"))?;
    let packages = meta["packages"].as_array().ok_or("cargo metadata: no packages")?;

    // Package name -> boundaries key (its directory under crates/).
    let mut keys = BTreeMap::new();
    let mut violations = Vec::new();
    for pkg in packages {
        let name = pkg["name"].as_str().unwrap_or_default().to_owned();
        let manifest = PathBuf::from(pkg["manifest_path"].as_str().unwrap_or_default());
        let dir = manifest.parent().map(Path::to_path_buf).unwrap_or_default();
        match dir.strip_prefix(&crates_dir).ok().and_then(|d| d.to_str()).filter(|d| !d.contains('/')) {
            Some(key) => {
                keys.insert(name, key.to_owned());
            }
            None => violations.push(format!("crate `{name}` is not directly under crates/")),
        }
    }
    for key in rules.rust.crates.keys() {
        if !keys.values().any(|k| k == key) {
            violations
                .push(format!("boundaries.toml names crate `{key}`, which doesn't exist under crates/"));
        }
    }
    for pkg in packages {
        let Some(key) = keys.get(pkg["name"].as_str().unwrap_or_default()) else { continue };
        let Some(rule) = rules.rust.crates.get(key) else {
            violations.push(format!("crate `{key}` has no [rust.crates] entry in boundaries.toml"));
            continue;
        };
        if !rule.ships && pkg["publish"] != json!([]) {
            violations.push(format!("{key} doesn't ship, so its Cargo.toml must set publish = false"));
        }
        for dep in pkg["dependencies"].as_array().into_iter().flatten() {
            let dep_name = dep["name"].as_str().unwrap_or_default();
            let kind = dep["kind"].as_str().unwrap_or("normal");
            match keys.get(dep_name) {
                Some(dep_key) => {
                    let Some(dep_rule) = rules.rust.crates.get(dep_key) else { continue };
                    if !dep_rule.ships {
                        violations.push(format!("{key} -> {dep_key} is not allowed ({dep_key} never ships)"));
                    } else if dep_rule.dev_only {
                        if kind != "dev" {
                            violations.push(format!(
                                "{key} -> {dep_key} is not allowed ({dep_key} is dev-only: use [dev-dependencies])"
                            ));
                        }
                    } else if !rule.may_depend.iter().any(|m| m == "*" || m == dep_key) {
                        violations.push(format!("{key} -> {dep_key} is not allowed"));
                    }
                }
                None => {
                    if let Some(allow) = &rule.external_allow
                        && kind != "dev"
                        && !allow.iter().any(|a| a == dep_name)
                    {
                        violations.push(format!("{key} -> {dep_name} is not in {key}'s external_allow"));
                    }
                }
            }
        }
    }
    Ok(violations)
}

/// No built-in pack ID followed by `.` in a string literal under `crates/kernel/src`.
pub fn check_kernel_literals(root: &Path) -> Result<Vec<String>, String> {
    let mut violations = Vec::new();
    for file in util::rust_files(&root.join("crates/kernel/src"))? {
        let src = fs::read_to_string(&file).map_err(|e| format!("{}: {e}", file.display()))?;
        for lit in rustlex::lex(&src).literals {
            if let Some(kind) = names_pack_kind(&lit.value) {
                violations.push(format!(
                    "{}:{}: the kernel is domain-free, but this string literal names the pack kind `{kind}`",
                    rel(root, &file),
                    lit.line
                ));
            }
        }
    }
    Ok(violations)
}

/// The first `<pack>.<name>` in `text` whose pack is built in, at a word boundary.
fn names_pack_kind(text: &str) -> Option<String> {
    let b = text.as_bytes();
    let word = |c: u8| c.is_ascii_alphanumeric() || c == b'_';
    for pack in BUILTIN_PACKS {
        let pat = format!("{pack}.");
        for (at, _) in text.match_indices(&pat) {
            let before_ok = at == 0 || !word(b[at - 1]);
            let after = at + pat.len();
            if before_ok && b.get(after).is_some_and(|&c| c.is_ascii_lowercase() || c == b'_') {
                let end = b[after..].iter().position(|&c| !word(c)).map_or(b.len(), |n| after + n);
                return Some(text[at..end].to_owned());
            }
        }
    }
    None
}

/// No `#[path = ...]` or `include!`/`include_str!`/`include_bytes!` leaving a crate's directory.
pub fn check_path_escapes(root: &Path) -> Result<Vec<String>, String> {
    let mut violations = Vec::new();
    for entry in fs::read_dir(root.join("crates")).map_err(|e| e.to_string())? {
        let crate_dir = util::canonical(&entry.map_err(|e| e.to_string())?.path())?;
        if !crate_dir.is_dir() {
            continue;
        }
        for file in util::rust_files(&crate_dir)? {
            let src = fs::read_to_string(&file).map_err(|e| format!("{}: {e}", file.display()))?;
            for (line, problem) in path_escapes(&src, file.parent().unwrap_or(&crate_dir), &crate_dir) {
                violations.push(format!("{}:{line}: {problem}", rel(root, &file)));
            }
        }
    }
    Ok(violations)
}

fn path_escapes(src: &str, file_dir: &Path, crate_dir: &Path) -> Vec<(usize, String)> {
    let lexed = rustlex::lex(src);
    let mut found = Vec::new();
    let line_at = |at: usize| src[..at].matches('\n').count() + 1;
    let code = lexed.code.as_bytes();
    let mut check = |at: usize, what: &str, needle_end: usize| {
        // The argument's span: up to the matching `)` for a macro, or the `]` of the attribute.
        let (mut depth, mut end) = (0usize, needle_end);
        while end < code.len() {
            match code[end] {
                b'(' | b'[' => depth += 1,
                b')' | b']' if depth <= 1 => break,
                b')' | b']' => depth -= 1,
                _ => {}
            }
            end += 1;
        }
        let lits: Vec<&rustlex::Literal> =
            lexed.literals.iter().filter(|l| l.start >= needle_end && l.start < end).collect();
        let rest_is_plain = |lit: &rustlex::Literal| {
            let outside = format!("{}{}", &lexed.code[needle_end..lit.start], &lexed.code[lit.start..end]);
            outside.chars().all(|c| c.is_whitespace() || c == '(' || c == '=' || c == ',')
        };
        let escapes = match lits.as_slice() {
            [lit] if rest_is_plain(lit) => {
                let target = Path::new(&lit.value);
                target.is_absolute() || !normalize(&file_dir.join(target)).starts_with(crate_dir)
            }
            // A built path: refuse any `..` segment in it.
            _ => lits.iter().any(|l| l.value.split('/').any(|seg| seg == "..")),
        };
        if escapes {
            found.push((line_at(at), format!("{what} leaves the crate's directory")));
        }
    };
    for what in ["include!", "include_str!", "include_bytes!"] {
        for (at, _) in lexed.code.match_indices(what) {
            let starts_word = at == 0 || !lexed.code.as_bytes()[at - 1].is_ascii_alphanumeric();
            if starts_word {
                check(at, what, at + what.len());
            }
        }
    }
    for (at, m) in lexed.code.match_indices("#[path") {
        check(at, "#[path]", at + m.len());
    }
    found
}

/// Resolve `.` and `..` without touching the file system.
fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in path.components() {
        match c {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            other => out.push(other),
        }
    }
    out
}

/// `.dependency-cruiser.cjs`, generated from `[ts.packages]` and `[ts.rules]`.
pub fn dependency_cruiser_config(root: &Path) -> Result<String, String> {
    let rules = load(root)?;
    let dir_of = |name: &str| -> Result<String, String> {
        ["packages", "tests"]
            .iter()
            .map(|parent| format!("{parent}/{name}"))
            .find(|d| root.join(d).join("package.json").is_file())
            .ok_or_else(|| {
                format!("boundaries.toml names TypeScript package `{name}`, which has no directory")
            })
    };
    let entry_of = |name: &str| -> Result<String, String> {
        let dir = dir_of(name)?;
        let text = fs::read_to_string(root.join(&dir).join("package.json")).map_err(|e| e.to_string())?;
        let pkg: Value = serde_json::from_str(&text).map_err(|e| format!("{dir}/package.json: {e}"))?;
        let entry = match &pkg["exports"] {
            Value::String(s) => s.clone(),
            other => other["."].as_str().ok_or(format!("{dir}/package.json has no \".\" export"))?.to_owned(),
        };
        let entry = entry.trim_start_matches("./");
        Ok(format!("^{}$", regex_escape(&format!("{dir}/{entry}"))))
    };

    let mut forbidden = Vec::new();
    if rules.ts.rules.not_to_unresolvable {
        forbidden.push(json!({"name": "not-to-unresolvable", "severity": "error", "from": {}, "to": {"couldNotResolve": true}}));
    }
    if rules.ts.rules.no_circular {
        forbidden
            .push(json!({"name": "no-circular", "severity": "error", "from": {}, "to": {"circular": true}}));
    }
    for (name, rule) in &rules.ts.packages {
        let from = json!({"path": format!("^{}/", regex_escape(&dir_of(name)?))});
        let (core, npm): (Vec<&String>, Vec<&String>) =
            rule.forbid.iter().partition(|f| f.as_str() == "node:*");
        if !npm.is_empty() {
            let names: Vec<String> = npm.iter().map(|n| regex_escape(n)).collect();
            forbidden.push(json!({
                "name": format!("{name}-forbidden-packages"),
                "comment": format!("{name} must not import {}", npm.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", ")),
                "severity": "error",
                "from": from,
                "to": {"path": format!("(^|/)node_modules/({})/", names.join("|"))},
            }));
        }
        if !core.is_empty() {
            forbidden.push(json!({
                "name": format!("{name}-no-node"),
                "comment": format!("{name} must not use Node APIs"),
                "severity": "error",
                "from": from,
                "to": {"dependencyTypes": ["core"]},
            }));
        }
    }

    let mut allowed = vec![
        json!({"comment": "inside one package", "from": {"path": "^((packages|tests)/[^/]+)/"}, "to": {"path": "^$1/"}}),
        json!({"comment": "third-party packages", "from": {}, "to": {"path": "(^|/)node_modules/"}}),
        json!({"comment": "Node built-ins (the forbidden rules narrow this)", "from": {}, "to": {"dependencyTypes": ["core"]}}),
    ];
    for (name, rule) in &rules.ts.packages {
        if rule.may_import.is_empty() {
            continue;
        }
        let targets = rule
            .may_import
            .iter()
            .map(|t| {
                if rules.ts.rules.entry_only {
                    entry_of(t)
                } else {
                    Ok(format!("^{}/", regex_escape(&dir_of(t)?)))
                }
            })
            .collect::<Result<Vec<_>, _>>()?;
        allowed.push(json!({
            "comment": format!("{name} -> {}", rule.may_import.join(", ")),
            "from": {"path": format!("^{}/", regex_escape(&dir_of(name)?))},
            "to": {"path": targets},
        }));
    }

    let config = json!({
        "forbidden": forbidden,
        "allowed": allowed,
        "allowedSeverity": "error",
        "options": {
            "doNotFollow": {"path": "node_modules"},
            "tsConfig": {"fileName": "tsconfig.base.json"},
            "tsPreCompilationDeps": true,
            "enhancedResolveOptions": {
                "exportsFields": ["exports"],
                "conditionNames": ["import", "types", "default"],
                "extensions": [".ts", ".tsx", ".js"],
            },
        },
    });
    let body = serde_json::to_string_pretty(&config).map_err(|e| e.to_string())?;
    Ok(format!(
        "// {}\n// Source: boundaries.toml ([ts.packages] and [ts.rules]).\n/** @type {{import('dependency-cruiser').IConfiguration}} */\nmodule.exports = {body};\n",
        crate::generate::GENERATED_HEADER
    ))
}

fn regex_escape(s: &str) -> String {
    let mut out = String::new();
    for c in s.chars() {
        if "\\.+*?()|[]{}^$".contains(c) {
            out.push('\\');
        }
        out.push(c);
    }
    out
}

/// The depcruise run every check uses.
pub const DEPCRUISE_ARGS: &[&str] = &[
    "exec",
    "depcruise",
    "packages",
    "tests/e2e",
    "--config",
    ".dependency-cruiser.cjs",
    "--output-type",
    "err",
];

/// Plant each crossing in a temporary worktree and expect each check to catch it.
/// Returns one line per step; any step that wasn't caught is an error.
pub fn self_test(root: &Path) -> Result<Vec<String>, String> {
    let tmp = std::env::temp_dir().join(format!("planroom-boundaries-self-test-{}", std::process::id()));
    let tmp_str = tmp.to_str().ok_or("temp dir is not UTF-8")?.to_owned();
    util::capture(root, "git", &["worktree", "add", "--detach", "--quiet", &tmp_str, "HEAD"])?;
    let _cleanup = Worktree { root: root.to_path_buf(), path: tmp.clone() };
    overlay_working_tree(root, &tmp)?;

    let mut lines = Vec::new();
    let mut failures = Vec::new();

    // Baseline: the unplanted copy passes, so a failure below comes from the plant.
    let baseline = check(&tmp)?;
    if !baseline.is_empty() {
        return Err(format!("the unplanted copy already fails: {}", baseline.join("; ")));
    }
    util::capture(&tmp, "pnpm", &["install", "--offline", "--frozen-lockfile"])?;
    let (ok, out) = util::run(&tmp, "pnpm", DEPCRUISE_ARGS);
    if !ok {
        return Err(format!("depcruise fails on the unplanted copy:\n{out}"));
    }

    // 1. A Rust crossing: kernel -> store.
    let kernel_toml = tmp.join("crates/kernel/Cargo.toml");
    let text = fs::read_to_string(&kernel_toml).map_err(|e| e.to_string())?;
    let planted =
        text.replacen("[dependencies]\n", "[dependencies]\nplanroom-store = { path = \"../store\" }\n", 1);
    if planted == text {
        return Err("crates/kernel/Cargo.toml has no [dependencies] table to plant into".into());
    }
    fs::write(&kernel_toml, planted).map_err(|e| e.to_string())?;
    append(&tmp.join("crates/kernel/src/lib.rs"), "\nuse planroom_store as _;\n")?;
    let found = check_crate_graph(&tmp, &load(&tmp)?)?;
    step(
        &mut lines,
        &mut failures,
        "kernel depends on store",
        found.iter().any(|v| v == "kernel -> store is not allowed"),
    );

    // 2. A pack kind named in the kernel.
    append(&tmp.join("crates/kernel/src/lib.rs"), "\npub const PLANTED: &str = \"code.service\";\n")?;
    let found = check_kernel_literals(&tmp)?;
    step(
        &mut lines,
        &mut failures,
        "\"code.service\" in the kernel",
        found.iter().any(|v| v.contains("`code.service`")),
    );

    // 3. A TypeScript crossing: canvas imports the mod, declared as a dependency so it resolves.
    let canvas_pkg = tmp.join("packages/canvas/package.json");
    let mut pkg: Value = serde_json::from_str(&fs::read_to_string(&canvas_pkg).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    pkg["dependencies"]["@planroom/mod"] = json!("workspace:*");
    fs::write(&canvas_pkg, serde_json::to_string_pretty(&pkg).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    append(&tmp.join("packages/canvas/src/index.ts"), "\nimport \"@planroom/mod\";\n")?;
    // Link it the way pnpm links a workspace dependency. A second `pnpm install --offline` would
    // re-resolve, which needs registry metadata a fresh CI runner has never cached.
    link_workspace_package(&tmp, "packages/canvas", "@planroom/mod", "packages/mod")?;
    let (ok, out) = util::run(&tmp, "pnpm", DEPCRUISE_ARGS);
    step(&mut lines, &mut failures, "canvas imports @planroom/mod", !ok && out.contains("not-in-allowed"));

    // 4. A pack requiring another pack: needs the pack loader.
    lines.push("skipped  a pack requires observability (no pack loader or flags pack yet)".to_owned());

    if failures.is_empty() {
        Ok(lines)
    } else {
        Err(format!("{}\nnot caught: {}", lines.join("\n"), failures.join(", ")))
    }
}

/// Symlinks `<pkg_dir>/node_modules/<name>` to the workspace package at `target`, as pnpm does.
fn link_workspace_package(root: &Path, pkg_dir: &str, name: &str, target: &str) -> Result<(), String> {
    let link = root.join(pkg_dir).join("node_modules").join(name);
    let parent = link.parent().ok_or("link has no parent")?;
    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let _ = fs::remove_file(&link);
    std::os::unix::fs::symlink(root.join(target), &link).map_err(|e| format!("{}: {e}", link.display()))
}

fn step(lines: &mut Vec<String>, failures: &mut Vec<String>, what: &str, caught: bool) {
    if caught {
        lines.push(format!("ok       {what}: caught"));
    } else {
        lines.push(format!("FAILED   {what}: not caught"));
        failures.push(what.to_owned());
    }
}

fn append(path: &Path, text: &str) -> Result<(), String> {
    let mut s = fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
    s.push_str(text);
    fs::write(path, s).map_err(|e| format!("{}: {e}", path.display()))
}

/// Copy the working tree's tracked and untracked (not ignored) files over the worktree, so the
/// self-test covers uncommitted changes too.
fn overlay_working_tree(root: &Path, tmp: &Path) -> Result<(), String> {
    let listed =
        util::capture(root, "git", &["ls-files", "-z", "--cached", "--others", "--exclude-standard"])?;
    for path in listed.split('\0').filter(|p| !p.is_empty()) {
        let (from, to) = (root.join(path), tmp.join(path));
        if from.is_file() {
            if let Some(parent) = to.parent() {
                fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            fs::copy(&from, &to).map_err(|e| format!("{path}: {e}"))?;
        } else if !from.exists() && to.is_file() {
            fs::remove_file(&to).map_err(|e| format!("{path}: {e}"))?;
        }
    }
    Ok(())
}

/// Removes the temporary worktree however the self-test ends.
struct Worktree {
    root: PathBuf,
    path: PathBuf,
}

impl Drop for Worktree {
    fn drop(&mut self) {
        if let Some(p) = self.path.to_str() {
            let _ = util::capture(&self.root, "git", &["worktree", "remove", "--force", p]);
        }
        let _ = fs::remove_dir_all(&self.path);
        let _ = util::capture(&self.root, "git", &["worktree", "prune"]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pack_kinds_are_found_at_word_boundaries_only() {
        assert_eq!(names_pack_kind("code.service").as_deref(), Some("code.service"));
        assert_eq!(names_pack_kind("x plan.item y").as_deref(), Some("plan.item"));
        assert_eq!(names_pack_kind("kernel.group"), None);
        assert_eq!(names_pack_kind("barcode.x"), None);
        assert_eq!(names_pack_kind("the plan. Next"), None);
    }

    #[test]
    fn includes_that_leave_the_crate_are_found() {
        let crate_dir = Path::new("/r/crates/k");
        let src_dir = Path::new("/r/crates/k/src");
        let found = |src: &str| path_escapes(src, src_dir, crate_dir).len();
        assert_eq!(found("const A: &str = include_str!(\"../../../schema/x.json\");"), 1);
        assert_eq!(found("const A: &str = include_str!(\"../Cargo.toml\");"), 0);
        assert_eq!(found("#[path = \"../../other/src/lib.rs\"] mod other;"), 1);
        assert_eq!(found("const A: &str = include_str!(concat!(env!(\"X\"), \"/../../y\"));"), 1);
        assert_eq!(found("// include_str!(\"../../../x\")\nlet s = \"include!(\\\"../../x\\\")\";"), 0);
    }

    #[test]
    fn regex_escape_escapes_metacharacters() {
        assert_eq!(regex_escape("packages/ui/src/index.ts"), "packages/ui/src/index\\.ts");
        assert_eq!(regex_escape("@scope/a+b"), "@scope/a\\+b");
    }
}
