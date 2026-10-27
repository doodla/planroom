//! The boundary rules from `boundaries.toml`: the Rust crate graph and each shipped crate's
//! allow-list, the network rule, the literal scans, path escapes, the generated `clippy.toml` files
//! and dependency-cruiser config, and the self-test (repo-layout.md, Boundaries and their
//! enforcement; DR-59, DR-70, DR-79, DR-81).

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Component, Path, PathBuf};

use serde::Deserialize;
use serde_json::{Value, json};

use crate::generate::{GENERATED_HEADER, Generated};
use crate::rustlex;
use crate::util::{self, rel};

/// The built-in packs (repo-layout.md, `packs/`). The kernel, contract and surfaces may not name
/// their kinds.
pub const BUILTIN_PACKS: &[&str] = &["code", "plan", "observability", "release", "artifacts", "machines"];

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Boundaries {
    pub rust: RustRules,
    pub ts: TsRules,
    pub packs: PackRules,
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
    /// Whether the crate may open a socket: only `models` and `planroom` [DR-79].
    #[serde(default)]
    pub net: bool,
}

impl CrateRule {
    /// A crate that ends up in a release: it ships and isn't a dev-dependency only.
    pub fn shipped(&self) -> bool {
        self.ships && !self.dev_only
    }
}

fn yes() -> bool {
    true
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KernelRules {
    pub domain_free: Vec<String>,
    pub kernel_vocabulary: Vec<String>,
    pub no_path_escapes: bool,
    pub no_io_crates: Vec<String>,
    pub no_io: NoIo,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NoIo {
    pub types: Vec<String>,
    pub methods: Vec<String>,
    pub macros: Vec<String>,
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
    pub third_party: Vec<String>,
    #[serde(default)]
    pub forbid: Vec<String>,
    #[serde(default)]
    pub host_types: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TsGlobalRules {
    pub entry_only: bool,
    pub no_circular: bool,
    pub not_to_unresolvable: bool,
    pub generated_read_only: Vec<String>,
    pub golden_readers: Vec<String>,
    pub domain_free: Vec<String>,
    pub test_tooling: TestTooling,
}

/// The test runner and build tool, which only tests and configs import.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TestTooling {
    pub files: Vec<String>,
    pub third_party: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PackRules {
    pub requires: Vec<Vec<String>>,
    pub out_of_tree_requires: String,
    pub no_executables: bool,
}

pub fn load(root: &Path) -> Result<Boundaries, String> {
    let path = root.join("boundaries.toml");
    let text = fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    toml::from_str(&text).map_err(|e| format!("boundaries.toml: {e}"))
}

/// Every boundary violation in the tree at `root`: the crate graph and allow-lists, the network
/// rule, the literal scans and path escapes.
pub fn check(root: &Path) -> Result<Vec<String>, String> {
    let rules = load(root)?;
    let mut violations = check_crate_graph(root, &rules)?;
    violations.extend(check_sockets(root, &rules)?);
    violations.extend(check_rust_literals(root, &rules)?);
    violations.extend(check_ts_literals(root, &rules)?);
    violations.extend(check_packs(root, &rules)?);
    if rules.rust.kernel.no_path_escapes {
        violations.extend(check_path_escapes(root)?);
    }
    Ok(violations)
}

/// Whether `name` matches an allow-list entry: exactly, or by the prefix before a trailing `*`.
pub fn allowed(list: &[String], name: &str) -> bool {
    list.iter().any(|a| match a.strip_suffix('*') {
        Some(prefix) => name.starts_with(prefix),
        None => a == name,
    })
}

/// The workspace's packages from `cargo metadata --no-deps`, each with its boundaries key (its
/// directory under `crates/`), plus a violation for a crate outside `crates/`.
fn workspace_packages(root: &Path, meta: &Value) -> Result<(BTreeMap<String, String>, Vec<String>), String> {
    let crates_dir = util::canonical(&root.join("crates"))?;
    let mut keys = BTreeMap::new();
    let mut violations = Vec::new();
    for pkg in meta["packages"].as_array().ok_or("cargo metadata: no packages")? {
        if pkg["source"] != Value::Null {
            continue;
        }
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
    Ok((keys, violations))
}

fn metadata(root: &Path, no_deps: bool) -> Result<Value, String> {
    let mut args = vec!["metadata", "--format-version", "1", "--offline", "--manifest-path", "Cargo.toml"];
    if no_deps {
        args.push("--no-deps");
    }
    // Offline first, so the check reads no network when every locked crate is already here. A
    // crate only another platform builds (a Windows console crate on Linux) may be missing from a
    // cache, since no build here fetched it: fetch the locked set once, then read again.
    let out = match util::capture(root, "cargo", &args) {
        Err(e) if e.contains("--offline was specified") => {
            util::capture(root, "cargo", &["fetch", "--locked", "--manifest-path", "Cargo.toml"])?;
            util::capture(root, "cargo", &args)?
        }
        other => other?,
    };
    serde_json::from_str(&out).map_err(|e| format!("cargo metadata: {e}"))
}

/// The crate graph from `cargo metadata` against `[rust.crates]`: workspace edges against
/// `may_depend`, `dev_only` and `ships`; third-party normal and build dependencies against each
/// crate's `external_allow`.
pub fn check_crate_graph(root: &Path, rules: &Boundaries) -> Result<Vec<String>, String> {
    let meta = metadata(root, true)?;
    let (keys, mut violations) = workspace_packages(root, &meta)?;
    for key in rules.rust.crates.keys() {
        if !keys.values().any(|k| k == key) {
            violations
                .push(format!("boundaries.toml names crate `{key}`, which doesn't exist under crates/"));
        }
    }
    for pkg in meta["packages"].as_array().into_iter().flatten() {
        let Some(key) = keys.get(pkg["name"].as_str().unwrap_or_default()) else { continue };
        let Some(rule) = rules.rust.crates.get(key) else {
            violations.push(format!("crate `{key}` has no [rust.crates] entry in boundaries.toml"));
            continue;
        };
        if rule.shipped() && rule.external_allow.is_none() {
            violations
                .push(format!("{key} ships, so boundaries.toml must give it an external_allow (DR-79)"));
        }
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
                                "{key} -> {dep_key} is not allowed ({dep_key} is dev_only: use [dev-dependencies])"
                            ));
                        }
                    } else if !rule.may_depend.iter().any(|m| m == "*" || m == dep_key) {
                        violations.push(format!("{key} -> {dep_key} is not allowed (may_depend)"));
                    }
                }
                None => {
                    if let Some(allow) = &rule.external_allow
                        && kind != "dev"
                        && !allowed(allow, dep_name)
                    {
                        violations.push(format!(
                            "{key} -> {dep_name} is not in {key}'s external_allow ({kind} dependency)"
                        ));
                    }
                }
            }
        }
    }
    Ok(violations)
}

