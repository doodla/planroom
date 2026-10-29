//! `doctor`'s event backup check (DR-53): how far the backup file and the store's mark have come
//! against the last person-authored event, and the `backup_behind` warning when either is short of
//! it, as a killed process or a failed write-through leaves them. The file counts by the
//! write-through's own test ([`planroom_store::held_seq`]), so a file it would rename aside, or a
//! workspace ID that names no backup directory, holds nothing. The next person-authored commit
//! catches up once a backup can be written.

use std::path::Path;

use planroom_contract::{DoctorBackup, Seq, Warning};
use planroom_store::{BackupMark, Notice};

use super::general::shown;
use crate::app::fs_type;

/// The backup's state from the store's `mark` and the backup file `file`, which holds the store's
/// events through `file_seq`, and the warning when it is behind.
pub fn check(mark: &BackupMark, file: &Path, file_seq: Seq) -> (DoctorBackup, Option<Warning>) {
    let BackupMark { backup_seq, last_authored, backup_ref } = mark.clone();
    let behind = backup_seq.min(file_seq) < last_authored;
    let warning =
        behind.then(|| fs_type::warning(Notice::BackupBehind { backup_seq, file_seq, last_authored }));
    let backup = DoctorBackup {
        file: shown(file),
        backup_seq,
        file_seq,
        last_authored,
        behind,
        temporary_host: crate::app::backup::temporary_host(),
        backup_ref: backup_ref.map(|r| shown(Path::new(&r))),
    };
    (backup, warning)
}
