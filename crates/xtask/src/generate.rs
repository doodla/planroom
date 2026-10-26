//! `cargo xtask gen`: every generated file, built in memory so `check` can compare byte for byte.

use std::fs;
use std::path::Path;

pub use planroom_contract::schema::GENERATED_HEADER;
use planroom_contract::schema::{self, View};
use planroom_contract::{Command, CommandSpec, Tool, Undo};
use serde_json::json;

use crate::boundaries;
use crate::modgen;
use crate::plugin;
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

type Generator = fn(&Path) -> Result<Group, String>;

/// Every generator, in the order `gen` writes them: the mod bundle last, since esbuild reads the
/// generated TypeScript from disk.
const GENERATORS: &[Generator] =
    &[|_| schemas(), typescript, catalogue, dependency_cruiser, engine_env, modgen::bundle];

/// Every generated file, grouped by drift check.
pub fn all(root: &Path) -> Vec<Result<Group, String>> {
    GENERATORS.iter().map(|g| g(root)).collect()
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
        if view == View::Output {
            files.push(modgen::state_types(&ts));
        }
        files.push(Generated { path: format!("packages/contract/src/generated/{name}"), contents: ts });
    }
    Ok(Group { name: "packages/contract/src/generated/ and plugin/types/", files })
}

/// `packages/contract/src/generated/catalogue.ts`: every catalogue leaf, for the mod's tools, `/room`
/// and its typeahead.
fn catalogue(root: &Path) -> Result<Group, String> {
    let leaves: Vec<_> = Command::SPECS.iter().map(leaf_json).collect();
    let body = serde_json::to_string_pretty(&leaves).map_err(|e| e.to_string())?;
    let contents = format!(
        "// {GENERATED_HEADER}

/** One catalogue leaf: what the mod's tools, `/room` and its typeahead need to know. */
export interface CatalogueLeaf {{
  readonly name: string;
  readonly idea: \"levels\" | \"lenses\" | \"intent\" | \"diffs\" | \"decisions\" | \"setup\";
  readonly summary: string;
  readonly actor: \"any\" | \"person_only\" | \"mixed\";
  readonly class: \"read\" | \"change\" | \"handoff\";
  readonly budget_tokens: number;
  readonly needs_base: boolean;
  readonly undo: \"edit\" | \"compensate\" | \"query\" | \"not_undoable\";
  readonly tool: \"room_query\" | \"room_change\" | \"room_handoff\" | null;
  readonly room: boolean;
}}

/** Every leaf, in catalogue order. */
export const CATALOGUE: readonly CatalogueLeaf[] = {body};

/** sha256 of the catalogue as `planroom schema --commands` lists it; `version --json` reports it. */
export const CATALOGUE_HASH = \"{hash}\";

/** One pane the mod draws before the engine answers `focus` (DR-57). */
export interface FallbackPane {{
  readonly headline: string;
  readonly detail: string | null;
  readonly action: string | null;
}}

/** The fallback panes' strings, from the engine's `crates/planroom/text/en.toml`. */
export const FALLBACK_PANES: Readonly<Record<\"{states}\", FallbackPane>> = {panes};
",
        hash = catalogue_hash()?,
        states = FALLBACK_STATES.join("\" | \""),
        panes = fallback_panes(root)?
    );
    Ok(Group {
        name: "packages/contract/src/generated/catalogue.ts",
        files: vec![Generated { path: "packages/contract/src/generated/catalogue.ts".into(), contents }],
    })
}

/// The states with a fallback pane, in the order `catalogue.ts` lists them.
const FALLBACK_STATES: &[&str] = &[
    "resolving",
    "fetching",
    "build_offered",
    "building",
    "version_mismatch",
    "person_key_mismatch",
    "unsupported_platform",
    "failed",
    "no_focus",
];

