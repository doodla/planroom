# Iris release bar

This page records what `doodla/iris` does at the repository level, so that Planroom can match it
before `v0.1.0`. It covers the repo files, CI, the release workflow, the docs rules, the agent
instruction files, contract drift tests, and how to use Iris for Planroom's art.

Source: `doodla/iris` `main` at commit `76a476a44a3d6289f69587488528587c7f35721f` (2026-10-01),
cloned to `/home/user/iris-ref` with `GIT_LFS_SKIP_SMUDGE=1 git clone --depth 1`. Research date:
2026-10-25.

## Evidence key

- **[V]** Verified here: a command was run, or the file was read at the cited path and line. Paths
  are relative to `/home/user/iris-ref` unless they start with `/`.
- **[D]** Read in documentation only, with its URL. Not checked here.
- **[A]** Assumed or recommended. Not checked.

## 1. Repo-level checklist

Every repository-level item Planroom needs, with what Iris has. "Planroom" notes say what changes
for a Rust plus TypeScript plus plugin repository.

| # | Item | Iris's version | Iris path | Planroom note |
|---|---|---|---|---|
| 1 | `README.md` | Landing page: one-paragraph pitch, one example command, an `[!IMPORTANT]` billing alert, "Why Iris" bullets with bold run-ins, Install (one `curl \| sh` line), numbered Quickstart with captured output, agents section, Models table, Documentation links grouped by type, Project status (version, platforms, preview caveats), Contributing, License. No images. [V] | `README.md:1-147` | Add the hero and logo at the top. Same section order. Links to non-archived files use absolute URLs. |
| 2 | `AGENTS.md` | Invariants and steering rules for agents and humans, in seven sections (see section 5). [V] | `AGENTS.md:1-100` | Required by KICKOFF. Imported at the top of Planroom's `CLAUDE.md`. |
| 3 | `CLAUDE.md` | One line: `@AGENTS.md`. [V] | `CLAUDE.md:1` | Planroom keeps its existing sections below the import. |
| 4 | `CONTRIBUTING.md` | Ways to contribute, Scope (out-of-scope list), dev setup (MSRV, no `rust-toolchain.toml`), offline tests, the one-command check, each check's command, how to regenerate generated files, compatibility, commit rules, security pointer, release pointer. [V] | `CONTRIBUTING.md:1-139` | Add Node and pnpm or npm setup, TS checks, plugin validation, guardrail tests. |
| 5 | `SECURITY.md` | Private reporting through GitHub's Security tab, fallback if the button is missing, supported versions (latest only before 1.0), in-scope and out-of-scope lists specific to the tool. [V] | `SECURITY.md:1-44` | Scope: local HTTP server binding, SQLite store, plugin binary download, Artifact `db` contents, key handling. |
| 6 | `CHANGELOG.md` | Keep a Changelog 1.1.0, `## [Unreleased]` on top, `## [X.Y.Z] - YYYY-MM-DD`, subsections Added, Changed, Fixed, and "Known limitations" in 0.1.0. Bold run-in per entry, then a user-facing explanation and a link. Links are absolute GitHub URLs. Compare links at the bottom. [V] | `CHANGELOG.md:1-176` | Same. Release notes are parsed from it (section 3). |
| 7 | `CODE_OF_CONDUCT.md` | **Iris has none.** Not on `main`, not in `git ls-files`. [V] | (absent) | KICKOFF requires one. [A] Use Contributor Covenant 2.1, <https://www.contributor-covenant.org/version/2/1/code_of_conduct/>, with the reporting route pointing at a maintainer contact. |
| 8 | `LICENSE` | MIT. [V] | `LICENSE` | Planroom already has one. |
| 9 | `.github/workflows/ci.yml` | 9 jobs (section 2). [V] | `.github/workflows/ci.yml:1-312` | Add TS, e2e, plugin, guardrails jobs. Trigger on every branch push. |
| 10 | `.github/workflows/release.yml` | Tag-triggered build, package, smoke test, publish (section 3). [V] | `.github/workflows/release.yml:1-316` | Add web bundle and plugin packaging. |
| 11 | `.github/workflows/advisories.yml` | Weekly `cargo deny check`. [V] | `.github/workflows/advisories.yml:1-32` | Add `npm audit` or equivalent. |
| 12 | `.github/workflows/toolchain.yml` | Weekly: fails when the pinned release Rust is more than 3 minors behind stable. [V] | `.github/workflows/toolchain.yml:1-48` | Same, and consider the same for the pinned Node. |
| 13 | `.github/dependabot.yml` | `cargo` and `github-actions`, weekly, limit 10, minor and patch grouped into one PR each. [V] | `.github/dependabot.yml:1-30` | Add `npm` for each package directory. |
| 14 | `.github/PULL_REQUEST_TEMPLATE.md` | What, Why, Contract changes ("None." if none), Checks run (checkboxes per command, conditional ones say when), Secrets checkbox, Live testing (cost if any). [V] | `.github/PULL_REQUEST_TEMPLATE.md:1-40` | Same headings. Contract list becomes Planroom's contracts. |
| 15 | `.github/ISSUE_TEMPLATE/bug_report.md` | Security warning comment, What happened, Version (`iris --json version`), Command and error (JSON, text not screenshots, redact), Environment, Anything else. Label `bug`. [V] | `.github/ISSUE_TEMPLATE/bug_report.md` | Ask for `planroom --json version` and the plugin version. |
| 16 | `.github/ISSUE_TEMPLATE/feature_request.md` | What you're trying to do, What's missing, Why existing commands don't cover it, Anything else. Label `enhancement`. [V] | `.github/ISSUE_TEMPLATE/feature_request.md` | Same. |
| 17 | `.github/ISSUE_TEMPLATE/config.yml` | `blank_issues_enabled: true`, a contact link to the Security tab. [V] | `.github/ISSUE_TEMPLATE/config.yml` | Same. |
| 18 | `install.sh` | POSIX sh installer with checksum and layout verification (section 3). [V] | `install.sh:1-520` | Planroom's plugin fetches the engine on first use "in the style of" this script (KICKOFF:236). |
| 19 | `deny.toml` | `[graph]` no features; advisories with `yanked = "deny"` and `git-fetch-with-cli = true`; licenses v2 allow list (MIT, Apache-2.0, Apache-2.0 WITH LLVM-exception, BSD-3-Clause, ISC, Unicode-3.0, Zlib, CDLA-Permissive-2.0), confidence 0.8, one crate-scoped MPL-2.0 exception; bans `multiple-versions = "warn"`, `wildcards = "deny"`; sources crates.io only. [V] | `deny.toml:1-70` | Copy, then adjust the allow list to what the tree uses (cargo-deny warns on unused entries). |
| 20 | `about.toml`, `about.hbs` | cargo-about config and template for `THIRD-PARTY-LICENSES` in each archive. Same allow list as `deny.toml`; checksum-pinned clarifications for `aws-lc-sys` and `aws-lc-rs`. [V] | `about.toml`, `about.hbs` | Same for Rust. The web bundle needs its own notices file [A]. |
| 21 | `Cargo.toml` | Single crate (no workspace): `edition = "2024"`, `rust-version = "1.89"`, explicit `include` list, bin plus lib, release profile `codegen-units = 1`, `lto = "thin"`, `strip = "symbols"`. [V] | `Cargo.toml:1-83` | Planroom will likely be a workspace [A]. Keep `include`, MSRV, and profiles. |
| 22 | Rust toolchain file | **None.** `CONTRIBUTING.md:35-37` says any recent stable works. Releases pin `RELEASE_RUST_TOOLCHAIN: "1.98.1"` in `release.yml:48`. [V] | (absent) | Local Rust here is 1.97.0 (`rustc -V` [V]), older than Iris's pin. Pick Planroom's pin from what CI and local both have. |
| 23 | `rustfmt.toml` | `max_width = 110`, `use_small_heuristics = "Max"`. [V] | `rustfmt.toml` | Copy. |
| 24 | `build.rs` | Embeds the target and an `IRIS_GIT_COMMIT` (7 to 40 hex chars, else null). Never reads `git` or `GITHUB_SHA`. [V] | `build.rs:1-50` | Copy the idea for `planroom version`. |
| 25 | `.gitignore` | Ignores generated media (`*.png`, `*.mp4`, and so on) except `!/tests/fixtures/**` and `!/docs/assets/**`, secrets (`.env*`, `*.pem`, `*.key`), local state, `/dist/`, scratch. [V] | `.gitignore` | Add `!/docs/screenshots/**` and the art directory. |
| 26 | `schema/` | One committed JSON Schema, `iris-output.v1.schema.json`, draft 2020-12, with `$id` on raw GitHub. [V] | `schema/iris-output.v1.schema.json` | One file per Planroom contract (section 6). |
| 27 | `scripts/check.sh` | Runs every CI check locally, never stops early, prints one line per check (`ok`, `FAILED`, `skipped`). `--fix` formats and regenerates; `--msrv` adds the MSRV. Unsets API keys first. [V] | `scripts/check.sh:1-151` | Must-copy. |
| 28 | `scripts/package-release.sh` | Reproducible archive builder (section 3). [V] | `scripts/package-release.sh:1-230` | Copy for the engine archive. |
| 29 | `scripts/smoke-test-release.sh` | Runs the packaged binary, then installs it with `install.sh` from `127.0.0.1`. [V] | `scripts/smoke-test-release.sh:1-154` | Copy. Add the plugin's fetch path. |
| 30 | `scripts/check-tag-version.sh` | Tag must equal `v` + `Cargo.toml` version; strict semver regex. [V] | `scripts/check-tag-version.sh:1-81` | Also check `package.json` and plugin manifest versions agree [A]. |
| 31 | `scripts/check-release-toolchain.sh` | Logs `rustc -Vv`, fails unless it is the pinned version. [V] | `scripts/check-release-toolchain.sh` | Copy. |
| 32 | `scripts/check-release-toolchain-age.sh` | Exit 2 when the pin is more than N minors behind stable. [V] | `scripts/check-release-toolchain-age.sh` | Copy. |
| 33 | `scripts/live-verify.sh` and `tests/live/` | Opt-in paid verification with a ledger; a mock of both APIs; CI runs only its `--plan` and mock mode. [V] | `scripts/live-verify.sh`, `tests/live/mock-run.sh`, `tests/live/mock_providers.py` | Planroom's Jev and Claude live tests behind an env flag, with a ledger; CI runs the replay mode. |
| 34 | `tests/installer/` | Offline installer suite: fixtures, a `127.0.0.1` server, `uname` and `sysctl` shims, BusyBox cases. [V] | `tests/installer/run.sh`, `make-fixtures.sh`, `server.py` | Copy for Planroom's installer and fetcher. |
| 35 | `tests/docs.rs` | Link, anchor and prose checks (section 4). [V] | `tests/docs.rs:1-359` | Port, as a Rust test or a TS script. |
| 36 | `tests/cli_reference.rs` | `docs/reference/cli.md` must equal rendered `--help`. [V] | `tests/cli_reference.rs:1-88` | Copy. |
| 37 | `tests/schema_contract.rs` | Schema drift and contract checks (section 6). [V] | `tests/schema_contract.rs:1-570` | Copy the pattern per contract. |
| 38 | `tests/release_pins.rs` | Hand-pinned tool versions agree everywhere they're named. [V] | `tests/release_pins.rs:1-222` | Copy, adding Node and any TS tool pins. |
| 39 | `docs/README.md` | Index of every page, grouped by type, each with a "What it covers" table. [V] | `docs/README.md:1-50` | Same. |
| 40 | `docs/guides/` | install, images, videos, models-and-costs, agents. [V] | `docs/guides/*.md` | Planroom's tasks. |
| 41 | `docs/concepts/` | paid-requests, video-jobs. [V] | `docs/concepts/*.md` | The graph model, reconciliation, decisions by Jev and Claude. |
| 42 | `docs/reference/` | cli (generated), json-output, errors, configuration. [V] | `docs/reference/*.md` | Add HTTP API, graph schema, mod protocol. |
| 43 | `docs/contributing/` | architecture, decisions (with sources and check dates), adding-a-provider (step-by-step plus checklist), live-testing (with a dated run log), releasing, style-guide. [V] | `docs/contributing/*.md` | Same set. Add "adding an extractor" in place of "adding a provider". |
| 44 | Art and prompts | **Iris has no art on `main`.** No image files tracked; `.gitignore` reserves `docs/assets/**` for doc images. [V] (`git ls-files`, grep for `logo`, `hero`, `docs/assets`) | (absent) | KICKOFF:229 means Planroom's own art made with Iris. See section 8. |

