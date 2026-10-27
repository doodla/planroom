//! The decision-record gate (repo-layout.md, Decision-record gate; DR-59, DR-70, DR-79, DR-81).
//!
//! `layout.lock` holds each gated surface's sha256 and the record that approved it. The state check
//! recomputes the hashes on any checkout; the walk checks, per commit, that a commit changing the
//! lock adds the record it names. In a linked worktree, where a branch is built, a changed surface is
//! `pending ratification`, never a failure, and `gen --layout-lock` refuses.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::path::Path;

use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::generate::GENERATED_HEADER;
use crate::util;

pub const LOCK: &str = "layout.lock";
pub const DECISIONS: &str = "docs/contributing/decisions.md";
pub const REPO_LAYOUT: &str = "docs/contributing/repo-layout.md";
/// The record the first lock names for every surface; its commit needs no record of its own (DR-81).
pub const FIRST_RECORD: u32 = 79;

/// `79` from `DR-79`.
pub fn record_number(record: &str) -> Option<u32> {
    record.strip_prefix("DR-")?.parse().ok()
}

/// Every `## DR-n` heading in `text`, in order, with its whole heading line.
pub fn headings_in(text: &str) -> Vec<(u32, String)> {
    text.lines()
        .filter_map(|line| {
            let rest = line.strip_prefix("## DR-")?;
            let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
            let after = &rest[digits.len()..];
            if digits.is_empty() || !(after.is_empty() || after.starts_with(':') || after.starts_with(' ')) {
                return None;
            }
            Some((digits.parse().ok()?, line.trim_start_matches("## ").to_owned()))
        })
        .collect()
}

/// The `## DR-n` headings of the working tree's `decisions.md`.
pub fn record_headings(root: &Path) -> Result<Vec<(u32, String)>, String> {
    let path = root.join(DECISIONS);
    let text = fs::read_to_string(&path).map_err(|e| format!("{DECISIONS}: {e}"))?;
    Ok(headings_in(&text))
}

/// Each heading number that appears more than once.
fn duplicates(headings: &[(u32, String)]) -> Vec<u32> {
    let mut seen = BTreeSet::new();
    let mut dup = BTreeSet::new();
    for (n, _) in headings {
        if !seen.insert(*n) {
            dup.insert(*n);
        }
    }
    dup.into_iter().collect()
}

/// Where the gated files are read from: the working tree, or one commit's tree.
pub enum Source<'a> {
    Work(&'a Path),
    Commit(&'a Path, String),
}

impl Source<'_> {
    fn read(&self, path: &str) -> Option<String> {
        match self {
            Source::Work(root) => fs::read_to_string(root.join(path)).ok(),
            Source::Commit(root, sha) => util::capture(root, "git", &["show", &format!("{sha}:{path}")]).ok(),
        }
    }

    /// Every file path: for the working tree, the tracked and the untracked files git doesn't
    /// ignore, less those deleted from disk.
    fn files(&self) -> Result<Vec<String>, String> {
        let listed = match self {
            Source::Work(root) => {
                let out = util::capture(
                    root,
                    "git",
                    &["ls-files", "-z", "--cached", "--others", "--exclude-standard"],
                )?;
                out.split('\0')
                    .filter(|p| !p.is_empty() && root.join(p).exists())
                    .map(str::to_owned)
                    .collect::<Vec<_>>()
            }
            Source::Commit(root, sha) => {
                let out = util::capture(root, "git", &["ls-tree", "-r", "-z", "--name-only", sha])?;
                out.split('\0').filter(|p| !p.is_empty()).map(str::to_owned).collect()
            }
        };
        let mut files = listed;
        files.sort();
        files.dedup();
        Ok(files)
    }
}

fn sha256_hex(text: &str) -> String {
    Sha256::digest(text.as_bytes()).iter().map(|b| format!("{b:02x}")).collect()
}

