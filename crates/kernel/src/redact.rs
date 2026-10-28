//! Redaction (architecture.md, "What leaves the machine"; DR-64): masks secrets in the free text
//! that comes from a repo before it reaches any output. It masks DSN user info, the values of
//! variables whose names end in `_KEY`, `_SECRET`, `_TOKEN` or `_PASSWORD`, private-key blocks,
//! well-known token shapes and high-entropy strings. It is deterministic. A token shaped like a node
//! ID is exempt from the high-entropy rule alone (see [`text`]).
//!
//! It masks by field (DR-89): exactly the fields the contract marks
//! `#[schemars(extend("x-redact" = true))]`, through the paths `cargo xtask gen` writes into
//! [`redact_fields`](crate::redact_fields). An ID, a handle, a cursor or a hash is never marked, so
//! it is never touched. An error's or a warning's `details` are masked by key: the registry names,
//! per code, the keys that hold repo-derived free text, and `cargo xtask gen` writes them there too.

use std::sync::OnceLock;

use planroom_contract::{Answer, Evidence, MapView, PaneModel, PresenceRow};
use regex::Regex;
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;

use crate::redact_fields::{DETAILS, ERROR_DETAILS, OPEN, PATHS, WARNING_DETAILS};

/// What a masked value becomes.
pub const MASK: &str = "[redacted]";

struct Rules {
    dsn: Regex,
    private_key: Regex,
    assignment: Regex,
    known: Regex,
    candidate: Regex,
    token: Regex,
}

fn rules() -> &'static Rules {
    static RULES: OnceLock<Rules> = OnceLock::new();
    RULES.get_or_init(|| Rules {
        // scheme://user:password@host, or scheme://:password@host with no user name
        dsn: Regex::new(r"([A-Za-z][A-Za-z0-9+.\-]*://)[^\s/:@]*:[^\s/@]+@").expect("a valid pattern"),
        private_key: Regex::new(r"-----BEGIN [A-Z0-9 ]*PRIVATE KEY-----(?s:.*?)(?:-----END [A-Z0-9 ]*PRIVATE KEY-----|\z)")
            .expect("a valid pattern"),
        // NAME_KEY=value, NAME_SECRET: value, "api_token":"value". An `=` always assigns; a `:`
        // assigns only when whitespace or a quote follows it, or a quote comes before it, so the
        // `:` that joins `access_token:dir:src` assigns nothing.
        assignment: Regex::new(
            r#"(?i)\b([A-Z0-9_]*(?:_KEY|_SECRET|_TOKEN|_PASSWORD|PASSWD|_PASS))(["']?\s*=\s*["']?|["']\s*:\s*["']?|\s*:\s+["']?|\s*:["'])([^\s"',;]+)"#,
        )
        .expect("a valid pattern"),
        known: Regex::new(
            r"\b(?:AKIA[0-9A-Z]{16}|gh[pousr]_[A-Za-z0-9]{30,}|github_pat_[A-Za-z0-9_]{30,}|sk-[A-Za-z0-9_\-]{20,}|xox[abprs]-[A-Za-z0-9\-]{10,})\b",
        )
        .expect("a valid pattern"),
        candidate: Regex::new(r"[A-Za-z0-9+/=]{20,}").expect("a valid pattern"),
        token: Regex::new(r"\S+").expect("a valid pattern"),
    })
}

/// `s` with every secret it holds masked. The named shapes (a private key, a DSN's user info, an
/// assignment's value, a known token shape) are masked wherever they are. The high-entropy rule
/// leaves alone a whitespace-delimited token shaped like a node ID (`namespace:kind-short:key`, see
/// [`id_shaped`]): an ID is never redacted, and the reply carries the same ID unmasked in its ID
/// fields, so masking a mixed-case path inside a line would hide nothing and leave the line naming
/// an ID nobody can pass. A named shape inside an ID-shaped token is still masked, so a secret
/// never hides behind the shape.
pub fn text(s: &str) -> String {
    let r = rules();
    let named = named(s);
    let mut out = String::with_capacity(named.len());
    let mut from = 0;
    for token in r.token.find_iter(&named) {
        let word = token.as_str();
        let core = word.trim_start_matches(OPENERS);
        let start = token.start() + (word.len() - core.len());
        let core = core.trim_end_matches(CLOSERS);
        // A token the named shapes have already masked part of held a secret, not an ID: masking
        // can remove the `=` that kept it from reading as one.
        if id_shaped(core) && !core.contains(MASK) {
            out.push_str(&entropy(&named[from..start]));
            out.push_str(core);
            from = start + core.len();
        }
    }
    out.push_str(&entropy(&named[from..]));
    out
}

