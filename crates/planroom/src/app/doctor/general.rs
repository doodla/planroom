//! `doctor` with no flag: git, the store, its migrations, the pack set and the event backup
//! (architecture.md, Store schema; DR-53, DR-89).
//!
//! - It opens the workspace's store, when there is one, as every command does, so an older store
//!   is migrated and the reply carries `store_migrated`, and a newer one is `schema_mismatch`.
//!   Then it reads one snapshot. It runs no write and creates no state directory.
//! - Outside a workspace, or before its first scan, the store, its migrations and the backup are
//!   null, and the packs are this engine's built-ins with no record to match.
//! - It keeps to `doctor`'s 1,500 tokens whatever the paths: each path goes through [`shown`], the
//!   migration copies are file names, and the pack list stops at [`SHOWN_PACKS`].

use std::path::Path;

use planroom_contract::{
    DoctorBackup, DoctorGit, DoctorMigrations, DoctorPacks, DoctorReport, DoctorStore, PackRecord, Warning,
};
use planroom_extract::git::TooOld;
use planroom_store::{STORE_VERSION, Store};

use crate::app::error::AppError;
use crate::app::{Reply, data, workspace};
use crate::packs::set_hash;
use crate::packs::state::{BUILTIN, builtin_records, engine_set};
use crate::text;

/// The packs the report lists; `more` counts the rest.
pub const SHOWN_PACKS: usize = 6;
/// The migration copies the report names.
const SHOWN_COPIES: usize = 3;
/// The longest path, in characters, the report shows whole.
const SHOWN_PATH: usize = 160;
/// The backup file in it.
const BACKUP_FILE: &str = "events.ndjson";

/// The general report for the workspace at or above `cwd`.
pub fn run(cwd: &Path) -> Result<Reply<DoctorReport>, AppError> {
    let git = DoctorGit {
        version: planroom_extract::git::version().map_err(|e| AppError::git_too_old(&e))?,
        minimum: TooOld::minimum(),
    };
    let builtins = builtin_records()?;
    let store = match workspace::find_root(cwd) {
        Some(root) => Store::open_existing(&workspace::store_path(&root))?,
        None => None,
    };
    let Some(store) = store else {
        let packs = packs(builtins, None);
        let text = lines(text::get("doctor.summary_no_workspace", &[]), &git, None, None, &packs, None);
        let result = DoctorReport {
            summary: text::get("doctor.summary_no_workspace", &[]),
            git,
            store: None,
            migrations: None,
            packs,
            backup: None,
        };
        return Ok(Reply { result, seq: 0, warnings: vec![], text });
    };

    let snapshot = store.snapshot()?;
    let workspace_id = snapshot.meta("workspace_id")?.unwrap_or_default();
    let seq = snapshot.seq();
    let found = DoctorStore {
        path: shown(store.path()),
        workspace_id: workspace_id.clone(),
        seq,
        events: snapshot.event_count()?,
        engine_version: snapshot.meta("engine_version")?,
    };
    let store_version = snapshot.meta("store_version")?.and_then(|v| v.parse().ok()).unwrap_or(STORE_VERSION);
    let mark = snapshot.backup_mark()?;
    let recorded = store.recorded_packs()?;
    drop(snapshot);

    // The workspace's directory under the data root, when its ID can name one, by the rule the
    // write-through and the migration copies follow.
    let data_root = data::root()?;
    let dir = planroom_store::dir_of(&data_root, &workspace_id);
    let migrations = DoctorMigrations {
        store_version,
        supported: STORE_VERSION,
        copies: dir.as_deref().map(copies).unwrap_or_default(),
    };
    let packs = packs(builtins, recorded.as_ref());
    // What the file holds of this store, by the write-through's own test. An ID that names no
    // directory leaves no place a backup can exist, so the backups area holds nothing of it.
    let (file, file_seq) = match &dir {
        Some(dir) => {
            let file = dir.join(BACKUP_FILE);
            let held = planroom_store::held_seq(&file, &workspace_id, store_version, store.state_dir(), seq);
            (file, held)
        }
        None => (planroom_store::backups_area(&data_root), 0),
    };
    let (backup, warning) = super::backup::check(&mark, &file, file_seq);
    let summary = text::get("doctor.summary", &[]);
    let text = lines(summary.clone(), &git, Some(&found), Some(&migrations), &packs, Some(&backup));
    let result = DoctorReport {
        summary,
        git,
        store: Some(found),
        migrations: Some(migrations),
        packs,
        backup: Some(backup),
    };
    let warnings: Vec<Warning> = warning.into_iter().collect();
    Ok(Reply { result, seq, warnings, text })
}

