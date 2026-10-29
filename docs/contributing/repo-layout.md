# Repo layout

This page is the map of the repository (R16): what every directory is for, each top-level area's contract, the boundary rules and how tooling enforces them in Rust and TypeScript, the generated directories, the one task entry point, toolchain pins, the codegen flow, and the conventions `AGENTS.md` holds. Changing the layout, a boundary or a convention needs a decision record in [Decisions](decisions.md) first, and `cargo xtask check layout` fails without one [DR-59].

## Directory map

```text
planroom/
├── AGENTS.md                      invariants and conventions for agents and people, the conventions' only home; CLAUDE.md imports it
├── CLAUDE.md                      "@AGENTS.md" on line 1, then the existing compaction-notes and subagent sections, unchanged
├── README.md  LICENSE              what Planroom is and the first command; the MIT license
├── CHANGELOG.md  CONTRIBUTING.md  SECURITY.md  CODE_OF_CONDUCT.md   (M5) the release notes, how to contribute,
│                                  the security scope and residual risks, and the code of conduct
├── KICKOFF.md                     the brief; never edited (S5 and S6 use examples/kickoff-snapshot.md)
├── progress.md                    the build record: phases, assumptions, pre-mortems, estimates, spend
├── Cargo.toml  Cargo.lock         the one Cargo workspace: crates/*
├── rust-toolchain.toml            channel 1.97.0, rustfmt and clippy, the four release targets
├── rustfmt.toml  deny.toml
├── package.json  pnpm-workspace.yaml  pnpm-lock.yaml  .npmrc   the one JS workspace: packages/*, tests/e2e
├── .node-version                  Node 22.22.0
├── tsconfig.base.json  eslint.config.js  .prettierrc.json
├── boundaries.toml                the single source of boundary rules for Rust, TypeScript and packs
├── .dependency-cruiser.cjs        GENERATED from boundaries.toml
├── feature_list.json              S1 to S15 (and tier C additions): tier, CI path, evidence path, tests, status
├── budgets.json                   hand-ratcheted token and time budgets; a commit that raises one adds a decision record [DR-59]
├── layout.lock                    the gated surfaces' sha256 and the record that approved each [DR-59]
├── .gitattributes                 generated paths marked linguist-generated, written from the Generated directories table [DR-79]
├── .cargo/config.toml             the cargo xtask alias
├── .claude-plugin/
│   └── marketplace.json           one plugin, "planroom", source "./plugin"
├── .claude/
│   ├── guardrails/                the agent hooks' runner and config (unchanged; run in CI)
│   ├── guardrails/rules/  guardrails/tests/   hook rules and their tests
│   ├── settings.json              env PLANROOM_REPLAY=strict for agents in this repo; cargo xtask sets the spend log's path [DR-49, DR-65];
│   │                              claudeMdExcludes over the fixture's agent files [A-16] [DR-70], and permissions.deny
│   │                              "Skill(run-worker-tests)", the fixture's skill [DR-79]
│   └── skills/**                  project skills for building Planroom, each a folder with SKILL.md, evals and scripts
│                                  (added once a procedure repeats)
├── .github/
│   ├── workflows/ci.yml           every job's one cargo xtask command
│   ├── workflows/release.yml  workflows/advisories.yml  workflows/toolchain.yml   (M5) the release, the weekly audit
│   │                              and the weekly pin-age checks
│   ├── dependabot.yml             cargo, npm, github-actions; weekly, grouped, with a 14-day cooldown, the direct-pin age [DR-70]
│   ├── ISSUE_TEMPLATE/            (M5) bug (it asks for the report file `planroom doctor --json` names), feature, pack request
│   └── PULL_REQUEST_TEMPLATE.md   (M5)
├── crates/
│   ├── contract/   planroom-contract   every public type; the error and warning registries; the command catalogue
│   ├── kernel/     planroom-kernel     pure, domain-free logic: fold, roll-up, query shapes, diff, impact,
│   │                                   reconcile, policy, lint, machine checks, the two loop tables; no I/O
│   ├── store/      planroom-store      SQLite: the ten tables, commit, tail, leases, presence, migrations; the user-level
│   │                                   ledger, event backups and pre-migration copies under the data root
│   ├── extract/    planroom-extract    git CLI, file walk, formats, tree-sitter, the declarative runner,
│   │                                   built-in extractors and linkers, evidence verification
│   ├── models/     planroom-models     transport (live, record, replay); Jev codecs; Claude gateway and session
│   ├── planroom/   planroom            the binary: cli.rs, server/, live.rs, app/ (composition root, actor and trust), config/, text/;
│   │                                   build.rs embeds the packs and, when built, target/canvas/
│   ├── testkit/    planroom-testkit    dev-only: fixture builder wrapper, truth comparator, S8 load and faults, golden helpers
│   └── xtask/      xtask               the one task entry point (publish = false)
├── packs/                          data only, embedded in the binary at build time; AGENTS.md and CLAUDE.md at this level only
│   ├── code/                       the base pack every other pack builds on
│   ├── plan/                       the plan pack: stages, their item types and methods, structural lint; decision kinds in M2
│   ├── observability/  release/    (M3) the observability and release packs
│   └── artifacts/  machines/       (M4) the artifacts and machines packs
├── packages/
│   ├── contract/   the @planroom/contract package: src/generated/ (GENERATED) and src/index.ts (re-exports only)
│   ├── ui/         the @planroom/ui package: design tokens, view-model rows, hotkeys, braille raster, machine stepper;
│   │               no React, no DOM, no Node
│   ├── canvas/     the @planroom/canvas package: the Vite app; builds to target/canvas/
│   └── mod/        the @planroom/mod package: mod source and tests; builds to plugin/hooks/planroom-mod.js;
│                   vendor/claude-code.d.ts, the mod API types of the pinned Claude Code [DR-81]
├── plugin/                          the shipped plugin; never holds bin/ or symlinks
│   ├── .claude-plugin/plugin.json   .claude-plugin/icon.png
│   ├── README.md  AGENTS.md         README.md is for users; AGENTS.md holds the area contract, with no CLAUDE.md [DR-70, DR-81]
│   ├── hooks/                       GENERATED, the whole directory, with its GENERATED.md [DR-79]: hooks.json,
│   │                                {"modules": ["./planroom-mod.js"]}, and planroom-mod.js, the bundle of packages/mod
│   ├── types/index.d.ts             GENERATED $.state contract; the directory holds it and its GENERATED.md, nothing else
│   ├── skills/planroom/  skills/room-plan/  skills/room-handoff/    SKILL.md each, user-invocable: false
│   ├── agents/implementer.md  agents/curator.md
│   └── scripts/planroom  scripts/engine.env   the POSIX sh resolver (with prefetch, build and status) and its pinned version,
│                                    which cargo xtask gen writes whole [DR-69, DR-79]; scripts/contract.sh, GENERATED:
│                                    the codes and output templates the resolver sources [DR-69]
├── schema/                          GENERATED: planroom.v1.schema.json, planroom.v1.input.schema.json; baseline/, the breaking-change guard's copy
├── examples/
│   ├── fixture/  build-fixture  check-fixture  TRUTH.json  README.md  kickoff-snapshot.md
│   ├── packs/ruby/                 an out-of-tree data pack for S10, also loaded by the fixture workspace
│   └── packs/flags/                (M5) an out-of-tree data pack for S10
├── tests/
│   ├── replay/                      recorded responses keyed by request hash
│   ├── replay/claude/               (M1) recorded Claude calls
│   ├── replay/jev/                  (M2) recorded Jev calls
│   ├── golden/                      GENERATED by cargo xtask gen --golden: view models, CLI outputs, change specs, which the
│   │                                Rust tests compare against [DR-79] and only golden_readers, the mod's tests and the
│   │                                e2e stand-in, read [DR-81]
│   ├── e2e/                         @planroom/e2e (private): Playwright specs, shims/ (window.claude, fake Artifact db)
│   ├── installer/                   the resolver's offline suite
│   └── plugin/                      (M1) plugin validation, install smoke, surface-token and slash-command snapshots
├── scripts/package-release.sh  scripts/smoke-test-release.sh  scripts/check-tag-version.sh   (M5) the release helpers,
│                                    POSIX sh, called by xtask and release CI
└── docs/
    ├── README.md                    (M5) the docs index and the fact-to-home table
    ├── guides/  concepts/           (M5) how to do a task; how it works and why
    ├── reference/errors.md  reference/configuration.md   GENERATED reference pages
    ├── reference/cli.md  reference/packs.md  reference/json-output.md  reference/http-api.md   (M5) the CLI reference and
    │                                the packs' marked tables (GENERATED), the JSON output and the HTTP API
    ├── contributing/                architecture, decisions, contracts, repo layout, traceability, design language;
    │                                research/ (Phase 1 notes and the reviews' records)
    ├── contributing/style-guide.md  contributing/adding-a-pack.md   (M5) the style guide; adding a pack
    ├── contributing/live-testing.md  contributing/releasing.md      (M5) live testing; releasing
    ├── evidence/sNN/                live runs, recorded once per scenario
    ├── evidence/spend.jsonl         every live call of the build, appended by the engine through one absolute path from every worktree; the $3 cap reads it
    ├── screenshots/                 every main view at 1440×900 and 390×844, both themes
    └── art/                         Iris images, their prompts, and spend.jsonl with one line per Iris run [DR-69]
```

