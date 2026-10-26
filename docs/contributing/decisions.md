# Decisions

This page holds Planroom's numbered decision records. Each one gives the context, the decision, the alternatives it rejected, its consequences for the layout, contracts, tests, release, agents and users, and its sources. Each record names the item of KICKOFF `<design_starting_points>` it kept, replaced or extended. Changing the layout, a boundary or a convention needs a new record here before it lands (R16), and `cargo xtask check layout` enforces that.

## How to read a record

- **Status** is `accepted`, `accepted, verify in WS` (it rests on an assumption the walking skeleton checks), `needs confirmation` (the main agent must confirm before it binds), or `superseded by DR-n`.
- **Starting point** names the `<design_starting_points>` item and whether this record keeps, replaces or extends it, or says "none" when the record serves a requirement or constraint the starting points don't cover.
- **Origin** says which Phase 2 candidate the idea came from (S for simplicity-first, E for extensibility-first, C for concurrency- and agent-first) and which judge raised it.
- Sources are the Phase 1 notes in `docs/contributing/research/`, checked on 2026-10-25, plus `examples/TRUTH.json` and `examples/README.md`.

## Index

| DR | Title | Starting point | Effect | Origin |
|---|---|---|---|---|
| 1 | SQLite is the single writer; no engine daemon | Store: single-writer local server | replaced | S; C grafts (FULL sync, version handshake) |
| 2 | Store facts and events; derive the rest on read | Kernel: everything derived from events; Store: SQLite for derived state | replaced | S |
| 3 | Three edit operations and eight event kinds; a view is a value | Kernel: four primitives | extended (View dropped as a primitive) | S |
| 4 | Actor identity and actor rules | Store: events carry the actor | extended | C; fixes a fatal flaw in S |
| 5 | Base required; field-level authored revisions | Store: optimistic concurrency on entity versions | replaced | S, C, E combined |
| 6 | The mod reaches the engine through the CLI | none (research suggested `http.fetch` over a socket) | replaced the research suggestion | S |
| 7 | A stale write is stored; only a person's raises a card | Store: optimistic concurrency | extended | C over S |
| 8 | Idempotency with request hashes, replays and a mod outbox | Store: events carry an idempotency key | extended | C |
| 9 | Packs are data; built-in extractors and linkers are declared by packs | Packs: declarative files | kept, narrowed | S over E |
| 10 | A pack conformance kit | Packs | extended | E |
| 11 | Scope cuts for v0.1 | Extractors tier 3; canvas kit dependency matrix; XState | replaced | S |
| 12 | Hints, built-in linkers, endpoints as via nodes, and full evidence recall | Extractors in tiers; Kernel primitives | extended | C, E; fixes a fatal flaw in E |
| 13 | Ruby is an out-of-tree data pack the fixture workspace loads | Extractors tier 2 language list | kept (no Ruby built in) | judges disagreed; J1 over J3 |
| 14 | One command catalogue; no MCP server | `rmcp` if there's an MCP server | replaced | S |
| 15 | Lenses with flow, a dedicated path lens, and question pipelines | Store: typed query builder plus named queries | replaced | S, E |
| 16 | The git CLI is the only git adapter | Libraries: `gix` or `git2` | replaced | S |
| 17 | Kernel roles | Jev policy | extended | E |
| 18 | Claude backends: the gateway, and the session's model through the mod | none (constraint "Claude") | added | E, C; fixes a fatal flaw in S |
| 19 | Eval protocol for thresholds | Jev policy | extended | S, E, C |
| 20 | One card at a time, batched by cause | Jev policy; Remote mode (queued decisions) | extended | S, E |
| 21 | Agent-first envelope and engine-authored strings | none (principle 2) | added | S, E, C |
| 22 | A tiny surface with measured always-on cost | none (R14) | added | S, C |
| 23 | Plan stages with their methods and structural lint | Jev policy: structural lint in code | extended | C |
| 24 | Duties under leases instead of a jobs API | none | added | S; C graft (off the critical path) |
| 25 | Engine-made worktrees, two implementers, a cross-repo merge preflight | none (research: an agent type with worktree isolation) | replaced the research suggestion | S, E |
| 26 | The hand-off guard asks the person | none | added | judges disagreed; E and C combined |
| 27 | Curation drafts: templates, then Claude, then the curator agent | none | added | S, C |
| 28 | Engine bootstrap without a SessionStart hook | Repo layout: engine bootstrap | kept, refined | S, E |
| 29 | Browser login with a one-time code | Store: local server | extended | C |
| 30 | Skills hidden from the slash menu | none (R14) | added | fixes a fatal flaw in all three |
| 31 | Remote mode: a published snapshot, a small live head, an attributed inbox | Remote mode | kept, refined | S, C |
| 32 | The S8 concurrency harness | none | added | C |
| 33 | Freshness on read instead of a file watcher | Libraries: `notify` | replaced | S |
| 34 | No per-repo export directory | Store: deterministic `.planroom/` export | replaced | S |
| 35 | Crates and packages | Repo layout sketch | replaced | S, C |
| 36 | One boundaries file for both languages | Repo layout: crate-graph test or `cargo-deny`; `dependency-cruiser` | kept, combined | C, S |
| 37 | One schema in two views, one version | Repo layout: `planroom-schema` as contract source; schema-to-TS generator | kept, refined | S |
| 38 | The xtask runner is the one task entry point | Repo layout: `justfile` or `cargo xtask` | kept (xtask) | all three |
| 39 | Rust library choices | Libraries to evaluate first (Rust) | kept mostly | research |
| 40 | TypeScript library choices | Libraries to evaluate first (TypeScript) | kept mostly | research |
| 41 | Jev adapter and pins | Jev policy | kept | research |
| 42 | Levels, IDs and fact merging | Kernel: levels, IDs | kept, refined | S |
| 43 | How S13's 1 s bar is measured in CI | none | added | needs confirmation |

## Judge disagreements and how they were resolved

| Disagreement | Judges | Resolution | Record |
|---|---|---|---|
| Which candidate is best overall | J1 and J3 ranked simplicity first; J2 ranked concurrency first | Start from simplicity (best feasibility, core simplicity, traceability, R14 and R16 scores) and graft concurrency's agent-native and attention mechanisms, which were J2's reasons | all |
| Where Ruby support lives | J1: out of tree, loaded in the S1 and S2 CI paths; J3: a pattern file in the base `code` pack | Out of tree, loaded by the fixture workspace's config on every path, because `examples/README.md` says Ruby has no built-in extractor and S10 is a stronger proof with a pack that isn't built in | DR-13 |
| Stale writes | S: every stale write is a card; C: an agent's stale write is only an error | Store every stale write as a conflict (nothing lost), raise a card only for a person | DR-7 |
| The hand-off guard | E: deny the chat's own edits on the touch set; C: only warn; J2 objected to blocking edits the person asked for | Ask the person through the permission prompt; deny with a one-line reason only if asking isn't possible | DR-26 |
| Daemon or not | E and C: a daemon over a Unix socket; S: no daemon | No daemon: `process.run` has the timeout `http.fetch` lacks, and nothing has to survive a reload | DR-1, DR-6 |
| Remote sync loop | C: a mod timer calling `ArtifactData`, auto-allowed; J2 flagged it as risky | No timer and no auto-allow; sync is model-driven or a person's key press | DR-31 |
| Number of tools | S: 3; E: 11; C: 9 | 3 tools, with a token snapshot | DR-22 |
| One schema or many | S: one; E: one per contract | One schema in two views with one version, per the principle "one versioned schema behind them all" | DR-37 |

## Grafts considered and rejected

| Graft | From | Why rejected |
|---|---|---|
| Executable pack extractors with a trust list, an SDK and an exec-versus-in-process parity test | E | Packs that run code need a trust model and a sandbox v0.1 doesn't have; data packs plus built-in linkers meet S10 (DR-9). The conformance kit is kept (DR-10). |
| A jobs API with `job.wait` long-polling | C | It exists to work around `http.fetch`'s missing timeout; the CLI path has `process.run` timeouts, and duties under leases already join duplicate work (DR-24) |
| `as_of` and `min_seq` read-your-writes tokens | C | Every process reads the same SQLite file, so a read after a commit sees it by construction; `serve` checks `data_version` before each read (DR-8) |
| A `reconcile.routine` role and a `change_significance` kind | E, C | Unplanned changes go to Diffs and never deal a card, so a routine commit needs no decision to stay quiet (DR-20) |
| A generic `planroom op NAME JSON` entry point | C | The catalogue already gives every command one JSON request shape over HTTP and tools; `planroom schema --commands` lists them (DR-14) |
| An "Engine" sixth help heading | C | Plumbing commands are grouped as "Setup", which is not a concept in the UI; S15's check is on the UI and the command family (DR-22) |
| XState for the statechart simulator | C | The machine is already data; a small pure step function with golden parity tests avoids a second machine model (DR-11) |
| A per-repo `.planroom/` export with git-author import | C | A second source of truth no scenario needs (DR-34) |

---

## DR-1: SQLite is the single writer; no engine daemon