/// The names that mark Rust code able to open a socket, once comments and strings are blanked.
const SOCKET_NAMES: &[&str] = &[
    "TcpStream",
    "TcpListener",
    "UdpSocket",
    "UnixStream",
    "UnixListener",
    "UnixDatagram",
    "socket2",
    "libc::socket",
];

/// The network rule: of the shipped crates, only those marked `net` may reach a socket-capable
/// crate through their normal dependencies. A third-party crate is socket-capable when its Rust
/// code (comments and strings aside) names a socket type, `socket2` or `libc::socket`.
pub fn check_sockets(root: &Path, rules: &Boundaries) -> Result<Vec<String>, String> {
    let meta = metadata(root, false)?;
    let (keys, _) = workspace_packages(root, &meta)?;
    let packages: BTreeMap<&str, &Value> = meta["packages"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|p| Some((p["id"].as_str()?, p)))
        .collect();
    let nodes: BTreeMap<&str, &Value> = meta["resolve"]["nodes"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|n| Some((n["id"].as_str()?, n)))
        .collect();
    let mut verdicts: BTreeMap<String, bool> = BTreeMap::new();
    let mut violations = Vec::new();
    for (id, pkg) in &packages {
        let Some(key) = keys.get(pkg["name"].as_str().unwrap_or_default()) else { continue };
        let Some(rule) = rules.rust.crates.get(key) else { continue };
        if !rule.shipped() || rule.net {
            continue;
        }
        // Every package this crate reaches through normal dependencies.
        let mut seen = BTreeSet::new();
        let mut stack = vec![*id];
        while let Some(at) = stack.pop() {
            for dep in nodes.get(at).and_then(|n| n["deps"].as_array()).into_iter().flatten() {
                let normal = dep["dep_kinds"].as_array().into_iter().flatten().any(|k| k["kind"].is_null());
                let Some(dep_id) = dep["pkg"].as_str() else { continue };
                if normal && seen.insert(dep_id) {
                    stack.push(dep_id);
                }
            }
        }
        for dep_id in seen {
            let Some(dep) = packages.get(dep_id) else { continue };
            // libc binds every C call, sockets among them; the scan finds its callers by `libc::socket`.
            if dep["source"].is_null() || dep["name"] == "libc" {
                continue;
            }
            let name = dep["name"].as_str().unwrap_or_default();
            let manifest = PathBuf::from(dep["manifest_path"].as_str().unwrap_or_default());
            let capable = match verdicts.get(dep_id) {
                Some(v) => *v,
                None => {
                    let v = names_a_socket(manifest.parent().unwrap_or(Path::new(".")))?;
                    verdicts.insert(dep_id.to_owned(), v);
                    v
                }
            };
            if capable {
                violations.push(format!(
                    "{key} reaches {name}, which can open a socket; only crates marked net (models, planroom) may (net)"
                ));
            }
        }
    }
    Ok(violations)
}

fn names_a_socket(dir: &Path) -> Result<bool, String> {
    let mut found = false;
    let src = dir.join("src");
    let scan = if src.is_dir() { src } else { dir.to_path_buf() };
    for file in util::rust_files(&scan)? {
        let text = fs::read_to_string(&file).unwrap_or_default();
        if !SOCKET_NAMES.iter().any(|n| text.contains(n)) {
            continue;
        }
        let code = rustlex::lex(&text).code;
        let word = |c: u8| c.is_ascii_alphanumeric() || c == b'_';
        let bytes = code.as_bytes();
        if SOCKET_NAMES.iter().any(|n| {
            code.match_indices(n).any(|(at, _)| {
                (at == 0 || !word(bytes[at - 1])) && bytes.get(at + n.len()).is_none_or(|&c| !word(c))
            })
        }) {
            found = true;
            break;
        }
    }
    Ok(found)
}

/// No string literal under `domain_free` crates' `src/` names a built-in pack kind or relation:
/// with its `<pack>.` prefix, or bare unless `kernel_vocabulary` holds the word (DR-45).
pub fn check_rust_literals(root: &Path, rules: &Boundaries) -> Result<Vec<String>, String> {
    let mut violations = Vec::new();
    let vocabulary: BTreeSet<&str> = rules.rust.kernel.kernel_vocabulary.iter().map(String::as_str).collect();
    let bare: BTreeMap<String, String> =
        bare_names(root)?.into_iter().filter(|(name, _)| !vocabulary.contains(name.as_str())).collect();
    for krate in &rules.rust.kernel.domain_free {
        let src = root.join("crates").join(krate).join("src");
        if !src.is_dir() {
            continue;
        }
        for file in util::rust_files(&src)? {
            let text = fs::read_to_string(&file).map_err(|e| format!("{}: {e}", file.display()))?;
            for lit in rustlex::lex(&text).literals {
                if let Some(kind) = names_pack_kind(&lit.value) {
                    violations.push(format!(
                        "{}:{}: {krate} is domain-free (domain_free), but this string literal names the pack kind `{kind}`",
                        rel(root, &file),
                        lit.line
                    ));
                } else if let Some(qualified) = bare.get(&lit.value) {
                    violations.push(format!(
                        "{}:{}: {krate} is domain-free (domain_free), but this string literal is the bare name of `{qualified}`; kernel_vocabulary lists the words the kernel spells for its own meaning",
                        rel(root, &file),
                        lit.line
                    ));
                }
            }
        }
    }
    Ok(violations)
}

