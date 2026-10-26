# Planroom build progress

The full record of the build that `KICKOFF.md` describes. A later session should be able to resume from this file plus `.claude/carry.md`.

- Started: 2026-10-25, branch `ccr-b124f0e3-xnzjca`.
- Prompt snapshot: `examples/kickoff-snapshot.md` (sha256 `6fcb9232…1879`, identical to root `KICKOFF.md` at kickoff).

## Status

| Phase | State |
|---|---|
| 1. Understand | research done; mod and Artifact spikes in main session |
| 2. Architecture | done: 3 candidates, 3 judges, synthesis; main agent decided DR-43 |
| 3. Architecture review | done: 3 rounds (49, 43, 29 material findings), all applied or accepted as risk |
| 4. Walking skeleton | WS1 to WS8 merged and live (scan, store, mod, canvas, Artifact); DR-81 to DR-84; review running |
| 5. Milestones M1–M5 | not started |
| 6. Release | not started |

## Assumptions

Reasonable defaults taken instead of asking (per `<autonomy>`).

- A1. The build container has 4 CPUs, so workflows run at most 2 agents at once. Phases use fewer, larger agents.
- A2. `just` isn't installed; the task runner choice is part of the architecture (a `cargo xtask` needs no extra install).
- A3. KICKOFF's verified facts are wrong on two plugin details for Claude Code 2.1.310: `claude plugin install` has a `--marketplace` flag, and a `bin/` folder passes local validation and install (only claude.ai and Cowork sync refuse it). Planroom still ships no `bin/`, and S14 uses the two commands as written. Evidence: `docs/contributing/research/plugin-packaging.md`.
- A4. "At most 64" characters for a plugin name isn't enforced locally; Planroom's name `planroom` is well inside it either way.

## Pre-mortems

### Phase 1 (Understand)

The five most damaging findings a final review could make about this phase, and how they're prevented:

1. **Designed around a mod or plugin capability that doesn't exist.** Prevented by reading the `plugin-authoring` skill's types and asking `claude-code-guide` how plugins declare skills, agents, hooks and MCP servers, before any design.
2. **Artifact `db` sync misunderstood** (size limits, who can write, how edits are noticed), so S11 fails late. Prevented by reading `artifact-capabilities` and spiking a published page in the walking skeleton.
3. **Jev thresholds with no basis**, so S7's precision ≥ 0.95 on 60 cases is unreachable or meaningless. Prevented by a live latency and calibration spike on realistic plan-node slices, with recorded fixtures.
4. **Libraries that don't build here or in CI** (tree-sitter grammar ABI mismatches, native git or TLS builds, slow compiles). Prevented by a Rust spike that builds the candidate stack and times parsing.
5. **Iris's release bar studied superficially**, so files and workflows are missing at release. Prevented by a structured checklist from the Iris repo, kept in `docs/contributing/research/iris-release-bar.md`.

## Phase 1 findings

Each note under `docs/contributing/research/` separates verified facts from docs-only claims.