## 2. CI jobs

### `ci.yml` [V]

Triggers: `push` to `main`, every `pull_request`, `workflow_dispatch` (`ci.yml:3-7`). Top-level
`permissions: contents: read`. Concurrency: PR runs share a group per ref and cancel older runs;
every other run gets a group of its own by `run_id`, so a `main` run is never cancelled
(`ci.yml:17-19`). Every checkout uses `persist-credentials: false`. Every job has
`timeout-minutes`.

| Job | Runner | Runs | Timeout |
|---|---|---|---|
| `fmt` | ubuntu-24.04 | `cargo fmt --all --check` (stable, rustfmt) | 10 |
| `clippy` | ubuntu-24.04 | `cargo clippy --all-targets --locked -- -D warnings` | 30 |
| `test (os)` | ubuntu-24.04, macos-15, macos-15-intel; `fail-fast: false` | `cargo test --locked` with `IRIS_GIT_COMMIT=${{ github.sha }}` | 45 |
| `msrv` | ubuntu-24.04 | Reads `rust-version` from `Cargo.toml` with awk, installs it, `cargo check --locked --all-targets`, `cargo test --locked` | 45 |
| `deny` | ubuntu-24.04 | `EmbarkStudios/cargo-deny-action` `check` with `arguments: ""` (overrides the default `--all-features`) | 15 |
| `installer (os)` | ubuntu-24.04, macos-15 | ShellCheck on `install.sh` (`-s sh`) and all scripts; BusyBox on Linux; `sh tests/installer/run.sh` | 20 |
| `live-verify.sh (offline mock)` | ubuntu-24.04 | `cargo build --locked`; `env -u OPENAI_API_KEY -u GEMINI_API_KEY sh tests/live/mock-run.sh target/debug/iris` | 30 |
| `release dry run (target)` | x86_64 musl on ubuntu-24.04, aarch64 musl on ubuntu-24.04-arm | Reads `RELEASE_RUST_TOOLCHAIN` from `release.yml` with sed; checks it; Clippy with it; musl-tools; release build; cargo-about 0.9.2; packages twice and `cmp`s the two archives; smoke test | 45 |
| `package-check` | ubuntu-24.04 | `cargo package --list --locked`; asserts required files and refuses any file outside the `include` list | 15 |

