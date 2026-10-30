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
/// The area blocks and the `GENERATED.md` stubs come before the plugin's version stamp, which
/// hashes `plugin/AGENTS.md`, the stubs and the bundle with every other file under `plugin/`.
const GENERATORS: &[Generator] = &[
    |_| schemas(),
    typescript,
    format_ts,
    catalogue,
    dependency_cruiser,
    clippy_tomls,
    engine_env,
    contract_sh,
    configuration_reference,
    area_contracts,
    modgen::bundle,
    crate::docs::generated_marks,
    plugin_version,
    redact_fields,
    errors_reference,
];

/// The Rust tests that own a file under `tests/golden/`, each run once with
/// `PLANROOM_GOLDEN=write`; a plain test run only compares (DR-79).
pub const GOLDEN_TESTS: &[(&str, &str, &[&str])] = &[(
    "planroom",
    "golden",
    &[
        "tests/golden/cli/focus.json",
        "tests/golden/cli/focus-no-repos.json",
        "tests/golden/cli/map-container.json",
        "tests/golden/cli/map-graph.json",
        "tests/golden/cli/map-empty.json",
        "tests/golden/cli/watch-frames.json",
        "tests/golden/cli/invalid-argument.json",
        "tests/golden/cli/focus-picker-lens.json",
        "tests/golden/cli/focus-picker-project.json",
        "tests/golden/cli/focus-picker-stage.json",
    ],
)];

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

