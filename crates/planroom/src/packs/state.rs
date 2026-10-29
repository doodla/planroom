//! The pack set as workspace state (architecture.md, The pack set is workspace state; DR-62).
//!
//! - The store's `meta` records the set: every pack's record in load order, its hash under the
//!   identity of the engine that recorded it, that engine, and when the workspace config's list
//!   was read ([`RecordedPacks`]).
//! - An out-of-tree pack loads only from its copy, `STATE_DIR/packs/<sha256>/`, named by the
//!   sha256 over its files ([`copy`]). A copy holds exactly the files [`read_dir`] reads, with
//!   directories at 0700 and files at 0600, and is never deleted here: an undo reads back the copy
//!   an update replaced, and pruning copies no record names is `gc`'s.
//! - A scan extracts with the built-ins alone in an untrusted workspace, and otherwise with the
//!   built-ins and each recorded copy, in record order ([`for_scan`]). Its first trusted scan
//!   copies the packs the config lists; after that the config counts only through `pack add` and
//!   `pack remove`.
//! - Every authored write runs [`default_verdict`] on the record under the write lock: only an
//!   older engine than the one that recorded the set is refused, and any other engine records its
//!   own built-ins with the record's out-of-tree packs unchanged.

use std::collections::BTreeSet;
use std::fs::{self, DirBuilder, OpenOptions};
use std::io::Write;
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};

use planroom_contract::format::compare_versions;
use planroom_contract::{ErrorCode, PackRecord};
use planroom_store::{PackGuard, PackVerdict, RecordedPacks};

use super::{Loaded, Source, embed, engine, read_dir, resolve, set_hash};
use crate::app::error::AppError;
use crate::app::trust::Trust;
use crate::text;

/// The state directory's subdirectory that holds the copies.
pub const DIR: &str = "packs";
/// What a built-in pack's record names as its source.
pub const BUILTIN: &str = "builtin";

/// The directory of the copies in the state directory `state_dir`.
pub fn dir(state_dir: &Path) -> PathBuf {
    state_dir.join(DIR)
}

/// Where the copy named `sha256` sits.
pub fn copy_path(state_dir: &Path, sha256: &str) -> PathBuf {
    dir(state_dir).join(sha256)
}

/// The out-of-tree records of `records`, in their order.
pub fn out_of_tree(records: &[PackRecord]) -> Vec<PackRecord> {
    records.iter().filter(|r| r.source != BUILTIN).cloned().collect()
}

/// The record of a set `packs`, recorded by this engine.
pub fn record_of(packs: Vec<PackRecord>, config_read: Option<String>) -> RecordedPacks {
    RecordedPacks { hash: set_hash(&packs), packs, engine: Some(engine()), config_read }
}

/// The verdict every authored write takes on the record `recorded` it finds, given this engine's
/// built-in records `builtins`: the set is this engine's built-ins followed by the record's
/// out-of-tree records, unchanged, hashed under this engine's identity.
///
/// - The record's hash is that set's: keep it.
/// - No record yet: record it, as an older store's first write.
/// - This engine is older than the one that recorded it: refuse, since it can't know what that
///   engine's built-ins extract.
/// - Otherwise, a newer engine, the same version at another commit, or an identity that isn't
///   semver: record the set, so a newer engine records its own built-ins at its first write. The
///   out-of-tree records stay as they are, so trust never gates this.
pub fn default_verdict(builtins: &[PackRecord], recorded: Option<&RecordedPacks>) -> PackVerdict {
    let identity = engine();
    let set = engine_set(builtins, recorded);
    let Some(recorded) = recorded else { return PackVerdict::Record(record_of(set, None)) };
    if recorded.hash == set_hash(&set) {
        return PackVerdict::Keep;
    }
    // An absent engine counts as older than this one.
    let older = recorded
        .engine
        .as_deref()
        .is_some_and(|by| compare_versions(&identity, by) == Some(std::cmp::Ordering::Less));
    if older {
        return PackVerdict::Refuse { recorded: recorded.engine.clone(), resolved: identity };
    }
    PackVerdict::Record(record_of(set, recorded.config_read.clone()))
}

/// This engine's set, given its built-in records `builtins` and the record `recorded`: the
/// built-ins, then the record's out-of-tree records in their order.
pub fn engine_set(builtins: &[PackRecord], recorded: Option<&RecordedPacks>) -> Vec<PackRecord> {
    let mut set = builtins.to_vec();
    set.extend(recorded.map(|r| out_of_tree(&r.packs)).unwrap_or_default());
    set
}

