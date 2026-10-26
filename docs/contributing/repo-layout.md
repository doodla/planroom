# Repo layout

This page is the map of the repository (R16): what every directory is for, each top-level area's contract, the boundary rules and how tooling enforces them in Rust and TypeScript, the generated directories, the one task entry point, toolchain pins, the codegen flow, and the conventions `AGENTS.md` holds. Changing the layout, a boundary or a convention needs a decision record in [Decisions](decisions.md) first, and `cargo xtask check layout` fails without one.

## Directory map

```text
planroom/
├── AGENTS.md                      invariants and conventions for agents and people; CLAUDE.md imports it
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
├── budgets.json                   GENERATED, reviewed like code: token and time budgets
├── .gitattributes                 generated paths marked linguist-generated
├── .claude-plugin/
│   └── marketplace.json           one plugin, "planroom", source "./plugin"
├── .claude/
│   ├── guardrails/                hook rules and tests (unchanged; run in CI)
│   ├── settings.json  carry.md
│   └── skills/                    project skills for building Planroom (added once a procedure repeats)
├── .github/
│   ├── workflows/                 ci.yml, release.yml, advisories.yml, toolchain.yml
│   ├── dependabot.yml             cargo, npm, github-actions; weekly, grouped
│   ├── ISSUE_TEMPLATE/            bug, feature, pack request
│   └── PULL_REQUEST_TEMPLATE.md
├── crates/
│   ├── contract/   planroom-contract   every public type; the error and warning registries; the command catalogue
│   ├── kernel/     planroom-kernel     pure, domain-free logic: fold, roll-up, query shapes, diff, impact,
│   │                                   reconcile, policy, lint, machine checks, the two loop tables; no I/O
│   ├── store/      planroom-store      SQLite: the ten tables, commit, tail, leases, presence, migrations
│   ├── extract/    planroom-extract    git CLI, file walk, formats, tree-sitter, the declarative runner,
│   │                                   built-in extractors and linkers, evidence verification
│   ├── models/     planroom-models     transport (live, record, replay); Jev codecs; Claude gateway and session
│   ├── planroom/   planroom            the binary: cli/, server/, live.rs, app/ (composition root), config/, text/
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
│   └── scripts/planroom  scripts/engine.env   the POSIX sh resolver and its pinned version
├── schema/                          GENERATED: planroom.v1.schema.json, planroom.v1.input.schema.json
├── examples/
│   ├── fixture/  build-fixture  check-fixture  TRUTH.json  README.md  kickoff-snapshot.md
│   └── packs/ruby/  packs/flags/    out-of-tree data packs for S10 (Ruby is also loaded by the fixture workspace)
├── tests/
│   ├── replay/jev/  replay/claude/  recorded responses keyed by request hash
│   ├── golden/                      GENERATED by the Rust tests: view models, CLI outputs, change specs
│   ├── e2e/                         @planroom/e2e (private): Playwright specs, shims/ (window.claude, fake Artifact db)
│   ├── installer/                   the resolver's offline suite
│   └── plugin/                      plugin validation, install smoke, surface-token and slash-command snapshots
├── scripts/                         package-release.sh, smoke-test-release.sh (POSIX sh, called by xtask and release CI)
└── docs/
    ├── README.md                    the docs index and the fact-to-home table
    ├── guides/  concepts/           how to do a task; how it works and why
    ├── reference/                   cli.md, errors.md, configuration.md, packs.md (GENERATED); json-output.md, http-api.md
    ├── contributing/                architecture, decisions, contracts, repo layout, traceability, design language,
    │                                style guide, adding a pack, live testing, releasing; research/ (Phase 1 notes)
    ├── evidence/sNN/                live runs, recorded once per scenario
    ├── screenshots/                 every main view at 1440×900 and 390×844, both themes
    └── art/                         Iris images and their prompts
```

## Area contracts