- **Status:** accepted, verify in WS4.
- **Starting point:** replaced "A single-writer local server per workspace: HTTP commands and queries, plus a scoped live stream."
- **Origin:** S (central idea, D1); `synchronous = FULL` and the version handshake grafted from C after J3's review.
- **Context:** R10 needs serialized writes, compare-and-set and idempotency across several Claude sessions, agents, the canvas and CI. A daemon gives one writer, but it brings a lifecycle: stale locks, orphan processes, upgrades while it runs. The mod can't host a process across reloads: `$.process.spawn` children die on every reload and at session end, and `process.run` blocks on a child that keeps stdio open [mod-api §7, §10]. `http.fetch` has no timeout and no streaming [mod-api §7].
- **Decision:** every engine process writes through `store::commit`, one `BEGIN IMMEDIATE` transaction with a field-level compare-and-set and the idempotency check, in WAL mode with `synchronous = FULL` and a 5 s busy timeout. `planroom serve` exists only for browsers and exits after 30 idle minutes. `planroom watch` (one per mod session) is a reader that may run duties. A newer binary asks an older `serve` to drain before migrating, and an older `watch` or `serve` exits with `schema_mismatch` after a migration.
- **Alternatives rejected:**
  - A self-daemonizing server with a Unix socket (E, C, and mod-api's design implication 1): more moving parts, a lifecycle to manage, and the mod's calls would use `http.fetch` with a hand-made timeout.
  - `synchronous = NORMAL` (S as written): can lose the last commits on power loss; J3 flagged it.
- **Consequences:**
  - Layout: no `daemon` module; `crates/planroom/src/live.rs` serves both `watch` and `serve`.
  - Contracts: `serve.json` holds `{port, pid, version, drain_token}`; `meta.engine_version` in the store.
  - Tests: `store/tests/concurrency.rs` (3 processes × 500 writes), `store/tests/crash.rs`, the S8 harness (DR-32), and `serve_drains_for_newer_engine`.
  - Release: no service files and no lock cleanup in the installer.
  - Agents: the CLI is complete on its own, with no server to start first.
  - Users: nothing to keep running; the canvas starts `serve` on demand.
  - Fallback without a contract change: if WS4 or S8 shows contention, route CLI writes through `serve` when it runs.
- **Sources (checked 2026-10-25):** [mod-api §7, §10, Design implications 1]; [rust-stack §5] (rusqlite, one writer); judge J3 (`synchronous = NORMAL` gap); candidate-simplicity §3.9, §16 R-1.

## DR-2: Store facts and events; derive the rest on read

- **Status:** accepted.
- **Starting point:** replaced "Event (append-only; everything else is derived from events)" and "SQLite in WAL mode for events, derived state and a decision cache."
- **Origin:** S (D2).
- **Context:** extracted observations are rebuildable and large; authored actions are small and precious. Storing scans as events floods the log, and derived tables need migrations and repair.
- **Decision:** the store holds two truths: facts (per repo, per commit, rebuildable) and events (authored, append-only). Everything else is a pure fold computed on read; `serve` and `watch` keep the fold in memory and apply new events incrementally. The only caches are model responses, layouts and the in-memory fold.
- **Alternatives rejected:**
  - Facts as events (the starting point's literal reading): thousands of fact events per scan.
  - Projections in SQLite (E, C): a closure table and derived rows to migrate and rebuild.
- **Consequences:**
  - Layout: `planroom-kernel` holds the fold; `planroom-store` holds ten tables.
  - Contracts: only `events` and `ledger` need migration care.
  - Tests: `cargo bench fold` in `budgets.json` (under 50 ms at 20,000 facts and 5,000 events); `incremental_fold_equals_full_fold`.
  - Release: migrations are rare and small; `export` and `import` are the upgrade escape hatch.
  - Agents and users: a corrupted cache can't corrupt meaning; deleting `store.db` facts and rescanning restores them.
- **Sources (checked 2026-10-25):** candidate-simplicity §3.1, §3.7; [ts-stack §1] (layout cache is the one expensive derived value).

## DR-3: Three edit operations and eight event kinds; a view is a value

- **Status:** accepted.
- **Starting point:** extended "A typed property graph of four primitives: Entity, Relation, Event, View" (Entity is `Node`, Relation is `Edge`, Event kept; View dropped as a primitive).
- **Origin:** S (D4, §3.6).
- **Context:** R6 lists many edit types (regroup, rename, re-scope, re-wire, hide, pin, edit a breakdown), and S15 needs an undo test for every one. One mechanism per edit type multiplies conflict, ghost and attribution rules.
- **Decision:** every change to the graph is an `edit` event with one of `put`, `set`, `retract` on one target. The eight event kinds are `edit`, `review`, `undo`, `version`, `decision`, `claim`, `scan`, `handoff`. A view is a value, `ViewKey {level, lens, focus, project}`; pinned positions are `edit pin`; auto layouts are a cache.
- **Alternatives rejected:** typed override records per edit type (C, E); saved views as a primitive (the starting point), which no requirement needs.
- **Consequences:**
  - Contracts: `Edit { op, target, layer, fields, base, over }`.
  - Tests: `kernel/tests/undo.rs` generates apply-then-undo for every `Op × target kind`, plus review, decision pick, version and claim.
  - Users: pin a layout is undoable and conflict-checked like any edit.
- **Sources (checked 2026-10-25):** candidate-simplicity §3.6, §15 D4; judge J2 graft (three edit ops).

## DR-4: Actor identity and actor rules

- **Status:** accepted, verify in WS8 (the Bash tool's TTY).
- **Starting point:** extended "Events carry the workspace, project, actor, cause and an idempotency key."
- **Origin:** C (§3.7, §3.11); fixes the fatal flaw J2 and J3 found in S, where a CLI call with no `PLANROOM_ACTOR` was attributed to a person, so an agent's Bash calls could skip ghosting and approve its own merge. E left the CLI's actor undefined.
- **Context:** R5 says nothing the AI invents counts until a person accepts it, and R8 says merges follow a person's approval. Agents reach the engine through the mod's tools and through Bash. Plugin variables aren't in the Bash tool's environment [plugin-packaging §3].
- **Decision:**
  - A CLI with stdin and stdout on a TTY and no `PLANROOM_ACTOR` is a person. A CLI with no TTY is an agent (`agent:cli`) unless `PLANROOM_ACTOR` names another agent.
  - A person actor without a TTY needs `PLANROOM_PERSON_KEY` equal to `.planroom/person.key` (32 random bytes, mode 0600). The mod passes it only for a button press, an input submit, or a `/room` the person typed (`CommandRunInput.origin`). The test harness passes it to act as a person.
  - Mod tool handlers always send `agent:<session8>:chat`. An Artifact inbox row is `person:artifact:<uid>` by its path.
  - Each catalogue command declares `any` or `person_only`. Person-only: picking or overriding a decision, `handoff start`, `handoff merge`, `handoff cancel`, `intent version`, and `review` (agents may reject only their own ghosts). An agent gets `person_required` with the card handle.
  - Ghost status is decided at commit time from the actor: any agent or model edit is a ghost, except an auto-enabled decision effect or an `infer` output on the actual layer.
  - The mod's `tool.check` guard denies Bash, Read and Grep calls that touch `.planroom/person.key` or `serve.json`, or set `PLANROOM_PERSON_KEY`.
- **Alternatives rejected:**
  - `PLANROOM_ACTOR` defaulting to `person:<git email>` (S): the fatal flaw.
  - Session tokens from a daemon (C): needs DR-1's daemon.
  - An HMAC of each request with the key: the mod has no Node crypto, and the env var never leaves the mod's `process.run`.
- **Consequences:**
  - Layout: `crates/planroom/src/app/actor.rs`; `packages/mod/src/actor.ts`; `packages/mod/src/guard.ts`.
  - Contracts: `Actor`; `ActorRule` on every `CommandSpec`; error `person_required`.
  - Tests: `actor.rs` (no TTY is an agent; a person claim without the key fails; tool handlers never send a person); `s03_plan.rs` (an agent's accept fails); `s04_handoff.rs` (an agent's merge fails).
  - Security: a guardrail for "AI proposes, person disposes", not a security boundary between processes of one OS user; `SECURITY.md` says so.
  - Users: no prompts; person actions are the keys and taps they already make.
  - If the Bash tool turns out to allocate a TTY, TTY detection is dropped and the person key is the only non-canvas person path.
- **Sources (checked 2026-10-25):** [mod-api §3] (`CommandRunInput.origin`), [mod-api §8] (`tool.check` sees every tool); [plugin-packaging §3] (Bash environment lacks plugin variables); judges J2 and J3 fatal flaws on S; J2 fatal flaw on E.

## DR-5: Base required; field-level authored revisions

- **Status:** accepted.
- **Starting point:** replaced "Optimistic concurrency on entity versions."
- **Origin:** field-level compare-and-set from S (§3.9); `base_required` from C (§3.7); separating fact changes from user revisions from E (§3.10).
- **Context:** R10 says user-authored state is never last-writer-wins. S's check `request.base ⊆ current` passes for an empty base, so a write with no base was last-writer-wins (J3 fatal flaw). Whole-entity versions make a rename and a regroup of the same node conflict for no reason, and a rescan that bumps an entity's version makes a person's pending edit stale.
- **Decision:**
  - Every write to user-authored state carries `base`, the `seq` of the view the writer acted on. HTTP and tool calls without it get `base_required`; a non-TTY CLI must pass `--base SEQ`; a person at a TTY gets a read-then-write.
  - The conflict test is per field: a field the edit sets has an authored revision above `base`, written by another actor.
  - Scans never change authored revisions. A fact change under an override is a `ScanChangedUnderEdit` diff item, not a stale write.
  - The mod fills `base` for tool calls from the latest `meta.seq` it returned to the model in the turn, else the pane's `seq`.
- **Alternatives rejected:** entity versions (the starting point, C); `base ⊆ current` (S as written); optional base.
- **Consequences:**
  - Contracts: `base: Seq` on write requests; `Edit.over` records the fact value an override was made over.
  - Tests: `kernel/tests/cas.rs` (rename plus regroup apply; two renames conflict; an empty base fails; a rescan never stales an edit); S8's never-last-writer-wins check.
  - Agents: one number to pass, from the envelope they just read; the error hint says so.
  - Users: no spurious conflicts from background scans.
- **Sources (checked 2026-10-25):** candidate-simplicity §3.9; candidate-concurrency-agents §3.7; candidate-extensibility §3.10; judge J3 fatal flaw on S; J1 graft (field-level compare-and-set); J3 graft (fact version separate from user version).

## DR-6: The mod reaches the engine through the CLI

- **Status:** accepted, verify in WS5.
- **Starting point:** none; replaces the research suggestion of `$.http.fetch` over a per-workspace socket [mod-api, Design implications 1].
- **Origin:** S (D6).
- **Context:** `process.run` has a timeout and a 10 minute cap; `http.fetch` has neither timeout nor streaming [mod-api §7]. Agents use the CLI anyway.
- **Decision:** the mod runs commands with `$.process.run` (argv and env in, one JSON envelope out) and streams with `$.process.spawn` of `planroom watch`, restarted from the last `seq` after a reload. Mod tests stub `process.run` with golden CLI outputs that the Rust tests produce.
- **Alternatives rejected:** `http.fetch` to a daemon (E, C), which also needs DR-1's daemon.
- **Consequences:**
  - Tests: `packages/mod` tests read `tests/golden/cli/*.json`, so a contract drift fails both sides.
  - Agents: every mod action is also a CLI call an agent can make.
  - Fallback: `http.fetch` over `serve`'s socket with the same catalogue, if a surface lacks `process.run`.
- **Sources (checked 2026-10-25):** [mod-api §7]; candidate-simplicity §15 D6; judge J1 graft (golden CLI outputs for mod tests).

## DR-7: A stale write is stored; only a person's raises a card

- **Status:** accepted.
- **Starting point:** extended "Optimistic concurrency on entity versions."
- **Origin:** C (§3.9), combined with S's rule that nothing is lost (§3.9). Resolves the judges' disagreement: J2 flagged S's cards for agent races as a principle 1 violation; J3 noted C dropped the agent's write.
- **Context:** principle 1: interrupt a person only for decisions only they can make. Under S8's load, two agents race often. R10 and the principle "never lose what someone did" forbid discarding the losing write.
- **Decision:** a stale write is always stored as an `edit` with `status: conflict`. A person's stale write raises a three-way card with "theirs" pre-selected. An agent's or model's returns `stale_write` with a compact three-way diff (at most 1 KB) in `details`, is listed in Diffs under "agent retries", collapsed, and raises no card.
- **Alternatives rejected:** a card for every conflict (S); an error with nothing stored (C); an automatic merge (never).
- **Consequences:**
  - Contracts: error `stale_write` with `details {base, theirs, yours, event}`.
  - Tests: S8 asserts the tab's stale rename is a card and an agent's stale write is a stored conflict with no card.
  - Agents: they get everything they need to retry in one reply.
- **Sources (checked 2026-10-25):** candidate-concurrency-agents §3.9; candidate-simplicity §3.9; judges J2 (fatal flaw on S), J3 (concurrency gap on C).

## DR-8: Idempotency with request hashes, replays and a mod outbox

- **Status:** accepted.
- **Starting point:** extended "Events carry ... an idempotency key."
- **Origin:** C (§3.8, §3.10, §7.5).
- **Context:** S8 requires safe retries. A reused key with a different payload silently replayed the wrong result in S (J3). Spawned children and module state die on a mod reload [mod-api §7], so an in-flight write could be lost.
- **Decision:**
  - Every event stores `idem` (UNIQUE) and `request_hash`. Same key and hash: the stored reply with `meta.replayed: true`. Same key, other hash: `idempotency_mismatch`.
  - A derived key when none is given: `sha256(actor ‖ command ‖ canonical(args) ‖ base)`. Tool calls use the tool call ID when the handler has one.
  - The mod keeps an outbox atom in `$.state` and resends it with the same keys after a reload.
  - The canvas keeps pending edits in `sessionStorage`, wrapped in try/catch, until confirmed.
  - Read-your-writes needs no token (rejected graft: `as_of` and `min_seq`).
- **Alternatives rejected:** client-minted keys only (agents forget them); `min_seq` tokens (unneeded with one SQLite file).
- **Consequences:**
  - Contracts: `meta.replayed`; error `idempotency_mismatch`; the `request_hash` column.
  - Tests: S8 retries and duplicates; `store/tests/idem.rs`; a mod test that reloads mid-write and checks one event.
- **Sources (checked 2026-10-25):** [mod-api §7, §10]; candidate-concurrency-agents §3.8; judges J2, J3 grafts.

## DR-9: Packs are data; built-in extractors and linkers are declared by packs

- **Status:** accepted.
- **Starting point:** kept and narrowed "Packs. Declarative files that add kinds, levels, roll-up rules, extractors, lenses, decision kinds and a default view."
- **Origin:** S (D11); E's executable packs rejected.
- **Context:** S10 loads a pack from outside the source tree; R11 says domains plug in with no core change. A pack that runs code needs a trust model; scanned repos are untrusted input.
- **Decision:** a pack is TOML, tree-sitter queries, prompts and evals, never code. It may declare built-in Rust extractors and linkers by name, and declarative regex or tree-sitter extractors that emit nodes, edges and hints. `requires` is `["code"]` or nothing. Roll-up rules are fixed in the kernel; packs choose what a lens lifts.
- **Alternatives rejected:** exec extractors over NDJSON with a trust list, an SDK and a parity test (E, and C's exec Ruby pack): code from anywhere, no sandbox.
- **Consequences:**
  - Security: loading a pack from any path is safe by construction; the loader rejects executable files.
  - Layout: `packs/` holds data only; Rust logic a pack needs lives in `planroom-extract`.
  - Tests: `tests/packs.rs` (validation, the inward-only rule, no executables).
  - Users: a pack can't do what the declarative runner and built-ins can't; a format that needs a parser is a built-in.
- **Sources (checked 2026-10-25):** candidate-simplicity §4.1, §15 D11; candidate-extensibility §4.4, §4.11; judge J3 (exec packs run unsandboxed code).

## DR-10: A pack conformance kit

- **Status:** accepted.
- **Starting point:** extended "Packs."
- **Origin:** E (§4.10), adapted to data packs.
- **Context:** S10's out-of-tree path should be first-class and checked in CI, and pack authors outside the repo have no `xtask`.
- **Decision:** `planroom doctor --pack DIR` validates the manifest, runs every extractor over the pack's `tests/inputs/`, compares sorted facts with `tests/expected.jsonl`, runs twice to prove determinism, and validates eval files. CI runs it on `packs/*` and `examples/packs/*`.
- **Alternatives rejected:** a separate `pack` verb (more surface); the exec parity test (no exec packs).
- **Consequences:** contracts gain the pack test layout; docs gain "Write a pack" in guides.
- **Sources (checked 2026-10-25):** candidate-extensibility §4.10; judge J1 graft.

## DR-11: Scope cuts for v0.1

- **Status:** accepted.
- **Starting point:** replaced "precise: SCIP/LSIF or a language server", "a dependency matrix synced with the graph", "XState for the simulator"; config formats k8s and Terraform deferred.
- **Origin:** S (D10, D15, D16).
- **Context:** no requirement or scenario needs these, and each adds a library or a second model. Principle: keep the bar on quality, not scope.
- **Decision:** v0.1 ships no SCIP reader, no dependency matrix, no XState, and no k8s or Terraform extractors. Each is one more built-in or view later, with no kernel change. The canvas's statechart step-through is a pure function in `@planroom/ui`, checked against the Rust `ask machine --step` golden outputs, so the Artifact can step a machine with no engine.
- **Alternatives rejected:** shipping them (C included the matrix and XState).
- **Consequences:** known gaps recorded at release; `feature_list.json` adds no entries for them.
- **Sources (checked 2026-10-25):** candidate-simplicity §15; [ts-stack §2] (xstate size, kept as a rejected option).

## DR-12: Hints, built-in linkers, endpoints as via nodes, and full evidence recall

- **Status:** accepted, verify in M1 (the S1 comparator).
- **Starting point:** extended "Extractors, in tiers" and "Relation {..., evidence[], confidence ...}".
- **Origin:** C's hints and two-phase scan (§4.2), E's link phase (§4.6). Fixes E's fatal flaw (S4 step 4 missed the planted regression) and closes S's gap (no extractor for the audit DSN, J1).
- **Context:** the truth's relations carry evidence from several repos at once: a client call, a compose line, the callee's route handler, the spec path, and call sites [TRUTH relations]. After the S4 merges, the worker's database edge is evidenced only by a literal default DSN, because compose no longer wires it [TRUTH scenarios.S4.post_merges]. The impact radius needs callee evidence for an intended edge that doesn't exist yet [TRUTH scenarios.S4.impact_radius.rule]. Prose in comments and docstrings is not evidence.
- **Decision:**
  - Facts gain a fourth shape, `Hint`. Extractors emit hints (URL and DSN literals, environment reads and sets, served and called routes, queue operations, database opens, SQL strings, definitions and calls) from syntax nodes, never comments or docstrings.
  - Built-in linkers in `planroom-extract::link`, declared by packs, join hints across the workspace into edges carrying the union of their evidence, plus call sites up to two hops within a container (at most three per hint).
  - A database open with a literal default DSN resolves by the literal's host when compose doesn't set the variable.
  - `runtime_wiring` makes `code.endpoint` nodes; an `http` edge's `via_ref` points at one, and an edge's evidence includes its via node's evidence.
  - Evidence is verified against the blob at ingest.
  - The truth comparator requires full evidence recall: every truth evidence item must appear. An item that proves infeasible gets a KICKOFF infeasibility record, the closest testable variant and an independent reviewer's approval.
- **Alternatives rejected:**
  - Aliases only (S): can't join code with compose wiring or a literal DSN.
  - A `code.db_open` hint with only an env var name (E): misses the regression.
  - Requiring at least one truth evidence item per relation: a weakening of the frozen truth without the KICKOFF process.
- **Consequences:**
  - Contracts: `Hint`; `Edge.via_ref`; the `link` snapshot purpose; warning `evidence_dropped`.
  - Tests: `extract/tests/link.rs`; `s01_first_look.rs` (every truth relation with every evidence item); `s04_handoff.rs` (post-merge regression from the code alone); a docstring-is-not-evidence test.
  - Risk: call-site evidence by name may over-include; extra verified evidence is allowed ("a scan may find more" [TRUTH about]).
- **Sources (checked 2026-10-25):** `examples/TRUTH.json` (relations, scenarios.S4); candidate-concurrency-agents §4.2; candidate-extensibility §4.6; judges J1 (fatal flaw on E, graft `*.db_dsn`), J3.

## DR-13: Ruby is an out-of-tree data pack the fixture workspace loads

- **Status:** accepted.
- **Starting point:** kept "syntactic: tree-sitter queries and import resolvers for TS/JS, Python, Go and Rust" (no Ruby built in).
- **Origin:** J1's graft; J3 proposed a Ruby pattern file in the base `code` pack instead. E and C kept Ruby in test-only packs, so their S1 truth match failed (fatal flaws from J1 and J3).
- **Context:** `TRUTH.json` lists the `healthcheck` container and its two `GET /healthz` edges as things that must exist [TRUTH containers, relations]. `examples/README.md` says Ruby has no built-in extractor, and S10 needs a new language to work through the extractor contract alone.
- **Decision:** Ruby support is the data pack `examples/packs/ruby/`, not embedded in the binary. `cargo xtask fixture OUT` writes `OUT/.planroom/config.toml` with `packs = ["<repo>/examples/packs/ruby"]`, so every CI path and evidence run of the fixture loads it, and `feature_list.json` records that fixture state. S10 builds a fixture without that config, shows `healthcheck` absent, then loads copies of the Ruby and flags packs from a temporary directory outside the tree.
- **Alternatives rejected:**
  - Ruby in the base pack (J3, S): contradicts the fixture contract and weakens S10's proof.
  - Ruby only in the S10 test (E, C): S1's truth match fails.
- **Consequences:**
  - Layout: `examples/packs/ruby/`, `examples/packs/flags/`.
  - Tests: `s01_first_look.rs` and `s02_questions.rs` use the configured fixture; `s10_extensibility.rs` asserts absence, then presence.
  - Users: a real workspace with Ruby files gets the universal tier for them until the Ruby pack is added; the release archive ships `examples/packs/ruby` for that.
  - Open for the main agent: add one line to `examples/README.md` saying the fixture workspace loads `examples/packs/ruby` through its config (not a `TRUTH.json` change).
- **Sources (checked 2026-10-25):** `examples/README.md` (scenario hooks, S10); `examples/TRUTH.json` (containers, relations, scenarios.S10); judges J1 and J3 grafts and fatal flaws.

## DR-14: One command catalogue; no MCP server

- **Status:** accepted.
- **Starting point:** replaced "`rmcp` (if there's an MCP server)"; principle 2 lists "an MCP server if the architecture calls for one".
- **Origin:** S (§7.1, D5); handler-coverage test from C (§7.1).
- **Context:** the CLI, HTTP API, tools and `/room` must stay in step. Plugin tools already appear as `mcp__planroom__*` [mod-api §4]. `rmcp` shipped 3 majors in 5 months [rust-stack §8].
- **Decision:** `planroom-contract::Command` is one enum. Each variant carries its request and result types, idea, actor rule, permission class, default budget, error codes, undo and surfaces. It generates the clap tree, the HTTP routes, the three tool schemas, the `/room` parser and typeahead, and `docs/reference/cli.md`. A test asserts every variant has exactly one handler. No MCP server: sessions without mods use the skills, which teach the CLI.
- **Alternatives rejected:** an rmcp stdio server (a second wiring path and a fast-moving dependency); hand-written bindings per surface.
- **Consequences:**
  - Layout: one `app/` handler per command; no `.mcp.json`.
  - Tests: `catalogue_has_one_handler_per_command`; the CLI reference drift test; the tool schema drift test.
  - Agents: one request shape per command everywhere; `planroom schema --commands` lists them.
- **Sources (checked 2026-10-25):** [mod-api §4]; [rust-stack §8]; [plugin-packaging, Design implications] (one tool-wiring path).

## DR-15: Lenses with flow, a dedicated path lens, and question pipelines

- **Status:** accepted.
- **Starting point:** replaced "A typed query builder plus named queries (neighbourhood, paths, roll-ups, diffs)."
- **Origin:** S (§4.6, D14); `fires-if` and `bump-breaks` named from E (§4.7, J1).
- **Context:** S2's path rule follows only `http`, `queue_publish` and `queue_consume`, the last one backward [TRUTH questions.path_storefront_to_worker.rule]. R11 names "what fires if this breaks?" and "what breaks if I bump X?", which S left out (J1). A query builder is a second API to version.
- **Decision:** a lens is data: relation kinds, a flow direction per kind, a default level. The default lens is `runtime` (with `db`); the `path` question uses the `flow` lens, which has exactly the truth's edge kinds. A question is a named pipeline of one to three kernel query shapes from a fixed set of twelve; `fires-if` and `bump-breaks` are pipelines. A new shape needs two packs that use it and a decision record.
- **Alternatives rejected:** a runtime lens with `db` and `deploys` for paths (E, C): goes beyond the truth's rule; a query builder (the starting point).
- **Consequences:** contracts gain `QuestionSpec { steps }`; tests `s02_questions.rs`, `kernel/tests/query.rs`, `questions_fires_if.rs`, `questions_bump_breaks.rs`.
- **Sources (checked 2026-10-25):** `examples/TRUTH.json` (questions); candidate-simplicity §4.6; candidate-extensibility §4.7; judge J1 grafts.

## DR-16: The git CLI is the only git adapter

- **Status:** accepted.
- **Starting point:** replaced "`gix` or `git2`."
- **Origin:** S (D7).
- **Context:** the scan needs history with numstat, tags and branches; the hand-off needs `worktree add`, `apply --3way`, `commit` and `merge --no-ff`. gix has no porcelain merge and its worktree mutation was not tried [rust-stack §6]. The truth's post-hand-off state is defined by `git apply` [TRUTH scenarios.S4.post_handoff].
- **Decision:** `planroom-extract::git` shells out to `git` (at least 2.30) with `-c core.hooksPath=/dev/null -c core.fsmonitor=false --no-optional-locks`, `LC_ALL=C` and `-z` porcelain.
- **Alternatives rejected:** gix 0.90.0 (a new 0.x every 2 to 4 weeks, a 27 to 53 s build unit, no merge porcelain); git2 0.21.0 (C libgit2 and its CVEs).
- **Consequences:**
  - Release: git is a runtime requirement; `doctor` checks the version.
  - Tests: CI on Linux and macOS; a hook fixture that must never fire.
  - Performance: `git log --numstat` took 0.90 s against 1.67 s for gix on 2,023 commits [rust-stack §6].
- **Sources (checked 2026-10-25):** [rust-stack §6]; judge J1 graft.

## DR-17: Kernel roles

- **Status:** accepted.
- **Starting point:** extended "Jev policy. Each decision kind declares its question, thresholds and escalation."
- **Origin:** E (§5.2).
- **Context:** the kernel needs some decisions (an override whose target vanished, a change unit the reconciler can't place, a next action with no target) but must name no pack kind (R11, R16).
- **Decision:** three kernel roles: `rescan.retarget`, `reconcile.residual`, `attach.target`. A pack's decision kind binds one with `role = "..."`. An unbound role falls back to a card with no pick; two bindings are `pack_invalid`.
- **Alternatives rejected:** naming pack kinds in the kernel (fails the domain-free scan); `reconcile.routine` (DR-20 makes it unneeded); `intent.stage` (stages are pack data already).
- **Consequences:** contracts gain `Role`; tests `kernel/tests/roles.rs` and the domain-free literal scan.
- **Sources (checked 2026-10-25):** candidate-extensibility §5.2; judge J1 graft.

## DR-18: Claude backends: the gateway, and the session's model through the mod

- **Status:** accepted, verify in WS8.
- **Starting point:** none; implements the constraint "The mod uses the chat model and `$.model.complete`, the Artifact uses `sample`, and the engine uses Claude models on the gateway, with the model pinned in config."
- **Origin:** E's session-model adapter (§5.1), C's broker (§5.1); fixes the fatal flaw all three judges found in S, which dropped `$.model.complete` with no record (a binding constraint can't be overridden by a decision record anyway).
- **Context:** most plugin users won't set `AI_GATEWAY_API_KEY`. `$.model.complete` uses the session's own account and needs an explicit model; behind a gateway, cache marks go in `prompt` [mod-api §6]. It isn't in the session's cost totals.
- **Decision:**
  - `planroom-models::claude` has a `gateway` backend (when a gateway credential resolves; `anthropic/claude-haiku-5.5` cheap and `anthropic/claude-sonnet-5.5` capable, pinned in config) and a `session` backend (no credential and a mod attached).
  - With `session`, a Claude call becomes a `ModelRequest` listed in `meta.model_requests` and written to `.planroom/models/requests/`. The mod runs `$.model.complete` with the stable prefix in `prompt` blocks, the last marked `cache: true`, and imports a `planroom.model-response/v1` file. The response lands in `model_cache` under the same input hash, so the command picks it up when re-run.
  - The first session request deals one consent card ("Use your Claude session for Planroom's Claude calls?") with "Yes" pre-selected, recorded as a decision. Usage goes into the ledger with `provider = session` and shows in the status line.
- **Alternatives rejected:** no `$.model.complete` (S: violates the constraint); a broker over a daemon's stream (C, E: needs DR-1's daemon); the mod calling Claude for its own summaries (the engine authors every string, DR-21).
- **Consequences:**
  - Contracts: `ModelRequest`, `ModelResponse` (`planroom.model-response/v1`), `meta.model_requests`.
  - Tests: `models/tests/session_backend.rs`; a mod test with a stubbed `model.complete`; the S9 evidence run uses the session backend once.
  - Cost: the spend guards cover session usage too.
  - Open for the main agent: confirm that the session backend, used whenever the gateway isn't configured, satisfies "the mod uses `$.model.complete`", or add a second mod-local use.
- **Sources (checked 2026-10-25):** [mod-api §6]; [claude-gateway §2, §7]; KICKOFF `<constraints>` Claude; judges J1, J2, J3 fatal flaws on S.

## DR-19: Eval protocol for thresholds

- **Status:** accepted.
- **Starting point:** extended "Jev policy" (thresholds come only from evals).
- **Origin:** hash split and byte-for-byte recompute from S (§5.9); `--verify-order` and injection canaries from E (§5.8); option shuffling from C (§5.8).
- **Context:** S7 needs an independent labeller who hasn't seen Jev's outputs, cases held out from tuning, and auto-action only at observed precision of at least 0.95 on at least 60 cases. Jev reads instructions literally and is confidently wrong on some ambiguous cases [jev §5].
- **Decision:** a case writer makes states (with at least two injection canaries per kind); an independent labeller labels them and the labels are committed before any Jev call (`cargo xtask eval --verify-order` checks git order and the cases digest); the split is `sha256(case_id) mod 5 < 2` for tune; instructions and bands are fitted on tune only; two live runs on `typesafe-ai/jev-1.14.0`, the second with option order shuffled; `auto_enabled` only at holdout precision of at least 0.95 on 60 or more cases in both runs with at least 30 acted on; CI recomputes every result byte for byte.
- **Alternatives rejected:** a hand-chosen split; thresholds set in pack files; one run.
- **Consequences:**
  - Contracts: `EvalCase`, `EvalResult` (`planroom.eval/v1`).
  - Tests: `kernel/tests/evals.rs`, `decision_policy_matches_eval_results`.
  - Spend: about $0.02 to $0.05 live for a full pass [jev §9].
- **Sources (checked 2026-10-25):** [jev §5, §8, §9, §10]; judges J1, J3 grafts.

## DR-20: One card at a time, batched by cause

- **Status:** accepted.
- **Starting point:** extended "Jev policy" and the remote mode's "shows cached decisions and queues new ones."
- **Origin:** S (§5.6); per-turn batching and the prompt section from E (§3.3, §7.5).
- **Context:** principle 1: decisions cheap, pre-decided, batched, one tap. S7: a routine rescan raises at most one card.
- **Decision:**
  - Surfaces show the top card by `stakes × staleness` and a count of the rest.
  - Ghosts from one cause (one chat turn, one plan stage run, one curation run) batch into one card ("Apply Claude's 2 edits?").
  - Unplanned changes go to Diffs and never deal a card; only regressions do.
  - The mod's `prompt.compose` section is one stable block of at most 300 tokens, so it stays in the cached prefix.
- **Alternatives rejected:** a card per ghost; a `reconcile.routine` decision kind (E, C).
- **Consequences:** contracts gain `Cause::Turn`; tests `s07_decisions.rs` (at most one card), `cards_batch_by_cause`.
- **Sources (checked 2026-10-25):** candidate-simplicity §5.6; candidate-extensibility §3.3, §7.5; judge J2 grafts.

## DR-21: Agent-first envelope and engine-authored strings

- **Status:** accepted.
- **Starting point:** none; implements principle 2 ("structured, compact, paginated, with stable IDs and pointers to evidence").
- **Origin:** S's envelope (§7.2); handles, `ambiguous_ref`, category and `retryable` from E (§7.3); `empty.next` from C (§7.2); the string catalog and `same_output` from E (§7.1); evidence verification at ingest from E and C.
- **Context:** agents pay in tokens and need to drill down and recover without guessing; three surfaces must say the same thing; no screen may be blank.
- **Decision:**
  - One JSON document on stdout: `schema_version`, `ok`, `command`, `result` (summary first), `meta {seq, took_ms, size, next, truncated, replayed, warnings, models, model_requests}`, or `error {code, category, retryable, message, hint, details}`.
  - Handles are exact command invocations.
  - Empty lists carry `empty: {reason, next: {command, args, label}}`.
  - Suffix lookups with several matches return `ambiguous_ref` with candidates.
  - Evidence is `{path, line, contains}`, verified at ingest, `contains` cut to 200 characters.
  - The engine authors every human-readable string from `crates/planroom/text/en.toml` and pack labels; `same_output` checks the CLI, the pane and the canvas show the same text.
- **Alternatives rejected:** per-surface prose; abbreviated keys.
- **Consequences:** tests `envelope_schema.rs`, `same_output`, `empty_state_audit`; contracts in [Contracts](contracts.md#cli-json-envelope).
- **Sources (checked 2026-10-25):** [iris-release-bar §6]; candidates S §7.2, E §7.1 and §7.3, C §7.2; judges J2, J3 grafts.

## DR-22: A tiny surface with measured always-on cost

- **Status:** accepted.
- **Starting point:** none; implements R14.
- **Origin:** S (§7.2, §7.4, §9.6); the token snapshot from C (§5.9).
- **Context:** R14: one CLI entry point, one command family, and check whether a better default removes a concept, flag or setting. Every always-on tool and skill costs tokens every turn [plugin-packaging §Design implications].
- **Decision:** three tools (two non-deferred), four global flags plus `--base` on writes, no `--quiet`, `--verbose` or `--format`, one optional config file with a default for every key, and a rule that a new setting needs a decision record saying why no default works. The only user-facing settings in v0.1 are extra pack paths, the model backends and pins, the spend limits, and `models = off`, a privacy and offline choice no default can make for the person. A snapshot test fails when non-deferred tool descriptions and schemas, skill frontmatter and the mod's prompt section exceed 1,500 projected tokens from `claude plugin details`. CLI help groups commands by the five ideas plus "Setup".
- **Alternatives rejected:** nine or eleven tools (C, E); an "Engine" concept heading (C).
- **Consequences:** tests `s15_surface.rs`, `tests/plugin/surface_tokens.sh`; the configuration reference is generated from `defaults.toml`.
- **Sources (checked 2026-10-25):** [plugin-packaging §Design implications] (about 40 tokens per short skill); candidate-simplicity §7.4; judge J2 grafts.

## DR-23: Plan stages with their methods and structural lint

- **Status:** accepted.
- **Starting point:** extended "Structural lint runs in code; semantic lint goes to Jev."
- **Origin:** C (§6.1); J1 noted S reduced R5's methods to a sentence.
- **Context:** R5 names the methods per stage (Jobs-to-be-Done and laddering, SMART and non-goals, MECE and the WBS 100% rule, GTD and HTN, MoSCoW and an MVP line, pre-mortem, lint), and S3's lint depends on them.
- **Decision:** the plan pack declares seven stages, each with its method, kinds and fields, structural lint rules (in kernel rule types) and semantic Jev kinds, as in [Architecture](architecture.md#planning-stages-ghosts-and-lint-r5). In chat, the planning partner proposes items itself as ghosts; with no chat, `intent next` asks Claude with the stage's cached prefix. Version 1 is tagged when a project's first batch is accepted; later versions are a person's.
- **Alternatives rejected:** a generic "stage with a prompt" (S).
- **Consequences:** contracts gain the `covers` rule type for the 100% rule; tests `s03_plan.rs`, `plan/tests/lint_rules.rs`.
- **Sources (checked 2026-10-25):** KICKOFF R5; candidate-concurrency-agents §6.1; judge J1 graft.

## DR-24: Duties under leases instead of a jobs API

- **Status:** accepted.
- **Starting point:** none.
- **Origin:** S's duties (§2.4); C's "decisions off the scan's critical path, keyed so duplicate triggers join" (J3 graft).
- **Context:** Jev takes about 300 to 600 ms per call at the client [jev §3]; S ran decisions inline in a cold scan. Concurrent triggers shouldn't duplicate work. `http.fetch` has no timeout.
- **Decision:** background work (rescan on a moved head, the decision run, curation) is a duty that a live process runs only while holding its lease (`scan:REPO`, `decide:HASH`, `curate`), so duplicate triggers join one run. A scan never waits on Jev; with no live process, `planroom decide --run` does the work. The server replies within 10 s; a command still running at 8 s detaches into its duty and completes on the stream.
- **Alternatives rejected:** a jobs API with `job.wait` (C).
- **Consequences:** contracts gain `duty` stream frames and the `{running}` reply; tests `duties_join_on_lease.rs`, `s07_decisions.rs`.
- **Sources (checked 2026-10-25):** [jev §3]; [mod-api §7]; judge J3 graft.

## DR-25: Engine-made worktrees, two implementers, a cross-repo merge preflight

- **Status:** accepted.
- **Starting point:** none; replaces the research suggestion of `planroom:implementer` registered with `isolation: 'worktree'` [mod-api §5].
- **Origin:** S (§6.6, D17); the merge preflight from E (§6.6).
- **Context:** S4's change spec touches `worker` and `platform` [TRUTH scenarios.S4.implementer_patches]; Claude Code's worktree isolation covers only the session's repo. A merge that fails in the second repo must not leave the first merged.
- **Decision:** the engine makes one worktree per touched repo on `planroom/KEY`. Two implementers share one interface: `agent` (`planroom:implementer`, spawned by the mod with `cwd` at the hand-off directory) and `patch` (applies the spec's patches with `git apply --3way`; the fixture implementer and curation). `handoff merge` is person-only and preflights every repo before merging any.
- **Alternatives rejected:** agent-type isolation; merging repo by repo with no preflight.
- **Consequences:** contracts `ChangeSpec`, `ImplementerRun`; tests `s04_handoff.rs`, `handoff/tests/preflight.rs` (a planted conflict in the second repo leaves both untouched).
- **Sources (checked 2026-10-25):** [mod-api §5]; `examples/TRUTH.json` scenarios.S4; judge J1 graft.

## DR-26: The hand-off guard asks the person

- **Status:** accepted, verify in WS8.
- **Starting point:** none; implements R8 "The planning partner in the chat never silently implements."
- **Origin:** E denied the chat's own edits on the touch set (§6.6); C only added a notice (§6.4); J2 objected that E blocks edits the person asked for.
- **Context:** "never silently" is a guarantee, so a notice is too weak; blocking a person's explicit request costs attention.
- **Decision:** while a hand-off is started and not closed, the mod's `tool.check` hook stops the main thread's `Edit`, `Write`, `MultiEdit` and `NotebookEdit` on any path in its touch set and asks the person through the permission prompt, with one line naming the hand-off. If `tool.check` can't ask, it denies with that line and names `/room handoff cancel KEY`.
- **Alternatives rejected:** deny always (E); notice only (C).
- **Consequences:** tests `packages/mod/src/guard.test.ts`; S9 evidence shows the prompt once.
- **Sources (checked 2026-10-25):** [mod-api §4, §8]; judges J1 (graft from E), J2 (objection).

## DR-27: Curation drafts: templates, then Claude, then the curator agent

- **Status:** accepted.
- **Starting point:** none; implements R12.
- **Origin:** S's templates (§6.8); C's Claude drafting and its two CI paths (§6.5, §10.2).
- **Context:** S12 must pass deterministically; drafts should read well when a model is reachable.
- **Decision:** miners (gotchas, procedures, stale lines) produce template drafts. When a model is reachable, a cheap-tier Claude call refines each draft with the target file in its cached prefix; in a live session the hidden curator agent can rework drafts that span files. Every draft must mention the cluster's key tokens before it's proposed. CI runs S12 twice: with replayed Claude and with models off.
- **Alternatives rejected:** Claude-only drafts; template-only drafts.
- **Consequences:** contracts `CurationDraft`; tests `s12_curation.rs` (both paths).
- **Sources (checked 2026-10-25):** `examples/TRUTH.json` scenarios.S12; judge J1 graft.

## DR-28: Engine bootstrap without a SessionStart hook

- **Status:** accepted.
- **Starting point:** kept and refined "plugin/ ... engine bootstrap."
- **Origin:** S (§11.2, D20); the source-build card from E (§11.2).
- **Context:** plugins can't hold a `bin/` folder; mixing `modules` with classic hooks in one `hooks.json` is unverified; an interactive SessionStart hook holds Claude's first reply [plugin-packaging §3].
- **Decision:** `plugin/scripts/planroom` resolves `$PLANROOM_ENGINE`, the dev checkout, the cache under `$CLAUDE_PLUGIN_DATA`, then a sha256-verified download for the fixed tag, then prints the exact `cargo install` fallback. The mod starts `prefetch` in `session.start`; if the fetch fails it deals one card, "Build the engine from source, about 4 minutes?", with progress in the status line.
- **Alternatives rejected:** a SessionStart prefetch hook; `releases/latest` (403 through the proxy).
- **Consequences:** tests `tests/installer/run.sh`; release writes `plugin/scripts/engine.env`.
- **Sources (checked 2026-10-25):** [plugin-packaging §3, Design implications]; [iris-release-bar §3]; judge J2 graft.

## DR-29: Browser login with a one-time code

- **Status:** accepted.
- **Starting point:** extended "A single-writer local server per workspace" (its exposure).
- **Origin:** C (§12.2); J3 flagged S's `?t=` query-string token.
- **Context:** a token in a query string lands in logs and history.
- **Decision:** `planroom open` prints a URL with a one-time code in the fragment; the page exchanges it for an `HttpOnly`, `SameSite=Strict` cookie. `Host`, `Origin` and `Sec-Fetch-Site` are checked; no CORS; strict CSP.
- **Alternatives rejected:** a bearer token in the query string (S).
- **Consequences:** tests `server/tests/auth.rs`.
- **Sources (checked 2026-10-25):** candidate-concurrency-agents §12.2; judge J3 graft.

## DR-30: Skills hidden from the slash menu

- **Status:** accepted, verify in WS8.
- **Starting point:** none; implements R14 and S15 "one command family."
- **Origin:** a fatal flaw the judges found in all three candidates.
- **Context:** plugin skills appear as `/planroom:<name>` slash commands [plugin-packaging §3], outside the `/room` family.
- **Decision:** three skills (`room`, `room-plan`, `room-handoff`), each with `user-invocable: false` in frontmatter, so the model can load them and the slash menu doesn't list them. The S15 snapshot asserts the headless session's `slash_commands` list holds only `/room` from Planroom. If `user-invocable: false` isn't honored on Claude Code 2.1.310, ship one skill named `room`, whose `/planroom:room` entry is the same family.
- **Alternatives rejected:** four or five visible skills (all candidates).
- **Consequences:** tests `tests/plugin/slash_commands_snapshot.sh` (evidence path, needs a session) and a static frontmatter check in CI.
- **Sources (checked 2026-10-25):** [plugin-packaging §3]; judge J2 fatal flaw (all three).

## DR-31: Remote mode: a published snapshot, a small live head, an attributed inbox

- **Status:** accepted, verify in WS7 and WS8.
- **Starting point:** kept and refined "Remote mode: the workspace is the source of truth, the session pushes snapshots to the Artifact db; Artifact edits land as events in a db collection; the session imports them on `/room open`, at turn start, and on edit notices; the Artifact shows cached decisions and queues new ones."
- **Origin:** S (§7.6); snippets excluded by default and the first-publish summary from C (§12.5); C's timer loop rejected (J2).
- **Context:** `db` writes are last-writer-wins page-side, need `if_version` session-side, and don't wake the session; bulk data fits a published data file; a `{self}` path proves the author [artifact §2, §6; artifact-spike].
- **Decision:** the bulk model ships as `data/snapshot.json` (code text excluded by default); a small `live/head` document carries the top cards and recent changes; viewers write only `inbox/{self}/events/{docId}`; `planroom import` commits rows as attributed person edits through the same compare-and-set. Sync runs on `/room sync`, `/room` opening, the page's Send to Claude, and a turn-start hint. The mod never calls `ArtifactData` from a timer and never auto-allows it.
- **Alternatives rejected:** all data in `db` shards (25,000-document cap, many batches); a 5 s mod timer auto-allowed through `tool.check` (C).
- **Consequences:** contracts `Snapshot`, `LiveHead`, `InboxRow`; tests `s11_remote.rs`, `tests/e2e/artifact.spec.ts` with a fake `db` that has the real rules.
- **Sources (checked 2026-10-25):** [artifact §2, §5, §6]; [artifact-spike]; judges J2 (fatal flaw on C's timer), J2 graft (snippets).

## DR-32: The S8 concurrency harness

- **Status:** accepted.
- **Starting point:** none; implements S8.
- **Origin:** C (§10.3).
- **Context:** S8 needs proof of no lost writes, idempotent retries, live updates within 1 s, claims and presence, and a stale write that becomes a diff, not a merge.
- **Decision:** two agent CLI writers on two projects, an HTTP writer as a canvas tab plus an SSE reader, 10% of writes killed after commit and before the reply and retried with the same key, 5% duplicated on purpose, a deliberate stale rename, a claim. Faults are compiled only in test builds (`PLANROOM_TEST_FAULTS`). A `writer_interleavings` proptest checks the same invariants in-process; `incremental_fold_equals_full_fold` checks `serve`'s fold.
- **Consequences:** layout `crates/testkit/src/load.rs`; tests `s08_concurrency.rs`.
- **Sources (checked 2026-10-25):** candidate-concurrency-agents §10.3; judges J1, J3 grafts.

## DR-33: Freshness on read instead of a file watcher

- **Status:** accepted.
- **Starting point:** replaced "`notify`."
- **Origin:** S (D8).
- **Context:** scans are of committed state; `notify` on macOS was not built or tried [rust-stack G13].
- **Decision:** each command compares each repo's `HEAD` and refs with its active snapshot and rescans what moved; live processes poll heads every second.
- **Alternatives rejected:** `notify` 8.2 with `notify-debouncer-full` (C).
- **Consequences:** S7's commit-triggered rescan is a head change seen by the poll or the next command.
- **Sources (checked 2026-10-25):** [rust-stack §2, G13]; candidate-simplicity §15 D8.

## DR-34: No per-repo export directory

- **Status:** accepted.
- **Starting point:** replaced "A deterministic `.planroom/` export per repo (intended layer, overrides, plans, views), reviewable in PRs."
- **Origin:** S (D3).
- **Context:** no requirement asks for per-repo files; two sources of truth need their own sync and merge rules.
- **Decision:** the workspace store is the one source; `planroom export` writes events as NDJSON for backup and upgrade. `.planroom/` holds only the store and local state, with a `.gitignore` of `*`.
- **Alternatives rejected:** C's export with git-author import.
- **Consequences:** teammates converge through the Artifact and hand-offs in v0.1; a shared export is a later feature with its own requirement.
- **Sources (checked 2026-10-25):** candidate-simplicity §15 D3; judge J3 (C's export is a second source of truth).

## DR-35: Crates and packages

- **Status:** accepted.
- **Starting point:** replaced the repo layout sketch `planroom-schema · -kernel · -store · -extract-* · -packs · -decide · -server · -cli` and `packages/ schema · canvas · mod · ui`.
- **Origin:** S (D12, D13); `planroom-testkit` and `@planroom/ui` as a view-model package from C.
- **Context:** R16 wants boundaries enforced by tooling; fewer crates make the rule one sentence. `docs/contributing/design-language.md` puts the tokens in `packages/ui`.
- **Decision:** crates `planroom-contract`, `-kernel`, `-store`, `-extract`, `-models`, `planroom` (the binary and composition root), `planroom-testkit` (dev-only), plus `xtask`. Packages `@planroom/contract` (generated only), `@planroom/ui` (design tokens, view-model rows, hotkeys, braille raster, the machine stepper; no React, DOM or Node), `@planroom/canvas`, `@planroom/mod`.
- **Alternatives rejected:** a crate per extractor tier or pack (E's 27 crates); a hand-written theme inside the generated contract package (S).
- **Consequences:** [Repo layout](repo-layout.md); `same_output` works because the mod and canvas share `@planroom/ui`.
- **Sources (checked 2026-10-25):** candidate-simplicity §9; candidate-concurrency-agents §9.3; `docs/contributing/design-language.md`.

## DR-36: One boundaries file for both languages

- **Status:** accepted.
- **Starting point:** kept and combined "a crate-graph test or `cargo-deny` bans in Rust, and `dependency-cruiser` or `eslint-plugin-boundaries` in TypeScript."
- **Origin:** C (§9.3) for one source; S (§9.3) for the kernel allow-list, the pack-literal scan, the three-way self-test and the decision-record gate.
- **Decision:** `boundaries.toml` is the single source. `cargo xtask check boundaries` reads it with `cargo metadata`; `cargo xtask gen` writes `.dependency-cruiser.cjs` from it. Also: the kernel's external-dependency allow-list, a scan that no built-in pack kind ID appears as a string literal in `crates/kernel/src`, and a self-test that plants a Rust, a TypeScript and a pack crossing in a temporary copy and expects each to fail.
- **Alternatives rejected:** `eslint-plugin-boundaries` (missed 1 of 5 planted imports, mid-migration [ts-stack §6.1]); two hand-kept configs.
- **Sources (checked 2026-10-25):** [ts-stack §6.1]; [rust-stack §10]; judges J1, J2 grafts.

## DR-37: One schema in two views, one version

- **Status:** accepted.
- **Starting point:** kept and refined "`planroom-schema` (contract source)" and "a schema-to-TS generator (`json-schema-to-typescript`, `ts-rs` or `specta`)."
- **Origin:** S (§8.1, D18).
- **Context:** principle 2 asks for one versioned schema. schemars makes `#[serde(default)]` and `Option` fields optional in the default view [rust-stack G7].
- **Decision:** Rust types in `planroom-contract` are the source. `cargo xtask gen` writes `schema/planroom.v1.schema.json` (serialize view) and `schema/planroom.v1.input.schema.json` (deserialize view) with schemars 1.2.3, then `json-schema-to-typescript` 16.0.0 writes `packages/contract/src/generated/`. One integer `schema_version = 1` covers the whole contract; persisted documents also carry `format: "planroom.<name>/v1"`. Open sets use `anyOf: [{enum}, {pattern}]`.
- **Alternatives rejected:** one schema per contract with its own major (E); ts-rs (`u64` as `bigint`); specta (a release candidate); schema-first with typify (draft-07 internals) [rust-stack §9].
- **Sources (checked 2026-10-25):** [rust-stack §9, G7, G8]; [iris-release-bar §6]; judge J2 fatal flaw on E.

## DR-38: The xtask runner is the one task entry point

- **Status:** accepted.
- **Starting point:** kept "`justfile` (or `cargo xtask`)", choosing xtask.
- **Context:** `just` isn't installed [progress.md A2]; CI and agents must run the same commands.
- **Decision:** `cargo xtask {check, test, e2e, gen, release-dry-run, fixture, eval, live}`; nothing else is documented for agents. It never stops at the first failure and prints one line per check, like Iris's `check.sh`.
- **Alternatives rejected:** `just` (extra install); a `scripts/check.sh` (a second language for task logic).
- **Sources (checked 2026-10-25):** [iris-release-bar §7]; `progress.md` A2.

## DR-39: Rust library choices

- **Status:** accepted.
- **Starting point:** kept mostly "Libraries to evaluate first: Rust."
- **Decision and rejected alternatives:**

| Need | Choice (exact pin) | Rejected | Why |
|---|---|---|---|
| Parsing | `tree-sitter =0.27.1`, `tree-sitter-language =0.1.9`, grammars TypeScript 0.23.2, JavaScript 0.25.0, Python 0.25.0, Go 0.25.0, Rust 0.24.2 | `syn`, `swc`, `oxc`, language servers | one runtime for all six grammars; 690 files/s; ABI floor test for TypeScript's ABI 14 |
| Graph algorithms | `petgraph =0.8.3` | hand-rolled adjacency | toposort, paths, SCC |
| SQLite | `rusqlite =0.41.0` (`bundled`) | `sqlx 0.9.0` | synchronous single writer; 12 s build against 48 s; the two can't share a workspace (G1) |
| HTTP server | `axum =0.8.9`, `tokio =1.54.1`, `tower-http =0.6.11`, `tokio-stream =0.1.19` | actix-web, warp | SSE and static files verified; one `tower-http` copy with reqwest |
| HTTP client | `reqwest =0.13.5` with `rustls` and the ring provider | hyper directly, ureq | no OpenSSL; ring builds 22 s faster |
| Serialization and schema | `serde =1.0.229`, `serde_json =1.0.154` (`preserve_order`), `schemars =1.2.3` (`preserve_order`) | schemars 0.8 | draft 2020-12, follows serde attributes |
| CLI | `clap =4.6.7` (derive) | argh, bpaf | |
| Diffs | `similar =3.2.1` | `diff`, `dissimilar` | text hunks and ratios |
| Logging | `tracing =0.1.44`, `tracing-subscriber =0.3.23` | log | stderr only |
| Walk and globs | `ignore =0.4.33`, `globset =0.4.20` | walkdir | respects `.gitignore` |
| Regex | `regex` (linear time) | fancy-regex | a pack can't stall a scan |
| YAML | `saphyr` with markers [A, WS3] | `serde_yaml` (unmaintained) | line markers for evidence |
| Markdown | `pulldown-cmark` | comrak | |
| PromQL | `promql-parser` | hand-rolled selectors | `job` and metric matchers |
| Fuzzy search | `nucleo-matcher` | | |
| Hashes and IDs | `sha2 =0.11.1`, `uuid =1.29.0` (`v7`) | blake3 (C build), ulid 3.0 (renamed API) | |
| Errors | `thiserror =2.0.21` in libraries, `anyhow =1.0.104` in the binary and tests | snafu | |
| Tests | `insta =1.49.0`, `assert_cmd =2.2.2`, `tempfile =3.27.0`, `proptest`, `jsonschema =0.60.1`, `wiremock =0.6.5` | expect-test | |
| CI tools | `cargo-deny 0.20.2`, `cargo-nextest 0.9.149`, `cargo-about 0.9.2`, downloaded with pinned sha256 | `cargo install` (2 min 30 s) | |
| Git | the git CLI (DR-16) | `gix =0.90.0`, `git2 =0.21.0` | |
| MCP | none (DR-14) | `rmcp =3.7.0` | |
| File watching | none (DR-33) | `notify =8.2.1` | |

- **Cool-off.** Several pins were released within 14 days of 2026-10-25 (rusqlite 0.41.0, tokio 1.54.1, uuid 1.29.0, schemars 1.2.3, sha2 0.11.1, similar 3.2.1, jsonschema 0.60.1). WS1 decides between these and the newest versions at least 14 days old listed in [rust-stack §2], and records the choice here.
- **Consequences:** `deny.toml` allows MIT, Apache-2.0, Unicode-3.0, Unlicense, ISC, Zlib, BSD, CC0-1.0, MIT-0, BSL-1.0, CDLA-Permissive-2.0 and MPL-2.0; bans `openssl` and `openssl-sys`; `[profile.dev.package."*"] opt-level = 2` for tree-sitter speed (G4).
- **Sources (checked 2026-10-25):** [rust-stack §1, §2, §5, §6, §9, §10, §11].

## DR-40: TypeScript library choices

- **Status:** accepted.
- **Starting point:** kept mostly "Libraries to evaluate first: TypeScript"; XState replaced (DR-11).
- **Decision and rejected alternatives:**

| Need | Choice (exact pin) | Rejected | Why |
|---|---|---|---|
| Bundler and dev server | `vite 8.3.4`, `@vitejs/plugin-react 6.1.2` | | relative `base` works for the Artifact |
| UI | `react 19.3.1`, `react-dom 19.3.1` | | |
| Graph view | `@xyflow/react 12.12.0` | | 60 fps at 200 nodes |
| Layout | `elkjs 0.12.0` in a Web Worker, positions persisted | `@dagrejs/dagre 3.1.3` (3× slower, crashed at 500 compound nodes) | EPL-2.0: the third-party notice and source pointer ship in the archive |
| Animation | `motion 14.3.0` through `LazyMotion` | eager `motion.*` | 30.9 KB against 44.8 KB gzip |
| Primitives | `radix-ui 1.8.1` | per-primitive packages | one pin |
| State | `zustand 5.0.15` | TanStack Query 5 | a pushed, event-sourced model, not request caching |
| Schema to TS | `json-schema-to-typescript 16.0.0` | ts-rs, specta | DR-37 |
| Language and lint | `typescript 6.0.3`, `eslint 10.13.0`, `typescript-eslint 8.71.2`, `eslint-plugin-react-hooks 7.1.1` | TypeScript 7.0.2 (no typescript-eslint support), ESLint 9 (deprecated) | |
| Format | `prettier 3.9.10` | Biome 2.5 and 2.6 | Markdown and YAML support too young |
| Boundaries | `dependency-cruiser 18.6.0` | `eslint-plugin-boundaries 7.x` | DR-36 |
| Tests | `vitest 5.0.5`, `@playwright/test 1.63.0` | | Playwright matches the preinstalled browsers |
| Mod bundle | `esbuild` (ESM, no splitting) | Vite for the mod | the mod can't use `import()` [mod-api §10] |
| Fonts | `@fontsource-variable/*` 5.3.0: Bricolage Grotesque, Atkinson Hyperlegible Next; JetBrains Mono | Inter, Roboto, Arial, system-ui, Space Grotesk | design language |
| Statecharts | none (DR-11) | `xstate 5.33.3` | |

- pnpm 10.28 with `minimumReleaseAge: 10080` (7 days).
- **Sources (checked 2026-10-25):** [ts-stack §1, §2, §6, §7, §8]; [mod-api §1, §10].

## DR-41: Jev adapter and pins

- **Status:** accepted.
- **Starting point:** kept "Jev policy" and the constraint "Jev ... behind one adapter that supports both the Vercel AI Gateway and direct TypeSafe."
- **Decision:** one `JevBackend` trait and one normalized model with two codecs: `Gateway` (`/v1/evaluate`, `typesafe-ai/jev-1.14.0`) and `TypeSafe` (`/v1/systemone` with `noul`, on the direct API or the gateway's `/typesafe` path; `jev-1.14.0` if the docs list it, else `jev-latest` with `model_reported` recorded). One request per slice, many questions per request, never several slices per state. Retries only on 429, 529, 5xx and transport errors. Both codecs pass one contract test over fixtures built from the docs and the Phase 1 recordings. A key-gated smoke test records the direct API's reported version.
- **Alternatives rejected:** the unpinned `typesafe-ai/jev` alias (reports no version, `zdr: "some"`); gateway model fallbacks for escalation (escalation goes through Planroom's own Claude adapter so it's recorded and budgeted); the AI SDK (no Rust SDK).
- **Sources (checked 2026-10-25):** [jev §1, §2, §7]; KICKOFF `<verified_facts>`.

## DR-42: Levels, IDs and fact merging

- **Status:** accepted.
- **Starting point:** kept "Levels. C4 (System, Container, Component, Code) with Intent on top. One parent per node, and edges roll up with counts" and "Stable, namespaced IDs such as `<repo>:<kind>:<key>`"; kept "`layer` is actual or intended."
- **Decision:** five fixed levels; one parent; roll-ups with counts, an evidence sample and child handles, with faithfulness as a tested invariant. IDs `<namespace>:<kind-short>:<key>`; edge IDs from a hash of kind, ends and `via`. When two extractors emit the same node ID, evidence unions and scalar fields take the more specific tier's value.
- **Consequences:** tests `kernel/tests/rollup.rs` (property), `fact_merge_by_tier.rs`.
- **Sources (checked 2026-10-25):** candidate-simplicity §3.3, §3.4; `examples/TRUTH.json` about (component granularity).

## DR-43: How S13's 1 s bar is measured in CI

- **Status:** needs confirmation by the main agent.
- **Starting point:** none; implements S13 "The canvas is interactive in under 1 s on the fixture."
- **Context:** shared CI runners vary. C proposed asserting 1.5 s in CI, which J1 found a weakening without the KICKOFF process.
- **Decision:** CI asserts the median of 5 cold loads is under 1 s, records every value in the test output, and fails on any console error. The bar stays 1 s; the median is the measurement method.
- **Alternatives rejected:** a 1.5 s CI bar (a weakening); a single load (flaky on shared runners).
- **Open:** if the main agent reads a median as a weakening, the alternative is every one of 3 loads under 1 s, with retries only on runner errors.
- **Sources (checked 2026-10-25):** [ts-stack §3.4] (0.31 to 0.45 s at 200 nodes, 1.4 to 1.6 s at 4× throttling); judge J1 fatal flaw on C.