/// This engine's built-in records, read once per process.
pub fn builtin_records() -> Result<&'static [PackRecord], AppError> {
    static RECORDS: OnceLock<Vec<PackRecord>> = OnceLock::new();
    if let Some(records) = RECORDS.get() {
        return Ok(records);
    }
    let records = super::loaded()?.records();
    Ok(RECORDS.get_or_init(|| records))
}

/// The guard a store this process opens to write runs: [`default_verdict`] over this engine's
/// built-ins. `None` only if the built-ins don't load, which every command reports first.
pub fn default_guard() -> Option<Box<PackGuard>> {
    let builtins = builtin_records().ok()?;
    Some(Box::new(move |recorded: Option<&RecordedPacks>| default_verdict(builtins, recorded)))
}

/// The guard of a scan that read the config's list and extracted with `set`: it records `set`,
/// and when the list was read, as long as the record's out-of-tree part, inside the write's own
/// transaction, is still `read`, what the scan read before it extracted. If a concurrent
/// `pack add` or `pack remove` moved it, or the engine is older than the record's, the default
/// verdict stands and the scan records nothing of its own.
pub fn config_guard(set: Vec<PackRecord>, read: Vec<PackRecord>, at: String) -> Option<Box<PackGuard>> {
    let builtins = builtin_records().ok()?;
    Some(Box::new(move |recorded: Option<&RecordedPacks>| {
        let verdict = default_verdict(builtins, recorded);
        if matches!(verdict, PackVerdict::Refuse { .. }) {
            return verdict;
        }
        let now = recorded.map(|r| out_of_tree(&r.packs)).unwrap_or_default();
        if now != read {
            return verdict;
        }
        PackVerdict::Record(record_of(set.clone(), Some(at.clone())))
    }))
}

/// What a scan extracts with, and when it read the config's list, the set to record.
pub struct ForScan {
    pub loaded: Loaded,
    /// The set this scan read from the workspace config and copied, to record with the time it
    /// was read; none when the record already counts the config.
    pub config: Option<Vec<PackRecord>>,
}

/// The pack set a scan of the workspace at `root`, whose state directory is `state_dir`, extracts
/// with:
///
/// - untrusted, or with no state directory yet: the built-ins alone, as this process loaded them,
///   so their compiled queries are shared (DR-48, DR-103);
/// - trusted, before the config's list was read: the built-ins and each pack `config_packs`
///   lists, each placed from the root, validated, compiled and copied;
/// - trusted after that: the built-ins and each recorded copy in record order, cached
///   process-wide by the set's hash, so a `watch` or `serve` that scans again compiles nothing
///   new.
pub fn for_scan(
    state_dir: &Path,
    root: &Path,
    trust: Trust,
    config_packs: &[String],
    recorded: Option<&RecordedPacks>,
) -> Result<ForScan, AppError> {
    let builtins = super::loaded()?;
    if trust != Trust::Trusted {
        return Ok(ForScan { loaded: builtins.clone(), config: None });
    }
    if recorded.is_none_or(|r| r.config_read.is_none()) {
        let mut sources = Vec::new();
        for entry in config_packs {
            let dir = place(root, entry)?;
            sources.push(read_named(&dir)?);
        }
        let loaded = if sources.is_empty() {
            builtins.clone()
        } else {
            validated(sources.clone())?;
            // Copied once the whole list loads, and read from the copies as every later scan is.
            let copies = sources.iter().map(|s| copied(state_dir, s)).collect::<Result<Vec<_>, _>>()?;
            validated(copies)?
        };
        return Ok(ForScan { config: Some(loaded.records()), loaded });
    }
    let oot = recorded.map(|r| out_of_tree(&r.packs)).unwrap_or_default();
    if oot.is_empty() {
        return Ok(ForScan { loaded: builtins.clone(), config: None });
    }
    let mut set = builtin_records()?.to_vec();
    set.extend(oot.iter().cloned());
    let key = set_hash(&set);
    static CACHE: Mutex<Option<(String, Loaded)>> = Mutex::new(None);
    if let Some((hash, loaded)) = CACHE.lock().map_err(AppError::internal)?.as_ref()
        && *hash == key
    {
        return Ok(ForScan { loaded: loaded.clone(), config: None });
    }
    let copies = oot.iter().map(|r| load_copy(state_dir, r)).collect::<Result<Vec<_>, _>>()?;
    let loaded = validated(copies)?;
    *CACHE.lock().map_err(AppError::internal)? = Some((key, loaded.clone()));
    Ok(ForScan { loaded, config: None })
}

