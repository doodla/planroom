# Repo layout

This page is the map of the repository (R16): what every directory is for, each top-level area's contract, the boundary rules and how tooling enforces them in Rust and TypeScript, the generated directories, the one task entry point, toolchain pins, the codegen flow, and the conventions `AGENTS.md` holds. Changing the layout, a boundary or a convention needs a decision record in [Decisions](decisions.md) first, and `cargo xtask check layout` fails without one [DR-59].

## Directory map

```text
planroom/
├── AGENTS.md                      invariants and conventions for agents and people, the conventions' only home; CLAUDE.md imports it
├── CLAUDE.md                      "@AGENTS.md" on line 1, then the existing carry and subagent sections, unchanged
├── README.md  CHANGELOG.md  CONTRIBUTING.md  SECURITY.md  CODE_OF_CONDUCT.md  LICENSE
├── KICKOFF.md                     the brief; never edited (S5 and S6 use examples/kickoff-snapshot.md)
├── progress.md                    the build record: phases, assumptions, pre-mortems, estimates, spend
├── Cargo.toml  Cargo.lock         the one Cargo workspace: crates/*
├── rust-toolchain.toml            channel 1.97.0, rustfmt and clippy, the four release targets
├── rustfmt.toml  deny.toml
├── package.json  pnpm-workspace.yaml  pnpm-lock.yaml  .npmrc   the one JS workspace: packages/*, tests/e2e
├── .node-version                  22.22.0
├── tsconfig.base.json  eslint.config.js  .prettierrc.json
├── boundaries.toml                the single source of boundary rules for Rust, TypeScript and packs
├── .dependency-cruiser.cjs        GENERATED from boundaries.toml
├── feature_list.json              S1 to S15 (and tier C additions): tier, CI path, evidence path, tests, status
├── budgets.json                   hand-ratcheted token and time budgets; a commit that raises one adds a decision record [DR-59]
├── layout.lock                    the gated surfaces' sha256 and the record that approved each [DR-59]
├── .gitattributes                 generated paths marked linguist-generated
├── .claude-plugin/
│   └── marketplace.json           one plugin, "planroom", source "./plugin"
├── .claude/
│   ├── guardrails/                hook rules and tests (unchanged; run in CI)
│   ├── settings.json              env PLANROOM_REPLAY=strict and PLANROOM_SPEND_LOG for agents in this repo [DR-49, DR-51]
│   ├── carry.md
│   └── skills/                    project skills for building Planroom (added once a procedure repeats)
├── .github/
│   ├── workflows/                 ci.yml, release.yml, advisories.yml, toolchain.yml
│   ├── dependabot.yml             cargo, npm, github-actions; weekly, grouped, with a 7-day cooldown like pnpm's minimumReleaseAge
│   ├── ISSUE_TEMPLATE/            bug (it asks for `planroom doctor --json`), feature, pack request
│   └── PULL_REQUEST_TEMPLATE.md
├── crates/
│   ├── contract/   planroom-contract   every public type; the error and warning registries; the command catalogue
│   ├── kernel/     planroom-kernel     pure, domain-free logic: fold, roll-up, query shapes, diff, impact,
│   │                                   reconcile, policy, lint, machine checks, the two loop tables; no I/O
│   ├── store/      planroom-store      SQLite: the ten tables, commit, tail, leases, presence, migrations; the user-level
│   │                                   ledger, event backups and pre-migration copies under the data root
│   ├── extract/    planroom-extract    git CLI, file walk, formats, tree-sitter, the declarative runner,
│   │                                   built-in extractors and linkers, evidence verification
│   ├── models/     planroom-models     transport (live, record, replay); Jev codecs; Claude gateway and session
│   ├── planroom/   planroom            the binary: cli/, server/, live.rs, app/ (composition root, actor and trust), config/, text/;
│   │                                   build.rs embeds the packs and, when built, target/canvas/
│   ├── testkit/    planroom-testkit    dev-only: fixture builder wrapper, truth comparator, S8 load and faults, golden helpers
│   └── xtask/      xtask               the one task entry point (publish = false)
├── packs/                          data only, embedded in the binary at build time
│   ├── code/  plan/  observability/  release/  artifacts/  machines/
├── packages/
│   ├── contract/   @planroom/contract  src/generated/ (GENERATED) and src/index.ts (re-exports only)
│   ├── ui/         @planroom/ui        design tokens, view-model rows, hotkeys, braille raster, machine stepper;
│   │                                   no React, no DOM, no Node
│   ├── canvas/     @planroom/canvas    the Vite app; builds to target/canvas/
│   └── mod/        @planroom/mod       mod source and tests; builds to plugin/hooks/planroom-mod.js
├── plugin/                          the shipped plugin; never holds bin/ or symlinks
│   ├── .claude-plugin/plugin.json   .claude-plugin/icon.png
│   ├── README.md
│   ├── hooks/hooks.json             {"modules": ["./planroom-mod.js"]}
│   ├── hooks/planroom-mod.js        GENERATED bundle of packages/mod
│   ├── types/index.d.ts             GENERATED $.state contract
│   ├── skills/room/  skills/room-plan/  skills/room-handoff/    SKILL.md each, user-invocable: false
│   ├── agents/implementer.md  agents/curator.md
│   └── scripts/planroom  scripts/engine.env   the POSIX sh resolver (with prefetch, build and status) and its pinned version
├── schema/                          GENERATED: planroom.v1.schema.json, planroom.v1.input.schema.json; baseline/, the breaking-change guard's copy
├── examples/
│   ├── fixture/  build-fixture  check-fixture  TRUTH.json  README.md  kickoff-snapshot.md
│   └── packs/ruby/  packs/flags/    out-of-tree data packs for S10 (Ruby is also loaded by the fixture workspace)
├── tests/
│   ├── replay/jev/  replay/claude/  recorded responses keyed by request hash
│   ├── golden/                      GENERATED by the Rust tests: view models, CLI outputs, change specs
│   ├── e2e/                         @planroom/e2e (private): Playwright specs, shims/ (window.claude, fake Artifact db)
│   ├── installer/                   the resolver's offline suite
│   └── plugin/                      plugin validation, install smoke, surface-token and slash-command snapshots
├── scripts/                         package-release.sh, smoke-test-release.sh, check-tag-version.sh (POSIX sh, called by xtask and release CI)
└── docs/
    ├── README.md                    the docs index and the fact-to-home table
    ├── guides/  concepts/           how to do a task; how it works and why
    ├── reference/                   cli.md, errors.md, configuration.md, packs.md (GENERATED); json-output.md, http-api.md
    ├── contributing/                architecture, decisions, contracts, repo layout, traceability, design language,
    │                                style guide, adding a pack, live testing, releasing; research/ (Phase 1 notes)
    ├── evidence/sNN/                live runs, recorded once per scenario
    ├── evidence/spend.jsonl         every live call of the build, appended by the engine; the $3 cap reads it
    ├── screenshots/                 every main view at 1440×900 and 390×844, both themes
    └── art/                         Iris images and their prompts
```