/// Every gated surface's sha256, by name, in `src`. A surface whose file is missing is left out,
/// so its removal shows as a change.
pub fn surfaces(src: &Source) -> Result<BTreeMap<String, String>, String> {
    let files = src.files()?;
    let mut out = BTreeMap::new();
    if let Some(text) = src.read("boundaries.toml") {
        out.insert("boundaries.toml".to_owned(), sha256_hex(&text));
    }
    out.insert("crate dependencies".to_owned(), sha256_hex(&crate_dependencies(src, &files)?));
    out.insert("top-level directories".to_owned(), sha256_hex(&top_level_directories(&files)));
    for manifest in package_manifests(&files) {
        let Some(text) = src.read(&manifest) else { continue };
        let dir = manifest.strip_suffix("package.json").unwrap_or(&manifest).trim_end_matches('/');
        let dir = if dir.is_empty() { "." } else { dir };
        out.insert(format!("package {dir}"), sha256_hex(&package_surface(&manifest, &text)?));
    }
    if let Some(text) = src.read(REPO_LAYOUT) {
        let rows = crate::areas::table_rows(&text, "## Area contracts");
        let canonical = serde_json::to_string(&rows).map_err(|e| e.to_string())?;
        out.insert("area contracts table".to_owned(), sha256_hex(&canonical));
    }
    if let Some(text) = src.read("AGENTS.md") {
        for section in ["Boundaries", "Conventions"] {
            if let Some(body) = section_of(&text, section) {
                out.insert(format!("AGENTS.md {section}"), sha256_hex(&body));
            }
        }
    }
    Ok(out)
}

/// The `## {name}` section of a Markdown file, up to the next `## ` heading, with trailing
/// whitespace and surrounding blank lines dropped.
pub fn section_of(text: &str, name: &str) -> Option<String> {
    let heading = format!("## {name}");
    let mut lines = text.lines().skip_while(|l| l.trim_end() != heading);
    lines.next()?;
    let body: Vec<&str> = lines.take_while(|l| !l.starts_with("## ")).map(str::trim_end).collect();
    Some(body.join("\n").trim().to_owned())
}

/// `[dependencies]` (with dev and build dependencies) between workspace crates, one line each.
fn crate_dependencies(src: &Source, files: &[String]) -> Result<String, String> {
    let manifests: Vec<&String> = files
        .iter()
        .filter(|f| f.starts_with("crates/") && f.ends_with("/Cargo.toml") && f.matches('/').count() == 2)
        .collect();
    let mut crates = Vec::new();
    for manifest in &manifests {
        let Some(text) = src.read(manifest) else { continue };
        let doc: toml::Table = toml::from_str(&text).map_err(|e| format!("{manifest}: {e}"))?;
        let name = doc
            .get("package")
            .and_then(|p| p.get("name"))
            .and_then(|n| n.as_str())
            .ok_or_else(|| format!("{manifest}: no [package] name"))?
            .to_owned();
        crates.push((name, doc));
    }
    let names: BTreeSet<String> = crates.iter().map(|(n, _)| n.clone()).collect();
    let mut lines = BTreeSet::new();
    for (name, doc) in &crates {
        let mut tables: Vec<(&str, &toml::Table)> = Vec::new();
        for kind in ["dependencies", "dev-dependencies", "build-dependencies"] {
            if let Some(t) = doc.get(kind).and_then(|t| t.as_table()) {
                tables.push((kind, t));
            }
        }
        if let Some(targets) = doc.get("target").and_then(|t| t.as_table()) {
            for target in targets.values().filter_map(|t| t.as_table()) {
                for kind in ["dependencies", "dev-dependencies", "build-dependencies"] {
                    if let Some(t) = target.get(kind).and_then(|t| t.as_table()) {
                        tables.push((kind, t));
                    }
                }
            }
        }
        for (kind, table) in tables {
            for (key, value) in table {
                let dep = value.get("package").and_then(|p| p.as_str()).unwrap_or(key);
                if names.contains(dep) {
                    lines.insert(format!("{name} -> {dep} ({kind})"));
                }
            }
        }
    }
    Ok(lines.into_iter().collect::<Vec<_>>().join("\n"))
}