Each row is the content of that area's `AGENTS.md`. Areas that ship to users (packs and the plugin) use a `README.md` instead.

| Area | Owns | May depend on | Must not | How to test it |
|---|---|---|---|---|
| `crates/contract` | every public type, `ErrorCode`, `WarningCode`, `Command` and `CommandSpec` | `serde`, `serde_json`, `schemars`, `clap` (derive only), `uuid`, `indexmap` | any other workspace crate; I/O; logic beyond validation | `cargo xtask test -p planroom-contract` (schema snapshot, round trips) |
| `crates/kernel` | the fold, resolution, roll-up, the twelve query shapes and question pipelines, diff, impact, reconcile, policy bands, lint rule types, machine checks and step, budgets, next action, redaction, the decision and hand-off loop tables, roles | `contract`, plus the allow-list in `boundaries.toml` (`serde`, `serde_json`, `petgraph`, `similar`, `nucleo-matcher`, `sha2`, `thiserror`, `indexmap`, `regex` for redaction) | I/O, async, `rusqlite`, `reqwest`, `tree-sitter`; any pack kind ID as a string literal | unit and property tests (`rollup`, `cas`, `undo`, `query`, `reconcile`, `evals`); `cargo bench fold` |
| `crates/store` | the tables, `commit` (idempotency, compare-and-set, status by actor), `tail`, leases, presence, layouts, the ledger, migrations | `contract`, `rusqlite` | other workspace crates; business rules (it calls a kernel function passed in by `app`) | `concurrency.rs` (3 processes × 500 writes), `crash.rs`, `idem.rs`, `writer_interleavings` proptest, migration fixtures |
| `crates/extract` | the `Extractor` and `Linker` traits, built-ins, the declarative runner, evidence location and verification, the git CLI adapter | `contract`, `tree-sitter` and grammars, `ignore`, `globset`, `regex`, `saphyr`, `toml`, `pulldown-cmark`, `promql-parser` | other workspace crates; running anything from a scanned repo except `git` with hooks off | `contract.rs` (every extractor over the fixture and a fuzz corpus), `link.rs`, per-extractor fixtures, the ABI floor test |
| `crates/models` | `Transport` (live, record, replay), Jev codecs, the Claude gateway and session backends, retries, spend accounting | `contract`, `reqwest`, `tokio`, `sha2` | other workspace crates; any network call in tests without `PLANROOM_LIVE_*` | `jev_contract.rs` (both codecs over every fixture), `claude_decoders.rs`, `session_backend.rs`, `redact.rs` |
| `crates/planroom` | the CLI, the server, the live loop, app services (orchestration only), actor resolution, pack loading and embedding, config, the string catalog | every workspace crate, `axum`, `tower-http`, `tokio`, `tokio-stream`, `tracing`, `anyhow` | business rules that belong in the kernel; spelling an error code outside the registry | scenario tests `tests/sNN_*.rs`, `budgets.rs`, `actor.rs`, golden outputs |
| `crates/testkit` | the fixture wrapper (`examples/build-fixture` plus the workspace config), the truth comparator and its planted-name mapping, the S8 load harness and fault injection, golden helpers | `contract`, `assert_cmd`, `tempfile`, `serde_json` | being a normal dependency of any crate (dev-only) | its own unit tests; every scenario test uses it |
| `crates/xtask` | task commands | anything | shipping in a release | it runs in CI |
| `packs/*` | kinds, relations, hints, extractor and linker declarations, lenses, questions, rules, decision kinds, suggestions, prompts, stages, evals, pack tests | the kernel's vocabulary and the `code` pack | code or executable files; other packs except `code` | `tests/packs.rs` (validation, the inward-only rule); `planroom doctor --pack` on each; each pack's scenario tests |
| `packages/contract` | generated types, the catalogue, tool schemas | nothing | hand edits under `generated/` | `tsc`, drift check |
| `packages/ui` | design tokens (TS and CSS variables, light and dark), view-model rows, hotkey assignment, truncation, the braille raster, the machine stepper | `@planroom/contract` | React, the DOM, Node APIs | vitest; `machine_step_parity` against Rust golden outputs |
| `packages/canvas` | the canvas app, both transports, views, the ELK worker, motion | `@planroom/contract`, `@planroom/ui`, React, `@xyflow/react`, `elkjs`, `zustand`, `motion`, `radix-ui` | `@planroom/mod`; engine internals; fetching any host but its own origin | vitest, Playwright e2e |
| `packages/mod` | the mod: wiring, pane, band, `/room`, tools, guards, broker, agents | `@planroom/contract`, `@planroom/ui`, the `claude-code` mod types | `@planroom/canvas`; the DOM or Node APIs; `import()` | vitest (pure logic); `claude plugin test` on all four surfaces with golden CLI outputs |
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
store    = { may_depend = ["contract"] }
extract  = { may_depend = ["contract"] }
models   = { may_depend = ["contract"] }
planroom = { may_depend = ["contract", "kernel", "store", "extract", "models"] }
testkit  = { may_depend = ["contract"], dev_only = true }
xtask    = { may_depend = ["*"], ships = false }

