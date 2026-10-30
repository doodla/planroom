//! The engine's side of the model boundary: it reads the live gate before any command that may
//! call a model, builds a request's uncached tail with its repo text masked, and turns a model
//! error into an envelope error with the engine's own line.
//!
//! The gate's rules live in `planroom-models` (architecture.md, Spend guards; DR-65, DR-89): a
//! relative `PLANROOM_SPEND_LOG`, or a `PLANROOM_LIVE_*` flag with no spend log, is
//! `invalid_argument` before the command does anything.

use std::path::{Path, PathBuf};
use std::time::Duration;

use planroom_contract::ErrorCode;
use planroom_contract::config::Config;
use planroom_kernel::redact;
use planroom_models::claude::{self, Call, Claude, REPLAY_PROVIDER, Reply};
use planroom_models::gate::{self, LiveEnv, Mode, Provider};
use planroom_models::replay::ReplayDir;
use planroom_models::transport::Live;
use planroom_models::{ModelError, canonical};
use planroom_store::{Ledger, LedgerRow};
use serde::Serialize;

use crate::app::error::AppError;
use crate::text;

/// The live gate, read from this process's environment.
pub fn live_env() -> Result<LiveEnv, AppError> {
    LiveEnv::from_process().map_err(AppError::from)
}

/// A model request's uncached tail, the two blocks `planroom_models::claude::Call::tail` holds:
/// first the slice, masked by its own type's marks and written as canonical JSON, then the
/// question, masked as an engine line built from names, like `Envelope.summary` (contracts.md, the
/// redaction paragraph; DR-49, DR-64). Every model request's tail is built here.
///
/// The slice goes inside a fence, `<repo-data id="r…">` … `</repo-data>`, that the prompt prefix
/// names as data, never instructions (architecture.md, the prompt injection row). Its ID is `r` and
/// the first 16 hex characters of the sha256 of the masked canonical JSON, so the same slice always
/// gives the same bytes and its replay key holds. Every `<` in the JSON is written `\u003c`, which
/// it can only hold inside a string, so the JSON stays valid and no text in it closes the fence.
///
/// It takes no raw flag and reads no environment, so a model request stays redacted whatever
/// `PLANROOM_RAW` says: `cli::raw_for` stays that variable's one reader, and the variable reaches
/// only a read command's `--json` stdout (DR-75).
///
/// A slice whose type `kernel::redact` doesn't list, such as a `Vec<Node>` or a wrapper of the
/// app's own, would pass through unmasked, and so would what a listed type's open value holds,
/// such as an `Envelope`'s or a `ToolResult`'s `result`; so `redact::is_listed` refuses each, and
/// the slice is refused with `internal`. A slice is always a contract type whose marks reach every
/// string it holds, such as `Node`, `MapView` or `FactBatch`.
pub fn request_tail<T: Serialize>(slice: &T, question: &str) -> Result<Vec<String>, AppError> {
    if !redact::is_listed::<T>() {
        let name = std::any::type_name::<T>();
        return Err(AppError::internal(format!(
            "a model request's slice of type {name} has no redaction entry"
        )));
    }
    let mut json = serde_json::to_value(slice).map_err(AppError::internal)?;
    redact::value_of::<T>(&mut json);
    let slice = String::from_utf8(canonical::bytes(&json)).map_err(AppError::internal)?;
    let id = &planroom_kernel::keys::sha256_plain_hex(slice.as_bytes())[..FENCE_HEX];
    let fenced = format!("<repo-data id=\"r{id}\">{}</repo-data>", slice.replace('<', "\\u003c"));
    Ok(vec![fenced, redact::text(question)])
}

/// Hex characters of the slice's hash a fence's ID keeps.
const FENCE_HEX: usize = 16;

/// How long one Claude call may take before the transport gives up.
const CALL_TIMEOUT: Duration = Duration::from_secs(120);
/// Where a missing record would live, as `replay_missing` names it when no
/// `PLANROOM_REPLAY_DIR` is set: the checkout's `tests/replay/`.
const REPLAY_HOME: &str = "tests/replay";