## Area contracts

Each row is the source of that area's contract block: `cargo xtask gen` writes the block into the area's `AGENTS.md`, or its `README.md` for the areas that ship to users (packs and the plugin), and `area_contracts_match_repo_layout` fails when they differ. A one-line `CLAUDE.md` holding `@AGENTS.md` sits beside each area's `AGENTS.md`, so Claude Code loads the contract when an agent reads files in that area, and `cargo xtask check` fails when an area in `boundaries.toml` lacks either file [DR-59].

| Area | Owns | May depend on | Must not | How to test it |
|---|---|---|---|---|
| `crates/contract` | every public type, `ErrorCode`, `WarningCode`, `Command` and `CommandSpec` | `serde`, `serde_json`, `schemars`, `clap` (derive only), `uuid`, `indexmap` | any other workspace crate; I/O; logic beyond validation | `cargo xtask test -p planroom-contract` (schema snapshot, round trips) |
| `crates/kernel` | the fold, resolution, roll-up, the twelve query shapes and question pipelines, diff, impact, reconcile, policy bands, lint rule types, machine checks and step, budgets, next action, redaction, the decision and hand-off loop tables, roles | `contract`, plus the allow-list in `boundaries.toml` (`serde`, `serde_json`, `petgraph`, `similar`, `nucleo-matcher`, `sha2`, `thiserror`, `indexmap`, `regex` for redaction) | I/O, async, `rusqlite`, `reqwest`, `tree-sitter`; any built-in pack kind or relation name as a string literal, with or without its pack prefix | unit and property tests (`rollup`, `cas`, `undo`, `query`, `reconcile`, `evals`); `cargo bench fold` |
| `crates/store` | the tables, `commit` (idempotency, compare-and-set by writer instance, status by actor), the per-blob fact cache, the filesystem check and file modes, `tail`, leases, presence, layouts, migrations with their `VACUUM INTO` copies; the user-level ledger and event backups under the data root | `contract`, `rusqlite` | other workspace crates; business rules (it calls a kernel function passed in by `app`) | `concurrency.rs` (3 processes × 500 writes), `crash.rs`, `idem.rs`, `fs_check.rs`, `ledger.rs`, `writer_interleavings` proptest, migration fixtures |
| `crates/extract` | the `Extractor` and `Linker` traits, built-ins, the declarative runner, evidence location and verification, the git CLI adapter | `contract`, `tree-sitter` and grammars, `ignore`, `globset`, `regex`, `saphyr`, `toml`, `pulldown-cmark`, `promql-parser` | other workspace crates; running anything from a scanned repo except `git` with hooks off | `contract.rs` (every extractor over the fixture and a fuzz corpus), `link.rs`, per-extractor fixtures, the ABI floor test |
| `crates/models` | `Transport` (live, record, replay), Jev codecs, the Claude gateway and session backends, retries, spend accounting, the live gate and the host allow-list | `contract`, `reqwest`, `tokio`, `sha2` | other workspace crates; any network call without `PLANROOM_LIVE_*` or a consented credential; treating an ambient proxy as a credential | `jev_contract.rs` (both codecs over every fixture), `claude_decoders.rs`, `session_backend.rs`, `redact.rs` |
| `crates/planroom` | the CLI, the server, the live loop, app services (orchestration only), actor resolution, workspace trust, consent cards, pack loading and embedding, canvas embedding, config (workspace and user-level), the string catalog | every workspace crate, `axum`, `tower-http`, `tokio`, `tokio-stream`, `tracing`, `anyhow` | business rules that belong in the kernel; spelling an error code outside the registry; reading a credential setting from a workspace's config | scenario tests `tests/sNN_*.rs`, `budgets.rs`, `actor.rs`, `trust.rs`, `privacy.rs`, `live_gate.rs`, `backup.rs`, golden outputs |
| `crates/testkit` | the fixture wrapper (`examples/build-fixture` plus the workspace config), the truth comparator and its planted-name mapping, the S8 load harness and fault injection, golden helpers | `contract`, `assert_cmd`, `tempfile`, `serde_json` | being a normal dependency of any crate (dev-only) | its own unit tests; every scenario test uses it |
| `crates/xtask` | task commands | anything | shipping in a release | it runs in CI |
| `packs/*` | kinds, relations, hints, languages, image roles, extractor and linker declarations, lenses, questions, rules, decision kinds, suggestions, prompts, stages, evals, pack tests | the kernel's vocabulary and the `code` pack | code or executable files; other packs except `code` | `tests/packs.rs` (validation, the inward-only rule); `planroom doctor --pack` on each; each pack's scenario tests |
| `packages/contract` | generated types, the catalogue, tool schemas | nothing | hand edits under `generated/` | `tsc`, drift check |
| `packages/ui` | design tokens (TS and CSS variables, light and dark), view-model rows, hotkey and row-key assignment, truncation, the braille raster, the machine stepper | `@planroom/contract` | React, the DOM, Node APIs | vitest; `machine_step_parity` against Rust golden outputs; `contrast_meets_the_bar` over the tokens |
| `packages/canvas` | the canvas app, both transports, views, the ELK worker, motion | `@planroom/contract`, `@planroom/ui`, React, `@xyflow/react`, `elkjs`, `zustand`, `motion`, `radix-ui` | `@planroom/mod`; engine internals; fetching any host but its own origin | vitest, Playwright e2e |
| `packages/mod` | the mod: wiring, pane, band, fallback panes, `/room`, tools, guards (touch set, stores, person-only commands, the implementer's worktrees), broker, agents and their usage | `@planroom/contract`, `@planroom/ui`, the `claude-code` mod types | `@planroom/canvas`; the DOM or Node APIs; `import()`; anything that changes by turn in the composed prompt section | vitest (pure logic); `claude plugin test target/mod-test`, the generated harness, on all four surfaces with golden CLI outputs |
| `tests/e2e` | Playwright specs and the browser shims | `@planroom/contract`, `@playwright/test` | product code (it drives the built canvas) | it is the test |
| `plugin/` | the shipped plugin | built outputs and hand-written skills, agents, resolver | `bin/`; symlinks; hand-written copies of generated files | `claude plugin validate --strict`; `tests/plugin/`; `tests/installer/` |
| `examples/` | the fixture, the truth, the snapshot, the example packs | | edits to `TRUTH.json` without a decision record; edits to `kickoff-snapshot.md` | `examples/check-fixture`; the scenario tests |
| `docs/` | guides, concepts, reference, contributing, evidence, screenshots, art | | restating a fact that has a home; hand edits to generated reference pages | the docs tests (links, prose rules, CLI reference) |

## Boundaries and their enforcement

`boundaries.toml` is the single source. `cargo xtask check boundaries` reads it with `cargo metadata`, and `cargo xtask gen` writes `.dependency-cruiser.cjs` from it, so the two languages' rules can't drift apart [DR-36].

```toml
[rust.crates]
contract = { may_depend = [] }
kernel   = { may_depend = ["contract"],
             external_allow = ["serde", "serde_json", "petgraph", "similar", "nucleo-matcher", "sha2", "thiserror", "indexmap", "regex"] }
store    = { may_depend = ["contract"], external_forbid = ["reqwest", "hyper", "tokio"] }
extract  = { may_depend = ["contract"], external_forbid = ["reqwest", "hyper", "tokio"] }
models   = { may_depend = ["contract"] }
planroom = { may_depend = ["contract", "kernel", "store", "extract", "models"] }
testkit  = { may_depend = ["contract"], dev_only = true }
xtask    = { may_depend = ["*"], ships = false }

[rust.kernel]
domain_free = true          # no string literal under crates/kernel/src equals a built-in pack kind or relation name, with or without "<pack>."
no_path_escapes = true      # no #[path = "../"] or include! leaving a crate's directory (every crate)
no_io = true                # cargo xtask gen writes crates/kernel/clippy.toml, disallowing std::fs, std::net, std::process, std::env, SystemTime::now

[ts.packages]
contract = { may_import = [] }
ui       = { may_import = ["contract"], forbid = ["react", "react-dom", "node:*"] }
canvas   = { may_import = ["contract", "ui"] }
mod      = { may_import = ["contract", "ui"], forbid = ["react-dom", "node:*"] }
e2e      = { may_import = ["contract"] }

[ts.rules]
entry_only = true           # cross-package imports only through package.json "exports"
no_circular = true
not_to_unresolvable = true
generated_read_only = ["packages/contract/src/generated/**", "plugin/types/index.d.ts", "plugin/hooks/planroom-mod.js"]

[packs]
requires = [[], ["code"]]   # the only allowed values
no_executables = true
```

| Boundary | Rule | Enforced by |
|---|---|---|
| The kernel depends on no domain | `planroom-kernel` depends only on `contract` and its external allow-list; no built-in pack kind or relation name, with or without its prefix, in its string literals; prop names reach machine checks as shape parameters | `cargo xtask check boundaries` (crate graph and allow-list from `cargo metadata`; the literal scan) |
| The kernel does no I/O | no `std::fs`, `std::net`, `std::process`, `std::env` or `SystemTime::now` in `crates/kernel/src` | clippy's `disallowed-methods` and `disallowed-types` from `crates/kernel/clippy.toml`, under `-D warnings` |
| Adapters depend inward, never sideways | `store`, `extract` and `models` depend on `contract` only; only `models` may use a network client | the same check, with each crate's `external_forbid` read from `cargo metadata` for normal, build and dev dependencies |
| One composition root | only `planroom` (and `xtask`) depend on more than one workspace crate; `testkit` only as a dev-dependency | the same check |
| Packs depend inward | `code` is the base ring every domain pack attaches to, so `requires` is `[]` or `["code"]`; every referenced kind belongs to the pack, `code` or the kernel; no executables [DR-9] | the pack loader and `crates/planroom/tests/packs.rs` |
| Surfaces reach the engine only through published contracts | `canvas` and `mod` import `@planroom/contract` and `@planroom/ui` only through their exports; `ui` has no React or DOM; the mod has no Node APIs | `depcruise packages tests/e2e --config .dependency-cruiser.cjs --output-type err` (allow-list, default deny); the mod's `tsconfig` `lib: ["es2023"]`, `types: []` |
| Generated code is never hand-edited | every file under a generated directory has the header and equals a fresh generation | `cargo xtask check` drift checks |
| No OpenSSL, no unknown sources | `deny.toml` bans `openssl` and `openssl-sys`, denies wildcards and unknown git sources | `cargo deny check licenses bans sources` inside `cargo xtask check` |

### Planted-import self-test

A planted bad import must fail CI (S15), and a broken checker must fail the self-test [DR-59]. `cargo xtask check boundaries --self-test` does the following:

1. Copies the repo to a temporary directory with `git worktree add --detach`, then runs `pnpm install --frozen-lockfile --offline` in the copy, from the store the main checkout filled.
2. Runs every boundary check on the unmodified copy and requires exit 0. This control proves the copy resolves, so a later failure is the plant's.
3. Adds `planroom-store` to `crates/kernel/Cargo.toml` and a `use planroom_store as _;` line, runs the Rust check, and expects exit 1 with "kernel -> store is not allowed".
4. Adds `"code.service"`, then the bare relation name `"watches"`, as string literals in `crates/kernel/src/lib.rs`, runs the literal scan, and expects each to fail. Then adds a `std::fs::read_to_string` call there, runs clippy on the kernel, and expects the disallowed-method error.
5. Adds `"@planroom/mod": "workspace:*"` to `packages/canvas/package.json`, links it offline, and writes `packages/canvas/src/planted.ts` importing `@planroom/mod`. Then it replaces that import with a relative `../../mod/src/index.ts`. For each, depcruise must report `not-in-allowed`, the default-deny rule, and no `not-to-unresolvable` error, so the plant fails for the right reason. The generated allow rule for third-party packages matches a resolved path under `node_modules/`, never a dependency type, so a declared workspace package, which resolves to `packages/`, can't pass as third-party.
6. Adds `requires = ["observability"]` to a copy of the flags pack, runs the loader, and expects `pack_invalid`.
7. Changes `boundaries.toml` with no new record, runs `cargo xtask check layout`, and expects it to fail.
8. Removes the worktree.

CI runs the self-test on every push.

### Decision-record gate

Changing the layout, a boundary or a convention needs a record before it lands (R16), and the gate checks it without depending on any branch's merge base [DR-59].

- **Gated surfaces:** `boundaries.toml`, the `[dependencies]` between workspace crates, the top-level directory list, each package's `exports`, and the Conventions section of `AGENTS.md`.
- **The lock.** `layout.lock` holds each surface's sha256 and the record that approved it. `cargo xtask check layout` recomputes the hashes and fails on any difference, naming the fix: `cargo xtask gen --layout-lock DR-n`. That command refuses unless `## DR-n` is in `decisions.md` and is newer than every record the lock already names.
- **Per commit.** In CI, `check layout` also walks each commit in `BASE..HEAD`, where `BASE` is the fork point from `main`, or a track's own fork point when the main agent merges a track. A commit that changes `layout.lock` must add the `## DR-n` heading the new entry names in the same commit. CI checks out with `fetch-depth: 0`, and a base it can't find fails the check, never skips it.
- **Budgets.** `budgets.json` is hand-ratcheted. A commit that raises any gated number in it must add a record in the same commit; lowering a number needs none.

## Generated directories

Each has a `GENERATED.md` stub or a header line, and a drift check. Nobody edits them by hand; run the generator instead.

| Path | Generator | Committed? |
|---|---|---|
| `schema/` | `cargo xtask gen` (schemars) | yes |
| `schema/baseline/` | `cargo xtask gen --schema-baseline`, run only with a `schema_version` bump | yes [DR-59] |
| `packages/contract/src/generated/` | `cargo xtask gen` (json-schema-to-typescript, the catalogue, its hash, and the fallback panes from `en.toml`) | yes |
| `plugin/types/index.d.ts` | `cargo xtask gen` | yes |
| `plugin/hooks/planroom-mod.js` | `cargo xtask gen --mod` (esbuild, ESM, no splitting, `claude-code` external; deterministic) | yes, because a local-path marketplace loads the plugin from the repo |
| `.dependency-cruiser.cjs` | `cargo xtask gen` from `boundaries.toml` | yes |
| `crates/kernel/clippy.toml` | `cargo xtask gen` from `boundaries.toml` `no_io` | yes [DR-59] |
| each area's contract block in its `AGENTS.md` or `README.md` | `cargo xtask gen` from the area contracts table on this page | yes [DR-59] |
| `layout.lock` | `cargo xtask gen --layout-lock DR-n` | yes [DR-59] |
| `docs/reference/cli.md`, `errors.md` tables, `configuration.md`, `packs.md` tables | `cargo xtask gen` (`PLANROOM_UPDATE_DOCS=1` for the CLI reference) | yes |
| `tests/golden/` | the Rust tests with `PLANROOM_UPDATE_GOLDEN=1` | yes |
| `packs/*/evals/*.result.json` | `cargo xtask eval` | yes |
| `target/canvas/` | `cargo xtask gen --canvas` (`vite build`); `crates/planroom/build.rs` embeds it | no |
| `target/mod-test/` | `cargo xtask gen --mod`: the mod and its tests bundled by esbuild, the workspace packages inlined, `claude-code` and `claude-code/testing` external, the golden CLI outputs as a generated module, and a `hooks/hooks.json` naming the bundle | no [DR-59] |
| `target/budgets.measured.json` | `cargo xtask gen --budgets`, which prints its diff against `budgets.json` and never writes that file | no [DR-59] |

## The one task entry point

`cargo xtask` needs nothing beyond the pinned Rust and Node, because `just` isn't installed [DR-38]. Agents and CI use only these commands, and CI runs nothing else [DR-59]. Each first runs `pnpm install --frozen-lockfile --offline`, falling back to an online install, and on failure prints one line naming the fix. Each never stops at the first failure, prints one `ok`, `FAILED` or `skipped (why)` line per check, and names the fix command on failure. Every command sets `PLANROOM_REPLAY=strict` and `PLANROOM_SPEND_LOG=docs/evidence/spend.jsonl`, and the test commands give each run a temporary `PLANROOM_DATA` [DR-49, DR-51].

| Command | Runs | Typical time (warm) |
|---|---|---|
| `cargo xtask check` | `cargo fmt --check`; clippy with `-D warnings`; `prettier --check`; `eslint`; `tsc -b`; depcruise; the boundary rules and self-test; every drift check; the docs tests; release pins; the guardrails tests (`python3 .claude/guardrails/tests/test_guardrails.py` and `test_context_restore.py`); `examples/check-fixture`; `claude plugin validate --strict` on `.` and `plugin/`; `cargo deny check licenses bans sources`; the layout gate; `ci_runs_only_xtask`; `feature_list_is_complete`; the secret scan over `tests/replay/` and `docs/evidence/` | 2 to 3 min |
| `cargo xtask check boundaries [--self-test]` | the boundary rules alone, or with the control and the planted crossings | 40 s |
| `cargo xtask check layout [--commits BASE..HEAD]` | the layout gate against `layout.lock`, and in CI each commit since the fork point | 5 s |
| `cargo xtask check audit` | `cargo deny check advisories` and `pnpm audit --prod`, which need the network; locally `skipped (offline)` when it's down, in CI a failure | 30 s |
| `cargo xtask check guardrails` | the two guardrails test files alone | 5 s |
| `cargo xtask test [--scenario SNN] [--installer]` | `cargo nextest run` plus `cargo test --doc`; `vitest run`; `claude plugin test target/mod-test`; the scenario CI paths S1 to S15 on fresh fixtures; the `tests/plugin/` scripts, except `slash_commands_snapshot.sh`, which needs a live session and runs from `cargo xtask live evidence S15`; budgets; eval recompute. `--installer` runs ShellCheck, BusyBox `sh` and `tests/installer/run.sh` instead | 4 to 6 min |
| `cargo xtask e2e [--screens]` | `playwright install chromium` at the pinned version (`--with-deps` in CI); builds the canvas; `planroom serve` on a built fixture with replay; Playwright at 1440×900 and 390×844, light and dark; the empty-state audit through both transports; reduced motion; console errors; the Artifact page with the fake `db`; the plugin install smoke in an isolated `HOME`, with mods on and off | 4 min |
| `cargo xtask gen [--budgets] [--mod] [--canvas] [--layout-lock DR-n] [--schema-baseline]` | every generator above | 30 s |
| `cargo xtask fixture OUT [--no-packs]` | `examples/build-fixture OUT`, then `OUT/.planroom/config.toml` loading `examples/packs/ruby` (omitted with `--no-packs`), recorded as a trusted workspace in the data root [DR-48] | 2 s |
| `cargo xtask eval [--verify-order] [--report]` | recompute every eval result from replay; check label order with git; write the S7 table | 10 s |
| `cargo xtask release-dry-run [--target T]` | release toolchain check; canvas build; release build, which embeds it; package twice and `cmp`; `scripts/check-tag-version.sh` against the workspace version; smoke test on the built fixture, with `serve` run from `target/release/` and from a `cargo install --path` root and a fetch of `/`; the resolver suite against a `127.0.0.1` server holding a locally built, checksummed archive | 6 min |
| `cargo xtask live <eval KIND \| record SNN \| evidence SNN>` | needs `PLANROOM_LIVE_JEV=1` or `PLANROOM_LIVE_GATEWAY=1`; prints its estimate from the live spend plan and refuses past what `docs/evidence/spend.jsonl` has left under `live_tests_total_usd`; every call appends to that file; writes `tests/replay/` or `docs/evidence/` [DR-51] | varies |

## Toolchain pins

Each pin has one home and a `release_pins` test that fails when another file names a different version [iris-release-bar §7].

| Pin | Home |
|---|---|
| Rust 1.97.0 | `rust-toolchain.toml` |
| Node 22.22.0 | `.node-version` |
| pnpm 10.28.2, exactly, with `minimumReleaseAge: 10080`; CI gets it through corepack | `package.json` `packageManager`, `pnpm-workspace.yaml` |
| Dependabot cooldown of 7 days, equal to `minimumReleaseAge` / 1440 | `.github/dependabot.yml`; `release_pins.rs::dependabot_cooldown_matches_minimum_release_age` |
| Claude Code 2.1.310 | an `xtask` constant; `xtask` installs that exact version with `DISABLE_AUTOUPDATER=1` |
| Python 3.11 or later (guardrails, `build-fixture`, `check-fixture`) | `xtask` check, CI setup |
| git 2.30 or later | `planroom doctor`, CI setup |
| `cargo-deny 0.20.2`, `cargo-nextest 0.9.149`, `cargo-about 0.9.2`, ShellCheck and BusyBox for the installer suite | sha256-pinned downloads in `xtask` |
| `json-schema-to-typescript 16.0.0` | `packages/contract/package.json` |
| Playwright 1.63.0 and its Chromium | `tests/e2e/package.json`; `cargo xtask e2e` installs the matching Chromium |
| Jev and Claude model IDs | `crates/planroom/src/config/defaults.toml` |
| Every crate and npm package | exact versions in `Cargo.toml` and `package.json`; `Cargo.lock` and `pnpm-lock.yaml` committed |

## Codegen flow

The Rust types in `crates/contract/src/` are the one source. `cargo xtask gen` turns them into the two schema views, then TypeScript, the catalogue, the mod's state types and the generated reference pages; `cargo xtask check` regenerates everything in memory and fails on any difference. The full flow and every drift test are in [Contracts](contracts.md#codegen-flow).

## Plugin layout

```text
.claude-plugin/marketplace.json      name "planroom"; one plugin, source "./plugin"  → install ID planroom@planroom
plugin/
  .claude-plugin/plugin.json         name, version (= the workspace version), description, author, license "MIT",
                                     homepage, repository, keywords
  .claude-plugin/icon.png            1024 px square, from the Iris logo
  README.md                          what it is, the first command, links; one line on turning mods on
  hooks/hooks.json                   {"modules": ["./planroom-mod.js"]}
  hooks/planroom-mod.js              GENERATED
  types/index.d.ts                   GENERATED
  skills/room/SKILL.md               user-invocable: false
  skills/room-plan/SKILL.md          user-invocable: false
  skills/room-handoff/SKILL.md       user-invocable: false
  agents/implementer.md              model sonnet, effort high, tools Read, Edit, Write, Bash, Grep, Glob
  agents/curator.md                  model sonnet, effort medium, tools Read, Bash, Grep, Glob
  scripts/planroom                   the POSIX sh resolver; finds engine.env and the checkout from its own path;
                                     prefetch, build and status subcommands for the mod [DR-58]
  scripts/engine.env                 PLANROOM_ENGINE_VERSION, PLANROOM_REPO (written at release); PLANROOM_CATALOGUE_HASH
                                     (written by cargo xtask gen, drift-checked)
```

No `bin/`, no `.mcp.json`, no symlinks, no classic hooks; `tests/plugin/` asserts each [DR-14, DR-28, DR-30].

## CI workflows

`.github/workflows/ci.yml` runs on `push` to every branch (`branches: ["**"]`), on pull requests and on `workflow_dispatch`. It cancels superseded runs per ref except on `main`, sets `permissions: contents: read` and `persist-credentials: false`, checks out with `fetch-depth: 0` so the layout gate and `eval --verify-order` can walk history, and pins every action by SHA. No job needs a secret or calls a live model. Every job's only `run:` steps are the listed setup steps (`rustup toolchain install`, `corepack enable`, `pnpm install --frozen-lockfile`) and one `cargo xtask` command, and `ci_runs_only_xtask` fails on anything else [DR-59].

| Job | Command | Runner |
|---|---|---|
| `check` | `cargo xtask check` | ubuntu-24.04 |
| `test (os)` | `cargo xtask test` | ubuntu-24.04, macos-15; wall-clock gates only on ubuntu-24.04, recorded on macos-15 [DR-43] |
| `e2e` | `cargo xtask e2e` | ubuntu-24.04 |
| `audit` | `cargo xtask check audit` | ubuntu-24.04 |
| `guardrails` | `cargo xtask check guardrails` (also inside `check`, so a hook regression shows by name) | ubuntu-24.04 |
| `installer (os)` | `cargo xtask test --installer` | ubuntu-24.04, macos-15 |
| `release-dry-run (target)` | `cargo xtask release-dry-run --target T` | the release workflow's runners: ubuntu-24.04 and ubuntu-24.04-arm for the musl targets, macos-15-intel for `x86_64-apple-darwin`, macos-15 for `aarch64-apple-darwin` |

Scheduled: `advisories.yml` (weekly `cargo xtask check audit`) and `toolchain.yml` (weekly pin-age checks for Rust, Node and Claude Code). `release.yml` runs `verify`, `checks`, `build` and `publish` on a `v*` tag push, or as a dry run on `workflow_dispatch`; only `publish` has `contents: write`. Its steps and targets are in [Architecture](architecture.md#packaging-and-release). `.github/dependabot.yml` opens weekly, grouped updates with a 7-day cooldown, so no update is younger than pnpm's `minimumReleaseAge`.

## Conventions

The conventions live once, in the Conventions section of [`AGENTS.md`](../../AGENTS.md#conventions), which the layout gate watches; this page doesn't restate them [DR-59]. `AGENTS.md` follows Iris's structure: one opening paragraph ("invariants and steering rules, not a description of the code"), then flat bullet sections: Compatibility, Providers, Money and credentials, Dependencies, Tests, Documentation, Boundaries, Conventions, Working in this repository [iris-release-bar §5].

| Convention | Where in `AGENTS.md` |
|---|---|
| Naming: crates, packages, kinds, IDs, extractor and linker names, commands, format tags, files, tests | Conventions, Naming |
| Error and warning codes: the registry, retry classes, engine-authored messages | Conventions, Error codes |
| Logging: stderr, the background logs under the data root, redaction | Conventions, Logging |
| Configuration: the workspace and user-level files, credential settings, defaults | Conventions, Configuration |
| Where new code goes: languages, formats, joins, questions, shapes, decision kinds, commands, views, pane keys, types, strings, helpers, recordings | Conventions, Where new code goes |
| Tests: offline and deterministic, live calls behind a flag, evidence paths for live sessions, when an entry turns `passing` | Tests |
| Working in this repository: the commands to run, the files never to edit, area contracts, carry notes, attribution | Working in this repository |