/// `path` as the report shows it: whole up to [`SHOWN_PATH`] characters, else `…` and its last
/// `SHOWN_PATH - 1`, the end that tells one workspace's file from another's.
pub fn shown(path: &Path) -> String {
    let path = path.to_string_lossy();
    let count = path.chars().count();
    if count <= SHOWN_PATH {
        return path.into_owned();
    }
    let tail: String = path.chars().skip(count - (SHOWN_PATH - 1)).collect();
    format!("…{tail}")
}

/// The `store-vN.db` copies in `dir`, by file name, newest first, at most [`SHOWN_COPIES`].
fn copies(dir: &Path) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(dir) else { return Vec::new() };
    let mut found: Vec<(u32, String)> = entries
        .filter_map(Result::ok)
        .filter_map(|e| {
            let name = e.file_name().into_string().ok()?;
            let n = name.strip_prefix("store-v")?.strip_suffix(".db")?.parse().ok()?;
            Some((n, name))
        })
        .collect();
    found.sort_by_key(|(n, _)| std::cmp::Reverse(*n));
    found.into_iter().take(SHOWN_COPIES).map(|(_, name)| name).collect()
}

/// The pack set: the recorded one, or this engine's built-ins when none is recorded, against this
/// engine's set as the next write's verdict builds it.
fn packs(builtins: &[PackRecord], recorded: Option<&planroom_store::RecordedPacks>) -> DoctorPacks {
    let hash = set_hash(&engine_set(builtins, recorded));
    let set = recorded.map_or_else(|| builtins.to_vec(), |r| r.packs.clone());
    let more = set.len().saturating_sub(SHOWN_PACKS) as u32;
    let packs = set
        .into_iter()
        .take(SHOWN_PACKS)
        .map(|mut p| {
            if p.source != BUILTIN {
                p.source = shown(Path::new(&p.source));
            }
            p
        })
        .collect();
    let recorded_hash = recorded.map(|r| r.hash.clone());
    let matches = recorded_hash.as_ref().map(|h| *h == hash);
    DoctorPacks { packs, more, recorded_hash, hash, matches }
}

/// What text mode prints: the summary, then one line per check.
fn lines(
    summary: String,
    git: &DoctorGit,
    store: Option<&DoctorStore>,
    migrations: Option<&DoctorMigrations>,
    packs: &DoctorPacks,
    backup: Option<&DoctorBackup>,
) -> String {
    let mut out =
        vec![summary, text::get("doctor.git", &[("version", &git.version), ("minimum", &git.minimum)])];
    if let Some(s) = store {
        let engine = s.engine_version.as_deref().unwrap_or_default();
        out.push(text::get(
            "doctor.store",
            &[
                ("path", &s.path),
                ("workspace_id", &s.workspace_id),
                ("seq", &s.seq),
                ("events", &s.events),
                ("engine", &engine),
            ],
        ));
    }
    if let Some(m) = migrations {
        out.push(text::get(
            "doctor.migrations",
            &[("store_version", &m.store_version), ("supported", &m.supported), ("copies", &m.copies.len())],
        ));
    }
    let ids: Vec<&str> = packs.packs.iter().map(|p| p.id.as_str()).collect();
    let key = if packs.matches == Some(false) { "doctor.packs_changed" } else { "doctor.packs" };
    out.push(text::get(key, &[("packs", &ids.join(", ")), ("more", &packs.more)]));
    if let Some(b) = backup {
        out.push(text::get(
            "doctor.backup",
            &[
                ("file", &b.file),
                ("file_seq", &b.file_seq),
                ("backup_seq", &b.backup_seq),
                ("last_authored", &b.last_authored),
            ],
        ));
        if let Some(repo) = &b.backup_ref {
            out.push(text::get("doctor.backup_ref", &[("repo", repo)]));
        }
    }
    out.join("\n")
}