/// Who pays for a model call and where its ledger row goes: the row's workspace, the store's
/// `meta.workspace_id`, and the data root that holds `ledger.db`.
pub struct Spender {
    pub data_root: PathBuf,
    pub workspace_id: Option<String>,
}

/// Ask Claude `call` through the gateway adapter, the one place the engine assembles it: the
/// configuration's `[claude]` settings and `live_tests_total_usd`, the credential and the values no
/// record may hold from this process's environment, the recordings under `PLANROOM_REPLAY_DIR`,
/// and the live transport. The gate in `env` decides between a recording, a recorded live call
/// and a refusal (DR-49, DR-65, DR-89).
///
/// Under strict replay or record mode with no `PLANROOM_REPLAY_DIR`, the call is `replay_missing`
/// naming where its record would live, and nothing is read: recordings are never looked for
/// relative to the directory the command runs in, where a workspace could plant one. A live call
/// outside record mode is the gate's `budget_exceeded`, guard `replay_only`, before anything else,
/// whatever `PLANROOM_REPLAY_DIR` is, so it never opens the ledger.
///
/// The ledger (`DATA_ROOT/ledger.db`) opens after the gate's refusals and before the credential is
/// read or the call made, so a ledger that refuses this engine is `schema_mismatch` naming it
/// before anything is sent. Every answered call, replayed ones included, writes one row: `day`
/// scope, the built model and `prefix_hash`, the reply's usage, and `cost_usd` NULL when replayed,
/// else the reported cost or the call's ceiling (architecture.md, Spend guards; DR-51, DR-78). A
/// refusal or a failed call writes none.
pub fn ask_claude(call: &Call, env: &LiveEnv, config: &Config, ledger: &Spender) -> Result<Reply, AppError> {
    if env.mode == Mode::Live {
        // Outside record mode the gate refuses every live call.
        return Err(env.live_call(Provider::Gateway).err().unwrap_or_else(ModelError::internal).into());
    }
    let replay = match &env.replay_dir {
        Some(dir) => ReplayDir::new(dir),
        None => {
            let built = claude::build(&config.claude, call);
            let path =
                ReplayDir::new(Path::new(REPLAY_HOME)).path(REPLAY_PROVIDER, &call.purpose, &built.key)?;
            return Err(ModelError::replay_missing(&built.key, &path.display().to_string()).into());
        }
    };
    let mut book = Ledger::open_at(&ledger.data_root, env!("CARGO_PKG_VERSION"))?;
    let var = |name: &str| std::env::var(name).ok();
    let credential = claude::credential(&config.claude, &var)?;
    let secrets = claude::secrets(credential.as_ref(), &var);
    let transport = Live::new(CALL_TIMEOUT)?;
    let adapter = Claude {
        config: &config.claude,
        cap_usd: config.budget.live_tests_total_usd,
        env,
        replay: &replay,
        transport: &transport,
        credential,
        secrets,
    };
    let reply = adapter.call(call)?;
    let built = claude::build(&config.claude, call);
    book.record(&LedgerRow {
        at: claude::now_rfc3339(),
        workspace_id: ledger.workspace_id.clone(),
        provider: Provider::Gateway.as_str().to_owned(),
        model: built.model,
        purpose: call.purpose.clone(),
        scope: "day".to_owned(),
        uncached_in: Some(count(reply.usage.input)?),
        cache_read: Some(count(reply.usage.cache_read)?),
        cache_write: Some(count(reply.usage.cache_write)?),
        output: Some(count(reply.usage.output)?),
        cost_usd: if reply.replayed { None } else { reply.cost_usd.or(Some(call.ceiling_usd)) },
        prefix_hash: Some(built.prefix_hash),
        generation_id: reply.generation_id.clone(),
        replayed: reply.replayed,
    })?;
    Ok(reply)
}

/// A token count as the ledger's integer column holds it.
fn count(n: u64) -> Result<i64, AppError> {
    i64::try_from(n).map_err(|_| AppError::internal("a token count past i64"))
}