## Area contracts

Each row is the source of that area's contract block: `cargo xtask gen` writes the block into the area's `AGENTS.md`, between BEGIN and END GENERATED markers, and `area_contracts_match_repo_layout` fails when they differ. A one-line `CLAUDE.md` holding `@AGENTS.md` sits beside each area's `AGENTS.md`, so Claude Code loads the contract when an agent reads files in that area. `plugin/` is the exception: `claude plugin validate --strict` warns on a `CLAUDE.md` in a plugin, so it carries `AGENTS.md` alone [DR-81]. `plugin/` and `packs/` keep a user-facing `README.md` beside their `AGENTS.md`, and the plugin ships both files, which nothing loads from an installed plugin. Every tracked top-level directory that isn't a dot directory, and each `tests/*` directory, has a row, and `cargo xtask check` fails when one lacks a row or either file, or when `plugin/` holds a `CLAUDE.md` [DR-59, DR-70, DR-81]. The checks read the directories `git ls-files` holds, so `target/` and `node_modules/` never count. `crates/`, `packages/` and `tests/` are containers whose children are the areas [DR-79].

Below the areas, each area's `AGENTS.md` holds a hand-written `## Layout` list, outside its generated block, with one line per subdirectory saying what goes there. `every_directory_has_a_purpose`, a `cargo xtask check` step, fails on a tracked directory that neither the Directory map nor its area's Layout list names, except under `examples/fixture/` and the directory rows of [Generated directories](#generated-directories), each with its subtree; a dot directory has no area, so only the map names it [DR-79]. Its rules:

- **What names a directory.** On the map, only a line with a `├── ` or `└── ` connector carries names: its depth is the number of 4-column glyph groups before the connector, the rest splits on runs of two or more spaces, and the names are the leading tokens that hold a `/` or a `.` and no space, each joining the path of the line above it at one depth less. In a Layout list, an item's leading code spans before its first `:` are paths relative to the area. An entry names a directory when it is that directory, a path below it (`src/views/` names `src/`), or an ancestor written with a trailing `**`. `**` stands only where a tree's layout is data: `docs/contributing/research/**`, `packs/code/tests/**` and `.claude/skills/**`. A single `*` names no directory.
- **Placeholders.** A segment written `<name>` matches any one segment, and `sNN` matches `s` and two digits. A placeholder entry exists when a tracked path matches it.
- **Milestone tags.** A map or Layout line that names a path that doesn't exist carries the milestone that builds it, `(Mn)`, and a line whose paths all exist carries none, as a test that resolves carries no `due`; so the change that creates a path drops its tag. A row of the table below whose area doesn't exist ends its Owns cell with `(Mn)`, and an existing area's row carries no tag there.
- **Layout lists.** An area with a tracked subdirectory has a `## Layout` list.

The "May depend on" cells of the crate and package rows list exactly their allow-lists in `boundaries.toml`, which `area_contracts_match_repo_layout` checks, so the prose can't drift from the rule. Each "How to test it" cell starts with the exact `cargo xtask` command that runs the area's tests, then names files and commands only, a file outside the area by its full path from the repo root: a named `.rs`, `.ts`, `.tsx` or `.sh` file must resolve from the root or under the area unless `(Mn)` follows it, and a test function's name, a backticked snake_case name alone or after its file as in `truth.rs::name`, fails, since planned tests live in [Traceability](traceability.md) and `feature_list.json`.

| Area | Owns | May depend on | Must not | How to test it |
|---|---|---|---|---|
| `crates/contract` | every public type, `ErrorCode`, `WarningCode`, `Command` and `CommandSpec` | its allow-list in `boundaries.toml`: `serde`, `serde_json`, `schemars`, `clap` (derive only), `uuid`, `indexmap` [DR-79] | any other workspace crate or third-party crate; I/O, which its generated `clippy.toml` refuses as the kernel's does [DR-79]; logic beyond validation; any built-in pack kind or relation name as a string literal, with or without its pack prefix, or, outside tests, a built-in pack's level ID, since the kernel reads every constant here | `cargo xtask test -p planroom-contract` (`tests/schema.rs`: the schema snapshot and round trips) |
| `crates/kernel` | the fold, resolution, roll-up, the twelve query shapes and question pipelines, diff, impact, reconcile, policy bands, lint rule types, machine checks and step, budgets, next action, redaction by field, the display-sanitizing pass, the decision and hand-off loop tables, roles | `contract`, plus the allow-list in `boundaries.toml` (`serde`, `serde_json`, `petgraph`, `similar`, `nucleo-matcher`, `sha2`, `thiserror`, `indexmap`, `regex` for redaction) | I/O in any target, tests and benches included, async, `rusqlite`, `reqwest`, `tree-sitter`; any built-in pack kind or relation name as a string literal, with or without its pack prefix, or, outside tests, a built-in pack's level ID | `cargo xtask test -p planroom-kernel` (unit and property tests on data passed in as values: `rollup.rs`, `cas.rs`, `undo.rs`, `query.rs`, `reconcile.rs` (M2)); `cargo xtask bench fold` (`benches/fold.rs`) |
| `crates/store` | the tables, `commit` (idempotency, compare-and-set by writer instance, status by actor, a person's field held against automatic writes, the store-version check [DR-73]), the per-blob fact cache, the filesystem check and file modes, `tail` on its own read-only connection, leases, presence, layouts, migrations with their `VACUUM INTO` copies; the user-level ledger and event backups under the data root | `contract`, and its allow-list in `boundaries.toml`: `rusqlite`, `serde`, `serde_json`, `sha2`, `thiserror` [DR-79] | other workspace crates; any other third-party crate; opening a socket [DR-79]; business rules (it calls a kernel function passed in by `app`) | `cargo xtask test -p planroom-store` (`concurrency.rs`, 3 processes × 500 writes; `crash.rs`, `idem.rs`, `commit.rs`, `fs_check.rs`, `ledger.rs` (M1), the writer-interleavings proptest in `interleavings.rs`, and the migration fixtures in `migrations.rs`) |
| `crates/extract` | the `Extractor` and `Linker` traits, built-ins, the declarative runner, evidence location and verification, the git CLI adapter | `contract`, and its allow-list in `boundaries.toml`: `tree-sitter` and grammars, `ignore`, `globset`, `regex`, `saphyr-parser`, `toml`, `pulldown-cmark`, `promql-parser`, `serde`, `serde_json`, `sha2`, `thiserror` [DR-79] | other workspace crates; any other third-party crate; opening a socket [DR-79]; running anything from a scanned repo except `git` with hooks, external diffs and textconv off; a checkout or merge in a repo whose local git config names a command [DR-64] | `cargo xtask test -p planroom-extract` (`contract.rs`, every extractor over the fixture and a fuzz corpus; `formats.rs`, the manifest and OpenAPI built-ins and each language's queries; `git.rs`, the git history; `link.rs`, the built-in linkers) |
| `crates/models` | `Transport` (live, record, replay), Jev codecs, the Claude gateway and session backends, retries, spend accounting, the live gate and the host allow-list | `contract`, and its allow-list in `boundaries.toml`: `reqwest`, `rustls`, `tokio`, `sha2`, `serde`, `serde_json`, `thiserror` [DR-79] | other workspace crates; any network call without `PLANROOM_LIVE_*` or a consented credential; treating an ambient proxy as a credential | `cargo xtask test -p planroom-models` (`claude_decoders.rs`, `redact.rs`, `transport.rs`, `replay.rs`, `hosts.rs`; `jev_contract.rs` (M2), both codecs over every fixture; `session_backend.rs` (M2)) |
| `crates/planroom` | the CLI, the server, the live loop, app services (orchestration only), actor resolution, workspace trust, consent cards, pack loading and embedding, canvas embedding, config (workspace and user-level), the string catalog, the daemonizing of `serve --detach` | every workspace crate, and its allow-list in `boundaries.toml`: `axum`, `tower-http`, `tokio`, `tokio-stream`, `tracing`, `anyhow`, `clap`, `serde`, `serde_json`, `toml`, `uuid`, `libc` (fork, `setsid`, `statfs` and `geteuid` only) [DR-69], `p256` [DR-72] [DR-79] | business rules that belong in the kernel; spelling an error code outside the registry; reading a credential setting from a workspace's config | `cargo xtask test -p planroom` (the scenario tests `tests/sNN_*.rs`, `budgets.rs`, `actor.rs`, `trust.rs`, `privacy.rs`, `live_gate.rs`, `backup.rs`, `git_hardening.rs`, `golden.rs` against the golden outputs, `handoff_clone.rs` (M2), and `evals.rs` (M2), the kernel's eval recompute over pack files) |
| `crates/testkit` | the fixture wrapper (`examples/build-fixture` plus the workspace config), the truth comparator and its planted-name mapping, the S8 load harness and fault injection, golden helpers | `contract`, `assert_cmd`, `tempfile`, `serde_json` | being a normal dependency of any crate (dev-only) | `cargo xtask test -p planroom-testkit` (its own tests, `truth.rs`); every scenario test uses it |
| `crates/xtask` | task commands | anything | shipping in a release | `cargo xtask test -p xtask` (`gates.rs`, `docs.rs`, `skips.rs`, `schema_guard.rs`, `bench.rs`); CI runs its commands |
| `packs/*` | kinds, relations, hints, languages, image roles, extractor and linker declarations, lenses, questions, rules, decision kinds, suggestions, prompts, stages, evals, pack tests | the kernel's vocabulary and the `code` pack | code or executable files; other packs except `code` (an out-of-tree pack may also require any built-in pack) | `cargo xtask test -p planroom` (`crates/planroom/tests/packs.rs`: validation, the inward-only rule); `planroom doctor --pack` on each; each pack's scenario tests |
| `packages/contract` | generated types, the catalogue, tool schemas | nothing | hand edits under `generated/` | `cargo xtask check` (`tsc -b`, and its drift step) |
| `packages/ui` | design tokens (TS and CSS variables, light and dark), view-model rows, hotkey and row-key assignment, truncation, the display-sanitizing pass [DR-64], the braille raster, the machine stepper | `@planroom/contract`; no third-party package | React, the DOM, Node APIs, directly or through a third-party package | `cargo xtask test --ts packages/ui` (vitest: the machine stepper; the display-sanitizing pass, whose parity with the engine's is one table repeated in both ports' tests; the tokens' contrast) |
| `packages/canvas` | the canvas app, both transports, views, hand edits and the keyboard model, the ELK worker, motion | `@planroom/contract`, `@planroom/ui`, and its `third_party` list: React, `@xyflow/react`, `elkjs`, `zustand`, `motion`, `radix-ui`, the `@fontsource` faces | `@planroom/mod`; engine internals; fetching any host but its own origin | `cargo xtask test --ts packages/canvas` (vitest); `cargo xtask e2e` (Playwright) |
| `packages/mod` | the mod: the host check [DR-76], wiring, pane, band, fallback panes, `/room`, tools, action tokens, guards (touch set, stores, the user-level config and the mod's own `$.store`, the plugin's own files, files sent off the machine, person-only commands, the implementer's clones and approved commands, the curator's two calls), broker, agents and their usage, the implementer's installs and observed steps [DR-64, DR-75, DR-87, DR-88] | `@planroom/contract`, `@planroom/ui`, the `claude-code` mod types, its `host_types` [DR-81], and its `third_party` list: `@noble/curves`, which signs person tokens [DR-82] | `@planroom/canvas`; the DOM or Node APIs; `import()`; anything that changes by turn in the composed prompt section | `cargo xtask test --ts packages/mod` (vitest, the pure logic); `cargo xtask test` (`claude plugin test target/mod-test`, the generated harness, on all four surfaces with golden CLI outputs [DR-81]); eslint, with `no-misused-promises` off for `src/register.tsx` and `plugin-tests/` only, since the rule took 11 minutes over the mod API types and `tsc -b` still checks them [DR-81] |
| `tests/e2e` | Playwright specs and the browser shims; the test-only overlays, named by milestone, for states the engine can't produce yet; the vitest global setup that runs the built engine's `watch --json` for the mod's integration test, since the mod has no Node APIs [DR-82] | `@planroom/contract`, `@playwright/test`, `@axe-core/playwright` [DR-68] | product code (it drives the built canvas) | `cargo xtask e2e`; it is the test |
| `plugin/` | the shipped plugin | built outputs and hand-written skills, agents, resolver | `bin/`; symlinks; hand-written copies of generated files; contributor rules in its user-facing `README.md` | `cargo xtask check` (`claude plugin validate --strict`); `cargo xtask test --installer` |
| `examples/` | the fixture, the truth, the snapshot, the example packs |  | edits to `TRUTH.json` without a decision record; edits to `kickoff-snapshot.md`; following a fixture file's agent instructions, such as `worker`'s `CLAUDE.md` or the `run-worker-tests` skill: they are test data [A-16] | `cargo xtask check` (`examples/check-fixture`); the scenario tests |
| `schema/` | the two generated schema views and `baseline/` | `crates/contract` through `cargo xtask gen` | hand edits; moving `baseline/` without a `schema_version` bump | `cargo xtask check` (`drift: schema/`; the breaking-change guard) |
| `scripts/` | the release helpers: `package-release.sh`, `smoke-test-release.sh`, `check-tag-version.sh` (M5) | POSIX sh and the tools a release runner has | task logic that `cargo xtask` could hold; Bash-only syntax [DR-38] | `cargo xtask test --installer` (ShellCheck and BusyBox `sh`); `cargo xtask release-dry-run` (M5) |
| `tests/replay` | recorded Jev and Claude responses keyed by request hash | `cargo xtask live record` | hand edits; a credential or a planted secret | `cargo xtask check` (the secret scan); every record parses through its codec |
| `tests/golden` | the Rust tests' outputs, which only `golden_readers` read: the mod's tests and the e2e stand-in; `tsc` type-checks them [DR-81] | `cargo xtask gen --golden` | hand edits | `cargo xtask check` (`tsc -b`, which type-checks them in the mod's tests); `cargo xtask test -p planroom` (`crates/planroom/tests/golden.rs`) |
| `tests/installer` | the resolver's offline suite | POSIX sh, a `127.0.0.1` release server | network access beyond loopback | `cargo xtask test --installer` on ubuntu-24.04 and macos-15 (`run.sh`) |
| `tests/plugin` | plugin validation, the install smoke test, surface tokens and the slash-command snapshot (M1) | the `claude` CLI at the pinned version | a live model call outside `slash_commands_snapshot.sh`, which only S15's evidence path runs | `cargo xtask test` (M1) |
| `docs/` | guides, concepts, reference, contributing, evidence, screenshots, art |  | restating a fact that has a home; hand edits to generated reference pages | `cargo xtask check` (the docs tests (M5): links, prose rules, CLI reference) |

### Read first, per area

`cargo xtask gen` also writes each area's row below into its `AGENTS.md` as "Read first", so an agent resuming after a compaction reads the sections that govern its area instead of whole design pages. Section names are headings in [Architecture](architecture.md) unless another page is named [DR-70]. `gen` writes each as a link, resolving the name against the page's headings, and fails on a name that matches none [DR-79]. The sections state the current rule. A record gives the reason, and an "Amended in review round N" paragraph overrides the record's bullets above it, as [How to read a record](decisions.md#how-to-read-a-record) says [DR-79].

| Area | Sections | Records | Scenarios |
|---|---|---|---|
| `crates/contract` | [Contracts](contracts.md); One command catalogue | DR-14, DR-21, DR-37 | S15 |
| `crates/kernel` | Data and event model; The fold; Levels and roll-ups; One diff engine; The reconciler (R7); The hand-off loop; Policy bands | DR-2, DR-3, DR-44, DR-45, DR-61, DR-71, DR-73, DR-74, DR-80 | S1, S4, S7 |
| `crates/store` | Store schema; Writes: actors, base, conflicts and retries; Duties | DR-1, DR-5, DR-7, DR-8, DR-53, DR-73, DR-78 | S8 |
| `crates/extract` | Packs and extractors; The extractor contract; Hints and linkers; Adapters | DR-12, DR-16, DR-46, DR-47, DR-74, DR-78 | S1, S2, S4, S10 |
| `crates/models` | Model adapters; Spend guards; What leaves the machine | DR-18, DR-41, DR-49, DR-65, DR-75 | S7 |
| `crates/planroom` | Surfaces; Local server and live stream; Actors; Where the workspace lives; Packaging and release | DR-4, DR-29, DR-58, DR-64, DR-69, DR-74, DR-75, DR-77, DR-78, DR-80 | S1 to S15 |
| `crates/testkit` | Testing strategy | DR-12, DR-32 | S1, S8 |
| `crates/xtask` | [The one task entry point](#the-one-task-entry-point); [Decision-record gate](#decision-record-gate) | DR-38, DR-59, DR-70, DR-79 | S14, S15 |
| `packs/*` | What a pack is; The v0.1 packs; Lenses and questions; A decision kind | DR-9, DR-15, DR-46, DR-62, DR-63 | S2, S10 |
| `packages/contract` | [Codegen flow](contracts.md#codegen-flow) | DR-37 | S15 |
| `packages/ui` | How all three render the same output; [Design language](design-language.md) | DR-21, DR-64, DR-77 | S9, S13 |
| `packages/canvas` | The canvas; Remote mode through the Artifact (S11); [Design language](design-language.md) | DR-56, DR-68, DR-75, DR-77 | S4, S11, S13 |
| `packages/mod` | The mod; Actors | DR-6, DR-20, DR-54, DR-57, DR-66, DR-75, DR-76, DR-77, DR-82 | S9 |
| `tests/e2e` | Testing strategy; The canvas | DR-43, DR-68 | S11, S13, S15 |
| `plugin/` | Packaging and release; [Plugin layout](#plugin-layout) | DR-28, DR-58, DR-69, DR-78, DR-79 | S14 |
| `examples/` | `examples/README.md` | DR-13, DR-79 | S1 to S15 |
| `schema/` | [Versioning rules](contracts.md#versioning-rules) | DR-37, DR-59 | S15 |
| `scripts/` | Packaging and release | DR-38, DR-69, DR-78 | S14 |
| `tests/replay`, `tests/golden` | Testing strategy | DR-6, DR-49, DR-79 | S7, S9 |
| `tests/installer`, `tests/plugin` | Packaging and release | DR-58, DR-69, DR-78 | S14, S15 |
| `docs/` | `docs/contributing/research/iris-release-bar.md` §4 | DR-58 | S14 |

## Boundaries and their enforcement

`boundaries.toml` is the single source. `cargo xtask check boundaries` reads it with `cargo metadata`, and `cargo xtask gen` writes `.dependency-cruiser.cjs` from it, so the two languages' rules can't drift apart [DR-36].

The file's tables, each read in [`boundaries.toml`](../../boundaries.toml) itself, since this page holds no copy of it:

- [`[rust.crates]`](../../boundaries.toml) gives each workspace crate the workspace crates it may depend on (`may_depend`), the third-party crates it may use for normal and build dependencies (`external_allow`), whether it may open a socket (`net`, only `models` and `planroom`), and whether it ships [DR-79].
- [`[rust.kernel]`](../../boundaries.toml) keeps the kernel and the contract domain-free (`domain_free`, with the bare words `kernel_vocabulary` allows and the level IDs `level_words` lets one file spell for another meaning), forbids `#[path]` and `include!` escapes from a crate (`no_path_escapes`), and lists the I/O both crates may not reach (`no_io_crates`, `no_io`), which `cargo xtask gen` expands into their `clippy.toml` files [DR-45, DR-70, DR-79].
- [`[ts.packages]`](../../boundaries.toml) gives each TypeScript package the workspace packages it may import (`may_import`), its third-party packages (`third_party`), what it must not use (`forbid`), and, for the mod alone, the host's types (`host_types`) [DR-70, DR-81].
- [`[ts.rules]`](../../boundaries.toml) holds the rules across packages: imports only through `exports`, no cycles, no unresolvable import, `generated_read_only`, which points at the [Generated directories](#generated-directories) table, `golden_readers`, the only files that read `tests/golden/`, the domain-free scan's packages (`domain_free`) with the level IDs `level_words` lets one file spell for another meaning, and the test tooling only tests and configs import [DR-79, DR-81, DR-83].
- [`[packs]`](../../boundaries.toml) holds the pack rules: a built-in pack requires `code` or nothing, an out-of-tree pack only built-in packs, and no pack holds an executable [DR-9, DR-62].

| Boundary | Rule | Enforced by |
|---|---|---|
| The kernel depends on no domain | `planroom-kernel` depends only on `contract` and its external allow-list; no built-in pack kind or relation name with its prefix, and no bare one outside `kernel_vocabulary`, in the string literals of the kernel or of `planroom-contract`, so a constant can't carry a domain name into the kernel; outside `#[cfg(test)]` items, no built-in pack's level ID in them but where `level_words` lists the file, so the engine reads levels from the vocabulary; prop names reach machine checks as shape parameters [DR-45] | `cargo xtask check boundaries` (crate graph and allow-list from `cargo metadata`; the literal scan over both crates) |
| Surfaces name no domain | no string literal under `packages/ui`, `canvas`, `mod` or `contract` holds a built-in kind with its `<pack>.` prefix; outside test files, none equals a built-in pack's level ID but where `level_words` lists the file; a block's color comes from `MapNode.tone` and the levels from the view model's `levels` [DR-46] | the same literal scan over the four packages' `src/`, the generated contract included |
| The kernel does no I/O | no function or type from `std::fs`, `std::net`, `std::process` or `std::env`, no I/O method on `Path`, no `std::io` standard stream, no `SystemTime::now` and no print macro in any kernel target, tests and benches included [DR-70]. `planroom-contract`, the kernel's one workspace dependency, holds the same rule, so a contract helper can't carry I/O into the kernel [DR-79] | clippy's `disallowed-methods`, `disallowed-types` and `disallowed-macros` from `crates/kernel/clippy.toml` and `crates/contract/clippy.toml`, which `cargo xtask gen` expands from `no_io`, under `-D warnings` |
| Adapters depend inward, never sideways | `store`, `extract` and `models` depend on `contract` only | the same check |
| Only the model adapter and the server open sockets | of the crates that ship, only `models` and `planroom` may open a socket, so every model or network call goes through `models`' live gate and host allow-list; every shipped crate depends only on the third-party crates its `external_allow` lists [DR-79] | the same check, with each crate's `external_allow` read from `cargo metadata` for normal and build dependencies; clippy's `disallowed-types` for `std::net` from each other shipped crate's generated `clippy.toml` |
| One composition root | only `planroom` (and `xtask`) depend on more than one workspace crate; `testkit` only as a dev-dependency | the same check |
| Packs depend inward | `code` is the base ring every domain pack attaches to, so a built-in pack's `requires` is `[]` or `["code"]`; the built-in packs are inward of every out-of-tree pack, which may require any of them but no other out-of-tree pack; every referenced kind belongs to the pack, a pack it requires, or the kernel; no executables [DR-9, DR-62] | the pack loader and `crates/planroom/tests/packs.rs` |
| Surfaces reach the engine only through published contracts | `canvas` and `mod` import `@planroom/contract` and `@planroom/ui` only through their exports; each package imports only the third-party packages its `third_party` lists, so `ui` reaches no React or DOM even through another package; the mod has no Node APIs [DR-70] | `depcruise packages tests/e2e --config .dependency-cruiser.cjs --output-type err` (allow-list, default deny, one allow rule per listed package); the mod's `tsconfig` `lib: ["es2023"]`, `types: []` |
| Only the mod reaches the host API and the golden outputs | only `packages/mod` imports `claude-code` and `claude-code/testing`, the mod API types, which the bundle keeps external; only `golden_readers`, the mod's tests and the e2e stand-in, read `tests/golden/*.json` [DR-81] | depcruise rules `cargo xtask gen` writes from `host_types` and `golden_readers`; `not-to-unresolvable` exempts only the host module, which no package installs; the literal scan in `cargo xtask check boundaries`, which fails a string or template literal in a script (`.ts`, `.tsx`, `.mts`, `.cts`, `.js`, `.jsx`, `.mjs` or `.cjs`) under `packages/` or `tests/` that `golden_readers` doesn't name when it holds `tests/golden` or is a relative path that resolves into it from the script's directory, so a runtime read fails as an import does. It doesn't catch a path split across several literals |
| Generated code is never hand-edited | a generated directory carries a `GENERATED.md` stub, a generated file in a hand-written directory carries the header, and a generated region carries BEGIN and END GENERATED markers; each equals a fresh generation | `cargo xtask check`'s `generated_headers`, over every path the [Generated directories](#generated-directories) table names, and its drift checks |
| No OpenSSL, no unknown sources | `deny.toml` bans `openssl` and `openssl-sys`, denies wildcards and unknown git sources, and allows the licenses DR-39 names, BSD-3-Clause among them; each workspace path dependency names `version = "=X.Y.Z[-dev]"`, so it isn't a wildcard and the crates stay publishable [DR-83] | `cargo deny check licenses bans sources` inside `cargo xtask check` |

### Planted-import self-test

A planted bad import must fail CI (S15), and a broken checker must fail the self-test [DR-59]. `cargo xtask check boundaries --self-test` does the following:

1. Copies the repo to a temporary directory with `git worktree add --detach`, then runs `pnpm install --frozen-lockfile --offline` in the copy, from the store the main checkout filled.
2. Runs every boundary check on the unmodified copy and requires exit 0. This control proves the copy resolves, so a later failure is the plant's.
3. Adds `planroom-store` to `crates/kernel/Cargo.toml` and a `use planroom_store as _;` line, runs the Rust check, and expects exit 1 with "kernel -> store is not allowed". Then it plants, one at a time, `planroom-models` in `crates/extract/Cargo.toml` (a sideways crossing), `ureq` in `crates/extract/Cargo.toml` (a network client off the allow-list), and `planroom-testkit` as a normal dependency of `crates/store`, and expects each to fail by its rule's name [DR-79].
4. Adds `"code.service"`, then the bare relation name `"watches"`, as string literals in `crates/kernel/src/lib.rs`, runs the literal scan, and expects each to fail. A bare `"person"`, which `kernel_vocabulary` holds, must pass, and `"code.service"` in `packages/ui/src/planted.ts` must fail [DR-45]. Then adds a `std::fs::read_to_string` call to the kernel's `lib.rs`, then a `std::path::Path::new("x").exists()` call, runs clippy on the kernel after each, and expects the disallowed-method error both times [DR-70]. Next, it adds `pub const SERVICE: &str = "code.service";` to `crates/contract/src/lib.rs`, used from the kernel, and expects the literal scan to fail on the contract file. Last, it adds a `pub fn read(p: &str) -> String` wrapping `std::fs::read_to_string` to `crates/contract/src/lib.rs`, then a `std::net::TcpStream::connect` call to `crates/store/src/lib.rs`, runs clippy on each crate, and expects the disallowed-type or disallowed-method error both times [DR-79].
5. Adds `"@planroom/mod": "workspace:*"` to `packages/canvas/package.json`, links it offline, and writes `packages/canvas/src/planted.ts` importing `@planroom/mod`. Then it replaces that import with a relative `../../mod/src/index.ts`. For each, depcruise must report `not-in-allowed`, the default-deny rule, and no `not-to-unresolvable` error, so the plant fails for the right reason. Each generated allow rule for a third-party package matches that package's resolved path under `node_modules/`, never a dependency type, so a declared workspace package, which resolves to `packages/`, can't pass as third-party. Next, it writes `packages/ui/src/planted.ts` importing `radix-ui`, which the canvas installs, and expects `not-in-allowed`, since `ui` lists no third-party package [DR-70]. Last, it writes `packages/canvas/src/planted.ts` importing `claude-code`, then one importing `tests/golden/cli/focus.json`, then one holding the string literal `"tests/golden/cli/focus.json"`, a runtime read no import rule sees, and then `tests/e2e/src/planted.ts` reading `"../../golden/cli/focus.json"` by a path relative to itself, and expects each to fail by its rule's name, the last two through the literal scan [DR-81].
6. Adds `requires = ["observability"]` to `packs/code/pack.toml`, a built-in that requires an outer ring, then `requires = ["ruby"]` to an out-of-tree copy of the flags pack loaded beside the Ruby pack, so it requires another out-of-tree pack. It runs the loader on each and expects `pack_invalid` naming the inward rule.
7. Changes `boundaries.toml` with no new record, runs `cargo xtask check layout`, and expects it to fail.
8. Removes the worktree.

CI runs the self-test on every push. Until packs land in M1, the bare-name literal scan in step 4 and the pack `requires` plants in step 6 print `not built`, not `skipped`: there is no check yet to skip, and none passes for them [DR-83].

### Decision-record gate

Changing the layout, a boundary or a convention needs a record before it lands (R16), and the gate checks it without depending on any branch's merge base [DR-59].

- **Gated surfaces:** `boundaries.toml`, the `[dependencies]` between workspace crates, the top-level directory list, each package's `exports` and its dependency names (never their versions, so a Dependabot bump needs no record), the area contracts table on this page, parsed so formatting doesn't count, and the Boundaries and Conventions sections of `AGENTS.md` [DR-70].
- **The lock and the state check.** `layout.lock` holds each surface's sha256 and the record that approved it. On every run `cargo xtask check layout` recomputes the hashes and fails on any difference, naming the fix: `cargo xtask gen --layout-lock DR-n`. It also fails when a record the lock names is missing from `decisions.md`, or when any `## DR-n` heading appears twice. `gen --layout-lock DR-n` refuses unless `## DR-n` is in `decisions.md` and is newer than every record the lock already names. The first lock names DR-79 for every surface, and the commit that writes it needs no record of its own. Every later lock change adds its record in the same commit [DR-81]. None of this depends on history, so it holds on any checkout [DR-70]. In a linked worktree, where a branch is built (`git rev-parse --git-dir` differs from `--git-common-dir`), a changed surface is a distinct, non-failing state: "pending ratification: gated surfaces X and Y changed; record them in the merge commit". There `gen --layout-lock` refuses, so a branch never writes a lock or invents a record number [DR-79].
- **Per commit, over what a push adds.** In CI, on a `push` to a branch or a `pull_request`, `check layout --commits RANGE` walks only the commits the push or pull request adds: `github.event.before..github.sha` for a push whose `before` is an ancestor of it, the commits no other remote branch holds for a new branch or a force push, and the pull request's base to its head. A `workflow_dispatch`, a `schedule` or a tag push runs the state check only, since its commits were walked when they reached a branch; so the release dry run and a tag's `checks` job can pass [DR-79]. A commit that changes `layout.lock` must add the `## DR-n` heading the changed entry names in the same commit, except the commit that writes the first lock [DR-81]. A commit already pushed is never walked again, so one bad commit can't keep CI red, and the state check still holds the lock to the surfaces. CI checks out with `fetch-depth: 0`, and a range it can't resolve fails the check, never skips it [DR-70].
- **Record numbers and branch merges.** A branch writes no record and no lock. When a branch merges, `check layout --commits FORK..TIP` lists every gated-surface change in it, and the merge commit adds the record and the lock, taking the next free number then. So numbers are never assigned ahead, and branches merging in any order can't break the "newer than every record the lock names" rule [DR-70, DR-79].
- **Budgets.** `budgets.json` is hand-ratcheted. A commit that raises any gated number in it must add a record in the same commit; tightening one needs none. What counts as a raise is in [Contracts](contracts.md) (`budgets.json`).

## Generated directories

A wholly generated directory has a `GENERATED.md` stub, which covers every file below it but the area's `AGENTS.md` and `CLAUDE.md`, and a nested generated directory has its own; a generated file in a hand-written directory has the header line, a JSON file in a top-level `$comment`; a generated region inside a hand-written file sits between BEGIN GENERATED and END GENERATED markers. Each has a drift check. Nobody edits them by hand; run the generator instead. `cargo xtask gen` writes `.gitattributes` and the stubs from this table, `boundaries.toml`'s `generated_read_only` points at it, and `generated_headers`, a `cargo xtask check` step, checks every committed path it names: the stub, the header or the markers, and no file in a generated directory that its generator doesn't write, since no drift check would compare it; a path that doesn't exist fails unless its cell tags it with the milestone that builds it, `(Mn)`, while a tagged path that exists fails, so the change that creates it drops the tag [DR-79]. The table's reading: each code span in a Path cell is a path or a glob, brace lists expanded; a cell that says "marked" names a region and one that says "field" a value inside a file; a path ending `/` is a directory; a row whose Committed cell starts with "no" is skipped. On a merge conflict in one, take either side and run the command in the last column [DR-79].

| Path | Generator | Committed? | On a conflict, run |
|---|---|---|---|
| `schema/` | `cargo xtask gen` (schemars) | yes | `cargo xtask gen` |
| `schema/baseline/` | `cargo xtask gen --schema-baseline`, run only with a `schema_version` bump | yes [DR-59] | keep the side with the higher `schema_version` |
| `packages/contract/src/generated/` | `cargo xtask gen` (json-schema-to-typescript, the catalogue, its hash, the fallback panes and the mod's strings from `en.toml`, and `format.ts`, the `checkFormat` port of `contract::format` [DR-85]) | yes | `cargo xtask gen` |
| `plugin/types/` | `cargo xtask gen`: `index.d.ts` | yes | `cargo xtask gen` |
| `plugin/hooks/` | `cargo xtask gen`, or `gen --mod` for the bundle alone: `hooks.json` naming the bundle, and `planroom-mod.js` (esbuild, ESM, no splitting, `claude-code` external; deterministic). The host loads a hooks module only when `register` is exported on its own declaration, so `gen` rewrites esbuild's export list to `export function register` [DR-70, DR-79, DR-81] | yes, because a local-path marketplace loads the plugin from the repo | `cargo xtask gen` |
| `plugin/.claude-plugin/plugin.json`, its `version` field | `cargo xtask gen` and `gen --mod` stamp it from the workspace version, `-dev.g<8 hex>` between releases [DR-83]; JSON carries no markers, so its drift check alone covers it | yes | `cargo xtask gen` |
| `plugin/scripts/engine.env` | `cargo xtask gen` writes the whole file: `PLANROOM_CATALOGUE_HASH`, `PLANROOM_ENGINE_VERSION` from the workspace version, which `gen --version V` (M5) sets with every other manifest, and `PLANROOM_REPO` from `[workspace.package] repository` [DR-69] | yes, in the commit that changes it | `cargo xtask gen` |
| `plugin/scripts/contract.sh` | `cargo xtask gen` from `contract::ErrorCode` and `contract::resolver`; its contents are in [Contracts](contracts.md#the-resolvers-environment) [DR-69, DR-85] | yes | `cargo xtask gen` |
| `.dependency-cruiser.cjs` | `cargo xtask gen` from `boundaries.toml` | yes | `cargo xtask gen` |
| `.gitattributes` | `cargo xtask gen` from this table: each whole-file row's paths and each directory row's `<dir>/**`, marked `linguist-generated` | yes | `cargo xtask gen` |
| `crates/{kernel,contract,store,extract}/clippy.toml` | `cargo xtask gen` from `boundaries.toml`: `no_io` for the kernel and contract, `std::net` for store and extract [DR-79] | yes [DR-59] | `cargo xtask gen` |
| `crates/kernel/src/redact_fields.rs` | `cargo xtask gen` from the `x-redact` marks of the output schema view: the paths `kernel::redact` masks ([Contracts](contracts.md#cli-json-envelope)) [DR-89] | yes | `cargo xtask gen` |
| each area's contract block and Read first lines, a marked region of its `**/AGENTS.md` | `cargo xtask gen` from the two area tables on this page; `area_contracts_match_repo_layout` checks the markers | yes [DR-59, DR-70] | `cargo xtask gen` |
| `layout.lock` | `cargo xtask gen --layout-lock DR-n`, in the commit that adds the record (a branch's merge commit when a branch brought the change) | yes [DR-59] | a record and `gen --layout-lock` with it, in the same commit |
| `docs/reference/cli.md` (M5), `docs/reference/configuration.md`, `docs/reference/errors.md` | `cargo xtask gen` | yes | `cargo xtask gen` |
| the marked tables in `docs/reference/packs.md` (M5) | `cargo xtask gen` | yes | `cargo xtask gen` |
| `tests/golden/` | `cargo xtask gen --golden`, which runs the Rust tests that write them on the built fixture, with `PLANROOM_GOLDEN=write`, which the engine never reads; a plain test run compares and never writes. Every file comes from a real engine run; `cli/map-empty.json` is `map --detail graph` before the first scan, with the temporary root written as `/home/you/shop` [DR-70, DR-79, DR-82] | yes | `cargo xtask gen --golden` |
| `packs/*/evals/*.result.json` (M2) | `cargo xtask eval` (M2) | yes | `cargo xtask eval` |
| `target/canvas/` | `cargo xtask gen --canvas` (`vite build --configLoader runner`, so the config can import `@planroom/ui`'s TypeScript [DR-82]); `crates/planroom/build.rs` embeds it | no | |
| `target/mod-test/` | `cargo xtask test`, before it runs `claude plugin test`: the mod and its tests bundled by esbuild, the workspace packages inlined, `claude-code` and `claude-code/testing` external, the golden CLI outputs as a generated module, and a `hooks/hooks.json` naming the bundle | no [DR-59] | |
| `target/budgets.measured.json` (M2) | `cargo xtask gen --budgets` (M2), which prints its diff against `budgets.json` and never writes that file | no [DR-59] | |

## The one task entry point

`cargo xtask` needs nothing beyond the pinned Rust and Node, because `just` isn't installed [DR-38]. Agents and CI use only these commands, and CI runs nothing else [DR-59]. Each first runs `pnpm install --frozen-lockfile --offline`, falling back to an online install, and on failure prints one line naming the fix. Each never stops at the first failure, prints one `ok`, `FAILED` or `skipped (why)` line per check, and names the fix command on failure. Under `CI=true` every `skipped` counts as `FAILED`, unless the check is in `crates/xtask/ci_skips.toml`, where each entry names the check, the reason and the record that approved it [DR-79]. So no gate in S14 or S15 can go unrun on a green CI, and `feature_list_is_complete` refuses `passing` for an entry whose listed test or step was skipped on its CI path. Every command sets `PLANROOM_REPLAY=strict` and `PLANROOM_SPEND_LOG` to the absolute path of the main checkout's `docs/evidence/spend.jsonl`, found through `git rev-parse --git-common-dir`, so every worktree shares one log. The test commands give each run a temporary `PLANROOM_DATA` [DR-49, DR-51, DR-65]. Under nextest, `crates/xtask/nextest.toml` runs each test that asserts a wall-clock budget with no other test beside it, since nextest gives every test a process of its own and a test file's lock can't hold across them.

| Command | Runs | Typical time (warm) |
|---|---|---|
| `cargo xtask check` | `cargo fmt --check`; clippy with `-D warnings`; `prettier --check`; `eslint`; `tsc -b`; depcruise; the boundary rules and self-test; every drift check; `generated_headers`, over every path the Generated directories table names; `area_contracts_match_repo_layout` and `every_directory_has_a_purpose`; `Cargo.lock is committed and current`, which under `CI=true` fails when building xtask rewrote `Cargo.lock`, so a manifest changed without it; the docs tests; release pins; the guardrails tests (`python3 .claude/guardrails/tests/test_guardrails.py` and `test_context_restore.py`); `examples/check-fixture`; `scripts/check-tag-version.sh` against the workspace version [DR-69]; `claude plugin validate --strict` on `.` and `plugin/`; `cargo deny check licenses bans sources`; the layout gate; `ci_runs_only_xtask`; `feature_list_is_complete`; the schema lint and the breaking-change guard over `schema/baseline/` ([Contracts](contracts.md#versioning-rules)); the secret scan over `tests/replay/` and `docs/evidence/`, for the kernel's named secret shapes, never its entropy rule. Until a step is built, its command prints a `not built` line for it, never `ok` or `skipped`, from one table keyed by command (`NOT_BUILT` in `crates/xtask/src/report.rs`): for `check`, `not built (M5)` for the docs tests and `scripts/check-tag-version.sh` [DR-85] | 2 to 3 min |
| `cargo xtask check boundaries [--self-test]` | the boundary rules alone, or with the control and the planted crossings | 40 s |
| `cargo xtask check layout [--commits RANGE]` | the state check of `layout.lock`, and in CI the walk over the commits a branch push or pull request adds, never on a `workflow_dispatch`, a `schedule` or a tag push [DR-79]; in a linked worktree, `pending ratification` instead of a failure for a changed surface [DR-79]; on a branch merge, the list of its gated changes [DR-70] | 5 s |
| `cargo xtask check audit` | `cargo deny check advisories`, `pnpm audit --prod`, and the pin-age check (each direct pin's release at least 14 days older than the commit that set it, from crates.io and the npm registry, except a release that fixes a RustSec advisory in the tree, which the commit and DR-83's list name [DR-83]), which need the network; locally `skipped (offline)` when it's down, in CI a failure [DR-70] | 30 s |
| `cargo xtask check guardrails` | the two guardrails test files alone | 5 s |
| `cargo xtask test [-p CRATE [FILTER]] \| --ts PATH \| --installer` | `cargo nextest run` plus `cargo test --doc` (locally, without nextest, `cargo test` with a `skipped` line; in CI nextest is installed and the fallback fails [DR-79]), each with `--no-fail-fast`, so one failure doesn't hide the next [DR-83]; `vitest run`; with `-p`, only that crate's tests, and with `FILTER` as well, only those whose names hold it, such as `cargo xtask test -p planroom truth_recall_never_drops`, or, when none does, the crate's test file `tests/FILTER.rs`, such as `cargo xtask test -p planroom git_hardening`; `cargo xtask test -p planroom` runs every scenario, and each step a `-p` run leaves out prints a `not run` line, which neither passes nor fails, in CI too [DR-104]; with `--ts`, only that vitest file or folder, for a quick loop [DR-70]; `claude plugin test target/mod-test`; the scenario CI paths S1 to S15 on fresh fixtures; the `tests/plugin/` scripts, except `slash_commands_snapshot.sh`, which needs a live session and runs from `cargo xtask live evidence S15`; budgets; eval recompute (M2). A test that skips itself (an `#[ignore]`d Rust test, from `cargo nextest list`, or `cargo test`'s output without nextest; a skipped or todo vitest test, from its JSON report) is a `skipped` line of its own, so under `CI=true` it fails unless `ci_skips.toml` lists it [DR-79]. Until they're built, a whole run prints `not built` lines for the `tests/plugin/` scripts and eval recompute. `--installer` runs ShellCheck and `tests/installer/run.sh` instead, under BusyBox `sh` and dash on Linux, and under `/bin/sh` and dash on macOS, which has no BusyBox build; the detach test runs on both [DR-78, DR-79]. The harnesses compare paths as the engine reports them, physical (`pwd -P`, `canonicalize`), so macOS's `/private/var` matches, and find a command by walking `PATH` rather than with `command -v` [DR-83] | 4 to 6 min |
| `cargo xtask e2e [--screens]` | `playwright install chromium` at the pinned version (`--with-deps` in CI); builds the canvas; `planroom serve` on a built fixture with replay; Playwright at 1440×900 and 390×844, light and dark; the empty-state audit through both transports; reduced motion; console errors; the Artifact page with the fake `db`; the plugin install smoke in an isolated `HOME`, with mods on and off. The specs are Playwright's `specs` project, and the screenshots its `screens` project, which only `--screens` runs, so no spec skips itself; a test Playwright's JSON report shows skipped is a `skipped` line, a failure under `CI=true` unless `ci_skips.toml` lists it [DR-79]. Until they're built, it prints `not built` lines for the empty-state audit through both transports and the plugin install smoke | 4 min |
| `cargo xtask gen [--mod] [--golden] [--budgets] [--canvas] [--layout-lock DR-n] [--schema-baseline] [--version V]` | plain `gen` writes every committed output whose generator the table above names as `cargo xtask gen`, the mod bundle included, and the CLI reference (M5); `--mod` writes only the mod bundle, for a quick loop on the mod [DR-81]; `--golden` reruns the tests that write `tests/golden/`, which a plain test run only compares against; `--budgets` (M2, DR-89) and `--canvas` write outputs that aren't committed; the last three are one-off writes, and `--version` (M5) isn't built yet. On a conflict in a generated file, run the command the table's last column names [DR-70, DR-79] | 30 s |
| `cargo xtask fixture OUT [--no-packs]` | `examples/build-fixture OUT`, then `OUT/.planroom/config.toml` loading `examples/packs/ruby` (omitted with `--no-packs`), recorded as a trusted workspace in the data root [DR-48] | 2 s |
| `cargo xtask bench fold` | runs `cargo bench -p planroom-kernel --bench fold`, which prints one `{"bench": NAME, "median_ms": N}` line per case, and fails a case whose median passes `budgets.json`'s `fold.20k` or `fold.50k` `ms`, that has no number there, or that printed no line; CI runs it on ubuntu-24.04 [DR-89] | 3 min |
| `cargo xtask eval [--verify-order] [--report]` (M2) | recompute every eval result from replay; check label order with git; write the S7 table | 10 s |
| `cargo xtask release-dry-run [--target T]` (M5) | release toolchain check; canvas build; release build, which embeds it; package twice and `cmp`; `scripts/check-tag-version.sh` against the workspace version; smoke test on the built fixture, with `serve` run from `target/release/` and from a `cargo install --path` root and a fetch of `/`; the resolver suite against a `127.0.0.1` server holding a locally built, checksummed archive | 6 min |
| `cargo xtask live <eval KIND \| record SNN \| evidence SNN>`; `live eval` and `live evidence` (M2) | needs `PLANROOM_LIVE_JEV=1` or `PLANROOM_LIVE_GATEWAY=1`; prints its estimate from the live spend plan and refuses past what `docs/evidence/spend.jsonl` has left under `live_tests_total_usd`; every call appends to that file under its lock; nested evidence sessions get the same absolute path, and each evidence run checks that its live calls reached the file; writes `tests/replay/` or `docs/evidence/` [DR-51, DR-65] | varies |

## Toolchain pins

Each pin has one home and a `release_pins` test that fails when another file names a different version [iris-release-bar §7].

| Pin | Home |
|---|---|
| Rust 1.97.0 | `rust-toolchain.toml` |
| Node 22.22.0 | `.node-version` |
| pnpm 10.28.2, exactly, with `minimumReleaseAge: 10080`; CI gets it through corepack | `package.json` `packageManager`, `pnpm-workspace.yaml` |
| Dependabot cooldown of 14 days, the direct-pin age in [`AGENTS.md`](../../AGENTS.md#dependencies); `minimumReleaseAge` stays 7 days for transitive packages [DR-70] | `.github/dependabot.yml`; `release_pins.rs::dependabot_cooldown_matches_the_pin_age` |
| Claude Code 2.1.310 | an `xtask` constant; `xtask` installs that exact version with `DISABLE_AUTOUPDATER=1`; `packages/mod/vendor/claude-code.d.ts` holds that version's mod API types [DR-81] |
| Python 3.11 or later (guardrails, `build-fixture`, `check-fixture`) | `xtask` check, CI setup |
| git 2.34.1 or later, Ubuntu 22.04's; the engine checks repo ownership itself [DR-78] | `planroom doctor` and `scan` (`git_too_old`), CI setup |
| `cargo-about 0.9.2` | a sha256-pinned download in `xtask` |
| `cargo-deny 0.20.2`, `cargo-nextest 0.9.149`; ShellCheck, BusyBox (Linux only) and dash for the installer suite | `xtask`, which installs them under `CI=true` (`crates/xtask/src/tools.rs`): cargo-deny from its sha256-pinned release archive, nextest with `cargo install --locked` at its exact version, ShellCheck, BusyBox and dash from the runner's package manager. A tool fetched as a release binary is pinned by sha256; one from a package manager or `cargo install --locked`, by version [DR-83] |
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
  .claude-plugin/plugin.json         name, version (the workspace version; a -dev one stamped .g<8 hex> by plain gen from
                                     plugin/'s own files [DR-83]), description, author, license "MIT",
                                     homepage, repository, keywords
  .claude-plugin/icon.png            1024 px square, from the Iris logo
  README.md                          for users: what it is, the first command, links; one line on turning mods on;
                                     the Claude Code version the mod is tested on [DR-76]
  AGENTS.md                         the area contract and its Read first lines; inert in an installed plugin; no CLAUDE.md,
                                     which validate --strict warns on [DR-70, DR-81]
  hooks/                             GENERATED, the whole directory [DR-79]
  hooks/GENERATED.md                 the stub that marks it, written by cargo xtask gen
  hooks/hooks.json                   {"modules": ["./planroom-mod.js"]}
  hooks/planroom-mod.js              the mod's bundle
  types/                             GENERATED, the whole directory [DR-79]
  types/GENERATED.md                 the stub that marks it
  types/index.d.ts                   the $.state contract
  skills/planroom/SKILL.md               user-invocable: false
  skills/room-plan/SKILL.md          user-invocable: false
  skills/room-handoff/SKILL.md       user-invocable: false
  agents/implementer.md              model sonnet, effort high, tools Read, Edit, Write, Bash, Grep, Glob; Bash sandbox
                                     on, writes only in its clones, no network [A-18, DR-75]
  agents/curator.md                  model sonnet, effort medium, tools Read, Bash, Grep, Glob
  scripts/planroom                   the POSIX sh resolver; finds engine.env and the checkout from its own path;
                                     prefetch, build and status subcommands for the mod [DR-58]
  scripts/engine.env                 GENERATED whole by cargo xtask gen: PLANROOM_ENGINE_VERSION, the workspace
                                     version, which gen --version V (M5) sets; PLANROOM_CATALOGUE_HASH; and
                                     PLANROOM_REPO, from [workspace.package] repository; committed and drift-checked [DR-69, DR-79]
  scripts/contract.sh                GENERATED: the codes and output templates the resolver sources, so every resolver
                                     output is a contract type [DR-69]
```

No `bin/`, no `.mcp.json`, no symlinks, no classic hooks; `tests/plugin/` asserts each [DR-14, DR-28, DR-30].

## CI workflows

`.github/workflows/ci.yml` runs on `push` to every branch (`branches: ["**"]`), on pull requests and on `workflow_dispatch`. It cancels superseded runs per ref except on `main`, sets `permissions: contents: read` and `persist-credentials: false`, checks out with `fetch-depth: 0` so the layout gate and `eval --verify-order` can walk history, and pins every action by SHA. Its `workflow_dispatch` runs check state only, as the layout gate defines [DR-79]. No job needs a secret or calls a live model. Every job's only `run:` steps are the listed setup steps (`rustup toolchain install`, `corepack enable`, `pnpm install --frozen-lockfile`) and one `cargo xtask` command, and `ci_runs_only_xtask` fails on anything else [DR-59].

| Job | Command | Runner |
|---|---|---|
| `check` | `cargo xtask check` | ubuntu-24.04 |
| `test (os)` | `cargo xtask test` | ubuntu-24.04, macos-15; wall-clock gates only on ubuntu-24.04, recorded on macos-15 [DR-43]. Unlike Iris, no macos-15-intel: the dry run builds and smoke-tests the Intel archive on that runner [DR-69] |
| `e2e` | `cargo xtask e2e` | ubuntu-24.04 |
| `audit` | `cargo xtask check audit` | ubuntu-24.04 |
| `guardrails` | `cargo xtask check guardrails` (also inside `check`, so a hook regression shows by name) | ubuntu-24.04 |
| `installer (os)` | `cargo xtask test --installer`, the detach test included [DR-69] | ubuntu-24.04, macos-15 |
| `release-dry-run (target)` | `cargo xtask release-dry-run --target T`, a job in `ci.yml` held at `if: false` until the command exists (M5) | the release workflow's runners: ubuntu-24.04 and ubuntu-24.04-arm for the musl targets, macos-15-intel for `x86_64-apple-darwin`, macos-15 for `aarch64-apple-darwin` |

Scheduled: `advisories.yml` (weekly `cargo xtask check audit`) and `toolchain.yml` (weekly pin-age checks for Rust, Node and Claude Code), both (M5). `main` is the default branch, so schedules, `workflow_dispatch` and Dependabot all read it [DR-69]. `release.yml` runs `verify`, `checks`, `build` and `publish` on a `v*` tag push, or as a dry run on `workflow_dispatch` from `main`; only `publish` has `contents: write`. Its steps and targets are in [Architecture](architecture.md#packaging-and-release). `.github/dependabot.yml` opens weekly, grouped updates against `main` with a 14-day cooldown, so no update is younger than a direct pin may be [DR-70].

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
| Working in this repository: the commands to run, the files never to edit, area contracts, the build record, what commits and docs say, attribution | Working in this repository |
