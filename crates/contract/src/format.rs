//! The versioning rules for persisted and exchanged documents (contracts.md, "Versioning rules"):
//! every one carries `format: "planroom.<name>/v<N>"`; a reader loads `N` up to its own
//! `schema_version` and refuses a newer one with `schema_mismatch`; and it keeps the fields it
//! doesn't know in `extra`, which it writes back unchanged. The data root's shared files also
//! record `min_reader_version`, the oldest engine that may read them (DR-78).
//!
//! This is the one format check every reader uses; `cargo xtask gen` writes its TypeScript port,
//! `checkFormat` in `@planroom/contract`, from the same constants.

use std::cmp::Ordering;

use serde::de::DeserializeOwned;
use serde_json::{Map, Value};

use crate::envelope::SCHEMA_VERSION;

/// The fields a document's reader doesn't know, kept in their order and written back unchanged.
pub type Extra = Map<String, Value>;

/// The format tag this version writes for the document `name`, such as `planroom.snapshot/v1`.
pub fn tag(name: &str) -> String {
    format!("planroom.{name}/v{SCHEMA_VERSION}")
}

/// Why a document's `format` can't be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FormatError {
    /// A later version wrote it: `schema_mismatch`.
    Newer {
        /// The tag the document carries.
        found: String,
        /// The newest tag this version reads.
        supported: String,
    },
    /// It names another document, or no version: not this document at all.
    Other { found: String, expected: String },
}

/// Check the tag `found` against the document `name`: `Ok(N)` for `planroom.<name>/vN` with `N`
/// from 1 up to this version's `schema_version`.
pub fn check(found: &str, name: &str) -> Result<u32, FormatError> {
    check_against(found, name, SCHEMA_VERSION)
}

/// `check` for a reader whose own `schema_version` is `own`.
pub fn check_against(found: &str, name: &str, own: u32) -> Result<u32, FormatError> {
    let other = || FormatError::Other { found: found.to_owned(), expected: tag(name) };
    let version = found
        .strip_prefix("planroom.")
        .and_then(|rest| rest.strip_prefix(name))
        .and_then(|rest| rest.strip_prefix("/v"))
        .filter(|v| !v.is_empty() && v.bytes().all(|b| b.is_ascii_digit()) && !v.starts_with('0'))
        .ok_or_else(other)?;
    match version.parse::<u32>() {
        Ok(v) if v <= own => Ok(v),
        _ => {
            Err(FormatError::Newer { found: found.to_owned(), supported: format!("planroom.{name}/v{own}") })
        }
    }
}

/// Check a document's own `format` field; a document with none names no format.
pub fn check_document(doc: &Value, name: &str) -> Result<u32, FormatError> {
    match doc.get("format").and_then(Value::as_str) {
        Some(found) => check(found, name),
        None => Err(FormatError::Other { found: String::new(), expected: tag(name) }),
    }
}

/// A document type with a format tag: `planroom.<NAME>/v<N>`.
pub trait Document: DeserializeOwned {
    /// The name in its tag, such as `snapshot` or `live-head`.
    const NAME: &'static str;

    /// The tag this version writes.
    fn format_tag() -> String {
        tag(Self::NAME)
    }
}

/// Why a document couldn't be read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReadError {
    /// Its `format` is newer, or names another document.
    Format(FormatError),
    /// Its format is fine but its fields don't fit the type.
    Shape(String),
}

impl std::fmt::Display for ReadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReadError::Format(FormatError::Newer { found, supported }) => {
                write!(f, "{found} is newer than this version reads ({supported})")
            }
            ReadError::Format(FormatError::Other { found, expected }) => {
                write!(f, "format {found:?} isn't {expected}")
            }
            ReadError::Shape(why) => f.write_str(why),
        }
    }
}

/// Read a document: check its format, then decode it, keeping unknown fields in its `extra`.
pub fn read<T: Document>(doc: Value) -> Result<T, ReadError> {
    check_document(&doc, T::NAME).map_err(ReadError::Format)?;
    serde_json::from_value(doc).map_err(|e| ReadError::Shape(e.to_string()))
}

/// `read` from JSON text.
pub fn read_str<T: Document>(text: &str) -> Result<T, ReadError> {
    read(serde_json::from_str(text).map_err(|e| ReadError::Shape(e.to_string()))?)
}

