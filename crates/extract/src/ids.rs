//! IDs and hashes that extractors derive.

use planroom_contract::{EdgeId, Evidence, Kind, NodeId, Ref};
use sha2::{Digest, Sha256};

/// `<namespace>:<kind-short>:<key>`.
pub fn node_id(namespace: &str, prefix: &str, key: &str) -> NodeId {
    NodeId(format!("{namespace}:{prefix}:{key}"))
}

/// 12 hex characters of `sha256(kind ‖ scope ‖ key ‖ evidence)`, each part ended by a NUL so no
/// two inputs run together.
pub fn hint_id(kind: &Kind, scope: &NodeId, key: &str, evidence: &[Evidence]) -> String {
    hint_id_of(kind, scope, &[key], evidence)
}

/// [`hint_id`] for an OpenAPI operation, whose method follows its key, so each operation of one
/// path item is its own hint even when they share a line: `sha256(kind ‖ scope ‖ key ‖ method ‖
/// evidence)`.
pub fn operation_id(kind: &Kind, scope: &NodeId, key: &str, method: &str, evidence: &[Evidence]) -> String {
    hint_id_of(kind, scope, &[key, method], evidence)
}

fn hint_id_of(kind: &Kind, scope: &NodeId, keys: &[&str], evidence: &[Evidence]) -> String {
    let mut hasher = Sha256::new();
    for part in [kind.0.as_str(), scope.0.as_str()].iter().chain(keys) {
        hasher.update(part.as_bytes());
        hasher.update([0]);
    }
    hasher.update(serde_json::to_string(evidence).expect("evidence serializes").as_bytes());
    hex(&hasher.finalize())[..12].to_owned()
}

/// `e:` and 12 hex characters of `sha256(kind ‖ from ‖ to ‖ via)`, each part ended by a NUL: one
/// ID per relation between two ends, whatever lines cite it, so a rescan keeps it. Every extracted
/// edge ID is made this way; an edge with no `via` hashes an empty one.
pub fn edge_id(kind: &Kind, from: &Ref, to: &Ref, via: Option<&str>) -> EdgeId {
    let end = |r: &Ref| match r {
        Ref::Id(id) => format!("id:{}", id.0),
        Ref::Alias(alias) => format!("alias:{alias}"),
    };
    let mut hasher = Sha256::new();
    for part in [kind.0.clone(), end(from), end(to), via.unwrap_or_default().to_owned()] {
        hasher.update(part.as_bytes());
        hasher.update([0]);
    }
    EdgeId(format!("e:{}", &hex(&hasher.finalize())[..12]))
}

/// Lowercase hex. Hand-rolled: two lines, where a crate would add a dependency.
pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_hint_id_is_12_hex_and_depends_on_every_part() {
        let kind = Kind("code.env_set".into());
        let scope = node_id("ws", "container", "redis");
        let ev = vec![Evidence::Ref { commit: "c".into(), ref_name: None }];
        let id = hint_id(&kind, &scope, "A", &ev);
        assert_eq!(id.len(), 12);
        assert!(id.chars().all(|c| c.is_ascii_hexdigit()));
        assert_ne!(id, hint_id(&kind, &scope, "B", &ev));
        assert_ne!(
            hint_id(&kind, &scope, "ab", &ev),
            hint_id(&kind, &node_id("ws", "container", "redisa"), "b", &ev)
        );
    }

    #[test]
    fn an_operations_id_holds_its_method() {
        let kind = Kind("code.route_serve".into());
        let scope = node_id("r", "spec", "openapi.yaml");
        let ev = vec![Evidence::Ref { commit: "c".into(), ref_name: None }];
        let get = operation_id(&kind, &scope, "/a", "GET", &ev);
        assert_ne!(get, operation_id(&kind, &scope, "/a", "POST", &ev));
        assert_ne!(get, hint_id(&kind, &scope, "/a", &ev));
        assert_ne!(operation_id(&kind, &scope, "/a", "", &ev), hint_id(&kind, &scope, "/a", &ev));
    }
}
