//! The docs checks over the committed pages: every Read first link in an area's `AGENTS.md`
//! resolves, anchor included, and every amended record says so in its Status (repo-layout.md, Read
//! first, per area; decisions.md, How to read a record; DR-79). They read files only, so they need
//! no dependency on xtask.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Component, Path, PathBuf};

use planroom_testkit::repo_root;

const DECISIONS: &str = "docs/contributing/decisions.md";

/// Every `AGENTS.md` under `root`, skipping hidden directories, build outputs and the fixture.
fn agents_files(root: &Path) -> Vec<PathBuf> {
    fn walk(root: &Path, dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(entries) = fs::read_dir(dir) else { return };
        for entry in entries.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().into_owned();
            if path.is_dir() {
                let fixture = path.strip_prefix(root).is_ok_and(|p| p == Path::new("examples/fixture"));
                if !(name.starts_with('.') || name == "target" || name == "node_modules" || fixture) {
                    walk(root, &path, out);
                }
            } else if name == "AGENTS.md" {
                out.push(path);
            }
        }
    }
    let mut out = Vec::new();
    walk(root, root, &mut out);
    out.sort();
    out
}

/// GitHub's heading anchor: lowercase, punctuation other than `-` and `_` dropped, spaces as `-`.
fn slug(heading: &str) -> String {
    heading
        .trim()
        .to_lowercase()
        .chars()
        .filter_map(|c| match c {
            ' ' => Some('-'),
            c if c.is_alphanumeric() || c == '-' || c == '_' => Some(c),
            _ => None,
        })
        .collect()
}

/// The anchors of a Markdown file's headings, outside code fences.
fn anchors(text: &str) -> BTreeSet<String> {
    let mut fenced = false;
    let mut out = BTreeSet::new();
    for line in text.lines() {
        if line.starts_with("```") {
            fenced = !fenced;
        } else if !fenced && line.starts_with('#') {
            let title = line.trim_start_matches('#');
            if title.starts_with(' ') {
                out.insert(slug(title));
            }
        }
    }
    out
}

fn normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in path.components() {
        match c {
            Component::ParentDir => {
                out.pop();
            }
            Component::CurDir => {}
            other => out.push(other),
        }
    }
    out
}

/// The link targets in `text`.
fn links(text: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = text;
    while let Some(at) = rest.find("](") {
        rest = &rest[at + 2..];
        let end = rest.find(')').unwrap_or(rest.len());
        out.push(rest[..end].to_owned());
        rest = &rest[end..];
    }
    out
}

/// Every Read first link, from the line that opens the Read first lines to the end of the generated
/// block, that names no existing file or no heading of it.
fn read_first_problems(root: &Path) -> Vec<String> {
    let mut problems = Vec::new();
    for agents in agents_files(root) {
        let text = fs::read_to_string(&agents).unwrap();
        let shown = agents.strip_prefix(root).unwrap_or(&agents).display().to_string();
        let Some(start) = text.find("Read first, from ") else { continue };
        let end = text[start..].find("<!-- END GENERATED").map_or(text.len(), |e| start + e);
        for target in links(&text[start..end]) {
            if target.starts_with("http") {
                continue;
            }
            let (file, anchor) =
                target.split_once('#').map_or((target.as_str(), None), |(f, a)| (f, Some(a)));
            let path = normalize(&agents.parent().unwrap().join(file));
            let Ok(page) = fs::read_to_string(&path) else {
                problems.push(format!("{shown}: the Read first link `{target}` names no file"));
                continue;
            };
            if let Some(anchor) = anchor
                && !anchors(&page).contains(anchor)
            {
                problems.push(format!("{shown}: the Read first link `{target}` names no heading of {file}"));
            }
        }
    }
    problems
}

/// The round numbers a Status names after "review round" or "review rounds", as in "Amended in
/// review rounds 1, 2 and 3, and by DR-86."
fn status_rounds(status: &str) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for (at, word) in status.match_indices("review round") {
        let rest = status[at + word.len()..].trim_start_matches('s');
        for token in rest.split_whitespace() {
            let number = token.trim_end_matches([',', '.', ';']);
            if number.chars().all(|c| c.is_ascii_digit()) && !number.is_empty() {
                out.insert(number.to_owned());
            } else if token != "and" {
                break;
            }
            if token.ends_with(['.', ';']) {
                break;
            }
        }
    }
    out
}