/// The top-level directories, one per line.
fn top_level_directories(files: &[String]) -> String {
    let dirs: BTreeSet<&str> = files.iter().filter_map(|f| f.split_once('/').map(|(d, _)| d)).collect();
    dirs.into_iter().collect::<Vec<_>>().join("\n")
}

/// The workspace's `package.json` files: the root's, and each under `packages/*` and `tests/*`.
fn package_manifests(files: &[String]) -> Vec<String> {
    files
        .iter()
        .filter(|f| {
            f.as_str() == "package.json"
                || ((f.starts_with("packages/") || f.starts_with("tests/"))
                    && f.ends_with("/package.json")
                    && f.matches('/').count() == 2)
        })
        .cloned()
        .collect()
}

/// A package's `exports` and its dependency names by section, never their versions.
fn package_surface(path: &str, text: &str) -> Result<String, String> {
    let pkg: Value = serde_json::from_str(text).map_err(|e| format!("{path}: {e}"))?;
    let mut surface = serde_json::Map::new();
    surface.insert("exports".into(), pkg.get("exports").cloned().unwrap_or(Value::Null));
    for section in ["dependencies", "devDependencies", "peerDependencies", "optionalDependencies"] {
        let names: BTreeSet<String> = pkg
            .get(section)
            .and_then(Value::as_object)
            .map(|o| o.keys().cloned().collect())
            .unwrap_or_default();
        if !names.is_empty() {
            surface.insert(section.into(), Value::from(names.into_iter().collect::<Vec<_>>()));
        }
    }
    serde_json::to_string(&Value::Object(surface)).map_err(|e| e.to_string())
}

/// One lock entry: the surface's sha256 and the record that approved it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Entry {
    pub sha256: String,
    pub record: u32,
}

pub type Lock = BTreeMap<String, Entry>;

pub fn parse_lock(text: &str) -> Result<Lock, String> {
    let table: toml::Table = toml::from_str(text).map_err(|e| format!("{LOCK}: {e}"))?;
    let mut lock = Lock::new();
    for (name, value) in table {
        let sha256 = value.get("sha256").and_then(|v| v.as_str());
        let record = value.get("record").and_then(|v| v.as_str()).and_then(record_number);
        match (sha256, record) {
            (Some(sha256), Some(record)) => {
                lock.insert(name, Entry { sha256: sha256.to_owned(), record });
            }
            _ => return Err(format!("{LOCK}: `{name}` needs sha256 and record = \"DR-n\"")),
        }
    }
    Ok(lock)
}

pub fn render_lock(lock: &Lock) -> String {
    let mut out = format!(
        "# {GENERATED_HEADER}\n# Written by `cargo xtask gen --layout-lock DR-n` in the commit that adds the record (when a\n# branch brought the change, its merge commit): each gated surface's sha256 and the decision\n# record that approved it (repo-layout.md, Decision-record gate).\n"
    );
    for (name, entry) in lock {
        let key = serde_json::to_string(name).unwrap_or_default();
        out.push_str(&format!(
            "\n[{key}]\nsha256 = \"{}\"\nrecord = \"DR-{}\"\n",
            entry.sha256, entry.record
        ));
    }
    out
}

fn read_lock(src: &Source) -> Result<Option<Lock>, String> {
    src.read(LOCK).map(|t| parse_lock(&t)).transpose()
}

/// Whether `root` is a linked worktree: its git directory differs from the common one.
pub fn linked_worktree(root: &Path) -> Result<bool, String> {
    let out = util::capture(
        root,
        "git",
        &["rev-parse", "--path-format=absolute", "--git-dir", "--git-common-dir"],
    )?;
    let dirs: Vec<&str> = out.lines().collect();
    match dirs.as_slice() {
        [git_dir, common] => {
            let canon = |p: &str| fs::canonicalize(p).unwrap_or_else(|_| Path::new(p).to_path_buf());
            Ok(canon(git_dir) != canon(common))
        }
        _ => Err(format!("git rev-parse printed {out:?}")),
    }
}

