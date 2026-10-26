//! Redaction (architecture.md, "What leaves the machine"; DR-64): masks secrets in the free text
//! that comes from a repo before it reaches any output. It masks DSN user info, the values of
//! variables whose names end in `_KEY`, `_SECRET`, `_TOKEN` or `_PASSWORD`, private-key blocks,
//! well-known token shapes and high-entropy strings. It is deterministic.
//!
//! It masks by field (DR-89): exactly the fields the contract marks
//! `#[schemars(extend("x-redact" = true))]`, through the paths `cargo xtask gen` writes into
//! [`redact_fields`](crate::redact_fields). An ID, a handle, a cursor or a hash is never marked, so
//! it is never touched.

use std::sync::OnceLock;

use planroom_contract::{Evidence, MapView, PaneModel, PresenceRow};
use regex::Regex;
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::redact_fields::PATHS;

/// What a masked value becomes.
pub const MASK: &str = "[redacted]";

struct Rules {
    dsn: Regex,
    private_key: Regex,
    assignment: Regex,
    known: Regex,
    candidate: Regex,
}

fn rules() -> &'static Rules {
    static RULES: OnceLock<Rules> = OnceLock::new();
    RULES.get_or_init(|| Rules {
        // scheme://user:password@host
        dsn: Regex::new(r"([A-Za-z][A-Za-z0-9+.\-]*://)[^\s/:@]+:[^\s/@]+@").expect("a valid pattern"),
        private_key: Regex::new(r"-----BEGIN [A-Z0-9 ]*PRIVATE KEY-----(?s:.*?)(?:-----END [A-Z0-9 ]*PRIVATE KEY-----|\z)")
            .expect("a valid pattern"),
        // NAME_KEY=value, NAME_SECRET: "value", api_token = value
        assignment: Regex::new(
            r#"(?i)\b([A-Z0-9_]*(?:_KEY|_SECRET|_TOKEN|_PASSWORD|PASSWD|_PASS))(["']?\s*[:=]\s*["']?)([^\s"',;]+)"#,
        )
        .expect("a valid pattern"),
        known: Regex::new(
            r"\b(?:AKIA[0-9A-Z]{16}|gh[pousr]_[A-Za-z0-9]{30,}|github_pat_[A-Za-z0-9_]{30,}|sk-[A-Za-z0-9_\-]{20,}|xox[abprs]-[A-Za-z0-9\-]{10,})\b",
        )
        .expect("a valid pattern"),
        candidate: Regex::new(r"[A-Za-z0-9+/=]{20,}").expect("a valid pattern"),
    })
}

/// `s` with every secret it holds masked.
pub fn text(s: &str) -> String {
    let r = rules();
    let out = r.private_key.replace_all(s, MASK);
    let out = r.dsn.replace_all(&out, format!("${{1}}{MASK}@").as_str());
    let out = r.assignment.replace_all(&out, format!("${{1}}${{2}}{MASK}").as_str());
    let out = r.known.replace_all(&out, MASK);
    let out = r.candidate.replace_all(&out, |c: &regex::Captures<'_>| {
        let token = &c[0];
        if high_entropy(token) { MASK.to_owned() } else { token.to_owned() }
    });
    out.into_owned()
}

/// A run of 20 or more token characters that mixes letters with a digit, `/` or `+`, and whose
/// Shannon entropy per character is above 84% of the most its length allows: a key or a token,
/// not a word, an identifier or a hex hash.
fn high_entropy(token: &str) -> bool {
    let mixed = token.bytes().any(|b| b.is_ascii_digit() || b == b'/' || b == b'+');
    if !mixed || !token.bytes().any(|b| b.is_ascii_alphabetic()) {
        return false;
    }
    let mut counts = [0u32; 256];
    for b in token.bytes() {
        counts[usize::from(b)] += 1;
    }
    let len = token.len() as f64;
    let entropy: f64 = counts
        .iter()
        .filter(|c| **c > 0)
        .map(|c| {
            let p = f64::from(*c) / len;
            -p * p.log2()
        })
        .sum();
    entropy > 0.84 * len.min(64.0).log2()
}

