//! Records the target triple for `version --json`, and embeds the canvas build, `target/canvas/`,
//! when it exists, so `serve` never depends on where the binary sits (DR-58).
//!
//! Without a canvas build it watches `target/` itself, whose entries change when
//! `cargo xtask gen --canvas` creates `target/canvas/`, so the next build embeds it.

use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    let target = std::env::var("TARGET").expect("cargo sets TARGET for build scripts");
    println!("cargo:rustc-env=PLANROOM_TARGET={target}");
    println!("cargo:rerun-if-changed=build.rs");

    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR"));
    let target_dir = manifest.join("../../target");
    let canvas = target_dir.join("canvas");
    let mut files = Vec::new();
    if canvas.join("index.html").is_file() {
        println!("cargo:rerun-if-changed={}", canvas.display());
        walk(&canvas, &canvas, &mut files);
    } else if target_dir.is_dir() {
        println!("cargo:rerun-if-changed={}", target_dir.display());
    }
    files.sort();
    let mut out =
        String::from("/// The canvas build: each file's path under `target/canvas/` and its bytes.\n");
    out.push_str("pub static FILES: &[(&str, &[u8])] = &[\n");
    for (rel, abs) in &files {
        writeln!(out, "    ({rel:?}, include_bytes!({abs:?})),").expect("a String takes writes");
    }
    out.push_str("];\n");
    let dest = PathBuf::from(std::env::var("OUT_DIR").expect("cargo sets OUT_DIR")).join("canvas.rs");
    fs::write(dest, out).expect("OUT_DIR is writable");
}

fn walk(base: &Path, dir: &Path, files: &mut Vec<(String, String)>) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk(base, &path, files);
        } else if let (Ok(rel), Some(abs)) = (path.strip_prefix(base), path.canonicalize().ok()) {
            files.push((rel.to_string_lossy().replace('\\', "/"), abs.display().to_string()));
        }
    }
}