Pinned actions (SHA plus version comment), all [V] in `ci.yml` and `release.yml`:

| Action | SHA | Version |
|---|---|---|
| `actions/checkout` | `3d3c42e5aac5ba805825da76410c181273ba90b1` | v7.0.1 |
| `dtolnay/rust-toolchain` | `02cb101ec7c40f2c49e1d9714d64511d8e1b74de` | master (2026-09-11) |
| `Swatinem/rust-cache` | `6323deb102c322ba6fcbdcafc7e3dddab59af2b6` | v2.9.2 |
| `EmbarkStudios/cargo-deny-action` | `3c6349835b2b7b196a839186cb8b78e02f7b5f25` | v2.1.1 |
| `taiki-e/install-action` | `4cef1412cce204788f482e778a0b9187f9626a29` | v2.87.21 |
| `actions/upload-artifact` | `043fb46d1a93c77aae656e7c1c64a875d1fc6a0a` | v7.0.1 |
| `actions/download-artifact` | `3e5f45b2cfb9172054b4087a40e8e0b5a5461e7c` | v8.0.1 |

### Scheduled workflows [V]

| Workflow | Job | Trigger | Runs |
|---|---|---|---|
| `advisories.yml` | `deny` | `cron: "17 6 * * 1"`, `workflow_dispatch` | Same `cargo deny check` as CI |
| `toolchain.yml` | `release toolchain age` | `cron: "43 6 * * 1"`, `workflow_dispatch` | Reads the pin from `release.yml`, installs stable, runs `check-release-toolchain-age.sh PIN STABLE 3` |

