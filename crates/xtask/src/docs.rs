//! The docs checks that have data to guard. `every_image_has_its_prompt_and_spend_row`: every image
//! under `docs/art/` has a row in `docs/art/spend.jsonl` naming a prompt file that exists, a row
//! with no image (a `drafts` row) is allowed, and the Iris total stays under KICKOFF's $5
//! (architecture.md, Art spend; DR-65, DR-69).
//!
//! The generated marks: the Generated directories table in repo-layout.md read as rows, the
//! `GENERATED.md` stubs and `.gitattributes` that `cargo xtask gen` writes from it, and
//! `generated_headers`, which checks every path it names (repo-layout.md, Generated directories;
//! DR-79).

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use serde_json::Value;

use planroom_contract::schema::View;

use crate::generate::{GENERATED_HEADER, GOLDEN_TESTS, Generated, Group};
use crate::schema_guard::BASELINE;
use crate::util;

pub const ART: &str = "docs/art";
pub const ART_SPEND: &str = "docs/art/spend.jsonl";
/// KICKOFF's cap on Iris spend, in dollars.
pub const ART_CAP_USD: f64 = 5.0;

const IMAGE_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "webp", "gif", "svg"];

/// What the art check found: its problems, and the total spend it read.
pub struct ArtSpend {
    pub problems: Vec<String>,
    pub total_usd: f64,
}

pub fn every_image_has_its_prompt_and_spend_row(root: &Path) -> Result<ArtSpend, String> {
    let dir = root.join(ART);
    let mut images = BTreeSet::new();
    for entry in fs::read_dir(&dir).map_err(|e| format!("{ART}: {e}"))? {
        let path = entry.map_err(|e| e.to_string())?.path();
        let ext = path.extension().and_then(|e| e.to_str()).map(str::to_ascii_lowercase);
        if path.is_file() && ext.is_some_and(|e| IMAGE_EXTENSIONS.contains(&e.as_str())) {
            images.insert(path.file_name().and_then(|n| n.to_str()).unwrap_or_default().to_owned());
        }
    }
    let text = fs::read_to_string(root.join(ART_SPEND)).map_err(|e| format!("{ART_SPEND}: {e}"))?;
    let mut problems = Vec::new();
    let mut total = 0.0;
    let mut rows_for = BTreeSet::new();
    for (i, line) in text.lines().enumerate().filter(|(_, l)| !l.trim().is_empty()) {
        let at = format!("{ART_SPEND}:{}", i + 1);
        let row: Value = match serde_json::from_str(line) {
            Ok(v) => v,
            Err(e) => {
                problems.push(format!("{at}: not a JSON object: {e}"));
                continue;
            }
        };
        let image = row["image"].as_str().unwrap_or_default();
        match row["cost_usd"].as_f64() {
            Some(c) if c >= 0.0 => total += c,
            _ => problems.push(format!("{at}: cost_usd must be a number of dollars, 0 or more")),
        }
        for key in ["at", "model"] {
            if row[key].as_str().is_none_or(str::is_empty) {
                problems.push(format!("{at}: needs `{key}`"));
            }
        }
        match row["prompt_path"].as_str() {
            Some(p) if root.join(p).is_file() => {}
            Some(p) => problems.push(format!("{at}: prompt_path {p} doesn't exist")),
            None if images.contains(image) => {
                problems.push(format!("{at}: the row for {image} names no prompt_path"))
            }
            None => {}
        }
        rows_for.insert(image.to_owned());
    }
    for image in &images {
        if !rows_for.contains(image) {
            problems.push(format!("{ART}/{image} has no row in {ART_SPEND}"));
        }
    }
    if total >= ART_CAP_USD {
        problems.push(format!("{ART_SPEND} totals ${total:.2}, not under the ${ART_CAP_USD:.0} cap"));
    }
    Ok(ArtSpend { problems, total_usd: total })
}

/// Where `boundaries.toml`'s `generated_read_only` must point: the Generated directories table, the
/// one list of what `cargo xtask gen` writes (repo-layout.md, Generated directories; DR-79).
pub const GENERATED_TABLE: &str = "docs/contributing/repo-layout.md#generated-directories";
/// The stub in each wholly generated directory.
pub const STUB: &str = "GENERATED.md";
pub const GITATTRIBUTES: &str = ".gitattributes";
/// The hand-written files a generated directory may also hold: its area contract.
const AREA_FILES: [&str; 2] = ["AGENTS.md", "CLAUDE.md"];

/// What a Generated directories row names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shape {
    /// A wholly generated directory, which holds a `GENERATED.md` stub.
    Directory,
    /// A generated file in a hand-written directory, which carries the header.
    File,
    /// A region of a hand-written file, between BEGIN and END GENERATED markers.
    Region,
    /// A value inside a file, which its drift check covers.
    Field,
}

