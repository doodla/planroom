//! The mod's generated files: `plugin/types/index.d.ts` (the `$.state` contract), the committed
//! bundle `plugin/hooks/planroom-mod.js`, and the `target/mod-test/` harness (DR-6, DR-59).

use std::fs;
use std::path::Path;

use planroom_contract::{Command, Tool};
use serde_json::json;

use crate::generate::{GENERATED_HEADER, Generated, Group};
use crate::plugin;
use crate::util;

pub const BUNDLE: &str = "plugin/hooks/planroom-mod.js";
pub const HOOKS_JSON: &str = "plugin/hooks/hooks.json";
/// `plugin/hooks/hooks.json`: the one hooks module, the bundle. It holds only `modules`, since the
/// plugin has no classic hooks (`plugin::layout_problems`).
const HOOKS_JSON_CONTENTS: &str = "{ \"modules\": [\"./planroom-mod.js\"] }\n";
pub const STATE_TYPES: &str = "plugin/types/index.d.ts";

/// The atoms the mod keeps under `planroom`, each `Shaped<T>` so a reload that changes a value's
/// idea can decline what older code stored. Each atom's type is a contract type in
/// `crates/contract/src/view.rs` (`EngineState`, `PaneState`, `LastSeq`, `TurnSeq`, `OutboxEntry`),
/// which the output types above already hold (DR-81); only the host's module augmentation is
/// written here.
const ATOMS: &str = r#"
declare module "claude-code" {
  interface PluginState {
    planroom: {
      engine: Shaped<EngineState>;
      pane: Shaped<PaneState>;
      lastSeq: Shaped<LastSeq>;
      turnSeq: Shaped<TurnSeq>;
      outbox: Shaped<OutboxEntry[]>;
    };
  }
}
"#;

/// `plugin/types/index.d.ts`: the contract's output types, then the atoms.
pub fn state_types(output_ts: &str) -> Generated {
    let header = format!("// {GENERATED_HEADER}");
    let body = output_ts.strip_prefix(header.as_str()).unwrap_or(output_ts).trim_start_matches('\n');
    Generated {
        path: STATE_TYPES.into(),
        contents: format!(
            "{header}\n// The $.state contract of the planroom mod: the contract's output types and the mod's atoms.\n\n{body}{ATOMS}"
        ),
    }
}

/// The slash commands the mod registers (`packages/mod/src/register.tsx`, from `MOD_COMMANDS` in
/// `catalogue.ts`). The host refuses a mod command named like one of its own plugin's skills, and
/// the refusal skips the mod's whole `session.start`, which the mocked harness never shows (DR-84).
pub const MOD_COMMANDS: [&str; 1] = ["room"];

/// What a synopsis line drops in the mod's generated lines: the binary's name and the flags the
/// tool handler or `personRun` supplies (`--json`, `--base`, `--idem`), or that `PLANROOM_CLIENT`
/// names (`--client`).
const SUPPLIED: [&str; 4] = ["[--json]", "[--base BASE]", "[--idem IDEM]", "[--client CLIENT]"];
const SUPPLIED_FLAGS: [&str; 3] = ["--base", "--idem", "--client"];

/// A leaf's synopsis with the strip rule applied: `map [--level ...] [--lens LENS] ...`.
fn stripped(synopsis: &str) -> String {
    let line = synopsis.strip_prefix("planroom ").unwrap_or(synopsis);
    line.split(' ')
        .fold(Vec::<String>::new(), |mut words, word| {
            // A bracketed flag and its value are two words: rejoin them to compare.
            match words.last_mut() {
                Some(last) if last.starts_with("[--") && !last.ends_with(']') => {
                    last.push(' ');
                    last.push_str(word);
                }
                _ => words.push(word.to_owned()),
            }
            words
        })
        .into_iter()
        .filter(|w| !SUPPLIED.contains(&w.as_str()))
        .collect::<Vec<_>>()
        .join(" ")
}

fn tool_name(tool: Tool) -> &'static str {
    match tool {
        Tool::RoomQuery => "room_query",
        Tool::RoomChange => "room_change",
        Tool::RoomHandoff => "room_handoff",
    }
}