/// Whether an engine at version `own` is older than a file's `min_reader_version`, and so must
/// neither read nor rewrite it (DR-78). Both are semver; precedence follows semver 2.0.0, so a
/// `-dev` build of a version is older than that version. A value that isn't semver is never
/// below `own`: the reader can't tell, so it refuses.
pub fn reader_too_old(min_reader_version: &str, own: &str) -> bool {
    match (Semver::parse(min_reader_version), Semver::parse(own)) {
        (Some(min), Some(own)) => own.cmp(&min) == Ordering::Less,
        _ => true,
    }
}

/// A semver version, without its build metadata, which precedence ignores.
#[derive(Debug, PartialEq, Eq)]
struct Semver<'a> {
    core: [u64; 3],
    pre: Vec<&'a str>,
}

impl<'a> Semver<'a> {
    fn parse(v: &'a str) -> Option<Semver<'a>> {
        let v = v.split_once('+').map_or(v, |(v, _)| v);
        let (core, pre) = v.split_once('-').map_or((v, None), |(c, p)| (c, Some(p)));
        let mut parts = core.split('.').map(|p| p.parse::<u64>().ok());
        let core = [parts.next()??, parts.next()??, parts.next()??];
        if parts.next().is_some() {
            return None;
        }
        let pre: Vec<&str> = pre.map(|p| p.split('.').collect()).unwrap_or_default();
        if pre.iter().any(|i| i.is_empty()) {
            return None;
        }
        Some(Semver { core, pre })
    }
}

impl Ord for Semver<'_> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.core.cmp(&other.core).then_with(|| match (self.pre.is_empty(), other.pre.is_empty()) {
            (true, true) => Ordering::Equal,
            (true, false) => Ordering::Greater,
            (false, true) => Ordering::Less,
            (false, false) => {
                for (a, b) in self.pre.iter().zip(&other.pre) {
                    let order = match (a.parse::<u64>(), b.parse::<u64>()) {
                        (Ok(a), Ok(b)) => a.cmp(&b),
                        (Ok(_), Err(_)) => Ordering::Less,
                        (Err(_), Ok(_)) => Ordering::Greater,
                        (Err(_), Err(_)) => a.cmp(b),
                    };
                    if order != Ordering::Equal {
                        return order;
                    }
                }
                self.pre.len().cmp(&other.pre.len())
            }
        })
    }
}

impl PartialOrd for Semver<'_> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn a_reader_loads_its_own_format_and_older_and_refuses_a_newer_one() {
        assert_eq!(check("planroom.snapshot/v1", "snapshot"), Ok(1));
        assert_eq!(
            check("planroom.snapshot/v2", "snapshot"),
            Err(FormatError::Newer { found: "planroom.snapshot/v2".into(), supported: tag("snapshot") })
        );
        assert!(matches!(
            check("planroom.snapshot/v99999999999", "snapshot"),
            Err(FormatError::Newer { .. })
        ));
        for other in
            ["planroom.live-head/v1", "planroom.snapshot/v0", "planroom.snapshot/v", "snapshot/v1", ""]
        {
            assert!(matches!(check(other, "snapshot"), Err(FormatError::Other { .. })), "{other}");
        }
        assert_eq!(check_against("planroom.snapshot/v1", "snapshot", 2), Ok(1), "a v2 reader loads v1");
        assert_eq!(check_against("planroom.snapshot/v2", "snapshot", 2), Ok(2));
        assert_eq!(check_document(&json!({"format": "planroom.ack/v1"}), "ack"), Ok(1));
        assert!(matches!(check_document(&json!({}), "ack"), Err(FormatError::Other { .. })));
    }

    #[test]
    fn min_reader_version_follows_semver_precedence() {
        assert!(!reader_too_old("0.1.0-dev", "0.1.0-dev"));
        assert!(!reader_too_old("0.1.0-dev", "0.1.0"));
        assert!(reader_too_old("0.1.0", "0.1.0-dev"));
        assert!(reader_too_old("0.2.0", "0.1.9"));
        assert!(!reader_too_old("0.1.0", "0.10.0"));
        assert!(reader_too_old("1.0.0-alpha.10", "1.0.0-alpha.9"));
        assert!(reader_too_old("1.0.0-beta", "1.0.0-alpha.1"));
        assert!(!reader_too_old("1.0.0+build.7", "1.0.0"));
        assert!(reader_too_old("not a version", "0.1.0"));
    }
}