/// What the state check found.
#[derive(Debug, Default)]
pub struct State {
    /// Failures whatever the checkout: a duplicate heading, a missing record, an unreadable lock.
    pub errors: Vec<String>,
    /// Gated surfaces that differ from the lock, or every surface when there is no lock.
    pub changed: Vec<String>,
    pub has_lock: bool,
}

/// The state check: the lock matches the surfaces, and every record it names appears once.
pub fn state(root: &Path) -> Result<State, String> {
    let src = Source::Work(root);
    let mut st = State::default();
    let headings = record_headings(root)?;
    for n in duplicates(&headings) {
        st.errors.push(format!("`## DR-{n}` appears more than once in {DECISIONS}"));
    }
    let lock = match read_lock(&src) {
        Ok(lock) => lock,
        Err(e) => {
            st.errors.push(e);
            return Ok(st);
        }
    };
    let now = surfaces(&src)?;
    match lock {
        None => st.changed = now.keys().cloned().collect(),
        Some(lock) => {
            st.has_lock = true;
            let named: BTreeSet<u32> = lock.values().map(|e| e.record).collect();
            for n in named {
                if !headings.iter().any(|(h, _)| *h == n) {
                    st.errors.push(format!("{LOCK} names DR-{n}, which has no heading in {DECISIONS}"));
                }
            }
            st.changed = changed(&lock_digests(&lock), &now);
        }
    }
    Ok(st)
}

fn lock_digests(lock: &Lock) -> BTreeMap<String, String> {
    lock.iter().map(|(k, e)| (k.clone(), e.sha256.clone())).collect()
}

/// Surface names whose digest differs between `a` and `b`, or that only one holds.
fn changed(a: &BTreeMap<String, String>, b: &BTreeMap<String, String>) -> Vec<String> {
    let names: BTreeSet<&String> = a.keys().chain(b.keys()).collect();
    names.into_iter().filter(|n| a.get(*n) != b.get(*n)).cloned().collect()
}

/// `cargo xtask gen --layout-lock DR-n`: refuses in a linked worktree, unless `## DR-n` exists
/// once and is newer than every record the lock names, and, for the first lock, unless it names
/// DR-79. Returns the surfaces the new lock ratifies with DR-n.
pub fn write_lock(root: &Path, record: &str) -> Result<Vec<String>, String> {
    if linked_worktree(root)? {
        return Err(
            "gen --layout-lock refuses in a linked worktree: a branch writes no lock and no record; its merge commit records its gated changes"
                .into(),
        );
    }
    write_lock_here(root, record)
}

/// `write_lock` without the worktree rule, for the self-test's temporary copy.
pub fn write_lock_here(root: &Path, record: &str) -> Result<Vec<String>, String> {
    let n = record_number(record).ok_or_else(|| format!("`{record}` is not DR-n"))?;
    let headings = record_headings(root)?;
    if let Some(d) = duplicates(&headings).first() {
        return Err(format!("`## DR-{d}` appears more than once in {DECISIONS}"));
    }
    if !headings.iter().any(|(h, _)| *h == n) {
        return Err(format!("{DECISIONS} has no `## DR-{n}` heading; write the record first"));
    }
    let src = Source::Work(root);
    let existing = read_lock(&src)?;
    let now = surfaces(&src)?;
    let (lock, ratified) = match existing {
        None => {
            if n != FIRST_RECORD {
                return Err(format!(
                    "there is no {LOCK} yet, and the first lock names DR-{FIRST_RECORD} for every surface (DR-81)"
                ));
            }
            let lock: Lock =
                now.iter().map(|(k, v)| (k.clone(), Entry { sha256: v.clone(), record: n })).collect();
            (lock, now.keys().cloned().collect())
        }
        Some(old) => {
            let newest = old.values().map(|e| e.record).max().unwrap_or(0);
            if n <= newest {
                return Err(format!(
                    "DR-{n} isn't newer than DR-{newest}, which {LOCK} already names; ratify with a new record"
                ));
            }
            let mut ratified = Vec::new();
            let lock: Lock = now
                .iter()
                .map(|(k, v)| match old.get(k) {
                    Some(e) if &e.sha256 == v => (k.clone(), e.clone()),
                    _ => {
                        ratified.push(k.clone());
                        (k.clone(), Entry { sha256: v.clone(), record: n })
                    }
                })
                .collect();
            ratified.extend(old.keys().filter(|k| !now.contains_key(*k)).map(|k| format!("{k} (removed)")));
            if ratified.is_empty() {
                return Ok(ratified);
            }
            (lock, ratified)
        }
    };
    let path = root.join(LOCK);
    fs::write(&path, render_lock(&lock)).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(ratified)
}