/// Every `Tool`, in order: each variant names the one after it, so a new variant fails to compile
/// here until it joins the chain, and `TOOL_SPECS` and the `ToolName` union both follow it.
fn tools() -> Vec<Tool> {
    fn after(tool: Tool) -> Option<Tool> {
        match tool {
            Tool::RoomQuery => Some(Tool::RoomChange),
            Tool::RoomChange => Some(Tool::RoomHandoff),
            Tool::RoomHandoff => None,
        }
    }
    std::iter::successors(Some(Tool::RoomQuery), |t| after(*t)).collect()
}

/// The tail of `catalogue.ts` that the mod's tools and `/room` read: `TOOL_SPECS`, each tool's
/// flat input schema and its description, the `[tools]` intro of `en.toml` then one stripped
/// synopsis line per leaf; `ROOM_COMMANDS`, every `/room` leaf with its synopsis, subcommands and
/// flags for the typeahead; `LEAF_OWN_FLAGS`, each leaf's own flags, so the mod supplies `--base`
/// and `--idem` only where the CLI reads them; and `MOD_COMMANDS` (DR-14, DR-66).
pub fn catalogue_tail(root: &Path) -> Result<String, String> {
    let path = root.join("crates/planroom/text/en.toml");
    let text = fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let catalog: toml::Table = text.parse().map_err(|e| format!("{}: {e}", path.display()))?;
    let intros = catalog.get("tools").and_then(|t| t.as_table()).ok_or("en.toml has no [tools] table")?;
    let said = |key: &str| -> Result<String, String> {
        intros
            .get(key)
            .and_then(|v| v.as_str())
            .map(str::to_owned)
            .ok_or(format!("en.toml has no tools.{key}"))
    };
    let mut specs = Vec::new();
    for tool in tools() {
        let name = tool_name(tool);
        let leaves: Vec<_> = Command::SPECS.iter().filter(|s| s.tool == Some(tool)).collect();
        if leaves.is_empty() {
            continue;
        }
        let mut properties = json!({
            "command": { "enum": leaves.iter().map(|l| l.name()).collect::<Vec<_>>() },
            "args": { "type": "array", "items": { "type": "string" }, "description": said("args")? },
            "base": { "type": "integer" },
            "idem": { "type": "string" },
        });
        if tool == Tool::RoomChange {
            properties["payload"] = json!({ "type": "object", "description": said("payload")? });
        }
        let lines: Vec<String> = leaves.iter().map(|l| stripped(&l.synopsis())).collect();
        specs.push(json!({
            "name": name,
            "description": format!("{}\n{}", said(name)?, lines.join("\n")),
            "inputSchema": {
                "type": "object",
                "required": ["command"],
                "additionalProperties": false,
                "properties": properties,
            },
            "isDeferred": tool == Tool::RoomHandoff,
        }));
    }
    let room: Vec<serde_json::Value> = Command::SPECS
        .iter()
        .filter(|s| s.room)
        .map(|s| {
            let cmd = s.command();
            let flags: Vec<serde_json::Value> = s
                .flags()
                .into_iter()
                .filter(|f| !SUPPLIED_FLAGS.contains(&f.flag.as_str()))
                .map(|f| json!({"sub": f.sub, "flag": f.flag, "valueName": f.value_name, "values": f.values}))
                .collect();
            json!({
                "name": s.name(),
                "summary": s.summary,
                "synopsis": format!("/room {}", stripped(&s.synopsis())),
                "subs": cmd.get_subcommands().map(|c| c.get_name()).collect::<Vec<_>>(),
                "flags": flags,
            })
        })
        .collect();
    let own: serde_json::Map<String, serde_json::Value> = Command::SPECS
        .iter()
        .map(|s| {
            let flags: Vec<String> =
                s.flags().into_iter().filter(|f| f.sub.is_none()).map(|f| f.flag).collect();
            (s.name(), json!(flags))
        })
        .collect();
    let tool_names: Vec<String> = tools().into_iter().map(|t| format!("\"{}\"", tool_name(t))).collect();
    Ok(format!(
        "
/** A mod tool, as the model sees it: `mcp__planroom__<name>`. */
export type ToolName = {tool_names};

/**
 * Each leaf's own long flags, its subcommands' left out, from its clap arguments. A tool handler
 * or `personRun` supplies `--base` or `--idem` only to a leaf that takes it, and puts it right
 * after the leaf's path, before any subcommand (`edit --base SEQ undo EVENT`); the CLI reads it on
 * either side of one.
 */
export const LEAF_OWN_FLAGS: Readonly<Record<string, readonly string[]>> = {own};

/** One long flag of a `/room` leaf or of one of its subcommands (`sub`), for the typeahead. */
export interface LeafFlag {{
  readonly sub: string | null;
  readonly flag: string;
  /** The value's name, such as `LENS`; null for a flag that takes no value. */
  readonly valueName: string | null;
  /** The values the flag accepts, when they are a fixed set. */
  readonly values: readonly string[];
}}

/** One `/room` command: its name, summary, synopsis line, subcommands and flags. */
export interface RoomCommand {{
  readonly name: string;
  readonly summary: string;
  readonly synopsis: string;
  readonly subs: readonly string[];
  readonly flags: readonly LeafFlag[];
}}

/** One mod tool as the mod registers it: a flat input and a generated description (DR-66). */
export interface ToolSpecData {{
  name: ToolName;
  description: string;
  inputSchema: Record<string, unknown>;
  isDeferred: boolean;
}}

/**
 * The tools that have leaves, each described by its `[tools]` intro from `en.toml` and one
 * synopsis line per leaf, less `planroom`, `[--json]`, `[--base BASE]`, `[--idem IDEM]` and
 * `[--client CLIENT]`, which the handler supplies.
 */
export const TOOL_SPECS: readonly ToolSpecData[] = {specs};

/** Every `/room` leaf in catalogue order, with its synopsis line under the same strip rule. */
export const ROOM_COMMANDS: readonly RoomCommand[] = {room};

/** The slash commands the mod registers; no plugin skill may share one's name (DR-84). */
export const MOD_COMMANDS = {mod_commands} as const;",
        tool_names = tool_names.join(" | "),
        own = serde_json::to_string_pretty(&own).map_err(|e| e.to_string())?,
        specs = serde_json::to_string_pretty(&specs).map_err(|e| e.to_string())?,
        room = serde_json::to_string_pretty(&room).map_err(|e| e.to_string())?,
        mod_commands = serde_json::to_string(&MOD_COMMANDS).map_err(|e| e.to_string())?,
    ))
}

