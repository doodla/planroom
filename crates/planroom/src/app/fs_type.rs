//! The real filesystem type for the store's filesystem check (architecture.md, Where the
//! workspace lives; DR-1), and the start-up registration that hands it to the store.
//!
//! The store crate classifies a place on its filesystem's type name and its path, and never
//! reads the type itself: it has no `libc`. This module reads it with `statfs(2)`: on Linux the
//! superblock magic number (`f_type`), named here for the filesystems the check knows; on macOS
//! the type name the kernel gives (`f_fstypename`).

use std::ffi::CString;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

use planroom_contract::{Warning, WarningCode};
use planroom_store::{Environment, Notice, Placement};
use serde_json::json;

use crate::app::data;
use crate::text;

/// Hand the store the data root and [`fs_type`], once, at start-up. With no data root (no
/// absolute `PLANROOM_DATA`, `XDG_DATA_HOME` or `HOME`), nothing is registered and the store
/// opens where it's asked; the commands that need the data root refuse on their own.
pub fn register() {
    if let Ok(data_root) = data::root() {
        planroom_store::register(Environment {
            data_root,
            fs_type,
            engine_version: env!("CARGO_PKG_VERSION").into(),
        });
    }
}

/// The type of the filesystem the existing path `path` is on, such as `ext4`, `nfs` or `apfs`;
/// `None` when `statfs` fails. An unnamed Linux type is its magic number in hex.
pub fn fs_type(path: &Path) -> Option<String> {
    let c_path = CString::new(path.as_os_str().as_bytes()).ok()?;
    let mut stat = std::mem::MaybeUninit::<libc::statfs>::zeroed();
    // SAFETY: `c_path` is a NUL-terminated string and `stat` is a writable `statfs` buffer.
    let rc = unsafe { libc::statfs(c_path.as_ptr(), stat.as_mut_ptr()) };
    if rc != 0 {
        return None;
    }
    // SAFETY: `statfs` returned 0, so it filled the buffer.
    let stat = unsafe { stat.assume_init() };
    type_name(&stat)
}

#[cfg(target_os = "linux")]
fn type_name(stat: &libc::statfs) -> Option<String> {
    // `f_type`'s width and sign differ between targets; its low 32 bits are the magic number.
    #[allow(clippy::unnecessary_cast)]
    Some(linux_name(stat.f_type as i64 as u32))
}

#[cfg(any(target_os = "macos", target_os = "freebsd"))]
fn type_name(stat: &libc::statfs) -> Option<String> {
    let bytes: Vec<u8> = stat.f_fstypename.iter().take_while(|&&c| c != 0).map(|&c| c as u8).collect();
    Some(String::from_utf8_lossy(&bytes).into_owned()).filter(|name| !name.is_empty())
}

/// Elsewhere the type is unknown, so the check passes, as the store's check does for any
/// type it doesn't list.
#[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "freebsd")))]
fn type_name(_: &libc::statfs) -> Option<String> {
    None
}

/// A Linux superblock magic number's name (`linux/magic.h` and the filesystems' own headers).
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn linux_name(magic: u32) -> String {
    let name = match magic {
        0xEF53 => "ext4",
        0x9123_683E => "btrfs",
        0x5846_5342 => "xfs",
        0x2FC1_2FC1 => "zfs",
        0x0102_1994 => "tmpfs",
        0x794C_7630 => "overlay",
        0xF2F5_2010 => "f2fs",
        0x6969 => "nfs",
        0x517B => "smb",
        0xFF53_4D42 => "cifs",
        0xFE53_4D42 => "smb2",
        0x0102_1997 => "9p",
        0x6573_5546 => "fuse",
        0x5346_414F => "afs",
        0x00C3_6400 => "ceph",
        0x7375_7245 => "coda",
        0x564C => "ncpfs",
        0x0BD0_0BD0 => "lustre",
        0x4750_4653 => "gpfs",
        _ => return format!("0x{magic:x}"),
    };
    name.to_owned()
}