/// One path of a row, brace lists expanded, and the milestone that builds it when the cell tags it
/// `(Mn)` right after it, so it may be missing until then.
#[derive(Debug, Clone)]
pub struct GeneratedPath {
    pub path: String,
    pub due: Option<String>,
}

/// One row of the Generated directories table.
#[derive(Debug, Clone)]
pub struct GeneratedRow {
    pub shape: Shape,
    pub paths: Vec<GeneratedPath>,
    pub committed: bool,
    /// The first `cargo xtask` command the Generator cell names.
    pub generator: String,
    /// The conflict cell, its trailing record tags left out.
    pub conflict: String,
}

/// The `(Mn)` tag at the start of `text`, after spaces, as `Mn`.
pub fn leading_tag(text: &str) -> Option<String> {
    let inner = text.trim_start().strip_prefix("(M")?;
    let end = inner.find(')')?;
    (end > 0 && inner[..end].chars().all(|c| c.is_ascii_digit())).then(|| format!("M{}", &inner[..end]))
}

/// The cell's code spans, each with the text that follows it up to the next span.
fn spans_with_after(cell: &str) -> Vec<(String, String)> {
    let parts: Vec<&str> = cell.split('`').collect();
    (1..parts.len())
        .step_by(2)
        .map(|i| (parts[i].to_owned(), parts.get(i + 1).copied().unwrap_or_default().to_owned()))
        .collect()
}

/// A code span names a path when it holds a `/` or a `.` and no space.
fn is_path_span(span: &str) -> bool {
    (span.contains('/') || span.contains('.')) && !span.contains(' ')
}

/// The text of a cell without its trailing record tags, such as ` [DR-59]`.
fn without_records(cell: &str) -> String {
    let mut text = cell.trim().to_owned();
    while text.ends_with(']')
        && let Some(open) = text.rfind(" [")
    {
        text.truncate(open);
    }
    text
}

/// The table `generated_read_only` names, as rows (repo-layout.md, Generated directories).
pub fn generated_table(root: &Path) -> Result<Vec<GeneratedRow>, String> {
    let rules = crate::boundaries::load(root)?;
    let (file, anchor) = rules
        .ts
        .rules
        .generated_read_only
        .split_once('#')
        .ok_or_else(|| format!("boundaries.toml: generated_read_only must be {GENERATED_TABLE}"))?;
    let text = fs::read_to_string(root.join(file)).map_err(|e| format!("{file}: {e}"))?;
    let heading = text
        .lines()
        .find(|l| l.starts_with("## ") && crate::areas::slug(l.trim_start_matches('#')) == anchor)
        .ok_or_else(|| format!("{file} has no heading for #{anchor}"))?;
    let mut out = Vec::new();
    for row in crate::areas::table_rows(&text, heading).iter().skip(1) {
        let [path, generator, committed, conflict] = row.as_slice() else {
            return Err(format!("{file}#{anchor}: a row has {} cells, not 4: {row:?}", row.len()));
        };
        let words: Vec<&str> = path.split(|c: char| !c.is_alphanumeric()).collect();
        let mut paths = Vec::new();
        for (span, after) in spans_with_after(path) {
            if is_path_span(&span) {
                let due = leading_tag(&after);
                paths.extend(
                    util::expand_braces(&span)
                        .into_iter()
                        .map(|p| GeneratedPath { path: p, due: due.clone() }),
                );
            }
        }
        if paths.is_empty() {
            return Err(format!("{file}#{anchor}: the row {path:?} names no path in a code span"));
        }
        let shape = if words.contains(&"marked") {
            Shape::Region
        } else if words.contains(&"field") {
            Shape::Field
        } else if paths.iter().all(|p| p.path.ends_with('/')) {
            Shape::Directory
        } else if paths.iter().any(|p| p.path.ends_with('/')) {
            return Err(format!("{file}#{anchor}: the row {path:?} mixes directories and files"));
        } else {
            Shape::File
        };
        let committed = match committed.split_whitespace().next().map(|w| w.trim_end_matches(',')) {
            Some("yes") => true,
            Some("no") => false,
            _ => {
                return Err(format!(
                    "{file}#{anchor}: the row {path:?} has a Committed cell that starts with neither yes nor no"
                ));
            }
        };
        let generator = spans_with_after(generator)
            .into_iter()
            .map(|(s, _)| s)
            .find(|s| s.starts_with("cargo xtask"))
            .unwrap_or_else(|| "cargo xtask gen".to_owned());
        out.push(GeneratedRow { shape, paths, committed, generator, conflict: without_records(conflict) });
    }
    if out.is_empty() {
        return Err(format!("{file}#{anchor}: no table"));
    }
    Ok(out)
}

