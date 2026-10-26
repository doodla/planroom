# Planroom build progress

The full record of the build that `KICKOFF.md` describes. A later session should be able to resume from this file plus `.claude/carry.md`.

- Started: 2026-10-25, branch `ccr-b124f0e3-xnzjca`.
- Prompt snapshot: `examples/kickoff-snapshot.md` (sha256 `6fcb9232…1879`, identical to root `KICKOFF.md` at kickoff).

## Status

| Phase | State |
|---|---|
| 1. Understand | research done; mod and Artifact spikes in main session |
| 2. Architecture | done: 3 candidates, 3 judges, synthesis; main agent decided DR-43 |
| 3. Architecture review | round 1 running |
| 4. Walking skeleton | not started |
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

## Effort estimates

From the synthesized architecture, in agent-hours of one focused build agent. Per KICKOFF, at twice a milestone's estimate, finish its tier A items, record the rest, and move on.

| Milestone | Estimate | Twice | Actual |
|---|---|---|---|
| Walking skeleton | 26 h | 52 h | |
| M1 kernel, store, code and plan packs, mod and canvas basics | 66 h | 132 h | |
| M2 diffs, reconciliation, hand-off, Jev decisions | 68 h | 136 h | |
| M3 multi-repo, observability, release | 30 h | 60 h | |
| M4 artifacts, state machines, anatomy | 34 h | 68 h | |
| M5 context curation, polish, release | 56 h | 112 h | |
| Total | 280 h | | |

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