/// The warning a store notice becomes on the command's reply.
pub fn warning(notice: Notice) -> Warning {
    match notice {
        Notice::UnsafeFilesystem { placement, path, fs, store } => {
            let (path, store) = (path.display().to_string(), store.display().to_string());
            let key = match placement {
                Placement::Moved => "warning.unsafe_filesystem_moved",
                Placement::Kept => "warning.unsafe_filesystem_kept",
                Placement::DataRoot => "warning.unsafe_filesystem_data_root",
            };
            Warning {
                code: WarningCode::UnsafeFilesystem,
                message: text::get(key, &[("path", &path), ("fs", &fs), ("store", &store)]),
                details: json!({"path": path, "fs": fs, "store": store}),
            }
        }
        Notice::Migrated { from, to, backup } => {
            let backup = backup.display().to_string();
            Warning {
                code: WarningCode::StoreMigrated,
                message: text::get(
                    "warning.store_migrated",
                    &[("from", &from), ("to", &to), ("backup", &backup)],
                ),
                details: json!({"from": from, "to": to, "backup": backup}),
            }
        }
        Notice::BackupBehind { backup_seq, file_seq, last_authored } => Warning {
            code: WarningCode::BackupBehind,
            message: text::get(
                "warning.backup_behind",
                &[("backup_seq", &backup_seq), ("file_seq", &file_seq), ("last_authored", &last_authored)],
            ),
            details: json!({"backup_seq": backup_seq, "file_seq": file_seq, "last_authored": last_authored}),
        },
        Notice::TemporaryHost { repo, reference } => Warning {
            code: WarningCode::TemporaryHost,
            message: text::get("warning.temporary_host", &[("ref", &reference), ("repo", &repo)]),
            details: json!({"repo": repo, "ref": reference}),
        },
        Notice::BackupRefFailed { repo, reference } => Warning {
            code: WarningCode::BackupRefFailed,
            message: text::get("warning.backup_ref_failed", &[("ref", &reference), ("repo", &repo)]),
            details: json!({"repo": repo, "ref": reference}),
        },
    }
}

/// The store's notices since the last call, as warnings.
pub fn warnings() -> Vec<Warning> {
    planroom_store::take_notices().into_iter().map(warning).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_network_and_fuse_magics_have_the_names_the_store_check_knows() {
        for (magic, name) in
            [(0x6969, "nfs"), (0xFF53_4D42, "cifs"), (0x0102_1997, "9p"), (0x6573_5546, "fuse")]
        {
            assert_eq!(linux_name(magic), name);
            assert!(planroom_store::fs_check::classify(name, Path::new("/home/a/ws")).is_some(), "{name}");
        }
        assert_eq!(linux_name(0x1234), "0x1234");
        assert!(planroom_store::fs_check::classify(&linux_name(0xEF53), Path::new("/home/a/ws")).is_none());
    }

    #[test]
    fn the_backup_notices_become_their_warnings() {
        let behind = warning(Notice::BackupBehind { backup_seq: 3, file_seq: 4, last_authored: 9 });
        assert_eq!(behind.code, WarningCode::BackupBehind);
        assert_eq!(behind.details, json!({"backup_seq": 3, "file_seq": 4, "last_authored": 9}));
        assert!(behind.message.contains('9') && !behind.message.contains('{'), "{}", behind.message);

        let reference = "refs/planroom/backup/ws-1";
        let copied = warning(Notice::TemporaryHost { repo: "app".into(), reference: reference.into() });
        assert_eq!(copied.code, WarningCode::TemporaryHost);
        assert_eq!(copied.details, json!({"repo": "app", "ref": reference}));
        assert!(copied.message.contains(reference) && copied.message.contains("app"), "{}", copied.message);

        let failed = warning(Notice::BackupRefFailed { repo: "app".into(), reference: reference.into() });
        assert_eq!(failed.code, WarningCode::BackupRefFailed);
        assert_eq!(failed.details, json!({"repo": "app", "ref": reference}));
        assert!(failed.message.contains(reference) && !failed.message.contains('{'), "{}", failed.message);
    }

    #[test]
    fn a_temporary_directory_has_a_type() {
        let dir = tempfile::tempdir().unwrap();
        assert!(fs_type(dir.path()).is_some_and(|t| !t.is_empty()));
        assert_eq!(fs_type(&dir.path().join("missing")), None);
    }
}