/// What may open or close a token around an ID in a line, such as the `,` after each ID a list
/// names or the brackets around one.
const OPENERS: &[char] = &['(', '[', '{', '<', '"', '\'', '`'];
const CLOSERS: &[char] = &[',', ';', '.', ':', '!', '?', ')', ']', '}', '>', '"', '\'', '`'];

/// Whether `token` is shaped like a node ID (AGENTS.md, Naming): `namespace:kind-short:key`, with a
/// namespace and a key that aren't empty, a kind-short in lowercase `snake_case`, and neither the
/// namespace nor the key holding `=` or `://`, which mark an assignment or a URL rather than an ID.
fn id_shaped(token: &str) -> bool {
    let mut parts = token.splitn(3, ':');
    let (Some(namespace), Some(kind), Some(key)) = (parts.next(), parts.next(), parts.next()) else {
        return false;
    };
    !namespace.is_empty()
        && !key.is_empty()
        && kind.starts_with(|c: char| c.is_ascii_lowercase())
        && kind.bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
        && [namespace, key].iter().all(|part| !part.contains('=') && !part.contains("://"))
}

/// `s` with the named shapes masked: private keys, a DSN's user info, assignments' values and
/// known token shapes.
fn named(s: &str) -> String {
    let r = rules();
    let out = r.private_key.replace_all(s, MASK);
    let out = r.dsn.replace_all(&out, format!("${{1}}{MASK}@").as_str());
    let out = r.assignment.replace_all(&out, format!("${{1}}${{2}}{MASK}").as_str());
    r.known.replace_all(&out, MASK).into_owned()
}

/// One named secret shape found in a text: which shape, and the byte range of the secret itself
/// (for an assignment, its value alone).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NamedSecret {
    pub shape: &'static str,
    pub start: usize,
    pub end: usize,
}

/// Every named secret shape in `s`, in the order the shapes are listed: a private key, a DSN's user
/// info, an assignment's value and a known token shape. These are the shapes [`text`] masks
/// wherever they are; the high-entropy rule isn't one of them, so a hash or an ID alone is never
/// found. The repo's secret scan over committed recordings and evidence reads them here.
pub fn named_secrets(s: &str) -> Vec<NamedSecret> {
    let r = rules();
    let at = |shape, start, end| NamedSecret { shape, start, end };
    let mut out: Vec<NamedSecret> =
        r.private_key.find_iter(s).map(|m| at("a private key", m.start(), m.end())).collect();
    // A DSN's user info: what follows the scheme, up to the `@`.
    out.extend(r.dsn.captures_iter(s).filter_map(|c| {
        let (whole, scheme) = (c.get(0)?, c.get(1)?);
        Some(at("a DSN's user info", scheme.end(), whole.end() - 1))
    }));
    out.extend(
        r.assignment
            .captures_iter(s)
            .filter_map(|c| c.get(3).map(|v| at("a secret assignment's value", v.start(), v.end()))),
    );
    out.extend(r.known.find_iter(s).map(|m| at("a known token shape", m.start(), m.end())));
    out
}

/// `s`, text that holds no ID, with every high-entropy run masked.
fn entropy(s: &str) -> String {
    rules()
        .candidate
        .replace_all(s, |c: &regex::Captures<'_>| {
            let token = &c[0];
            if high_entropy(token) { MASK.to_owned() } else { token.to_owned() }
        })
        .into_owned()
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

/// The paths from the output type named `type_name` to each error or warning it holds, `""` for
/// the type itself, with the registry its code is in (`error` or `warning`).
pub fn details_paths(type_name: &str) -> &'static [(&'static str, &'static str)] {
    DETAILS.iter().find(|(name, _)| *name == type_name).map_or(&[], |(_, paths)| paths)
}

/// The keys of the `details` of the `registry` (`error` or `warning`) code `code` that hold
/// repo-derived free text; none for a code the registry gives none, or doesn't know.
pub fn details_keys(registry: &str, code: &str) -> &'static [&'static str] {
    let table = if registry == "error" { ERROR_DETAILS } else { WARNING_DETAILS };
    table.iter().find(|(c, _)| *c == code).map_or(&[], |(_, keys)| keys)
}