/// `GENERATED.md` for one generated directory: the header, what writes the directory and what to
/// do on a merge conflict.
/// Its text leaves out the area's own files and each nested generated directory, which has a stub
/// of its own.
fn stub(root: &Path, dir: &str, row: &GeneratedRow, nested: &[&str]) -> String {
    let mut exempt: Vec<String> =
        AREA_FILES.iter().filter(|f| root.join(dir).join(f).is_file()).map(|f| format!("`{f}`")).collect();
    exempt.extend(nested.iter().map(|n| format!("`{}`", &n[dir.len()..])));
    let but = match exempt.split_last() {
        None => String::new(),
        Some((last, [])) => format!(", except {last}"),
        Some((last, rest)) => format!(", except {} and {last}", rest.join(", ")),
    };
    let but = match nested.len() {
        0 => but,
        1 => format!("{but}, which has a stub of its own"),
        _ => format!("{but}, which have stubs of their own"),
    };
    let c = &row.conflict;
    let conflict = if c.starts_with('`') && c.ends_with('`') && c.matches('`').count() == 2 {
        format!("take either side and run {c}")
    } else {
        c.clone()
    };
    format!(
        "<!-- {GENERATED_HEADER} -->\n\n# Generated directory\n\n`{}` writes every file in `{dir}` and below it{but}. Don't edit one by hand; run the generator instead. On a merge conflict in one, {conflict}. The Generated directories table in `docs/contributing/repo-layout.md` lists every generated path.\n",
        row.generator,
    )
}

/// The committed wholly generated directories the table names, each ending `/`.
fn directory_rows(rows: &[GeneratedRow]) -> Vec<&str> {
    rows.iter()
        .filter(|r| r.committed && r.shape == Shape::Directory)
        .flat_map(|r| r.paths.iter().map(|p| p.path.as_str()))
        .collect()
}

/// Every file a generator writes: the groups `cargo xtask gen` builds, given, the files the golden
/// tests own, which `gen --golden` writes, and the two views `gen --schema-baseline` copies.
pub fn generator_outputs(groups: &[Group]) -> BTreeSet<String> {
    let mut out: BTreeSet<String> =
        groups.iter().flat_map(|g| g.files.iter().map(|f| f.path.clone())).collect();
    out.extend(GOLDEN_TESTS.iter().flat_map(|(_, _, files)| files.iter().map(|f| (*f).to_owned())));
    out.extend(View::ALL.iter().map(|v| format!("{BASELINE}/{}", v.file_name())));
    out
}

/// `.gitattributes`: each committed generated file and directory marked `linguist-generated`, so
/// diffs collapse them. Regions and fields sit in hand-written files and are left out.
fn gitattributes(rows: &[GeneratedRow]) -> String {
    let mut out = format!(
        "# {GENERATED_HEADER}\n# Source: the Generated directories table in docs/contributing/repo-layout.md.\n"
    );
    for row in rows.iter().filter(|r| r.committed) {
        for p in &row.paths {
            match row.shape {
                Shape::Directory => out.push_str(&format!("{}** linguist-generated=true\n", p.path)),
                Shape::File => out.push_str(&format!("{} linguist-generated=true\n", p.path)),
                Shape::Region | Shape::Field => {}
            }
        }
    }
    out
}

/// The generated marks: a `GENERATED.md` stub in each committed generated directory that exists,
/// and `.gitattributes`, both from the Generated directories table.
pub fn generated_marks(root: &Path) -> Result<Group, String> {
    let rows = generated_table(root)?;
    let dirs = directory_rows(&rows);
    let mut files = Vec::new();
    for row in rows.iter().filter(|r| r.committed && r.shape == Shape::Directory) {
        for p in row.paths.iter().filter(|p| root.join(&p.path).is_dir()) {
            let nested: Vec<&str> =
                dirs.iter().copied().filter(|d| d.len() > p.path.len() && d.starts_with(&p.path)).collect();
            files.push(Generated {
                path: format!("{}{STUB}", p.path),
                contents: stub(root, &p.path, row, &nested),
            });
        }
    }
    files.push(Generated { path: GITATTRIBUTES.into(), contents: gitattributes(&rows) });
    Ok(Group { name: "GENERATED.md stubs and .gitattributes", files })
}

