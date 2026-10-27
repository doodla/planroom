# Contracts

This page lists every public contract Planroom has: what each one is, its single source, its version, what is generated from it, who consumes it, and the test that catches drift. It ends with Rust-style sketches of the core types. The sketches show shape and intent; the Rust types in `crates/contract/src/` are the source of truth once they exist, and this page links to them instead of restating them.

## Versioning rules

These rules apply to every contract below unless its row says otherwise [DR-37].

- **One source.** Every public shape is a Rust type in `planroom-contract`. Nothing else defines one. TypeScript types are generated; nobody hand-copies a type across the language boundary.
- **One schema in two views.** `cargo xtask gen` runs schemars 1.2.3 (draft 2020-12, `preserve_order`) and writes `schema/planroom.v1.schema.json` (the serialize view: what Planroom emits) and `schema/planroom.v1.input.schema.json` (the deserialize view: what Planroom accepts). Every contract is a `$defs` entry in them. `$id` is the raw GitHub URL on `main`.
- **One version.** `schema_version = 1` is the major version of the whole public contract: commands, flags, exit codes, the envelope, result types, the error object, events, files and the code tables. Additive changes keep it. A rename, a removal, a changed meaning, or a new required input field bumps it, and the schema file names with it.
- **Format tags.** Every persisted or exchanged document carries `format: "planroom.<name>/v1"`, so a file read out of context says what it is. The `/v1` follows `schema_version`.
- **Open sets.** Error codes, warning codes, node and relation kinds, event kinds, question IDs, hint kinds and command names are `anyOf: [{enum: [...]}, {pattern}]`, so an older reader still validates a newer value. Everything else is closed. An event body whose `kind` a reader doesn't know decodes as `EventBody::Unknown`, which keeps the raw body, `kind` included, and writes it back unchanged; the fold skips it (DR-85).
- **One format check.** `contract::format` is the check every Rust reader of a persisted document uses: it loads a `format` version from 1 up to the reader's own `schema_version` and refuses a newer one with `schema_mismatch`. `cargo xtask gen` writes its TypeScript port, `checkFormat` in `packages/contract/src/generated/format.ts`, from the same constants, and the Artifact transport reads every document through it (DR-85).
- **Unknown fields survive.** A persisted document keeps the fields it doesn't know in a flattened `extra` (`contract::format::Extra`) and writes them back unchanged. The types that do so are in [Persisted documents](#persisted-documents). These rules only loosen what a reader accepts, so they came before any release with `schema_version` still 1 (DR-85).
- **Integers** stay below 2^53 (`seq`, counts, line numbers, byte sizes). Hashes and IDs are strings. A schema lint in `xtask` fails on a `uint64` without an annotation [rust-stack G8].
- **Breaking-change guard.** `cargo xtask check` compares the generated schema with the committed baseline in `schema/baseline/`, and, once a release exists, with the schema at the last `v*` tag. It fails on a removed property, a removed enum value or a new required input field without a `schema_version` bump. The baseline moves only with a `schema_version` bump, through `cargo xtask gen --schema-baseline`, and a tag it needs and can't find fails the check, never skips it. So the result depends on no branch tip and no fetch timing [DR-59].

### Persisted documents

| Document | Format tag | Type | Unknown content kept |
|---|---|---|---|
| The Artifact snapshot | `planroom.snapshot/v1` | `contract::remote::Snapshot` | fields, in `extra` |
| `live/head` | `planroom.live-head/v1` | `contract::remote::LiveHead` | fields, in `extra` |
| An inbox row | `planroom.inbox/v1` | `contract::remote::InboxRow` | fields, in `extra`, read as data only, so a forged `actor` grants nothing |
| An answer | `planroom.answer/v1` | `contract::remote::Answer` | fields, in `extra` |
| An ack | `planroom.ack/v1` | `contract::remote::Ack` | fields, in `extra` |
| An event export or backup | `planroom.event/v1` | `contract::event::Event` | an unknown kind's body, as `EventBody::Unknown` |
| `workspaces.json` | `planroom.workspaces/v1` | `crates/planroom/src/app/data.rs` | fields, in `extra`, at the top level and in each entry |
| `serve.json`, and a running server's record under the data root | `planroom.serve/v1` | `ServeFile` in `crates/planroom/src/server/mod.rs` | fields, in `extra` |

