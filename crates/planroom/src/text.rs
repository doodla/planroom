//! The string catalog: every user-facing string comes from `text/en.toml` (DR-21).

use std::sync::OnceLock;

const EN: &str = include_str!("../text/en.toml");

fn catalog() -> &'static toml::Table {
    static CATALOG: OnceLock<toml::Table> = OnceLock::new();
    CATALOG.get_or_init(|| EN.parse().expect("text/en.toml parses"))
}

/// The string at `section.key`, with each `{name}` replaced by its value.
pub fn get(key: &str, vars: &[(&str, &dyn std::fmt::Display)]) -> String {
    let (section, name) = key.split_once('.').expect("keys are section.name");
    let template = catalog()
        .get(section)
        .and_then(|s| s.get(name))
        .and_then(|v| v.as_str())
        .unwrap_or_else(|| panic!("text/en.toml has no {key}"));
    let mut out = template.to_owned();
    for (var, value) in vars {
        out = out.replace(&format!("{{{var}}}"), &value.to_string());
    }
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
}