/// No string literal in the `domain_free` TypeScript packages names a built-in pack kind.
pub fn check_ts_literals(root: &Path, rules: &Boundaries) -> Result<Vec<String>, String> {
    let mut violations = Vec::new();
    for pkg in &rules.ts.rules.domain_free {
        let dir = root.join("packages").join(pkg);
        if !dir.is_dir() {
            continue;
        }
        let mut files = Vec::new();
        util::walk(&dir, &mut |p| {
            let ts = p.extension().is_some_and(|e| e == "ts" || e == "tsx");
            let vendored = p.components().any(|c| c.as_os_str() == "vendor");
            if ts && !vendored {
                files.push(p.to_path_buf());
            }
        })?;
        files.sort();
        for file in files {
            let text = fs::read_to_string(&file).map_err(|e| format!("{}: {e}", file.display()))?;
            for (line, value) in ts_string_literals(&text) {
                if let Some(kind) = names_pack_kind(&value) {
                    violations.push(format!(
                        "{}:{line}: {pkg} is domain-free (domain_free), but this string literal names the pack kind `{kind}`; style from MapNode.tone",
                        rel(root, &file)
                    ));
                }
            }
        }
    }
    Ok(violations)
}

/// The string and template literals of a TypeScript file, with their lines, comments skipped.
/// Regular expression literals aren't told apart from division, which the scan can live with.
pub fn ts_string_literals(src: &str) -> Vec<(usize, String)> {
    let b = src.as_bytes();
    let mut out = Vec::new();
    let mut i = 0;
    let line_at = |at: usize| b[..at].iter().filter(|&&c| c == b'\n').count() + 1;
    while i < b.len() {
        match b[i] {
            b'/' if b.get(i + 1) == Some(&b'/') => {
                i = src[i..].find('\n').map_or(b.len(), |n| i + n);
            }
            b'/' if b.get(i + 1) == Some(&b'*') => {
                i = src[i + 2..].find("*/").map_or(b.len(), |n| i + 2 + n + 2);
            }
            q @ (b'"' | b'\'' | b'`') => {
                // A template's `${...}` parts are code, not text: they read as one space.
                let mut j = i + 1;
                let mut text = String::new();
                let mut from = j;
                while j < b.len() && b[j] != q {
                    if q == b'`' && b[j] == b'$' && b.get(j + 1) == Some(&b'{') {
                        text.push_str(&src[from..j]);
                        text.push(' ');
                        let mut depth = 0usize;
                        while j < b.len() {
                            match b[j] {
                                b'{' => depth += 1,
                                b'}' => {
                                    depth -= 1;
                                    if depth == 0 {
                                        break;
                                    }
                                }
                                _ => {}
                            }
                            j += 1;
                        }
                        j += 1;
                        from = j.min(b.len());
                        continue;
                    }
                    j += if b[j] == b'\\' { 2 } else { 1 };
                }
                let end = j.min(b.len());
                text.push_str(&src[from.min(end)..end]);
                out.push((line_at(i), text));
                i = end + 1;
            }
            _ => i += 1,
        }
    }
    out
}

/// One pack's manifest, as far as the boundary checks read it.
#[derive(Deserialize)]
struct Manifest {
    id: String,
    #[serde(default)]
    requires: Vec<String>,
    #[serde(default)]
    kinds: Vec<Named>,
    #[serde(default)]
    relations: Vec<Named>,
}

#[derive(Deserialize)]
struct Named {
    id: String,
}

/// Each pack directory under `parent` (`packs` or `examples/packs`) that holds a manifest, with
/// it, in path order.
fn manifests(root: &Path, parent: &str) -> Result<Vec<(PathBuf, Manifest)>, String> {
    let Ok(entries) = fs::read_dir(root.join(parent)) else { return Ok(Vec::new()) };
    let mut dirs: Vec<PathBuf> = entries.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect();
    dirs.sort();
    let mut out = Vec::new();
    for dir in dirs {
        let path = dir.join("pack.toml");
        let Ok(text) = fs::read_to_string(&path) else { continue };
        let manifest: Manifest = toml::from_str(&text).map_err(|e| format!("{}: {e}", rel(root, &path)))?;
        out.push((path, manifest));
    }
    Ok(out)
}

/// Every built-in pack's kind and relation names, bare, each with its qualified name.
fn bare_names(root: &Path) -> Result<BTreeMap<String, String>, String> {
    let mut names = BTreeMap::new();
    for (_, manifest) in manifests(root, "packs")? {
        for named in manifest.kinds.iter().chain(&manifest.relations) {
            names.entry(named.id.clone()).or_insert_with(|| format!("{}.{}", manifest.id, named.id));
        }
    }
    Ok(names)
}

