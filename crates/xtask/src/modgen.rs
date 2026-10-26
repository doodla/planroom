//! The mod's generated files: `plugin/types/index.d.ts` (the `$.state` contract), the committed
//! bundle `plugin/hooks/planroom-mod.js`, and the `target/mod-test/` harness (DR-6, DR-59).

use std::fs;
use std::path::Path;

use crate::generate::{GENERATED_HEADER, Generated, Group};
use crate::plugin;
use crate::util;

pub const BUNDLE: &str = "plugin/hooks/planroom-mod.js";
pub const STATE_TYPES: &str = "plugin/types/index.d.ts";

/// The atoms the mod keeps under `planroom`, each `Shaped<T>` so a reload that changes a value's
/// idea can decline what older code stored. Each atom's type is a contract type in
/// `crates/contract/src/view.rs` (`EngineState`, `PaneState`, `LastSeq`, `TurnSeq`), which the
/// output types above already hold (DR-81); only the host's module augmentation is written here.
const ATOMS: &str = r#"
declare module "claude-code" {
  interface PluginState {
    planroom: {
      engine: Shaped<EngineState>;
      pane: Shaped<PaneState>;
      lastSeq: Shaped<LastSeq>;
      turnSeq: Shaped<TurnSeq>;
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

fn esbuild(root: &Path, entry: &str, externals: &[&str]) -> Result<String, String> {
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
    util::capture(&root.join("packages/mod"), "pnpm", &args)
        .map_err(|e| format!("{e}\n(install the JavaScript tools with: pnpm install --frozen-lockfile)"))
}

/// The committed bundle of `packages/mod`: ESM, no splitting, `claude-code` external.
pub fn bundle(root: &Path) -> Result<Group, String> {
    let contents = export_register_inline(&esbuild(root, "src/register.tsx", &["claude-code"])?)?;
    Ok(Group { name: BUNDLE, files: vec![Generated { path: BUNDLE.into(), contents }] })
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
    write("hooks/hooks.json", "{\"modules\": [\"./planroom-mod.js\"]}\n")?;
    write("hooks/planroom-mod.js", &bundle(root)?.files.remove(0).contents)?;
    let types = fs::read_to_string(root.join(STATE_TYPES)).map_err(|e| format!("{STATE_TYPES}: {e}"))?;
    write("types/index.d.ts", &types)?;
    let tests_dir = root.join("packages/mod/plugin-tests");
    let mut tests: Vec<String> = fs::read_dir(&tests_dir)
        .map_err(|e| format!("{}: {e}", tests_dir.display()))?
        .filter_map(|e| e.ok().and_then(|e| e.file_name().into_string().ok()))
        .filter(|n| n.ends_with(".test.tsx") || n.ends_with(".test.ts"))
        .collect();
    tests.sort();
    for name in tests {
        let js = esbuild(root, &format!("plugin-tests/{name}"), &["claude-code", "claude-code/testing"])?;
        let out = name.replace(".test.tsx", ".test.ts");
        write(&out, &js)?;
    }
    Ok(dir)
}
