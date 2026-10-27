//! Records the target triple and the git commit for `version --json`, embeds every built-in pack
//! under `packs/`, and embeds the canvas build, `target/canvas/`, when it exists, so `serve` never
//! depends on where the binary sits (DR-58).
//!
//! Every directory under `packs/` is a built-in pack: a new one needs no code edit, because this
//! script embeds whatever the directory holds, but its tests (`tests/`, the conformance kit that
//! `doctor --pack` reads from the directory itself).
//!
//! The commit is `PLANROOM_GIT_COMMIT` when the build environment sets it, else the checkout's
//! `HEAD`, and none outside a git checkout. The script reruns when `HEAD` or the branch it names
//! moves, so a new commit never leaves a binary naming the one before it.
//!
//! Without a canvas build it watches `target/` itself, whose entries change when
//! `cargo xtask gen --canvas` creates `target/canvas/`, so the next build embeds it.

use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    let target = std::env::var("TARGET").expect("cargo sets TARGET for build scripts");
    println!("cargo:rustc-env=PLANROOM_TARGET={target}");
    println!("cargo:rerun-if-changed=build.rs");

    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("cargo sets CARGO_MANIFEST_DIR"));
    let out_dir = PathBuf::from(std::env::var("OUT_DIR").expect("cargo sets OUT_DIR"));
    if let Some(commit) = git_commit(&manifest) {
        println!("cargo:rustc-env=PLANROOM_GIT_COMMIT={commit}");
    }
    packs(&manifest.join("../../packs"), &out_dir);
    canvas(&manifest.join("../../target"), &out_dir);
}

/// The commit this build is from: the environment's `PLANROOM_GIT_COMMIT`, else `HEAD`.
fn git_commit(manifest: &Path) -> Option<String> {
    println!("cargo:rerun-if-env-changed=PLANROOM_GIT_COMMIT");
    if let Some(commit) = std::env::var("PLANROOM_GIT_COMMIT").ok().filter(|c| !c.trim().is_empty()) {
        return Some(commit.trim().to_owned());
    }
    let git = |args: &[&str]| -> Option<String> {
        let out = Command::new("git").arg("-C").arg(manifest).args(args).output().ok()?;
        out.status.success().then(|| String::from_utf8_lossy(&out.stdout).trim().to_owned())
    };
    let commit = git(&["rev-parse", "--verify", "--quiet", "HEAD"]).filter(|c| !c.is_empty())?;
    // `--git-path` finds each file in a linked worktree too, whose `HEAD` is its own and whose
    // branches sit in the main checkout's git directory.
    let path = |name: &str| git(&["rev-parse", "--path-format=absolute", "--git-path", name]);
    if let Some(head) = path("HEAD") {
        println!("cargo:rerun-if-changed={head}");
        let branch =
            fs::read_to_string(&head).ok().and_then(|h| h.trim().strip_prefix("ref: ").map(str::to_owned));
        if let Some(file) = branch.and_then(|b| path(&b)) {
            println!("cargo:rerun-if-changed={file}");
        }
    }
    if let Some(packed) = path("packed-refs") {
        println!("cargo:rerun-if-changed={packed}");
    }
    Some(commit)
}

/// `OUT_DIR/packs.rs`: every file of every directory under `packs/`, as `(pack directory, path
/// in it, bytes)`, sorted.
fn packs(dir: &Path, out_dir: &Path) {
    println!("cargo:rerun-if-changed={}", dir.display());
    let mut files = Vec::new();
    let mut packs: Vec<PathBuf> = fs::read_dir(dir)
        .map(|entries| entries.flatten().map(|e| e.path()).filter(|p| p.is_dir()).collect())
        .unwrap_or_default();
    packs.sort();
    for pack in &packs {
        let name = pack.file_name().and_then(|n| n.to_str()).expect("a pack directory's name is UTF-8");
        let mut found = Vec::new();
        walk(pack, pack, &mut found);
        found.retain(|(rel, _)| !rel.starts_with("tests/"));
        found.sort();
        files.extend(found.into_iter().map(|(rel, abs)| (name.to_owned(), rel, abs)));
    }
    let mut out = String::from(
        "/// The built-in packs: each file's pack directory under `packs/`, its path there and its bytes.\n",
    );
    out.push_str("pub static FILES: &[(&str, &str, &[u8])] = &[\n");
    for (pack, rel, abs) in &files {
        writeln!(out, "    ({pack:?}, {rel:?}, include_bytes!({abs:?})),").expect("a String takes writes");
    }
    out.push_str("];\n");
    fs::write(out_dir.join("packs.rs"), out).expect("OUT_DIR is writable");
}

/// `OUT_DIR/canvas.rs`: the canvas build's files, when one exists.
fn canvas(target_dir: &Path, out_dir: &Path) {
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
    fs::write(out_dir.join("canvas.rs"), out).expect("OUT_DIR is writable");
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
