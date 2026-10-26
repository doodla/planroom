# Planroom build progress

The full record of the build that `KICKOFF.md` describes. A later session should be able to resume from this file plus `.claude/carry.md`.

- Started: 2026-10-25, branch `ccr-b124f0e3-xnzjca`.
- Prompt snapshot: `examples/kickoff-snapshot.md` (sha256 `6fcb9232…1879`, identical to root `KICKOFF.md` at kickoff).

## Status

| Phase | State |
|---|---|
| 1. Understand | research done; mod and Artifact spikes in main session |
| 2. Architecture | not started |
| 3. Architecture review | not started |
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

## Effort estimates

Recorded per milestone at the end of Phase 2.

## Spend log

| Phase | Agent tokens (rough) | Live model spend |
|---|---|---|
| 1. Understand (research + fixture) | about 3.1M subagent tokens | Jev $0.005, Claude gateway $0.022, Iris $0.27 |

## Log

- 2026-10-25: Read `KICKOFF.md`, `CLAUDE.md`, `.claude/guardrails/`. Snapshot copied. Phase 1 research workflow started.
- 2026-10-25: Fixture built from the `examples/README.md` contract (5 repos, 9 branches, deterministic heads, 191 evidence items). Truth review round 1: 0 blockers, 4 material, 6 minor. The material ones were unstated rules (direct vs indirect alert coverage, which edges a path follows, the impact-radius rule, and S4 compose evidence surviving a worker-only patch). Resolution: the contract now states each rule, and the S4 hand-off is multi-repo (one patch per touched repo), which also makes the implementer contract general. Round 2 is running.
- 2026-10-25: Spikes. Mod: dock is 49x34 at 120x40 and 87x46 at 200x50; a bare digit at the empty prompt presses the band's Button; `/room` then one hotkey drives the pane (`docs/contributing/research/mod-spike.md`). Artifact: multi-file publish with a module script and data file accepted; access rules hold under `as_level` (`docs/contributing/research/artifact-spike.md`). Page-side numbers arrive when the probe is first opened. Design language set: a planning table with toy blocks, chalk intent outlines, yarn lenses, ghost blocks, decision cards (`docs/contributing/design-language.md`).
- 2026-10-26: Truth review round 3 (the last allowed): 4 material (the S4 radius mixed before and after states; the radius rule omitted API specs; the process machine added a `done` state; language and component granularity unstated). All fixed by the main agent and verified by `examples/check-fixture` (0 failures). Accepted risk: the round-3 fixes were not re-reviewed by an independent agent, because the review protocol caps rounds at 3; `check-fixture` re-derives the radius, evidence lines and patch behavior mechanically. **TRUTH.json is frozen from this commit; later changes need a decision record.**
- 2026-10-26: Art made with Iris (logo, hero, social base, three empty states), about $0.27, prompts beside the images in `docs/art/`.
