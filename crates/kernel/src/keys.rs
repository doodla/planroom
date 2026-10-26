//! Idempotency keys and content hashes (DR-8): pure functions of their inputs.

use planroom_contract::Seq;
use serde_json::Value;
use sha2::{Digest, Sha256};

/// sha256 of `parts`, each ended by a NUL so no two inputs run together, as lowercase hex.
pub fn sha256_hex(parts: &[&str]) -> String {
    let mut hasher = Sha256::new();
    for part in parts {
        hasher.update(part.as_bytes());
        hasher.update([0]);
    }
    hasher.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

/// sha256 of `bytes` as lowercase hex, with nothing added: for a hash another tool recomputes,
/// such as the catalogue hash.
pub fn sha256_plain_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

/// JSON with every object's keys sorted, so equal values hash equally.
pub fn canonical(value: &Value) -> String {
    fn sort(value: &Value) -> Value {
        match value {
            Value::Object(map) => {
                let mut entries: Vec<(&String, &Value)> = map.iter().collect();
                entries.sort_by(|a, b| a.0.cmp(b.0));
                Value::Object(entries.into_iter().map(|(k, v)| (k.clone(), sort(v))).collect())
            }
            Value::Array(items) => Value::Array(items.iter().map(sort).collect()),
            other => other.clone(),
        }
    }
    sort(value).to_string()
}

/// The key derived for a command that carries `base`, whose base pins the intent:
/// `sha256(actor ‖ command ‖ canonical(args) ‖ base)`. Every other command mints a fresh key.
pub fn derived_idem(actor: &str, command: &str, args: &Value, base: Seq) -> String {
    format!("drv:{}", sha256_hex(&[actor, command, &canonical(args), &base.to_string()]))
}

/// The request hash: sha256 of the canonical request.
pub fn request_hash(command: &str, args: &Value) -> String {
    sha256_hex(&[command, &canonical(args)])
}

/// A repo's `scan` event key and request hash, both over
/// `repo ‖ commit ‖ packs_hash ‖ previous active snapshot` (DR-8).
pub fn scan_key(repo: &str, commit: &str, packs_hash: &str, previous: Option<u64>) -> (String, String) {
    let previous = previous.map_or_else(|| "none".to_owned(), |p| p.to_string());
    let parts = [repo, commit, packs_hash, previous.as_str()];
    (format!("scan:{}", sha256_hex(&parts)), sha256_hex(&parts))
}

/// A link snapshot's `commit_id`: the sha256 of its sorted member snapshot IDs and the packs hash.
pub fn link_key(members: &[u64], packs_hash: &str) -> String {
    let mut sorted = members.to_vec();
    sorted.sort_unstable();
    let ids: Vec<String> = sorted.iter().map(u64::to_string).collect();
    sha256_hex(&[&ids.join(","), packs_hash])
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn canonical_json_sorts_keys_at_every_depth() {
        assert_eq!(
            canonical(&json!({"b": 1, "a": {"d": 2, "c": [ {"f": 1, "e": 0} ]}})),
            r#"{"a":{"c":[{"e":0,"f":1}],"d":2},"b":1}"#
        );
    }

    #[test]
    fn a_derived_key_depends_on_the_base() {
        let args = json!({"target": "x"});
        assert_eq!(
            derived_idem("agent:a:chat", "edit", &args, 3),
            derived_idem("agent:a:chat", "edit", &args, 3)
        );
        assert_ne!(
            derived_idem("agent:a:chat", "edit", &args, 3),
            derived_idem("agent:a:chat", "edit", &args, 4)
        );
    }

    #[test]
    fn a_scan_key_depends_on_the_previous_active_snapshot() {
        let first = scan_key("orders", "c1", "p", None);
        assert_eq!(first, scan_key("orders", "c1", "p", None));
        assert_ne!(first.0, scan_key("orders", "c1", "p", Some(1)).0);
        assert_ne!(first.0, scan_key("orders", "c2", "p", None).0);
    }

    #[test]
    fn a_link_key_ignores_member_order() {
        assert_eq!(link_key(&[3, 1, 2], "p"), link_key(&[1, 2, 3], "p"));
        assert_ne!(link_key(&[1, 2], "p"), link_key(&[1, 2], "q"));
    }
}