/// What the walk over a range of commits found.
#[derive(Debug, Default)]
pub struct Walk {
    /// One line per commit that changes a gated surface or the lock.
    pub lines: Vec<String>,
    pub failures: Vec<String>,
    pub commits: usize,
}

/// Walk the commits `rev-list` names, oldest first: list each gated change, and require a commit
/// that changes the lock to add the `## DR-n` heading each changed entry names, except the commit
/// that writes the first lock, which names DR-79 throughout. A range that can't be resolved fails.
pub fn walk(root: &Path, rev_list: &[String]) -> Result<Walk, String> {
    let mut args = vec!["rev-list", "--reverse", "--topo-order"];
    args.extend(rev_list.iter().map(String::as_str));
    let out = util::capture(root, "git", &args)
        .map_err(|e| format!("the commit range {} can't be resolved: {e}", rev_list.join(" ")))?;
    let commits: Vec<&str> = out.lines().filter(|l| !l.is_empty()).collect();
    let mut walk = Walk { commits: commits.len(), ..Walk::default() };
    let mut cache: HashMap<String, BTreeMap<String, String>> = HashMap::new();
    let mut surfaces_at = |sha: &str| -> Result<BTreeMap<String, String>, String> {
        if let Some(s) = cache.get(sha) {
            return Ok(s.clone());
        }
        let s = surfaces(&Source::Commit(root, sha.to_owned()))?;
        cache.insert(sha.to_owned(), s.clone());
        Ok(s)
    };
    for sha in commits {
        let short = &sha[..sha.len().min(10)];
        let subject = util::capture(root, "git", &["log", "-1", "--format=%s", sha])?.trim().to_owned();
        let parent = util::capture(root, "git", &["rev-parse", "--verify", "--quiet", &format!("{sha}^")])
            .ok()
            .map(|p| p.trim().to_owned());
        let after = surfaces_at(sha)?;
        let before = match &parent {
            Some(p) => surfaces_at(p)?,
            None => BTreeMap::new(),
        };
        let gated = changed(&before, &after);
        if !gated.is_empty() {
            walk.lines.push(format!("{short} {subject}: gated surfaces changed: {}", gated.join(", ")));
        }
        let budgets_after = Source::Commit(root, sha.to_owned()).read(BUDGETS);
        let budgets_before = parent.as_ref().and_then(|p| Source::Commit(root, p.clone()).read(BUDGETS));
        if budgets_after != budgets_before {
            match budget_raises_between(budgets_before.as_deref(), budgets_after.as_deref()) {
                Ok(raises) if raises.is_empty() => {}
                Ok(raises) => {
                    let added = added_headings(root, sha, parent.as_deref())?;
                    if let Some(record) = added.iter().next_back() {
                        walk.lines.push(format!(
                            "{short} {subject}: DR-{record} records the {BUDGETS} raises: {}",
                            raises.join("; ")
                        ));
                    } else {
                        walk.failures.push(format!(
                            "{short} {subject}: raises gated numbers in {BUDGETS} without adding a record to {DECISIONS}: {}",
                            raises.join("; ")
                        ));
                    }
                }
                Err(e) => walk.failures.push(format!("{short} {subject}: {e}")),
            }
        }
        let lock_after = read_lock(&Source::Commit(root, sha.to_owned()))?;
        let lock_before = match &parent {
            Some(p) => read_lock(&Source::Commit(root, p.clone()))?,
            None => None,
        };
        if lock_after == lock_before {
            continue;
        }
        let Some(lock_after) = lock_after else {
            walk.failures.push(format!("{short} {subject}: removes {LOCK}"));
            continue;
        };
        let Some(lock_before) = lock_before else {
            let others: BTreeSet<u32> =
                lock_after.values().map(|e| e.record).filter(|r| *r != FIRST_RECORD).collect();
            if others.is_empty() {
                walk.lines
                    .push(format!("{short} {subject}: writes the first {LOCK}, naming DR-{FIRST_RECORD}"));
            } else {
                walk.failures.push(format!(
                    "{short} {subject}: the first {LOCK} names {}, not DR-{FIRST_RECORD} for every surface (DR-81)",
                    others.iter().map(|r| format!("DR-{r}")).collect::<Vec<_>>().join(", ")
                ));
            }
            continue;
        };
        let added = added_headings(root, sha, parent.as_deref())?;
        let mut named = BTreeMap::<u32, Vec<String>>::new();
        for (name, entry) in &lock_after {
            if lock_before.get(name) != Some(entry) {
                named.entry(entry.record).or_default().push(name.clone());
            }
        }
        for (record, names) in named {
            if added.contains(&record) {
                walk.lines.push(format!("{short} {subject}: DR-{record} ratifies {}", names.join(", ")));
            } else {
                walk.failures.push(format!(
                    "{short} {subject}: {LOCK} names DR-{record} for {}, but this commit doesn't add `## DR-{record}` to {DECISIONS}",
                    names.join(", ")
                ));
            }
        }
    }
    Ok(walk)
}