Both files note that GitHub disables schedules after 60 days without activity in a public repo
(`advisories.yml:1-5`, `toolchain.yml:1-7`).

### Release workflow jobs [V]

`verify` → (`checks`, `build` x4) → `publish`. See section 3.

### Planroom additions required by KICKOFF [A]

KICKOFF:258-264 and :310 add these to the Iris set: TS lint, format, typecheck and tests; end-to-end
tests (Playwright is installed here, per KICKOFF verified facts); `claude plugin validate` and
`claude plugin test`; guardrail tests (`python3 .claude/guardrails/tests/test_guardrails.py`, from
`.claude/guardrails/README.md`); schema drift (Rust and TS); `npm audit`; and a `push` trigger on
every branch (`branches: ["**"]`). With branch pushes, keep Iris's concurrency rule: cancel
superseded runs per ref only for non-`main` refs.

## 3. Release workflow mechanics

### Triggers and gating [V]

- `push` of tags `v*`, or `workflow_dispatch` with a `dry_run` boolean (default true). A manual run
  never publishes: `publish` has `if: github.event_name == 'push' && startsWith(github.ref,
  'refs/tags/v')` (`release.yml:245`).
- `verify` (read-only token): runs `scripts/check-tag-version.sh TAG` (tag must be `v` + Cargo
  version), installs `parse-changelog@0.6.17` through `taiki-e/install-action` with
  `fallback: none` (no unverified cargo-binstall fallback), extracts the version's changelog
  section, and fails a tag push when the section is missing or blank. It writes the notes to the
  job summary and uploads them as artifact `notes` (`release.yml:51-125`).
- `checks`: fmt, Clippy, and tests again on the tagged commit with the pinned toolchain, because a
  tag can point at a commit CI never passed (`release.yml:127-150`).
- `publish` needs `verify`, `checks` and `build`. Only it has `contents: write`, and it runs no
  third-party tool.

### Targets and artifact naming [V]

| Target | Runner | Floor |
|---|---|---|
| `x86_64-unknown-linux-musl` | ubuntu-24.04 | static; kernel 3.2+ |
| `aarch64-unknown-linux-musl` | ubuntu-24.04-arm | static; kernel 4.1+; `CC_aarch64_unknown_linux_musl=musl-gcc` |
| `x86_64-apple-darwin` | macos-15-intel | `MACOSX_DEPLOYMENT_TARGET=10.12` |
| `aarch64-apple-darwin` | macos-15 | `MACOSX_DEPLOYMENT_TARGET=11.0` |

- Archive: `iris-vX.Y.Z-TARGET.tar.gz` holding one directory `iris-vX.Y.Z-TARGET/` with `iris`,
  `LICENSE`, `THIRD-PARTY-LICENSES`, `README.md`, `CHANGELOG.md`, `docs/`
  (`docs/contributing/releasing.md:104-115`).
- Each build uploads artifact `release-TARGET`. `publish` downloads `release-*` into `dist/`, then
  fails unless the set of `*.tar.gz` is exactly the four expected names (`release.yml:264-289`).
- Release assets: the four archives, `SHA256SUMS`, and `install.sh`. Title is the tag. A tag with
  `-` becomes `--prerelease`. Created with `gh release create` using `github.token` through `env:`
  (`release.yml:294-316`).

### Reproducible packaging [V]

`scripts/package-release.sh`: requires exactly cargo-about 0.9.2 and fails on any cargo-about
`WARN` or `ERROR` line; generates `THIRD-PARTY-LICENSES` for the target only; refuses links and
special files; fixes modes (755 dirs, 644 files, 755 binary); sorts entries with `LC_ALL=C`;
archives with GNU tar (`--owner=0 --group=0 --numeric-owner --mtime=@SOURCE_DATE_EPOCH`) or BSD tar
equivalents; `gzip -n`; self-checks the listing against the expected set. `SOURCE_DATE_EPOCH`
defaults to HEAD's commit time. CI packages twice and `cmp`s.

### Checksums [V]

`publish` runs `sha256sum -- *.tar.gz > SHA256SUMS` over the verified set (`release.yml:291`).

### How `install.sh` verifies [V]

From `install.sh:20-49` and the functions it names:

1. Detects the target, preferring arm64 on Apple silicon under Rosetta (`sysctl hw.optional.arm64`).
2. Needs `curl` or `wget`, and `sha256sum` or `shasum`. Refuses a BusyBox `wget` that reports
   "certificate validation not implemented". `curl` runs with `--proto =https` (http allowed only
   for an `http://` test base URL), a 30 s connect timeout and a stall limit.
3. Resolves `latest` by following `releases/latest` to `.../tag/TAG` (no API). If that fails and
   `git` exists, it takes the highest `vX.Y.Z` tag from `git ls-remote` whose `SHA256SUMS` is
   published (at most 3 tries), and warns. On macOS it skips `/usr/bin/git` without Xcode tools.
4. Downloads `SHA256SUMS` first, then requires exactly one well-formed 64-hex line for the archive.
5. Downloads the archive into a private `mktemp -d` dir and compares SHA-256.
6. Checks the tar listing: only the documented entries, no `..` or empty components under `docs/`,
   no links or special files (`tar -tvzf` type column).
