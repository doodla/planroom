//! The docs checks with data to guard (architecture.md, Art spend; DR-65).

use std::fs;
use std::path::{Path, PathBuf};

use xtask::docs::every_image_has_its_prompt_and_spend_row as check;

fn repo() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// A temporary root holding `docs/art/` with the given files.
fn art(name: &str, files: &[(&str, &str)]) -> PathBuf {
    let root = std::env::temp_dir().join(format!("planroom-art-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(root.join("docs/art")).unwrap();
    for (path, text) in files {
        fs::write(root.join("docs/art").join(path), text).unwrap();
    }
    root
}

const ROW: &str = r#"{"at": "2026-10-25T00:00:00Z", "image": "logo.png", "model": "m", "cost_usd": 0.05, "prompt_path": "docs/art/logo.prompt.txt"}"#;

#[test]
fn every_image_has_its_prompt_and_spend_row() {
    let found = check(&repo()).unwrap();
    assert!(found.problems.is_empty(), "{:#?}", found.problems);
    assert!(found.total_usd > 0.0 && found.total_usd < 5.0, "{}", found.total_usd);
}

#[test]
fn an_image_without_a_row_fails() {
    let root = art(
        "norow",
        &[("logo.png", "png"), ("hero.png", "png"), ("logo.prompt.txt", "p"), ("spend.jsonl", ROW)],
    );
    let found = check(&root).unwrap();
    assert_eq!(found.problems, ["docs/art/hero.png has no row in docs/art/spend.jsonl"]);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_missing_prompt_fails_and_a_drafts_row_passes() {
    let drafts = r#"{"at": "2026-10-25T00:00:00Z", "image": "drafts", "model": "m", "cost_usd": 0.1, "prompt_path": null}"#;
    let root = art("prompt", &[("logo.png", "png"), ("spend.jsonl", &format!("{ROW}\n{drafts}\n"))]);
    let found = check(&root).unwrap();
    assert_eq!(found.problems.len(), 1, "{:?}", found.problems);
    assert!(found.problems[0].contains("prompt_path docs/art/logo.prompt.txt doesn't exist"));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn a_total_at_the_cap_fails() {
    let big = ROW.replace("0.05", "5.0");
    let root = art("cap", &[("logo.png", "png"), ("logo.prompt.txt", "p"), ("spend.jsonl", &big)]);
    let found = check(&root).unwrap();
    assert!(found.problems.iter().any(|p| p.contains("not under the $5 cap")), "{:?}", found.problems);
    fs::remove_dir_all(root).unwrap();
}