[rust.kernel]
domain_free = true          # no built-in pack ID followed by "." in a string literal under crates/kernel/src
no_path_escapes = true      # no #[path = "../"] or include! leaving a crate's directory (every crate)

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
| The kernel depends on no domain | `planroom-kernel` depends only on `contract` and its external allow-list; no built-in pack kind ID in its string literals | `cargo xtask check boundaries` (crate graph and allow-list from `cargo metadata`; the literal scan) |
| Adapters depend inward, never sideways | `store`, `extract` and `models` depend on `contract` only | the same check |
| One composition root | only `planroom` (and `xtask`) depend on more than one workspace crate; `testkit` only as a dev-dependency | the same check |
| Packs depend inward | `requires` is `[]` or `["code"]`; every referenced kind belongs to the pack, `code` or the kernel; no executables | the pack loader and `crates/planroom/tests/packs.rs` |
| Surfaces reach the engine only through published contracts | `canvas` and `mod` import `@planroom/contract` and `@planroom/ui` only through their exports; `ui` has no React or DOM; the mod has no Node APIs | `depcruise packages tests/e2e --config .dependency-cruiser.cjs --output-type err` (allow-list, default deny); the mod's `tsconfig` `lib: ["es2023"]`, `types: []` |
| Generated code is never hand-edited | every file under a generated directory has the header and equals a fresh generation | `cargo xtask check` drift checks |
| No OpenSSL, no unknown sources | `deny.toml` bans `openssl` and `openssl-sys`, denies wildcards and unknown git sources | `cargo deny check` |

### Planted-import self-test

A planted bad import must fail CI (S15). `cargo xtask check boundaries --self-test` does the following:

1. Copies the repo to a temporary directory with `git worktree add --detach`.
2. Adds `planroom-store` to `crates/kernel/Cargo.toml` and a `use planroom_store as _;` line, runs the Rust check, and expects exit 1 with "kernel -> store is not allowed".
3. Adds `"code.service"` as a string literal in `crates/kernel/src/lib.rs`, runs the literal scan, and expects it to fail.
4. Adds `import "@planroom/mod"` to `packages/canvas/src/main.tsx`, runs depcruise, and expects a `not-in-allowed` error.
5. Adds `requires = ["observability"]` to a copy of the flags pack, runs the loader, and expects `pack_invalid`.
6. Removes the worktree.

CI runs the self-test on every push.

### Decision-record gate

`cargo xtask check layout` compares with the merge base. If the diff touches `boundaries.toml`, `[dependencies]` between workspace crates, the top-level directory list, a package's `exports`, or the "Conventions" section of `AGENTS.md`, the same diff must add a record to `docs/contributing/decisions.md`.

## Generated directories