/// The built-ins and `sources`, out-of-tree, checked as one set and with every declarative
/// extractor compiled, so a pack whose query doesn't compile is `pack_invalid` before it is
/// copied or recorded (DR-103).
/// A failure in one of them takes the hint that checks that pack ([`checked_by`]).
pub fn validated(sources: Vec<Source>) -> Result<Loaded, AppError> {
    let dirs: Vec<(String, PathBuf)> =
        sources.iter().filter_map(|s| Some((s.label.clone(), s.dir.clone()?))).collect();
    let check = |e: AppError| {
        let file = e.details["file"].as_str().unwrap_or_default();
        let ours = |label: &str| {
            file.strip_prefix(label).is_some_and(|rest| rest.is_empty() || rest.starts_with('/'))
        };
        match dirs.iter().find(|(label, _)| ours(label)) {
            Some((_, dir)) => checked_by(e, dir),
            None => e,
        }
    };
    let mut all = embed::sources();
    all.extend(sources);
    let loaded = resolve(all).map_err(check)?;
    loaded.programs().map_err(check)?;
    Ok(loaded)
}

/// `e` with the exact line that checks the pack in `dir`, `planroom doctor --pack DIR` with `DIR`
/// quoted for a shell, in place of the catalog's hint, whose `DIR` names no directory. Any other
/// error, or one that carries a hint of its own, is returned as it is.
pub fn checked_by(e: AppError, dir: &Path) -> AppError {
    if e.code != ErrorCode::PackInvalid || e.hint != text::try_get("hint.pack_invalid") {
        return e;
    }
    match dir.to_str() {
        Some(dir) => e.with_hint(format!("planroom doctor --pack {}", text::shell_word(dir))),
        None => e.without_hint(),
    }
}

/// The directory a config entry or a person's `DIR` names, placed from `base` and canonical, as
/// a record names it: `pack_invalid` naming it when it doesn't resolve, with the hint that checks
/// it ([`checked_by`]), or isn't UTF-8.
pub fn place(base: &Path, entry: &str) -> Result<PathBuf, AppError> {
    let placed = base.join(entry);
    let shown = placed.display().to_string();
    let canonical = fs::canonicalize(&placed).map_err(|e| {
        checked_by(AppError::pack_invalid(&shown, 1, &shown, "unreadable", &[("why", &e)]), &placed)
    })?;
    if canonical.to_str().is_none() {
        // No line a shell takes as typed names this directory.
        let why = text::get("pack.not_utf8", &[]);
        return Err(AppError::pack_invalid(&shown, 1, &shown, "unreadable", &[("why", &why)]).without_hint());
    }
    Ok(canonical)
}

/// The pack in `dir`, a canonical directory, read with the bounded walk, recorded as `dir`. A
/// failure takes the hint that checks it ([`checked_by`]).
pub fn read_named(dir: &Path) -> Result<Source, AppError> {
    let mut source = read_dir(dir).map_err(|e| checked_by(e, dir))?;
    source.recorded = Some(dir.display().to_string());
    Ok(source)
}

/// `source` copied into the state directory and read back as its copy: the copy's path as its
/// label, so errors name the copy, and the directory it came from as its record's source.
pub fn copied(state_dir: &Path, source: &Source) -> Result<Source, AppError> {
    let sha = copy(state_dir, source)?;
    let mut back = read_dir(&copy_path(state_dir, &sha))?;
    back.recorded = source.recorded.clone().or_else(|| Some(source.label.clone()));
    Ok(back)
}