/// Whether a generated file carries the header: in its first 512 bytes, or for JSON in a
/// top-level `$comment`.
fn has_header(path: &Path) -> bool {
    let Ok(bytes) = fs::read(path) else { return false };
    if path.extension().is_some_and(|e| e == "json") {
        return serde_json::from_slice::<Value>(&bytes)
            .ok()
            .and_then(|v| v["$comment"].as_str().map(|c| c.contains(GENERATED_HEADER)))
            .unwrap_or(false);
    }
    String::from_utf8_lossy(&bytes[..bytes.len().min(512)]).contains(GENERATED_HEADER)
}

/// `generated_headers`: every committed path the Generated directories table names is marked as
/// generated. A directory holds its `GENERATED.md` stub, which covers every file below it but the
/// area's `AGENTS.md` and `CLAUDE.md`; a whole file carries the header; a region with a literal path
/// sits between BEGIN and END GENERATED markers. A missing path fails unless its cell tags it with
/// the milestone that builds it, and a tagged path that exists fails, so the change that creates it
/// drops the tag. A file in a generated directory that no generator writes fails too, since no drift
/// check compares it; `written` holds every path a generator writes ([`generator_outputs`]), and
/// `None` leaves that part out when a generator failed and its drift step says so. Whether each
/// written file equals a fresh generation is the drift checks' part (DR-79).
pub fn generated_headers(root: &Path, written: Option<&BTreeSet<String>>) -> Result<Vec<String>, String> {
    let rows = generated_table(root)?;
    let listed = util::listed_files(root)?;
    let dirs = directory_rows(&rows);
    // The files each directory's own stub covers: those below it, less the area's files beside
    // the stub and those under a nested directory row, which has a stub of its own.
    let covered = |dir: &str| -> Vec<&String> {
        listed
            .iter()
            .filter(|f| dirs.iter().filter(|d| f.starts_with(*d)).max_by_key(|d| d.len()) == Some(&dir))
            .filter(|f| !AREA_FILES.contains(&&f[dir.len()..]) && f[dir.len()..] != *STUB)
            .collect()
    };
    let mut problems = Vec::new();
    for row in rows.iter().filter(|r| r.committed) {
        for p in &row.paths {
            let pattern = util::is_pattern(&p.path);
            let matches: Vec<&String> =
                listed.iter().filter(|f| pattern && util::path_matches(&p.path, f)).collect();
            let exists = if pattern { !matches.is_empty() } else { root.join(&p.path).exists() };
            match (&p.due, exists) {
                (None, false) => {
                    problems.push(format!(
                        "{}: the Generated directories table names it, but nothing is there; run `{}`, or tag the path in the table with the milestone that builds it, as `(M2)`",
                        p.path, row.generator
                    ));
                    continue;
                }
                (Some(_), false) => continue,
                (Some(m), true) => problems.push(format!(
                    "{}: exists, but the Generated directories table still tags it ({m}); drop the tag",
                    p.path
                )),
                (None, true) => {}
            }
            let files: Vec<&str> =
                if pattern { matches.iter().map(|s| s.as_str()).collect() } else { vec![&p.path] };
            match row.shape {
                Shape::Directory => {
                    let strays =
                        covered(&p.path).into_iter().filter(|f| written.is_some_and(|w| !w.contains(*f)));
                    for f in strays {
                        problems.push(format!(
                            "{f}: sits in the generated directory `{}`, but `{}` doesn't write it, so no drift check compares it; delete it, or make the generator write it and run `{}`",
                            p.path, row.generator, row.generator
                        ));
                    }
                    let stub = format!("{}{STUB}", p.path);
                    if !has_header(&root.join(&stub)) {
                        let unmarked = covered(&p.path);
                        problems.push(format!(
                            "{stub}: missing, or without the GENERATED header, so nothing marks the {} generated file{} in `{}` as generated; run `cargo xtask gen`",
                            unmarked.len(),
                            if unmarked.len() == 1 { "" } else { "s" },
                            p.path
                        ));
                    }
                }
                Shape::File => {
                    for f in files.into_iter().filter(|f| !has_header(&root.join(f))) {
                        problems.push(format!(
                            "{f}: no GENERATED header in its first 512 bytes (in a JSON file, in a top-level `$comment`); regenerate it with `{}`",
                            row.generator
                        ));
                    }
                }
                Shape::Region if !pattern => {
                    let text = fs::read_to_string(root.join(&p.path)).unwrap_or_default();
                    if !(text.contains("BEGIN GENERATED") && text.contains("END GENERATED")) {
                        problems.push(format!(
                            "{}: its generated region has no BEGIN GENERATED and END GENERATED markers; regenerate it with `{}`",
                            p.path, row.generator
                        ));
                    }
                }
                Shape::Region | Shape::Field => {}
            }
        }
    }
    problems.sort();
    problems.dedup();
    Ok(problems)
}