/// The hand-ratcheted budgets file at the repo root (contracts.md, budgets.json; DR-59, DR-89).
pub const BUDGETS: &str = "budgets.json";

/// The raises from one `budgets.json` to the next, each `key: what changed`; a file that appears
/// raises nothing, and one removed removes every key.
pub fn budget_raises_between(before: Option<&str>, after: Option<&str>) -> Result<Vec<String>, String> {
    let parse = |text: &str| serde_json::from_str::<Value>(text).map_err(|e| format!("{BUDGETS}: {e}"));
    let Some(before) = before else { return Ok(Vec::new()) };
    let after = match after {
        Some(text) => parse(text)?,
        None => Value::Null,
    };
    Ok(budget_raises(&parse(before)?, &after))
}

/// Each gated number `new` raises over `old`, as `key: what changed`. A ceiling is raised by a
/// larger value, a floor (a key ending in `_rate`, such as `cache_hit_rate`) by a smaller one,
/// and any gated key by its removal or by a number set back to null or replaced by another shape.
/// A key that is new, or a null that gets its first number, is no raise (DR-89); `recorded`
/// changes only with the command it names, so it is no gate here (DR-51).
pub fn budget_raises(old: &Value, new: &Value) -> Vec<String> {
    let mut out = Vec::new();
    if let Some(old) = old.as_object() {
        for (key, value) in old.iter().filter(|(k, _)| k.as_str() != "recorded") {
            raises_at(key, key, value, new.get(key), &mut out);
        }
    }
    out
}

fn raises_at(path: &str, key: &str, old: &Value, new: Option<&Value>, out: &mut Vec<String>) {
    let floor = key.ends_with("_rate");
    match (old, new) {
        (_, None) => out.push(format!("{path}: removed")),
        (Value::Null, Some(_)) => {}
        (Value::Number(a), Some(Value::Number(b))) => {
            let (a, b) = (a.as_f64().unwrap_or(0.0), b.as_f64().unwrap_or(0.0));
            if floor && b < a {
                out.push(format!("{path}: a floor lowered from {a} to {b}"));
            } else if !floor && b > a {
                out.push(format!("{path}: raised from {a} to {b}"));
            }
        }
        (Value::Number(a), Some(Value::Null)) => out.push(format!("{path}: {a} set back to null")),
        (Value::Object(a), Some(Value::Object(b))) => {
            for (k, v) in a {
                raises_at(&format!("{path}.{k}"), k, v, b.get(k), out);
            }
        }
        (_, Some(other)) => out.push(format!("{path}: replaced by {other}")),
    }
}