/// The paths from the output type named `type_name` to its marked fields, as
/// [`redact_fields::PATHS`](crate::redact_fields::PATHS) lists them; none for a type that holds no
/// marked field.
pub fn paths(type_name: &str) -> &'static [&'static str] {
    PATHS.iter().find(|(name, _)| *name == type_name).map_or(&[], |(_, paths)| paths)
}

/// `value`, an output of the type named `type_name` as JSON, with every string under each of its
/// marked fields masked: a string, the strings of a list, or the string values of a map, at any
/// depth. Keys and every other field stay as they are.
pub fn value(type_name: &str, value: &mut Value) {
    for path in paths(type_name) {
        mask_at(value, &segments(path));
    }
}

/// `item` with every string under each of its type's marked fields masked.
pub fn output<T: Serialize + DeserializeOwned>(item: &mut T) {
    let name = std::any::type_name::<T>().rsplit("::").next().unwrap_or_default();
    if paths(name).is_empty() {
        return;
    }
    let mut json = serde_json::to_value(&*item).expect("a contract type serializes");
    value(name, &mut json);
    *item = serde_json::from_value(json).expect("a contract type reads back what it wrote");
}

/// A path's steps: property names, `[]` for an array's items and `{}` for a map's values.
fn segments(path: &str) -> Vec<&str> {
    let mut steps = Vec::new();
    for part in path.split('.') {
        let name = part.trim_end_matches(['[', ']', '{', '}']);
        if !name.is_empty() {
            steps.push(name);
        }
        let mut rest = &part[name.len()..];
        while let Some(step) = ["[]", "{}"].into_iter().find(|s| rest.starts_with(s)) {
            steps.push(step);
            rest = &rest[2..];
        }
    }
    steps
}

fn mask_at(value: &mut Value, steps: &[&str]) {
    match steps.split_first() {
        None => mask_all(value),
        Some((&"[]", rest)) => {
            if let Value::Array(items) = value {
                items.iter_mut().for_each(|item| mask_at(item, rest));
            }
        }
        Some((&"{}", rest)) => {
            if let Value::Object(map) = value {
                map.values_mut().for_each(|item| mask_at(item, rest));
            }
        }
        Some((field, rest)) => {
            if let Some(item) = value.as_object_mut().and_then(|o| o.get_mut(*field)) {
                mask_at(item, rest);
            }
        }
    }
}

fn mask_all(value: &mut Value) {
    match value {
        Value::String(s) => *s = text(s),
        Value::Array(items) => items.iter_mut().for_each(mask_all),
        Value::Object(map) => map.values_mut().for_each(mask_all),
        Value::Null | Value::Bool(_) | Value::Number(_) => {}
    }
}

/// An evidence item for an output: `contains` dropped unless `snippets`, and masked when kept.
pub fn evidence(item: &mut Evidence, snippets: bool) {
    if !snippets {
        drop_contains(item);
    }
    output(item);
}

fn drop_contains(item: &mut Evidence) {
    if let Evidence::Line { contains, .. } = item {
        *contains = None;
    }
}

/// A map for an output: its marked fields masked, and evidence `contains` dropped unless
/// `snippets`.
pub fn map_view(view: &mut MapView, snippets: bool) {
    if !snippets {
        view.edges.iter_mut().flat_map(|e| e.evidence_sample.iter_mut()).for_each(drop_contains);
    }
    output(view);
}

/// A pane for an output: its marked fields masked.
pub fn pane(pane: &mut PaneModel) {
    output(pane);
}