/// The `[packs]` rules: a built-in pack's `requires` is one of the listed values, an out-of-tree
/// pack requires only built-in packs, and no pack holds an executable file (DR-9, DR-62).
pub fn check_packs(root: &Path, rules: &Boundaries) -> Result<Vec<String>, String> {
    let mut violations = Vec::new();
    let builtins = manifests(root, "packs")?;
    let ids: BTreeSet<&str> = builtins.iter().map(|(_, m)| m.id.as_str()).collect();
    for (path, manifest) in &builtins {
        if !rules.packs.requires.contains(&manifest.requires) {
            violations.push(format!(
                "{}: the built-in pack {} requires {:?}, but a built-in pack requires one of {:?} ([packs] requires): packs depend inward",
                rel(root, path),
                manifest.id,
                manifest.requires,
                rules.packs.requires
            ));
        }
    }
    if rules.packs.out_of_tree_requires == "built_in" {
        for (path, manifest) in manifests(root, "examples/packs")? {
            for required in manifest.requires.iter().filter(|r| !ids.contains(r.as_str())) {
                violations.push(format!(
                    "{}: the out-of-tree pack {} requires {required}, which isn't a built-in pack (out_of_tree_requires): an out-of-tree pack never requires another",
                    rel(root, &path),
                    manifest.id
                ));
            }
        }
    }
    if rules.packs.no_executables {
        for parent in ["packs", "examples/packs"] {
            let dir = root.join(parent);
            if !dir.is_dir() {
                continue;
            }
            let mut found = Vec::new();
            util::walk(&dir, &mut |p| {
                use std::os::unix::fs::PermissionsExt;
                if fs::metadata(p).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0) {
                    found.push(p.to_path_buf());
                }
            })?;
            found.sort();
            for path in found {
                violations.push(format!(
                    "{}: a pack holds data only, but this file is executable (no_executables)",
                    rel(root, &path)
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

/// The public functions and types of each `std` module `no_io` names with `::*`, as of the pinned
/// Rust (1.97): clippy matches item paths, not modules, so `gen` expands each module [DR-70].
const STD_MODULES: &[(&str, &[&str], &[&str])] = &[
    (
        "std::fs",
        &[
            "canonicalize",
            "copy",
            "create_dir",
            "create_dir_all",
            "exists",
            "hard_link",
            "metadata",
            "read",
            "read_dir",
            "read_link",
            "read_to_string",
            "remove_dir",
            "remove_dir_all",
            "remove_file",
            "rename",
            "set_permissions",
            "symlink_metadata",
            "write",
        ],
        &[
            "DirBuilder",
            "DirEntry",
            "File",
            "FileTimes",
            "FileType",
            "Metadata",
            "OpenOptions",
            "Permissions",
            "ReadDir",
        ],
    ),
    (
        "std::net",
        &[],
        &[
            "AddrParseError",
            "Incoming",
            "IpAddr",
            "Ipv4Addr",
            "Ipv6Addr",
            "Shutdown",
            "SocketAddr",
            "SocketAddrV4",
            "SocketAddrV6",
            "TcpListener",
            "TcpStream",
            "UdpSocket",
        ],
    ),
    (
        "std::process",
        &["abort", "exit", "id"],
        &[
            "Child",
            "ChildStderr",
            "ChildStdin",
            "ChildStdout",
            "Command",
            "CommandArgs",
            "CommandEnvs",
            "ExitCode",
            "ExitStatus",
            "Output",
            "Stdio",
        ],
    ),
    (
        "std::env",
        &[
            "args",
            "args_os",
            "current_dir",
            "current_exe",
            "home_dir",
            "join_paths",
            "remove_var",
            "set_current_dir",
            "set_var",
            "split_paths",
            "temp_dir",
            "var",
            "var_os",
            "vars",
            "vars_os",
        ],
        &["Args", "ArgsOs", "JoinPathsError", "SplitPaths", "VarError", "Vars", "VarsOs"],
    ),
];

/// A module's functions and types, as full paths.
fn expand(module: &str) -> Result<(Vec<String>, Vec<String>), String> {
    let (_, fns, types) = STD_MODULES
        .iter()
        .find(|(m, _, _)| *m == module)
        .ok_or_else(|| format!("boundaries.toml: no expansion for `{module}::*`"))?;
    Ok((
        fns.iter().map(|f| format!("{module}::{f}")).collect(),
        types.iter().map(|t| format!("{module}::{t}")).collect(),
    ))
}

/// Each shipped crate's `clippy.toml` that the rules call for: `no_io` for `no_io_crates`, and
/// `std::net` for every other shipped crate not marked `net` [DR-70, DR-79].
pub fn clippy_tomls(root: &Path) -> Result<Vec<Generated>, String> {
    let rules = load(root)?;
    let mut files = Vec::new();
    for (key, rule) in &rules.rust.crates {
        if !rule.shipped() {
            continue;
        }
        let (mut methods, mut types, mut macros) = (Vec::new(), Vec::new(), Vec::new());
        let reason;
        if rules.rust.kernel.no_io_crates.contains(key) {
            reason = format!(
                "{key} does no I/O in any target: pass values in, and keep I/O in the crates that compose it (boundaries.toml no_io)"
            );
            let no_io = &rules.rust.kernel.no_io;
            for t in &no_io.types {
                match t.strip_suffix("::*") {
                    Some(module) => {
                        let (f, ty) = expand(module)?;
                        methods.extend(f);
                        types.extend(ty);
                    }
                    None => types.push(t.clone()),
                }
            }
            methods.extend(no_io.methods.iter().cloned());
            macros.extend(no_io.macros.iter().cloned());
        } else if !rule.net {
            reason = format!(
                "of the shipped crates, only models and planroom open sockets; {key} must not (boundaries.toml net)"
            );
            let (f, ty) = expand("std::net")?;
            methods.extend(f);
            types.extend(ty);
        } else {
            continue;
        }
        let list = |items: &[String]| -> String {
            if items.is_empty() {
                return "[]".into();
            }
            let body: Vec<String> =
                items.iter().map(|p| format!("  {{ path = \"{p}\", reason = \"{reason}\" }},")).collect();
            format!("[\n{}\n]", body.join("\n"))
        };
        let contents = format!(
            "# {GENERATED_HEADER}\n# Source: boundaries.toml ([rust.crates] and [rust.kernel]).\n\ndisallowed-methods = {}\n\ndisallowed-types = {}\n\ndisallowed-macros = {}\n",
            list(&methods),
            list(&types),
            list(&macros)
        );
        files.push(Generated { path: format!("crates/{key}/clippy.toml"), contents });
    }
    Ok(files)
}

/// A glob over repo paths as a dependency-cruiser regex: `**/` any directories, `*` within one.
fn glob_regex(glob: &str) -> String {
    let mut out = String::from("^");
    let mut rest = glob;
    while !rest.is_empty() {
        if let Some(r) = rest.strip_prefix("**/") {
            out.push_str("(|.*/)");
            rest = r;
        } else if let Some(r) = rest.strip_prefix("**") {
            out.push_str(".*");
            rest = r;
        } else if let Some(r) = rest.strip_prefix('*') {
            out.push_str("[^/]*");
            rest = r;
        } else {
            let c = rest.chars().next().expect("not empty");
            out.push_str(&regex_escape(&c.to_string()));
            rest = &rest[c.len_utf8()..];
        }
    }
    out.push('$');
    out
}

/// A third-party package's resolved path under `node_modules/`, its `@types/` package included,
/// never a dependency type, so a workspace package, which resolves to `packages/`, can't pass.
fn package_path(name: &str) -> String {
    let name = match name.strip_suffix("/*") {
        Some(scope) => format!("{}/[^/]+", regex_escape(scope)),
        None => regex_escape(name),
    };
    format!("(^|/)node_modules/(@types/)?{name}/")
}

/// `.dependency-cruiser.cjs`, generated from `[ts.packages]` and `[ts.rules]`: an allow-list with
/// default deny, one allow rule per listed package.
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
    let from_pkg = |name: &str| -> Result<Value, String> {
        Ok(json!({"path": format!("^{}/", regex_escape(&dir_of(name)?))}))
    };

    let host: Vec<&String> = rules.ts.packages.values().flat_map(|r| &r.host_types).collect();
    let host_regex = format!("^({})$", host.iter().map(|h| regex_escape(h)).collect::<Vec<_>>().join("|"));
    let host_dirs: Vec<String> = rules
        .ts
        .packages
        .iter()
        .filter(|(_, r)| !r.host_types.is_empty())
        .map(|(n, _)| dir_of(n).map(|d| format!("^{}/", regex_escape(&d))))
        .collect::<Result<_, _>>()?;
    let golden_from: Vec<String> = rules.ts.rules.golden_readers.iter().map(|g| glob_regex(g)).collect();
    const GOLDEN: &str = "^tests/golden/.*\\.json$";

    let mut forbidden = Vec::new();
    if rules.ts.rules.not_to_unresolvable {
        // The host's module resolves only inside Claude Code, so it never resolves here.
        forbidden.push(json!({"name": "not-to-unresolvable", "severity": "error", "from": {}, "to": {"couldNotResolve": true, "pathNot": host_regex}}));
    }
    forbidden.push(json!({
        "name": "host-module-only-in-mod",
        "comment": "only the packages with host_types import the mod API's types, which the bundle keeps external [DR-81]",
        "severity": "error",
        "from": {"pathNot": host_dirs},
        "to": {"path": host_regex},
    }));
    forbidden.push(json!({
        "name": "golden-only-in-mod-tests",
        "comment": "only golden_readers read the golden CLI outputs [DR-81]",
        "severity": "error",
        "from": {"pathNot": golden_from},
        "to": {"path": GOLDEN},
    }));
    if rules.ts.rules.no_circular {
        forbidden
            .push(json!({"name": "no-circular", "severity": "error", "from": {}, "to": {"circular": true}}));
    }
    for (name, rule) in &rules.ts.packages {
        let (core, npm): (Vec<&String>, Vec<&String>) =
            rule.forbid.iter().partition(|f| f.as_str() == "node:*");
        if !npm.is_empty() {
            let names: Vec<String> = npm.iter().map(|n| regex_escape(n)).collect();
            forbidden.push(json!({
                "name": format!("{name}-forbidden-packages"),
                "comment": format!("{name} must not import {}", npm.iter().map(|s| s.as_str()).collect::<Vec<_>>().join(", ")),
                "severity": "error",
                "from": from_pkg(name)?,
                "to": {"path": format!("(^|/)node_modules/({})/", names.join("|"))},
            }));
        }
        if !core.is_empty() {
            forbidden.push(json!({
                "name": format!("{name}-no-node"),
                "comment": format!("{name} must not use Node APIs"),
                "severity": "error",
                "from": from_pkg(name)?,
                "to": {"dependencyTypes": ["core"]},
            }));
        }
    }

    let mut allowed = vec![
        json!({"comment": "inside one package", "from": {"path": "^((packages|tests)/[^/]+)/"}, "to": {"path": "^$1/", "pathNot": "(^|/)node_modules/"}}),
        json!({"comment": "Node built-ins (each package's forbid narrows this)", "from": {}, "to": {"dependencyTypes": ["core"]}}),
        json!({"comment": "host_types: the mod API's types", "from": {"path": host_dirs}, "to": {"path": host_regex}}),
        json!({"comment": "golden_readers -> the golden CLI outputs", "from": {"path": golden_from}, "to": {"path": GOLDEN}}),
    ];
    for (name, rule) in &rules.ts.packages {
        if !rule.may_import.is_empty() {
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
                "from": from_pkg(name)?,
                "to": {"path": targets},
            }));
        }
        for package in &rule.third_party {
            allowed.push(json!({
                "comment": format!("{name} -> {package} (third_party)"),
                "from": from_pkg(name)?,
                "to": {"path": package_path(package)},
            }));
        }
    }
    let tooling = &rules.ts.rules.test_tooling;
    let tooling_from: Vec<String> = tooling.files.iter().map(|g| glob_regex(g)).collect();
    for package in &tooling.third_party {
        allowed.push(json!({
            "comment": format!("tests and configs -> {package} (test_tooling)"),
            "from": {"path": tooling_from},
            "to": {"path": package_path(package)},
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
        "// {GENERATED_HEADER}\n// Source: boundaries.toml ([ts.packages] and [ts.rules]).\n/** @type {{import('dependency-cruiser').IConfiguration}} */\nmodule.exports = {body};\n"
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

/// The clippy run on the crates with a generated `clippy.toml`.
fn clippy_args(krate: &str) -> Vec<String> {
    ["clippy", "--locked", "--all-targets", "-p", krate, "--", "-D", "warnings"].map(str::to_owned).to_vec()
}

/// What the self-test did: one line per step, and the steps it could not run yet.
pub struct SelfTest {
    pub lines: Vec<String>,
    /// Plants whose rule has nothing to enforce yet (no packs, no loader), with why. They print a
    /// `not built` line, not a `skipped` step: no check exists to skip, and none passes for them.
    pub not_built: Vec<(String, String)>,
}

/// Plant each crossing in a temporary worktree and expect each check to catch it, after a control
/// run on the unplanted copy. Any plant that isn't caught is an error.
pub fn self_test(root: &Path) -> Result<SelfTest, String> {
    let tmp = std::env::temp_dir().join(format!("planroom-boundaries-self-test-{}", std::process::id()));
    let tmp_str = tmp.to_str().ok_or("temp dir is not UTF-8")?.to_owned();
    util::capture(root, "git", &["worktree", "add", "--detach", "--quiet", &tmp_str, "HEAD"])?;
    let _cleanup = Worktree { root: root.to_path_buf(), path: tmp.clone() };
    overlay_working_tree(root, &tmp)?;
    // Share the checkout's build directory, so the copy's clippy runs reuse its dependencies.
    let target = root.join("target");

    let mut t = Plants { lines: Vec::new(), failures: Vec::new(), not_built: Vec::new() };

    // Control: every check passes on the unplanted copy, so a failure below comes from its plant.
    let baseline = check(&tmp)?;
    if !baseline.is_empty() {
        return Err(format!("the unplanted copy already fails: {}", baseline.join("; ")));
    }
    util::capture(&tmp, "pnpm", &["install", "--offline", "--frozen-lockfile"])?;
    let (ok, out) = util::run(&tmp, "pnpm", DEPCRUISE_ARGS);
    if !ok {
        return Err(format!("depcruise fails on the unplanted copy:\n{out}"));
    }
    let clippy_crates: Vec<String> = clippy_tomls(&tmp)?
        .iter()
        .filter_map(|g| g.path.split('/').nth(1).map(|k| format!("planroom-{k}")))
        .collect();
    for krate in &clippy_crates {
        let (ok, out) = cargo(&tmp, &target, &clippy_args(krate));
        if !ok {
            return Err(format!("clippy fails on the unplanted copy's {krate}:\n{out}"));
        }
    }
    let lock = tmp.join(crate::layout::LOCK);
    let _ = fs::remove_file(&lock);
    crate::layout::write_lock_here(&tmp, &format!("DR-{}", crate::layout::FIRST_RECORD))?;
    let control = crate::layout::state(&tmp)?;
    if !control.errors.is_empty() || !control.changed.is_empty() {
        return Err(format!("the layout state check fails on the unplanted copy: {control:?}"));
    }
    t.lines.push("ok       control: every check passes on the unplanted copy".into());

    // Rust crossings, each planted alone and taken out again.
    let graph = |tmp: &Path| -> Result<Vec<String>, String> { check_crate_graph(tmp, &load(tmp)?) };
    let dep = |manifest: &str, line: &str| -> Result<Restore, String> {
        let r = Restore::save(&tmp.join(manifest))?;
        let text = fs::read_to_string(&r.path).map_err(|e| e.to_string())?;
        let planted = text.replacen("[dependencies]\n", &format!("[dependencies]\n{line}\n"), 1);
        if planted == text {
            return Err(format!("{manifest} has no [dependencies] table to plant into"));
        }
        fs::write(&r.path, planted).map_err(|e| e.to_string())?;
        Ok(r)
    };
    {
        let _a = dep("crates/kernel/Cargo.toml", "planroom-store = { path = \"../store\" }")?;
        let _b = Restore::append(&tmp.join("crates/kernel/src/lib.rs"), "\nuse planroom_store as _;\n")?;
        let found = graph(&tmp)?;
        t.step("kernel depends on store", &found, "kernel -> store is not allowed");
    }
    {
        let _a = dep("crates/extract/Cargo.toml", "planroom-models = { path = \"../models\" }")?;
        let found = graph(&tmp)?;
        t.step("extract depends on models (sideways)", &found, "extract -> models is not allowed");
    }
    {
        let _a = dep("crates/extract/Cargo.toml", "ureq = \"=2.12.1\"")?;
        let found = graph(&tmp)?;
        t.step(
            "extract depends on ureq (a network client)",
            &found,
            "extract -> ureq is not in extract's external_allow",
        );
    }
    {
        let _a = dep("crates/store/Cargo.toml", "planroom-testkit = { path = \"../testkit\" }")?;
        let found = graph(&tmp)?;
        t.step(
            "store depends on testkit as a normal dependency",
            &found,
            "store -> testkit is not allowed (testkit is dev_only",
        );
    }

    // Literal scans.
    let rules = load(&tmp)?;
    {
        let _a = Restore::append(
            &tmp.join("crates/kernel/src/lib.rs"),
            "\npub const PLANTED: &str = \"code.service\";\n",
        )?;
        let found = check_rust_literals(&tmp, &rules)?;
        t.step("\"code.service\" in the kernel", &found, "names the pack kind `code.service`");
    }
    {
        let _a = Restore::append(
            &tmp.join("crates/contract/src/lib.rs"),
            "\npub const SERVICE: &str = \"code.service\";\n",
        )?;
        let _b = Restore::append(
            &tmp.join("crates/kernel/src/lib.rs"),
            "\npub use planroom_contract::SERVICE;\n",
        )?;
        let found = check_rust_literals(&tmp, &rules)?;
        t.step(
            "\"code.service\" in a contract constant the kernel uses",
            &found,
            "crates/contract/src/lib.rs",
        );
    }
    {
        let _a = Restore::create(
            &tmp.join("packages/ui/src/planted.ts"),
            "export const KIND = \"code.service\";\n",
        )?;
        let found = check_ts_literals(&tmp, &rules)?;
        t.step("\"code.service\" in packages/ui", &found, "packages/ui/src/planted.ts");
    }
    {
        let _a = Restore::append(
            &tmp.join("crates/kernel/src/lib.rs"),
            "\npub const PLANTED: &str = \"depends_on_package\";\n",
        )?;
        let found = check_rust_literals(&tmp, &rules)?;
        t.step(
            "the bare relation \"depends_on_package\" in the kernel",
            &found,
            "bare name of `code.depends_on_package`",
        );
    }
    {
        let _a = Restore::append(
            &tmp.join("crates/kernel/src/lib.rs"),
            "\npub const PLANTED: &str = \"person\";\n",
        )?;
        let found = check_rust_literals(&tmp, &rules)?;
        t.absent(
            "a bare \"person\", which kernel_vocabulary lists, in the kernel",
            &found,
            "crates/kernel/src/lib.rs",
        );
    }

    // Clippy plants: the no-I/O lint in the kernel and contract, and std::net in store.
    let clippy = |krate: &str| -> Vec<String> {
        let (_, out) = cargo(&tmp, &target, &clippy_args(krate));
        out.lines().map(str::to_owned).collect()
    };
    {
        let _a = Restore::append(
            &tmp.join("crates/kernel/src/lib.rs"),
            "\npub fn planted_read() -> String {\n    std::fs::read_to_string(\"x\").unwrap_or_default()\n}\n",
        )?;
        t.step(
            "std::fs::read_to_string in the kernel",
            &clippy("planroom-kernel"),
            "use of a disallowed method `std::fs::read_to_string`",
        );
    }
    {
        let _a = Restore::append(
            &tmp.join("crates/kernel/src/lib.rs"),
            "\npub fn planted_exists() -> bool {\n    std::path::Path::new(\"x\").exists()\n}\n",
        )?;
        t.step(
            "Path::exists in the kernel",
            &clippy("planroom-kernel"),
            "use of a disallowed method `std::path::Path::exists`",
        );
    }
    {
        let _a = Restore::append(
            &tmp.join("crates/contract/src/lib.rs"),
            "\npub fn read(p: &str) -> String {\n    std::fs::read_to_string(p).unwrap_or_default()\n}\n",
        )?;
        t.step(
            "a std::fs wrapper in the contract",
            &clippy("planroom-contract"),
            "use of a disallowed method `std::fs::read_to_string`",
        );
    }
    {
        let _a = Restore::append(
            &tmp.join("crates/store/src/lib.rs"),
            "\npub fn planted_connect() -> bool {\n    std::net::TcpStream::connect(\"127.0.0.1:9\").is_ok()\n}\n",
        )?;
        t.step(
            "std::net::TcpStream in store",
            &clippy("planroom-store"),
            "use of a disallowed type `std::net::TcpStream`",
        );
    }

    // TypeScript crossings.
    let depcruise = |tmp: &Path| -> Vec<String> {
        let (_, out) = util::run(tmp, "pnpm", DEPCRUISE_ARGS);
        out.lines().map(str::to_owned).collect()
    };
    let not_unresolvable = |out: &[String]| !out.iter().any(|l| l.contains("not-to-unresolvable"));
    {
        let pkg_path = tmp.join("packages/canvas/package.json");
        let _a = Restore::save(&pkg_path)?;
        let mut pkg: Value = serde_json::from_str(&fs::read_to_string(&pkg_path).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        pkg["dependencies"]["@planroom/mod"] = json!("workspace:*");
        fs::write(&pkg_path, serde_json::to_string_pretty(&pkg).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
        // Link it the way pnpm links a workspace dependency. A second `pnpm install --offline` would
        // re-resolve, which needs registry metadata a fresh CI runner has never cached.
        let link = link_package(&tmp, "packages/canvas", "@planroom/mod", &tmp.join("packages/mod"))?;
        {
            let _b =
                Restore::create(&tmp.join("packages/canvas/src/planted.ts"), "import \"@planroom/mod\";\n")?;
            let out = depcruise(&tmp);
            t.step_if(
                "canvas imports @planroom/mod, declared",
                &out,
                "not-in-allowed",
                not_unresolvable(&out),
            );
        }
        let _ = fs::remove_file(link);
    }
    {
        let _a = Restore::create(
            &tmp.join("packages/canvas/src/planted.ts"),
            "import \"../../mod/src/index.ts\";\n",
        )?;
        let out = depcruise(&tmp);
        t.step_if(
            "canvas imports the mod by a relative path",
            &out,
            "not-in-allowed",
            not_unresolvable(&out),
        );
    }
    {
        // zustand, which the canvas installs, declared by ui and linked as pnpm would.
        let resolved = fs::canonicalize(tmp.join("packages/canvas/node_modules/zustand"))
            .map_err(|e| format!("the canvas's zustand: {e}"))?;
        let link = link_package(&tmp, "packages/ui", "zustand", &resolved)?;
        {
            let _b = Restore::create(&tmp.join("packages/ui/src/planted.ts"), "import \"zustand\";\n")?;
            let out = depcruise(&tmp);
            t.step_if(
                "ui imports zustand, a third-party package it doesn't list",
                &out,
                "not-in-allowed",
                not_unresolvable(&out),
            );
        }
        let _ = fs::remove_file(link);
    }
    {
        let _a = Restore::create(&tmp.join("packages/canvas/src/planted.ts"), "import \"claude-code\";\n")?;
        t.step("canvas imports claude-code", &depcruise(&tmp), "host-module-only-in-mod");
    }
    {
        let _a = Restore::create(
            &tmp.join("packages/canvas/src/planted.ts"),
            "import focus from \"../../../tests/golden/cli/focus.json\";\nexport const F = focus;\n",
        )?;
        t.step("canvas reads a golden CLI output", &depcruise(&tmp), "golden-only-in-mod-tests");
    }

    // Pack plants: requires point inward by origin, and a pack holds no executable.
    let manifest = |id: &str, requires: &str| {
        format!(
            "format = \"planroom.pack/v1\"\nid = \"{id}\"\nversion = 1\ntitle = \"{id}\"\nrequires = {requires}\n"
        )
    };
    let make_dir = |dir: &str| fs::create_dir_all(tmp.join(dir)).map_err(|e| e.to_string());
    {
        make_dir("packs/planted")?;
        let _a = Restore::create(
            &tmp.join("packs/planted/pack.toml"),
            &manifest("planted", "[\"observability\"]"),
        )?;
        let found = check_packs(&tmp, &rules)?;
        t.step("a built-in pack requires observability", &found, "packs/planted/pack.toml");
    }
    {
        make_dir("examples/packs/planted_a")?;
        make_dir("examples/packs/planted_b")?;
        let _a = Restore::create(
            &tmp.join("examples/packs/planted_a/pack.toml"),
            &manifest("planted_a", "[\"code\"]"),
        )?;
        let _b = Restore::create(
            &tmp.join("examples/packs/planted_b/pack.toml"),
            &manifest("planted_b", "[\"planted_a\"]"),
        )?;
        let found = check_packs(&tmp, &rules)?;
        t.step(
            "an out-of-tree pack requires another out-of-tree pack",
            &found,
            "examples/packs/planted_b/pack.toml",
        );
    }
    {
        use std::os::unix::fs::PermissionsExt;
        let script = tmp.join("packs/code/planted.sh");
        let _a = Restore::create(&script, "#!/bin/sh\n")?;
        fs::set_permissions(&script, fs::Permissions::from_mode(0o755)).map_err(|e| e.to_string())?;
        let found = check_packs(&tmp, &rules)?;
        t.step("an executable file in a pack", &found, "packs/code/planted.sh");
    }

    // The layout gate: a changed boundaries.toml with no new record fails outside a worktree.
    {
        let _a = Restore::append(&tmp.join("boundaries.toml"), "\n# planted\n")?;
        let st = crate::layout::state(&tmp)?;
        t.step("boundaries.toml changed with no new record", &st.changed, "boundaries.toml");
    }

    if t.failures.is_empty() {
        Ok(SelfTest { lines: t.lines, not_built: t.not_built })
    } else {
        Err(format!("{}\nnot caught: {}", t.lines.join("\n"), t.failures.join(", ")))
    }
}

struct Plants {
    lines: Vec<String>,
    failures: Vec<String>,
    not_built: Vec<(String, String)>,
}

impl Plants {
    /// The plant was caught when some line of the check's output contains `expect`.
    fn step(&mut self, what: &str, found: &[String], expect: &str) {
        self.step_if(what, found, expect, true);
    }

    /// The plant is allowed: no line of the check's output contains `needle`.
    fn absent(&mut self, what: &str, found: &[String], needle: &str) {
        if found.iter().any(|l| l.contains(needle)) {
            self.lines.push(format!("FAILED   {what}: flagged, but it is allowed"));
            self.failures.push(what.to_owned());
        } else {
            self.lines.push(format!("ok       {what}: allowed"));
        }
    }

    fn step_if(&mut self, what: &str, found: &[String], expect: &str, also: bool) {
        if also && found.iter().any(|l| l.contains(expect)) {
            self.lines.push(format!("ok       {what}: caught ({expect})"));
        } else {
            self.lines.push(format!("FAILED   {what}: not caught by `{expect}`"));
            self.failures.push(what.to_owned());
            let tail: Vec<&String> = found.iter().rev().take(20).collect();
            for l in tail.into_iter().rev() {
                self.lines.push(format!("           | {l}"));
            }
        }
    }
}

fn cargo(dir: &Path, target: &Path, args: &[String]) -> (bool, String) {
    let out = std::process::Command::new("cargo")
        .args(args)
        .current_dir(dir)
        .env("CARGO_TARGET_DIR", target)
        .stdin(std::process::Stdio::null())
        .output();
    match out {
        Ok(out) => {
            let mut text = String::from_utf8_lossy(&out.stdout).into_owned();
            text.push_str(&String::from_utf8_lossy(&out.stderr));
            (out.status.success(), text)
        }
        Err(e) => (false, format!("could not run cargo: {e}")),
    }
}

/// A file's contents saved on creation and put back on drop; a file it created is removed.
struct Restore {
    path: PathBuf,
    saved: Option<Vec<u8>>,
}

impl Restore {
    fn save(path: &Path) -> Result<Self, String> {
        let saved = fs::read(path).ok();
        Ok(Self { path: path.to_path_buf(), saved })
    }

    fn append(path: &Path, text: &str) -> Result<Self, String> {
        let r = Self::save(path)?;
        let mut s = fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        s.push_str(text);
        fs::write(path, s).map_err(|e| format!("{}: {e}", path.display()))?;
        Ok(r)
    }

    fn create(path: &Path, text: &str) -> Result<Self, String> {
        let r = Self::save(path)?;
        fs::write(path, text).map_err(|e| format!("{}: {e}", path.display()))?;
        Ok(r)
    }
}

impl Drop for Restore {
    fn drop(&mut self) {
        let _ = match &self.saved {
            Some(bytes) => fs::write(&self.path, bytes),
            None => fs::remove_file(&self.path),
        };
    }
}

/// Symlinks `<pkg_dir>/node_modules/<name>` to `target`, as pnpm links a declared dependency.
fn link_package(root: &Path, pkg_dir: &str, name: &str, target: &Path) -> Result<PathBuf, String> {
    let link = root.join(pkg_dir).join("node_modules").join(name);
    let parent = link.parent().ok_or("link has no parent")?;
    fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let _ = fs::remove_file(&link);
    std::os::unix::fs::symlink(target, &link).map_err(|e| format!("{}: {e}", link.display()))?;
    Ok(link)
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

    #[test]
    fn allow_lists_match_names_and_prefix_globs() {
        let list = vec!["tree-sitter".to_owned(), "tree-sitter-*".to_owned(), "serde".to_owned()];
        assert!(allowed(&list, "tree-sitter"));
        assert!(allowed(&list, "tree-sitter-rust"));
        assert!(allowed(&list, "serde"));
        assert!(!allowed(&list, "serde_json"));
        assert!(!allowed(&list, "ureq"));
    }

    #[test]
    fn globs_become_anchored_regexes() {
        assert_eq!(glob_regex("packages/mod/src/**/*.test.ts"), "^packages/mod/src/(|.*/)[^/]*\\.test\\.ts$");
        assert_eq!(glob_regex("packages/mod/plugin-tests/**"), "^packages/mod/plugin-tests/.*$");
        assert_eq!(package_path("@fontsource/*"), "(^|/)node_modules/(@types/)?@fontsource/[^/]+/");
        assert_eq!(package_path("react"), "(^|/)node_modules/(@types/)?react/");
    }

    #[test]
    fn ts_literals_skip_comments() {
        let src = "// \"code.a\"\n/** `code.b` */\nconst x = \"code.c\"; const y = 'it\\'s'; const z = `t ${code.toString({a: 1})} and `;\n";
        let found: Vec<String> = ts_string_literals(src).into_iter().map(|(_, v)| v).collect();
        assert_eq!(found, ["code.c", "it\\'s", "t   and "]);
    }

    #[test]
    fn socket_names_in_code_count_and_in_comments_do_not() {
        let dir = std::env::temp_dir().join(format!("planroom-socket-scan-{}", std::process::id()));
        let src = dir.join("src");
        fs::create_dir_all(&src).unwrap();
        fs::write(src.join("lib.rs"), "//! Like a TcpStream.\nconst S: &str = \"UdpSocket\";\n").unwrap();
        assert!(!names_a_socket(&dir).unwrap());
        fs::write(src.join("net.rs"), "use std::net::TcpStream;\n").unwrap();
        assert!(names_a_socket(&dir).unwrap());
        fs::remove_dir_all(&dir).unwrap();
    }
}