/// The record headings `sha` adds to `decisions.md` over its first parent.
fn added_headings(root: &Path, sha: &str, parent: Option<&str>) -> Result<BTreeSet<u32>, String> {
    let at = |rev: &str| -> BTreeSet<u32> {
        Source::Commit(root, rev.to_owned())
            .read(DECISIONS)
            .map(|t| headings_in(&t).into_iter().map(|(n, _)| n).collect())
            .unwrap_or_default()
    };
    let before = parent.map(at).unwrap_or_default();
    Ok(at(sha).difference(&before).copied().collect())
}

/// Which commits CI walks for an event (DR-79).
#[derive(Debug, PartialEq, Eq)]
pub enum CiWalk {
    /// A `workflow_dispatch`, a `schedule`, a tag push or a local run: the state check only.
    StateOnly(String),
    /// `git rev-list` arguments naming the commits the push or pull request adds.
    Range(Vec<String>),
}

/// The walk for a GitHub Actions event: `before..sha` for a branch push whose `before` is an
/// ancestor of it; the commits no other remote branch holds for a new branch or a force push; the
/// pull request's base to its head; the state check alone for anything else.
pub fn ci_walk(
    event_name: &str,
    git_ref: &str,
    sha: &str,
    event: &Value,
    is_ancestor: &dyn Fn(&str, &str) -> bool,
) -> Result<CiWalk, String> {
    match event_name {
        "push" => {
            if let Some(tag) = git_ref.strip_prefix("refs/tags/") {
                return Ok(CiWalk::StateOnly(format!("a tag push ({tag})")));
            }
            let Some(branch) = git_ref.strip_prefix("refs/heads/") else {
                return Err(format!("a push to {git_ref:?}, which is neither a branch nor a tag"));
            };
            let before = event.get("before").and_then(Value::as_str).unwrap_or_default();
            let new_branch = before.is_empty() || before.bytes().all(|b| b == b'0');
            if !new_branch && is_ancestor(before, sha) {
                Ok(CiWalk::Range(vec![format!("{before}..{sha}")]))
            } else {
                Ok(CiWalk::Range(vec![
                    sha.to_owned(),
                    "--not".into(),
                    format!("--exclude=refs/remotes/origin/{branch}"),
                    "--remotes=origin".into(),
                ]))
            }
        }
        "pull_request" | "pull_request_target" => {
            let pr = event.get("pull_request").ok_or("the pull_request event has no pull_request")?;
            let base = pr.pointer("/base/sha").and_then(Value::as_str);
            let head = pr.pointer("/head/sha").and_then(Value::as_str);
            match (base, head) {
                (Some(base), Some(head)) => Ok(CiWalk::Range(vec![format!("{base}..{head}")])),
                _ => Err("the pull_request event names no base or head sha".into()),
            }
        }
        "" => Ok(CiWalk::StateOnly("not a CI run".into())),
        other => Ok(CiWalk::StateOnly(format!("a {other} run"))),
    }
}

