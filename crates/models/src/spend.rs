//! The build-wide spend log (architecture.md, One build-wide spend record; contracts.md, Repo
//! artifacts; DR-65): `docs/evidence/spend.jsonl` in the main checkout, which `cargo xtask` names
//! by one absolute path for every worktree.
//!
//! Before a live call is sent, the engine takes an exclusive lock on `spend.jsonl.lock` beside
//! the log, sums every row's `cost_usd`, refuses the call if the sum plus the call's ceiling would
//! pass `live_tests_total_usd` less a $0.25 margin, and appends a `reserved` row for that ceiling,
//! all under the one lock, so concurrent callers serialize and no call is sent unchecked. When the
//! reply reports its cost, a `settled` row adds the difference, so the rows still sum to what was
//! spent; a call that reports none keeps its ceiling.

use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{ModelError, round_usd};
use crate::gate::SPEND_LOG;

/// What the cap keeps back, in dollars (DR-65).
pub const MARGIN_USD: f64 = 0.25;
/// The guard `budget_exceeded` names when the spend log stops a call.
pub const GUARD: &str = "live_tests_total_usd";

/// One line of the spend log. Fields a row has beyond these, such as a seed row's `note`, are
/// read past.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Row {
    /// RFC 3339, UTC.
    pub at: String,
    pub generation_id: Option<String>,
    /// `jev`, `gateway`, `typesafe` or `sample`.
    pub provider: String,
    pub model: String,
    pub cost_usd: f64,
    pub scenario: String,
    /// `reserved` before the call, with its ceiling; `settled` after, with the difference to
    /// what the provider reported. Rows written before the engine appended live calls have none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub phase: Option<String>,
}

/// The spend log at one absolute path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpendLog {
    path: PathBuf,
}

impl SpendLog {
    /// The log at `path`, which must be absolute and must exist: a missing file would start the
    /// total again at zero.
    pub fn open(path: &Path) -> Result<SpendLog, ModelError> {
        if !path.is_absolute() {
            return Err(ModelError::invalid_argument(SPEND_LOG, "spend_log_relative"));
        }
        if !path.is_file() {
            return Err(ModelError::invalid_argument(SPEND_LOG, "spend_log_missing"));
        }
        Ok(SpendLog { path: path.to_path_buf() })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    /// The sum of every row's `cost_usd`, read under the lock.
    pub fn total(&self) -> Result<f64, ModelError> {
        let _lock = self.lock()?;
        sum(&self.read()?)
    }

    /// What `cap_usd` less the margin leaves, read under the lock.
    pub fn remaining(&self, cap_usd: f64) -> Result<f64, ModelError> {
        Ok(cap_usd - MARGIN_USD - self.total()?)
    }

    /// Under the lock: sum the log, refuse `row` if its `cost_usd` (the call's ceiling) would take
    /// the total past `cap_usd` less the margin, else append it. Returns the total before it.
    pub fn reserve(&self, cap_usd: f64, row: &Row) -> Result<f64, ModelError> {
        let cost = round_usd(row.cost_usd);
        if !cost.is_finite() || cost <= 0.0 || !cap_usd.is_finite() {
            return Err(ModelError::internal());
        }
        let _lock = self.lock()?;
        let text = self.read()?;
        let total = sum(&text)?;
        let limit = cap_usd - MARGIN_USD;
        if total + cost > limit + 1e-9 {
            return Err(ModelError::budget_exceeded(GUARD, limit, total));
        }
        self.append_locked(&text, row)?;
        Ok(total)
    }

    /// Append `row` under the lock, with no cap check: a settling row after a call.
    pub fn append(&self, row: &Row) -> Result<(), ModelError> {
        let _lock = self.lock()?;
        let text = self.read()?;
        self.append_locked(&text, row)
    }

    fn lock(&self) -> Result<Lock, ModelError> {
        let mut name = self.path.file_name().unwrap_or_default().to_os_string();
        name.push(".lock");
        let file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(self.path.with_file_name(name))
            .map_err(|_| ModelError::internal())?;
        file.lock().map_err(|_| ModelError::internal())?;
        Ok(Lock(file))
    }

    fn read(&self) -> Result<String, ModelError> {
        let mut text = String::new();
        File::open(&self.path)
            .and_then(|mut f| f.read_to_string(&mut text))
            .map_err(|_| ModelError::invalid_argument(SPEND_LOG, "spend_log_missing"))?;
        Ok(text)
    }

    fn append_locked(&self, text: &str, row: &Row) -> Result<(), ModelError> {
        let mut line = String::new();
        if !text.is_empty() && !text.ends_with('\n') {
            line.push('\n');
        }
        // Every row holds whole millionths of a dollar, so a difference such as a settling row's
        // never carries a float's remainder into the committed log.
        let row = Row { cost_usd: round_usd(row.cost_usd), ..row.clone() };
        line.push_str(&serde_json::to_string(&row).map_err(|_| ModelError::internal())?);
        line.push('\n');
        let mut file =
            OpenOptions::new().append(true).open(&self.path).map_err(|_| ModelError::internal())?;
        file.write_all(line.as_bytes()).and_then(|()| file.sync_data()).map_err(|_| ModelError::internal())
    }
}

/// The exclusive lock on `spend.jsonl.lock`, released when dropped.
struct Lock(File);

impl Drop for Lock {
    fn drop(&mut self) {
        let _ = self.0.unlock();
    }
}

/// The sum of every row's `cost_usd`. A line that isn't a row with a finite cost stops the sum:
/// a log the engine can't add up can't hold a cap.
fn sum(text: &str) -> Result<f64, ModelError> {
    #[derive(Deserialize)]
    struct Cost {
        cost_usd: f64,
    }
    let mut total = 0.0;
    for line in text.lines().filter(|l| !l.trim().is_empty()) {
        let cost: Cost = serde_json::from_str(line)
            .map_err(|_| ModelError::invalid_argument(SPEND_LOG, "spend_log_unreadable"))?;
        if !cost.cost_usd.is_finite() {
            return Err(ModelError::invalid_argument(SPEND_LOG, "spend_log_unreadable"));
        }
        total += cost.cost_usd;
    }
    Ok(round_usd(total))
}
