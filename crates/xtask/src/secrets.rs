//! The secret scan over the committed recordings and evidence: `tests/replay/` and
//! `docs/evidence/` (repo-layout.md, Area contracts; AGENTS.md, Money and credentials).
//!
//! It looks for the kernel's named secret shapes, through `kernel::redact::named_secrets`: a private
//! key, a DSN's user info, a secret assignment's value and a known token shape. It never applies the
//! high-entropy rule, so a hash or an ID fails nothing. An assignment whose value is already masked
//! holds no secret, nor does a number, a hex hash or an ID under one of the handle fields Planroom
//! itself writes ([`HANDLES`]); under any other name, any value is a finding, so a credential
//! named in no list (`encryption_key`, `webhook_token`) is still found. A finding names the file,
//! the line and the shape, never the secret.

use std::fs;
use std::path::Path;

use planroom_kernel::redact::{MASK, NamedSecret, named_secrets};

/// The directories the scan reads, from the repo root.
pub const SCANNED: [&str; 2] = ["tests/replay", "docs/evidence"];

/// Every finding under the scanned directories of `root`, as `path:line: shape`.
pub fn scan(root: &Path) -> Result<Vec<String>, String> {
    let mut out = Vec::new();
    for dir in SCANNED {
        let mut files = Vec::new();
        walk(&root.join(dir), &mut files)?;
        files.sort();
        for path in files {
            let rel = path.strip_prefix(root).unwrap_or(&path).display().to_string();
            let bytes = fs::read(&path).map_err(|e| format!("{rel}: {e}"))?;
            // An image or other binary file holds no text to scan.
            if bytes.contains(&0) {
                continue;
            }
            let text = String::from_utf8_lossy(&bytes);
            out.extend(findings(&text).into_iter().map(|(line, shape)| format!("{rel}:{line}: {shape}")));
        }
    }
    Ok(out)
}

fn walk(dir: &Path, out: &mut Vec<std::path::PathBuf>) -> Result<(), String> {
    if !dir.is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(dir).map_err(|e| format!("{}: {e}", dir.display()))? {
        let entry = entry.map_err(|e| format!("{}: {e}", dir.display()))?;
        let kind = entry.file_type().map_err(|e| format!("{}: {e}", entry.path().display()))?;
        if kind.is_dir() {
            walk(&entry.path(), out)?;
        } else if kind.is_file() {
            out.push(entry.path());
        }
    }
    Ok(())
}