/// `packages/contract/src/generated/format.ts`: the TypeScript port of `contract::format::check`,
/// the one format check every reader of a persisted document uses (contracts.md, Versioning rules).
fn format_ts(_: &Path) -> Result<Group, String> {
    use planroom_contract::format::Document;
    use planroom_contract::{Ack, Answer, ErrorCode, InboxRow, LiveHead, SCHEMA_VERSION, Snapshot};
    let names = [Snapshot::NAME, LiveHead::NAME, InboxRow::NAME, Answer::NAME, Ack::NAME];
    let names = names.iter().map(|n| format!("\"{n}\"")).collect::<Vec<_>>().join(" | ");
    let mismatch = ErrorCode::SchemaMismatch.as_str();
    let contents = format!(
        "// {GENERATED_HEADER}
// The TypeScript port of `contract::format::check` (crates/contract/src/format.rs).

/** The major version of the whole public contract: a reader loads formats up to it. */
export const SCHEMA_VERSION = {SCHEMA_VERSION};

/** The documents a surface reads with a format tag, `planroom.<name>/v<N>`. */
export type DocumentName = {names};

/** The format tag this version writes for the document `name`, such as `planroom.inbox/v1`. */
export function formatTag(name: DocumentName): string {{
  return `planroom.${{name}}/v${{SCHEMA_VERSION}}`;
}}

/** What a document's `format` allows: read it (at version `version`), or refuse it. */
export type FormatCheck =
  | {{ readonly ok: true; readonly version: number }}
  | {{
      readonly ok: false;
      /** `{mismatch}` for a newer format; `other` for another document or none. */
      readonly code: \"{mismatch}\" | \"other\";
      readonly found: string;
      readonly supported: string;
    }};

/**
 * Check `doc.format` against the document `name`: `planroom.<name>/vN` with `N` from 1 up to
 * `SCHEMA_VERSION` loads; a newer `N` is `{mismatch}`; anything else names another document.
 */
export function checkFormat(doc: unknown, name: DocumentName): FormatCheck {{
  const supported = formatTag(name);
  const found =
    typeof doc === \"object\" && doc !== null && typeof (doc as {{ format?: unknown }}).format === \"string\"
      ? (doc as {{ format: string }}).format
      : \"\";
  const version = /^[1-9][0-9]*$/.exec(found.slice(`planroom.${{name}}/v`.length));
  if (!found.startsWith(`planroom.${{name}}/v`) || version === null) {{
    return {{ ok: false, code: \"other\", found, supported }};
  }}
  const n = Number(version[0]);
  return n <= SCHEMA_VERSION
    ? {{ ok: true, version: n }}
    : {{ ok: false, code: \"{mismatch}\", found, supported }};
}}
"
    );
    Ok(Group {
        name: "packages/contract/src/generated/format.ts",
        files: vec![Generated { path: "packages/contract/src/generated/format.ts".into(), contents }],
    })
}

/// `packages/contract/src/generated/catalogue.ts`: every catalogue leaf, for the mod's tools, `/room`
/// and its typeahead.
fn catalogue(root: &Path) -> Result<Group, String> {
    let (mod_keys, mod_strings) = mod_strings(root)?;
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
  /** Can make a paid model call; the band and `PaneModel.next` never name such a leaf (DR-66). */
  readonly spends: boolean;
  readonly undo: \"edit\" | \"compensate\" | \"query\" | \"not_undoable\";
  readonly tool: ToolName | null;
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

/**
 * The mod's own lines, from the `[mod]` table of `en.toml`: templates whose `{{name}}`s the mod
 * fills with `fill` from `@planroom/ui`, then sanitizes.
 */
export const MOD_STRINGS: Readonly<Record<\"{mod_keys}\", string>> = {mod_strings};
{tail}
",
        hash = catalogue_hash()?,
        states = FALLBACK_STATES.join("\" | \""),
        panes = fallback_panes(root)?,
        mod_keys = mod_keys.join("\" | \""),
        tail = modgen::catalogue_tail(root)?,
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

/// The `[mod]` table of `en.toml`: its keys, sorted, and the table as pretty JSON in that order.
fn mod_strings(root: &Path) -> Result<(Vec<String>, String), String> {
    let path = root.join("crates/planroom/text/en.toml");
    let text = fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    let catalog: toml::Table = text.parse().map_err(|e| format!("{}: {e}", path.display()))?;
    let table = catalog.get("mod").and_then(|t| t.as_table()).ok_or("en.toml has no [mod] table")?;
    let mut strings = serde_json::Map::new();
    for (key, value) in table {
        let value = value.as_str().ok_or(format!("en.toml's mod.{key} isn't a string"))?;
        strings.insert(key.clone(), json!(value));
    }
    let keys = strings.keys().cloned().collect();
    Ok((keys, serde_json::to_string_pretty(&strings).map_err(|e| e.to_string())?))
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
        "spends": spec.spends,
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

/// Each shipped crate's `clippy.toml` from `boundaries.toml`: `no_io` for the kernel and contract,
/// `std::net` for store and extract [DR-79].
fn clippy_tomls(root: &Path) -> Result<Group, String> {
    Ok(Group { name: "crates/*/clippy.toml", files: boundaries::clippy_tomls(root)? })
}

/// Each area's contract block and Read first lines in its `AGENTS.md`, from repo-layout.md.
fn area_contracts(root: &Path) -> Result<Group, String> {
    Ok(Group { name: "area contract blocks in AGENTS.md", files: crate::areas::generated(root)? })
}

/// `plugin/.claude-plugin/plugin.json`'s version: `X.Y.Z-dev.g<hash>` between releases, so
/// `/plugin update` sees each change; a release version as is.
fn plugin_version(root: &Path) -> Result<Group, String> {
    Ok(Group {
        name: "plugin/.claude-plugin/plugin.json version",
        files: vec![plugin::stamped_manifest(root)?],
    })
}

pub const REDACT_FIELDS_RS: &str = "crates/kernel/src/redact_fields.rs";

/// `crates/kernel/src/redact_fields.rs`: the paths to every field the contract marks `x-redact`,
/// from each output type that holds one, so `kernel::redact` masks exactly those (DR-89). The
/// kernel can't include a file from outside its crate, so the list is written inside it.
fn redact_fields(_: &Path) -> Result<Group, String> {
    let mark = schema::REDACT_MARK;
    let mut contents = format!(
        "// {GENERATED_HEADER}
// Source: the `{mark}` marks of the output schema view (`planroom_contract::schema::redacted_paths`).

//! The fields `kernel::redact` masks: for each output type that holds a field the contract marks
//! `#[schemars(extend(\"{mark}\" = true))]`, the paths from it to each marked field (DR-89).

/// Each output type, by name, with the paths to its marked fields, both sorted: property names
/// joined by `.`, with `[]` for an array's items and `{{}}` for a map's values.
#[rustfmt::skip]
pub const PATHS: &[(&str, &[&str])] = &[
"
    );
    for (name, paths) in schema::redacted_paths() {
        contents.push_str(&format!("    ({name:?}, &[\n"));
        for path in paths {
            contents.push_str(&format!("        {path:?},\n"));
        }
        contents.push_str("    ]),\n");
    }
    contents.push_str("];\n");
    use planroom_contract::{ErrorCode, WarningCode};
    let details_mark = schema::REDACT_DETAILS_MARK;
    contents.push_str(&format!(
        "
/// Each output type, by name, that holds an error or a warning (the `{details_mark}` mark), with
/// the path to each one (`\"\"` for the type itself) and the registry its `code` is in, both
/// sorted.
#[rustfmt::skip]
pub const DETAILS: &[(&str, &[(&str, &str)])] = &[
"
    ));
    for (name, holders) in schema::redacted_details() {
        contents.push_str(&format!("    ({name:?}, &[\n"));
        for (path, registry) in holders {
            contents.push_str(&format!("        ({path:?}, {registry:?}),\n"));
        }
        contents.push_str("    ]),\n");
    }
    contents.push_str("];\n");
    contents.push_str(
        "
/// Each output type above, by name, that also holds an open value no mark covers, with the path to
/// each one (`{}` at the type itself for the unknown fields it keeps), both sorted: what such a
/// value holds is masked only by its writer, so `redact::is_listed` refuses the type
/// (`planroom_contract::schema::open_paths`).
#[rustfmt::skip]
pub const OPEN: &[(&str, &[&str])] = &[
",
    );
    let listed: std::collections::BTreeSet<String> = schema::redacted_paths()
        .into_iter()
        .map(|(name, _)| name)
        .chain(schema::redacted_details().into_iter().map(|(name, _)| name))
        .collect();
    for (name, paths) in schema::open_paths().into_iter().filter(|(name, _)| listed.contains(name)) {
        contents.push_str(&format!("    ({name:?}, &[\n"));
        for path in paths {
            contents.push_str(&format!("        {path:?},\n"));
        }
        contents.push_str("    ]),\n");
    }
    contents.push_str("];\n");
    let codes = |title: &str, name: &str, specs: Vec<(&str, &[&str])>| {
        let mut out = format!(
            "
/// Each {title} code whose `details` hold repo-derived free text, with those keys, as the
/// registry's `redact` names them.
#[rustfmt::skip]
pub const {name}: &[(&str, &[&str])] = &[
"
        );
        for (code, keys) in specs.into_iter().filter(|(_, keys)| !keys.is_empty()) {
            let keys: Vec<String> = keys.iter().map(|k| format!("{k:?}")).collect();
            out.push_str(&format!("    ({code:?}, &[{}]),\n", keys.join(", ")));
        }
        out.push_str("];\n");
        out
    };
    contents.push_str(&codes(
        "error",
        "ERROR_DETAILS",
        ErrorCode::ALL.iter().map(|c| (c.as_str(), c.spec().redact)).collect(),
    ));
    contents.push_str(&codes(
        "warning",
        "WARNING_DETAILS",
        WarningCode::ALL.iter().map(|c| (c.as_str(), c.spec().redact)).collect(),
    ));
    Ok(Group { name: REDACT_FIELDS_RS, files: vec![Generated { path: REDACT_FIELDS_RS.into(), contents }] })
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

pub const CONTRACT_SH: &str = "plugin/scripts/contract.sh";

/// A printf template from a contract value: each `"@@name@@"` string in its JSON becomes `%s`, and
/// the names come back in the order the template takes them. The caller passes each as JSON (a
/// quoted string, a number or an object).
fn printf_template(value: &serde_json::Value) -> Result<(String, Vec<String>), String> {
    let mut text = serde_json::to_string(value).map_err(|e| e.to_string())?;
    let mut holes = Vec::new();
    while let Some(start) = text.find("\"@@") {
        let end = start + 3 + text[start + 3..].find("@@\"").ok_or("an unterminated hole")?;
        holes.push(text[start + 3..end].to_owned());
        text.replace_range(start..end + 3, "%s");
    }
    if text.contains('\'') || text.contains('\\') || text.matches('%').count() != holes.len() {
        return Err(format!("a template the resolver can't quote or printf: {text}"));
    }
    Ok((text, holes))
}

/// `plugin/scripts/contract.sh`: what the resolver prints, from the contract (DR-69). The POSIX sh
/// resolver sources it, so it spells no error code, exit code, retry class or output shape itself.
fn contract_sh(_: &Path) -> Result<Group, String> {
    use planroom_contract::{
        CommandName, Envelope, ErrorCode, ErrorObject, Meta, ResolverStatus, ResolverStep, SCHEMA_VERSION,
        Size, UnavailableReason,
    };
    let hole = |name: &str| json!(format!("@@{name}@@"));
    let envelope = |code: ErrorCode| -> Result<serde_json::Value, String> {
        let spec = code.spec();
        let env: Envelope<()> = Envelope {
            schema_version: SCHEMA_VERSION,
            ok: false,
            command: CommandName("@@command@@".into()),
            summary: None,
            result: None,
            error: Some(ErrorObject {
                code,
                category: spec.category,
                retry: spec.retry,
                retry_after_ms: (code.retry() == planroom_contract::Retry::Later).then_some(1),
                message: "@@message@@".into(),
                hint: Some("@@hint@@".into()),
                details: hole("details"),
            }),
            meta: Meta { size: Size { bytes: 1, tokens_est: 1 }, ..Meta::default() },
        };
        let mut v = serde_json::to_value(env).map_err(|e| e.to_string())?;
        if v["error"].get("retry_after_ms").is_some() {
            v["error"]["retry_after_ms"] = hole("retry_after_ms");
        }
        v["meta"]["size"]["bytes"] = hole("bytes");
        v["meta"]["size"]["tokens_est"] = hole("tokens_est");
        Ok(v)
    };
    let mut out = format!(
        "# {GENERATED_HEADER}
# shellcheck shell=sh disable=SC2034
# The contract the resolver, plugin/scripts/planroom, prints from (contracts.md, The resolver's
# environment; DR-69), generated from crates/contract: contract::ErrorCode and ResolverStatus.
# The resolver sources this file, so it spells no code, exit code, retry class or output shape.
#
# Each *_template is a printf format whose %s holes take JSON values, in the order its *_args
# names them: a string goes in quoted (the resolver's json_string), a number or an object as is.

contract_schema_version={SCHEMA_VERSION}
"
    );
    out.push_str("\n# The resolver's error codes: each one's exit code, from its category.\n");
    for code in [ErrorCode::EngineUnavailable, ErrorCode::EngineFetching, ErrorCode::InvalidArgument] {
        let spec = code.spec();
        let (template, args) = printf_template(&envelope(code)?)?;
        if args[args.len() - 2..] != ["bytes", "tokens_est"] {
            return Err(format!("{}'s envelope must end with its size: {args:?}", code.as_str()));
        }
        let name = code.as_str();
        out.push_str(&format!(
            "# {name}: {category:?}, retry {retry:?}: {meaning}\ncontract_{name}_exit={exit}\ncontract_{name}_template='{template}'\ncontract_{name}_args='{args}'\n",
            category = spec.category,
            retry = spec.retry,
            meaning = spec.meaning,
            exit = code.exit_code(),
            args = args.join(" "),
        ));
    }
    out.push_str("\n# engine_unavailable's details.reason values.\n");
    for reason in UnavailableReason::ALL {
        out.push_str(&format!("contract_reason_{0}={0}\n", reason.as_str()));
    }
    out.push_str("\n# ResolverStatus, what status --json prints: the steps, then the two shapes.\n");
    for step in ResolverStep::ALL {
        out.push_str(&format!("contract_step_{0}={0}\n", step.as_str()));
    }
    for (name, path) in [("resolved", Some("@@path@@".to_owned())), ("unresolved", None)] {
        let status = ResolverStatus {
            ok: path.is_some(),
            step: ResolverStep::Download,
            path,
            version: "@@version@@".into(),
        };
        let mut v = serde_json::to_value(status).map_err(|e| e.to_string())?;
        v["step"] = hole("step");
        let (template, args) = printf_template(&v)?;
        out.push_str(&format!(
            "contract_status_{name}_template='{template}'\ncontract_status_{name}_args='{}'\n",
            args.join(" ")
        ));
    }
    Ok(Group { name: CONTRACT_SH, files: vec![Generated { path: CONTRACT_SH.into(), contents: out }] })
}

pub const ERRORS_MD: &str = "docs/reference/errors.md";

/// `docs/reference/errors.md`: every error and warning code, from `contract::ErrorCode` and
/// `contract::WarningCode`, so the reference never drifts from what the engine can return.
fn errors_reference(_: &Path) -> Result<Group, String> {
    use planroom_contract::{ErrorCode, WarningCode};
    let wire = |v: serde_json::Value| v.as_str().map(str::to_owned).ok_or("a non-string enum value");
    let cell = |text: &str| text.replace('|', "\\|");
    let keys = |keys: &[&str], masked: &[&str]| match keys {
        [] => "none".to_owned(),
        keys => keys
            .iter()
            .map(|k| if masked.contains(k) { format!("`{k}` (masked)") } else { format!("`{k}`") })
            .collect::<Vec<_>>()
            .join(", "),
    };
    let mut out = format!(
        "<!-- {GENERATED_HEADER} -->

# Error and warning codes

Every `--json` envelope that fails carries one error code, and any envelope may carry warnings.
This page lists every code this version can return, from `contract::ErrorCode` and
`contract::WarningCode` in `crates/contract/src/errors.rs`. For when each one is returned, see
[Contracts](../contributing/contracts.md#error-codes).

## Error codes

The category sets the exit code. The retry class says when you may send the same request again:

| Retry | What you do |
|---|---|
| `never` | Don't retry. A person or another command must act first; `hint` or `details` names it. |
| `now` | Retry at once. |
| `after_reread` | Read again, then retry with the new `meta.seq` as `base`. |
| `later` | Wait, then retry; `retry_after_ms` says how long when known. |

An older reader maps a code it doesn't know to its category and exit code. A `details` key
marked (masked) holds repo-derived text, so its secrets are masked like a reply's lines.

| Code | Category | Exit | Retry | Meaning | `details` |
|---|---|---|---|---|---|
"
    );
    for code in ErrorCode::ALL {
        let spec = code.spec();
        let category = wire(json!(spec.category))?;
        let retry = wire(json!(spec.retry))?;
        out.push_str(&format!(
            "| `{}` | {category} | {} | {retry} | {} | {} |\n",
            code.as_str(),
            code.exit_code(),
            cell(spec.meaning),
            keys(spec.details, spec.redact),
        ));
    }
    out.push_str(
        "
## Warning codes

A warning never changes the exit code.

| Code | Meaning | `details` |
|---|---|---|
",
    );
    for code in WarningCode::ALL {
        let spec = code.spec();
        out.push_str(&format!(
            "| `{}` | {} | {} |\n",
            code.as_str(),
            cell(spec.meaning),
            keys(spec.details, spec.redact)
        ));
    }
    Ok(Group { name: ERRORS_MD, files: vec![Generated { path: ERRORS_MD.into(), contents: out }] })
}

/// Write every generated file.
pub fn write_all(root: &Path) -> Result<Vec<String>, String> {
    write_groups(root, GENERATORS)
}

/// Write only the mod bundle (`cargo xtask gen --mod`).
pub fn write_mod(root: &Path) -> Result<Vec<String>, String> {
    write_groups(root, &[modgen::bundle, plugin_version])
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

/// The configuration defaults file the engine compiles in, the one home of every key.
pub const CONFIG_DEFAULTS: &str = "crates/planroom/src/config/defaults.toml";
pub const CONFIG_REFERENCE: &str = "docs/reference/configuration.md";

/// `docs/reference/configuration.md`: one row per key of the defaults file, with its default,
/// where it may be set, the variable that overrides it and its meaning. Each key's comment there
/// starts with its place in brackets, as the engine reads it (`config/schema.rs`).
fn configuration_reference(root: &Path) -> Result<Group, String> {
    let text =
        fs::read_to_string(root.join(CONFIG_DEFAULTS)).map_err(|e| format!("{CONFIG_DEFAULTS}: {e}"))?;
    let table: toml::Table = text.parse().map_err(|e| format!("{CONFIG_DEFAULTS}: {e}"))?;
    let mut rows = Vec::new();
    let mut section = String::new();
    let mut comment: Vec<&str> = Vec::new();
    for line in text.lines() {
        if let Some(c) = line.strip_prefix('#') {
            comment.push(c.trim());
            continue;
        }
        if let Some(name) = line.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            section = name.trim().to_owned();
            comment.clear();
            continue;
        }
        let key = match line.split_once('=') {
            Some((key, _)) if line.starts_with(|c: char| c.is_ascii_lowercase()) => key.trim(),
            _ => {
                if line.trim().is_empty() {
                    comment.clear();
                }
                continue;
            }
        };
        let joined = comment.join(" ");
        let (tag, meaning) =
            joined.strip_prefix('[').and_then(|rest| rest.split_once(']')).ok_or_else(|| {
                format!("{CONFIG_DEFAULTS}: {key} needs a `# [scope] meaning` comment above it")
            })?;
        let value =
            if section.is_empty() { table.get(key) } else { table.get(&section).and_then(|s| s.get(key)) }
                .ok_or_else(|| format!("{CONFIG_DEFAULTS}: no value for {key}"))?;
        let default = match value {
            toml::Value::String(s) if s.is_empty() => "none".to_owned(),
            other => format!("`{other}`"),
        };
        let path = if section.is_empty() { key.to_owned() } else { format!("{section}.{key}") };
        let shown = if section.is_empty() { format!("`{key}`") } else { format!("`[{section}] {key}`") };
        let variable = if path == "packs" || tag.contains("mode 0600") || tag.contains("no variable") {
            "none".to_owned()
        } else {
            format!("`PLANROOM_{}`", path.replace('.', "_").to_ascii_uppercase())
        };
        let cell = |s: &str| s.trim().replace('|', "\\|");
        rows.push(format!("| {shown} | {} | {default} | {variable} | {} |", cell(tag), cell(meaning)));
        comment.clear();
    }
    let contents = format!(
        "<!-- {GENERATED_HEADER} Source: {CONFIG_DEFAULTS}. -->

# Configuration

Every key Planroom reads, with its default. You can set a key in two optional files of one schema:

- your user-level `${{XDG_CONFIG_HOME:-$HOME/.config}}/planroom/config.toml`;
- a workspace's `config.toml` in its state directory (`.planroom/`, or `.git/planroom/` when the workspace is one repo), which loads only once you trust the workspace.

A workspace's file wins over yours, and a variable wins over both. A list variable takes TOML array syntax, `[\"a\", \"b\"]`, or comma-separated values.

**Where** says which file may set the key. `user` keys are credential settings and limits across every workspace: a workspace's file never sets one, and Planroom ignores it there with the warning `config_key_ignored`. `api_key` counts only in a file of mode 0600.

| Key | Where | Default | Variable | Meaning |
|---|---|---|---|---|
{}
",
        rows.join("\n")
    );
    Ok(Group { name: CONFIG_REFERENCE, files: vec![Generated { path: CONFIG_REFERENCE.into(), contents }] })
}
