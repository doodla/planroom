# AGENTS.md

Shared instructions for coding agents (and people) working on Planroom, a shared, living map of a software system that people and agents plan on together. These are invariants and steering rules, not a description of the code; read `docs/contributing/architecture.md`, `docs/contributing/repo-layout.md` and the source for that.

## Compatibility

- The CLI command tree, flags, the `--json` envelope, error codes, warning codes, exit codes, events and every persisted format are public contracts, versioned by one `schema_version` ([Contracts](docs/contributing/contracts.md#versioning-rules)). Additive fields and codes are fine; a rename, a removal, a changed meaning or a new required input field bumps `schema_version`.
- Public types live only in `crates/contract`. Change them there, then run `cargo xtask gen`; `cargo xtask check` fails on any generated file that differs from a fresh generation.
- `--json` mode prints exactly one JSON document on stdout. Everything else goes to stderr.
- Readers of persisted documents keep loading older `format` versions and keep fields they don't know.

## Providers

- Model calls go only through `crates/models` (Jev and Claude transports, live, record and replay). The rest of the engine sees the shared types, never a provider's wire format.
- Provider error strings never enter the error taxonomy; map them to a `contract::ErrorCode`.
- Model IDs are pinned in `crates/planroom/src/config/defaults.toml`; Planroom never picks a model on its own.

## Money and credentials

- Configuration holds only the names of environment variables, never a secret. Never print, log, persist or put a credential in argv, a URL, a fixture or an error message.
- Spend guards and the `live_tests_total_usd` ledger bound every paid call; a guard that stops work turns it into a card, never a silent drop.
- Never send a credential to a host other than the configured provider origin.

## Dependencies

- Prefer stable, maintained crates and packages over hand-rolled code; hand-roll only trivial code, and say why in the commit message.
- Pin every dependency exactly (`=x.y.z` in `Cargo.toml`, an exact version in `package.json`) to a release at least 14 days old, and commit `Cargo.lock` and `pnpm-lock.yaml`. `pnpm-workspace.yaml` sets `minimumReleaseAge` for the rest.
- Dependencies must pass `cargo deny check` (`deny.toml`: MIT-compatible licenses, no OpenSSL, no unknown sources).

## Tests

- Tests are offline, free and deterministic: strict replay, the fixture built per run, the `patch` implementer, fake keys only.
- Live tests are opt-in behind `PLANROOM_LIVE_*` and never run in CI.
- Test observable behavior through the CLI `--json` or the HTTP API; compare with `TRUTH.json` through `planroom-testkit::truth`.
- A `feature_list.json` entry turns `passing` only when every test it lists passes on its CI path.

## Documentation

- Write docs by the style rules in `docs/contributing/research/iris-release-bar.md` §4 until the style guide exists: one reader and one type per page, second person, short sentences, tables for reference facts.
- Every fact has one home. Change the home, then link to it instead of restating it.
- Never hand-edit a generated reference page; regenerate it.

## Boundaries

- `boundaries.toml` is the single source of the boundary rules for Rust, TypeScript and packs. `cargo xtask check boundaries` enforces the Rust side and `cargo xtask gen` writes `.dependency-cruiser.cjs` for the TypeScript side.
- Library crates depend only on `planroom-contract`; only `planroom` composes them; `planroom-testkit` is a dev-dependency only.
- The kernel is domain-free: no I/O, and no built-in pack kind ID in its string literals.
- Each top-level area's `AGENTS.md` (or `README.md` for `plugin/` and `packs/`) states what it owns, what it may depend on and how to test it.

## Conventions

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

| You are adding                     | It goes in                                                                                |
| ---------------------------------- | ----------------------------------------------------------------------------------------- |
| a language with a built-in grammar | a `.scm` file and `[[extractors]]` in `packs/code`, emitting the existing hint kinds      |
| any other language                 | a `regex` extractor in a pack, emitting the existing hint kinds                           |
| a file format that needs a parser  | a module in `crates/extract`, plus a `builtin` declaration in a pack                      |
| a cross-repo join                  | a linker in `crates/extract/src/link/`, plus a `[[linkers]]` declaration                  |
| a question                         | `[[questions]]` in a pack, as a pipeline of existing shapes                               |
| a query shape                      | the kernel, with a decision record naming the two packs that need it                      |
| a decision kind                    | `[[decisions]]` in a pack, plus `evals/`; a kernel role only with a decision record       |
| a command                          | a `Command` variant with its `CommandSpec`, and one handler in `crates/planroom/src/app/` |
| a canvas view                      | `packages/canvas/src/views/`, reached from one of the five concepts                       |
| a pane section or key              | `packages/mod/src/pane.tsx` and the key map snapshot; hotkeys come from `@planroom/ui`    |
| a public type                      | `crates/contract/src/`, then `cargo xtask gen`                                            |
| a user-facing string               | `crates/planroom/text/en.toml`, or a pack label                                           |
| a test helper                      | `crates/testkit` (Rust) or `tests/e2e/shims/` (browser)                                   |
| a recorded model response          | `cargo xtask live record`, never by hand                                                  |

## Working in this repository

- Run `cargo xtask check` and `cargo xtask test` before claiming success, and say what was skipped. Agents and CI use only `cargo xtask` commands.
- Never edit `KICKOFF.md`, `examples/kickoff-snapshot.md` or `TRUTH.json` (the last only through a decision record).
- Keep `.claude/carry.md` current and under 4,000 characters; keep `progress.md` as the full record.
- Changing the layout, a boundary or a convention in this file needs a decision record in `docs/contributing/decisions.md` first.
- Respect unrelated work: don't revert, reformat or "clean up" code outside your task.
- Commits follow the attribution rules in `CLAUDE.md`; never fabricate a git identity; never discard others' changes.