/// The secrets in `text`, as `(line, shape)`, line numbers from 1.
pub fn findings(text: &str) -> Vec<(usize, &'static str)> {
    let mut out: Vec<(usize, &'static str)> = named_secrets(text)
        .into_iter()
        .filter(|f| !holds_no_secret(text, f))
        .map(|f| (1 + text[..f.start].matches('\n').count(), f.shape))
        .collect();
    out.sort();
    out.dedup();
    out
}

/// An assignment whose value is already masked, or a literal, names no secret; nor does a number,
/// a hash or an ID under a handle field Planroom writes, such as `view_key`.
fn holds_no_secret(text: &str, found: &NamedSecret) -> bool {
    if found.shape != "a secret assignment's value" {
        return false;
    }
    let value = &text[found.start..found.end];
    if value.contains(MASK) || ["null", "true", "false"].contains(&value) {
        return true;
    }
    // A commit (7 to 12 characters, or 40) or a sha256: lowercase hex of a hash's length. A hex
    // key of another length, such as 32, is still a finding.
    let hex = |s: &str| {
        matches!(s.len(), 7..=12 | 40 | 64)
            && s.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    };
    holds_a_handle(assigned_name(text, found.start))
        && (value.bytes().all(|b| b.is_ascii_digit() || b == b'.')
            || hex(value)
            || value.strip_prefix("sha256:").is_some_and(hex)
            || id_shaped(value))
}

/// The name an assignment's value at `start` is assigned to: the word before its `=` or `:`.
fn assigned_name(text: &str, start: usize) -> &str {
    let before = text[..start].trim_end_matches(['"', '\'', ' ', '\t', '=', ':']);
    let from = before.rfind(|c: char| !(c.is_ascii_alphanumeric() || c == '_')).map_or(0, |i| i + 1);
    &before[from..]
}

/// The `_key` and `_token` fields Planroom itself writes that hold a hash, a count or an ID, never a
/// credential: the contract's `view_key`. The list fails safe: a name it doesn't hold is a
/// credential's, whatever its value looks like, and a new handle field joins it in a reviewed
/// change.
const HANDLES: [&str; 1] = ["view_key"];

/// Whether `name` is one of Planroom's [`HANDLES`].
fn holds_a_handle(name: &str) -> bool {
    HANDLES.contains(&name)
}

/// `namespace:kind-short:key`, an edge ID `e:<12 hex>` or a plan item `plan:<project>:<key>`
/// (AGENTS.md, Naming).
fn id_shaped(value: &str) -> bool {
    if let Some(rest) = value.strip_prefix("e:") {
        return rest.len() == 12 && rest.bytes().all(|b| b.is_ascii_hexdigit());
    }
    let parts: Vec<&str> = value.splitn(3, ':').collect();
    parts.len() == 3
        && parts.iter().all(|p| !p.is_empty())
        && parts[1].bytes().all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_planted_key_dsn_or_token_is_found_by_line_and_shape() {
        let text = "{\n\"a\": \"-----BEGIN OPENSSH PRIVATE KEY-----\\nb3Bl\\n-----END OPENSSH PRIVATE KEY-----\",\n\
                    \"url\": \"postgres://app:hunter2@db/orders\",\n\"auth\": \"ghp_0123456789abcdefghijABCDEFGHIJ012345\",\n\
                    \"STRIPE_SECRET\": \"live1x\"\n}";
        assert_eq!(
            findings(text),
            [
                (2, "a private key"),
                (3, "a DSN's user info"),
                (4, "a known token shape"),
                (5, "a secret assignment's value")
            ]
        );
        // Under a credential's name, a value shaped like a hash, a number or an ID is still the
        // credential: a token or a webhook secret can be 40 or 64 hex, and a password all digits.
        for planted in [
            "GITHUB_TOKEN=0123456789abcdef0123456789abcdef01234567",
            "WEBHOOK_SECRET=e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
            "DB_PASSWORD=12345678",
            "DB_PASSWORD=deadbeef",
            "API_KEY=abc:def:ghi",
            "{\"api_key\": \"0123456789abcdef0123456789abcdef01234567\"}",
            "{\"webhook_secret\": \"e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855\"}",
            "{\"access_token\": \"orders:pkg:x\"}",
            "{\"encryption_key\": \"e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855\"}",
            "{\"signing_key\": \"0123456789abcdef0123456789abcdef01234567\"}",
            "{\"github_token\": \"0123456789abcdef0123456789abcdef01234567\"}",
            "{\"webhook_token\": \"104c10e\"}",
        ] {
            assert_eq!(findings(planted), [(1, "a secret assignment's value")], "{planted}");
        }
        assert_eq!(findings("REDIS_URL=redis://:secretpw@cache:6379"), [(1, "a DSN's user info")]);
    }

    #[test]
    fn hashes_ids_numbers_and_masked_values_are_no_finding() {
        let text = "{\"request_hash\": \"e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855\",\n\
                    \"api_key\": \"[redacted]\", \"id_token\": null,\n\
                    \"entropy\": \"wJalrXUtnFEMI/K7MDENG/bPxRfiCYEXAMPLEKEY\"}";
        assert_eq!(findings(text), []);
        // Under a handle field Planroom writes, a hash, a number or an ID is no secret.
        for handle in [
            "{\"view_key\": \"9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08\"}",
            "{\"view_key\": \"104c10e\"}",
            "{\"view_key\": 4, \"n\": 1}",
            "{\"view_key\": \"orders:pkg:internal/http\"}",
            "{\"view_key\": \"e:0123456789ab\"}",
        ] {
            assert_eq!(findings(handle), [], "{handle}");
        }
        // A hex value of no hash's length under a secret's name is a key, not a hash.
        assert_eq!(
            findings("DD_API_KEY=0123456789abcdef0123456789abcdef"),
            [(1, "a secret assignment's value")]
        );
    }
}