/// The `DR-n` numbers in `text`.
fn records_named(text: &str) -> BTreeSet<String> {
    text.match_indices("DR-")
        .filter_map(|(at, _)| {
            let digits: String = text[at + 3..].chars().take_while(char::is_ascii_digit).collect();
            (!digits.is_empty()).then_some(digits)
        })
        .collect()
}

/// Each record whose Status misses a round of its "Amended in review round N" bullets, the
/// parenthesised form included, or a record of its "Amended by DR-n" and "Extended by DR-n"
/// bullets. Only that direction: a later record's own "It amends DR-n" line isn't checked.
fn status_problems(decisions: &str) -> Vec<String> {
    let mut problems = Vec::new();
    let mut records: Vec<(String, Vec<&str>)> = Vec::new();
    for line in decisions.lines() {
        if let Some(rest) = line.strip_prefix("## DR-") {
            let number: String = rest.chars().take_while(char::is_ascii_digit).collect();
            records.push((format!("DR-{number}"), Vec::new()));
        } else if let Some((_, lines)) = records.last_mut() {
            lines.push(line);
        }
    }
    for (record, lines) in &records {
        let status = lines.iter().find_map(|l| l.strip_prefix("- **Status:**")).unwrap_or_default();
        let (rounds, named) = (status_rounds(status), records_named(status));
        for line in lines {
            let bullet = line.trim_start();
            if let Some(rest) = bullet.strip_prefix("- **Amended in review round ") {
                let round: String = rest.chars().take_while(char::is_ascii_digit).collect();
                if !rounds.contains(&round) {
                    problems.push(format!(
                        "{record}: amended in review round {round}, which its Status doesn't name"
                    ));
                }
            }
            for lead in ["- **Amended by DR-", "- **Extended by DR-"] {
                if let Some(rest) = bullet.strip_prefix(lead) {
                    let number: String = rest.chars().take_while(char::is_ascii_digit).collect();
                    if !named.contains(&number) {
                        let how = lead[4..lead.len() - 3].trim_end();
                        problems.push(format!("{record}: {how} DR-{number}, which its Status doesn't name"));
                    }
                }
            }
        }
    }
    problems
}

#[test]
fn read_first_names_resolve() {
    let root = repo_root();
    assert!(agents_files(&root).len() >= 20, "every area has an AGENTS.md");
    assert_eq!(read_first_problems(&root), Vec::<String>::new());

    // A planted link to a heading that doesn't exist, and one to a missing page.
    let tmp = tempfile::tempdir().unwrap();
    fs::create_dir_all(tmp.path().join("area")).unwrap();
    fs::create_dir_all(tmp.path().join("docs")).unwrap();
    fs::write(tmp.path().join("docs/page.md"), "# Page\n\n## The fold\n\n```\n# Not a heading\n```\n")
        .unwrap();
    fs::write(
        tmp.path().join("area/AGENTS.md"),
        "# area\n\nRead first, from [Page](../docs/page.md#page):\n\n- **Sections:** [The fold](../docs/page.md#the-fold); [Gone](../docs/page.md#not-a-heading); [Missing](../docs/missing.md).\n\n<!-- END GENERATED -->\n\n[After the block](../docs/elsewhere.md)\n",
    )
    .unwrap();
    assert_eq!(
        read_first_problems(tmp.path()),
        [
            "area/AGENTS.md: the Read first link `../docs/page.md#not-a-heading` names no heading of ../docs/page.md",
            "area/AGENTS.md: the Read first link `../docs/missing.md` names no file",
        ]
    );
}

#[test]
fn an_amended_record_says_so_in_its_status() {
    let decisions = fs::read_to_string(repo_root().join(DECISIONS)).unwrap();
    assert!(decisions.matches("\n## DR-").count() >= 100, "every record is read");
    assert_eq!(status_problems(&decisions), Vec::<String>::new());

    // Planted records: a round, a parenthesised round and a later record the Status misses.
    let planted = "\
## DR-1: One

- **Status:** accepted. Amended in review rounds 1 and 2, and by DR-3.
- **Amended in review round 1:** a.
- **Amended in review round 2 (a fix pass):** b.
- **Amended by DR-3:** c.

## DR-2: Two

- **Status:** accepted. Amended in review round 1.
- **Amended in review round 1:** a.
- **Amended in review round 3 (a fix pass):** b.
- **Extended by DR-4:** c.

## DR-3: Three

- **Status:** accepted. It amends DR-1.
";
    assert_eq!(
        status_problems(planted),
        [
            "DR-2: amended in review round 3, which its Status doesn't name",
            "DR-2: Extended by DR-4, which its Status doesn't name",
        ]
    );
}