/// `value`, an output of the type named `type_name` as JSON, with every string under each of its
/// marked fields masked: a string, the strings of a list, or the string values of a map, at any
/// depth; and, in each error and warning it holds, every string under the `details` keys its code
/// names. Keys and every other field stay as they are.
pub fn value(type_name: &str, value: &mut Value) {
    for path in paths(type_name) {
        visit(value, &segments(path), &mut mask_all);
    }
    for (path, registry) in details_paths(type_name) {
        visit(value, &segments(path), &mut |holder| mask_details(holder, registry));
    }
}

/// `holder`, an error or a warning, with the `details` keys its code names masked.
fn mask_details(holder: &mut Value, registry: &str) {
    let Some(code) = holder.get("code").and_then(Value::as_str) else { return };
    let keys = details_keys(registry, code);
    if let Some(details) = holder.get_mut("details").and_then(Value::as_object_mut) {
        for key in keys {
            if let Some(item) = details.get_mut(*key) {
                mask_all(item);
            }
        }
    }
}

/// `item` with every string under each of its type's marked fields masked.
pub fn output<T: Serialize + DeserializeOwned>(item: &mut T) {
    let name = type_name::<T>();
    if paths(name).is_empty() && details_paths(name).is_empty() {
        return;
    }
    let mut json = serde_json::to_value(&*item).expect("a contract type serializes");
    value(name, &mut json);
    *item = serde_json::from_value(json).expect("a contract type reads back what it wrote");
}

/// `json`, an output of type `T` as JSON, with every string under each of `T`'s marked fields
/// masked: how a reply's result of any type is masked once it is JSON.
pub fn value_of<T>(json: &mut Value) {
    value(type_name::<T>(), json);
}

/// Whether [`value_of`] masks every repo-derived string `T` can hold: `T` is a contract type that
/// [`redact_fields`](crate::redact_fields) lists, because it holds a marked field or an error or a
/// warning, and it holds no open value that no mark covers (`redact_fields::OPEN`). `value_of`
/// finds a type by its last path segment and passes one it doesn't list through unmasked, such as
/// a `Vec<Node>` or an app's own wrapper around a contract type. It masks only a listed type's own
/// marks, never what its open values hold, such as `Envelope`'s or `Answer`'s `result`, and it
/// can't see a generic type's arguments, since its name leaves them out. So a caller that must mask
/// whatever it is given, such as a model request's slice, checks this first.
pub fn is_listed<T>() -> bool {
    let full = std::any::type_name::<T>();
    let name = type_name::<T>();
    full.starts_with("planroom_contract::")
        && !full.contains('<')
        && (!paths(name).is_empty() || !details_paths(name).is_empty())
        && !OPEN.iter().any(|(open, _)| *open == name)
}

/// The name [`redact_fields::PATHS`](crate::redact_fields::PATHS) lists `T` under: its last path
/// segment with its type arguments left out, so `Envelope<Value>` is `Envelope`.
fn type_name<T>() -> &'static str {
    let full = std::any::type_name::<T>();
    let base = full.split('<').next().unwrap_or(full);
    base.rsplit("::").next().unwrap_or(base)
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

