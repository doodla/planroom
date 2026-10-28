//! `planroom person init [--rekey]`: register the mod's public key for this workspace (DR-60,
//! DR-72, DR-75). The key arrives on stdin; the store keeps only it, never anything that signs.
//!
//! - No key registered: the call's token must verify under the key on stdin.
//! - The same key registered: a token under it; nothing changes.
//! - Another key registered: `person_key_mismatch`, unless `--rekey`, which takes a token under
//!   the old key, or under the new one, which is the mismatch pane's button press.

use std::io::Read;
use std::path::Path;

use planroom_contract::{Command, ErrorCode, PersonInitReq, PersonKey, Surface, Timestamp};
use planroom_store::Store;
use serde_json::json;

use crate::app::actor::{self, Bound, Caller, Refused, Terminal};
use crate::app::error::AppError;
use crate::app::{Reply, workspace};
use crate::{clock, text};

const KEY: &str = "person_pubkey";
const AT: &str = "person_pubkey_registered_at";
const SURFACE: &str = "person_pubkey_surface";

pub fn run(
    req: &PersonInitReq,
    cwd: &Path,
    caller: &Caller,
    stdin: &mut dyn Read,
    terminal: &mut dyn Terminal,
) -> Result<Reply<PersonKey>, AppError> {
    let spec = Command::PersonInit(req.clone()).spec();
    let command = spec.name();
    let root = workspace::find_root(cwd).ok_or_else(|| AppError::workspace_not_found(cwd))?;
    let mut store = Store::open_existing(&workspace::store_path(&root))?
        .ok_or_else(|| AppError::workspace_not_found(&root))?;
    let mut submitted = String::new();
    stdin.read_to_string(&mut submitted).map_err(AppError::internal)?;
    let submitted = submitted.trim().to_owned();
    let new_key = actor::parse_key(&submitted).ok_or_else(|| {
        AppError::new(ErrorCode::InvalidArgument, &[("what", &text::get("argument.person_key", &[]))])
            .details(json!({"field": "stdin"}))
    })?;
    let registered = store.meta(KEY)?;
    let old_key = registered.as_deref().and_then(actor::parse_key);
    let mut allowed = vec![&new_key];
    allowed.extend(old_key.as_ref());
    let now = clock::unix_micros() / 1_000_000;
    let bound = Bound { idem: req.common.idem.as_deref(), base: req.common.base };
    // Never from a terminal confirmation: a first registration proves it holds the key on stdin,
    // and a rekey takes a token under the old key or the mismatch pane's press (DR-75).
    let path = actor::terminal_path(&command, registered.is_some());
    let acting = actor::person(caller, spec, &bound, &allowed, now, path, terminal)?;
    let digest = acting.digest.ok_or_else(|| actor::person_required(&command, Refused::Missing, path))?;

    let same = registered.as_deref() == Some(submitted.as_str());
    if registered.is_some() && !same && !req.rekey {
        return Err(mismatch(&store)?);
    }
    actor::spend(&mut store, &digest, now, &command, path)?;
    let surface = caller.client().surface;
    let rekeyed = registered.is_some() && !same;
    if !same {
        let at = clock::now();
        let surface_name = surface_str(surface);
        let swapped = store.meta_swap(
            KEY,
            registered.as_deref(),
            &submitted,
            &[(AT, &at.0), (SURFACE, &surface_name)],
        )?;
        if !swapped {
            // Another call registered a key between the read and the write.
            return Err(mismatch(&store)?);
        }
    }
    // The data root pins the key for this root, while the workspace is trusted (DR-88).
    actor::pin_registered(&store, &root)?;
    let registered_at = Timestamp(store.meta(AT)?.unwrap_or_default());
    let surface: Surface = store
        .meta(SURFACE)?
        .and_then(|s| serde_json::from_value(serde_json::Value::String(s)).ok())
        .unwrap_or(surface);
    let fingerprint = actor::fingerprint(&submitted);
    let key = match (same, rekeyed) {
        (true, _) => "person.already",
        (false, true) => "person.rekeyed",
        (false, false) => "person.registered",
    };
    let summary = text::get(key, &[("fingerprint", &fingerprint)]);
    let seq = store.seq()?;
    let result = PersonKey { summary: summary.clone(), fingerprint, registered_at, surface, rekeyed };
    Ok(Reply { result, seq, warnings: vec![], text: summary })
}

/// `person_key_mismatch`, naming when and from where the registered key came.
fn mismatch(store: &Store) -> Result<AppError, AppError> {
    let at = store.meta(AT)?.unwrap_or_default();
    let surface = store.meta(SURFACE)?.unwrap_or_default();
    Ok(AppError::new(ErrorCode::PersonKeyMismatch, &[("registered_at", &at)])
        .details(json!({"registered_at": at, "surface": surface})))
}

fn surface_str(surface: Surface) -> String {
    serde_json::to_value(surface).ok().and_then(|v| v.as_str().map(str::to_owned)).unwrap_or_default()
}