impl From<ModelError> for AppError {
    fn from(e: ModelError) -> AppError {
        let d = &e.details;
        let field = d["field"].as_str().unwrap_or_default();
        match e.code {
            ErrorCode::InvalidArgument => {
                let reason = e.reason.unwrap_or("model_setting");
                AppError::invalid_argument(field, &format!("argument.{reason}"))
                    .with_hint(text::get(&format!("hint.model_{reason}"), &[("field", &field)]))
            }
            ErrorCode::BudgetExceeded if d["guard"] == gate::REPLAY_ONLY => AppError::new(e.code, &[])
                .details(e.details.clone())
                .with_message(text::get("models.replay_only", &[])),
            ErrorCode::BudgetExceeded => {
                let (guard, limit, spent) = (d["guard"].clone(), d["limit"].clone(), d["spent"].clone());
                let (guard, limit, spent) =
                    (guard.as_str().unwrap_or_default(), dollars(&limit), dollars(&spent));
                AppError::new(e.code, &[("guard", &guard), ("limit", &limit), ("spent", &spent)])
                    .details(e.details.clone())
            }
            ErrorCode::ReplayMissing => {
                let key = d["key"].as_str().unwrap_or_default();
                AppError::new(e.code, &[("key", &key)]).details(e.details.clone())
            }
            ErrorCode::ModelUnavailable => {
                let provider = d["provider"].as_str().unwrap_or_default();
                AppError::new(e.code, &[("provider", &provider)]).details(e.details.clone())
            }
            ErrorCode::Internal => AppError::internal(text::get("models.internal", &[])),
            code => AppError::new(code, &[]).details(e.details.clone()),
        }
    }
}

