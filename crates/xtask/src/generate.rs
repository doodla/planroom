//! `cargo xtask gen`: every generated file, built in memory so `check` can compare byte for byte.

use std::fs;
use std::path::Path;

pub use planroom_contract::schema::GENERATED_HEADER;
use planroom_contract::schema::{self, View};

use crate::boundaries;
use crate::util;

/// One generated file: its path from the repo root and its bytes.
pub struct Generated {
    pub path: String,
    pub contents: String,
}

/// A group of generated files that one drift check covers.
pub struct Group {
    pub name: &'static str,
    pub files: Vec<Generated>,
}

/// Every generated file, grouped by drift check.
pub fn all(root: &Path) -> Vec<Result<Group, String>> {
    vec![schemas(), typescript(root), dependency_cruiser(root)]
}

fn schemas() -> Result<Group, String> {
    let files = View::ALL
        .iter()
        .map(|&view| Generated {
            path: format!("schema/{}", view.file_name()),
            contents: schema::render(view),
        })
        .collect();
    Ok(Group { name: "schema/", files })
}

/// `json2ts` over each schema view, fed the in-memory schema on stdin.
fn typescript(root: &Path) -> Result<Group, String> {
    let dir = root.join("packages/contract");
    let banner = format!("// {GENERATED_HEADER}");
    let mut files = Vec::new();
    for (view, name) in [(View::Output, "output.ts"), (View::Input, "input.ts")] {
        let ts = util::capture_with_input(
            &dir,
            "pnpm",
            &[
                "exec",
                "json2ts",
                "--no-additionalProperties",
                "--unreachableDefinitions",
                "--unknownAny",
                "--bannerComment",
                &banner,
            ],
            Some(&schema::render(view)),
        )
        .map_err(|e| format!("{e}\n(install the JavaScript tools with: pnpm install --frozen-lockfile)"))?;
        files.push(Generated { path: format!("packages/contract/src/generated/{name}"), contents: ts });
    }
    Ok(Group { name: "packages/contract/src/generated/", files })
}

fn dependency_cruiser(root: &Path) -> Result<Group, String> {
    let contents = boundaries::dependency_cruiser_config(root)?;
    Ok(Group {
        name: ".dependency-cruiser.cjs",
        files: vec![Generated { path: ".dependency-cruiser.cjs".into(), contents }],
    })
}

/// Write every generated file.
pub fn write_all(root: &Path) -> Result<Vec<String>, String> {
    let mut written = Vec::new();
    for group in all(root) {
        for file in group?.files {
            let path = root.join(&file.path);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
            }
            fs::write(&path, &file.contents).map_err(|e| format!("{}: {e}", path.display()))?;
            written.push(file.path);
        }
    }
    Ok(written)
}

/// The paths in a group whose committed bytes differ from a fresh generation.
pub fn drift(root: &Path, group: &Group) -> Vec<String> {
    group
        .files
        .iter()
        .filter(|f| fs::read_to_string(root.join(&f.path)).ok().as_deref() != Some(f.contents.as_str()))
        .map(|f| f.path.clone())
        .collect()
}

/// Files under the read-only globs of `boundaries.toml` that lack the header.
pub fn missing_headers(root: &Path, globs: &[String]) -> Result<Vec<String>, String> {
    let mut missing = Vec::new();
    for glob in globs {
        let mut paths = Vec::new();
        match glob.strip_suffix("/**") {
            Some(dir) if root.join(dir).is_dir() => {
                util::walk(&root.join(dir), &mut |p| paths.push(p.to_path_buf()))?
            }
            Some(_) => {}
            None if root.join(glob).is_file() => paths.push(root.join(glob)),
            None => {}
        }
        for path in paths {
            let text = fs::read_to_string(&path).unwrap_or_default();
            if !text.chars().take(512).collect::<String>().contains(GENERATED_HEADER) {
                missing.push(util::rel(root, &path));
            }
        }
    }
    missing.sort();
    Ok(missing)
}