Each has a `GENERATED.md` stub or a header line, and a drift check. Nobody edits them by hand; run the generator instead.

| Path | Generator | Committed? |
|---|---|---|
| `schema/` | `cargo xtask gen` (schemars) | yes |
| `packages/contract/src/generated/` | `cargo xtask gen` (json-schema-to-typescript, the catalogue) | yes |
| `plugin/types/index.d.ts` | `cargo xtask gen` | yes |
| `plugin/hooks/planroom-mod.js` | `cargo xtask gen --mod` (esbuild, ESM, no splitting, `claude-code` external; deterministic) | yes, because a local-path marketplace loads the plugin from the repo |
| `.dependency-cruiser.cjs` | `cargo xtask gen` from `boundaries.toml` | yes |
| `docs/reference/cli.md`, `errors.md` tables, `configuration.md`, `packs.md` tables | `cargo xtask gen` (`PLANROOM_UPDATE_DOCS=1` for the CLI reference) | yes |
| `tests/golden/` | the Rust tests with `PLANROOM_UPDATE_GOLDEN=1` | yes |
| `packs/*/evals/*.result.json` | `cargo xtask eval` | yes |
| `budgets.json` | `cargo xtask gen --budgets` | yes, reviewed like code |
| `target/canvas/` | `cargo xtask gen --canvas` (`vite build`) | no |

## The one task entry point

`cargo xtask` needs nothing beyond the pinned Rust and Node, because `just` isn't installed [DR-38]. Agents and CI use only these commands. Each never stops at the first failure, prints one `ok`, `FAILED` or `skipped (why)` line per check, and names the fix command on failure.

| Command | Runs | Typical time (warm) |
|---|---|---|
| `cargo xtask check` | `cargo fmt --check`; clippy with `-D warnings`; `prettier --check`; `eslint`; `tsc -b`; depcruise; the boundary rules and self-test; every drift check; the docs tests; release pins; the guardrails tests (`python3 .claude/guardrails/tests/test_guardrails.py` and `test_context_restore.py`); `examples/check-fixture`; `claude plugin validate --strict` on `.` and `plugin/`; the layout gate; the secret scan over `tests/replay/` and `docs/evidence/` | 2 to 3 min |
| `cargo xtask check boundaries [--self-test]` | the boundary rules alone, or with the planted crossings | 20 s |
| `cargo xtask test [--scenario SNN]` | `cargo nextest run` plus `cargo test --doc`; `vitest run`; `claude plugin test packages/mod`; the scenario CI paths S1 to S15 on fresh fixtures; budgets; eval recompute | 4 to 6 min |
| `cargo xtask e2e [--screens]` | builds the canvas; `planroom serve` on a built fixture with replay; Playwright at 1440×900 and 390×844, light and dark; the empty-state audit; reduced motion; console errors; the Artifact page with the fake `db`; the plugin install smoke in an isolated `HOME` | 4 min |
| `cargo xtask gen [--budgets] [--mod] [--canvas]` | every generator above | 30 s |
| `cargo xtask fixture OUT [--no-packs]` | `examples/build-fixture OUT`, then `OUT/.planroom/config.toml` loading `examples/packs/ruby` (omitted with `--no-packs`) | 2 s |
| `cargo xtask eval [--verify-order] [--report]` | recompute every eval result from replay; check label order with git; write the S7 table | 10 s |
| `cargo xtask release-dry-run [--target T]` | release toolchain check; release build; canvas build; package twice and `cmp`; smoke test on the built fixture; the resolver suite against a `127.0.0.1` server holding a locally built, checksummed archive | 6 min |
| `cargo xtask live <eval KIND \| record SNN \| evidence SNN>` | needs `PLANROOM_LIVE_JEV=1` or `PLANROOM_LIVE_GATEWAY=1`; spends against the `live_tests_total_usd` ledger; writes `tests/replay/` or `docs/evidence/` | varies |

## Toolchain pins