/// Call `f` on each value at the end of `steps` under `value`.
fn visit(value: &mut Value, steps: &[&str], f: &mut dyn FnMut(&mut Value)) {
    match steps.split_first() {
        None => f(value),
        Some((&"[]", rest)) => {
            if let Value::Array(items) = value {
                items.iter_mut().for_each(|item| visit(item, rest, f));
            }
        }
        Some((&"{}", rest)) => {
            if let Value::Object(map) = value {
                map.values_mut().for_each(|item| visit(item, rest, f));
            }
        }
        Some((field, rest)) => {
            if let Some(item) = value.as_object_mut().and_then(|o| o.get_mut(*field)) {
                visit(item, rest, f);
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

/// A read request's answer for an output: its marked fields masked, its error's and warnings'
/// lines and their repo-text `details` among them. Its `result` is masked by its own type, such as
/// [`map_view`], before it becomes JSON.
pub fn answer(answer: &mut Answer) {
    output(answer);
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
        assert_eq!(text("redis://:secretpw@cache:6379"), "redis://[redacted]@cache:6379");
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
    fn only_a_listed_contract_type_is_listed() {
        use planroom_contract::{FactBatch, Node, Seq};
        assert!(is_listed::<FactBatch>() && is_listed::<Node>() && is_listed::<planroom_contract::MapView>());
        // A list, an option, a contract type with nothing to mask, and a type of another crate
        // that shares a listed type's name all pass `value_of` unmasked.
        #[allow(dead_code)]
        struct MapView {
            name: String,
        }
        assert!(!is_listed::<Vec<Node>>());
        assert!(!is_listed::<Option<FactBatch>>());
        assert!(!is_listed::<Seq>());
        assert!(!is_listed::<MapView>());
        assert!(!is_listed::<serde_json::Value>());
        // A listed type whose payload no mark covers: a generic wrapper, whose type arguments its
        // name leaves out, and a type that carries another command's result or unknown fields as
        // open JSON, masked by its own writer rather than by marks.
        use planroom_contract::{Answer, Envelope, MapView as View, Snapshot, ToolResult};
        assert!(!is_listed::<Envelope<View>>());
        assert!(!is_listed::<Envelope>());
        assert!(!is_listed::<ToolResult<View>>());
        assert!(!is_listed::<Answer>());
        assert!(!is_listed::<Snapshot>());
    }

    #[test]
    fn named_secrets_finds_each_named_shape_and_never_a_hash_or_an_id() {
        let found = |s: &str| -> Vec<(&str, String)> {
            named_secrets(s).into_iter().map(|f| (f.shape, s[f.start..f.end].to_owned())).collect()
        };
        assert_eq!(
            found("DATABASE_URL=postgres://app:hunter2@db:5432/orders"),
            [("a DSN's user info", "app:hunter2".to_owned())]
        );
        // A password with no user name is user info too.
        assert_eq!(
            found("REDIS_URL=redis://:secretpw@cache:6379"),
            [("a DSN's user info", ":secretpw".to_owned())]
        );
        assert_eq!(found("STRIPE_SECRET=abc123"), [("a secret assignment's value", "abc123".to_owned())]);
        assert_eq!(
            found("key AKIAIOSFODNN7EXAMPLE here"),
            [("a known token shape", "AKIAIOSFODNN7EXAMPLE".to_owned())]
        );
        let key = "-----BEGIN EC PRIVATE KEY-----\nMHcC\n-----END EC PRIVATE KEY-----";
        assert_eq!(found(key), [("a private key", key.to_owned())]);
        // A sha256, a commit, a high-entropy run and an ID are the entropy rule's, never a named shape.
        for s in [
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
            "commit 104c10e9f2a1",
            "wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY",
            "orders:pkg:internal/http e:0123456789ab",
        ] {
            assert_eq!(found(s), [], "{s}");
        }
    }

    #[test]
    fn the_engine_tokens_of_a_line_survive_masking() {
        // Commit SHAs short and full, a sha256, seqs, counts, levels and IDs: what an engine line
        // names beside repo text, which masking keeps whole.
        for s in [
            "Scanned orders at 3d50834.",
            "Scanned orders at 6b4e49494921a118e4fb2957c66c2fba86495810.",
            "structure 9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08",
            "shop · system · lens all · seq 412",
            "Mapped 5 repos, 5 of them moved: 9 containers, 135 nodes, 17 hints.",
            "10 at the top: kit (11 code, 4 component); orders (23 code, 14 component, 1 container).",
            "--focus src names 2 blocks, such as kit:dir:src, storefront:file:src/components/checkout/OrderSummaryPanel2.tsx; pass one of their IDs.",
            "edge e:0123456789ab, event 0192f5e4-7c1a-7b3e-9a4d-5f6e7d8c9b0a, ws:container:orders-db",
        ] {
            assert_eq!(text(s), s);
        }
        // Beside a secret, the same tokens stay whole and only the secret goes.
        assert_eq!(
            text("Scanned db_PASSWORD=hunter2 at 3d50834."),
            "Scanned db_PASSWORD=[redacted] at 3d50834."
        );
        assert_eq!(
            text("3 under AKIAIOSFODNN7EXAMPLE at the code level: a (2 code); b."),
            "3 under [redacted] at the code level: a (2 code); b."
        );
        assert_eq!(
            text("Nothing under postgres://app:hunter2@db at the component level."),
            "Nothing under postgres://[redacted]@db at the component level."
        );
    }

    #[test]
    fn an_id_stays_whole_by_its_shape() {
        // A whitespace-delimited token shaped like `namespace:kind-short:key` is an ID: the
        // high-entropy rule leaves it whole, whatever its namespace or key looks like, while the text
        // around it is masked as before.
        for s in [
            "--focus OAuth2PKCEFlow.ts names 2 blocks, such as alpha:file:lib/v3/OAuth2PKCEFlow.ts, beta:file:lib/v3/OAuth2PKCEFlow.ts; pass one of their IDs.",
            "x:file:internal/pkg/xdg/b64url/Encode.go",
            "--focus src names 3 blocks, such as access_token:dir:src, kit:dir:src, storefront:dir:src; pass one of their IDs.",
            "There is no block access_token:dir:nope on this map.",
            "(stripe_secret:repo:stripe_secret)",
            "x:file:src/k7Gq2ZpX9vLm3RtY8wNc5HbJ4dFs6QaE1uKo/config.yml",
        ] {
            assert_eq!(text(s), s);
        }
        // The same secret outside an ID is still masked, beside an ID that stays whole.
        assert_eq!(
            text("access_token: hunter2 under access_token:dir:src"),
            "access_token: [redacted] under access_token:dir:src"
        );
    }

    #[test]
    fn a_named_secret_shape_is_masked_inside_an_id_shaped_token() {
        // The ID shape keeps only the high-entropy rule away: a known token shape, an assignment's
        // value or a DSN's user info is masked wherever it sits, so a secret can't hide behind it.
        assert_eq!(
            text("key AKIAIOSFODNN7EXAMPLE in AKIAIOSFODNN7EXAMPLE:dir:src."),
            "key [redacted] in [redacted]:dir:src."
        );
        assert_eq!(
            text("{\"api_token\": \"auth:bearer:ghp_0123456789abcdefghijABCDEFGHIJ012345\"}"),
            "{\"api_token\": \"[redacted]\"}"
        );
        assert_eq!(text("ghp_0123456789abcdefghijABCDEFGHIJ012345:repo:x"), "[redacted]:repo:x");
        assert_eq!(text("API_KEY= kit:dir:src"), "API_KEY= [redacted]");
        // A masked assignment can take the `=` that kept a token from reading as an ID; the rest of
        // that token is still weighed by the high-entropy rule.
        for s in [
            "kit:file:\"api_token\":\"p=q\"k7Gq2ZpX9vLm3RtY8wNc5HbJ4dFs6QaE1uKo",
            "kit:file:api_token:\"p=q\"k7Gq2ZpX9vLm3RtY8wNc5HbJ4dFs6QaE1uKo",
        ] {
            let masked = text(s);
            assert!(!masked.contains("k7Gq2ZpX9vLm3RtY8wNc5HbJ4dFs6QaE1uKo"), "{masked}");
        }
    }

    #[test]
    fn an_id_shaped_token_holding_an_assignment_or_a_url_is_masked() {
        // A namespace or key that holds `=` or `://` keeps the token from reading as an ID, so the
        // rules mask it as any other text.
        assert_eq!(
            text("--focus src names 2 blocks, such as db_PASSWORD=hunter2:dir:src, kit:dir:src; pass one."),
            "--focus src names 2 blocks, such as db_PASSWORD=[redacted], kit:dir:src; pass one."
        );
        assert_eq!(text("kit:file:STRIPE_SECRET=abc123"), "kit:file:STRIPE_SECRET=[redacted]");
        assert_eq!(text("ws:container:postgres://app:hunter2@db"), "ws:container:postgres://[redacted]@db");
        // A kind-short that isn't lowercase snake_case makes no ID either.
        assert_eq!(text("Stripe:Key:wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY"), "Stripe:Key:[redacted]");
    }

    #[test]
    fn a_colon_assigns_only_before_whitespace_or_a_quote_or_after_a_quote() {
        assert_eq!(text("db_password: hunter2"), "db_password: [redacted]");
        assert_eq!(text("db_password:\thunter2"), "db_password:\t[redacted]");
        assert_eq!(text("{\"api_token\":\"x\"}"), "{\"api_token\":\"[redacted]\"}");
        assert_eq!(text("api_token:'x'"), "api_token:'[redacted]'");
        assert_eq!(text("API_KEY=x"), "API_KEY=[redacted]");
        assert_eq!(text("API_KEY = x"), "API_KEY = [redacted]");
        // A `:` with neither is no assignment: it joins the parts of a name.
        for s in ["access_token:dir", "see access_token:dir:src/app.txt", "client_secret:v2"] {
            assert_eq!(text(s), s);
        }
    }

    #[test]
    fn a_base64_secret_with_slashes_is_masked_whole() {
        // A run that holds a `+`, or ends in `=` padding, is base64: one candidate, `/`s and all.
        assert_eq!(text("key wJalrXUtnFEMI/K7MDENG+bPxRfiCYEXAMPLEKEY here"), "key [redacted] here");
        assert_eq!(text("blob q3Vx/9Lm2Pz8/Ra7Kd4Wn1Tf6Yc0Hs5Bj=="), "blob [redacted]");
        assert_eq!(text("sig dGhpcy9pcy9hL3Rlc3QvM2tleQ/abc12XYZ="), "sig [redacted]");
    }

    #[test]
    fn a_generic_output_is_masked_under_its_own_name() {
        assert_eq!(type_name::<MapView>(), "MapView");
        assert_eq!(type_name::<planroom_contract::Envelope<Value>>(), "Envelope");
        let mut json = serde_json::json!({"summary": SECRET, "ok": true});
        value_of::<planroom_contract::Envelope<Value>>(&mut json);
        assert_eq!(json["summary"], MASKED);
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
    fn every_error_and_warning_masks_the_details_its_code_names_and_nothing_else() {
        assert!(!DETAILS.is_empty() && !ERROR_DETAILS.is_empty() && !WARNING_DETAILS.is_empty());
        for (type_name, holders) in DETAILS {
            for (path, registry) in *holders {
                let table = if *registry == "error" { ERROR_DETAILS } else { WARNING_DETAILS };
                for (code, keys) in table {
                    // Every key the code names is masked; a key it doesn't name, such as an ID
                    // or a handle beside them, stays as it is.
                    let mut details = serde_json::json!({ "id": ID, "next": {"args": [ID]} });
                    for key in *keys {
                        details[*key] = Value::from(SECRET);
                    }
                    let mut want = details.clone();
                    for key in *keys {
                        want[*key] = Value::from(MASKED);
                    }
                    let holder = |details: Value| serde_json::json!({"code": code, "details": details});
                    let mut got = planted(&segments(path), holder(details));
                    value(type_name, &mut got);
                    assert_eq!(got, planted(&segments(path), holder(want)), "{type_name}: {path} {code}");
                }
            }
        }
        // A code whose registry entry names no key keeps its details whole.
        let mut warning =
            serde_json::json!({"code": "truncated", "message": "m", "details": {"next": SECRET}});
        value("Warning", &mut warning);
        assert_eq!(warning["details"]["next"], SECRET);
        let mut error = serde_json::json!({"code": "invalid_argument", "details": {"field": SECRET}});
        value("ErrorObject", &mut error);
        assert_eq!(error["details"]["field"], SECRET);
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
        assert_eq!(paths("MapView"), ["edges[].evidence_sample[].contains", "empty.line", "nodes[].name"]);
        assert_eq!(
            paths("PaneModel"),
            [
                "decision.question",
                "decision.why",
                "diffs[].hunk",
                "diffs[].summary",
                "drift.summary",
                "empty.line",
                "focus.breadcrumb[].name",
                "minimap.blocks[].name",
                "who[].doing"
            ]
        );
        assert_eq!(paths("PresenceRow"), ["doing"]);
        assert_eq!(paths("Evidence"), ["contains"]);
        assert!(paths("Handle").is_empty());
        // The engine lines that name repo text: a reply's summary, an empty state's line, and
        // the lines of warnings, errors and stream frames; never the handle beside them.
        assert_eq!(paths("Empty"), ["line"]);
        // A dirty repo's name is repo text too, though the line naming it is the reply's own.
        assert_eq!(
            paths("Envelope"),
            ["error.message", "meta.dirty_repos[].repo", "meta.warnings[].message", "summary"]
        );
        assert_eq!(paths("StreamFrame"), ["summary"]);
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
        assert_eq!(pane.minimap.as_ref().unwrap().blocks[0].name, MASKED);
        let mut rows = pane.who.clone();
        rows[0].doing = Some(SECRET.into());
        presence(&mut rows);
        assert_eq!(rows[0].doing.as_deref(), Some(MASKED));
    }
}