/// The copy `record` names, read from `STATE_DIR/packs/<sha256>/` and checked against its name.
/// A copy that is missing, or whose files no longer hash to its name, is copied again from the
/// record's source when the source still hashes to it; a bad copy is renamed aside first.
/// Otherwise it is `pack_invalid`: with the hint `planroom pack add SOURCE` when the source reads
/// with another hash, and `planroom pack remove ID` when it doesn't read.
pub fn load_copy(state_dir: &Path, record: &PackRecord) -> Result<Source, AppError> {
    let path = copy_path(state_dir, &record.sha256);
    let as_recorded = |mut source: Source| {
        source.recorded = Some(record.source.clone());
        source
    };
    if let Ok(source) = read_dir(&path)
        && source.sha256() == record.sha256
    {
        return Ok(as_recorded(source));
    }
    let why = if path.exists() { "copy_changed" } else { "copy_missing" };
    let shown = path.display().to_string();
    let vars: [(&str, &dyn std::fmt::Display); 3] = [
        ("id", &record.id),
        ("copy", &text::shell_word(&shown)),
        ("source", &text::shell_word(&record.source)),
    ];
    match read_dir(Path::new(&record.source)) {
        Ok(source) if source.sha256() == record.sha256 => {
            copy(state_dir, &source)?;
            Ok(as_recorded(read_dir(&path)?))
        }
        Ok(_) => Err(AppError::pack_invalid(&shown, 1, &record.id, why, &vars)
            .with_hint(format!("planroom pack add {}", text::shell_word(&record.source)))),
        Err(_) => Err(AppError::pack_invalid(&shown, 1, &record.id, why, &vars)
            .with_hint(format!("planroom pack remove {}", text::shell_word(&record.id)))),
    }
}

/// Copy `source`'s files into `STATE_DIR/packs/<sha256>/`, named by [`Source::sha256`]: written
/// to `packs/.<sha256>.<uuid>.tmp/`, then renamed into place. A target that already hashes to its
/// name is reused; one that doesn't is first renamed aside to `packs/.<sha256>.<uuid>.stale/`,
/// which stays. Returns the copy's name.
pub fn copy(state_dir: &Path, source: &Source) -> Result<String, AppError> {
    let sha = source.sha256();
    let packs = dir(state_dir);
    private_dir(&packs)?;
    let target = packs.join(&sha);
    let holds = |dir: &Path| read_dir(dir).is_ok_and(|s| s.sha256() == sha);
    if fs::symlink_metadata(&target).is_ok() {
        if holds(&target) {
            return Ok(sha);
        }
        let aside = packs.join(format!(".{sha}.{}.stale", uuid::Uuid::now_v7()));
        fs::rename(&target, &aside).map_err(|e| io(&target, e))?;
    }
    let tmp = packs.join(format!(".{sha}.{}.tmp", uuid::Uuid::now_v7()));
    private_dir(&tmp)?;
    for (rel, bytes) in &source.files {
        let file = tmp.join(rel);
        if let Some(parent) = file.parent() {
            private_dir(parent)?;
        }
        let mut out = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(&file)
            .map_err(|e| io(&file, e))?;
        out.write_all(bytes).and_then(|()| out.sync_all()).map_err(|e| io(&file, e))?;
    }
    if fs::rename(&tmp, &target).is_err() {
        // Another process put the same copy in place first.
        let _ = fs::remove_dir_all(&tmp);
        if !holds(&target) {
            return Err(AppError::internal(format!(
                "{}: the pack's copy can't be put in place",
                target.display()
            )));
        }
    }
    Ok(sha)
}

fn private_dir(dir: &Path) -> Result<(), AppError> {
    DirBuilder::new().recursive(true).mode(0o700).create(dir).map_err(|e| io(dir, e))
}

fn io(path: &Path, e: std::io::Error) -> AppError {
    AppError::internal(format!("{}: {e}", path.display()))
}

/// A recorded out-of-tree pack whose source no longer hashes to its copy: what the changed-source
/// card asks about.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Changed {
    pub id: String,
    /// The recorded copy's sha256.
    pub from: String,
    /// The source's sha256 as it reads now.
    pub to: String,
    /// The paths added, removed or changed.
    pub files: u32,
}

/// Each recorded out-of-tree pack of `recorded` whose source now reads with another hash than its
/// copy, compared file by file with the copy `loaded` holds. A source that can't be read, has
/// moved, holds a link or an executable file, or passes a bound gives nothing: a missing source
/// changes nothing.
pub fn changed(loaded: &Loaded, recorded: &RecordedPacks) -> Vec<Changed> {
    let mut out = Vec::new();
    for record in out_of_tree(&recorded.packs) {
        let Ok(source) = read_dir(Path::new(&record.source)) else { continue };
        let to = source.sha256();
        if to == record.sha256 {
            continue;
        }
        let Some(pack) = loaded.packs.iter().find(|p| p.sha256 == record.sha256) else { continue };
        let paths: BTreeSet<&String> = pack.source.files.keys().chain(source.files.keys()).collect();
        let files = paths.into_iter().filter(|p| pack.source.files.get(*p) != source.files.get(*p)).count();
        out.push(Changed { id: record.id, from: record.sha256, to, files: files as u32 });
    }
    out
}