Each pin has one home and a `release_pins` test that fails when another file names a different version [iris-release-bar §7].

| Pin | Home |
|---|---|
| Rust 1.97.0 | `rust-toolchain.toml` |
| Node 22.22.0 | `.node-version` |
| pnpm 10.28.x, with `minimumReleaseAge: 10080` | `package.json` `packageManager`, `pnpm-workspace.yaml` |
| Claude Code 2.1.310 | an `xtask` constant, which CI installs |
| Python 3.11 or later (guardrails, `build-fixture`, `check-fixture`) | `xtask` check, CI setup |
| git 2.30 or later | `planroom doctor`, CI setup |
| `cargo-deny 0.20.2`, `cargo-nextest 0.9.149`, `cargo-about 0.9.2` | sha256-pinned downloads in `xtask` |
| `json-schema-to-typescript 16.0.0` | `packages/contract/package.json` |
| Playwright 1.63.0 | `tests/e2e/package.json`; matches the preinstalled browsers |
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
  README.md                          what it is, the first command, links
  hooks/hooks.json                   {"modules": ["./planroom-mod.js"]}
  hooks/planroom-mod.js              GENERATED
  types/index.d.ts                   GENERATED
  skills/room/SKILL.md               user-invocable: false
  skills/room-plan/SKILL.md          user-invocable: false
  skills/room-handoff/SKILL.md       user-invocable: false
  agents/implementer.md              model sonnet, effort high, tools Read, Edit, Write, Bash, Grep, Glob
  agents/curator.md                  model sonnet, effort medium, tools Read, Bash, Grep, Glob
  scripts/planroom                   the POSIX sh resolver
  scripts/engine.env                 PLANROOM_ENGINE_VERSION, PLANROOM_REPO (written at release)
