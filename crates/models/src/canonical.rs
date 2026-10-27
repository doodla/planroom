//! Canonical JSON: object keys sorted at every depth and no whitespace, so the same value always
//! gives the same bytes, whatever order its keys were inserted in. Replay keys and the cache
//! prefix hash are taken over it.

use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

/// `value` with every object's keys in sorted order.
pub fn sorted(value: &Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut keys: Vec<&String> = map.keys().collect();
            keys.sort();
            let mut out = Map::new();
            for k in keys {
                out.insert(k.clone(), sorted(&map[k]));
            }
            Value::Object(out)
        }
        Value::Array(items) => Value::Array(items.iter().map(sorted).collect()),
        other => other.clone(),
    }
}

/// The canonical bytes of `value`.
pub fn bytes(value: &Value) -> Vec<u8> {
    serde_json::to_vec(&sorted(value)).expect("a JSON value always serializes")
}

/// Lowercase hex of the sha256 of `parts`, in order.
pub fn sha256_hex(parts: &[&[u8]]) -> String {
    let mut hasher = Sha256::new();
    for part in parts {
        hasher.update(part);
    }
    hasher.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn key_order_never_changes_the_bytes() {
        let a: Value = serde_json::from_str(r#"{"b":1,"a":{"d":[{"z":1,"y":2}],"c":"x"}}"#).unwrap();
        let b = json!({"a": {"c": "x", "d": [{"y": 2, "z": 1}]}, "b": 1});
        assert_eq!(bytes(&a), bytes(&b));
        assert_eq!(String::from_utf8(bytes(&a)).unwrap(), r#"{"a":{"c":"x","d":[{"y":2,"z":1}]},"b":1}"#);
    }
}