`workspaces.json`, `serve.json` and the server's record are internal: no surface reads them, so their types live in `crates/planroom`, not the contract crate (DR-85). The rules for the data root's shared files are in [Store schema and migrations](#store-schema-and-migrations).

## Codegen flow

```text
crates/contract/src/*.rs ──schemars──▶ schema/planroom.v1.schema.json          ──json2ts──▶ packages/contract/src/generated/output.ts
                         ──schemars──▶ schema/planroom.v1.input.schema.json    ──json2ts──▶ packages/contract/src/generated/input.ts
contract::Command (catalogue) ──xtask──▶ packages/contract/src/generated/catalogue.ts (command names, ideas, actor rules, budgets,
                                         undo classes, tool input schemas, ROOM_COMMANDS, the catalogue hash)
                              ──xtask──▶ plugin/scripts/engine.env PLANROOM_CATALOGUE_HASH
                              ──clap───▶ docs/reference/cli.md
contract::ErrorCode::ALL, WarningCode::ALL ──xtask──▶ docs/reference/errors.md (tables)
crates/planroom/text/en.toml (the fallback panes) ──xtask──▶ catalogue.ts FALLBACK_PANES
contract::format ──xtask──▶ packages/contract/src/generated/format.ts (checkFormat)
contract::ErrorCode and contract::resolver ──xtask──▶ plugin/scripts/contract.sh (the resolver's codes, reasons, steps and templates)
contract::PaneModel and the atom types (EngineState, PaneState, LastSeq, TurnSeq) ──xtask──▶ plugin/types/index.d.ts
crates/planroom/src/config/defaults.toml ──xtask──▶ docs/reference/configuration.md
boundaries.toml ──xtask──▶ .dependency-cruiser.cjs
repo-layout.md area contracts ──xtask──▶ each area's AGENTS.md contract block, with its Read first lines
```

`cargo xtask gen` writes every generated file; `cargo xtask check` regenerates each in memory and fails on any byte difference, with the regenerate command in the message. Every generated file starts with `GENERATED by cargo xtask gen. Do not edit.` (an HTML comment in Markdown, a `$comment` key in JSON schemas, a line comment in code). Generated directories are listed in [Repo layout](repo-layout.md#generated-directories).

## Every public contract

| Contract | Format tag or name | Single source | Codegen targets | Consumers | Drift test |
|---|---|---|---|---|---|
| CLI `--json` envelope, error and warning codes | `schema_version` | `crates/contract/src/envelope.rs`, `errors.rs` | schema `$defs` `Envelope`, `ErrorObject`, `ErrorCode`, `WarningCode`; `output.ts`; `docs/reference/errors.md` | agents, the mod, the canvas, CI | `committed_schema_matches_the_generated_schema`; `every_cli_json_output_validates` (every CLI output in the test suite against `Envelope` and its command's result type); `the_documented_error_table_is_the_error_codes` (both ways); `errors_are_only_built_from_the_registry` (source scan) |
| Command catalogue (requests, results, help) | command names | `crates/contract/src/command.rs` | clap tree; HTTP routes; `catalogue.ts`; the three tool schemas; `ROOM_COMMANDS`; `docs/reference/cli.md` | the CLI, `serve`, the mod, docs | `catalogue_has_one_handler_per_command`; `cli_reference_equals_help` (regenerate with `cargo xtask gen`, DR-70); `help_snapshot` (S15); `tool_schemas_equal_catalogue`; `undo_classes_snapshot` (DR-55) |
| HTTP API | `/api/v1/` | `crates/planroom/src/server/routes.rs`, generated from the catalogue | none beyond the catalogue | the canvas | `routes.rs` (every catalogue command has a route; status per category) |
| Live stream (SSE and NDJSON) | `StreamFrame` | `crates/contract/src/stream.rs` | schema `$defs` `StreamFrame`; `output.ts` | the canvas, the mod (`watch`), S8 | `stream_frames_validate` (every frame in S8 against the schema) |
| Store schema and migrations | `meta.store_version`; the ledger's own `user_version` | `crates/store/migrations/*.sql`, `crates/store/ledger/*.sql` | none | `planroom-store` only (internal; listed because it's versioned) | `migrations_open_every_released_fixture` over `crates/store/tests/fixtures/store-v*.db`; `store_sql_matches_migrations`; `a_failed_migration_rolls_back` |
| Event export | `planroom.event/v1` (NDJSON, first line a header) | `crates/contract/src/event.rs` | schema `$defs` `Event`, `EventBody`; `output.ts` | `export`, a person's `import`, the automatic backup under the data root, upgrades | `export_import_round_trip` (export, import into a fresh store, fold equals); `a_lost_state_directory_then_scan_offers_one_restore_card` |
| Graph model | `planroom.facts/v1` | `crates/contract/src/model.rs` | schema `$defs` `Node`, `Edge`, `Evidence`, `Hint`, `FactBatch`; `output.ts` | the kernel, view models, pack tests | `extract/tests/contract.rs` validates every batch; `round_trip` (Rust to schema to TS type-check) |
| View models | none (inside the envelope) | `crates/contract/src/view.rs` | schema `$defs` `PaneModel`, `Picker`, `MapView`, `Diff`, `DecisionCard`, `Breakdown`, `MachineView`, `PathView`; `output.ts`; `plugin/types/index.d.ts`; the fallback panes in `catalogue.ts` | the mod, the canvas, the Artifact page | golden files in `tests/golden/*.json`, which the Rust tests compare against and write only under `cargo xtask gen --golden`, type-checked by `tsc` (DR-79); `golden_outputs_equal_engine_output`; `same_output`; `no_blank_pane` |
| Pack manifest | `planroom.pack/v1` | `crates/contract/src/pack.rs` | input schema `$defs` `PackFile`; `docs/reference/packs.md` (generated tables) | pack authors, the loader | every pack under `packs/` and `examples/packs/` validates; a bad pack fixture fails with `pack_invalid`; `doctor --pack` on every pack |
| Extractor and linker contract | `FactBatch`, emit templates | `crates/contract/src/model.rs`, `crates/extract/src/lib.rs` (the traits) | schema `$defs` `FactBatch`, `EmitTemplate` | built-in extractors, the declarative runner, pack tests | `extract/tests/contract.rs` (determinism, tolerance, bounds, evidence verified, sorted output) |
| Decision kind and policy | inside `planroom.pack/v1` | `crates/contract/src/decision.rs` | input schema `$defs` `DecisionKind`, `Policy`, `Role` | pack authors, the decision runner | pack validation; `decision_policy_matches_eval_results` |
| Decision record | inside `planroom.event/v1` | `crates/contract/src/decision.rs` | schema `$defs` `DecisionRecord`, `DecisionCard` | `decide`, `show`, surfaces | round trip; `decide_all_validates` |
| Eval cases and results | `planroom.eval/v1` | `crates/contract/src/eval.rs` | schema `$defs` `EvalCase`, `EvalResult` | the labeller, `xtask eval`, the policy loader, the report | `evals_recompute_byte_for_byte`; `xtask eval --verify-order` |
| Model request and response (session backend) | `planroom.model-request/v1`, `planroom.model-response/v1` | `crates/contract/src/models.rs` | schema `$defs`; `output.ts`, `input.ts` | the engine, the mod's broker | `session_backend_round_trip`; `session_backend.rs::a_rewritten_request_file_is_refused`; the mod's broker test against a golden request (DR-75) |
| Replay record | `planroom.replay/v1` | `crates/contract/src/models.rs` | schema `$defs` `ReplayRecord` | `planroom-models` in record and replay mode | every replay file parses through both Jev codecs or the Claude decoder; secret scan |
| Change spec | `planroom.change-spec/v1` | `crates/contract/src/handoff.rs` | schema `$defs` `ChangeSpec`; `output.ts` | implementers (`agent`, `patch`), the review | golden spec for S4 in `tests/golden/change-spec-worker-status-via-api.json` |
| Implementer run | `planroom.implementer-run/v1` | `crates/contract/src/handoff.rs` (matches `examples/README.md`) | schema `$defs` `ImplementerRun` | implementers (`result.json`), `handoff report`, `curate`, `import` | `examples/fixture/implementer-runs.jsonl` validates line by line; `fixture_format_is_unchanged` (required fields equal the README's) |
| Review threads | `planroom.review-threads/v1` | `crates/contract/src/review.rs` (matches `examples/README.md`) | input schema `$defs` `ReviewThreads` | the `review_threads` extractor, people, exporters | `examples/fixture/reviews/*.json` validates |
| Mod tool schemas | `mcp__planroom__room_query`, `room_change`, `room_handoff` | the catalogue | `catalogue.ts` (`TOOL_SPECS`) | the chat model, through the mod | `tool_schemas_equal_catalogue`; `surface_tokens` (at most 1,500 projected tokens from the exact bytes: tools 900, the prompt section 300, skills and agents 300); `tool_results_within_budget`; `error_envelopes_reach_the_model`; `handles_round_trip_after_redaction` |
| Mod state contract | `planroom` atoms | `crates/contract/src/view.rs`, which holds every atom's type: `EngineState`, `EngineStatus`, `PaneState`, `LastSeq` and `TurnSeq` (DR-81, DR-85); and `packages/mod/src/atoms.ts` atom list | `plugin/types/index.d.ts` | the mod, `claude plugin validate` | drift check on `index.d.ts`; `tsc -p packages/mod` |
| `/room` command family | `ROOM_COMMANDS` | the catalogue | `catalogue.ts`; the mod's `/room help` | people in Claude Code | `room_commands_snapshot` (S15) |
| Artifact `db` layout and event format | `planroom.snapshot/v1`, `planroom.live-head/v1`, `planroom.inbox/v1`, `planroom.answer/v1`, `planroom.ack/v1`, the `db` rules, `import -`'s stdin lines and result, the share page's transport marker (DR-83) | `crates/contract/src/remote.rs` | schema `$defs` `Snapshot`, `LiveHead`, `InboxRow`, `Answer`, `Ack`, `ImportLine`, `ImportResult`; `output.ts`, `input.ts` | `share`, `import`, the mod's `/room sync`, the Artifact page | the page's transport is type-checked against them; `s11_remote.rs` (validation at import, a forged actor is ignored, a signed owner row applies, rows come only from the mod, snapshot size under 1 MB, no `contains` by default) |
| Configuration | `config.toml` in the workspace's state directory (`.planroom/` or a single repo's `.git/planroom/`, DR-74) and `${XDG_CONFIG_HOME:-$HOME/.config}/planroom/config.toml` (user-level) | `crates/planroom/src/config/defaults.toml` and `crates/contract/src/config.rs` | input schema `$defs` `Config`; `docs/reference/configuration.md` | people, the engine | `configuration_reference_equals_defaults`; `every_key_has_a_default`; `credential_settings_come_only_from_user_config` |
| Plugin and marketplace manifests | Claude Code's `plugin.json` and `marketplace.json` | `plugin/.claude-plugin/plugin.json`, `.claude-plugin/marketplace.json` | none (external formats) | Claude Code | `claude plugin validate --strict` on `.` and `plugin/`; `plugin_versions_agree` (plugin, Cargo workspace, `package.json`, `engine.env`, with the plugin's `-dev` stamp left out; the version is semver) (DR-83); `no_bin_no_symlinks` |
| Budgets file | `budgets.json` | hand-ratcheted: `cargo xtask gen --budgets` writes measurements to `target/budgets.measured.json` and prints the diff, and a person edits the file; plus a `recorded` section from evidence runs | none | `crates/planroom/tests/budgets.rs` | the budget test itself (20% growth fails, the S2 chat proxy and skill bodies included; the S3 cache-hit floor on gateway recordings); `cargo xtask check layout` (a commit that raises a gated number adds a decision record, DR-59; a new key isn't a raise, DR-89) |
| Spend log | `docs/evidence/spend.jsonl` in the main checkout | every live call, appended by the engine under a lock when `PLANROOM_SPEND_LOG` names it by an absolute path, which `cargo xtask` resolves through `git rev-parse --git-common-dir` (DR-65) | none | `cargo xtask live`, the final report | `the_spend_log_stops_live_calls_at_the_cap`; `the_spend_log_is_shared_across_worktrees_and_cwds`; `a_relative_spend_log_is_refused` |
| Art spend log | `docs/art/spend.jsonl` | one line per Iris run, appended by the recorded art step (DR-69), seeded with a row per committed image and one `drafts` row (DR-65) | none | the final report | `docs.rs::every_image_has_its_prompt_and_spend_row` (every image has a prompt and a row, a row may have no image, and the total stays under $5) |
| Feature list | `feature_list.json` | maintained by hand | none | the build process, reviewers | `feature_list_is_complete` (one entry per S1 to S15; every listed test resolves to a test that runs under a CI `cargo xtask` command with no `PLANROOM_LIVE_*` flag and nothing live, or carries a `due` tag naming the milestone that builds it; a test that resolves carries no `due`, and neither does an entry marked `passing`, DR-59, DR-89) |
| Layout lock | `layout.lock` | the gated surfaces' sha256 and the decision record that approved each | written by `cargo xtask gen --layout-lock DR-n` | `cargo xtask check layout` | the layout gate itself: a state check on every run, and a walk over the commits a push or pull request adds (DR-59, DR-70) |
| Resolver environment and status | `PLANROOM_ENGINE`, `PLANROOM_RESOLVE`, `PLANROOM_RELEASE_BASE_URL`; `scripts/planroom status --json`; the `engine_fetching` and `engine_unavailable` envelopes | `crates/contract/src/resolver.rs` (`ResolverStatus`, `ResolverStep`, `UnavailableReason`) and the error registry for the outputs, which are `Envelope`s with an `ErrorObject` (DR-85); the table in [The resolver's environment](#the-resolvers-environment) for the variables | schema roots `ResolverStatus` and `UnavailableReason`, so the mod switches on the generated reason type with a `never` default and a reason it doesn't handle fails its build (DR-87); `output.ts`; `plugin/scripts/contract.sh`, which the resolver sources, so it spells no code or reason and builds every output from a generated template | the mod, skills, agents' Bash, S14's evidence steps | `tests/installer/run.sh::the_resolver_spells_no_code_or_reason_itself`, `resolver_outputs_are_one_document_with_their_size`, `base_url_refuses_plain_http_to_a_non_loopback_host`, `resolve_release_skips_the_checkout`; planned for M5: `every_resolver_output_validates` (see [Drift tests](#drift-tests)) and `status_names_the_step_that_resolved`; the `contract.sh` drift check (DR-69, DR-85) |

`examples/TRUTH.json` (`planroom.truth/v1`) is a test input, not a Planroom contract. Its shape belongs to `examples/README.md`, and `crates/testkit/src/truth.rs` reads it.

## CLI JSON envelope

Every command with `--json` prints exactly one JSON document on stdout, on one line. Logs go to stderr. Text mode renders the same structs.

```rust
// crates/contract/src/envelope.rs
pub const SCHEMA_VERSION: u32 = 1;

pub struct Envelope<T> {
    pub schema_version: u32,                 // const 1
    pub ok: bool,
    pub command: CommandName,                // "ask", "handoff merge"; "planroom" for a usage error that names no leaf
    pub summary: Option<String>,             // optional, on success: the engine-authored line text mode prints first,
                                             // display-sanitized; /room shows it with the reply's handles (DR-85)
    #[serde(skip_serializing_if = "Option::is_none")] pub result: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")] pub error: Option<ErrorObject>,
    pub meta: Meta,
}

pub struct Meta {                            // every field but seq and size is skipped when empty, false, null or zero, and the
                                             // schema gives each its default, so readers stay typed (DR-66)
    pub seq: Seq,                            // the store version this reply reflects: the highest seq inside the one read
                                             // transaction its fold ran in (DR-73); pass it back as `base`
    pub took_ms: u32,
    pub size: Size,                          // { bytes, tokens_est = ceil(bytes / 4) }
    pub next: Option<Cursor>,                // opaque: (seq, last_id, filter hash); map's also holds the last block's level,
                                             // never a name; a cursor from other filters is invalid_argument
    pub truncated: bool,
    pub replayed: bool,                      // an idempotent replay of a stored reply
    pub replayed_from: Option<Seq>,          // a replay of a derived idem: the original seq; the hint says to pass a new --idem
    pub stale_repos: Vec<String>,            // a refresh read skipped these rescans because a live process holds scan:REPO
    pub dirty_repos: Vec<DirtyRepo>,         // scan, focus and map only (M1): { repo, files }, repos with uncommitted changes, untracked
                                             // files included, which never reach the map (DR-71, DR-85); the first 5 by name, `repo`
                                             // masked as repo text
    pub more_dirty_repos: u32,               // the repos with uncommitted changes past those dirty_repos lists
    pub warnings: Vec<Warning>,
    pub models: Option<ModelUse>,            // { jev: Usage, claude: Usage } with calls, cache_hits, cost_usd; None when both call counts are 0
    pub model_requests: Vec<ModelRequestRef>,// session backend: { id, purpose, sha256, rerun: Handle }; rerun is the exact command the
                                             // mod's broker re-runs itself once it fulfils the request (DR-18); the mod sends the
                                             // request file only when its bytes match sha256 (DR-75)
}

pub struct ErrorObject {
    pub code: ErrorCode,
    pub category: Category,                  // Usage | NotFound | Conflict | Permission | Budget | Unavailable | Failed | Internal
    pub retry: Retry,                        // Never | Now | AfterReread | Later
    pub retry_after_ms: Option<u32>,         // with Later, when the caller may try again
    pub message: String,                     // one line, engine-authored
    pub hint: Option<String>,                // an exact command, such as "planroom show ev_01J..."
    pub details: serde_json::Value,          // per code, documented in the registry
}

pub struct Warning { pub code: WarningCode, pub message: String, pub details: serde_json::Value }

pub struct Handle { pub command: CommandName, pub args: Vec<String>, pub label: String }   // an exact invocation
pub struct Empty  { pub reason: EmptyReason, pub line: Option<String>, pub next: Handle } // no blank states; line: the one
                                             // engine-authored line every surface shows, omitted where a surface still words it
pub struct DirtyRepo { pub repo: String, pub files: u32 }                                // files: the count git status lists
pub struct Listing<T> { pub summary: String, pub items: Vec<T>, pub empty: Option<Empty> }
```

A successful reply, and a stale write:

```json
{"schema_version":1,"ok":true,"command":"ask","summary":"4 runtime containers have no alert watching them directly",
 "result":{"question":"unmonitored","summary":"4 runtime containers have no alert watching them directly",
   "items":[{"id":"ws:container:redis","name":"redis","why":"no alert selects a metric it emits or that is scraped from it",
             "handles":[{"command":"ask","args":["why","ws:container:redis","platform:alert:OrdersHighErrorRate"],"label":"Why"}]}],
   "empty":null},
 "meta":{"seq":412,"took_ms":38,"size":{"bytes":545,"tokens_est":137}}}
```

```json
{"schema_version":1,"ok":false,"command":"edit",
 "error":{"code":"stale_write","category":"conflict","retry":"after_reread",
          "message":"Bob renamed this block 2 s ago.","hint":"planroom show ev_01JB...",
          "details":{"event":"ev_01JB...","base":{"name":"cart"},"theirs":{"name":"basket","by":"person:bob","seq":411},
                     "yours":{"name":"shopping"},"card":"card_01JB..."}},
 "meta":{"seq":412,"took_ms":12,"size":{"bytes":433,"tokens_est":109}}}
```

`details.card` is present only for a person's stale write [DR-7]. `summary` is absent on an error, whose `message` is its line.

No level or view is blank: an empty one carries an `Empty`, whose `next` runs today. When the top of the map is empty, every level takes the top's empty state, which says why (DR-87). The engine fills `line` for every reason it sends, from `en.toml`, and text mode prints it; `next.label` is the short action beside it, as the table gives it. The reasons the engine sends:

| `reason` | When | `next` |
|---|---|---|
| `not_scanned` | `focus` or `map` in a directory with no store, where at least one repo has a commit; `N` counts only those repos (DR-57, DR-87) | `scan DIR`, "Map these N repos? About 2 s" |
| `no_repos` | `focus` or `map` in a directory with no repos and no store; `map` returns it `ok`, not `workspace_not_found` | `focus`, "check again": no `scan` there could succeed (DR-85) |
| `repos_unborn` | `focus` or `map` with nothing at the top, where no repo has a commit and at least one has none yet; the line names those repos, the first three, each cut to 40 characters, and counts the rest | `focus`, "check again" after a first commit: a `scan` there would map nothing (DR-87) |
| `repos_unreadable` | `focus` or `map` with nothing at the top, where git can read none of the workspace's repos; the line names them as `repos_unborn`'s does. When no repo has a commit and at least one is unborn, `repos_unborn` wins and names only the unborn repos | `focus`, "check again" after `git status` in each: a `scan` there would map nothing (DR-86, DR-87) |
| `no_intent` | `map --level intent` before any plan, while the top has blocks | `map`, "Show the top of the map" (DR-85, DR-87) |
| `empty_level` | `map --level L` where the workspace has nothing at `L`, while the top has blocks | `map`, "Show the top of the map" (DR-85, DR-87) |
| `nothing_mapped` | the top of the map after a scan found nothing to map, while a repo with a commit could be mapped | `scan ROOT`, "Scan again" (DR-85, DR-87) |
| `no_match` | `find TEXT` where no node's name or ID matches the text | `map`, "Show the top of the map" |
| `no_answer` | `ask QUESTION ARGS` whose pipeline finds nothing on the map, such as `path` between two blocks no flow joins; the line repeats the question's summary with its arguments | `ask`, "Show what you can ask" |
| `nothing_under` | `map --focus ID` where nothing lies under that node at the level shown | `map --level L --focus PARENT`, the node among the blocks beside it at its own level `L`; `map --level L` for a node with no parent (DR-85) |
| `not_in_snapshot` | the Artifact's request state for a view the snapshot doesn't hold | the read request row (DR-77) |
| `nothing_after_cursor` | a `map --cursor` page whose map moved on, so no block is left after the cursor's; the page also carries `stale_page` | `map` with the same flags and no cursor, "Show the first page" (DR-73) |
| `lens_shows_nothing` | `map --lens X` where X has `kinds` and leaves none of the level's blocks: none is of its kinds or touched by its relations; the line names the lens and the level | `map --level L --lens all`, with the same `--focus`, "Show every block at this level" (DR-21) |

`find` and `ask QUESTION` return the `not_scanned`, `no_repos`, `repos_unborn` or `repos_unreadable` state `map` returns in the same directory, as long as nothing is scanned there.

Redaction is by field [DR-64]. A field that holds repo-derived free text carries the `#[redact]` mark, `#[schemars(extend("x-redact" = true))]`, in `crates/contract/src/`: `Evidence.contains`, `Hint.value` and `Hint.attrs`, `Node.name`, the string values of `Node.props` and `Edge.props`, the names a view shows (`MapNode.name`, `Crumb.name`, `MiniBlock.name`, `AskItem.name`, `FindItem.name`), `DiffLine.hunk`, `PresenceRow.doing`, `HandoffStep.note` (a note can quote a test's output) and `Card.vars` (the names a card's lines fill in) [DR-91]; the engine lines built from that text: `Envelope.summary`, `Empty.line`, `Warning.message`, `ErrorObject.message` and `ToolError.message`, an event frame's `summary`, `DecisionCard.question` and `why`, `DiffLine.summary`, `DriftLine.summary`, `PaneState.error`, `AskResult.summary`, `AskItem.why`, `FindResult.summary` and `ImportedRow.reason`; and, as their types land, `ContextExcerpt.excerpt`, `RunStep::Command.output_excerpt` and prompt text. An error's or a warning's `details` are masked by key: each code's registry entry in `crates/contract/src/errors.rs` names, in its `redact`, the keys that hold repo-derived free text, such as `git_failed`'s `stderr` and the `repo` of `repo_unborn` and the other scan warnings, and `docs/reference/errors.md` marks them (masked). A key that is a handle, an ID, a field name or a command, such as `details.next`, `details.field` or `details.closest`, is never marked, and neither is a `hint`: it is an exact command, like a handle's `args`. `cargo xtask gen` writes the marked field paths into `crates/kernel/src/redact_fields.rs`: for each output type that holds a marked field, the paths from it to each one, such as `MapView`'s `nodes[].name` (`.` joins fields, `[]` is an array's items, `{}` a map's values); and, for each output type that holds an error or a warning, the path to each one, beside each code's masked `details` keys. `kernel::redact` masks secrets in every string under exactly those fields, keys excepted, on every output path [DR-89], CLI text and `--json` included; text mode masks each line it prints, since each is one of those fields. The engine tokens in a masked line stay whole: commit SHAs, seqs, counts and levels. So does an ID: a whitespace-delimited token shaped like a node ID, `namespace:kind-short:key` with a lowercase `snake_case` kind-short and neither the namespace nor the key holding `=` or `://`, is left alone by the high-entropy rule, since an ID is never redacted and the reply carries the same ID unmasked in its ID fields. So `access_token:dir:src` and `alpha:file:lib/v3/OAuth2PKCEFlow.ts` stay whole, while an ID-like token that holds `=` or `://`, such as `db_PASSWORD=hunter2:dir:src`, is masked like any other text. The named shapes (a private key, a DSN's user info, an assignment's value, a known token shape) are masked wherever they sit, an ID-shaped token included, so a secret never hides behind the shape. An `=` always assigns, and a `:` assigns only when whitespace or a quote follows it or a quote comes before it (`DB_PASSWORD: x`, `"api_token":"x"`) (`kernel::redact`'s `the_engine_tokens_of_a_line_survive_masking`, `an_id_stays_whole_by_its_shape`, `an_id_shaped_token_holding_an_assignment_or_a_url_is_masked`, `a_named_secret_shape_is_masked_inside_an_id_shaped_token` and `a_colon_assigns_only_before_whitespace_or_a_quote_or_after_a_quote`). `contract/tests/schema.rs::every_redacted_field_is_listed` checks that every mark is listed and that none is on an ID. It never touches an ID, a handle, a cursor or a hash, so a handle's `label` names no repo text: it is a short action, and the line beside it names what it acts on (`planroom::text::LABELS`, checked by `text.rs::no_label_takes_repo_text`); join keys such as `Hint.key`, `Edge.via` and `Node.aliases` stay unmarked, and an edit's `fields` and `over` stay unmarked until the `edit` leaf gives them types [DR-91]. `PLANROOM_RAW=1` turns it off for the truth comparator alone, and only on a read command's `--json` stdout: model requests, session request files, the snapshot, `live/head`, acks, tool results and logs stay redacted whatever it says. The mod's guard denies that variable to agents (DR-75).

### Error codes

`contract::ErrorCode` is the only place a code is spelled; a source scan fails on a code string anywhere else but a generated file, such as `plugin/scripts/contract.sh` (DR-69). Exit codes follow the category.

| Code | Category | Exit | Retry | Meaning | `details` |
|---|---|---|---|---|---|
| `invalid_argument` | usage | 2 | never | bad flags or request, a parent change that would make a cycle, a relative `PLANROOM_SPEND_LOG`, a `PLANROOM_LIVE_*` flag with no spend log, or a credential bound for a host off the allow-list (DR-48, DR-65). Under `--json` a bad flag or value is one envelope, never clap's text; one that names no leaf has `command: "planroom"` (DR-85) | `field`, the request field the error names; `synopsis`, the leaf's one-line synopsis, generated from clap; `schema`, the `schema --command LEAF` handle, or `schema --commands` with no leaf, which `hint` names as an exact command (DR-66, DR-85), except where `field` is an environment variable, whose `hint` stays the engine's exact command that sets it (DR-93); `cycle`, the node IDs, for a parent cycle; `lenses`, the lenses a request may name (`all` and each loaded pack's), for an unknown `--lens`. Every CLI `invalid_argument` carries `field`, `synopsis` and `schema`; one over HTTP doesn't until M2's budgets slice, and the resolver's carries only `field` |
| `ambiguous_ref` | usage | 2 | never | an ID suffix matched several records | `candidates`; for an `ask` argument, the `level` the candidates share |
| `base_required` | usage | 2 | never | a write to user-authored state without `base` | `hint` names `meta.seq` |
| `actor_required` | usage | 2 | never | a write with no `PLANROOM_ACTOR`, no person token and no terminal | `hint` shows `PLANROOM_ACTOR=agent:<id>:<role>` |
| `workspace_not_found` | not_found | 3 | never | `scan` found no repos at the path, or a command that needs a store found none, such as a read whose `--workspace` names a directory with no store; `focus` and `map` instead return `ok` with an empty state, one of the [empty reasons](#cli-json-envelope) (DR-57, DR-85) | `path` |
| `not_found` | not_found | 3 | never | unknown ID, question or file | `closest` |
| `stale_write` | conflict | 4 | after_reread | a field changed since `base`; stored as a conflict, never merged | `event`, `base`, `theirs`, `yours`, `card` |
| `idempotency_mismatch` | conflict | 4 | never | the same `idem` with a different request | `idem`, `stored_event` |
| `already_resolved` | conflict | 4 | never | the card was already picked | `by`, `at`, `pick` |
| `handoff_conflict` | conflict | 4 | never | the merge preflight failed; every repo untouched | `repo`, `files` |
| `person_required` | permission | 5 | never | an agent ran a person-only command or input; a person token was missing, stale, spent or for another command; or `handoff start`, `merge`, `revert`, `person pair`, `open` or `person init --rekey` came through the terminal path, except `open` where no mod key is registered (DR-64, DR-75, DR-81) | `card` or `command` a person can run |
| `person_key_mismatch` | permission | 5 | never | `person init` found another key registered, such as after a reinstall or under another `CLAUDE_CONFIG_DIR`; the pane draws its fallback state (DR-60) | `registered_at`, `surface`; `hint` names the pane's Use this session's key button, which runs `person init --rekey` with a press (DR-75) |
| `consent_required` | permission | 5 | never | a model-only command before the person allowed this provider for the workspace; only the person's pick on the consent card clears it (DR-66) | `provider`, `card`; `hint` names the card |
| `path_refused` | permission | 5 | never | a read named a path outside the workspace's repos and scanned files, a non-person named a credential path, or a repo's git directory belongs to another user (DR-78) | `path`, `reason` (`outside_workspace`, `credential` or `other_owner`); `hint` names `planroom scan PATH` |
| `repo_config_refused` | permission | 5 | never | a checkout or merge in a workspace repo whose local git config names a command git would run: a filter, a diff command or textconv, a merge driver, an editor, a pager, an ssh command or an include (DR-64) | `repo`, `keys`, `card` |
| `unauthorized` | permission | 5 | never | HTTP: no bearer token, a foreign `Origin`, a missing `Origin` where [the origin rules](#http-api-and-live-stream) need one, or a used or expired login code (DR-82) | |
| `budget_exceeded` | budget | 6 | later | a spend or size guard stopped a model-only command; decisions keep their default instead. `guard` names it: `live_tests_total_usd` for the spend log's cap, whose `limit` is the cap less the $0.25 margin, or `replay_only` for a live call outside record mode, refused before it is sent (DR-65, DR-89) | `guard`, `limit`, `spent` |
| `model_unavailable` | unavailable | 7 | later | a model-only command found Jev or Claude unreachable; other commands return the deterministic result with the `deterministic_fallback` warning | `provider` |
| `store_busy` | unavailable | 7 | now | still locked after the 5 s busy timeout | |
| `port_in_use` | unavailable | 7 | never | another program holds the workspace's recorded canvas port; `serve` never moves unasked, so a tab's pending edits keep their origin (DR-78) | `port`, `fix` (stop that program and press `e`, or `planroom serve --port N`) |
| `git_too_old` | unavailable | 7 | never | the `git` on `PATH` is older than 2.34.1, the oldest whose plumbing the adapter uses; `scan` and `doctor` return it (DR-78) | `found`, `minimum`, `fix` |
| `engine_fetching` | unavailable | 7 | later, with `retry_after_ms` 5000 | the resolver's envelope to a caller other than the mod, printed from the generated `contract.sh`: a prefetch held the fetch lock for 20 s (DR-69, DR-85) | `progress`, one line on who holds the lock |
| `engine_unavailable` | unavailable | 7 | never | the resolver's envelope, printed from the generated `contract.sh`, when it can't hand back an engine: the download failed, the version has no release, a checkout's build is stale, `PLANROOM_ENGINE` names no executable file, or `PLANROOM_RESOLVE=checkout` finds no build; it never starts a build for a caller other than the mod (DR-69) | `reason` (`download_failed`, `no_release`, `stale_build`, `no_toolchain`, `override_missing` with the fix `unset PLANROOM_ENGINE`, or `no_build` with the source build's offer, DR-81); `fix` (an exact command, such as `scripts/planroom build`); with the source build's offer, `est_minutes` and `canvas`, whether Node and pnpm are present to build it; for `no_toolchain`, `missing`, the tool. For `download_failed` the archive's and `SHA256SUMS`'s URLs are still owed, in M5 (DR-78, DR-85) |
| `schema_mismatch` | unavailable | 7 | never | the store or a file is from a newer version, or this process is older than the store, checked at open, again inside every commit and duty transaction (DR-73), and by a live `watch` whenever the store changes; or a data-root file's `min_reader_version` is above this engine's (DR-78) | `found`, `supported`, `writer_version` (the engine version that wrote the store, so the mod names the update or the restore, DR-69); `file`, for a file other than the store, such as `workspaces.json` (DR-85) |
| `packs_mismatch` | unavailable | 7 | never | this process resolved another pack set than the one the store records; a write is refused. Packs load from their copies in the state directory, so only engine version skew causes it (DR-62) | `recorded`, `resolved`; `hint` names a restart for an older engine, or the update for a newer store |
| `replay_missing` | unavailable | 7 | never | strict replay found no record (CI and tests) | `key`, `path` |
| `serve_limit` | unavailable | 7 | later | HTTP: the local server hit one of its [limits](architecture.md#local-server-and-live-stream): a request still running at 10 s (it may still finish and commit), or a 65th live stream | `limit` (`request_time` or `stream_clients`), `max` |
| `pack_invalid` | failed | 8 | never | a pack failed validation | `file`, `line`, `reference` |
| `git_failed` | failed | 8 | never | git exited non-zero; stderr redacted. A scan returns it only when git can't list a root repo's submodules, which leaves the repo set unknown; it skips any repo git refuses, with `repo_skipped` (DR-86) | `argv`, `stderr`; `hint` is `git -C PATH status`, which shows why (DR-86) |
| `internal` | internal | 70 | never | a bug | `report` (a path to a redacted report) |

A code that only a person can clear has retry `never` and names the card or command a person can use, in `details` or `hint`; `the_documented_error_table_is_the_error_codes` enforces it (DR-66). Older readers map an unknown code to its category and exit code. A job or event recorded with a code this binary doesn't know reads as `internal` with `details.recorded_code`.

### Warning codes

| Code | Meaning |
|---|---|
| `claimed` | the target is claimed by someone else; `details {by, until}` |
| `truncated` | the result hit its budget; the summary and first page are kept. For `map`, `details {kept, total, budget_tokens, next}`, where `next` is the drill-down handle in [Token budgets](architecture.md#token-budgets-s7) (DR-85). From the CLI's check of a whole reply against its leaf's budget, `details {budget_tokens, left_out, codes}`: `left_out` counts the warnings it left out after those it kept in order, and `codes` counts them by code. A page of `ask`'s listing that leaves out an entry too long for any page adds `entries_left_out`, the count of those entries. A result that passes the budget alone keeps only the leaf's own `truncated` warning and leaves out every other, so `left_out` counts them all. Only the message marks that case: it says the reply passes its budget (DR-86) |
| `unresolved_ref` | an alias or a linker join had zero or several candidates; no edge was made |
| `evidence_dropped` | an evidence item failed verification at ingest; `details {repo, count, items}`, where `items` is a sample of the dropped items |
| `partial_parse` | files parsed with error nodes, so their facts are weak; `details {repo, count, paths}`, where `paths` is a sample: the first 10 in sorted order (DR-86) |
| `partial_scan` | an extractor reached its file or byte bound, or a partial clone lacks blobs, which a scan never fetches; `details {extractor, repo, count, uncovered}`, where `count` counts every uncovered file and `uncovered` is a sample: the first 10 in sorted order (DR-86). `focus` repeats it for each repo whose active snapshot's scan was cut, from the count the scan event keeps, with `uncovered` empty |
| `unnamed_paths` | files whose paths aren't UTF-8, or hold a newline or a backslash, which no node ID can name, so the scan reports them and doesn't map them; `details {repo, count, paths}`, where `paths` is a sample: the first 10 in sorted order, each escaped (`\xe9` for a byte that isn't UTF-8, `\n`, `\\`) |
| `deterministic_fallback` | no model was reachable, allowed or within budget; deterministic tiers, `low` kinds keep their default and cards pre-select it |
| `workspace_untrusted` | the workspace's `.planroom/` is untrusted, so its config wasn't loaded; `details {card}` |
| `config_key_ignored` | a user-level key in a workspace's config, or `api_key` in a user-level config that isn't mode 0600; `details {file, key}` |
| `inferred_unconfirmed` | the result includes inferred facts no person confirmed |
| `cache_regression` | an unchanged Claude prefix wrote the cache again inside its TTL |
| `cache_not_applied` | a marked Claude prefix reported no cache write and no cache read, so it is below the model's minimum (DR-65) |
| `hidden_characters` | a diff hunk holds bidi controls or zero-width characters, which surfaces show as markers; `details {path, lines}` (DR-64) |
| `outside_touch_points` | a hand-off's diff changes files outside the components its spec's touch points name; `details {files}`; the review card expands those hunks before it offers Merge (DR-75) |
| `scan_changed_under_edit` | a fact under a person's override changed; see Diffs |
| `stale_page` | a page cursor's `seq` is behind, or its snapshot was pruned so the page folds at the current `seq` (DR-73); unchanged rows have no gaps or duplicates |
| `model_request_pending` | the session backend is waiting for the mod |
| `store_migrated` | this command migrated the store; `details {from, to, backup}`, the `VACUUM INTO` copy (DR-53) |
| `temporary_host` | this host loses its files with its container, so a redacted copy of the backup is also kept at a local ref; `details {repo, ref}`, and no push command, so no envelope suggests a push; the mod shows its own copy-out line once (DR-53, DR-75) |
| `reconcile_paused` | a repo's `HEAD` moved other than forward along its tracked branch, so the map follows it and the reconciler skipped it; `details {repo, head, tracked_branch, reason}`, `reason` `off_branch` or `rewritten` (DR-71) |
| `workspace_nested` | another workspace's `.planroom/` sits inside this one, or this one inside it; `details {other_root}` (DR-71) |
| `repo_unborn` | a repo has no commit yet, so it maps nothing until its first commit; the rest of the workspace maps. `details {repo, branch}` (DR-86) |
| `repo_skipped` | git refused a repo, so the scan left it out and its last map stays; the rest of the workspace maps. `details {repo, reason, fix}`, where `reason` is `other_owner` or `git_failed` and `fix` is one line with an exact command (DR-86) |
| `scan_superseded` | the scan read a state that's no longer the repo's, so its map stays as it was; `details {repo, commit, reason}`, where `commit` is the commit this scan read and `reason` is `head_moved` (the repo's `HEAD` moved on while the scan read it) or `repos_changed` (another scan moved the repo after this one began, and the workspace's repos changed since this one read them) (DR-86, DR-87) |
| `duty_failed` | a background duty failed and hasn't succeeded since; `details {duty, at, log}`, the log under the data root (DR-58) |

## Command catalogue

```rust
// crates/contract/src/command.rs
pub struct CommandSpec {
    pub path: &'static [&'static str],      // ["handoff", "merge"]
    pub idea: Idea,                          // Levels | Lenses | Intent | Diffs | Decisions | Setup; `planroom --help` shows the five
                                             // ideas and one line naming `planroom help --all`, which alone lists Setup (DR-77)
    pub summary: &'static str,               // one line: help, tool description, docs
    pub actor: ActorRule,                    // Any | PersonOnly | Mixed: the leaf table names the inputs that need a person
    pub class: Class,                        // Read | Change | Handoff
    pub budget: Budget,                      // Tokens(u32), the default output budget, or Stream for watch, serve and export;
                                             // every other agent-facing leaf has one, schema and doctor included (DR-21)
    pub needs_base: bool,                    // writes to user-authored state; the only commands whose missing idem is derived
    pub spends: bool,                        // can make a paid model call (intent next, decide --run, curate); no band names it (DR-66)
    pub errors: &'static [ErrorCode],
    pub undo: Undo,                          // Edit | Compensate(&'static str) names the reversing command | Query (changes no
                                             // authored state) | NotUndoable(&'static str) gives the reason: only `share` (DR-55)
    pub tool: Option<Tool>,                  // RoomQuery | RoomChange | RoomHandoff
    pub room: bool,                          // reachable as /room ...
}

pub enum Command {                           // one variant per leaf; each request derives clap::Args, Serialize, JsonSchema
    Map(MapReq), Show(ShowReq), Find(FindReq), Focus(FocusReq),
    Ask(AskReq),
    IntentNew(IntentNewReq), IntentNext(IntentNextReq), IntentPropose(ProposeReq), IntentLint(LintReq),
    IntentVersion(VersionReq), IntentList(ListReq),
    Edit(EditReq), Claim(ClaimReq), Who(WhoReq),
    Diff(DiffReq),
    HandoffStart(HandoffStartReq), HandoffReport(HandoffReportReq), HandoffCollect(KeyReq),
    HandoffMerge(KeyReq), HandoffRevert(KeyReq), HandoffCancel(KeyReq),
    Curate(CurateReq),
    DecideList(DecideListReq), DecidePick(DecidePickReq), DecideRun(DecideRunReq),
    Review(ReviewReq),                       // { id, verdict: Accept | Reject } plus WriteCommon
    Scan(ScanReq), Share(ShareReq), Import(ImportReq), Serve(ServeReq), Watch(WatchReq),
    PackAdd(PackAddReq), PackRemove(PackRemoveReq),   // person only; write the workspace config and the store's pack record (DR-62)
    PersonInit(PersonInitReq), PersonPair,            // person only: register the mod's public key from stdin, or --rekey to replace it with
                                                      // a token under the old key or a press (DR-60, DR-75); mint a pairing code (DR-67)
    Open(OpenReq), Export(ExportReq), Schema(SchemaReq), Doctor(DoctorReq), Version,
}

pub struct WriteCommon {                     // flattened into every write request
    pub base: Option<Seq>,                   // required when needs_base; validated as base_required
    pub idem: Option<String>,                // when absent: derived for a needs_base command, else a fresh uuid v7 per invocation;
                                             // a one-transaction command keys its events idem#1, idem#2, ...; a multi-step one
                                             // (handoff merge, revert) keys each step idem#<step> and resumes from the last one
    pub client: Option<String>,              // the client ID of the writer instance: a canvas tab, a mod session; a flush from a tab's
                                             // pending store sends the minting tab's ID, never the flusher's (DR-73); else an agent's own
                                             // ID from PLANROOM_ACTOR, or cli:<pid> for a person at a terminal (DR-60);
                                             // required over HTTP, where each tab is its own writer instance (DR-92)
    pub project: Option<NodeId>,
}

// crates/contract/src/requests.rs: the M1 leaves' requests and results; a target is an ID, an ID suffix or a name
pub struct MapReq { pub level: Option<Level>, pub lens: Option<String>, pub focus: Option<String>, pub detail: Detail,
                    pub cursor: Option<Cursor> }             // in command.rs; cursor: meta.next of the page before
pub struct EditReq { #[serde(flatten)] pub op: EditOp, #[serde(flatten)] pub common: WriteCommon }
#[serde(tag = "op", rename_all = "snake_case")]
pub enum EditOp {                            // a clap subcommand: `edit [--base SEQ] OP ...`
    Move { id: String, parent: String },
    Group { name: String, ids: Vec<String> },               // a group made by hand; one undo removes it and restores every parent
    Rename { id: String, name: String },
    Hide { id: String },
    Pin { id: String, view: String, x: f64, y: f64 },       // pos@VIEW
    Relate { from: String, to: String, kind: String },      // an intended relation
    Unrelate { from: String, to: String, kind: String },    // removes an intended relation
    Forbid { from: String, to: String, kind: Option<String> },  // a boundary rule; None is invalid_argument until a rule
                                                            // over every kind has a stored form (DR-92)
    Undo { event: String },
    // `set`, `text` and `propose` (architecture.md's CLI table) arrive with the plan pack's intent leaves, which fix
    // their fields (DR-92)
}
pub struct EditResult { pub event: EventId /* the command's first, which `edit undo` takes */, pub count: u32,
                        pub status: Status /* applied, or ghost for an agent's */,
                        pub targets: Vec<String> /* the changed IDs that fit the budget */, pub more_targets: u32,
                        pub undo: Handle /* edit --base SEQ undo EVENT */ }
                                             // the line naming what changed is the envelope's summary; a write that
                                             // replays its key sets meta.replayed, and meta.replayed_from when derived
pub struct ReviewReq { pub id: String, pub verdict: Verdict, #[serde(flatten)] pub common: WriteCommon }
pub struct IntentNextReq { pub stage: Option<String>, #[serde(flatten)] pub common: WriteCommon }
pub struct ProposeReq { pub file: String /* a path or - */, #[serde(flatten)] pub common: WriteCommon }
pub struct LintReq { pub project: Option<String> }
pub struct AskReq { pub question: Option<String>, pub args: Vec<String>,
                    pub cursor: Option<Cursor> /* --cursor C: the next page of ask alone's listing */ }
pub struct AskResult { pub question: String, pub summary: String, pub items: Vec<AskItem>, pub empty: Option<Empty> }
pub struct AskItem { pub id: String /* a node, or with ask alone a kind, lens or question */, pub name: String,
                     pub why: String, pub handles: Vec<Handle> }
pub struct FindReq { pub text: String }
pub struct FindResult { pub summary: String, pub items: Vec<FindItem>, pub empty: Option<Empty> }
pub struct FindItem { pub id: NodeId, pub name: String, pub level: Level, pub kind: Kind,
                      pub handles: Vec<Handle> /* map --level L [--focus PARENT], the match among the blocks beside it;
                                                  then map --focus ID when it holds blocks */ }
pub struct DoctorReq { pub pack: Option<String> /* --pack DIR */, pub update: bool /* rewrite tests/expected.jsonl */ }
                                             // --restore and the general report join it as flags
```

| Leaf | Idea | Actor | Class | Tool |
|---|---|---|---|---|
| `map`, `show`, `find`, `focus` (with `--picker NAME`, the pane's pickers, DR-77) | Levels | any | read | `room_query` |
| `ask` | Lenses | any | read | `room_query` |
| `intent new`, `intent next`, `intent propose`, `intent lint`, `intent list` | Intent | any (agent writes are ghosts) | change, read | `room_change` |
| `intent version` | Intent | person only | change | none |
| `edit` (every op, including `undo`) | Intent | any (agent writes are ghosts) | change | `room_change` |
| `claim`, `who` | Intent | any | change, read | `room_change`, `room_query` |
| `diff` | Diffs | any | read | `room_query` |
| `handoff start`, `handoff merge`, `handoff revert`, `handoff cancel` | Diffs | person only; `start`, `merge` and `revert` refuse the terminal path (DR-64) | handoff | none |
| `handoff report`, `handoff collect` | Diffs | any | handoff | `room_handoff` |
| `curate` | Diffs | any | change | `room_change` |
| `decide` (list) | Decisions | any | read | `room_query` |
| `decide --run` | Decisions | any | change | `room_change` |
| `decide ID CHOICE` | Decisions | person only | change | none |
| `review ID accept\|reject` | Decisions | mixed: an agent may reject only its own ghosts | change | none |
| `scan` | Levels (DR-77) | mixed: a credential path needs a person | change | `room_change` |
| `import` | Setup | mixed: Artifact inbox rows and event exports need a person, except inbox rows a paired device signed (DR-67) and read requests (DR-77) | change | none |
| `share` | Levels (DR-77) | any | handoff | `room_handoff` |
| `pack add`, `pack remove` | Setup | person only | change | none |
| `person init`, `person pair` | Setup | person only; the guard denies both to agents, and `pair` and `init --rekey` refuse the terminal path (DR-60, DR-67, DR-75) | change | none |
| `open` | Levels (DR-77) | person only, from a mod press; refuses the terminal path (DR-75), except in a workspace with no registered mod key, where a `/dev/tty` confirmation naming it serves (DR-81) | read | none |
| `doctor` | Setup | mixed: `--restore` needs a person | read, change | none |
| `serve`, `watch`, `export`, `schema`, `version` | Setup | any | read | none |

`review CARD accept|reject` also picks a card a write dealt, through the same `Review { of }` body: a second pick returns `already_resolved`, and an agent's pick `person_required`. The Apply card is picked through its ghosts, one `review` each, and resolves with its last; `review` on the card itself returns `invalid_argument` naming its ghosts until M2's batch review. From M2, `decide ID CHOICE` picks the same `card` events, read beside its `DecisionRecord`s (DR-89).

`planroom schema --commands` prints the catalogue as JSON, paged under 1,500 tokens: each leaf's path, idea, actor rule, budget, undo class and its `schema --command LEAF` handle. `schema --command LEAF` returns that leaf's request and result types with the closure of their `$defs`, summary first, and `schema --def NAME` one type the same way, each under 1,500 tokens. To fit, `schema --command` drops descriptions, first from the result's types, then from the request's, and keeps every shape whole; its summary says which it dropped (DR-86). `schema` alone returns a summary with the schema's `$id`, version and those handles, and `schema --out DIR` writes both views to files. `catalogue.rs::every_leaf_has_a_budget_or_streams` checks the budgets (DR-21).

`PaneModel.next`, every band Button and every pack `[[suggestions]]` action name only a leaf whose undo class isn't `NotUndoable`, whose class isn't `Handoff`, and whose `spends` is false; the pack loader refuses any other with `pack_invalid` (DR-54, DR-66).

A person action carries `PLANROOM_PERSON_TOKEN=<at>.<sig>`, made with the mod's private key (DR-72, DR-82):

- `at` is Unix time in whole seconds, and `sig` is the base64url, without padding, of the 64-byte `r ‖ s` ECDSA P-256 signature over SHA-256 of the message.
- The message is `command ‖ args ‖ idem ‖ base ‖ at`, each field followed by one NUL byte: `command` is the leaf's path joined with spaces (`person init`); `args` is the canonical JSON array of the argv words after that path, less `--json` (`--idem`, `--base` and their values stay in); `idem` and `base` are those flags' values or empty; `at` is its decimal text. `kernel::keys::person_token_message` builds it, and the mod's `tokenMessage` matches it byte for byte.
- The key `person init` reads on stdin is one line, the base64url of the 65-byte SEC1 uncompressed point; its fingerprint is the first 16 hex characters of the sha256 of those bytes.

The engine verifies the token against `meta.person_pubkey`, the registered public key; no store, backup or export holds anything that can sign (DR-72). It refuses a token more than 30 s from its own clock, one for another command, and one already spent, and removes it from every child's environment (DR-64). With no key registered, `person init`'s token must verify under the key on its own stdin.

`planroom open --json` returns `OpenResult { opened: bool, empty: Option<Empty> }`: `opened` is false where no browser can be seen, and `empty` then has the reason `no_browser` and a `next` that names `share`, which the pane offers as `/room share` (DR-29).

`planroom version --json` returns `VersionInfo { version, git_commit, target, schema_version, catalogue_hash }`, `target` being the Rust target triple the binary was built for (DR-82). The catalogue hash is the sha256 of the whole catalogue as `planroom schema --commands` lists it across its pages, which `cargo xtask gen` also writes into `catalogue.ts`, so the mod and `serve` detect a mismatched engine (DR-58).

## HTTP API and live stream

`planroom serve` binds `127.0.0.1` on the workspace's port, which `workspaces.json` records, writes its record with the control token at `DATA_ROOT/serve/<port>.json` (mode 0600 in a 0700 folder), and writes `serve.json` without the token in the state directory (mode 0600) once there is one [DR-29, DR-88]. It starts in a directory no scan has made a workspace yet, makes no state directory there, and serves the empty map, whose next action is `scan`; the live stream sends that scan's frames, and `serve.json` appears with the state directory the scan makes [DR-82]. The first `serve` picks a free port no other workspace has recorded. When another program holds the recorded port, `serve` exits with `port_in_use` instead of moving, and `serve --port N` records a new one (DR-78).

| Route | Request | Reply |
|---|---|---|
| `POST /api/v1/{command path, slash-joined}` | the command's request JSON, including `base` and `idem`; `Idempotency-Key` header accepted | the envelope; HTTP status by category: usage 400, not_found 404, conflict 409, permission 403 (`unauthorized` 401), budget 429, unavailable 503, failed 422, internal 500 |
| `GET /api/v1/stream?since=SEQ` | `Last-Event-ID` resumes; read with `fetch`, so the token goes in a header | SSE with `event: <type>`, `id: <seq>`, `data: <StreamFrame JSON>` |
| `PUT /api/v1/layout` | `{view_key, structure_hash, positions}` | `204`; a cache, last-writer-wins |
| `POST /api/v1/code` | `CodeReq {code_sha256}`, with `Authorization: Bearer <control_token>` from the server's record under the data root | `204`; the code is valid once, for 60 s, for this `serve` process |
| `POST /api/v1/session` | `{code}` from the URL fragment | `200` with the envelope, `result: {token}`: a random bearer token the tab keeps in `sessionStorage`; a code works once, and a second try at it revokes the token the first issued (DR-64) |
| `POST /api/v1/drain` | `{token}`, the control token from the server's record under the data root | `202`; the server finishes in-flight requests and exits |
| `GET /`, `GET /assets/*` | | the canvas build embedded in the binary; a binary built without it serves one engine-rendered page naming `cargo xtask gen --canvas` (DR-58) |

```rust
// crates/contract/src/stream.rs: one frame per SSE message and per watch NDJSON line
#[serde(tag = "type", rename_all = "snake_case")]
pub enum StreamFrame {
    Event { seq: Seq, kind: EventKind, target: Option<String>, project: Option<NodeId>,
            actor: Actor, status: Status, summary: String },       // at most 2 KB
    Presence { actors: Vec<PresenceRow> },
    Duty { name: String, state: DutyState, seq: Option<Seq> },    // Started | Done | Failed
    ModelRequest { id: String, purpose: String, est_tokens: u32 },
    Reset { seq: Seq },                                           // client is too far behind: refetch
    Ping,
}
pub struct PresenceRow { pub actor: Actor, pub surface: Surface, pub client_id: String,   // one row per writer instance
                         pub doing: Option<String>, pub focus: Option<NodeId>, pub seen_at: Timestamp }
```

Every API request except `session`, `code` and `drain` carries the tab's `Authorization: Bearer <token>`. A foreign `Origin` is always `unauthorized`. A browser sends no `Origin` on a same-origin `GET`, so a `GET` may instead prove its origin with `Sec-Fetch-Site: same-origin`; a `GET` with neither, or with `Sec-Fetch-Site` `cross-site`, `same-site` or `none`, is `unauthorized`. Every other request from the page carries an exact `Origin` of `http://127.0.0.1:PORT` or `http://localhost:PORT`. `code` and `drain`, which local processes call with the control token, may carry no `Origin`, never a foreign one [DR-29, DR-82].

Every response carries exactly this CSP, what the canvas needs and no more (React Flow places blocks with inline styles): `default-src 'none'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self'; font-src 'self'; connect-src 'self'; worker-src 'self'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'` [DR-82].

A running server's record (`planroom.serve/v1`, internal) is `{format, port, pid, version, catalogue_hash, control_token, started_at, binary_mtime_ms}`, the last being the server binary's mtime when it started, which the handshake compares for two builds of one version. It lives at `DATA_ROOT/serve/<port>.json`, mode 0600 in a 0700 folder, keyed by the port `workspaces.json` gives the workspace alone, so a tree read of the workspace, such as `grep -r x .`, never reaches the control token [DR-88]. `serve.json` (mode 0600 in the 0700 state directory) holds the same fields but `control_token`, and is written only while the state directory exists. A later `serve`, and `open`, read the record under the data root; an older engine's `serve.json` that still holds `control_token` is read for it, so a newer `serve` can drain that server. A server removes both when it exits, on a drain, a signal or a `schema_mismatch` [DR-82]. `serve` reuses a server only when `version` and `catalogue_hash` both match its own, and only one on the port the workspace records, so a copied workspace's `serve.json` names no server of its own (DR-58). The port is the workspace's own, recorded in `workspaces.json` as `{root, workspace_id, port}`, and no two entries share one. `serve` binds only that port and exits with `port_in_use` when another program holds it; `serve --port N` records a new one (DR-78).

## Store schema and migrations

- The DDL is in [Architecture](architecture.md#store-schema). The source is `crates/store/migrations/NNNN_name.sql`, applied in order by `PRAGMA user_version`, which equals `meta.store_version`.
- The store isn't a public contract: only `planroom-store` reads it. It's versioned because people keep stores across upgrades.
- A migration that touches `events` ships with `crates/store/tests/fixtures/store-vN.db` from the previous release and a test that opens it, migrates it and folds it.
- Every `store::commit` and duty transaction reads `PRAGMA user_version` after `BEGIN IMMEDIATE` and aborts with `schema_mismatch` when it isn't the process's own version, so an older process never writes into a store a newer one migrated while it waited (DR-73).
- A binary that finds a newer `store_version` refuses with `schema_mismatch` naming the version that wrote the store, and the mod shows one line once instead of restarting it: the update commands when a release wrote the store, or `planroom doctor --restore` when a `-dev` build did (DR-69). One that finds an older one asks any older `serve` to drain, writes `VACUUM INTO DATA_ROOT/backups/<workspace_id>/store-vN.db` and checks it with `PRAGMA integrity_check` (the last 3 are kept), then migrates and folds in one transaction, rolling back on any failure, and reports `store_migrated` [DR-53]. `planroom doctor --restore BACKUP` (person only) puts a copy back, the downgrade path. It first writes `store-pre-restore-<at>.db` the same way, so restoring that copy undoes it, and sets the person-authored events above the restored copy's last `seq` aside as a replay set, `replay-<at>.ndjson`, named in `meta.replay_set`. The next successful migration replays the set through `store::commit` with each event's actor and `idem`, and with the restored copy's last `seq` as each edit's base, so a field written after the restore conflicts with it, whoever wrote it, and becomes a person card (DR-53, DR-73).
- `planroom export` writes NDJSON: a header line `{"format":"planroom.event/v1","workspace_id":...,"store_version":...}`, then one `Event` per line in `seq` order. A person's `planroom import` of that file replays it into a store with no authored events, keeping each event's recorded actor and stored status.
- The automatic backup, `DATA_ROOT/backups/<workspace_id>/events.ndjson`, is the same format, written through after each person-authored commit: under a lock on the file, every event above `meta.backup_seq` is appended and fsynced, then the mark advances. A restore sorts by `seq`, dedupes by `idem` and keeps each event's stored status. It skips `scan` events and writes one `scan` event holding the last reconcile outcome with `snapshot` and `link` 0, which the fold reads as no snapshot; the next scan rescans every repo and keeps that outcome's classes (DR-73). On a temporary host the same step also commits the file to `refs/planroom/backup/<workspace_id>` in the repo `meta.backup_ref` names, with git plumbing and hooks off, and `scan` restores from that ref when the data root has no backup (DR-53).
- The ledger isn't in the store. It's `DATA_ROOT/ledger.db`, one per user, with its own migrations in `crates/store/ledger/` [DR-51].
- The data root's shared files, `ledger.db` and `workspaces.json`, serve every engine version on the machine at once, so they change only by additions: a new table, column or key, never a rename or a removal. Each records the oldest engine version that can read it, `min_reader_version`, which a migration raises only when old readers would misread it, and an engine older than that refuses with `schema_mismatch` naming the file. So an update in one workspace never stops older sessions in others. A change to `workspaces.json` reads, edits and renames the file under an exclusive lock on `workspaces.json.lock` (the standard library's `File::lock`), so two engines recording at once lose no entry. A newer `format`, or a `min_reader_version` above the engine's version, is refused with `schema_mismatch` whose `details.file` names the file, and the file is never rewritten (DR-85). Before each ledger migration the engine writes `VACUUM INTO DATA_ROOT/backups/ledger-vN.db`, and `doctor --restore` takes that copy too, so a downgrade keeps the daily guard's history (DR-78).
- A live `watch` re-reads `PRAGMA user_version` when it starts and whenever `PRAGMA data_version` changes, and an older one exits with `schema_mismatch` within one 200 ms poll of a migration plus its exit, under 400 ms (DR-78, DR-82). A client whose cursor is more than 5,000 events behind, or past the store's last `seq` (after a restore), gets one `reset` (DR-82).

## Graph model

```rust
// crates/contract/src/model.rs
pub struct NodeId(String);                   // "<namespace>:<kind-short>:<key>", such as "orders:pkg:internal/http"; a repo's
                                             // own node is "<ns>:repo:<ns>", such as "orders:repo:orders" (DR-81); a repo's
                                             // namespace is its directory name, or for the later of two same-named repos the name,
                                             // "-" and 4 hex of sha256(its path from the workspace root), fixed in meta.repos (DR-74)
pub struct EdgeId(String);                   // "e:" + 12 hex of sha256(kind ‖ from ‖ to ‖ via)
pub struct Kind(String);                     // "<pack>.<name>", such as "code.service"; the kernel's own: "kernel.group",
                                             // "kernel.external", "kernel.project", "kernel.item", and the relation "kernel.touches"
pub type Seq = u64;                          // below 2^53

pub struct Node {
    pub id: NodeId,
    pub kind: Kind,
    pub level: Level,
    pub parent: Option<NodeId>,              // exactly one parent; None only at the top
    pub name: String,
    pub aliases: Vec<String>,                // "host:orders", "service:orders", "pkg:@acme/kit", "endpoint:orders:POST /orders"
    pub props: BTreeMap<String, Value>,      // validated against the kind's prop schema
    pub layer: Layer,
    pub provenance: Provenance,
    pub evidence: Vec<Evidence>,
}

pub struct Edge {
    pub id: EdgeId,
    pub kind: Kind,                          // "code.http", "observability.watches"
    pub from: Ref, pub to: Ref,
    pub via: Option<String>,                 // "POST /orders", "order-jobs", "@acme/kit@1.1.0"
    pub via_ref: Option<Ref>,                // a node whose evidence also counts for this edge (an endpoint)
    pub props: BTreeMap<String, Value>,      // "uses", "latest", "rule" (require | forbid | allow-only)
    pub evidence: Vec<Evidence>,
    pub confidence: f32,                     // 1.0 for deterministic tiers
    pub layer: Layer,
    pub provenance: Provenance,
}

#[serde(tag = "type", rename_all = "snake_case")]
pub enum Evidence {
    Line {                                   // a file fact: verified against the blob's line index
        path: String,                        // "<repo>/<repo-relative path>"
        line: u32,                           // 1-based
        #[serde(skip_serializing_if = "Option::is_none")] #[schemars(extend("x-redact" = true))]
        contains: Option<String>,            // the exact line, cut to 200 chars; omitted in a default Artifact snapshot
        commit: Option<String>,
        branch: Option<String>,              // None from a head scan: evidence names the commit, and the branch is on the scan
                                             // event, since a branch can move or be deleted while the commit stays (DR-86)
    },
    Ref {                                    // a git-derived fact (tag, commit, co-change, ownership); also a directory,
                                             // a repo, and an empty or non-UTF-8 file, read at that commit (DR-81)
        commit: String,                      // verified with git cat-file -e
        #[serde(rename = "ref")] ref_name: Option<String>,   // "refs/tags/v1.5.0"; verified with git for-each-ref; None for a
                                             // directory, a repo or a file from a head scan, which names no branch (DR-86)
    },
}

pub struct Hint {
    pub id: String,                          // 12 hex of sha256(kind ‖ scope ‖ key ‖ evidence)
    pub kind: Kind,                          // "code.url", "code.env_read", "code.route_serve", ...
    pub scope: NodeId,                       // the file, or the compose service
    pub key: String,                         // the join key: a host, an env var name, a path, a queue key
    #[schemars(extend("x-redact" = true))] pub value: Option<String>,
    #[schemars(extend("x-redact" = true))] pub attrs: BTreeMap<String, Value>,   // scheme, port, path, method, op, driver, default
    pub evidence: Vec<Evidence>,
    pub provenance: Provenance,              // always Extracted
}

pub enum Ref { Id(NodeId), Alias(String) }
pub enum Level { Intent, System, Container, Component, Code }
pub enum Layer { Actual, Intended }
pub enum Tier { Universal, Syntactic, Precise /* SCIP or LSIF; no v0.1 extractor (DR-11) */, Config, Pattern, Inference, Linked }
pub enum Provenance {
    Extracted { extractor: String, tier: Tier },       // pack-qualified: "ruby/ruby-http"
    Linked    { linker: String, hints: Vec<HintRef> }, // linker pack-qualified: "code/runtime_wiring"
    Inferred  { model: String, decision: EventId, confidence: f32 },
    Authored  { actor: Actor, event: EventId },
}

pub struct FactBatch {                       // what every extractor and linker returns; sorted by ID
    pub format: String,                      // "planroom.facts/v1"
    pub extractor: String,
    pub tier: Tier,
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    pub hints: Vec<Hint>,
    pub partial: Vec<PartialFile>,           // files parsed with error nodes
}
pub struct PartialFile { pub path: String, pub errors: u32 }
pub struct HintRef { pub id: String, pub extractor: String }
```

## Event model

```rust
// crates/contract/src/event.rs
pub struct Event {
    pub seq: Seq,
    pub id: EventId,                         // uuid v7
    pub idem: String,
    pub request_hash: String,
    pub at: Timestamp,                       // RFC 3339 UTC
    pub actor: Actor,
    pub client: Client,                      // with actor, the writer instance that compare-and-set compares
    pub project: Option<NodeId>,
    pub cause: Option<Cause>,
    pub status: Status,
    pub body: EventBody,
}

#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Actor {
    Person { id: String, name: Option<String> },                     // "person:sridhar", "person:artifact:u_123"
    Agent  { session: String, role: String, key: Option<String> },   // "agent:1a2b3c4d:chat", "agent:<agentId>:implementer"
    Model  { id: String },                                           // "typesafe-ai/jev-1.14.0"
    System { part: String },                                         // "scan", "duty:decide"
}
pub enum Surface { Cli, Mod, Canvas, Artifact, Test }
pub struct Client { pub surface: Surface, pub id: String }       // "tab_01J...", "mod:<sessionId>", "artifact:u_123:<instance>", "cli:4242"
pub enum Cause { Event(EventId), Turn(String), Duty(String) }        // a chat turn batches ghosts into one card
pub enum Status { Applied, Ghost, Conflict, Open, Closed }       // stored; the fold derives Superseded and Accepted for a ghost;
                                                                 // a restore keeps each event's stored status (DR-73)

#[serde(tag = "kind", rename_all = "snake_case")]
pub enum EventBody {
    Edit(Edit),                                             // the only event that changes the graph
    Review { of: EventId, verdict: Verdict },               // Accept | Reject: ghosts, conflicts, inferred facts and cards; an accept
                                                            // of a ghost or conflict commits with a new Edit whose cause is `of`
    Undo { of: EventId, events: Vec<EventId>, base: Seq },  // `of` is what the person named; `events` is every event its command
                                                            // wrote, reversed together under compare-and-set from `base` (DR-61)
    Version { project: Option<NodeId>, label: String },     // a tag on seq
    Decision(DecisionRecord),
    Claim { target: Target, until: Timestamp, release: bool },
    Scan { repo: String, commit: String,
           branch: String,                                  // the branch HEAD named; the snapshot and its evidence name none (DR-86)
           snapshot: u64,                                   // 0 only in the outcome-holding event a restore writes (DR-73)
           link: u64,                                       // the workspace link snapshot of the active set this scan committed
           renames: Vec<Rename>,                            // { from: NodeId, to: NodeId }: git's renames since the last
                                                            // snapshot, files and wholly moved directories; the fold
                                                            // resolves a vanished edit target through them (DR-74)
           stats: ScanStats,                                // { files, nodes, edges, hints, partial, uncovered }; uncovered,
                                                            // left out when 0, counts the files the walk didn't cover, so a
                                                            // later focus still says the repo is partly mapped
           reconcile: Option<ReconcileOutcome> },          // on the last scan event of a scan's transaction
    Handoff(HandoffStep),
    Card(Card),                                             // a card a write dealt; open while no Review names it (DR-89)
}

pub struct Card {                            // crates/contract/src/card.rs; fills a DecisionCard; person_only is true for every
                                             // kind M1 deals (DR-89)
    #[serde(rename = "card_kind")]           // not `kind`, the event body's tag, as with DecisionRecord.decision_kind
    pub kind: CardKind,                      // an open set: apply | trust | pack_source | restore
    pub subject: String,                     // what the card is about; one open card per kind and subject
    pub question: String, pub why: String,   // en.toml keys
    pub vars: BTreeMap<String, String>,      // the values their {name}s take; omitted when empty
    pub stakes: Stakes,                      // Low | Person
    pub choices: Vec<Choice>,                // each label an en.toml key
    pub default: String,                     // the ID of the choice that holds while nobody picks
    pub dealt_by: CommandName,               // the leaf whose write dealt it: import or scan
}

pub struct Edit {
    pub op: Op,                                             // Put | Set | Retract
    pub target: Target,                                     // Node(NodeId) | Edge(EdgeId)
    pub layer: Layer,
    pub fields: BTreeMap<String, Value>,                    // Put: the whole node or edge; Set: changed fields
    pub base: Seq,                                          // the seq of the view the writer acted on
    pub over: BTreeMap<String, Value>,                      // fact values the fields override, for scan-drift checks
}

pub struct HandoffStep { pub key: String, pub from: HandoffState, pub to: HandoffState, pub note: Option<String>,
                         pub repos: Vec<RepoStep>,       // Merging, Merged, Reverting, Reverted: per repo
                         pub patch_ids: BTreeMap<String, String>,  // Reviewing, written by collect: per repo, git patch-id --stable over
                                                         // the branch's diff from its base commit, which a scan matches on the
                                                         // tracked branch to find a change landed by pull request (DR-80)
                         pub landed_by: Option<LandedBy> }  // Merged: Merge (handoff merge) | Ancestor | PatchId | Rules (DR-80)
pub struct RepoStep { pub repo: String, pub before: String /* HEAD before the step */, pub target: String /* planned commit */,
                      pub done: Option<String> /* the merge or revert commit */,
                      pub landed: Vec<String> /* commits that carried a change landed by pull request, which a revert reverts (DR-80) */ }
pub enum HandoffState { Proposed, Started, Reported, Failed, Reviewing, Merging, Merged, Reconciled,
                        Reverting, Reverted, Dropped }   // cancel and drop move planroom/KEY to refs/planroom/archive/KEY (DR-55)

pub struct ReconcileOutcome {                // persisted, so classes don't depend on when a scan ran
    pub heads: BTreeMap<String, RepoHead>,   // per repo: { commit, snapshot, tracked_branch }: the next scan's window start, and the
                                             // branch recorded at the first scan, whose fast-forward moves alone it reconciles (DR-71)
    pub link: u64,                           // the link snapshot the delta ran to; the next window's link start
    pub items: Vec<ReconcileItem>,           // class, subject, breaks, regressions, against
    pub now: Timestamp,                      // the clock the claims overlay used
}
```

The engine writes every `Timestamp` in one fixed form: RFC 3339 UTC with microseconds, `YYYY-MM-DDTHH:MM:SS.ffffffZ`. That covers an event's `at`, a claim's `until` and the claims overlay's `now`. So two timestamps compare as text in time order, and the claims overlay compares a claim's `until` with `now` that way.

## View models

```rust
// crates/contract/src/view.rs
pub struct PaneModel {                       // planroom focus; at most 4 KB
    pub focus: Focus,                        // breadcrumb, level, lens, project, seq
    pub decision: Option<DecisionCard>,      // the one open decision
    pub more_decisions: u32,
    pub progress: Progress,                  // stage, done/total, hand-offs running, scan state
    pub diffs: Vec<DiffLine>,                // top 3 one-liners, the first with up to 5 hunk lines
    pub drift: DriftLine,                    // the counts and the top item's handle, which takes a row key (DR-77)
    pub who: Vec<PresenceRow>,               // the top 3 with --detail summary; the first takes a row key (DR-77)
    pub next: Handle,                        // the next best action: the band's primary, and the pane's face-up card when no decision is
                                             // open; never NotUndoable, never class Handoff, never spends (DR-54, DR-66)
    pub minimap: Option<MiniMap>,            // up to 60 blocks at the focus level, edges by lens; omitted with --detail summary (the tools)
    pub empty: Option<Empty>,                // also the not-yet-a-workspace state outside a store (DR-57)
}
// catalogue.ts FALLBACK_PANES: one PaneModel per state before an engine answers (fetching, build_offered, building,
// failed, unsupported_platform, version_mismatch, store_busy, person_key_mismatch), generated from en.toml; each `next` is a
// resolver action or a retry of the mod's own start-up steps, except person_key_mismatch's pane Button, person init --rekey
// (DR-75). catalogue.ts also holds two lines from en.toml that no envelope carries: the failed host check's status line
// (DR-76) and the temporary host's copy-out line (DR-75)

// focus --picker lens|project|stage|handoff|remote: the card a picker key deals face up in the pane (DR-77)
pub struct Picker {
    pub name: String,                        // lens, project, stage, handoff or remote
    pub question: String,                    // one line, engine-authored; for a paid or repo-changing pick it names the cost or the
                                             // change, so the stage picker is a stage run's cost card: "Draft a stage with Claude?
                                             // About 4,500 tokens each, cached"
    pub why: String,
    pub entries: Vec<PickerEntry>,           // ranked: the entry the next action needs first, the selected row first when it's one;
                                             // the pane keys them 1 to 4, then 5 to 9, 0, b, j, k, q, r, v
    pub estimate: Option<PickEstimate>,      // when a pick spends: { tokens, usd, cached }, per pick
}
pub struct PickerEntry { pub label: String /* at most 20 columns */, pub detail: String, pub handle: Handle,
                         pub current: bool }

pub struct DecisionCard {
    pub id: String,
    pub question: String,                    // one line, engine-authored
    pub why: String,                         // one line
    pub choices: Vec<Choice>,                // 2 to 4; { id, label, preselected }; the pane sets them two to a row, and a fixed label
                                             // holds at most 20 columns (DR-54)
    pub stakes: Stakes,
    pub batch: Option<BatchInfo>,            // "Apply Claude's 2 edits": count and item handles; an accept takes only these items (DR-50)
    pub person_only: bool,
}

pub struct MapView {
    pub level: Level, pub lens: String, pub focus: Option<NodeId>,
    pub nodes: Vec<MapNode>,                 // id, kind, tone (the kind's, so surfaces style from the view model, DR-46), name,
                                             // level, parent, counts, position, ghost, claimed_by; under a lens with kinds, its
                                             // kinds' nodes come first, then by the lifted edges each carries (DR-15)
    pub edges: Vec<LiftedEdge>,
    pub hidden: u32,
    pub empty: Option<Empty>,
}
pub struct LiftedEdge { pub kind: Kind, pub from: NodeId, pub to: NodeId, pub count: u32,
                        pub evidence_sample: Vec<Evidence>, pub children: Vec<EdgeId> }

pub struct PathView { pub from: NodeId, pub to: NodeId, pub lens: String, pub hops: Vec<Hop>, pub summary: String }
pub struct Hop { pub from: NodeId, pub to: NodeId, pub kind: Kind, pub via: Option<String>,
                 pub relation: EdgeId, pub traversed: Direction, pub evidence: Vec<Evidence> }

pub struct Breakdown { pub target: NodeId, pub kind: Kind, pub parts: Vec<Part>, pub missing: Vec<MissingPart>,
                       pub relations: Vec<PartRelation>, pub touched: Vec<NodeId> }   // thread status is the pr question's filter step
pub struct MachineView { pub id: NodeId, pub states: Vec<State>, pub initial: String, pub terminal: Vec<String>,
                         pub transitions: Vec<Transition>, pub variables: BTreeMap<String, Variable>,
                         pub checks: Checks, pub step: Option<StepResult> }
pub struct Transition { pub from: String, pub event: String, pub to: String, pub guard: Option<String>,
                        pub action: Option<String>, pub evidence: Vec<Evidence> }
```

The `Diff`, `Impact`, `Mismatch`, `ReconcileItem` and `ReconcileOutcome` types are in [Architecture](architecture.md#one-diff-engine) and [the reconciler](architecture.md#the-reconciler-r7); they live in `crates/contract/src/diff.rs`.

## Pack manifest

```rust
// crates/contract/src/pack.rs: the deserialize view; TOML on disk. Every declaration struct here and in DecisionKind is
// #[serde(deny_unknown_fields)]. serde can't refuse one in a table an untagged enum reads (ExtractorDecl, EmitTemplate,
// SubjectKind) or beside Effect's flattened parameters, so the loader first walks each TOML file against the input
// schema: a key no table declares, at any depth, is pack_invalid at that key's own line
pub struct PackFile {
    pub format: String,                      // "planroom.pack/v1"
    pub id: String,                          // lowercase, [a-z0-9_]; this and every declared ID, a question's
                                             // included, at most 64 characters, so a reply shows each whole (DR-94)
    pub version: u32,
    pub title: String,
    pub requires: Vec<String>,               // a built-in pack: [] or ["code"]; an out-of-tree pack: any built-in packs (DR-62)
    #[serde(default)] pub languages: Vec<LanguageDecl>,  // merged across packs; read by the files extractor; a duplicate extension is pack_invalid
    #[serde(default)] pub image_roles: Vec<ImageRole>,   // merged across packs; read by the compose extractor; a duplicate glob is
                                                         // pack_invalid, and overlapping globs take the first match in load order
    #[serde(default)] pub kinds: Vec<KindDecl>,
    #[serde(default)] pub relations: Vec<RelationDecl>,
    #[serde(default)] pub hints: Vec<HintDecl>,        // a hint kind no loaded linker consumes is pack_invalid (DR-9)
    #[serde(default)] pub extractors: Vec<ExtractorDecl>,
    #[serde(default)] pub linkers: Vec<LinkerDecl>,
    #[serde(default)] pub lenses: Vec<LensDecl>,
    #[serde(default)] pub questions: Vec<QuestionDecl>,
    #[serde(default)] pub rules: Vec<RuleDecl>,
    #[serde(default)] pub decisions: Vec<DecisionKind>,
    #[serde(default)] pub suggestions: Vec<SuggestionDecl>, // action: never a NotUndoable or Handoff-class leaf (DR-54)
    #[serde(default)] pub prompts: Vec<PromptDecl>,
    #[serde(default)] pub stages: Vec<StageDecl>,      // plan pack only
}
pub struct LanguageDecl { pub id: String, pub extensions: Vec<String>, pub line_comment: Option<String>,
                          pub block_comment: Option<[String; 2]> }
pub struct ImageRole { pub image: String /* glob */, pub role: String, pub kind: String }
pub struct KindDecl { pub id: String, pub id_prefix: String /* the ID's kind-short; owned by one pack, except the kernel's
                                                                  "container", which every Container-level kind uses */,
                      pub level: Level, pub label: String, pub parents: Vec<String>,
                      pub props: Value /* JSON Schema fragment */, pub icon: Option<String>,
                      pub tone: Option<Tone> /* Service | Library | Store | Infra: a Container-level kind's block color;
                                                surfaces style from MapNode.tone, never from a kind ID (DR-46) */,
                      pub empty_text: Option<String>, pub empty_action: Option<String> }
pub struct RelationDecl { pub id: String, pub from: Vec<String>, pub to: Vec<String>, pub label: String,
                          pub props: Value /* JSON Schema fragment */, pub inferred: bool }
pub struct HintDecl { pub id: String, pub attrs: Vec<String> /* the attribute names it carries */ }
pub enum ExtractorDecl {                     // untagged: told apart by which of builtin, regex or query it sets
    Builtin { id: String, builtin: String, files: Vec<String>, options: Value },
    Regex   { id: String, files: Vec<String>, regex: String, emit: Vec<EmitTemplate> },
    Query   { id: String, files: Vec<String>, language: String /* a [[languages]] ID */, query: String /* path to .scm */,
              emit: Vec<EmitTemplate> },
}
pub enum EmitTemplate {                      // untagged, by node, edge or hint; values may use $file, $stem, $repo, $line, $1..$9, @capture
    Node { node: String, key: String, name: Option<String>, parent: Option<String> /* a ref, as `to` */,
           props: BTreeMap<String, Value>, aliases: Vec<String> },   // TOML values; captures coerced to the prop schema's type
    Edge { edge: String, from: String /* a ref, as `to` */, to: String /* @alias:..., an ID, @id:KIND:KEY, or @ext:NAME */,
           via: Option<String>, confidence: Option<f32>, props: BTreeMap<String, Value> },
    Hint { hint: String, scope: String /* a ref, as `to` */, key: String, value: Option<String>, attrs: BTreeMap<String, String> },
}                                            // a bare path such as "$file" in a ref is pack_invalid: write "@id:code.file:$file"
pub struct LinkerDecl { pub builtin: String, pub options: Value }
pub struct LensDecl { pub id: String, pub label: String, pub relations: Vec<String>,
                      pub flow: BTreeMap<String, Flow> /* Forward | Backward */,
                      pub kinds: Vec<String> /* the node kinds the lens is about: its map shows their nodes first, edges
                                                or none, and `map --lens` alone takes the lens's level (DR-15) */,
                      pub level: Level,
                      pub default: bool /* two defaults are pack_invalid unless one is the code pack's, which wins */,
                      /* id: never "all", which --lens all takes for every relation of every loaded pack */
                      pub empty_text: Option<String>, pub empty_action: Option<String> }
pub struct QuestionDecl { pub id: String, pub args: Vec<ArgDecl>, pub steps: Vec<ShapeStep> /* 1 to 3 */,
                          pub summary: String, pub budget_tokens: Option<u32> /* clamped to 3,000; None takes ask's 1,500 (DR-21) */ }
pub struct ArgDecl { pub name: String /* "X" in fires-if X */, pub optional: bool }
pub struct ShapeStep { pub shape: Shape, pub params: Value /* only the keys Shape::params declares; each names node kinds,
                        relations, a lens, node IDs, a field test or nothing, and every name is in the pack's scope (DR-62);
                        a filter's `kinds` are node kinds, so a relation there is pack_invalid; a node ID may be a question's
                        argument such as "$X", and a kind, relation or lens never holds a placeholder */ }
pub enum Shape { Node, Find, Neighbors, Path, Why, Filter, Compare, Order, Subgraph, Tree, Machine, Checks }
// Shape::params, RuleType::params, EffectOp::params and PREDICATE_PARAMS (a when predicate's keys) are each a table of
// Param { key, names: ParamNames }, where ParamNames is Nothing | Test (a value, or { lt | gt = N } or { in = [...] }) |
// NodeKind | SubjectKind (a when predicate's subject kinds: node kinds for a node kind's subjects, relations for a
// relation's) | Relation | Lens | NodeId | Table(&[Param]); a key ending in ".*", such as "props.*", stands for every key
// with that prefix. An undeclared key, at any depth, or a value form no check reads, such as { eq = ... }, is
// pack_invalid. architecture.md's shape, rule type, predicate and effect tables list them, and
// packs.rs::the_parameter_tables_in_the_docs_match_the_contract keeps both copies equal.
// kinds, stage, lint, missing and drift are built-in questions the engine serves, not shapes; no pack declares one (DR-15)
pub struct RuleDecl { pub id: String, pub kind: RuleType /* RequiresProp | RequiresEdge | MinChildren | Covers */,
                      pub subject: String, pub params: Value /* only the keys RuleType::params declares, as ShapeStep's */,
                      pub message: String }
pub struct StageDecl { pub id: String, pub method: String, pub items: Vec<ItemType>,
                      pub prompt: String /* the stage's instructions, in the uncached tail after the plan pack's shared prefix */,
                      pub lint: Vec<String> }
pub struct ItemType { pub id: String /* the kernel.item type: "next_action" */, pub props: Value /* JSON Schema fragment */ }
pub struct PromptDecl { pub id: String, pub file: String, pub tier: ModelTier /* Cheap | Capable */,
                        pub role: PromptRole /* Infer | Propose */, pub output: String /* a contract type name */,
                        pub ttl: CacheTtl /* M5 (default) | H1 for person-paced purposes */ }
pub struct SuggestionDecl { pub when: Predicate /* the filter shape's predicate, as a decision kind's when, through
                                                    PREDICATE_PARAMS; its kinds may be node or edge kinds */,
                            pub text: String /* the next action's label */, pub action: ActionDecl,
                            pub priority: u32 /* higher ranks first: the pane's next is the highest whose when holds on
                                                 some node or edge of the map, the first in load order on a tie */ }
pub struct ActionDecl { pub command: CommandName, pub args: Vec<String> /* parsed by the CLI's own clap tree, so a flag the
                        leaf doesn't take is pack_invalid; every lens and node ID they name is in the pack's scope */ }
```

The loader turns the loaded packs into one `Vocabulary` (`crates/contract/src/vocab.rs`), in load order, with every ID qualified as `<pack>.<name>`. It is what the kernel's resolution and fold read, so the kernel learns a kind only from it:

```rust
pub struct Vocabulary { pub kinds: Vec<KindVocab>, pub relations: Vec<RelationVocab>, pub lenses: Vec<LensVocab>,
                        pub suggestions: Vec<SuggestionDecl> }
pub struct KindVocab { pub kind: Kind, pub level: Level, pub id_prefix: String, pub label: String, pub props: Value,
                       pub tone: Option<Tone> }
pub struct RelationVocab { pub kind: Kind, pub label: String, pub from: Vec<Kind>, pub to: Vec<Kind>, pub props: Value,
                           pub inferred: bool }
pub struct LensVocab { pub id: String, pub label: String, pub relations: Vec<Kind>, pub flow: BTreeMap<Kind, Flow>,
                       pub kinds: Vec<Kind>, pub level: Level, pub default: bool }
```

### Pack test layout

`planroom doctor --pack DIR` reads `tests/inputs/<repo>/...` (files to scan), `tests/expected.jsonl` (sorted `FactBatch` lines; `--update` rewrites it), and optional `tests/slow/` (a fixture past the scan bounds).

- **Which ring.** A directory is checked as an out-of-tree pack beside the built-ins, since it was found on a path ([Requires, inward only](architecture.md#packs-and-extractors)). A directory whose files are a built-in pack's own, file for file but its `tests/`, checks that built-in, so `doctor --pack packs/code` checks the code pack this build embeds. A directory that claims a built-in pack's ID with any other files is `pack_invalid`: to check a changed built-in, build Planroom from that source first.
- **The inputs.** Each directory under `tests/inputs/` is one repo, named by the directory. The inputs aren't a git repo, so their evidence names the commit `0000000000000000000000000000000000000000`. `expected.jsonl` holds one `FactBatch` per line, repos in name order, each repo's extractors in load order.
- **What it returns.** `PackReport { summary, id, version, origin, sha256, kinds, relations, extractors, linkers, lenses, checks }`, where `origin` is `builtin` or `out_of_tree` and each `PackCheck { check, line }` names a check the pack passed: `validate`, `expected`, `determinism`, `evidence` and `evals`. The first check that fails fails the command with `pack_invalid`, naming the file, the line and the reference.

The store's `meta.packs` records the loaded set in load order, one `PackRecord { id, version, source, sha256 }` per pack, where `source` is `builtin` or the directory the pack loaded from and `sha256` hashes the pack's files, each path and its bytes in path order (`tests/` left out). `meta.packs_hash` hashes those records under the engine's version.

A built-in extractor's `options.kinds` names the kinds it emits, such as `kinds = { repo = "code.repo", directory = "code.directory", file = "code.file" }` for `files`. The engine replaces each name with `{ kind, id_prefix, level }` from the vocabulary, so no built-in spells a kind, a level or an ID prefix of its own. It also fills in `files`'s merged `languages`, and `compose`'s merged `image_roles` and the workspace's sorted `repos`.

## Extractor and linker contract

Built-in extractors and linkers implement Rust traits in `planroom-extract`. Pack authors use the declarative form above; both produce the same `FactBatch`.

```rust
// crates/extract/src/lib.rs
pub trait Extractor {
    fn name(&self) -> &'static str;
    fn run(&self, repo: &RepoView, files: &[RepoPath], opts: &Value) -> Result<FactBatch, ExtractError>;
}
pub trait Linker {                           // pure: the workspace's hints and nodes in, linked facts out
    fn name(&self) -> &'static str;
    fn link(&self, ws: &WorkspaceFacts, opts: &Value) -> Result<FactBatch, ExtractError>;
}
pub struct RepoView {                        // no branch: it's no input of the facts (DR-86)
    pub name: String,
    pub root: PathBuf,                       // where git runs; an extractor never reads a file from here (DR-85)
    pub commit: String,
    pub blobs: Blobs,                        // the commit's files the walk took; read them with RepoView::read
}
```

Rules checked by `extract/tests/contract.rs`: deterministic and sorted output; tolerant of error nodes; only the commit's tree read, never the working tree (DR-85); files over 1 MiB, binaries and paths a committed `.gitignore` ignores skipped; at most 50,000 files and 256 MiB per repo, in sorted path order, from one walk every extractor reads, with `partial_scan` on a cut; unchanged blobs reuse cached facts; every fact has evidence, a `Line` whose `contains` verifies against the blob or a `Ref` that verifies against git; nodes, edges and joinable hints come from syntax nodes, never comments or docstrings, and only `metric_mentions` reads comments, for evidence; emit props coerce to the prop schema; cross-file targets go out as aliases or hints; nothing from the repo runs.

## Decision kind, policy and record

```rust
// crates/contract/src/decision.rs
pub struct DecisionKind {                    // [[decisions]] in a pack
    pub id: String, pub version: u32,
    pub subject: SubjectKind,                // Kind("code.file"): a node or edge kind | Role(Role): subjects the role's raiser supplies
    pub when: Predicate,                     // the deterministic precheck, the filter shape's predicate, evaluated by the kernel over
                                             // the folded model: { field = value }, { field = { lt | gt | in = ... } }, and
                                             // with_edge or without_edge = { relation, direction, layer? }; a prop only an extractor
                                             // can compute is precomputed: placed_by_rule, touched_after_anchor (DR-63); only
                                             // PREDICATE_PARAMS's keys: kind, kinds, id, parent, status, confidence, props.*,
                                             // with_edge, without_edge
    pub group: Option<String>,               // kinds with the same group and subject share one Jev request
    pub question: Question,
    pub slice: SliceSpec,                    // { query, relations, fields, max_tokens }; evidence goes in as path and contains, with an
                                             // occurrence index for a repeated line, never line (DR-65)
    pub effect: Effect,                      // { on, do: None | Lint | SetProp | PutEdge | RetractEdge | Retarget, ... }: closed, six
                                             // the rest are only the keys EffectOp::params declares (SetProp: prop and value, or
                                             // parent; PutEdge, RetractEdge: relation, from, to; Lint: rule; None, Retarget: none);
                                             // SetProp sets a prop or `parent`; a node ID of "$pick" is the chosen candidate;
                                             // Retarget only on a Role(RescanRetarget) subject in a Person-stakes kind (DR-63);
                                             // no effect sets a field a person holds (DR-73)
    pub stakes: Stakes,                      // Low: prefill and list in one batch per kind | Person: deal a card
    pub default: Value,                      // the answer that leaves the deterministic result as it is
    pub policy: Policy,
    pub eval: Option<String>,                // "evals/<kind>"
    pub budget: Budget,                      // { max_calls_per_scan }, counted per scan event, which its one decide duty run spends,
                                             // and per explicit decide --run (DR-65)
    pub role: Option<Role>,                  // RescanRetarget | ReconcileResidual | AttachTarget
}
pub enum Question {
    Boolean { instructions: String, criteria: Option<BoolCriteria> },
    Choice  { instructions: String,
              options: IndexMap<String, String>,           // static options; empty when options_from is set
              options_from: Option<OptionsFrom>,           // { shape, params, max }: candidate nodes from a query shape over the subject
              auto_choices: Vec<String> },                 // static options only; with options_from any candidate may auto-act
    Score   { instructions: String, levels: Vec<String> },             // 2 to 10, low to high
}
pub struct Policy { pub auto_at: Option<f64> /* None: never auto, never asks Claude up front */,
                    pub card_band: [f64; 2], pub escalate_band: [f64; 2], pub escalate_to: ModelTier }

pub struct DecisionRecord {                  // the body of a decision event
    pub decision_kind: String,               // not `kind`: `kind` is the event body's serde tag
    pub kind_version: u32, pub subject: Subject, pub input_hash: String,   // Subject: Node | Edge | Role { role, key };
                                                                          // the input hash covers the candidates options_from produced
    pub backend: String, pub model_requested: String, pub model_reported: Option<String>,
    pub scores: Value,                       // the normalized Jev or Claude result
    pub pick: Option<String>, pub band: Band, // Auto | Card | Escalate
    pub outcome: Outcome,                    // Acted | Prefilled | Card | Escalated | Picked | Overridden | Superseded: a person
                                             // held a field the effect would set, so nothing applied and no card (DR-73)
    pub cost_usd: f64, pub cache_hit: bool, pub request_id: Option<String>,
}
```

## Evals

```rust
// crates/contract/src/eval.rs
pub struct EvalCase {                        // one line of packs/<pack>/evals/<kind>.cases.jsonl
    pub format: String,                      // "planroom.eval/v1"
    pub case_id: String,
    pub state: Value,                        // the slice as Jev will see it
    pub options: Option<Vec<String>>,        // the candidates options_from produced for this case; a label names one of them
    pub label: Value,                        // true | false | an option | a candidate | a level
    pub rationale: String,
    pub labeller: String,                    // the labeller's agent ID; never the tuner
    pub tags: Vec<String>,                   // "easy", "ambiguous", "canary"
}                                            // the split is derived: tune when sha256(case_id) mod 5 < 2
pub struct EvalResult {                      // packs/<pack>/evals/<kind>.result.json, GENERATED
    pub format: String, pub kind: String, pub kind_version: u32, pub model: String, pub date: String,
    pub n_tune: u32, pub n_holdout: u32, pub auto_at: f64,
    pub precision_holdout: f64, pub coverage_holdout: f64, pub wilson_lo: f64, pub acted_holdout: u32,
    pub runs_agree: bool, pub cases_digest: String, pub auto_enabled: bool,
}
```

## Model request, response and replay

```rust
// crates/contract/src/models.rs
pub struct ModelRequest {                    // .planroom/models/requests/<id>.json, session backend; prompt text passes kernel::redact
    pub format: String,                      // "planroom.model-request/v1"
    pub id: String,                          // the input hash; the cache key
    pub purpose: String,                     // "plan.stage.outcome", "artifacts.segment", "decide.escalate"
    pub tier: ModelTier,                         // the mod maps Cheap to "haiku", Capable to "sonnet"
    pub prompt: Vec<PromptBlock>,            // { text, cache: bool }: stable prefix first, the last prefix block cached
    pub output_schema: Value,                // the reply must validate against it
    pub max_tokens: u32,
    pub effort: Option<String>,
}
pub struct ModelResponse {                   // written by the mod, read by `planroom import`, only for a request_id the engine derives as pending
    pub format: String,                      // "planroom.model-response/v1"
    pub request_id: String,
    pub request_sha256: String,              // the sha256 of the request file the mod sent; import refuses one that isn't the engine's (DR-75)
    pub model: String,
    pub text: String,
    pub usage: SessionUsage,                 // input_tokens, output_tokens, cache_read_input_tokens, cache_creation_input_tokens
}
pub struct ReplayRecord {                    // tests/replay/{jev,claude}/<purpose>/<key>.json
    pub format: String,                      // "planroom.replay/v1"
    pub key: String,                         // sha256(path ‖ canonical body): the body as canonical JSON (keys sorted, no
                                             // whitespace) less the gateway's routing options (providerOptions); never a
                                             // header, a credential or a transport field
    pub purpose: String,                     // "plan.stage.outcome": the folder the record lives in
    pub request: RecordedRequest,            // method, url, headers without auth, body
    pub response: RecordedResponse,          // status, headers, body or sse
    pub measured: Measured,                  // wall_ms, recorded_at, model_reported
}
```

## Change spec

`planroom.change-spec/v1`, written by `handoff start` to `DATA_ROOT/handoffs/<workspace_id>/KEY/spec.json`, beside the hand-off's clones. It's self-contained: an implementer needs nothing else.

```rust
// crates/contract/src/handoff.rs
pub struct ChangeSpec {
    pub format: String,                      // "planroom.change-spec/v1"
    pub key: String,                         // "worker-status-via-api"
    pub title: String,
    pub created_at: Timestamp,
    pub by: Actor,                           // the person who started it
    pub intent: SpecIntent,                  // { summary, plan_items, why }
    pub rules: Vec<SpecRule>,                // { kind: Present | Absent, from, to, edge_kind, via }
    pub repos: Vec<SpecRepo>,                // { name, clone (a local clone under DATA_ROOT/handoffs/<workspace_id>/KEY/, DR-64), branch: "planroom/KEY",
                                             //   base_commit, base_branch }
                                             // the merge preflight checks HEAD is on base_branch
    pub touch_points: Vec<TouchPoint>,       // { component, path, line, why }: pointers, never bodies
    pub context: Vec<ContextExcerpt>,        // { path, lines, excerpt (marked x-redact), untrusted: true }; 4,000 tokens in all
    pub checks: Vec<SpecCheck>,              // { repo, command, env } and the reconcile rules
    pub installs: Vec<SpecCommand>,          // { repo, run, why }: the installs each repo's lockfiles call for, which the mod runs in each
                                             // clone at its base commit before the implementer starts, outside its sandbox (DR-75)
    pub commands: Vec<SpecCommand>,          // { repo, run, why }: what the person approves the implementer to run with no prompt, inside
                                             // its sandbox: the checks, git add, git commit; --allow adds one, an install to installs (DR-64, DR-75)
    pub boundaries: Vec<String>,             // "Edit only inside these clones", "Don't push", "Commit on the branch"
    pub report: ReportSpec,                  // { path: "result.json", format: "planroom.implementer-run/v1" }
    pub patches: Vec<PatchRef>,              // patch implementer only: { repo, path }
}
```

For S4 the rules are `Present worker -> orders http POST /orders/{id}/events` and `Absent worker -> orders-db db`, the repos are `worker` and `platform`, and the CI path's patches are `examples/fixture/implementer/worker-status-via-api/{worker,platform}.patch` [TRUTH scenarios.S4.implementer_patches].

## Implementer run

`planroom.implementer-run/v1`, one JSON object per line, exactly as `examples/README.md` defines it and the fixture uses it. Planroom writes the same format for every `handoff report`, so seeded and real history feed curation the same way.

```rust
pub struct ImplementerRun {
    pub format: String,                      // "planroom.implementer-run/v1"
    pub run_id: String,                      // "run-0001"
    pub change_spec: String,                 // "spec-..." or a hand-off key
    pub repo: String,
    pub started_at: Timestamp,
    pub finished_at: Timestamp,
    pub outcome: RunOutcome,                 // Merged | Rejected | Abandoned
    pub steps: Vec<RunStep>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub repos: Vec<RunRepo>,                 // additive, optional: { repo, branch, head } written by Planroom's implementers
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub denied: Vec<String>,                 // additive, optional: Bash commands the guard denied, which the review card lists (DR-64)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub claimed: Vec<RunStep>,               // additive, optional: for the agent implementer, the steps and notes it wrote in result.json,
                                             // kept for the review card and never mined; its `steps` are the Bash calls the mod observed
                                             // (observed.json) and the branch's commits (DR-75)
    #[serde(flatten)] pub extra: BTreeMap<String, Value>,
}
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RunStep {
    Command { command: String, exit_code: i32, #[schemars(extend("x-redact" = true))] output_excerpt: String },
    Edit    { path: String, summary: String },
    Note    { text: String },
}
```

The required fields are the README's; `fixture_format_is_unchanged` checks that the schema's required list equals them, so an additive field never becomes required.

## Review threads

`planroom.review-threads/v1`, one file per PR, exactly as `examples/README.md` defines it. Threads anchor to a commit by subject, not SHA.

```rust
// crates/contract/src/review.rs
pub struct ReviewThreads {
    pub format: String,                      // "planroom.review-threads/v1"
    pub repo: String,
    pub pr: PullRequest,                     // { number, title, author, base, head, state, body }
    pub threads: Vec<Thread>,
    #[serde(flatten)] pub extra: BTreeMap<String, Value>,
}
pub struct Thread {
    pub id: String,                          // "T1"
    pub path: String,                        // repo-relative
    pub line: u32,                           // a line in the anchored commit's version of path
    pub commit_subject: String,
    pub resolved: bool,
    pub comments: Vec<Comment>,              // { author, at, body }
}
```

## Mod tool schemas, state and the room family

The three tools come from the catalogue. Typed unions of each leaf's request, with their `$defs`, would cost about 1,300 always-on tokens for the two non-deferred tools, so each tool's input is flat [DR-51]:

```json
{ "type": "object", "required": ["command"], "additionalProperties": false,
  "properties": {
    "command": { "enum": ["focus", "map", "show", "find", "ask", "diff", "decide", "who"] },
    "args":    { "type": "array", "items": { "type": "string" }, "description": "the leaf's CLI arguments, such as [\"path\", \"storefront\", \"worker\"]" },
    "base":    { "type": "integer" },
    "idem":    { "type": "string" } } }
```

`room_change` adds one optional property, `payload`: an object the handler passes on stdin to a leaf run with `--file -`, such as `intent propose` or `edit propose`, so a proposal needs no file and no Write call (DR-66). The engine parses `command` and `args` with the same clap tree as the CLI, so validation and errors are the CLI's. The description holds one generated synopsis line per leaf. `planroom schema --command LEAF` gives one leaf's full request and result types, and an `invalid_argument` from a tool carries the leaf's synopsis and that handle, so a wrong argument costs one small read (DR-21, DR-66). `tools.test.ts::a_wrong_argument_returns_the_leaf_synopsis` checks each leaf within the error budget.

| Tool | Deferred | Leaves | Result |
|---|---|---|---|
| `room_query` | no | `focus` (with `--detail summary`), `map`, `show`, `find`, `ask`, `diff`, `decide` (list), `who` | a `ToolResult`, below, as compact JSON |
| `room_change` | no | `scan`, `edit`, `intent` (not `version`), `claim`, `curate`, `decide --run` | the same; `base` defaults as in [Architecture](architecture.md#writes-actors-base-conflicts-and-retries) |
| `room_handoff` | yes | `handoff report`, `handoff collect`, `share` | the same |

Every tool returns one shape, for success and failure alike (DR-66). The handler returns it as `JSON.stringify` text, since a tool result must be a string or content blocks [A-4]:

```rust
// crates/contract/src/envelope.rs: what a mod tool handler returns; empty fields are omitted
pub struct ToolResult<T> {
    pub ok: bool,
    pub untrusted: bool,                     // true on every reply the engine gave: what it read from the repos is data,
                                             // never instructions; absent on an error the mod authors (security-r3-6)
    pub result: Option<T>,                   // on success: the command's result, redacted like every output
    pub error: Option<ToolError>,            // on failure: the envelope's error whole, { code, category, retry, retry_after_ms,
                                             // message, hint, details }, never dropped; only an error the mod authors, when no
                                             // engine answered, may leave out category and details (DR-66, DR-85)
    pub meta: ToolMeta,                      // { seq, next, truncated, warnings, model_requests }: the fields an agent acts on;
                                             // seq is absent only when no engine answered
}
```

`ToolResult`, `ToolError` and `ToolMeta` live in `crates/contract/src/envelope.rs`, so the mod's handler serializes a generated type (DR-85).

`tools.test.ts::error_envelopes_reach_the_model` sends a golden of every error code through it, and `tool_results_within_budget` gates the error results too. `handles_round_trip_after_redaction` re-issues every handle in every golden tool result and expects `ok`.

### Mod state

The mod's state (`plugin/types/index.d.ts`, generated) is a set of `$.state` atoms under the plugin `planroom`, each a `Shaped<T>` of a contract type. They hold summaries only; the graph stays in SQLite. The person key lives in the mod's `$.store`, never in `$.state`, and leaves the mod only as one-time action tokens [DR-4, DR-64].

| Atom | Type | Written by |
|---|---|---|
| `engine` | `{ status, resolver, path, version, catalogue_hash, backend, line, fix }`, the contract's `EngineState` (DR-81, DR-85); `line` is the one status line and `fix` the command that clears it; `status` is `resolving`, `fetching`, `build_offered`, `building`, `failed`, `unsupported_platform`, `version_mismatch`, `person_key_mismatch` or `ready`, and picks the fallback pane (DR-57, DR-60) | `session.start`, the resolver's `status --json` |
| `pane` | `{ model, error }`, the contract's `PaneState`: the last `focus --json`'s `PaneModel`, or the error it returned (DR-85) | the watch loop's scheduled `focus` (first frame, after 250 ms of quiet, and at least every 500 ms under load) |
| `lastSeq` | `{ seq: Seq \| null }`, the contract's `LastSeq`, null before the first frame (DR-82, DR-85) | the watch loop |
| `outbox` | `{ idem, argv, env_names }[]` (never a token or the key) | each write, cleared on reply |
| `agents` | `{ agentId, role, key, clones, commands, denied, started_at }[]`: the hand-off's clones and approved commands, which the guard reads, and the commands it denied (DR-64) | spawns, `turn.complete`, the guard |
| `remote` | `{ url, last_published_seq, last_sync_at }` | `/room share`, `/room sync` |
| `turnSeq` | `{ turnId, seq }`, the contract's `TurnSeq` (DR-85) | `room_query` replies, for the default `base` |

### The room family

`ROOM_COMMANDS` in `catalogue.ts` lists every leaf with `room: true`, plus `/room` alone (focus the pane), `/room share` and `/room sync`. `/room help` prints it, and `room_commands_snapshot` pins it. `room` is registered with `immediate: true`, so it runs mid-turn (DR-77). Bare `/room` returns no `text` and starts no sync. Every other `/room` result is drawn in full through the `CommandOutput` site. Its `text`, which the model reads on every later turn, is only the envelope's `summary` and the reply's handles, never JSON, at most 300 tokens (DR-85). `room.test.ts::room_text_within_budget` gates it as `budgets.json` `room_text` (DR-65).

## Artifact db layout and event format

The page's manifest declares these capabilities [artifact-spike]:

```js
capabilities: {
  db: { rules: [
    { path: "", read: "view", write: "owner" },                 // snapshot heads and acks: only the session writes
    { path: "inbox", read: "owner", write: "owner" },           // no viewer reads another's pending rows
    { path: "inbox/{self}", read: "interact", write: "interact" }, // each viewer writes only their own inbox
  ] },
  user: { scopes: ["profile"] },
  sample: {},
  comments: {},                                                 // Send to Claude: comments.sendToClaude, for viewers with edit access
}
```

Until Ask Claude and Send to Claude land, `share`'s publish arguments declare only `db` and `user`, and add `sample` and `comments` with them (DR-83). `share` writes the canvas build with relative paths and marks its root `<div id="root" data-transport="artifact">`, which `main.tsx` reads to pick the Artifact transport. It records the snapshot's `seq` in `meta.share_snapshot_seq`, which import writes as `live/head.snapshot_seq` (DR-83).

| Path | Writer | Format | Size rule |
|---|---|---|---|
| `data/snapshot.json` (a published file, not `db`) | the session's publish | `planroom.snapshot/v1` | under 1 MB for the fixture; at most 16 MB |
| `live/head` | the session (`ArtifactData` with `if_version`), written last in each batch | `planroom.live-head/v1` | at most 256 KiB |
| `inbox/<uid>` | the viewer | `{ seq, updated_at, sample: [{ at, prompt_chars, response_chars, model }] }`; `sample` logs each Ask Claude call and the model the platform applied, which import prices into the ledger and, when `PLANROOM_SPEND_LOG` is set, appends to the spend log (DR-65) | |
| `inbox/<uid>/events/<docId>` | the viewer (`collection.doc()` mints `docId`) | `planroom.inbox/v1` | deleted after import |
| `answers/<docId>` | the session, in the sync's batch | `planroom.answer/v1`: `{ format, handle, seq, result, error?, next?, warnings? }`, the result of a read request row, redacted, with `result` null and `error` set when the read failed (DR-83); `next` is the page's `meta.next`, which a read request passes back as its `cursor` for the page after; `warnings` are the reply's `meta.warnings`, such as `stale_page` for a cursor from a map that has moved on, and absent when there are none; every viewer reads it, and a republish clears the folder (DR-77) | the leaf's budget |
| `acks/<uid>` | the session | `planroom.ack/v1`: `{ format, imported_through, rows: { <docId>: imported \| held \| conflict \| refused } }`; `held` is a row waiting for the desk, as a ghost or a pre-selected pick (DR-75); an existing ack is written as an `update`, so its rows merge (DR-83); the page marks its own rows queued until they appear here | one per viewer |

```rust
// crates/contract/src/remote.rs
pub struct ShareResult {                     // what `share` returns
    pub summary: String,                     // what will be shared, from the snapshot's sections; the first publish shows it
    pub publish: PublishArgs,
    pub db: Vec<DbBatch>,                    // the writes to make after the publish, live/head last
    pub files: Vec<SharedFile>,              // { path, bytes }: only what share built from this workspace, that is the page, the
                                             // snapshot and the first live/head; the canvas build's assets are only in publish (DR-86)
    pub holds: Vec<String>,                  // as Snapshot.capabilities names it
}
pub struct PublishArgs {                     // the exact arguments of the session's Artifact publish
    pub file_path: String,                   // the page fragment, an absolute path
    pub root: String,                        // share's --out, absolute; every files source is relative to it, so the
                                             // arguments keep to share's budget (DR-86)
    pub files: BTreeMap<String, String>,     // each published path and its source, relative to root
    pub capabilities: Value, pub title: String, pub description: String,
    pub icon: String,                        // on the first publish
}
pub struct Snapshot {
    pub format: String,                      // "planroom.snapshot/v1"
    pub workspace_id: String, pub seq: Seq, pub published_at: Timestamp,
    pub snippets: bool,                      // false by default: evidence has path and line, no contains
    pub maps: Vec<MapView>,                  // every level for every lens (DR-77)
    pub layouts: Vec<LayoutEntry>,
    pub cards: Vec<DecisionCard>,
    pub diffs: Vec<DiffSummary>,
    pub plans: Vec<PlanSummary>,
    pub machines: Vec<MachineView>,
    pub who: Option<Vec<PresenceRow>>,       // None unless share --include-presence
    pub pane: PaneModel,
    pub answers: Vec<Answer>,                // precomputed views keyed by the handle that asks for each, in fill order: every no-argument
                                             // question; path between each pair of top-level blocks a lens connects; release-order and
                                             // impact for each block another repo depends on; every Breakdown outline; the show page of
                                             // each level's top blocks; within the 1 MB budget (DR-77)
    pub capabilities: Vec<String>,           // the views it serves, as view keys: "map:<level>:<lens>" for each map it holds, "pane",
                                             // "answers" (DR-83), such as "map:system:all"
    pub unavailable: Empty,                  // shown for any other view: "Not in this snapshot. Ask your computer for it"; its next writes
                                             // a read request row, and "Back to the map" stays; never /room open (DR-56, DR-77)
}
pub struct Answer { pub format: String /* "planroom.answer/v1" */, pub handle: Handle, pub seq: Seq,
                    pub result: Value,               // null when the read failed
                    pub error: Option<ErrorObject>,   // also under answers/<docId> (DR-83)
                    pub next: Option<Cursor>,         // the page's meta.next; absent on the last page
                    pub warnings: Vec<Warning> }      // the reply's meta.warnings, such as stale_page; absent when none
pub struct LiveHead { pub format: String /* "planroom.live-head/v1" */, pub seq: Seq, pub snapshot_seq: Seq,
                      pub cards: Vec<DecisionCard>, pub changes: Vec<StreamFrame>,
                      pub delta: Diff,       // nodes and edges of the snapshot's maps since snapshot_seq; the page applies it
                      pub conflicts: Vec<String> }
pub struct InboxRow {                        // input view; validated at import
    pub format: String,                      // "planroom.inbox/v1"
    pub instance: String,                    // minted by the page on each load: with the uid, the writer instance (DR-61)
    pub counter: u64,                        // per instance, monotonic; import applies rows in (instance, counter) order
    pub command: CommandName,                // "edit", "review", "decide" or "intent propose"; or a read request: "map", "show", "find",
                                             // "diff", or an "ask" question that doesn't spend (DR-77); any other is refused
    pub request: Value,                      // validated against that command's request schema
    pub base: Seq,                           // the snapshot or live-head seq the viewer saw
    pub at: Timestamp,
    pub source: Option<String>,              // "sample" when the edit came from Ask Claude
    pub sig: Option<String>,                 // a paired device's HMAC-SHA256(pairing key, docId ‖ canonical(row without sig)) (DR-67)
}
```

Import rules [DR-52, DR-67]: importing inbox rows is person-only, except rows a paired device signed and read requests (DR-77), and in no tool.

- **Stdin.** `import -` takes one `ImportLine` per line, `{path, version, row}`, from the mod, which read the rows from the text block of the `ArtifactData list` result it observed: one JSON object per line between `BEGIN ARTIFACT DB` and `END ARTIFACT DB`, each checked against `InboxRow` (DR-83). `version` is the document's version in that result. A line with no `row` gives the version of a document the session wrote before, such as `live/head` or `acks/<uid>`. Import never reads rows from a folder.
- **Result.** `ImportResult { summary, person, counts, rows, root, batch_files }` keeps to import's budget however many rows a sync brings (DR-87). Its fields:

  | Field | What it holds |
  |---|---|
  | `counts` | `ImportCounts { imported, held, conflict, refused, left, skipped }`: how many stdin lines came to each outcome. `left` rows stay in the inbox, unacked, for a person's sync; `skipped` lines name no inbox row. |
  | `rows` | the first `IMPORT_ROWS_SAMPLE` (5) lines' outcomes, in the order they applied. `acks/<uid>` still lists every acked row. |
  | `root` | the sync's absolute directory, `.planroom/share/sync/<seq>-<µs>/`, which holds the documents and the batch files. |
  | `batch_files` | one `batch-N.json` per `ArtifactData batch` call, in order, relative to `root`. Each holds a `DbBatch` of at most 50 writes and 1 MiB, whose `file_path`s are absolute: answers, then acks, then each acked row's delete, with `live/head` the last write of the last batch. |

  The session reads each batch file and passes its `writes` as they are to one `ArtifactData batch` call. Every write to an existing document carries `if_version` from its stdin line, so a sync never overwrites a viewer's newer write (DR-83).
- **Commands.** A row may name only `edit`, `review`, `decide` on a card in the published snapshot, `intent propose`, or a read request; any other is refused and listed as `refused` in `acks/<uid>`. A read request (`map`, `show`, `find`, `diff`, or an `ask` question that doesn't spend) runs as a read, needs no person, and its redacted result goes to `answers/<docId>` in the same batch, so every sync answers it, a tokenless one included (DR-77).
- **Attribution.** The actor is `person:artifact:<uid>` from the path, named for display, never as authority: the owner's session can write any viewer's inbox (DR-83). Import ignores a row's unknown fields, so a forged `actor` is ignored. The client is `artifact:<uid>:<instance>`, so two devices or tabs of one viewer conflict like two canvas tabs; rows apply sorted by `(instance, counter)`; `idem` is `artifact:<uid>:<docId>`; a stale `base` is a person conflict, reported back through `live/head`.
- **Signed rows.** A row whose `sig` checks against `meta.pairing_key` applies as `person:artifact:<uid>` when it is a reversible edit (`edit`, `review` or `intent propose`). A page the model republishes could read the pairing key, so a signed row never runs a person-only command (DR-75).
- **Unsigned rows.** Every row no paired device signed, whoever's inbox holds it, applies nothing on its own (DR-83). Each is stored as a ghost under `person:artifact:<uid>`, with the import as its cause, and acked `held`. They batch into one card the local person confirms, with Apply pre-selected. An applied row becomes an edit as `person:artifact:<uid>` with the card as its cause; a rejected one is an ordinary review reject, which `edit undo` reverses.
- **Picks.** A `decide` row from any viewer, signed or not, applies nothing at import: its choice becomes the pre-selected pick of that card at the desk, marked with its viewer, acked `held`, and one key confirms it (DR-75).
- **No token.** An import with no person token, the sync a Send to Claude wake or a cloud session's turn start begins, applies only signed rows' reversible edits, answers read requests, and leaves the rest in the inbox, unacked, for the person's next sync (DR-77).

## Configuration

Two optional files share one schema, and every key has a default in `crates/planroom/src/config/defaults.toml`, from which `cargo xtask gen` writes [the configuration reference](../reference/configuration.md). A `PLANROOM_<SECTION>_<KEY>` environment variable overrides a key, such as `PLANROOM_SCAN_EXCLUDE`, except `packs`, which is workspace state (DR-62), `[claude] api_key`, whose variable is the one `api_key_env` names, and `[claude] base_url`, which only the user-level file sets, since it decides where a credential goes. A workspace's file wins over the user-level one. A new key needs a decision record saying why no default works [DR-22].

- The workspace's `config.toml`, in its state directory, loads only when the workspace is trusted [DR-48]: `.planroom/config.toml` beside a directory of repos, or `planroom/config.toml` in the git directory of a workspace that is one repo, `.git/planroom/config.toml` in an ordinary clone [DR-74].
- The keys marked "user" below may appear only in the user-level `${XDG_CONFIG_HOME:-$HOME/.config}/planroom/config.toml`: the credential settings, and the limits that span every workspace. The engine ignores such a key in a workspace's file, whatever its value, and `api_key` in a user-level file that isn't exactly mode 0600, with a `config_key_ignored` warning naming the file and key.

| Key | Where | Default | Meaning |
|---|---|---|---|
| `packs` | workspace | `[]` | extra pack directories, after the built-ins; written by the person-only `planroom pack add`, which copies the pack into the state directory's `packs/` for every process to load from there, and by `pack remove`; recorded in the store with the resolved set, and never overridden from the environment (DR-62): `PLANROOM_PACKS` only names directories for `doctor --pack` (DR-62) |
| `[scan] exclude` | either | `["**/fixture/**", "**/fixtures/**", "**/__fixtures__/**", "**/testdata/**", "**/test-data/**"]` | directories a scan skips, as globs relative to each repo's root; a value replaces the default list; part of the snapshot key (DR-89) |
| `models` | either | `auto` | `off` turns every Jev and Claude call off: deterministic tiers, defaults and pre-selected cards (privacy, offline work, the models-off test path) |
| `[claude] backend` | user | `auto` | `auto` (gateway if a credential resolves, else session if a mod is attached, else off), `gateway`, `session`, `off` |
| `[claude] base_url` | user | `https://ai-gateway.vercel.sh` | a credential goes only to this host, `ai-gateway.vercel.sh` or `api.typesafe.ai` |
| `[claude] api_key_env` | user | `AI_GATEWAY_API_KEY` | the variable's name, one of `AI_GATEWAY_API_KEY`, `VERCEL_OIDC_TOKEN` or `TYPESAFE_API_KEY` |
| `[claude] api_key` | user, mode 0600 | none | the gateway key itself, for people who keep it in config rather than the environment |
| `[claude] model_cheap`, `model_capable` | either | `anthropic/claude-haiku-5.5`, `anthropic/claude-sonnet-5.5` | pinned model IDs |
| `[claude] providers` | either | `["anthropic"]` | `providerOptions.gateway.only` |
| `[jev] backend` | user | `gateway` | `gateway` or `typesafe` |
| `[jev] model` | either | `typesafe-ai/jev-1.14.0` | `jev-1.14.0` or `jev-latest` on the direct API |
| `[budget] jev_per_scan_usd` | either | `0.001` | counted per `scan` event, which its one `decide` duty run spends, and per explicit `decide --run`; requests left pending wait for the next scan, so duty runs never chain; S7's CI path raises it to `0.01`, so its $0.001 bound measures real spend (DR-65) |
| `[budget] claude_per_day_usd` | user | `1.00` | real dollars, gateway and TypeSafe calls, across every workspace, from `DATA_ROOT/ledger.db`; session-backend and spawned agents' usage is priced and reported there but never gated (DR-65) |
| `[budget] live_tests_total_usd` | user | `3.00` | the cap on the total of `PLANROOM_SPEND_LOG`, an absolute path, less a $0.25 margin (DR-65) |

### The resolver's environment

The resolver, `plugin/scripts/planroom`, reads no configuration file. It reads only these variables, and `scripts/planroom status --json` reports the step that resolved and where the binary came from, as a `ResolverStatus` built from the generated `contract.sh` (DR-69), naming a non-default source. Its cache is keyed by the version and the first 8 characters of the catalogue hash, `DATA_ROOT/engine/VERSION+HASH/`, so a newer `-dev` build never resolves an older one (DR-78). The mod's guard denies agents the two that choose a binary.

| Variable | Default | Meaning |
|---|---|---|
| `PLANROOM_ENGINE` | unset | a binary to use as it is, never cached; one that isn't an executable file is `engine_unavailable` with reason `override_missing` (DR-81); agents may not set it |
| `PLANROOM_RESOLVE` | `auto` | `auto` tries the checkout, the cache, the download, then the source build; `checkout` skips the cache and the download, and with no build in the checkout reports reason `no_build` (DR-81); `release` skips the checkout |
| `PLANROOM_RELEASE_BASE_URL` | `https://github.com/$PLANROOM_REPO/releases/download`, from `engine.env` | where the archives and `SHA256SUMS` come from: `https` only, or plain `http` to `127.0.0.1`, `localhost` or `::1`. A download from any other base than the default is cached under `DATA_ROOT/engine-alt/<sha256 of the base>/` and used only while the same base is set, so it never becomes the binary later sessions run (DR-78); agents may not set it |
| `PLANROOM_DATA` | `${XDG_DATA_HOME:-$HOME/.local/share}/planroom` | the data root the engine shares (DR-58); always absolute: a relative `XDG_DATA_HOME` counts as unset, and with no absolute place the resolver answers `invalid_argument` with `details.field` `PLANROOM_DATA`, as the engine does ([Where the workspace lives](architecture.md#where-the-workspace-lives)) |

`cargo xtask gen` writes `plugin/scripts/contract.sh` from `contract::ErrorCode` and `contract::resolver`, and the resolver sources it (DR-85). It holds:

- the schema version;
- for each code the resolver prints, `engine_unavailable`, `engine_fetching` and `invalid_argument`, its exit code and a printf template of its whole envelope, category and retry class included, with the list of the template's arguments;
- `engine_unavailable`'s reasons, from `UnavailableReason`;
- the steps, from `ResolverStep`, and the two `status --json` templates, resolved and unresolved.

The resolver spells no code or reason itself, which `tests/installer/run.sh::the_resolver_spells_no_code_or_reason_itself` checks, and `boundaries.toml` lists the file among the generated read-only files.

## Plugin manifests

Owned by Claude Code. Users install from the release line the README names: a marketplace entry whose `git-subdir` source names `path: "plugin"` and the release tag as its `ref` (DR-78, DR-83; A-20 in [Architecture](architecture.md#assumptions-to-verify)). Planroom's tests check only what it relies on: `marketplace.json` names one plugin `planroom` with source `./plugin` on `main`, for the local-path flow; `plugin.json`'s `name` equals the marketplace entry; its `version` is semver, which `claude plugin validate` doesn't check, and equals the Cargo workspace version, every `package.json` version and `plugin/scripts/engine.env`, except that a `-dev` version carries the stamp `.g<8 hex>` of the plugin's own files, which the resolver and `engine.env` leave out when they compare (DR-83); `plugin/` holds no `bin/` and no symlinks; the icon is a square PNG of 512 to 2,048 px [mod-spike].

## Repo artifacts

These aren't product contracts, but tools and reviewers read them, so their shape is fixed here.

- **`budgets.json`** (hand-ratcheted): `{ "<scenario>.<question>": { "tool_tokens": N, "tool_result_tokens": N, "chat_proxy": N, "claude": { "prompt_total": N, "output": N, "cache_hit_rate": F } }, "fold.20k": { "ms": N }, "fold.50k": { "ms": N }, "surface": { "tools": N, "section": N, "skills_and_agents": N, "skill_bodies": { "<skill>": N } }, "room_text": N, "recorded": { "<scenario>.<question>": { "chat_prompt": N, "chat_output": N, "chat_cache_hit_rate": F, "measured_by": "cargo xtask live evidence S02" }, "token_ratio": F } }`. CI gates every number outside `recorded`; `chat_proxy`, an S2 question's projected chat input (the always-on surface, the skill bodies its path loads and its tool results), is one of them. `recorded` changes only with the command it names [DR-51], and that command refuses a value more than 20% above the old one, or a hit rate more than 0.1 lower, unless the commit adds a decision record (DR-65). `cargo xtask gen --budgets` never writes this file: it writes `target/budgets.measured.json` and prints the diff. A person lowers a number by hand; a commit that raises a gated one must add a decision record (DR-59). A raise is a larger value for a key the file already holds; a new key isn't a raise and needs no record (DR-89). A key that holds no number yet is `{}` (an object of numbers) or `null` (a number), and its first number isn't a raise: M1 creates the file with every key above that it gates and none of their numbers, which arrive with the checks that measure them.
- **`layout.lock`** (written by `cargo xtask gen --layout-lock DR-n`): TOML with the generated header, one table per gated surface, `["<surface>"]` with `sha256 = "..."` and `record = "DR-n"` (DR-83), one per gated surface: `boundaries.toml`, the workspace-crate dependency graph, the top-level directory list, each package's `exports` and its dependency names (never their versions, so a Dependabot bump needs no record), the area contracts table in `repo-layout.md`, parsed so formatting doesn't count, and the Boundaries and Conventions sections of `AGENTS.md` (DR-59, DR-70). Each `record` names a record heading that appears once in `decisions.md`; the first lock names DR-79 for every surface (DR-81).
- **`schema/baseline/`**: copies of both schema views at the last `schema_version` bump, or before any, at the commit that added them. `cargo xtask gen --schema-baseline` writes them, and the breaking-change guard reads them (DR-59).
- **`docs/evidence/spend.jsonl`** (appended, committed): rows of live spend, the `sample` calls an import prices included, `{ "at", "generation_id", "provider", "model", "cost_usd", "scenario" }` and an optional `phase`, where `provider` is `jev`, `gateway`, `typesafe` or `sample` and `cost_usd` is in dollars to six decimal places. A live call the engine makes appends a row with `phase: "reserved"` and its ceiling before it is sent and, when the reply reports its cost, one with `phase: "settled"`, its `generation_id` and the difference to that cost, so the rows sum to what was spent (DR-65). A row with no `phase` is one whole call. The walking skeleton seeds it with Phase 1's live spend, `scenario` `phase1`. It is written only through the absolute path `cargo xtask` sets, under `spend.jsonl.lock` (DR-65).
- **`feature_list.json`**: `{ "format": "planroom.feature-list/v1", "about", "fixture_note", "features": [{ id, title, tier (A, B or C), status, requirements, ci_path: { fixture, steps, compare, asserts, stand_ins }, evidence_path: { dir, steps }, tests, due?, decision }] }`. `compare` names the `TRUTH.json` paths a step is checked against; `decision` (tier C only) names the record that added the entry. Status is `failing` or `passing`; an entry turns `passing` only after its tests pass. An entry may carry `due`, an object from a listed test to the milestone that builds it, such as `"due": {"crates/planroom/tests/first_run.rs::first_scan_deals_at_most_three_cards": "M2"}`; the field is additive, so the format stays `v1`. The list is complete when every listed test resolves to a test that CI runs with nothing live, or carries a `due`. A milestone closes only when no listed test still carries its `due` (DR-89).

## Drift tests

| Test | Checks |
|---|---|
| `committed_schema_matches_the_generated_schema` | both schema views equal a fresh generation byte for byte |
| `generated_ts_matches_the_schema` | `json2ts` into a temp dir equals `packages/contract/src/generated/` |
| `the_schema_is_valid_and_enumerates_the_stable_codes` | valid draft 2020-12; `$id`; `schema_version` const; enums equal `ErrorCode::ALL`, `WarningCode::ALL`, `EventKind::KNOWN` and the command list |
| `open_sets_accept_later_values_of_the_documented_form_only` | future codes of the right form validate; malformed ones and other `schema_version`s don't |
| `every_cli_json_output_validates` | every envelope the test suite produces validates against `Envelope` and its command's result |
| `the_documented_error_table_is_the_error_codes` | `docs/reference/errors.md` tables equal the registry both ways, retry class included; a code only a person can clear has retry `never` (DR-66) |
| `errors_are_only_built_from_the_registry` | source scan: no code string outside the registry and its generated files |
| `every_resolver_output_validates` | `tests/installer/run.sh`, planned for M5: the resolver's `status --json`, `engine_fetching` and each `engine_unavailable` reason validate against the committed schema (DR-69). Until then, the resolver's row in [Every public contract](#every-public-contract) names the tests that check its outputs (DR-85) |
| `every_leaf_has_a_budget_or_streams` | `catalogue.rs`: every leaf has a token budget, or is `watch`, `serve` or `export`; a pack question's budget is at most 3,000 (DR-21) |
| `cli_reference_equals_help` | `docs/reference/cli.md` equals clap's rendered help |
| `catalogue_has_one_handler_per_command` | every leaf has exactly one handler and every handler a leaf |
| `tool_schemas_equal_catalogue` | the mod's tool schemas equal the generated ones |
| `golden_outputs_type_check` | `tests/golden/*.json` type-check in the mod's tests, the only code that may read them (DR-81) |
| `golden_outputs_equal_engine_output` | each Rust test that owns a golden file compares the engine's output with the committed file and fails on a difference; only `cargo xtask gen --golden` writes them, so the mod tests never run against a stale envelope (DR-79) |
| `migrations_open_every_released_fixture` | each released store fixture opens, migrates and folds |
| `fixture_format_is_unchanged` | `ImplementerRun` and `ReviewThreads` required fields equal `examples/README.md` |
| `breaking_change_guard` | against `schema/baseline/` and the last `v*` tag's schema: no removed property, removed enum value or new required input without a version bump |
| `undo_classes_snapshot` | every leaf's undo class; `NotUndoable` is only `share`, and `scan` is `Query` (DR-55, DR-81) |
| `area_contracts_match_repo_layout` | each area's `AGENTS.md` contract block, between its generated markers and its Read first lines included, equals its rows in `repo-layout.md`; every tracked top-level directory and each `tests/*` directory has an area row; each area row's "May depend on" cell lists exactly that crate's or package's allow-list in `boundaries.toml`; every Read first section name resolves to a heading of its page (DR-59, DR-70, DR-79) |
| `every_directory_has_a_purpose` | every directory `git ls-files` holds, except under `examples/fixture/` and the generated directories, appears in the Directory map of `repo-layout.md` or in its area's Layout lines in `AGENTS.md` (DR-79) |
| `generated_headers` | every path the Generated directories table in `repo-layout.md` names has the header, or its generated region its BEGIN and END GENERATED markers, and equals a fresh generation; `generated_read_only` and `.gitattributes` are generated from that table (DR-79) |