7. Extracts, runs `iris --version`, then copies only `iris` to a temp name in the target dir and
   `mv -f`s it into place. Never uses `sudo`. Never reads stdin (`exec </dev/null`).
8. Prints a `PATH` line for zsh, bash, fish or `.profile`, refusing to print one that would need
   escaping.

Verified run here: `sh /home/user/iris-ref/install.sh` printed

```text
iris-install: warning: could not find the latest release at https://github.com/doodla/iris/releases/latest (HTTP 403); using v0.1.0, the newest released version tag of https://github.com/doodla/iris
iris-install: installing iris v0.1.0 (x86_64-unknown-linux-musl) into /root/.local/bin
iris-install: verified SHA-256 of iris-v0.1.0-x86_64-unknown-linux-musl.tar.gz
iris-install: installed iris 0.1.0 (x86_64-unknown-linux-musl) to /root/.local/bin/iris
```

The `releases/latest` page returns HTTP 403 through this environment's proxy, so the git-tag
fallback was what worked. The `curl ... | sh` form was refused by this session's permission
classifier; the local copy, which was read in full first, was run instead.

### Signing [V]

None. `install.sh:47-49` and `docs/guides/install.md:88-101` say the checksum guards against
corruption, not a compromised release. No signatures or provenance attestations. macOS binaries
are not signed or notarized (`CHANGELOG.md`, 0.1.0 Known limitations).

### Version and changelog management [V]

- One source of truth: `version` in `Cargo.toml`. `Cargo.lock` is updated with a plain
  `cargo check` (`docs/contributing/releasing.md:11-12`).
- Cut steps: bump on a branch, move `[Unreleased]` entries to `## [X.Y.Z] - YYYY-MM-DD`, merge,
  wait for green CI on `main`, run Release by hand (covers macOS), then `git tag vX.Y.Z COMMIT` and
  push (`releasing.md:7-27`). Never move a pushed tag; release a new version instead.
- Pinned-by-hand versions (release Rust, cargo-about, parse-changelog, MSRV) each have one home and
  a table of where else they're named; `tests/release_pins.rs` fails when they disagree
  (`releasing.md:58-72`).

## 4. Docs taxonomy and style guide rules

These are condensed from `docs/contributing/style-guide.md:1-161` [V]. They bind Planroom's
README, `docs/`, `CONTRIBUTING.md` and `SECURITY.md`.

### Base and types