| Topic | Finding that shapes the design | Note |
|---|---|---|
| Release bar | Iris: 9 CI jobs, reproducible release packaging, installer with git-tag fallback (`releases/latest` 403s through this proxy), docs style enforced by tests, one integer contract version with byte-equality drift tests. No art and no code of conduct in Iris. | `iris-release-bar.md` |
| Mod API | One hooks module, no `import()`, no Node; Svg off the terminal only, Raster on it; `http.fetch` has a socket path but no streaming or timeout; spawned children die on reload; `$.agent.spawn` has no isolation (it's on the agent type); tools appear as `mcp__planroom__*`, deferred by default. | `mod-api.md` |
| Plugin packaging | `CLAUDE_PLUGIN_DATA` (`~/.claude/plugins/data/planroom-planroom/`) holds the fetched engine; local-path marketplaces load live from the source folder; validate has blind spots (doesn't descend into `./plugin`, misses bad agent model or effort). | `plugin-packaging.md` |
| Artifact | `db` is last-writer-wins page-side with live `onSnapshot`; the session writes with `if_version`; `{self}` paths make authorship server-enforced; db writes don't wake the session; bulk data fits better as a published data file. | `artifact.md` |
| Jev | p50 about 300 ms round trip here (65 ms model time), about $0.000019 a request; 60/60 easy cases right, but ambiguous cases come out confident, so the card band must be wide and S7 thresholds come from held-out labels. One request per slice, many questions per request. | `jev.md` |
| Claude on the gateway | `/v1/messages` with `cache_control` works; cache-hit rate is `cache_read / (input + cache_creation + cache_read)`; cost is in the body; pin `anthropic/claude-sonnet-5.5` and `anthropic/claude-haiku-5.5`; send `only: ["anthropic"]` to stop cross-provider retries. | `claude-gateway.md` |
| Rust stack | Everything builds without OpenSSL; tree-sitter 0.27 about 690 files/s; gix and git2 both fine; rusqlite and sqlx can't coexist; schemars to JSON Schema to json-schema-to-typescript is the contract flow. | `rust-stack.md` |
| TS stack | ELK takes 1.1 to 1.5 s at 200 nodes, so layout runs in a worker and positions persist in the engine; pan and zoom p95 16.8 ms at 200 nodes; 215 KB gzip first paint; dependency-cruiser beats eslint-plugin-boundaries; elkjs is EPL-2.0. | `ts-stack.md` |

Phase 1 live spend: Jev $0.00517 (282 calls), Claude on the gateway about $0.022.

### Phase 2 (Architecture)

1. **A candidate built on an unmeasured bet.** Prevented by giving every architect the measured research and requiring each bet to name its proof and fallback (the synthesized design's "Biggest bets" table).
2. **A scenario with no deterministic CI path.** Prevented by the judges' requirements lens scoring each tier A and B scenario's CI path, and `feature_list.json` naming one per scenario.
3. **Constraint violations hidden in grafts** (the simplicity candidate dropped `$.model.complete`). Prevented by judges listing fatal flaws and the synthesis fixing each (DR-18).
4. **Docs that contradict each other.** Prevented by one synthesizer writing all six deliverables, then the Phase 3 consistency checks.
5. **A layout agents can't navigate.** Prevented by the R16 lens, `repo-layout.md`'s area contracts and the "where new code goes" table.

### Phase 3 (Architecture review)

1. **Reviewers anchored on the synthesis's framing.** Prevented by giving reviewers the documents and the brief only, never the synthesis notes or my reasoning, with one distinct lens each.
2. **Real problems refuted away, or noise kept.** Prevented by a separate refuter per lens that must cite evidence to refute, and by materiality defined exactly as KICKOFF defines it.
3. **Fixes applied in one doc but not its other homes.** Prevented by a fix pass that edits every home a fact has (architecture, decisions, contracts, layout, traceability, feature list), then a consistency check.
4. **Lenses voting each other down.** Prevented by judging each finding within its own lens; a security finding never needs the UX reviewer's agreement.
5. **Unbounded review.** Capped at 3 rounds; what's left is recorded as accepted risk with a one-line reason.

### Phase 4 (Walking skeleton), written at the phase's end

This pre-mortem was owed before the phase began and wasn't written then; it is recorded here late, with what the phase did against each risk.

1. **Tracks that pass alone and fail together.** Met: every seam bug (the person-token encoding, the mod's plugin root, the skill named like the mod's command, `scan`'s undo class) showed only when the pieces ran together, so each merge ran the full suite and the real mod ran live.
2. **Mocked harnesses hiding the host.** Met late: the mod harness mocked the environment and command registration, so two start-up failures reached the live session. `packages/mod/AGENTS.md` now says a start-up change isn't proven until the bundle has run live.
3. **CI-only failures.** Met: cargo-deny, a BusyBox that prefers its applets and macOS paths broke CI after local runs passed; fixed, and CI is read through the GitHub tools after every push.
4. **Disk and CPU exhaustion from parallel worktrees.** Met twice (a full disk mid-build): worktree build directories are deleted after each merge, and M1 shrinks debug info.
5. **Docs drifting from the code the skeleton built.** Met: every track's doc conflicts went into DR-81 to DR-84 and a docs fold, and a review lens checks drift.

### Phase 5, M1 (kernel, store, code and plan packs, mod and canvas basics)

1. **The fold and the packs land out of order, so tracks build on interfaces that change.** Prevented by waves: the kernel fold and IDs (K) and the extractors and linkers (X) first; packs and the plan pack (P) on top of both; the mod (M), the canvas (C), actors and backups (A) beside them on the contract.
2. **S1 fails on truth because the fixture's map is polluted or IDs drift.** Prevented by default excludes for fixture and test-data trees, the ID-stability test, and the truth comparator running in CI from the first pack merge.
3. **A track breaks a gate it can't see from its worktree.** Prevented by `cargo xtask check layout`'s pending ratification in worktrees, the merge-track skill, and a decision record in every merge that changes a gated surface.
4. **Live-only failures in the mod and canvas.** Prevented by running the bundle live from the dev-mods folder and driving the canvas with Playwright at both sizes and themes after each surface merge.
5. **Estimate overrun.** M1 is 119 agent-hours (115 before the skeleton review scheduled its unbuilt gates and the filesystem check). At twice that, finish its tier A items, record the rest and move on (KICKOFF).

## Effort estimates

From the synthesized architecture, in agent-hours of one focused build agent. Per KICKOFF, at twice a milestone's estimate, finish its tier A items, record the rest, and move on.

| Milestone | Estimate | Twice | Actual |
|---|---|---|---|
| Walking skeleton | 28 h | 56 h | about 15 to 20 agent-hours of wall-clock across tracks, plus the main session's merges and live checks (rough, from workflow durations) |
| M1 kernel, store, code and plan packs, mod and canvas basics | 119 h | 238 h | |
| M2 diffs, reconciliation, hand-off, Jev decisions | 92 h | 184 h | |
| M3 multi-repo, observability, release | 30 h | 60 h | |
| M4 artifacts, state machines, anatomy | 35 h | 70 h | |
| M5 context curation, polish, release | 76 h | 152 h | |
| Total | 380 h (280 h before the review, 333 h after round 2, 376 h before the skeleton review) | | |

## Spend log

| Phase | Agent tokens (rough) | Live model spend |
|---|---|---|
| 1. Understand (research + fixture) | about 3.1M subagent tokens | Jev $0.005, Claude gateway $0.022, Iris $0.27 |
| 2. Architecture | about 3.1M subagent tokens | none |

## Log

- 2026-10-25: Read `KICKOFF.md`, `CLAUDE.md`, `.claude/guardrails/`. Snapshot copied. Phase 1 research workflow started.
- 2026-10-25: Fixture built from the `examples/README.md` contract (5 repos, 9 branches, deterministic heads, 191 evidence items). Truth review round 1: 0 blockers, 4 material, 6 minor. The material ones were unstated rules (direct vs indirect alert coverage, which edges a path follows, the impact-radius rule, and S4 compose evidence surviving a worker-only patch). Resolution: the contract now states each rule, and the S4 hand-off is multi-repo (one patch per touched repo), which also makes the implementer contract general. Round 2 is running.
- 2026-10-25: Spikes. Mod: dock is 49x34 at 120x40 and 87x46 at 200x50; a bare digit at the empty prompt presses the band's Button; `/room` then one hotkey drives the pane (`docs/contributing/research/mod-spike.md`). Artifact: multi-file publish with a module script and data file accepted; access rules hold under `as_level` (`docs/contributing/research/artifact-spike.md`). Page-side numbers arrive when the probe is first opened. Design language set: a planning table with toy blocks, chalk intent outlines, yarn lenses, ghost blocks, decision cards (`docs/contributing/design-language.md`).
- 2026-10-26: Truth review round 3 (the last allowed): 4 material (the S4 radius mixed before and after states; the radius rule omitted API specs; the process machine added a `done` state; language and component granularity unstated). All fixed by the main agent and verified by `examples/check-fixture` (0 failures). Accepted risk: the round-3 fixes were not re-reviewed by an independent agent, because the review protocol caps rounds at 3; `check-fixture` re-derives the radius, evidence lines and patch behavior mechanically. **TRUTH.json is frozen from this commit; later changes need a decision record.**
- 2026-10-26: Art made with Iris (logo, hero, social base, three empty states), about $0.27, prompts beside the images in `docs/art/`.
- 2026-10-26: Assumptions settled during the Phase 3 review (to fold into `architecture.md` after it):
  - A-1: a plugin skill with `user-invocable: false` is absent from `slash_commands` and from the init `skills` list, yet the model still lists and invokes it with the Skill tool (headless `claude -p` with a spike plugin, about $0.012).
  - A-2: `tool.check` hooks may return `ask` with a `reason`; a hook's ask goes to a person, and where nobody can be asked it becomes a deny with that reason (`ToolCheckResult` in the 2.1.310 types).
  - A-3: the Bash tool runs with stdin and stdout not on a TTY.
  - A-4: settled earlier (string or content blocks).
  - A-8: `tool.call` inputs carry `tool_use_id` (spike diagnostics).
  - A-11: a cloud session can start with no surface.
  - A-9: `$.model.complete` caching works in this session: the first call wrote 14,403 cache tokens, the second read 14,403 (`mcp__planroom-spike__spike_probe what=model`, haiku, 8 output tokens).
  - A-10: `$.process.run` works in this cloud session (exit 0, no TTY, git 2.43.0), and `$.process.spawn` streams stdout as written (chunks at 6, 310 and 624 ms).
- 2026-10-26: Architecture review round 1: nine lenses with independent refuters gave 102 findings (49 material and upheld, 5 downgraded, 0 refuted). Owner decisions D1 to D21 are in `docs/contributing/research/arch-review/round-1/owner-decisions.md`; the fixes run as three sequential clusters plus a consistency pass.
- 2026-10-26: Architecture review round 2: fewer, deeper findings (data_version misses a connection's own commits; derived idempotency keys replay legitimate repeats; per-repo scan leases race on the link snapshot; the pack set was per-process). Owner decisions E1 to E17, fixes as DR-61 to DR-71, owner follow-ups as DR-72 (ECDSA person tokens so the store holds no signing key; Git LFS filters allowed; S7 fast-forwards). The estimate grew to 333 agent-hours as the design closed real gaps (M1 93 h, M2 82 h, M5 67 h).
- 2026-10-26: ENGINE track merged (WS3 and WS4). Bet 1 holds: 3 writers x 500 commits at about 1,650 commits/s, 0 `SQLITE_BUSY`, commit p99 about 2 ms (a 632 ms busy-back-off tail), `watch` commit-to-frame p99 about 195 ms, own commits under 4 ms through the commit hook, kill-after-commit retries replay. Details in `docs/contributing/research/ws4-concurrency.md`. Implementation notes to fold into the docs after the round 3 fix pass: pins at least 14 days old (rusqlite 0.40.2, sha2 0.11.0, uuid 1.28.0, saphyr-parser 0.1.0); `events.reply` column for DR-8's stored reply; scan keys get `#after:<seq>` so A->B->A->B moves don't repeat a key; the walk reads the working tree but evidence is checked against HEAD (dirty lines dropped); directories, repos and empty files carry `Evidence::Ref`; a `code.depends_on` hint for compose `depends_on`; repo node ID `<repo>:repo:<repo>`; `serde_json` in store and `uuid`, `toml` in planroom beyond the area allow-lists. Not built yet: `blob_facts`, `fact_rev`, pre-migration `VACUUM INTO`, the WAL-unsafe filesystem check, `packs_mismatch`, field-level compare-and-set, link and reconcile steps.
- 2026-10-26: PLUGIN track merged after resolving the xtask command table and updating its golden envelope to the engine's `Meta`; `cargo xtask check` (22 steps, including `claude plugin validate --strict` on `.` and `plugin/`) and `cargo xtask test` (cargo, vitest 35, `claude plugin test target/mod-test` 4 of 4) pass; installer suite 15 of 15 under dash (ShellCheck and BusyBox not installable here; CI runs them). Notes to fold in: the host loads a hooks module only when `register` is exported on its own declaration, so xtask rewrites esbuild's export list; plain `gen` and `gen --mod` both build the bundle; the mod's fallback strings are hand-written until `en.toml` exists; the contract marks `scan` NotUndoable and `CommandSpec` lacks `spends`; the engine atom type lives in xtask; generated depcruise rules let only the mod import `claude-code` and only the mod's tests read golden JSON (a boundary change to ratify in a DR); `no-misused-promises` is off for `register.tsx` and the plugin tests (eslint took 11 minutes on the mod API types; tsc still checks them); the plugin carries `AGENTS.md`, not `CLAUDE.md` (validate `--strict` warns on it); resolver reasons `override_missing` and `no_build`; the 2.1.310 `claude-code.d.ts` is vendored in `packages/mod/vendor/`. Next engine work: `version --json`, `focus`, `schema --commands`, the string catalog.
- 2026-10-26: **Phase 3 closed.** Three review rounds (9 lenses each, an independent refuter per lens): 102, 95 and 75 findings, of which 49, 43 and 29 were material and upheld; round 3's fixes added DR-73 to DR-80. Accepted risks, signed off by the main agent with their reasons in `docs/contributing/research/arch-review/round-3/accepted-risk.md`: unasked-4 (a single-repo store travels with a copied `.git`), security-1 (approved checks run implementer-written code inside its sandboxed clone), security-2 (a model-republished page can read the pairing key; forged rows are attributed, undoable edits at worst), security-4 (re-keying by a press stays a guardrail between processes of one OS user), plus the partly applied ux-4 and cost-1. Known gaps per round are in each round's `known-gaps.md`.
- 2026-10-26: ENGINE-2, CANVAS and MOD-2 merged (e148109, 945d291, ff12c02), each verified with `cargo xtask check` and `cargo xtask test` on the merged tree. Every golden under `tests/golden/cli/` now comes from a real engine run; the canvas's and mod's stand-in generator (`crates/xtask/src/golden.rs`) is gone, and the golden test also writes `map-empty.json`. **The merge found a real seam bug:** the mod signed person tokens with `at` in milliseconds and without `--idem`/`--base` in `args`, while the engine reads seconds and signs every word but `--json`; each side's tests passed alone. Fixed in the mod and pinned by a shared byte example on both sides (DR-82). A-17 fails (`crypto.subtle` has only `digest`); the mod signs with `@noble/curves` 2.4.0. Disk filled once (worktree `target/` dirs, 14 GB); worktree build dirs are now deleted after each merge. Integration running: WATCH (`watch --json`, a real `watch-frames.json`, a real-binary test of the mod's frame parser) and REAL-SERVE (the canvas's e2e against `planroom serve`; the same-origin GET, CSP and route seams). Found while merging and queued for the skeleton's end: the repo gates trail `repo-layout.md` (no `third_party` allow-lists, per-crate network allow-list or `check layout`/`layout.lock`); `share` and `import` and the canvas's Artifact transport don't exist yet, so WS7 needs its own slice.
- 2026-10-26: Integration merged: WATCH (`watch --json` replays from `--since`, stops on a newer store, `reset` frames; a real `watch-frames.json`; the mod's loop fed by the real binary) and REAL-SERVE (the canvas's e2e on the real `planroom serve`; same-origin GETs via `Sec-Fetch-Site`; the exact CSP; `POST /api/v1/code`; overlays named by milestone for the M1 and M2 states). `cargo xtask check`, `test` and `e2e` all pass on the merged tree. DR-82 extended with the serve and stream seams. WS8 so far (`docs/contributing/research/ws8-install.md`): the local install works; A-20 settled (`git-subdir` with `ref` installs exactly `plugin/` at a tag; a `github` source ignores `path`); A-18 holds (a spawned implementer's Bash under the session's sandbox writes only where allowed and has no network; the sandbox is a session setting, readable by the mod); A-16 holds (the guards are now in `.claude/settings.json`); A-15 found the `$.store` file under `plugins/store/`, which MOD-2's interim guard had exempted: fixed; A-14 and A-19 settled from the live mod; A-23 fails, so the `-dev` version gets a content-hash stamp. First-turn usage varies by more than the plugin's surface, which confirms DR-51's `count_tokens` gate. Spend logs seeded; $0.32 logged so far, all session usage or Phase 1.
- 2026-10-26: GATES and REMOTE merged (b0a1b6c, b45758e), each verified with `cargo xtask check`, `test`, `test --installer` and `e2e`. GATES built the documented repo gates: `boundaries.toml` in full with generated `clippy.toml`s and depcruise allow-lists, 18 planted crossings, `check layout` and `gen --layout-lock` (the first `layout.lock` names DR-79, written in the merge commit), `ci_skips.toml` with CI installing its tools, the area contract blocks, the art spend check, and `0.1.0-dev` with a content-stamped plugin version. REMOTE built `remote.rs`, `share --out`, `import -` and the canvas's Artifact transport. CI's first full runs on the merged tree found what local runs couldn't (tokio's RUSTSEC-2026-0386, BSD-3-Clause, unversioned path dependencies, a BusyBox that prefers its applets, macOS's `/private/var` and slow `python3`); all fixed, and run 43 was green but for one macOS installer test, fixed in 2792488. WS7 ran on the real platform (`docs/contributing/research/ws7-remote.md`): `share`'s exact publish arguments published https://claude.ai/artifact/69VbWfb55J2p6kduVUJcG8; a read request row went through `ArtifactData list`, the real `planroom import -` and one atomic batch (answer, ack, delete, `live/head` at version 2). A-13 fails (the owner's session can write any viewer's inbox; a viewer can't), so every unsigned row waits for the desk. WS8 spikes: `tool.check` carries a spawned agent's `agentId`; `ArtifactData` rows arrive in the text block. DR-83 records it all. `scan`'s undo class now follows DR-81. Dogfooding: scanning this repo maps the fixture's compose services as containers (M1 needs default excludes), and a single-repo store landed in `.planroom/`, not `.git/planroom/` as DR-74 requires (M1). The real plugin is linked into the dev-mods folder to run live in this session.
- 2026-10-26: **Walking skeleton review, round 1.** Eight lenses, each refuted by a separate verifier: 94 findings, 18 material (17 distinct defects), 75 minor known gaps, 1 refuted (`docs/contributing/research/skeleton-review/`). Fixed by four worktree tracks and the main agent, all merged: SCAN (`89c1bd4`: scan reads HEAD's tree through `ls-tree` and `cat-file --batch`, so a dirty first scan no longer freezes a wrong map), CONTRACT (`90476d3`: open event kinds, `extra` on persisted documents, one format check with a TS port, the `workspaces.json` lock, EngineState/ToolResult/ResolverStatus in the contract, a generated `contract.sh`), ENGINE (`0297970`: usage errors as one envelope, `map` within its budget, no blank or failing empty state, `kernel::display::sanitize`, the envelope's `summary`), SURFACES (`330a430`: every pane button works, `/room` shows the summary line, the guard reads shell forms, the phone's way back up) and the main agent (`24c6bac`: the source build builds the canvas, the install line, `check`'s `not built` lines; `20179de`: M1 schedules the unbuilt gates and the filesystem check, 119 h). **Seam bugs at merge:** the resolver change spelled `no_toolchain` itself, which CONTRACT's new test forbids; xtask's `contract.sh` generator lacked ENGINE's new `summary` field. DR-85 records every doc conflict; `layout.lock` ratifies `contract.sh` as generated. Disk: 8 GB of Phase 1 scratch builds deleted. New project skill `review-round` (its eval passed 5 of 5 with and without it, so a sharper case is owed).