fn esbuild(
    root: &Path,
    entry: &str,
    externals: &[&str],
    defines: &[(&str, String)],
) -> Result<String, String> {
    let banner = format!("--banner:js=// {GENERATED_HEADER}");
    let mut args = vec![
        "exec",
        "esbuild",
        entry,
        "--bundle",
        "--format=esm",
        "--platform=neutral",
        "--target=es2023",
        "--charset=utf8",
        "--legal-comments=none",
        "--log-level=warning",
        "--jsx=transform",
        "--jsx-factory=h",
        "--jsx-fragment=Fragment",
        &banner,
    ];
    let externals: Vec<String> = externals.iter().map(|e| format!("--external:{e}")).collect();
    args.extend(externals.iter().map(String::as_str));
    let defines: Vec<String> =
        defines.iter().map(|(name, value)| format!("--define:{name}={value}")).collect();
    args.extend(defines.iter().map(String::as_str));
    util::capture(&root.join("packages/mod"), "pnpm", &args)
        .map_err(|e| format!("{e}\n(install the JavaScript tools with: pnpm install --frozen-lockfile)"))
}

/// The committed bundle of `packages/mod`: ESM, no splitting, `claude-code` external.
pub fn bundle(root: &Path) -> Result<Group, String> {
    let contents = export_register_inline(&esbuild(root, "src/register.tsx", &["claude-code"], &[])?)?;
    Ok(Group {
        name: "plugin/hooks/",
        files: vec![
            Generated { path: HOOKS_JSON.into(), contents: HOOKS_JSON_CONTENTS.into() },
            Generated { path: BUNDLE.into(), contents },
        ],
    })
}

