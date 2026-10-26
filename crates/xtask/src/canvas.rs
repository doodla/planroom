//! The canvas build, `cargo xtask gen --canvas`: `vite build` of `packages/canvas` into
//! `target/canvas/`, with relative paths and stable file names, which `crates/planroom/build.rs`
//! embeds and `share` publishes. It isn't committed.

use std::path::Path;

use crate::util;

pub const OUT: &str = "target/canvas";

/// Build the canvas and return the files it wrote, relative to the repo root.
pub fn build(root: &Path) -> Result<Vec<String>, String> {
    // The config imports the design tokens from @planroom/ui's TypeScript source, which only Vite's
    // module runner can load.
    util::capture(
        &root.join("packages/canvas"),
        "pnpm",
        &["exec", "vite", "build", "--configLoader", "runner", "--logLevel", "warn"],
    )
    .map_err(|e| format!("{e}\n(install the JavaScript tools with: pnpm install --frozen-lockfile)"))?;
    let mut files = Vec::new();
    util::walk(&root.join(OUT), &mut |p| files.push(util::rel(root, p)))?;
    files.sort();
    if !files.iter().any(|f| f.ends_with("index.html")) {
        return Err(format!("vite build wrote no {OUT}/index.html"));
    }
    Ok(files)
}
