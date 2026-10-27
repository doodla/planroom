//! The string catalog: every user-facing string comes from `text/en.toml` (DR-21).

use std::sync::OnceLock;

const EN: &str = include_str!("../text/en.toml");

fn catalog() -> &'static toml::Table {
    static CATALOG: OnceLock<toml::Table> = OnceLock::new();
    CATALOG.get_or_init(|| EN.parse().expect("text/en.toml parses"))
}

/// The string at `section.key`, with each `{name}` replaced by its value, display-sanitized, so a
/// repo-derived value can't put a control or a bidi character into an engine line (DR-64). The
/// template is read once, left to right, and a value is written as it is, never read again, so a
/// repo named `{commit}` stays `{commit}` in the line instead of taking another value's place.
/// A `{name}` with no value stays as written.
pub fn get(key: &str, vars: &[(&str, &dyn std::fmt::Display)]) -> String {
    let (section, name) = key.split_once('.').expect("keys are section.name");
    let template = catalog()
        .get(section)
        .and_then(|s| s.get(name))
        .and_then(|v| v.as_str())
        .unwrap_or_else(|| panic!("text/en.toml has no {key}"));
    let mut out = String::with_capacity(template.len());
    let mut rest = template;
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        let value = after
            .find('}')
            .and_then(|close| vars.iter().find(|(var, _)| *var == &after[..close]).map(|(_, v)| (close, v)));
        match value {
            Some((close, value)) => {
                out.push_str(&planroom_kernel::display::sanitize(&value.to_string()));
                rest = &after[close + 1..];
            }
            None => {
                out.push('{');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    // One line, always.
    out.replace(['\n', '\r'], " ")
}

/// The string at `section.key`, when the catalog has one.
pub fn try_get(key: &str) -> Option<String> {
    let (section, name) = key.split_once('.')?;
    catalog().get(section)?.get(name)?.as_str().map(str::to_owned)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn templates_fill_their_names() {
        assert_eq!(get("hint.workspace_not_found", &[]), "planroom scan DIR");
        assert_eq!(get("frame.version", &[("label", &"v1")]), "Tagged v1.");
        assert!(try_get("error.store_busy").is_some());
        assert!(try_get("error.nope").is_none());
    }

    #[test]
    fn a_value_that_holds_a_placeholder_is_written_as_it_is() {
        let line = get("frame.scan", &[("repo", &"{commit}"), ("commit", &"0b305cb")]);
        assert_eq!(line, "Scanned {commit} at 0b305cb.");
        let line = get("frame.scan", &[("repo", &"orders"), ("commit", &"{repo}")]);
        assert_eq!(line, "Scanned orders at {repo}.");
        // A placeholder with no value, and a lone brace, stay as written.
        assert_eq!(get("frame.scan", &[("repo", &"a{b")]), "Scanned a{b at {commit}.");
    }
}