/// `FALLBACK_PANES` from the `[fallback]` table of `en.toml`: each state's headline, detail and
/// action label, a missing detail or action as null.
fn fallback_panes(root: &Path) -> Result<String, String> {
    let path = root.join("crates/planroom/text/en.toml");
    let text = fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let catalog: toml::Table = text.parse().map_err(|e| format!("{}: {e}", path.display()))?;
    let table =
        catalog.get("fallback").and_then(|t| t.as_table()).ok_or("en.toml has no [fallback] table")?;
    let get = |key: String| table.get(&key).and_then(|v| v.as_str()).map(str::to_owned);
    let mut panes = serde_json::Map::new();
    for state in FALLBACK_STATES {
        let headline =
            get(format!("{state}_headline")).ok_or(format!("en.toml has no fallback.{state}_headline"))?;
        panes.insert(
            (*state).into(),
            json!({"headline": headline, "detail": get(format!("{state}_detail")), "action": get(format!("{state}_action"))}),
        );
    }
    serde_json::to_string_pretty(&panes).map_err(|e| e.to_string())
}

/// sha256 of the compact JSON of `Command::listing()`, as the engine's `version --json` reports it.
pub fn catalogue_hash() -> Result<String, String> {
    use sha2::{Digest, Sha256};
    let listing = serde_json::to_string(&Command::listing()).map_err(|e| e.to_string())?;
    Ok(Sha256::digest(listing.as_bytes()).iter().map(|b| format!("{b:02x}")).collect())
}

fn leaf_json(spec: &CommandSpec) -> serde_json::Value {
    let tool = spec.tool.map(|t| match t {
        Tool::RoomQuery => "room_query",
        Tool::RoomChange => "room_change",
        Tool::RoomHandoff => "room_handoff",
    });
    let undo = match spec.undo {
        Undo::Edit => "edit",
        Undo::Compensate => "compensate",
        Undo::Query => "query",
        Undo::NotUndoable(_) => "not_undoable",
    };
    json!({
        "name": spec.name(),
        "idea": snake(&format!("{:?}", spec.idea)),
        "summary": spec.summary,
        "actor": snake(&format!("{:?}", spec.actor)),
        "class": snake(&format!("{:?}", spec.class)),
        "budget_tokens": spec.budget_tokens,
        "needs_base": spec.needs_base,
        "undo": undo,
        "tool": tool,
        "room": spec.room,
    })
}

/// `PersonOnly` as `person_only`.
fn snake(name: &str) -> String {
    let mut out = String::new();
    for (i, c) in name.chars().enumerate() {
        if c.is_ascii_uppercase() {
            if i > 0 {
                out.push('_');
            }
            out.push(c.to_ascii_lowercase());
        } else {
            out.push(c);
        }
    }
    out
}

fn dependency_cruiser(root: &Path) -> Result<Group, String> {
    let contents = boundaries::dependency_cruiser_config(root)?;
    Ok(Group {
        name: ".dependency-cruiser.cjs",
        files: vec![Generated { path: ".dependency-cruiser.cjs".into(), contents }],
    })
}

/// `plugin/scripts/engine.env`: the engine version the resolver fetches, the catalogue hash a checkout
/// build must report, and the repo it comes from.
fn engine_env(root: &Path) -> Result<Group, String> {
    let version = plugin::workspace_version(root)?;
    let repo = plugin::workspace_repo(root)?;
    let hash = catalogue_hash()?;
    let contents = format!(
        "# {GENERATED_HEADER}\nPLANROOM_ENGINE_VERSION={version}\nPLANROOM_CATALOGUE_HASH={hash}\nPLANROOM_REPO={repo}\n"
    );
    Ok(Group {
        name: "plugin/scripts/engine.env",
        files: vec![Generated { path: "plugin/scripts/engine.env".into(), contents }],
    })
}

/// Write every generated file.
pub fn write_all(root: &Path) -> Result<Vec<String>, String> {
    write_groups(root, GENERATORS)
}

/// Write only the mod bundle (`cargo xtask gen --mod`).
pub fn write_mod(root: &Path) -> Result<Vec<String>, String> {
    write_groups(root, &[modgen::bundle])
}

/// Generate and write each group in turn, so a later generator reads what an earlier one wrote.
fn write_groups(root: &Path, generators: &[Generator]) -> Result<Vec<String>, String> {
    let mut written = Vec::new();
    for generate in generators {
        for file in generate(root)?.files {
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