/// `$0.25` from a JSON number of dollars.
fn dollars(v: &serde_json::Value) -> String {
    format!("${:.2}", v.as_f64().unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use planroom_contract::{
        CacheTtl, Measured, ModelTier, REPLAY_FORMAT, RecordedRequest, RecordedResponse, ReplayRecord,
    };
    use planroom_store::LEDGER_MIGRATIONS;
    use planroom_store::migrate::Migration;
    use serde_json::json;

    use super::*;

    fn config() -> Config {
        let env = |_: &str| None;
        crate::config::load(&crate::config::Sources { user: None, workspace: None, env: &env })
            .unwrap()
            .config
    }

    fn call() -> Call {
        Call {
            purpose: "test.ledger".into(),
            tier: ModelTier::Cheap,
            prefix: vec!["You answer with an empty JSON object.".into()],
            ttl: CacheTtl::H1,
            tail: vec!["{}".into()],
            output_schema: None,
            max_tokens: 64,
            effort: None,
            ceiling_usd: 0.10,
            scenario: "test".into(),
        }
    }

    /// Plant `call`'s recording under `dir`, answering `{}` with a usage and a reported cost.
    fn plant(dir: &Path, config: &Config, call: &Call) {
        let built = claude::build(&config.claude, call);
        let body = json!({
            "type": "message",
            "id": "gen-1",
            "model": built.model,
            "content": [{"type": "text", "text": "{}"}],
            "stop_reason": "end_turn",
            "usage": {
                "input_tokens": 11,
                "cache_creation_input_tokens": 7,
                "cache_read_input_tokens": 5,
                "output_tokens": 3
            },
            "provider_metadata": {"gateway": {"cost": 0.0123}}
        });
        let record = ReplayRecord {
            format: REPLAY_FORMAT.into(),
            key: built.key,
            purpose: call.purpose.clone(),
            request: RecordedRequest {
                method: "POST".into(),
                url: built.url,
                headers: built.headers,
                body: built.body,
            },
            response: RecordedResponse { status: 200, headers: vec![], body },
            measured: Measured {
                wall_ms: 1,
                recorded_at: "2026-10-01T00:00:00Z".into(),
                model_reported: None,
            },
        };
        ReplayDir::new(dir).write(REPLAY_PROVIDER, &record, &[]).unwrap();
    }

    fn strict(dir: &Path) -> LiveEnv {
        LiveEnv {
            mode: Mode::Strict,
            gateway: false,
            jev: false,
            spend_log: None,
            replay_dir: Some(dir.to_path_buf()),
        }
    }

    #[test]
    fn a_replayed_call_is_recorded_in_the_ledger_at_no_cost() {
        let tmp = tempfile::tempdir().unwrap();
        let (replay, data) = (tmp.path().join("replay"), tmp.path().join("data"));
        let (config, call) = (config(), call());
        plant(&replay, &config, &call);
        let spender = Spender { data_root: data.clone(), workspace_id: Some("ws-test".into()) };
        let reply = ask_claude(&call, &strict(&replay), &config, &spender).unwrap();
        assert_eq!(reply.text, "{}");
        let rows = Ledger::open_at(&data, env!("CARGO_PKG_VERSION")).unwrap().rows(0, 10).unwrap();
        assert_eq!(rows.len(), 1, "one answered call, one row");
        let row = &rows[0].1;
        let built = claude::build(&config.claude, &call);
        assert!(row.replayed);
        assert_eq!(row.cost_usd, None, "a replayed answer costs nothing, whatever its recording reported");
        assert_eq!(row.prefix_hash, Some(built.prefix_hash));
        assert_eq!(row.model, built.model);
        assert_eq!(row.scope, "day");
        assert_eq!(row.provider, "gateway");
        assert_eq!(row.purpose, "test.ledger");
        assert_eq!(row.workspace_id.as_deref(), Some("ws-test"));
        assert!(row.at.len() == 20 && row.at.ends_with('Z'), "{}", row.at);
        assert_eq!(
            (row.uncached_in, row.cache_write, row.cache_read, row.output),
            (Some(11), Some(7), Some(5), Some(3))
        );
        assert_eq!(row.generation_id.as_deref(), Some("gen-1"));
    }

    #[test]
    fn a_newer_ledger_refuses_the_call_before_it_is_sent() {
        const RAISED: &[Migration] =
            &[LEDGER_MIGRATIONS[0], (2, "UPDATE meta SET value = '9.0.0' WHERE key = 'min_reader_version'")];
        let tmp = tempfile::tempdir().unwrap();
        let (replay, data) = (tmp.path().join("replay"), tmp.path().join("data"));
        drop(Ledger::open_with_migrations(&data, "9.0.0", RAISED).unwrap());
        // No recording is planted: a call made before the ledger refuses would be `replay_missing`,
        // so only a ledger that refuses first answers `schema_mismatch`.
        let (config, call) = (config(), call());
        let spender = Spender { data_root: data.clone(), workspace_id: Some("ws-test".into()) };
        let Err(e) = ask_claude(&call, &strict(&replay), &config, &spender) else {
            panic!("a ledger this engine may not read answered the call");
        };
        assert_eq!(e.code, ErrorCode::SchemaMismatch);
        let file = e.details["file"].as_str().unwrap_or_default();
        assert!(file.ends_with("ledger.db"), "{}", e.details);
        let rows = Ledger::open_with_migrations(&data, "9.0.0", RAISED).unwrap().rows(0, 10).unwrap();
        assert!(rows.is_empty(), "a refused call writes no row");
    }

    #[test]
    fn a_live_call_is_refused_before_the_ledger_opens() {
        let tmp = tempfile::tempdir().unwrap();
        let (replay, data) = (tmp.path().join("replay"), tmp.path().join("data"));
        let (config, call) = (config(), call());
        plant(&replay, &config, &call);
        let live = LiveEnv { mode: Mode::Live, ..strict(&replay) };
        let spender = Spender { data_root: data.clone(), workspace_id: Some("ws-test".into()) };
        let Err(e) = ask_claude(&call, &live, &config, &spender) else {
            panic!("a live call outside record mode was answered");
        };
        assert_eq!(e.code, ErrorCode::BudgetExceeded);
        assert_eq!(e.details["guard"], "replay_only", "{}", e.details);
        assert!(!data.exists(), "the refusal opened the ledger under {}", data.display());
    }
}