/// The host loads a hooks module only when `register` is a function declared and exported at its
/// top level, but esbuild exports through a trailing `export { register };` list. Move the export
/// onto the declaration.
fn export_register_inline(js: &str) -> Result<String, String> {
    const DECLARATION: &str = "\nfunction register(on) {\n";
    const EXPORT_LIST: &str = "export {\n  register\n};\n";
    if js.matches(DECLARATION).count() != 1 || !js.ends_with(EXPORT_LIST) {
        return Err("the mod bundle no longer has one top-level `function register(on)` and a trailing \
                    `export { register };`; update export_register_inline in crates/xtask/src/modgen.rs"
            .into());
    }
    let js = js.strip_suffix(EXPORT_LIST).unwrap_or(js);
    Ok(js.replacen(DECLARATION, "\nexport function register(on) {\n", 1))
}

/// Write `target/mod-test/`: the mod bundle, its `claude plugin test` files bundled with the
/// golden CLI outputs inlined, the state contract and a manifest naming them.
pub fn harness(root: &Path) -> Result<std::path::PathBuf, String> {
    let dir = root.join("target/mod-test");
    if dir.exists() {
        fs::remove_dir_all(&dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    let write = |rel: &str, text: &str| -> Result<(), String> {
        let path = dir.join(rel);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("{}: {e}", parent.display()))?;
        }
        fs::write(&path, text).map_err(|e| format!("{}: {e}", path.display()))
    };
    let version = plugin::workspace_version(root)?;
    write(
        ".claude-plugin/plugin.json",
        &format!(
            "{{\"name\": \"planroom\", \"version\": \"{version}\", \"description\": \"The planroom mod under test.\", \"types\": \"./types/index.d.ts\"}}\n"
        ),
    )?;
    for file in bundle(root)?.files {
        write(file.path.strip_prefix("plugin/").unwrap_or(&file.path), &file.contents)?;
    }
    let types = fs::read_to_string(root.join(STATE_TYPES)).map_err(|e| format!("{STATE_TYPES}: {e}"))?;
    write("types/index.d.ts", &types)?;
    // The plugin's skills, so the host stand-in refuses a mod command named like one, as the host
    // does (DR-84); only the test bundles get it, never the product bundle.
    let skills_dir = root.join("plugin/skills");
    let mut skills: Vec<String> = fs::read_dir(&skills_dir)
        .map_err(|e| format!("{}: {e}", skills_dir.display()))?
        .filter_map(|e| e.ok().filter(|e| e.path().is_dir()).and_then(|e| e.file_name().into_string().ok()))
        .collect();
    skills.sort();
    let defines = [("__PLANROOM_SKILLS__", serde_json::to_string(&skills).map_err(|e| e.to_string())?)];
    let tests_dir = root.join("packages/mod/plugin-tests");
    let mut tests: Vec<String> = fs::read_dir(&tests_dir)
        .map_err(|e| format!("{}: {e}", tests_dir.display()))?
        .filter_map(|e| e.ok().and_then(|e| e.file_name().into_string().ok()))
        .filter(|n| n.ends_with(".test.tsx") || n.ends_with(".test.ts"))
        .collect();
    tests.sort();
    for name in tests {
        let js = esbuild(
            root,
            &format!("plugin-tests/{name}"),
            &["claude-code", "claude-code/testing"],
            &defines,
        )?;
        let out = name.replace(".test.tsx", ".test.ts");
        write(&out, &js)?;
    }
    Ok(dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The `ToolName` union comes from the one list of `Tool` variants that `TOOL_SPECS` walks, so
    /// a new variant can't reach the specs without the union.
    #[test]
    fn the_tool_name_union_lists_every_tool() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        let tail = catalogue_tail(&root).unwrap();
        let union: Vec<String> = tools().into_iter().map(|t| format!("\"{}\"", tool_name(t))).collect();
        assert_eq!(union.len(), 3);
        assert!(tail.contains(&format!("export type ToolName = {};", union.join(" | "))), "{tail}");
        // A catalogue leaf's tool is that union too, never a second list of names.
        let generated =
            fs::read_to_string(root.join("packages/contract/src/generated/catalogue.ts")).unwrap();
        assert!(
            generated.contains("readonly tool: ToolName | null;"),
            "CatalogueLeaf.tool lists its own names"
        );
    }
}
