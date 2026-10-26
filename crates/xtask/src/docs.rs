//! The docs checks that have data to guard. `every_image_has_its_prompt_and_spend_row`: every image
//! under `docs/art/` has a row in `docs/art/spend.jsonl` naming a prompt file that exists, a row
//! with no image (a `drafts` row) is allowed, and the Iris total stays under KICKOFF's $5
//! (architecture.md, Art spend; DR-65, DR-69).

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use serde_json::Value;

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
