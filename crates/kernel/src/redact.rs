//! Redaction (architecture.md, "What leaves the machine"; DR-64): masks secrets in the free text
//! that comes from a repo before it reaches any output. It masks DSN user info, the values of
//! variables whose names end in `_KEY`, `_SECRET`, `_TOKEN` or `_PASSWORD`, private-key blocks,
//! well-known token shapes and high-entropy strings. It is deterministic, and it never touches an
//! ID, a handle, a cursor or a hash: the walkers below change only free-text fields.
//!
//! Until the contract marks fields `#[redact]` (M1), the walkers name the fields by hand: block
//! and breadcrumb names, evidence `contains`, hunk lines and presence `doing`.

use std::sync::OnceLock;

use planroom_contract::{Evidence, MapView, PaneModel};
use regex::Regex;

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

/// An evidence item for an output: `contains` dropped unless `snippets`, and masked when kept.
pub fn evidence(item: &mut Evidence, snippets: bool) {
    if let Evidence::Line { contains, .. } = item {
        *contains = if snippets { contains.as_deref().map(text) } else { None };
    }
}

/// A map for an output: block names and evidence.
pub fn map_view(view: &mut MapView, snippets: bool) {
    for node in &mut view.nodes {
        node.name = text(&node.name);
    }
    for edge in &mut view.edges {
        edge.evidence_sample.iter_mut().for_each(|e| evidence(e, snippets));
    }
}

/// A pane for an output: breadcrumb and mini-map names, hunk lines and what people are doing.
pub fn pane(pane: &mut PaneModel) {
    for crumb in &mut pane.focus.breadcrumb {
        crumb.name = text(&crumb.name);
    }
    for block in &mut pane.minimap.blocks {
        block.name = text(&block.name);
    }
    for diff in &mut pane.diffs {
        diff.hunk.iter_mut().for_each(|line| *line = text(line));
    }
    for row in &mut pane.who {
        row.doing = row.doing.as_deref().map(text);
    }
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
        assert_eq!(text("DATABASE_URL=postgres://app:hunter2@db:5432/orders"), "DATABASE_URL=postgres://[redacted]@db:5432/orders");
        assert_eq!(text("STRIPE_SECRET=abc123"), "STRIPE_SECRET=[redacted]");
        assert_eq!(text("api_token: \"tok\""), "api_token: \"[redacted]\"");
        assert_eq!(text("export DB_PASSWORD='p'"), "export DB_PASSWORD='[redacted]'");
        assert_eq!(text("key AKIAIOSFODNN7EXAMPLE here"), "key [redacted] here");
        assert_eq!(text("ghp_0123456789abcdefghijABCDEFGHIJ012345"), "[redacted]");
        assert_eq!(text("-----BEGIN RSA PRIVATE KEY-----\nMIIE\n-----END RSA PRIVATE KEY-----"), "[redacted]");
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
}