- Follow the Google developer documentation style guide (<https://developers.google.com/style>)
  where the house rules are silent.
- One type per page, each in its directory: Guide (`docs/guides/`, how do I do this task),
  Concept (`docs/concepts/`, how does this work and why), Reference (`docs/reference/`, what
  exactly does this do), Contributing (`docs/contributing/`, how do I change it).
- The README is the landing page: what it is, install, first result, links to everything.
- Start every page with one or two sentences on what it covers.
- Keep implementation details (locks, module names, retry classes) out of guides.
- End a guide with "What's next" links.

### One home per fact

- Each behavior is documented in one place; others link with at most one sentence of summary.
- The guide has a table of fact to home (commands to CLI reference, JSON to JSON reference, codes
  to errors reference, settings to configuration reference, guarantees to a concept page, rationale
  to Decisions). Planroom needs its own table.
- On a change: update the home, then search and replace restatements with links.
- Change the README only for install, quickstart, or a headline capability.

### Voice and tone

- "You", present tense, active voice, contractions.
- Say what it does and what the reader can do. State a limitation once, where needed, often in a
  note.
- Don't defend a design in a guide; link to a concept page or Decisions.
- No self-praise adjectives ("polished", "honest", "simple", "powerful").
- Save "never" for guarantees, stated in their home.

### Sentences and paragraphs

- One idea per sentence, 25 words or fewer.
- Paragraphs about five lines. Series to lists, comparisons to tables.
- Condition before instruction ("To resume a job, run ...").
- Few parentheses and semicolons, never nested parentheses.
- No bold for emphasis. Bold only for a list item's run-in heading.

### Word list

response (not answer, for a provider's reply); job ID, model ID; job record; state directory; dry
run (noun) and `--dry-run` (flag); "for example" and "such as" (not e.g.); "that is" (not i.e.);
"and so on" or a complete list (not etc.); Ctrl+C; provider (not vendor). Product names as their
owners write them. Commands in code format.

### Headings

Sentence case. Guide headings start with a verb; concept and reference headings are noun phrases.
No flags, paths or type signatures in headings, except a reference entry whose name is code.

### Procedures

Numbered, one action per step, each starting with a verb. Introduce with a sentence ending in a
colon. Show output the reader needs.

### Commands and output

- `sh` blocks without prompts. Output in a separate `text` block, introduced by "The output is
  similar to the following:".
- JSON formatted for reading, say once that it prints on one line, shorten with `"...": "..."`.
- Break commands over about 100 characters with a trailing backslash.
- `UPPER_SNAKE_CASE` placeholders listed under "Replace the following:", with the same names on
  every page.
- Capture every output from the current binary. Provider-calling commands are captured against the
  mock server, without mentioning the mock on user pages.

### Alerts

`> [!NOTE]` for limitations, `> [!IMPORTANT]` for billing and access, `> [!WARNING]` for data loss
or double cost. At most three per page, one or two sentences each.

### Links

Page title or a description as link text, never a file name or "here". Relative links inside the
repo. Files shipped in the release archive (`README.md`, `CHANGELOG.md`, `LICENSE`, `docs/`) link
to anything else with an absolute `https://github.com/OWNER/REPO/blob/main/...` URL. Link to
official docs, not third-party pages.

### Formatting

Wrap prose at 100 characters (not tables, URLs or code). Tables for reference data with more than
two attributes. Dates as `YYYY-MM-DD`. Don't write "currently", "today", "new" or "soon"; name the
version or date. Refer to releases by version.

### Enforced by tests [V]

- `tests/docs.rs`: every relative link and anchor resolves; archived files link outside the
  archive only with absolute URLs (`tests/docs.rs:161-221`); prose outside code has no `e.g.`,
  `i.e.`, `etc.`, no spaced em dash, and no "answer" in any form (`tests/docs.rs:250-276`). Exempt:
  the style guide, the generated CLI reference, `AGENTS.md`, `CLAUDE.md` (`tests/docs.rs:15-16`).
  Markdown files come from `git ls-files --cached --others --exclude-standard`, so ignored files
  and nested worktrees aren't checked. A self-test (`the_checks_catch_problems`) proves the checks
  fire.
- `tests/cli_reference.rs`: the CLI reference equals the help.
- `tests/schema_contract.rs`: the errors reference tables equal the code registry.

## 5. AGENTS.md structure and the CLAUDE.md import

`AGENTS.md` [V] opens with one paragraph: shared instructions for agents and humans; "invariants and
steering rules, not a description of the code" (`AGENTS.md:3-6`). Sections, each a flat bullet
list:

| Section | Content |
|---|---|
| Compatibility | What is public contract; additive vs breaking; schema regeneration enforced by a test; one-JSON-document rule; provider strings never in the taxonomy. |
| Providers | Adapter isolation; catalog declares capabilities with date and source; validate before network; never choose a model; honest subset; what "adding a provider" means. |
| Money, credentials, and safety | Credential sources; never retry an ambiguous paid request; generation vs download; credential origin pinning; never discard paid output; persist async jobs; don't log prompts. |
| Dependencies | Prefer maintained crates; hand-roll only with a reason in the commit; `cargo deny check`. |
| Tests | Offline, free, fake keys only; live tests opt-in and never in CI; test observable behavior. |
| Documentation | Follow the style guide; one home per fact; README rule; regenerate the CLI reference. |
| Working in this repository | Respect unrelated work; run `sh scripts/check.sh` before claiming success and say what was skipped; commit rules; never fabricate a git identity; never discard others' changes. |

`CLAUDE.md` is exactly `@AGENTS.md` (`CLAUDE.md:1`), the Claude Code import syntax. Planroom's
version [A]: put `@AGENTS.md` as line 1 of `CLAUDE.md`, then keep the existing "Keeping track
across compactions" and "Subagents" sections below it, and add `CLAUDE.md` and `AGENTS.md` to the
prose-check exemptions.

## 6. Public contracts: versioning and drift tests

### What is versioned [V]

- One integer, `schema_version` (`SCHEMA_VERSION: u32 = 1` at `src/output/envelope.rs:14`), is the
  major version of the whole public contract: commands, flags, exit codes, the JSON envelope,
  result types, the error object, and the code tables (`docs/reference/json-output.md:58-72`).
- Additive changes keep it. Renames, removals, or meaning changes increment it.
- Open sets: error codes, commands, warning codes, provider IDs and `billing` are
  `anyOf: [{enum: [known]}, {pattern}]`, so a newer minor's new value still validates; everything
  else is closed (`json-output.md:44-52`, `tests/schema_contract.rs:121-127`).
- Readers of unknown values: unknown error code by category and exit code, unknown warning as
  text. Old binaries reading newer job records map unknown codes to `internal_error` with
  `details.recorded_code` (`docs/reference/errors.md:384-390`).
- Persisted job records carry their own version (`JOB_RECORD_VERSION: u32 = 1`,
  `src/jobs/record.rs:51`); readers accept `<=` and reject newer; unknown fields are kept through
  `#[serde(flatten)] extra` (`src/jobs/record.rs:146-181`).
- The schema is generated by `schemars` from the serialized Rust types, printed by `iris schema`,
  committed as `schema/iris-output.v1.schema.json`, with `$id`
  `https://raw.githubusercontent.com/doodla/iris/main/schema/iris-output.v1.schema.json`.

### Drift tests [V]

| Test | Checks |
|---|---|
| `committed_schema_matches_the_generated_schema` | The committed file equals the generated one byte for byte, with the regenerate command in the failure message (`tests/schema_contract.rs:63-73`). |
| `the_schema_is_valid_and_enumerates_the_stable_codes` | Valid draft 2020-12; `$id`; `schema_version` const; enums equal the Rust registries (`ErrorCode::ALL`, `WarningCode::ALL`, and so on) and a fixed command list. |
| `open_sets_accept_later_values_of_the_documented_form_only` | Future values of the right form pass; malformed values and other `schema_version`s fail. |
| A run of every command in JSON mode | Every real output validates against the envelope and its command's `$defs` type (`tests/support/schema.rs:73-88`). |
| `the_schema_alone_rejects_envelopes_that_break_the_contract` | The schema itself, without helper code, encodes `ok` vs `result`/`error` and category per code. |
| `the_documented_error_table_is_the_error_codes`, `the_documented_warning_codes_are_the_registry` | The Markdown tables in `docs/reference/errors.md` equal the registries both ways. |
| `warnings_are_only_built_from_the_registry` | Source scan: no file but the registry spells a warning code. |
| `tests/cli_reference.rs` | `docs/reference/cli.md` equals clap's rendered long help, and one section equals the real binary's output in a cleared environment. Regenerate with `IRIS_UPDATE_DOCS=1`. |

### Planroom mapping [A]

Planroom's contracts: the CLI and `--json` envelope, the local HTTP API, the graph JSON (nodes,
edges, C4 levels, Intent), the SQLite store schema and its migrations, the mod to engine protocol,
the Artifact `db` event format, and the plugin manifest. For each: one integer version, a schema
generated from the Rust types, committed under `schema/` with a `vN` file name, a byte-equality
test, a test that real outputs validate, and a TS check that the TS types are generated from (or
validated against) the same committed file. SQLite needs a migration test that opens every older
fixture database.

## 7. Worth copying

All [V] in the cited files.

- **One local command equals CI** (`scripts/check.sh`): never stops at the first failure, says
  what it skipped and how to install it, `--fix` regenerates generated files.
- **Generated docs with an update env var** (`IRIS_UPDATE_DOCS=1`) and failure messages that print
  the exact fix command.
- **Docs style enforced by tests**, with a self-test that the checks catch planted problems, and
  the file list from `git ls-files` so worktrees don't leak in.
- **Pin registry with a test** (`tests/release_pins.rs`): every hand-pinned version has one home,
  and the test fails when another file names a different one. CI reads the pin from the one
  file with `sed` rather than repeating it.
- **Weekly pin-age check** for what Dependabot can't bump.
- **Release dry run in CI on every change**, building with the release toolchain and packaging
  twice to prove reproducibility, then smoke-testing the real installer against a `127.0.0.1`
  server.
- **Least-privilege publishing**: the parse step has a read-only token and passes notes as an
  artifact; the write-token job runs no extra tools; every `${{ }}` value reaches scripts through
  `env:`; `persist-credentials: false` everywhere; `install-action` with `fallback: none`.
- **Fail-closed asset set**: publish only when the archive set equals the expected list.
- **Release notes from the changelog**, with a blank section failing the tag push before any
  build. Changelog links are absolute so they work on the GitHub release page.
- **Installer hardening**: `exec </dev/null`, checksum line must be unique and well-formed, tar
  layout allow list, atomic rename, no `sudo`, git-tag fallback when `releases/latest` is blocked
  (exercised here, section 3), refusal of a non-verifying BusyBox `wget`.
- **Commit identity from an explicit variable** (`build.rs`), never `git` or `GITHUB_SHA`.
- **`cargo package --list` allow list** in CI so a crate never ships stray files.
- **Decisions page** with sources and "checked on" dates, and a live-testing page with a dated
  run log and a cost ledger.
- **PR template** that asks for contract changes and which checks ran, with "only if" conditions.

## 8. Iris for Planroom's art

### Setup [V]

- Installed: `iris 0.1.0 (x86_64-unknown-linux-musl)` at `/root/.local/bin/iris` (section 3).
- `iris doctor` printed `[ok]` for config, both credentials, state dir, output dir, both base URLs
  and jobs, then `Healthy.`, exit 0. Account access isn't checked by doctor.

### Image models [V] (`iris models list`, `iris models show MODEL`)

| Model (alias) | Sizes | Transparent background | Standard 1024x1024 price |
|---|---|---|---|
| `gpt-image-2.5-sunburst` | `auto` or WxH: multiples of 16, max edge 3840, ratio up to 3:1, 655,360 to 8,294,400 px; above 2560x1440 is experimental; recommended 1024x1024, 1536x1024, 1024x1536 | Yes, `-O background=transparent` with png or webp | low $0.00588, medium $0.01317, high $0.05268, xhigh $0.09366, max $0.21072 |
| `gpt-image-2.5-flare` | Same as Sunburst | Same | Same token rates as Sunburst |
| `gpt-image-2` | Older; OpenAI says use 2.5 | (not checked) | low $0.00588, medium $0.05268, high $0.21072 |
| `gemini-3.1-flash-image` (`nano-banana-2`) | `--aspect-ratio` 1:1, 1:4, 1:8, 2:3, 3:2, 3:4, 4:1, 4:3, 4:5, 5:4, 8:1, 9:16, 16:9, 21:9; `--resolution` 512, 1K, 2K, 4K | No option; outputs jpeg or png | 512 $0.045, 1K $0.067, 2K $0.101, 4K $0.151 |
| `gemini-3.1-flash-lite-image` (`nano-banana-2-lite`) | 10 ratios (no 1:4, 1:8, 4:1, 8:1); 1K only | No | $0.0336 |
| `gemini-3-pro-image` (`nano-banana-pro`) | 10 ratios; 1K, 2K, 4K | No | 1K or 2K $0.134, 4K $0.24 |

All estimates exclude prompt and input-image tokens, and Gemini's thinking tokens. Prices are
Iris's catalog as of 2026-09-24.

### Dry-run costs [V]

Each ran `iris --json image generate ... --dry-run`; nothing was sent or charged, and no file was
written.

| Use | Command options | Estimate (USD) |
|---|---|---|
| Logo | `-m gpt-image-2.5-sunburst --size 1024x1024 --quality high -O background=transparent --format png` | 0.05268 |
| README hero | `-m gpt-image-2.5-sunburst --size 1536x1024 --quality high` | 0.04116 |
| README hero, wide | `-m gpt-image-2.5-sunburst --size 2560x1440 --quality high` | 0.05529 |
| README hero, wide | `-m nano-banana-2 --aspect-ratio 21:9 --resolution 2K` | 0.101 |
| Social preview | `-m gpt-image-2.5-sunburst --size 1280x640 --quality high` | 0.02436 |
| Social preview draft | `-m gpt-image-2.5-flare --size 1280x640 --quality medium` | 0.00609 |
| Empty-state art | `-m gpt-image-2.5-flare --size 1024x1024 --quality medium -O background=transparent` | 0.01317 |
| Comparison | `-m nano-banana-pro --aspect-ratio 16:9 --resolution 2K` | 0.134 |
| Comparison | `-m nano-banana-2-lite --aspect-ratio 1:1` | 0.0336 |

The full JSON plan for the social preview, shortened:

```json
{"command": "image.generate", "ok": true, "schema_version": 1, "warnings": [],
 "result": {"dry_run": true, "billing": "paid", "model": "gpt-image-2.5-sunburst",
  "options": {"size": "1280x640", "quality": "high", "format": "png", "background": "auto", "...": "..."},
  "cost_estimate": {"amount": 0.02436, "currency": "USD", "estimated": true, "as_of": "2026-09-24",
   "basis": "estimate: 1 image × 812 output tokens × $30.00/1M (gpt-image-2.5-sunburst, high, 1280x640); ..."},
  "max_cost": 0.05, "prompt_fingerprint": {"chars": 32, "sha256": "df8330e0..."}, "...": "..."}}
```

Other verified behavior: `--max-cost 0.01` on that request exits 2 with
`error[cost_limit_exceeded]`, and `--size 1280x630` exits with `invalid_argument` because edges
must be multiples of 16.

### Recommendations [A]

- **Logo and empty-state art:** GPT Image 2.5 with `-O background=transparent`. The Gemini models
  have no background option, so their output needs manual cut-out.
- **Social preview:** generate at exactly 1280x640 on GPT Image 2.5. No cropping needed. Keep the
  wordmark out of the prompt and overlay it as SVG text afterwards, so the text is exact.
- **Hero:** Sunburst at 1536x1024 or 2560x1440 high, or `nano-banana-2` at 21:9 2K for a wider
  banner, cropped after.
- **Draft cheaply, finish once:** Flare at medium (about $0.006 to $0.013) for iterations, then
  Sunburst high for finals.
- **Budget:** about 3 logo finals ($0.16), 3 hero finals ($0.17), 3 social finals ($0.07), and 8
  empty states with one retry each ($0.21), plus drafts, stays under $1, against KICKOFF's $5 cap.
  Pass `--max-cost` on every call. The pre-call estimate leaves out prompt tokens ($5.00 per 1M
  text input tokens for GPT Image 2.5), which is small for short prompts.
- **Prompts next to images:** store each prompt as a file and pass it with `-f PROMPT_FILE`, so the
  committed file is exactly what was sent. Record the model, options, the `prompt_fingerprint`
  sha256 and the reported cost from the `--json` output in a small manifest beside the images. Add
  the image directory to the `.gitignore` exceptions, as Iris does with `!/docs/assets/**`.

## Recommended decisions [A]

1. Copy Iris's CI shape job by job, then add TS, e2e, plugin, guardrails and npm-audit jobs. Trigger
   on every branch push, with Iris's concurrency rule.
2. Copy the release pipeline: `verify` with parse-changelog 0.6.17, `checks`, native `build` legs
   for the same four targets, reproducible packaging with cargo-about 0.9.2, `SHA256SUMS`, and a
   fail-closed `publish`. Add the plugin and the web bundle as further assets.
3. Make the plugin's first-use engine fetch a port of `install.sh`'s verification steps, tested by a
   copy of `tests/installer/`, including the git-tag fallback.
4. One versioned schema per contract under `schema/`, generated from Rust, with byte-equality and
   validate-real-output tests, and TS types checked against the same files.
5. Port `tests/docs.rs`, `tests/cli_reference.rs` and `tests/release_pins.rs`, and write
   `scripts/check.sh` early, so the docs and pins are held to the bar from the walking skeleton on.
6. Add `CODE_OF_CONDUCT.md`, which Iris lacks.

## Top risks [A]

1. **The engine download path.** Planroom's plugin must fetch, verify and install a binary at first
   use. `releases/latest` returned HTTP 403 through this environment's proxy, and only Iris's
   git-tag fallback worked. A fetcher without that fallback, or without layout and checksum
   checks, fails here or falls below the bar.
2. **More contracts across two languages.** Iris has one schema and one language. Planroom has
   several contracts shared by Rust and TS; without generated, committed schemas and drift tests on
   both sides, they drift silently.
3. **Docs volume under enforced style.** Iris's docs are checked by tests and captured from the real
   binary. Writing Planroom's docs late, or without porting the checks, misses the bar. macOS legs
   are also only testable in the Release workflow, so a manual dry run is needed before tagging.

## Not verified

- The GitHub-hosted behavior of any workflow (no Actions run was observed).
- `cargo deny`, ShellCheck and cargo-about aren't installed here (`cargo deny --version` and
  `cargo about --version` fail, `shellcheck` isn't found), so Iris's local checks weren't run.
- Account access and real image generation: no paid request was sent.
- GitHub's recommended social preview size of 1280x640 is from KICKOFF, not checked here.