/// The walk for this process's GitHub Actions environment, if any.
pub fn ci_walk_from_env(root: &Path) -> Result<CiWalk, String> {
    let name = std::env::var("GITHUB_EVENT_NAME").unwrap_or_default();
    if name.is_empty() {
        return Ok(CiWalk::StateOnly("not a CI run".into()));
    }
    let git_ref = std::env::var("GITHUB_REF").unwrap_or_default();
    let sha = std::env::var("GITHUB_SHA").unwrap_or_default();
    let event = match std::env::var("GITHUB_EVENT_PATH") {
        Ok(path) => {
            let text = fs::read_to_string(&path).map_err(|e| format!("{path}: {e}"))?;
            serde_json::from_str(&text).map_err(|e| format!("{path}: {e}"))?
        }
        Err(_) => Value::Null,
    };
    let is_ancestor = |a: &str, b: &str| util::run(root, "git", &["merge-base", "--is-ancestor", a, b]).0;
    ci_walk(&name, &git_ref, &sha, &event, &is_ancestor)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn record_headings_are_found_with_their_numbers() {
        let text =
            "# Decisions\n## DR-1: One\ntext\n## DR-12: Two\n## DR-3x: not\n### DR-4: deeper\n## DR-5\n";
        let found: Vec<u32> = headings_in(text).into_iter().map(|(n, _)| n).collect();
        assert_eq!(found, vec![1, 12, 5]);
        assert_eq!(duplicates(&headings_in("## DR-1: a\n## DR-2: b\n## DR-1: c\n")), vec![1]);
    }

    #[test]
    fn a_section_runs_to_the_next_second_level_heading() {
        let text = "## Boundaries\n\n- a  \n### Sub\n- b\n\n## Conventions\n- c\n";
        assert_eq!(section_of(text, "Boundaries").as_deref(), Some("- a\n### Sub\n- b"));
        assert_eq!(section_of(text, "Conventions").as_deref(), Some("- c"));
        assert_eq!(section_of(text, "Missing"), None);
    }

    #[test]
    fn a_dependency_version_is_not_part_of_a_package_surface() {
        let a = package_surface("p", r#"{"exports":{".":"./a.ts"},"dependencies":{"x":"1.0.0"}}"#).unwrap();
        let b = package_surface("p", r#"{"dependencies":{"x":"2.0.0"},"exports":{".":"./a.ts"}}"#).unwrap();
        let c = package_surface("p", r#"{"dependencies":{"y":"1.0.0"},"exports":{".":"./a.ts"}}"#).unwrap();
        assert_eq!(a, b);
        assert_ne!(a, c);
    }

    #[test]
    fn the_lock_round_trips() {
        let mut lock = Lock::new();
        lock.insert("package packages/ui".into(), Entry { sha256: "ab".into(), record: 79 });
        lock.insert("boundaries.toml".into(), Entry { sha256: "cd".into(), record: 83 });
        assert_eq!(parse_lock(&render_lock(&lock)).unwrap(), lock);
    }

    #[test]
    fn a_dispatch_run_checks_state_only() {
        let never = |_: &str, _: &str| panic!("no ancestry question for a dispatch");
        for event in ["workflow_dispatch", "schedule"] {
            let walk = ci_walk(event, "refs/heads/main", "abc", &json!({}), &never).unwrap();
            assert!(matches!(walk, CiWalk::StateOnly(_)), "{event}: {walk:?}");
        }
    }

    #[test]
    fn a_tag_push_checks_state_only() {
        let never = |_: &str, _: &str| panic!("no ancestry question for a tag");
        let walk = ci_walk("push", "refs/tags/v0.1.0", "abc", &json!({"before": "def"}), &never).unwrap();
        assert!(matches!(walk, CiWalk::StateOnly(_)), "{walk:?}");
    }

    #[test]
    fn a_branch_push_walks_what_it_adds() {
        let yes = |_: &str, _: &str| true;
        let no = |_: &str, _: &str| false;
        let ev = json!({"before": "b4"});
        assert_eq!(
            ci_walk("push", "refs/heads/topic", "tip", &ev, &yes).unwrap(),
            CiWalk::Range(vec!["b4..tip".into()])
        );
        let not_held = CiWalk::Range(vec![
            "tip".into(),
            "--not".into(),
            "--exclude=refs/remotes/origin/topic".into(),
            "--remotes=origin".into(),
        ]);
        assert_eq!(ci_walk("push", "refs/heads/topic", "tip", &ev, &no).unwrap(), not_held);
        let new_branch = json!({"before": "0000000000000000000000000000000000000000"});
        assert_eq!(ci_walk("push", "refs/heads/topic", "tip", &new_branch, &yes).unwrap(), not_held);
        let pr = json!({"pull_request": {"base": {"sha": "base"}, "head": {"sha": "head"}}});
        assert_eq!(
            ci_walk("pull_request", "refs/pull/1/merge", "merge", &pr, &yes).unwrap(),
            CiWalk::Range(vec!["base..head".into()])
        );
    }
}