```

No `bin/`, no `.mcp.json`, no symlinks, no classic hooks; `tests/plugin/` asserts each [DR-14, DR-28, DR-30].

## CI workflows

`.github/workflows/ci.yml` runs on `push` to every branch (`branches: ["**"]`), on pull requests and on `workflow_dispatch`. It cancels superseded runs per ref except on `main`, sets `permissions: contents: read` and `persist-credentials: false`, and pins every action by SHA. No job needs a secret or calls a live model.

| Job | Command | Runner |
|---|---|---|
| `check` | `cargo xtask check` | ubuntu-24.04 |
| `test (os)` | `cargo xtask test` | ubuntu-24.04, macos-15 |
| `e2e` | `cargo xtask e2e` | ubuntu-24.04 |
| `deny-and-audit` | `cargo deny check`, `pnpm audit --prod` | ubuntu-24.04 |
| `guardrails` | the two guardrails test files (also inside `check`, so a hook regression shows by name) | ubuntu-24.04 |
| `installer (os)` | ShellCheck, BusyBox, `sh tests/installer/run.sh` | ubuntu-24.04, macos-15 |
| `release-dry-run (target)` | `cargo xtask release-dry-run --target T` | x86_64 and aarch64 musl on ubuntu-24.04 and ubuntu-24.04-arm; both darwin targets on macos-15 |

Scheduled: `advisories.yml` (weekly `cargo deny` and `pnpm audit`) and `toolchain.yml` (weekly pin-age checks for Rust, Node and Claude Code). `release.yml` runs on a `v*` tag push or `workflow_dispatch` (a dry run by default); only its `publish` job has `contents: write`.

## Conventions

These go in `AGENTS.md` once, under "Conventions", and nowhere else. `AGENTS.md` follows Iris's structure: one opening paragraph ("invariants and steering rules, not a description of the code"), then flat bullet sections: Compatibility, Providers, Money and credentials, Dependencies, Tests, Documentation, Boundaries, Conventions, Working in this repository [iris-release-bar §5].

### Naming

- Crates are `planroom-<area>`; the binary is `planroom`; npm packages are `@planroom/<area>`.
- Kinds are `<pack>.<snake_case>`; the kernel's own are `kernel.group` and `kernel.external`.
- Node IDs are `<namespace>:<kind-short>:<key>`; edge IDs `e:<12 hex>`; plan items `plan:<project>:<key>`.
- Commands are lowercase verbs or nouns in the catalogue; CLI flags are long-form only.
- Format tags are `planroom.<kebab-name>/v<schema_version>`.
- Rust modules are `snake_case`; TypeScript files are kebab-case and React components PascalCase.
- Tests are named for the behavior they check (`a_rescan_keeps_the_regrouping`), scenario tests `sNN_<slug>.rs`.

### Error codes

- Add a variant to `contract::ErrorCode` with its category, exit code, retryable flag, `details` shape and docs row; regenerate `docs/reference/errors.md`.
- Never return an ad-hoc code string; a source scan fails on one outside the registry.
- Messages are one line, engine-authored, from `crates/planroom/text/en.toml`; hints are exact commands.
- Warnings use `contract::WarningCode` the same way.

### Logging

- `tracing` to stderr only, so `--json` stdout stays one document; `PLANROOM_LOG=info` by default.
- Never log prompts, slices, file contents, headers or the person key.
- Redact anything matching the secret patterns in `kernel::redact` before it reaches a log, an error `details` or a snapshot.

### Configuration

- One optional file, `.planroom/config.toml`; every key has a default in `crates/planroom/src/config/defaults.toml`.
- Environment overrides are `PLANROOM_*`; keys never hold secrets, only the names of environment variables.
- `docs/reference/configuration.md` is generated from the defaults file.
- A new setting needs a decision record saying why no default works (R14).

### Where new code goes

| You are adding | It goes in |
|---|---|
| a language with a built-in grammar | a `.scm` file and `[[extractors]]` in `packs/code`, emitting the existing hint kinds |
| any other language | a `regex` extractor in a pack, emitting the existing hint kinds |
| a file format that needs a parser | a module in `crates/extract`, plus a `builtin` declaration in a pack |
| a cross-repo join | a linker in `crates/extract/src/link/`, plus a `[[linkers]]` declaration |
| a question | `[[questions]]` in a pack, as a pipeline of existing shapes |
| a query shape | the kernel, with a decision record naming the two packs that need it |
| a decision kind | `[[decisions]]` in a pack, plus `evals/`; a kernel role only with a decision record |
| a command | a `Command` variant with its `CommandSpec`, and one handler in `crates/planroom/src/app/` |
| a canvas view | `packages/canvas/src/views/`, reached from one of the five concepts |
| a pane section or key | `packages/mod/src/pane.tsx` and the key map snapshot; hotkeys come from `@planroom/ui` |
| a public type | `crates/contract/src/`, then `cargo xtask gen` |
| a user-facing string | `crates/planroom/text/en.toml`, or a pack label |
| a test helper | `crates/testkit` (Rust) or `tests/e2e/shims/` (browser) |
| a recorded model response | `cargo xtask live record`, never by hand |

### Tests

- Tests are offline, free and deterministic: strict replay, the fixture built per run, the `patch` implementer, fake keys only.
- Live tests are opt-in behind `PLANROOM_LIVE_*` and never run in CI.
- Test observable behavior through the CLI `--json` or the HTTP API; compare with `TRUTH.json` through `planroom-testkit::truth`.
- A `feature_list.json` entry turns `passing` only when every test it lists passes on its CI path.

### Working in this repository

- Run `cargo xtask check` and `cargo xtask test` before claiming success, and say what was skipped.
- Never edit `KICKOFF.md`, `examples/kickoff-snapshot.md` or `TRUTH.json` (the last only through a decision record).
- Keep `.claude/carry.md` current and under 4,000 characters; keep `progress.md` as the full record.
- Commits follow the attribution rules in `CLAUDE.md`; never fabricate a git identity; never discard others' changes.
