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

/// The catalog keys a label reads: the short action on a handle, which a surface puts on the
/// button beside an engine line, or a fallback pane's button. A handle is never masked (AGENTS.md,
/// Logging), so a label names no repo-derived value: the line beside it says what it acts on.
pub const LABELS: &[&str] = &[
    "argument.schema_label",
    "card.apply_accept",
    "card.apply_item",
    "card.apply_reject",
    "edit.undo_label",
    "ask.next_page",
    "ask.run",
    "ask.show",
    "ask.show_level",
    "ask.show_lens",
    "ask.why",
    "empty.check_again_next",
    "empty.lens_shows_nothing_next",
    "empty.no_answer_next",
    "empty.not_scanned_next",
    "empty.nothing_after_cursor_next",
    "empty.nothing_mapped_next",
    "empty.nothing_under_next",
    "empty.top_next",
    "fallback.build_offered_action",
    "fallback.building_action",
    "fallback.failed_action",
    "fallback.fetching_action",
    "fallback.no_focus_action",
    "fallback.person_key_mismatch_action",
    "fallback.resolving_action",
    "fallback.version_mismatch_action",
    "find.inside",
    "find.narrower",
    "find.show",
    "import.map_label",
    "map.coarser",
    "map.pane",
    "open.share_label",
    "scan.next_label",
    "schema.commands_label",
    "schema.def_label",
    "share.unavailable",
];

/// The values a label may name: engine-authored counts, levels, catalogue leaves and the contract
/// schema's type names.
pub const LABEL_VARS: &[&str] = &["count", "level", "leaf", "def"];

/// The label at `section.key`, one of [`LABELS`], filled as [`get`] fills a line from values
/// [`LABEL_VARS`] names.
pub fn label(key: &str, vars: &[(&str, &dyn std::fmt::Display)]) -> String {
    assert!(LABELS.contains(&key), "{key} isn't in text::LABELS");
    assert!(vars.iter().all(|(name, _)| LABEL_VARS.contains(name)), "{key} takes a value a label can't name");
    get(key, vars)
}

/// `text` trimmed, and past `chars` characters, its prefix and an ellipsis: how a line quotes
/// text it doesn't bound, such as a search or a repo's name, so the line stays short.
pub fn short(text: &str, chars: usize) -> String {
    let text = text.trim();
    match text.char_indices().nth(chars) {
        Some((cut, _)) => format!("{}…", text[..cut].trim_end()),
        None => text.to_owned(),
    }
}

/// `word` as one word of a POSIX shell command line: as it is when every character is plainly
/// safe, else in single quotes with each `'` written `'\''`. Hand-rolled: a few lines, where a
/// crate would add a dependency for one function.
pub fn shell_word(word: &str) -> String {
    let plain = |c: char| c.is_ascii_alphanumeric() || "_./:@%+=,-".contains(c);
    if !word.is_empty() && word.chars().all(plain) {
        word.to_owned()
    } else {
        format!("'{}'", word.replace('\'', "'\\''"))
    }
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

    /// The `{name}`s a template takes.
    fn placeholders(template: &str) -> Vec<&str> {
        template.split('{').skip(1).filter_map(|rest| rest.split_once('}').map(|(name, _)| name)).collect()
    }

    #[test]
    fn no_label_takes_repo_text() {
        for key in LABELS {
            let template = try_get(key).unwrap_or_else(|| panic!("text/en.toml has no {key}"));
            for name in placeholders(&template) {
                assert!(
                    LABEL_VARS.contains(&name),
                    "{key} = {template:?} names {{{name}}}, which may be repo text"
                );
            }
        }
        // Every key named as a label is listed, so the check above reads it.
        for (section, table) in catalog() {
            for key in table.as_table().into_iter().flat_map(|t| t.keys()) {
                let key = format!("{section}.{key}");
                if ["_next", "_label", "_action"].iter().any(|s| key.ends_with(s)) {
                    assert!(LABELS.contains(&key.as_str()), "{key} reads like a label: list it in LABELS");
                }
            }
        }
        // Every handle the engine builds reads its label through `label`, which takes only those keys.
        let needle = ["label:", "text::get("].concat();
        let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
        let mut dirs = vec![src];
        while let Some(dir) = dirs.pop() {
            for entry in std::fs::read_dir(&dir).unwrap() {
                let path = entry.unwrap().path();
                if path.is_dir() {
                    dirs.push(path);
                } else if path.extension().is_some_and(|e| e == "rs") {
                    let code = std::fs::read_to_string(&path).unwrap();
                    let code: String = code.split_whitespace().collect();
                    assert!(!code.contains(&needle), "{}: a label read with text::get", path.display());
                }
            }
        }
    }

    #[test]
    fn a_comment_that_names_a_command_sits_above_its_table() {
        // A comment opening with a command in backticks, such as `find TEXT`, describes the table
        // of that name, so the next table after it is that one.
        let tables: Vec<&str> = catalog().keys().map(String::as_str).collect();
        let lines: Vec<&str> = EN.lines().collect();
        for (n, line) in lines.iter().enumerate() {
            let Some(named) = line.strip_prefix("# `").and_then(|r| r.split([' ', '`']).next()) else {
                continue;
            };
            if !tables.contains(&named) {
                continue;
            }
            let next = lines[n + 1..].iter().find(|l| l.starts_with('[')).copied().unwrap_or_default();
            assert_eq!(next, format!("[{named}]"), "line {}: {line}", n + 1);
        }
    }

    #[test]
    fn every_bound_that_refuses_a_file_has_its_warning_line() {
        use planroom_contract::PartialReason;
        // Listed through a match, so a new bound can't be left out of this list.
        let every = |r: PartialReason| match r {
            PartialReason::NestingLimit | PartialReason::WorkLimit | PartialReason::RepoLimit => r,
        };
        for reason in
            [PartialReason::NestingLimit, PartialReason::WorkLimit, PartialReason::RepoLimit].map(every)
        {
            let bound = serde_json::to_value(reason).unwrap();
            let key = format!("warning.unparsed_files_{}", bound.as_str().unwrap());
            let line = try_get(&key).unwrap_or_else(|| panic!("text/en.toml has no {key}"));
            assert!(line.contains("{count}") && line.contains("{repo}"), "{key}: {line}");
        }
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
