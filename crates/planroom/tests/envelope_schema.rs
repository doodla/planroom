//! The envelope's documented contract against the code: contracts.md's error and warning tables
//! and the registries in `contract::errors` (S15).

use std::collections::BTreeMap;

use planroom_contract::{ErrorCode, WarningCode};
use planroom_testkit::repo_root;

/// The rows of the first table after `heading` in contracts.md, each as its cells, keyed by the
/// code in the first cell.
fn table_after(heading: &str) -> BTreeMap<String, Vec<String>> {
    let text = std::fs::read_to_string(repo_root().join("docs/contributing/contracts.md")).unwrap();
    let start =
        text.find(&format!("\n{heading}\n")).unwrap_or_else(|| panic!("contracts.md has `{heading}`"));
    let mut rows = BTreeMap::new();
    let mut in_table = false;
    for line in text[start..].lines().skip(2) {
        if !line.starts_with('|') {
            if in_table {
                break;
            }
            continue;
        }
        in_table = true;
        let cells: Vec<String> = line.trim_matches('|').split(" | ").map(|c| c.trim().to_owned()).collect();
        let Some(code) = cells[0].strip_prefix('`').and_then(|c| c.strip_suffix('`')) else {
            continue; // the header and its rule
        };
        assert!(rows.insert(code.to_owned(), cells.clone()).is_none(), "`{code}` is documented twice");
    }
    assert!(!rows.is_empty(), "no table after `{heading}`");
    rows
}

/// A registry value as it appears on the wire, such as `not_found` or `after_reread`.
fn wire<T: serde::Serialize>(value: T) -> String {
    serde_json::to_value(value).unwrap().as_str().unwrap().to_owned()
}

/// The codes only a person can clear: each has retry `never` and names, in its `details` or its
/// `hint`, the card or the command a person can use (DR-66).
const PERSON_CLEARS: &[ErrorCode] = &[
    ErrorCode::PersonRequired,
    ErrorCode::PersonKeyMismatch,
    ErrorCode::ConsentRequired,
    ErrorCode::RepoConfigRefused,
];

#[test]
fn the_documented_error_table_is_the_error_codes() {
    let errors = table_after("### Error codes");
    let registry: BTreeMap<String, ErrorCode> =
        ErrorCode::ALL.iter().map(|c| (c.as_str().to_owned(), *c)).collect();
    let documented: Vec<&String> = errors.keys().collect();
    let registered: Vec<&String> = registry.keys().collect();
    assert_eq!(documented, registered, "contracts.md's error table and contract::ErrorCode list other codes");

    for (name, cells) in &errors {
        let code = registry[name];
        let spec = code.spec();
        assert_eq!(cells.len(), 6, "`{name}`'s row has six cells: {cells:?}");
        assert_eq!(cells[1], wire(spec.category), "`{name}`'s category");
        assert_eq!(cells[2], code.exit_code().to_string(), "`{name}`'s exit code");
        // A retry cell may say more after its class, such as `later, with retry_after_ms 5000`.
        let retry = cells[3].split([',', ' ']).next().unwrap();
        assert_eq!(retry, wire(spec.retry), "`{name}`'s retry class");
    }

    for code in PERSON_CLEARS {
        let cells = &errors[code.as_str()];
        assert_eq!(wire(code.retry()), "never", "`{}` only a person can clear", code.as_str());
        let details = &cells[5];
        assert!(
            ["`card`", "`command`", "`hint` names"].iter().any(|named| details.contains(named)),
            "`{}` names the card or command a person can use: {details}",
            code.as_str()
        );
    }

    let warnings = table_after("### Warning codes");
    let documented: Vec<&String> = warnings.keys().collect();
    let mut registered: Vec<&str> = WarningCode::ALL.iter().map(|c| c.as_str()).collect();
    registered.sort_unstable();
    assert_eq!(
        documented, registered,
        "contracts.md's warning table and contract::WarningCode list other codes"
    );
}