/// Presence rows for an output: their marked fields masked.
pub fn presence(rows: &mut [PresenceRow]) {
    rows.iter_mut().for_each(output);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_code_and_names_pass_unchanged() {
        for s in [
            "orders-db",
            "otel-collector",
            "    image: acme/storefront:2.3.0",
            "def cancel_order_and_refund_customer(order)",
            "worker:w:worker",
            "port = 8080",
            "OrderServiceV2Handler",
            "ProcessPaymentRequest2024Handler",
            "commit 6b4e49494921a118e4fb2957c66c2fba86495810",
            "src/main/java/com/acme/orders/OrderController.java",
        ] {
            assert_eq!(text(s), s);
        }
    }

    #[test]
    fn each_secret_shape_is_masked() {
        assert_eq!(
            text("DATABASE_URL=postgres://app:hunter2@db:5432/orders"),
            "DATABASE_URL=postgres://[redacted]@db:5432/orders"
        );
        assert_eq!(text("STRIPE_SECRET=abc123"), "STRIPE_SECRET=[redacted]");
        assert_eq!(text("api_token: \"tok\""), "api_token: \"[redacted]\"");
        assert_eq!(text("export DB_PASSWORD='p'"), "export DB_PASSWORD='[redacted]'");
        assert_eq!(text("key AKIAIOSFODNN7EXAMPLE here"), "key [redacted] here");
        assert_eq!(text("ghp_0123456789abcdefghijABCDEFGHIJ012345"), "[redacted]");
        assert_eq!(
            text("-----BEGIN RSA PRIVATE KEY-----\nMIIE\n-----END RSA PRIVATE KEY-----"),
            "[redacted]"
        );
        assert_eq!(text("token wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY"), "token [redacted]");
    }

    #[test]
    fn redaction_is_deterministic() {
        let s = "AWS_SECRET_KEY=wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY";
        assert_eq!(text(s), text(s));
        assert_eq!(text(s), "AWS_SECRET_KEY=[redacted]");
    }

    #[test]
    fn evidence_drops_its_text_unless_snippets_and_masks_what_it_keeps() {
        let line = || Evidence::Line {
            path: "orders/.env".into(),
            line: 3,
            contains: Some("ORDERS_API_KEY=sk-live-0123456789abcdefghij".into()),
            commit: Some("6b4e49494921a118e4fb2957c66c2fba86495810".into()),
            branch: None,
        };
        let mut dropped = line();
        evidence(&mut dropped, false);
        assert!(matches!(dropped, Evidence::Line { contains: None, .. }));
        let mut kept = line();
        evidence(&mut kept, true);
        let Evidence::Line { contains, commit, path, .. } = kept else { unreachable!() };
        assert_eq!(contains.as_deref(), Some("ORDERS_API_KEY=[redacted]"));
        // A hash and a path are identifiers: never masked.
        assert_eq!(commit.as_deref(), Some("6b4e49494921a118e4fb2957c66c2fba86495810"));
        assert_eq!(path, "orders/.env");
    }

    const SECRET: &str = "DB_PASSWORD=hunter2";
    const MASKED: &str = "DB_PASSWORD=[redacted]";
    /// An ID-shaped value that holds what would be masked in free text.
    const ID: &str = "ws:container:STRIPE_SECRET=abc123";

    /// A value of the shape `steps` walk, with `leaf` at its end and an `id` beside every object.
    fn planted(steps: &[&str], leaf: Value) -> Value {
        match steps.split_first() {
            None => leaf,
            Some((&"[]", rest)) => Value::Array(vec![planted(rest, leaf)]),
            Some((&"{}", rest)) => serde_json::json!({ "some_key": planted(rest, leaf) }),
            Some((field, rest)) => serde_json::json!({ "id": ID, *field: planted(rest, leaf) }),
        }
    }

    #[test]
    fn every_generated_path_masks_its_field_and_leaves_ids_alone() {
        assert!(!PATHS.is_empty());
        for (type_name, paths) in PATHS {
            for path in *paths {
                let steps = segments(path);
                for (leaf, want) in [
                    (Value::from(SECRET), Value::from(MASKED)),
                    (serde_json::json!([SECRET, "plain"]), serde_json::json!([MASKED, "plain"])),
                    (
                        serde_json::json!({ SECRET: SECRET, "port": 8080 }),
                        serde_json::json!({ SECRET: MASKED, "port": 8080 }),
                    ),
                ] {
                    let mut got = planted(&steps, leaf);
                    value(type_name, &mut got);
                    assert_eq!(got, planted(&steps, want), "{type_name}: {path}");
                }
            }
        }
    }

    #[test]
    fn a_path_splits_into_fields_items_and_map_values() {
        assert_eq!(segments("nodes[].name"), ["nodes", "[]", "name"]);
        assert_eq!(segments("maps{}[].edges[].evidence_sample[].contains")[..3], ["maps", "{}", "[]"]);
        assert_eq!(segments("doing"), ["doing"]);
        assert_eq!(segments("[].name"), ["[]", "name"]);
    }

    #[test]
    fn the_views_mask_the_fields_their_marks_name() {
        assert_eq!(paths("MapView"), ["edges[].evidence_sample[].contains", "nodes[].name"]);
        assert_eq!(
            paths("PaneModel"),
            ["diffs[].hunk", "focus.breadcrumb[].name", "minimap.blocks[].name", "who[].doing"]
        );
        assert_eq!(paths("PresenceRow"), ["doing"]);
        assert_eq!(paths("Evidence"), ["contains"]);
        assert!(paths("Handle").is_empty());
    }

    #[test]
    fn a_map_view_masks_block_names_and_evidence_and_keeps_its_ids() {
        let view = || -> MapView {
            serde_json::from_value(serde_json::json!({
                "level": "system", "lens": "all", "focus": null, "hidden": 0, "empty": null,
                "nodes": [{
                    "id": ID, "kind": "k.x", "tone": null, "name": SECRET, "level": "system",
                    "parent": null, "counts": {}, "position": null, "ghost": false, "claimed_by": null
                }],
                "edges": [{
                    "kind": "k.y", "from": ID, "to": ID, "count": 1, "children": [],
                    "evidence_sample": [{"type": "line", "path": "r/.env", "line": 1, "contains": SECRET,
                                         "commit": null, "branch": null}]
                }]
            }))
            .unwrap()
        };
        let mut kept = view();
        map_view(&mut kept, true);
        assert_eq!(kept.nodes[0].name, MASKED);
        assert_eq!(kept.nodes[0].id.0, ID);
        assert_eq!(kept.edges[0].from.0, ID);
        let Evidence::Line { contains, .. } = &kept.edges[0].evidence_sample[0] else { unreachable!() };
        assert_eq!(contains.as_deref(), Some(MASKED));
        let mut dropped = view();
        map_view(&mut dropped, false);
        assert!(matches!(dropped.edges[0].evidence_sample[0], Evidence::Line { contains: None, .. }));
    }

    #[test]
    fn a_pane_masks_names_hunks_and_what_people_are_doing() {
        let handle = serde_json::json!({"command": "map", "args": [], "label": "Open"});
        let mut pane: PaneModel = serde_json::from_value(serde_json::json!({
            "focus": {"breadcrumb": [{"id": ID, "name": SECRET}], "level": "system", "lens": "all",
                      "project": null, "seq": 1},
            "decision": null, "more_decisions": 0,
            "progress": {"stage": null, "done": 0, "total": 0, "handoffs_running": 0, "scan": "done"},
            "diffs": [{"summary": "one", "open": handle, "hunk": [SECRET, "+ plain"]}],
            "drift": {"summary": "none", "rules_broken": 0, "machine_drift": 0},
            "who": [{"actor": {"kind": "system", "part": "scan"}, "surface": "cli", "doing": SECRET,
                     "focus": null, "seen_at": "2026-10-26T00:00:00Z"}],
            "next": handle,
            "minimap": {"level": "system", "blocks": [{"id": ID, "name": SECRET, "focused": true}], "edges": []},
            "empty": null
        }))
        .unwrap();
        super::pane(&mut pane);
        assert_eq!(pane.focus.breadcrumb[0].name, MASKED);
        assert_eq!(pane.focus.breadcrumb[0].id.0, ID);
        assert_eq!(pane.diffs[0].hunk, [MASKED, "+ plain"]);
        assert_eq!(pane.who[0].doing.as_deref(), Some(MASKED));
        assert_eq!(pane.minimap.blocks[0].name, MASKED);
        let mut rows = pane.who.clone();
        rows[0].doing = Some(SECRET.into());
        presence(&mut rows);
        assert_eq!(rows[0].doing.as_deref(), Some(MASKED));
    }
}
