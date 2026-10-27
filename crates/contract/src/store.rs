//! `store.pointer` (architecture.md, Where the workspace lives; DR-1): what the state directory
//! holds in place of `store.db` when its filesystem can't share a write-ahead log between
//! processes, so the store lives under the data root instead.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::format::{Document, Extra};

/// `store.pointer`'s file name in the state directory.
pub const STORE_POINTER_FILE: &str = "store.pointer";

/// `planroom.store-pointer/v1`: where a workspace's store lives when it isn't in the state
/// directory. The engine follows it only to `DATA_ROOT/stores/<workspace_id>/store.db`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct StorePointer {
    /// `planroom.store-pointer/v1`.
    pub format: String,
    /// The store file's absolute path: `DATA_ROOT/stores/<workspace_id>/store.db`.
    #[schemars(extend("x-redact" = true))]
    pub path: String,
    /// The workspace the store belongs to, which its `meta.workspace_id` also holds.
    pub workspace_id: String,
    /// Fields a later version added, kept and written back unchanged.
    #[serde(flatten, default, skip_serializing_if = "Extra::is_empty")]
    pub extra: Extra,
}

impl Document for StorePointer {
    const NAME: &'static str = "store-pointer";
}

impl StorePointer {
    /// A pointer this version writes.
    pub fn new(path: String, workspace_id: String) -> StorePointer {
        StorePointer { format: StorePointer::format_tag(), path, workspace_id, extra: Extra::new() }
    }
}
