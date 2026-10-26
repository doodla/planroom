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
| 43 | How S13's 1 s bar is measured in CI | none | added | confirmed by the main agent: strict |
| 44 | The reconciler: non-merge change units, attributed fact deltas, a persisted outcome | none | added | review round 1 (D5) |
| 45 | Intent is kernel vocabulary | Kernel: a typed property graph | extended | review round 1 (D10) |
| 46 | Pack data tables and typed emits | Packs: declarative files | extended | review round 1 (D11, D13, D21) |
| 47 | Deterministic scan bounds and a per-blob fact cache | none | added | review round 1 (D21) |
| 48 | Workspace trust, user-level credential settings and policy-bearing packs | Packs: declarative files; constraint "Claude" (key from environment or config) | extended | review round 1 (D7, D19) |
| 49 | Model calls are opt-in, consented and redacted | none (constraints "Claude" and "Spend") | added | review round 1 (D7, D8) |
| 50 | Decisions prefill before they interrupt | Jev policy | extended | review round 1 (D9, D20) |
| 51 | One spend record, a user-level ledger and a measured always-on surface | none (constraint "Spend", principle 2) | added | review round 1 (D20) |
| 52 | What agents can reach: confined reads, person-only imports, and guarded worktrees | Store: events carry the actor | extended | review round 1 (D15, D16, D18) |
| 53 | Automatic event backups and safe migrations | Store: deterministic `.planroom/` export | extended | review round 1 (D6) |
| 54 | Surfaces act through catalogue verbs, reversibly, in real keystrokes | none (R13, S9, principle 1) | added | review round 1 (D15) |
| 55 | Hand-off revert and an enumerated irreversible list | none (principle 1, R8) | added | review round 1 (D15) |
| 56 | The Artifact declares what it serves and shows synced edits | Remote mode | kept, refined | review round 1 (D15) |
| 57 | The pane before the engine and outside a workspace | none (principle 1, S14) | added | review round 1 (D15) |
| 58 | One data root, a self-locating resolver, an embedded canvas and visible background work | Repo layout: engine bootstrap | kept, refined | review round 1 (D16) |
| 59 | Repo gates that fire | Repo layout: boundary checks; `justfile` or `cargo xtask` | extended | review round 1 (D14) |
| 60 | Owner follow-ups after review round 1: agent writer instances, system writes, the person key's first registration, no card for configured packs | Store: events carry the actor | extended | main agent |

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
  - Contracts: `serve.json` holds `{port, pid, version, control_token}`; `meta.engine_version` in the store.
  - Tests: `store/tests/concurrency.rs` (3 processes × 500 writes), `store/tests/crash.rs`, the S8 harness (DR-32), and `serve_drains_for_newer_engine`.
  - Release: no service files and no lock cleanup in the installer.
  - Agents: the CLI is complete on its own, with no server to start first.
  - Users: nothing to keep running; the canvas starts `serve` on demand.
  - Fallback without a contract change: if WS4 or S8 shows contention, route CLI writes through `serve` when it runs.
- **Amended in review round 1:** multi-process WAL needs a local filesystem (unasked-5). At store open the engine checks the filesystem `.planroom/` is on: network mounts, WSL `/mnt/*`, FUSE, and macOS network volumes and sync folders. On one of those it keeps the store at `DATA_ROOT/stores/<workspace_id>/store.db` with a `.planroom/store.pointer`, and says so in one line. `doctor` reports it, and `store/tests/fs_check.rs::unsafe_filesystem_moves_the_store` tests it. Before a migration the engine writes a `VACUUM INTO` copy under the data root, keeps the last 3, and migrates and folds in one transaction. When an older `watch` or `serve` meets a newer store, the mod says "Restart this session to use Planroom vX" once instead of restarting it [DR-53] (principles-4). The drain token is now the control token, which also registers login codes [DR-29] (security-7). `serve.json` also records the catalogue hash, and `serve --detach` drains a `serve` of an older version, or of the same version with another catalogue hash, not only before a migration, and the mod shows one line when its engine's catalogue doesn't match (operability-5) [DR-58].
- **Sources (checked 2026-10-25):** [mod-api §7, §10, Design implications 1]; [rust-stack §5] (rusqlite, one writer); judge J3 (`synchronous = NORMAL` gap); candidate-simplicity §3.9, §16 R-1.

## DR-2: Store facts and events; derive the rest on read

- **Status:** accepted.
- **Starting point:** replaced "Event (append-only; everything else is derived from events)" and "SQLite in WAL mode for events, derived state and a decision cache."
- **Origin:** S (D2).
- **Context:** extracted observations are rebuildable and large; authored actions are small and precious. Storing scans as events floods the log, and derived tables need migrations and repair.
- **Decision:** the store holds two truths: facts (per repo, per commit, rebuildable) and events (authored, append-only). Everything else is a pure fold computed on read; `serve` and `watch` keep the fold in memory and apply new events incrementally. The only caches are model responses, layouts, per-blob facts and the in-memory fold.
- **Alternatives rejected:**
  - Facts as events (the starting point's literal reading): thousands of fact events per scan.
  - Projections in SQLite (E, C): a closure table and derived rows to migrate and rebuild.
- **Consequences:**
  - Layout: `planroom-kernel` holds the fold; `planroom-store` holds ten tables, plus the user-level ledger in its own file.
  - Contracts: only `events` needs migration care in the store.
  - Tests: `cargo bench fold` in `budgets.json` (under 50 ms at 20,000 facts and 5,000 events); `incremental_fold_equals_full_fold`.
  - Release: migrations are rare and small; `export` and `import` are the upgrade escape hatch.
  - Agents and users: a corrupted cache can't corrupt meaning; deleting `store.db` facts and rescanning restores them.
- **Amended in review round 1:** one derived result is stored: the reconciler's outcome, on the `scan` event, so classes don't depend on when a scan ran [DR-44] (correctness-3). Per-file extractors' facts are cached per blob in an eleventh table [DR-47] (unasked-4). Claims and presence depend on the clock, so they are an overlay with an explicit `now`, outside the pure fold, and `now` is recorded in replays (correctness-12). The ledger moves to `DATA_ROOT/ledger.db`, shared by every workspace, so the store keeps ten tables [DR-51] (cost-2).
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
- **Amended in review round 1:** `review` is a catalogue leaf, and `catalogue.rs::undo_classes_snapshot` pins every leaf's undo class, with `share` the only `NotUndoable` one (ux-3, principles-3) [DR-54, DR-55].
- **Sources (checked 2026-10-25):** candidate-simplicity §3.6, §15 D4; judge J2 graft (three edit ops).

## DR-4: Actor identity and actor rules

- **Status:** accepted; A-3 settled: the Bash tool has no TTY.
- **Starting point:** extended "Events carry the workspace, project, actor, cause and an idempotency key."
- **Origin:** C (§3.7, §3.11); fixes the fatal flaw J2 and J3 found in S, where a CLI call with no `PLANROOM_ACTOR` was attributed to a person, so an agent's Bash calls could skip ghosting and approve its own merge. E left the CLI's actor undefined.
- **Context:** R5 says nothing the AI invents counts until a person accepts it, and R8 says merges follow a person's approval. Agents reach the engine through the mod's tools and through Bash. Plugin variables aren't in the Bash tool's environment [plugin-packaging §3].
- **Decision:**
  - No terminal makes a person on its own. A person at a terminal confirms each write, and each person-only command, through a prompt read from `/dev/tty` that names the action. A write with no `PLANROOM_ACTOR`, no person key and no terminal is `actor_required`, so every Bash agent names itself (`agent:<id>:<role>`). Reads need no actor.
  - Otherwise a person actor needs `PLANROOM_PERSON_KEY`: 32 random bytes kept in the mod's `$.store`, with only its sha256 in `store.db`. The mod passes it only for a button press, an input submit, or a `/room` the person typed (`CommandRunInput.origin`). The test harness registers and passes its own.
  - Every write carries a client ID (a canvas tab, a mod session, an Artifact viewer, a CLI process), so a writer instance is `{actor, surface, client_id}` [DR-5].
  - Mod tool handlers always send `agent:<session8>:chat`. Another viewer's Artifact inbox row, in a person's import, is `person:artifact:<uid>` by its path.
  - Each catalogue command declares `any`, `person_only` or `mixed`. Person-only: picking or overriding a decision, `handoff start`, `handoff merge`, `handoff revert`, `handoff cancel`, `intent version` and `open`. Mixed: `review` (agents may reject only their own ghosts), `import` of inbox rows or an event export, `doctor --restore`, and `scan` of a credential path. An agent gets `person_required` with the card handle.
  - Ghost status is decided at commit time from the actor: any agent or model edit is a ghost, except an auto-enabled decision effect, a `low`-stakes prefill [DR-50] or an `infer` output on the actual layer.
  - The mod's `tool.check` guard resolves paths and denies any agent call that reaches the stores, writes under `.planroom/` or the data root, names a person-only command (even inside a pty wrapper), or sets `PLANROOM_PERSON_KEY`. It keeps a spawned implementer inside its worktrees [DR-52].
- **Alternatives rejected:**
  - `PLANROOM_ACTOR` defaulting to `person:<git email>` (S): the fatal flaw.
  - Session tokens from a daemon (C): needs DR-1's daemon.
  - An HMAC of each request with the key: the mod has no Node crypto, and the env var never leaves the mod's `process.run`.
- **Consequences:**
  - Layout: `crates/planroom/src/app/actor.rs`; `packages/mod/src/actor.ts`; `packages/mod/src/guard.ts`.
  - Contracts: `Actor`; `Client`; `ActorRule` on every `CommandSpec`; errors `person_required` and `actor_required`.
  - Tests: `actor.rs` (an unnamed write with no terminal is `actor_required`; a pty with no confirmation is no person; reads need no actor; a person claim without the key fails; tool handlers never send a person); `s03_plan.rs` (an agent's accept fails); `s04_handoff.rs` (an agent's merge fails).
  - Security: a guardrail for "AI proposes, person disposes", not a security boundary between processes of one OS user; `SECURITY.md` says so.
  - Users: no prompts in the mod or the canvas, where person actions are the keys and taps they already make; at a terminal, one confirmation per write.
  - A Bash tool that allocates a TTY changes nothing: a terminal alone never makes a person.
- **Amended in review round 1:** TTY-based person detection is dropped in favor of the `/dev/tty` confirmation; unnamed non-terminal writes are `actor_required`; the person key moves to the mod's `$.store` with only its hash in `store.db`; writes carry a client ID (owner decisions D1 and D2; correctness-2, security-3). `open` is person-only and never prints its code. Importing inbox rows and event exports is person-only and leaves `room_change`. The guard resolves paths, covers every file tool and pty-wrapped person-only commands, and keeps the implementer in worktrees under the data root (D15, D16; security-2, security-3, principles-1) [DR-52]. `handoff revert` is person-only, and `review` is a mixed catalogue leaf (ux-3, principles-3) [DR-54, DR-55]. A-3 settled by a live spike: the Bash tool runs with stdin and stdout not on a TTY, so an unnamed Bash write is always `actor_required`.
- **Sources (checked 2026-10-25):** [mod-api §3] (`CommandRunInput.origin`), [mod-api §8] (`tool.check` sees every tool); [plugin-packaging §3] (Bash environment lacks plugin variables); judges J2 and J3 fatal flaws on S; J2 fatal flaw on E.

## DR-5: Base required; field-level authored revisions

- **Status:** accepted.
- **Starting point:** replaced "Optimistic concurrency on entity versions."
- **Origin:** field-level compare-and-set from S (§3.9); `base_required` from C (§3.7); separating fact changes from user revisions from E (§3.10).
- **Context:** R10 says user-authored state is never last-writer-wins. S's check `request.base ⊆ current` passes for an empty base, so a write with no base was last-writer-wins (J3 fatal flaw). Whole-entity versions make a rename and a regroup of the same node conflict for no reason, and a rescan that bumps an entity's version makes a person's pending edit stale.
- **Decision:**
  - Every write to user-authored state carries `base`, the `seq` of the view the writer acted on. HTTP and tool calls without it get `base_required`; a CLI with no terminal must pass `--base SEQ`; a person at a terminal gets a read-then-confirm.
  - The conflict test is per field: a field the edit sets has an authored revision above `base`, written by another writer instance `{actor, surface, client_id}`: a canvas tab, a mod session, an Artifact viewer or a CLI process. Two tabs of one person conflict like two people; a writer instance never conflicts with its own later writes.
  - Accepting a ghost, or choosing "yours" on a conflict, writes a new `edit` that copies the fields, with `base` from the accepting view and `cause` set to the reviewed event, under the normal compare-and-set. An accept over a newer write becomes a three-way card. The `review` event stays as the audit trail.
  - Scans never change authored revisions. A fact change under an override is a `ScanChangedUnderEdit` diff item, not a stale write.
  - The mod fills `base` for tool calls from the latest `meta.seq` it returned to the model in the turn, else the pane's `seq`.
- **Alternatives rejected:** entity versions (the starting point, C); `base ⊆ current` (S as written); optional base.
- **Consequences:**
  - Contracts: `base: Seq` on write requests; `Edit.over` records the fact value an override was made over.
  - Tests: `kernel/tests/cas.rs` (rename plus regroup apply; two renames conflict; an empty base fails; a rescan never stales an edit; two tabs of one person conflict; a tab and a mod press conflict; an accept or a resolution after an intervening write is a card); S8's never-last-writer-wins check.
  - Agents: one number to pass, from the envelope they just read; the error hint says so.
  - Users: no spurious conflicts from background scans.
- **Amended in review round 1:** compare-and-set compares writer instances, not actors (D1, correctness-2), and accepting is a new write (D3, correctness-4). Contracts gain `Event.client` and `WriteCommon.client`.
- **Sources (checked 2026-10-25):** candidate-simplicity §3.9; candidate-concurrency-agents §3.7; candidate-extensibility §3.10; judge J3 fatal flaw on S; J1 graft (field-level compare-and-set); J3 graft (fact version separate from user version).

## DR-6: The mod reaches the engine through the CLI

- **Status:** accepted; A-10 settled the mod's process APIs in a cloud session, and WS5 measures them in a terminal session.
- **Starting point:** none; replaces the research suggestion of `$.http.fetch` over a per-workspace socket [mod-api, Design implications 1].
- **Origin:** S (D6).
- **Context:** `process.run` has a timeout and a 10 minute cap; `http.fetch` has neither timeout nor streaming [mod-api §7]. Agents use the CLI anyway.
- **Decision:** the mod runs commands with `$.process.run` (argv and env in, one JSON envelope out) and streams with `$.process.spawn` of `planroom watch`, restarted from the last `seq` after a reload. Mod tests stub `process.run` with golden CLI outputs that the Rust tests produce.
- **Alternatives rejected:** `http.fetch` to a daemon (E, C), which also needs DR-1's daemon.
- **Consequences:**
  - Tests: `packages/mod` tests read `tests/golden/cli/*.json`, so a contract drift fails both sides.
  - Agents: every mod action is also a CLI call an agent can make.
  - Fallback: `http.fetch` over `serve`'s socket with the same catalogue, if a surface lacks `process.run`.
- **Amended in review round 1:** `claude plugin test` exits 1 on a package with no `modules` entry and gives tests no file system, so `cargo xtask gen --mod` also builds a harness in `target/mod-test/`: the mod and its tests bundled by esbuild, the workspace packages inlined, the golden CLI outputs inlined as a generated module, and a `hooks/hooks.json` naming the bundle. `claude plugin test target/mod-test` runs it (D14, repo-6) [DR-59]. A-10 settled by a live spike: `$.process.run` works in a cloud session, and `$.process.spawn` streams stdout as it's written.
- **Sources (checked 2026-10-25):** [mod-api §7]; candidate-simplicity §15 D6; judge J1 graft (golden CLI outputs for mod tests).

## DR-7: A stale write is stored; only a person's raises a card

- **Status:** accepted.
- **Starting point:** extended "Optimistic concurrency on entity versions."
- **Origin:** C (§3.9), combined with S's rule that nothing is lost (§3.9). Resolves the judges' disagreement: J2 flagged S's cards for agent races as a principle 1 violation; J3 noted C dropped the agent's write.
- **Context:** principle 1: interrupt a person only for decisions only they can make. Under S8's load, two agents race often. R10 and the principle "never lose what someone did" forbid discarding the losing write.
- **Decision:** a stale write is always stored as an `edit` with `status: conflict`. A person's stale write raises a three-way card with "theirs" pre-selected. An agent's or model's returns `stale_write` with a compact three-way diff (at most 1 KB) in `details`, is listed in Diffs under "agent retries", collapsed, and raises no card. A person's edit over a pending ghost is no conflict: it supersedes the ghost, which is listed in Diffs as superseded and raises no card. S8's stale write is person against person: the canvas tab against a mod press on the same field.
- **Alternatives rejected:** a card for every conflict (S); an error with nothing stored (C); an automatic merge (never).
- **Consequences:**
  - Contracts: error `stale_write` with `details {event, base, theirs, yours, card}`, where `card` is present only for a person's write.
  - Tests: S8 asserts the tab's stale rename over a mod press is a card, agent B's ghost is superseded, and an agent's stale write is a stored conflict with no card; `kernel/tests/cas.rs` pins the press, tab and ghost cases.
  - Agents: they get everything they need to retry in one reply.
- **Amended in review round 1:** a person's edit supersedes a pending ghost, and S8's stale write is person against person (D4, correctness-1); choosing "yours" writes a new edit under compare-and-set [DR-5].
- **Sources (checked 2026-10-25):** candidate-concurrency-agents §3.9; candidate-simplicity §3.9; judges J2 (fatal flaw on S), J3 (concurrency gap on C).

## DR-8: Idempotency with request hashes, replays and a mod outbox

- **Status:** accepted.
- **Starting point:** extended "Events carry ... an idempotency key."
- **Origin:** C (§3.8, §3.10, §7.5).
- **Context:** S8 requires safe retries. A reused key with a different payload silently replayed the wrong result in S (J3). Spawned children and module state die on a mod reload [mod-api §7], so an in-flight write could be lost.
- **Decision:**
  - Every event stores `idem` (UNIQUE) and `request_hash`. Same key and hash: the stored reply with `meta.replayed: true`. Same key, other hash: `idempotency_mismatch`.
  - A derived key when none is given: `sha256(actor ‖ command ‖ canonical(args) ‖ base)`. Tool calls use the `tool_use_id` that every `tool.call` input carries.
  - The mod keeps an outbox atom in `$.state` and resends it with the same keys after a reload.
  - The canvas keeps pending edits in `sessionStorage`, wrapped in try/catch, until confirmed.
  - Read-your-writes needs no token (rejected graft: `as_of` and `min_seq`).
- **Alternatives rejected:** client-minted keys only (agents forget them); `min_seq` tokens (unneeded with one SQLite file).
- **Consequences:**
  - Contracts: `meta.replayed`; error `idempotency_mismatch`; the `request_hash` column.
  - Tests: S8 retries and duplicates; `store/tests/idem.rs`; a mod test that reloads mid-write and checks one event.
- **Amended in review round 1:** a command that writes several events keys them `idem#1`, `idem#2` and so on and stores the whole reply on the first; a replay of a derived key sets `meta.replayed_from` and hints to pass a new `--idem`; a decision event's key is `sha256(kind ‖ kind_version ‖ subject ‖ input_hash)` (correctness-11, correctness-9). Test `store/tests/idem.rs::multi_event_replay_returns_the_whole_reply`. A-8 settled by the spike's diagnostics: every `tool.call` input carries `tool_use_id`, which tool calls use as their key.
- **Sources (checked 2026-10-25):** [mod-api §7, §10]; candidate-concurrency-agents §3.8; judges J2, J3 grafts.

## DR-9: Packs are data; built-in extractors and linkers are declared by packs

- **Status:** accepted.
- **Starting point:** kept and narrowed "Packs. Declarative files that add kinds, levels, roll-up rules, extractors, lenses, decision kinds and a default view."
- **Origin:** S (D11); E's executable packs rejected.
- **Context:** S10 loads a pack from outside the source tree; R11 says domains plug in with no core change. A pack that runs code needs a trust model; scanned repos are untrusted input.
- **Decision:** a pack is TOML, tree-sitter queries, prompts and evals, never code. It may declare built-in Rust extractors and linkers by name, and declarative regex or tree-sitter extractors that emit nodes, edges and hints. `requires` is `["code"]` or nothing. Roll-up rules are fixed in the kernel; packs choose what a lens lifts.
- **Alternatives rejected:** exec extractors over NDJSON with a trust list, an SDK and a parity test (E, and C's exec Ruby pack): code from anywhere, no sandbox.
- **Consequences:**
  - Security: nothing in a pack runs, and the loader rejects executable files. A pack is still policy-bearing, so an out-of-tree pack's `auto_enabled` waits for a person [DR-48]; loading it deals no card, because a person listed it [DR-60].
  - Layout: `packs/` holds data only; Rust logic a pack needs lives in `planroom-extract`.
  - Tests: `tests/packs.rs` (validation, the inward-only rule, no executables).
  - Users: a pack can't do what the declarative runner and built-ins can't; a format that needs a parser is a built-in.
- **Amended in review round 1:** pack data gains `[[languages]]` and `[[image_roles]]`, typed emit props and ID prefixes, and extractor and linker names are pack-qualified [DR-46]. An out-of-tree pack is data but policy-bearing: its `auto_enabled` counts only after a person enables auto-action per kind (D19, security-5) [DR-48]. Its first-load card was dropped by DR-60. `code` is the base ring every domain pack attaches to, and the other packs form the outer ring: a pack may require only a pack in a lower ring, which in v0.1 means `code` or nothing. That is R16's "packs depend inward" for packs, and `packs.rs::requires_is_inward_only` enforces it (repo-5).
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
  - Facts gain a fourth shape, `Hint`, with an `id` and `Extracted` provenance. Extractors emit hints (URL and DSN literals, environment reads and sets, served and called routes, queue operations, database opens, SQL strings, definitions and calls) from syntax nodes, never comments or docstrings. A linked edge's provenance lists `HintRef { id, extractor }`.
  - One evidence source reads comments: the observability pack's `metric_mentions`. Its hints only attach a comment line that names a metric as evidence on that metric and its `watches` edges, and never create a relation. The `telemetry` linker maps an OTel instrument to its Prometheus name (unit suffix, `_total` for a counter).
  - `Evidence` has two variants: `Line` for file facts, verified against the blob, and `Ref` for git-derived facts (tags, commits, co-change, ownership), verified against git.
  - The release pack's `changelog` built-in sets each repo's `version` from its newest `## [x.y.z]` heading, with line evidence.
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
  - Tests: `extract/tests/link.rs` (including `metric_mentions_add_evidence_only` and `otel_counter_joins_its_prometheus_name`); `extract/tests/formats.rs::changelog_sets_repo_version`; `extract/tests/contract.rs::git_facts_carry_ref_evidence`; `s01_first_look.rs` (every truth relation with every evidence item); `s04_handoff.rs` (post-merge regression from the code alone); a docstring-is-not-evidence test.
  - Risk: call-site evidence by name may over-include; extra verified evidence is allowed ("a scan may find more" [TRUTH about]).
- **Amended in review round 1:** comments may add evidence but never relations, `Evidence::Ref` covers git-derived facts, and a `changelog` extractor supplies Go repos' versions (D21; correctness-5, correctness-6). Hints carry IDs and provenance (extensibility-4). TRUTH needs `orders/internal/http/middleware.go:26` and `orders/CHANGELOG.md:7` [TRUTH questions.observability.alerts, questions.stale_versions].
- **Sources (checked 2026-10-25):** `examples/TRUTH.json` (relations, scenarios.S4); candidate-concurrency-agents §4.2; candidate-extensibility §4.6; judges J1 (fatal flaw on E, graft `*.db_dsn`), J3.

## DR-13: Ruby is an out-of-tree data pack the fixture workspace loads

- **Status:** accepted.
- **Starting point:** kept "syntactic: tree-sitter queries and import resolvers for TS/JS, Python, Go and Rust" (no Ruby built in).
- **Origin:** J1's graft; J3 proposed a Ruby pattern file in the base `code` pack instead. E and C kept Ruby in test-only packs, so their S1 truth match failed (fatal flaws from J1 and J3).
- **Context:** `TRUTH.json` lists the `healthcheck` container and its two `GET /healthz` edges as things that must exist [TRUTH containers, relations]. `examples/README.md` says Ruby has no built-in extractor, and S10 needs a new language to work through the extractor contract alone.
- **Decision:** Ruby support is the data pack `examples/packs/ruby/`, not embedded in the binary. `cargo xtask fixture OUT` writes `OUT/.planroom/config.toml` with `packs = ["<repo>/examples/packs/ruby"]`, so every CI path and evidence run of the fixture loads it, and `feature_list.json` records that fixture state. S10 builds a fixture without that config, shows `healthcheck` absent, then loads copies of the Ruby and flags packs from a temporary directory outside the tree. Its no-core-change proof scans only core, the shipped engine crates (`contract`, `kernel`, `store`, `extract`, `models`, `planroom` with `build.rs`) and `packs/`, not `xtask` or `testkit`, for the toy packs' identifiers. It also shows that the same prebuilt binary, by sha256, runs S1 without the packs and S10 with them.
- **Alternatives rejected:**
  - Ruby in the base pack (J3, S): contradicts the fixture contract and weakens S10's proof.
  - Ruby only in the S10 test (E, C): S1's truth match fails.
- **Consequences:**
  - Layout: `examples/packs/ruby/`, `examples/packs/flags/`.
  - Tests: `s01_first_look.rs` and `s02_questions.rs` use the configured fixture; `s10_extensibility.rs` asserts absence, then presence, then `no_engine_source_mentions_the_packs` and `same_binary_runs_with_and_without_the_packs`.
  - Users: a real workspace with Ruby files gets the universal tier for them until the Ruby pack is added; the release archive ships `examples/packs/ruby` for that.
  - Open for the main agent: add one line to `examples/README.md` saying the fixture workspace loads `examples/packs/ruby` through its config (not a `TRUTH.json` change).
- **Amended in review round 1:** the S10 check is restated: core is the shipped engine crates and `packs/`, matched on pack identifiers, plus a binary-identity proof in place of `git diff --stat crates/` (D12, extensibility-3). The Ruby pack declares its language in `[[languages]]` [DR-46].
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
- **Consequences:** contracts gain `QuestionSpec { steps }`; tests `s02_questions.rs`, `kernel/tests/query.rs`, `kernel/tests/questions.rs::fires_if_lists_direct_and_indirect_alerts`, `bump_breaks_lists_consumers`.
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
- **Decision:** three kernel roles: `rescan.retarget`, `reconcile.residual`, `attach.target`. A pack's decision kind binds one with `role = "..."`. `attach.target` is raised for an accepted `kernel.item` with no `kernel.touches` edge, narrowed by the bound kind's `subject` and `when` [DR-45]. An unbound role lists its item in Diffs and deals no card; two bindings are `pack_invalid`.
- **Alternatives rejected:** naming pack kinds in the kernel (fails the domain-free scan); `reconcile.routine` (DR-20 makes it unneeded); `intent.stage` (stages are pack data already).
- **Consequences:** contracts gain `Role`; tests `kernel/tests/roles.rs` and the domain-free literal scan.
- **Amended in review round 1:** roles still bind decision kinds only; the node kinds and relation the kernel's algorithms need are kernel vocabulary instead [DR-45] (D10, extensibility-1). An unbound role no longer deals a card with no pick; it lists its item in Diffs (D9, principles-2) [DR-50].
- **Sources (checked 2026-10-25):** candidate-extensibility §5.2; judge J1 graft.

## DR-18: Claude backends: the gateway, and the session's model through the mod

- **Status:** accepted; A-9 settled.
- **Starting point:** none; implements the constraint "The mod uses the chat model and `$.model.complete`, the Artifact uses `sample`, and the engine uses Claude models on the gateway, with the model pinned in config."
- **Origin:** E's session-model adapter (§5.1), C's broker (§5.1); fixes the fatal flaw all three judges found in S, which dropped `$.model.complete` with no record (a binding constraint can't be overridden by a decision record anyway).
- **Context:** most plugin users won't set `AI_GATEWAY_API_KEY`. `$.model.complete` uses the session's own account and needs an explicit model; behind a gateway, cache marks go in `prompt` [mod-api §6]. It isn't in the session's cost totals.
- **Decision:**
  - `planroom-models::claude` has a `gateway` backend (when a gateway credential resolves; `anthropic/claude-haiku-5.5` cheap and `anthropic/claude-sonnet-5.5` capable, pinned in config) and a `session` backend (no credential and a mod attached).
  - With `session`, a Claude call becomes a `ModelRequest` listed in `meta.model_requests` and written to `.planroom/models/requests/`. The mod runs `$.model.complete` with the stable prefix in `prompt` blocks, the last marked `cache: true`, and imports a `planroom.model-response/v1` file. The response lands in `model_cache` under the same input hash, so the command picks it up when re-run.
  - The first session request deals the session provider's consent card ("Use your Claude session for Planroom's Claude calls?") with "Yes" pre-selected, recorded as a decision [DR-49]. Usage goes into the user-level ledger with `provider = session`, priced from the pinned table, and shows in the status line, as does each spawned agent's usage [DR-51].
- **Alternatives rejected:** no `$.model.complete` (S: violates the constraint); a broker over a daemon's stream (C, E: needs DR-1's daemon); the mod calling Claude for its own summaries (the engine authors every string, DR-21).
- **Consequences:**
  - Contracts: `ModelRequest`, `ModelResponse` (`planroom.model-response/v1`), `meta.model_requests`.
  - Tests: `models/tests/session_backend.rs`; a mod test with a stubbed `model.complete`; the S9 evidence run uses the session backend once.
  - Cost: the spend guards cover session usage too.
  - Open for the main agent: confirm that the session backend, used whenever the gateway isn't configured, satisfies "the mod uses `$.model.complete`", or add a second mod-local use.
- **Amended in review round 1:** the consent card is one case of the per-provider card before any billable call (D7, unasked-3) [DR-49]. The build container's proxy is a gateway credential only with `PLANROOM_LIVE_GATEWAY=1` (D8, cost-1). Session and spawned-agent usage is priced into the user-level ledger (D20; cost-2, cost-10) [DR-51]. A-9 settled by a live spike in a cloud session: `$.model.complete` with `cache: true` caches through the session's provider, a write of 14,403 tokens and then a read of 14,403.
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
  - Spend: about $0.02 to $0.05 live for a full pass [jev §9], within the build's live spend plan [DR-51].
- **Amended in review round 1:** eval runs append to the build's spend log and count against its $3 (D20, cost-2, cost-9) [DR-51]. An out-of-tree pack's `auto_enabled` waits for a person (D19, security-5) [DR-48].
- **Sources (checked 2026-10-25):** [jev §5, §8, §9, §10]; judges J1, J3 grafts.

## DR-20: One card at a time, batched by cause

- **Status:** accepted.
- **Starting point:** extended "Jev policy" and the remote mode's "shows cached decisions and queues new ones."
- **Origin:** S (§5.6); per-turn batching and the prompt section from E (§3.3, §7.5).
- **Context:** principle 1: decisions cheap, pre-decided, batched, one tap. S7: a routine rescan raises at most one card.
- **Decision:**
  - Surfaces show the top card by `stakes × staleness` and a count of the rest.
  - Ghosts from one cause (one chat turn, one plan stage run, one curation run) batch into one card ("Apply Claude's 2 edits?"), and so do one kind's cards and prefills [DR-50].
  - Unplanned changes go to Diffs and never deal a card; only regressions do.
  - The mod's `prompt.compose` section is one stable block of at most 300 tokens, byte-identical across turns and states, so it stays in the cached prefix. Per-turn hints go in the user-side context through `prompt.submit`, and the tools are registered before the engine resolves, so the tool list never changes mid-session.
- **Alternatives rejected:** a card per ghost; a `reconcile.routine` decision kind (E, C).
- **Consequences:** contracts gain `Cause::Turn`; tests `s07_decisions.rs` (at most one card), `kernel/tests/policy.rs::cards_batch_by_cause`.
- **Amended in review round 1:** one kind's cards and prefills batch like ghosts (D9, principles-2) [DR-50]. Dynamic hints, such as the remote-sync line, never enter the composed section, and the tools register from the static catalogue before the engine resolves (cost-11, principles-9). Test `packages/mod/src/prompt.test.ts::section_is_byte_identical_across_states`.
- **Sources (checked 2026-10-25):** candidate-simplicity §5.6; candidate-extensibility §3.3, §7.5; judge J2 grafts.

## DR-21: Agent-first envelope and engine-authored strings

- **Status:** accepted.
- **Starting point:** none; implements principle 2 ("structured, compact, paginated, with stable IDs and pointers to evidence").
- **Origin:** S's envelope (§7.2); handles, `ambiguous_ref`, category and the retry flag from E (§7.3); `empty.next` from C (§7.2); the string catalog and `same_output` from E (§7.1); evidence verification at ingest from E and C.
- **Context:** agents pay in tokens and need to drill down and recover without guessing; three surfaces must say the same thing; no screen may be blank.
- **Decision:**
  - One JSON document on stdout: `schema_version`, `ok`, `command`, `result` (summary first), `meta {seq, took_ms, size, next, truncated, replayed, warnings, models, model_requests}`, or `error {code, category, retry, retry_after_ms, message, hint, details}`.
  - Handles are exact command invocations.
  - Empty lists carry `empty: {reason, next: {command, args, label}}`.
  - Suffix lookups with several matches return `ambiguous_ref` with candidates.
  - Evidence is `{path, line, contains}`, verified at ingest, `contains` cut to 200 characters.
  - The engine authors every human-readable string from `crates/planroom/text/en.toml` and pack labels; `same_output` checks the CLI, the pane and the canvas show the same text.
- **Alternatives rejected:** per-surface prose; abbreviated keys.
- **Consequences:** tests `envelope_schema.rs`, `same_output`, `empty_state_audit`; contracts in [Contracts](contracts.md#cli-json-envelope).
- **Amended in review round 1:** `retryable: bool` becomes a `retry` class, `never`, `now`, `after_reread` or `later`, with an optional `retry_after_ms`, so `stale_write` tells an agent to re-read and retry. `model_unavailable` is an error for model-only commands only; `deterministic_fallback` is the one warning for that condition (principles-8). A pending session-backend request carries its exact re-run handle, and the tools' `focus` uses `--detail summary`, which drops the mini-map and keeps the top 3 presence rows (principles-10). Empty states cover every surface: before an engine responds, the mod draws fallback panes generated from `en.toml`; `focus` outside a workspace returns an empty state, not an error; and the audit runs through both canvas transports and on the pane (principles-5, ux-5) [DR-56, DR-57].
- **Sources (checked 2026-10-25):** [iris-release-bar §6]; candidates S §7.2, E §7.1 and §7.3, C §7.2; judges J2, J3 grafts.

## DR-22: A tiny surface with measured always-on cost

- **Status:** accepted.
- **Starting point:** none; implements R14.
- **Origin:** S (§7.2, §7.4, §9.6); the token snapshot from C (§5.9).
- **Context:** R14: one CLI entry point, one command family, and check whether a better default removes a concept, flag or setting. Every always-on tool and skill costs tokens every turn [plugin-packaging §Design implications].
- **Decision:** three tools (two non-deferred), four global flags plus `--base` on writes, no `--quiet`, `--verbose` or `--format`, one optional config file per workspace, plus a user-level one that alone holds credential settings [DR-48], with a default for every key, and a rule that a new setting needs a decision record saying why no default works. The only user-facing settings in v0.1 are extra pack paths, the model backends, pins and credentials, the spend limits, and `models = off`, a privacy and offline choice no default can make for the person. A snapshot test fails when non-deferred tool descriptions and schemas, skill and agent frontmatter and the mod's prompt section exceed 1,500 projected tokens, measured from the exact bytes the model receives [DR-51]. CLI help groups commands by the five ideas plus "Setup".
- **Alternatives rejected:** nine or eleven tools (C, E); an "Engine" concept heading (C).
- **Consequences:** tests `s15_surface.rs`, `tests/plugin/surface_tokens.sh`; the configuration reference is generated from `defaults.toml`.
- **Amended in review round 1:** the always-on number is measured from the exact bytes, because `claude plugin details` may miss runtime-registered tools. It has sub-budgets of 900 for tools, 300 for the section and 300 for skills and agents, and tool inputs are flat to fit. CI gates envelope and tool-result tokens, replayed Claude usage and the S3 cache-hit floor; `budgets.json` `recorded` holds the S02 chat usage and the token ratio (D20; cost-5, cost-8) [DR-51]. The user-level config is a second file of the same keys, for credential settings only [DR-48]. `room_change` is auto-allowed for its ghost-only leaves (`edit`, `intent`, `claim`, `curate`), while `scan` and `decide --run` keep the permission prompt (principles-7).
- **Sources (checked 2026-10-25):** [plugin-packaging §Design implications] (about 40 tokens per short skill); candidate-simplicity §7.4; judge J2 grafts.

## DR-23: Plan stages with their methods and structural lint

- **Status:** accepted.
- **Starting point:** extended "Structural lint runs in code; semantic lint goes to Jev."
- **Origin:** C (§6.1); J1 noted S reduced R5's methods to a sentence.
- **Context:** R5 names the methods per stage (Jobs-to-be-Done and laddering, SMART and non-goals, MECE and the WBS 100% rule, GTD and HTN, MoSCoW and an MVP line, pre-mortem, lint), and S3's lint depends on them.
- **Decision:** the plan pack declares seven stages, each with its method, kinds and fields, structural lint rules (in kernel rule types) and semantic Jev kinds, as in [Architecture](architecture.md#planning-stages-ghosts-and-lint-r5). In chat, the planning partner proposes items itself as ghosts; with no chat, `intent next` asks Claude with the plan pack's one shared cached prefix and the stage's method in the tail. Version 1 is tagged when a project's first batch is accepted; later versions are a person's.
- **Alternatives rejected:** a generic "stage with a prompt" (S).
- **Consequences:** contracts gain the `covers` rule type for the 100% rule; tests `s03_plan.rs`, `kernel/tests/lint.rs`.
- **Amended in review round 1:** stages declare the `kernel.item` types they write, with prop schemas, instead of plan-pack kinds [DR-45]. Pack `[[rules]]` lose `forbid_edge`: boundary rules are intended-layer edits only, so one violation has one report path (extensibility-11). every stage shares one plan-pack prefix, the planning rules and the union `PlanProposal` schema, with a 1-hour TTL. The stage's method goes in the uncached tail, so a plan's later stages read the cache. The budget test asserts a hit-rate floor of 0.6 after the first stage (cost-6, principles-6).
- **Sources (checked 2026-10-25):** KICKOFF R5; candidate-concurrency-agents §6.1; judge J1 graft.

## DR-24: Duties under leases instead of a jobs API

- **Status:** accepted.
- **Starting point:** none.
- **Origin:** S's duties (§2.4); C's "decisions off the scan's critical path, keyed so duplicate triggers join" (J3 graft).
- **Context:** Jev takes about 300 to 600 ms per call at the client [jev §3]; S ran decisions inline in a cold scan. Concurrent triggers shouldn't duplicate work. `http.fetch` has no timeout.
- **Decision:** background work (rescan on a moved head, the decision run, curation) is a duty that a live process runs only while holding its lease (`scan:REPO`, `decide:HASH`, `curate`), so duplicate triggers join one run. A scan never waits on Jev; with no live process, `planroom decide --run` does the work. The server replies within 10 s; a command still running at 8 s detaches into its duty and completes on the stream.
- **Alternatives rejected:** a jobs API with `job.wait` (C).
- **Consequences:** contracts gain `duty` stream frames and the `{running}` reply; tests `duties.rs::duties_join_on_lease`, `s07_decisions.rs`.
- **Amended in review round 1:** an inline rescan takes the same `scan:REPO` lease. A refresh read never waits on it and returns the current snapshot with `meta.stale_repos`; any other command waits up to 5 s and reuses the holder's snapshot. Leases carry an epoch that the committing transaction checks, and snapshot inserts are idempotent (correctness-7, correctness-9). Test `duties.rs::an_expired_holder_commits_nothing`.
- **Sources (checked 2026-10-25):** [jev §3]; [mod-api §7]; judge J3 graft.

## DR-25: Engine-made worktrees, two implementers, a cross-repo merge preflight

- **Status:** accepted.
- **Starting point:** none; replaces the research suggestion of `planroom:implementer` registered with `isolation: 'worktree'` [mod-api §5].
- **Origin:** S (§6.6, D17); the merge preflight from E (§6.6).
- **Context:** S4's change spec touches `worker` and `platform` [TRUTH scenarios.S4.implementer_patches]; Claude Code's worktree isolation covers only the session's repo. A merge that fails in the second repo must not leave the first merged.
- **Decision:** the engine makes one worktree per touched repo on `planroom/KEY`. Two implementers share one interface: `agent` (`planroom:implementer`, spawned by the mod with `cwd` at the hand-off directory) and `patch` (applies the spec's patches with `git apply --3way`; the fixture implementer and curation). `handoff merge` is person-only and preflights every repo before merging any.
- **Alternatives rejected:** agent-type isolation; merging repo by repo with no preflight.
- **Consequences:** contracts `ChangeSpec`, `ImplementerRun`; tests `s04_handoff.rs`, `handoff_preflight.rs::a_conflict_in_the_second_repo_leaves_both_untouched`.
- **Amended in review round 1:** `handoff merge` commits a `merging` step with each repo's planned target before touching git, records each repo's result, then commits `merged`. A retry resumes from the step and skips a repo whose `HEAD` already contains `planroom/KEY` (correctness-10). Contracts gain `HandoffState::Merging`; test `handoff_preflight.rs::a_crash_between_repos_resumes_on_retry`. the hand-off directory, its spec and its worktrees live in `DATA_ROOT/handoffs/<workspace_id>/KEY/`, outside `.planroom/`, and the mod's guard keeps the implementer's Edit and Write inside its worktrees (D16, security-3) [DR-52]. `handoff revert` undoes a merge in every repo behind the same kind of preflight and resume, the `merging` step records each repo's head before the merge, and cancel or drop moves the branch to `refs/planroom/archive/KEY` (D15, principles-3) [DR-55].
- **Sources (checked 2026-10-25):** [mod-api §5]; `examples/TRUTH.json` scenarios.S4; judge J1 graft.

## DR-26: The hand-off guard asks the person

- **Status:** accepted; A-2 settled.
- **Starting point:** none; implements R8 "The planning partner in the chat never silently implements."
- **Origin:** E denied the chat's own edits on the touch set (§6.6); C only added a notice (§6.4); J2 objected that E blocks edits the person asked for.
- **Context:** "never silently" is a guarantee, so a notice is too weak; blocking a person's explicit request costs attention.
- **Decision:** while a hand-off is started and not closed, the mod's `tool.check` hook stops the main thread's `Edit`, `Write`, `MultiEdit` and `NotebookEdit` on any path in its touch set and asks the person through the permission prompt, with one line naming the hand-off. If `tool.check` can't ask, it denies with that line and names `/room handoff cancel KEY`.
- **Alternatives rejected:** deny always (E); notice only (C).
- **Consequences:** tests `packages/mod/src/guard.test.ts`; S9 evidence shows the prompt once.
- **Amended in review round 1:** A-2 settled from the 2.1.310 types (`ToolCheckResult`): `tool.check` returns `ask` with the reason, and where nobody can be asked Claude Code turns it into a deny with that reason. So the guard returns one value, `ask`, whose line names `/room handoff cancel KEY` for the deny case.
- **Sources (checked 2026-10-25):** [mod-api §4, §8]; judges J1 (graft from E), J2 (objection).

## DR-27: Curation drafts: templates, then Claude, then the curator agent

- **Status:** accepted.
- **Starting point:** none; implements R12.
- **Origin:** S's templates (§6.8); C's Claude drafting and its two CI paths (§6.5, §10.2).
- **Context:** S12 must pass deterministically; drafts should read well when a model is reachable.
- **Decision:** miners (gotchas, procedures, stale lines) produce template drafts. When a model is reachable, a cheap-tier Claude call refines each draft with the target file in its cached prefix; in a live session the hidden curator agent can rework drafts that span files. Every draft must mention the cluster's key tokens before it's proposed. CI runs S12 twice: with replayed Claude and with models off.
- **Alternatives rejected:** Claude-only drafts; template-only drafts.
- **Consequences:** contracts `CurationDraft`; tests `s12_curation.rs` (both paths).
- **Amended in review round 1:** curation mines runs from `handoff report` and from a person's import only; agent-imported runs are kept untrusted. Cards name their source runs and show in full any draft with a URL, command substitution, a shell pipe or `allowed-tools` (security-9) [DR-52]. The curator spawns only after a person's card or `/room curate`, and its usage goes into the ledger (cost-10) [DR-51].
- **Sources (checked 2026-10-25):** `examples/TRUTH.json` scenarios.S12; judge J1 graft.

## DR-28: Engine bootstrap without a SessionStart hook

- **Status:** accepted.
- **Starting point:** kept and refined "plugin/ ... engine bootstrap."
- **Origin:** S (§11.2, D20); the source-build card from E (§11.2).
- **Context:** plugins can't hold a `bin/` folder; mixing `modules` with classic hooks in one `hooks.json` is unverified; an interactive SessionStart hook holds Claude's first reply [plugin-packaging §3].
- **Decision:** `plugin/scripts/planroom` resolves `$PLANROOM_ENGINE`, the dev checkout found from the script's own path, the cache under the data root, then a sha256-verified download for the fixed tag, then a source build after a person's card. The mod starts `prefetch --detach` in `session.start`; if the fetch fails and the toolchain preflight passes, it deals one card, "Build the engine from source, about 4 minutes?", with progress in the status line.
- **Alternatives rejected:** a SessionStart prefetch hook; `releases/latest` (403 through the proxy).
- **Consequences:** tests `tests/installer/run.sh`; release writes `plugin/scripts/engine.env`.
- **Amended in review round 1:** the resolver uses no plugin variable, since the Bash tool sets none: it finds `engine.env` and the dev checkout from its own path, and its cache is `DATA_ROOT/engine/VERSION/`, shared by the mod, the Bash tool and agents. The source build runs from a checkout when the plugin is loaded from one, preflights cargo, a C compiler and the network, runs detached with a log and a pid file, and says so when the release tag doesn't exist yet. A stale fetch lock breaks (D16; operability-1, operability-3, unasked-8) [DR-58].
- **Sources (checked 2026-10-25):** [plugin-packaging §3, Design implications]; [iris-release-bar §3]; judge J2 graft.

## DR-29: Browser login with a one-time code

- **Status:** accepted.
- **Starting point:** extended "A single-writer local server per workspace" (its exposure).
- **Origin:** C (§12.2); J3 flagged S's `?t=` query-string token.
- **Context:** a token in a query string lands in logs and history.
- **Decision:** `planroom open` (person only) makes a 128-bit one-time code, registers its sha256 with `serve` through the control token, and opens the URL with the code in the fragment in the browser itself; it never prints the code. The code works once, within 60 s, for that `serve` process. The page exchanges it for a random bearer token, kept in the tab's memory and sent as `Authorization: Bearer`, the stream included, which the canvas reads through `fetch`. Every API request needs that token, an exact `Origin` (a missing one is refused) and a matching `Host`; no CORS; strict CSP. `.planroom/` is 0700 and its files 0600.
- **Alternatives rejected:** a bearer token in the query string (S); an `HttpOnly`, `SameSite=Strict` cookie, the first design, which a browser sends to every other server on `127.0.0.1`, since cookies ignore the port and every local port is one site.
- **Consequences:** tests `crates/planroom/tests/server_auth.rs` (no token, a missing or foreign `Origin`, a wrong `Host`, a reused or expired code); `store/tests/fs_check.rs::store_files_are_0600_in_a_0700_directory`. Users: a reload of the tab asks for a new code, one `e` press.
- **Amended in review round 1:** an in-memory bearer token sent as a header replaces the cookie, and every request needs an exact `Origin` (D17, security-7). The code has 128 bits and a 60 s life, reaches `serve` through the control token, and is never printed (security-3). `.planroom/` is 0700 and its files 0600.
- **Sources (checked 2026-10-25):** candidate-concurrency-agents §12.2; judge J3 graft.

## DR-30: Skills hidden from the slash menu

- **Status:** accepted; A-1 settled.
- **Starting point:** none; implements R14 and S15 "one command family."
- **Origin:** a fatal flaw the judges found in all three candidates.
- **Context:** plugin skills appear as `/planroom:<name>` slash commands [plugin-packaging §3], outside the `/room` family.
- **Decision:** three skills (`room`, `room-plan`, `room-handoff`), each with `user-invocable: false` in frontmatter, so the model can load them and the slash menu doesn't list them. The S15 snapshot asserts the headless session's `slash_commands` list holds only `/room` from Planroom. If `user-invocable: false` isn't honored on Claude Code 2.1.310, ship one skill named `room`, whose `/planroom:room` entry is the same family.
- **Alternatives rejected:** four or five visible skills (all candidates).
- **Consequences:** tests `tests/plugin/slash_commands_snapshot.sh` (evidence path, needs a session) and a static frontmatter check in CI.
- **Amended in review round 1:** A-1 settled by a live spike (headless `claude -p` with a spike plugin): a skill with `user-invocable: false` is absent from `slash_commands` and the init `skills` list, and the model still invokes it with the Skill tool. The slash-command snapshot runs only on S15's evidence path, and S15's CI path has the static check `s15_surface.rs::skills_are_not_user_invocable` (D14, repo-7) [DR-59].
- **Sources (checked 2026-10-25):** [plugin-packaging §3]; judge J2 fatal flaw (all three).

## DR-31: Remote mode: a published snapshot, a small live head, an attributed inbox

- **Status:** accepted, verify in WS7 and WS8.
- **Starting point:** kept and refined "Remote mode: the workspace is the source of truth, the session pushes snapshots to the Artifact db; Artifact edits land as events in a db collection; the session imports them on `/room open`, at turn start, and on edit notices; the Artifact shows cached decisions and queues new ones."
- **Origin:** S (§7.6); snippets excluded by default and the first-publish summary from C (§12.5); C's timer loop rejected (J2).
- **Context:** `db` writes are last-writer-wins page-side, need `if_version` session-side, and don't wake the session; bulk data fits a published data file; a `{self}` path proves the author [artifact §2, §6; artifact-spike].
- **Decision:** the bulk model ships as `data/snapshot.json` (code text excluded by default); a small `live/head` document carries the top cards and recent changes; viewers write only `inbox/{self}/events/{docId}`, and only the owner reads `inbox`. A person's `planroom import` commits other viewers' rows as attributed person edits through the same compare-and-set. It turns the owner's own rows and person-only rows into one card [DR-52]. Sync runs on `/room sync`, `/room` opening, the page's Send to Claude, and a hint in the per-turn context; a wake carries only "sync requested". The mod never calls `ArtifactData` from a timer and never auto-allows it.
- **Alternatives rejected:** all data in `db` shards (25,000-document cap, many batches); a 5 s mod timer auto-allowed through `tool.check` (C).
- **Consequences:** contracts `Snapshot`, `LiveHead`, `InboxRow`; tests `s11_remote.rs`, `tests/e2e/artifact.spec.ts` with a fake `db` that has the real rules.
- **Amended in review round 1:** Importing inbox rows is person-only and in no tool, and the owner's rows and person-only rows become one confirming card (D15; security-2, principles-1) [DR-52]. Only the owner reads `inbox`. A wake carries only "sync requested", and viewer text is data. The first-publish line comes from the snapshot's sections, and presence is left out unless `--include-presence` is passed (security-8). The sync hint is per-turn context, never the composed section (principles-9). The page logs each `sample` call for the ledger (cost-2) [DR-51]. The snapshot names what it serves and carries the empty state for the rest; `live/head` carries the data delta since the snapshot, which the page applies; the page marks its rows queued until `acks/<uid>` lists them; the manifest declares `comments` for Send to Claude; and an inbox row may name only `edit`, `review`, `decide` or `intent propose` (D15; ux-5, ux-7, ux-1) [DR-56].
- **Sources (checked 2026-10-25):** [artifact §2, §5, §6]; [artifact-spike]; judges J2 (fatal flaw on C's timer), J2 graft (snippets).

## DR-32: The S8 concurrency harness

- **Status:** accepted.
- **Starting point:** none; implements S8.
- **Origin:** C (§10.3).
- **Context:** S8 needs proof of no lost writes, idempotent retries, live updates within 1 s, claims and presence, and a stale write that becomes a diff, not a merge.
- **Decision:** two agent CLI writers on two projects, an HTTP writer as a canvas tab plus an SSE reader, 10% of writes killed after commit and before the reply and retried with the same key, 5% duplicated on purpose, a claim, and one field renamed by agent B (a ghost), then by a mod press, then by the tab from a base read before the press. Live latency is measured from commit to rendered: the tab's `data-seq` marker and the pane's `seq`. Faults are compiled only in test builds (`PLANROOM_TEST_FAULTS`). A `writer_interleavings` proptest checks the same invariants in-process; `incremental_fold_equals_full_fold` checks `serve`'s fold.
- **Consequences:** layout `crates/testkit/src/load.rs`; tests `s08_concurrency.rs`.
- **Amended in review round 1:** S8's stale write is two person writer instances, with B's ghost superseded (D4, correctness-1), and the latency gate is commit to rendered (D21, correctness-7).
- **Sources (checked 2026-10-25):** candidate-concurrency-agents §10.3; judges J1, J3 grafts.

## DR-33: Freshness on read instead of a file watcher

- **Status:** accepted.
- **Starting point:** replaced "`notify`."
- **Origin:** S (D8).
- **Context:** scans are of committed state; `notify` on macOS was not built or tried [rust-stack G13].
- **Decision:** each command compares each repo's `HEAD` and refs with its active snapshot and rescans what moved; live processes poll heads every second. A refresh read (`focus` and every surface's refetch) never rescans inline while a live process holds the repo's `scan:REPO` lease. Surfaces refresh on the first frame of a burst, after quiet, and at a maximum wait of 300 ms (canvas) or 500 ms (mod).
- **Alternatives rejected:** `notify` 8.2 with `notify-debouncer-full` (C).
- **Consequences:** S7's commit-triggered rescan is a head change seen by the poll or the next command.
- **Amended in review round 1:** leading-plus-trailing refresh with a maximum wait, and no inline rescan in refresh reads (D21, correctness-7).
- **Sources (checked 2026-10-25):** [rust-stack §2, G13]; candidate-simplicity §15 D8.

## DR-34: No per-repo export directory

- **Status:** accepted.
- **Starting point:** replaced "A deterministic `.planroom/` export per repo (intended layer, overrides, plans, views), reviewable in PRs."
- **Origin:** S (D3).
- **Context:** no requirement asks for per-repo files; two sources of truth need their own sync and merge rules.
- **Decision:** the workspace store is the one source; `planroom export` writes events as NDJSON for upgrade, and the engine appends the same format to a backup under the data root after each person-authored commit [DR-53]. `.planroom/` holds only the store and local state, with a `.gitignore` of `*`.
- **Alternatives rejected:** C's export with git-author import.
- **Consequences:** teammates converge through the Artifact and hand-offs in v0.1; a shared export is a later feature with its own requirement.
- **Amended in review round 1:** with no per-repo export, nothing else held a person's events, so `git clean -xfd` lost them. An automatic, append-only backup under the data root and a restore card close that, without a second source of truth: the backup is read only to restore (D6, principles-4) [DR-53].
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
- **Decision:** `boundaries.toml` is the single source. `cargo xtask check boundaries` reads it with `cargo metadata`; `cargo xtask gen` writes `.dependency-cruiser.cjs` from it. Also: the kernel's external-dependency allow-list, a scan that no string literal in `crates/kernel/src` equals a built-in pack kind or relation name, with or without its `<pack>.` prefix, and a self-test that plants a Rust, a TypeScript and a pack crossing in a temporary copy and expects each to fail.
- **Alternatives rejected:** `eslint-plugin-boundaries` (missed 1 of 5 planted imports, mid-migration [ts-stack §6.1]); two hand-kept configs.
- **Amended in review round 1:** the literal scan matches bare kind and relation names too, and the self-test plants a bare relation name (D10, extensibility-1) [DR-45]. The self-test installs JS dependencies offline in its copy, runs every check on the unmodified copy as a control, and plants a declared `workspace:*` crossing and a relative import, expecting exactly the rule's name and no unresolvable import. It also plants a layout change without a record (D14, repo-2) [DR-59].
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
- **Amended in review round 1:** every `cargo xtask` command sets `PLANROOM_REPLAY=strict`, `PLANROOM_SPEND_LOG=docs/evidence/spend.jsonl`, and for tests a temporary `PLANROOM_DATA`, and this repo's `.claude/settings.json` sets the first two for agents (D8, D20; cost-1, cost-2) [DR-49, DR-51]. `cargo xtask fixture` records its fixture as a trusted workspace [DR-48]. CI runs only `cargo xtask` commands, and `ci_runs_only_xtask` fails on any other `run:` step except a listed setup step. `cargo deny check` (licenses, bans, sources) joins `check`; advisories and `pnpm audit` become `check audit`; the installer suite becomes `test --installer`; the guardrails job runs `check guardrails`. Every command first installs JS dependencies from the lockfile, offline when it can (D14; repo-3, repo-11) [DR-59].
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
| Tests | `vitest 5.0.5`, `@playwright/test 1.63.0` | | `cargo xtask e2e` installs the matching Chromium |
| Mod bundle | `esbuild` (ESM, no splitting) | Vite for the mod | the mod can't use `import()` [mod-api §10] |
| Fonts | `@fontsource-variable/*` 5.3.0: Bricolage Grotesque, Atkinson Hyperlegible Next; JetBrains Mono | Inter, Roboto, Arial, system-ui, Space Grotesk | design language |
| Statecharts | none (DR-11) | `xstate 5.33.3` | |

- pnpm 10.28 with `minimumReleaseAge: 10080` (7 days).
- **Amended in review round 1:** CI runners have no preinstalled browsers, so `cargo xtask e2e` runs `playwright install chromium` at the pinned version, with `--with-deps` in CI. pnpm is pinned exactly in `packageManager` (10.28.2, as WS1 committed it), and Dependabot waits 7 days like `minimumReleaseAge` (operability-10, operability-12, repo-11) [DR-59].
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
- **Decision:** five fixed levels; one parent; roll-ups with counts, an evidence sample and child handles, with faithfulness as a tested invariant. IDs `<namespace>:<kind-short>:<key>`, where kind-short is the kind's declared `id_prefix`, owned by one pack [DR-46]; edge IDs from a hash of kind, ends and `via`. When two extractors emit the same node ID, evidence unions and scalar fields take the more specific tier's value.
- **Consequences:** tests `kernel/tests/rollup.rs` (property), `kernel/tests/fold.rs::fact_merge_by_tier`.
- **Amended in review round 1:** kind-short is defined as `KindDecl.id_prefix`; two packs may not declare one prefix (extensibility-8).
- **Sources (checked 2026-10-25):** candidate-simplicity §3.3, §3.4; `examples/TRUTH.json` about (component granularity).

## DR-43: How S13's 1 s bar is measured in CI

- **Status:** accepted (decided by the main agent, 2026-10-26).
- **Starting point:** none; implements S13 "The canvas is interactive in under 1 s on the fixture."
- **Context:** shared CI runners vary. C proposed asserting 1.5 s in CI, which J1 found a weakening without the KICKOFF process. The synthesis proposed the median of 5 cold loads.
- **Decision:** CI asserts that every one of 5 cold loads is interactive in under 1 s, records every value in the test output, and fails on any console error. A load is retried only when the runner itself fails (the browser or server didn't start), never because it was slow. A median would let two slow loads pass, which reads as a weakening of "interactive in under 1 s".
- **Alternatives rejected:** a 1.5 s CI bar (a weakening); the median of 5 (lets slow loads pass); a single load (one sample says little about the bar).
- **Consequences:** the canvas must stay well under the bar, not just near it: persisted layouts, folded first views and a small first-paint bundle (about 215 KB gzip measured) are requirements, not optimizations. If shared runners make the gate flaky, the fix is in the canvas or the harness, never the threshold.
- **Amended in review round 1:** every wall-clock gate, this one, the cold scan, the fold benchmark and S8's commit to rendered, runs only on ubuntu-24.04 and writes every sample to the job summary. On macos-15, `cargo xtask test` asserts the same functional invariants and records the timings without gating them. No threshold changes (operability-11).
- **Sources (checked 2026-10-25):** [ts-stack §3.4] (0.31 to 0.45 s at 200 nodes, 1.4 to 1.6 s at 4× throttling); judge J1 fatal flaw on C.

## DR-44: The reconciler: non-merge change units, attributed fact deltas, a persisted outcome

- **Status:** accepted (decided by the main agent in review round 1, D5).
- **Starting point:** none; implements R7's reconciliation and S4 step 4.
- **Origin:** review round 1, correctness-3.
- **Context:** the reconciler named change units and a per-commit fact delta without saying how either is computed. Facts exist only for head snapshots, and merge commits had no rule. A window of "since the last scan" also lost a regression at the next scan. On S4, `git merge --no-ff teammates` makes merge commits, and `teammates` starts from a `main` older than the hand-off [TRUTH scenarios.S4.reconciliation_after_merges].
- **Decision:**
  - The fact delta runs from the previous reconciled snapshots to the current head snapshots, for the workspace link and each repo.
  - Each changed fact goes to the non-merge commits in `git log --no-merges prev..head` whose touched files intersect its evidence paths.
  - Merge commits are never change units. That includes a hand-off's own merge, which has `planroom/KEY` as its second parent.
  - The outcome is the `reconcile` body of the scan's last `scan` event: each repo's reconciled head and snapshot, the item classes, each regression's `against`, and the `now` the claims overlay used. The next scan starts from it. A scan that finds a newer outcome recomputes before it commits.
  - A fact missing only because a partial scan didn't cover its file is not a delta [DR-47].
- **Alternatives rejected:**
  - A first-parent delta: it puts the regression on "Merge branch teammates".
  - A delta against each commit's own parent: the rule wasn't met there, so no regression shows.
  - Classes derived on read: they change with scan timing, and "stays done" can't hold.
- **Consequences:**
  - Contracts: `EventBody::Scan.reconcile: Option<ReconcileOutcome>`; still eight event kinds.
  - Store: snapshot pruning keeps the snapshots the last outcome names.
  - Tests: `kernel/tests/reconcile.rs::merge_commits_are_never_change_units`, `a_branch_from_an_older_main_keeps_its_regression`, `classes_survive_later_scans`, `a_partial_scan_raises_no_regression`; `s04_handoff.rs::reconciliation_after_merges_matches_truth`.
  - It amends DR-2: one derived result is stored.
- **Sources (checked 2026-10-26):** `examples/TRUTH.json` scenarios.S4; `examples/fixture/repos/worker/10-audit-rows/COMMIT.toml`; review round 1 correctness-3 and owner decision D5.

## DR-45: Intent is kernel vocabulary

- **Status:** accepted (decided by the main agent in review round 1, D10).
- **Starting point:** extended "A typed property graph of four primitives" with R14's Intent idea.
- **Origin:** review round 1, extensibility-1, option a.
- **Context:** the kernel's own algorithms needed plan-pack names. The reconciler read `touches`, the attach trigger read next actions, and projects were fixed to `plan:<project>`. The `tree` shape carried PR thread status, and the machine shapes read fixed prop names. The literal scan caught only `pack.` prefixes, so a coupled kernel could still pass it. R14 makes Intent one of the five core ideas, and R16 says the kernel depends on no domain.
- **Decision:**
  - The kernel defines `kernel.project`, `kernel.item` and the relation `kernel.touches`, as it defines `kernel.group` and `kernel.external`. A plan item is a `kernel.item` whose `type` prop names a plan-pack item type, such as `next_action`.
  - The plan pack supplies stages, their item types and prop schemas, methods, prompts, lint and decision kinds on top.
  - `attach.target` is raised for an accepted `kernel.item` with no `kernel.touches` edge, narrowed by the bound kind's `subject` and `when`.
  - The `machine` and `checks` shapes take their prop names (event, guard, action, initial, terminal) as parameters.
  - PR thread status leaves the `tree` shape. The artifacts pack's `pr` question adds a `filter` step over the PR's thread parts.
  - The domain-free scan fails on a kernel string literal that equals a built-in pack kind or relation name, with or without its `<pack>.` prefix.
- **Alternatives rejected:**
  - Declared kind and relation roles in pack data (option b): a second role mechanism for one of the five ideas.
  - Names passed from `app` as constants: the kernel stays coupled while the check passes.
- **Consequences:**
  - Contracts: `StageDecl.items`; `Breakdown` loses `threads`; `Kind` lists the kernel's own kinds.
  - Layout: the `domain_free` rule in `boundaries.toml` and a bare-name case in the self-test.
  - Tests: `kernel/tests/roles.rs`; `s03_plan.rs`; `cargo xtask check boundaries --self-test`.
  - Users: no visible change; plan items show their type as before.
- **Sources (checked 2026-10-26):** KICKOFF R11, R14, R16; review round 1 extensibility-1 and owner decision D10.

## DR-46: Pack data tables and typed emits

- **Status:** accepted (decided by the main agent in review round 1, D11, D13 and D21).
- **Starting point:** extended "Packs. Declarative files that add kinds, levels, roll-up rules, extractors, lenses, decision kinds and a default view."
- **Origin:** review round 1: extensibility-2, extensibility-5 and correctness-8; the minor findings extensibility-8 and extensibility-10.
- **Context:**
  - S1 compares each repo's languages, and only a language table in the engine could supply `ruby`, which S10 forbids.
  - Emit props were strings only and edges had none, so the toy pack's boolean `enabled` couldn't meet its own `when`.
  - Nothing classified compose services, so observability infrastructure counted as unmonitored.
  - Kind-short was undefined, and the Ruby example's `parent` was no valid reference.
  - Regex extractors had no notion of comments.
- **Decision:**
  - `[[languages]]` (`id`, `extensions`, `line_comment`, `block_comment`) is pack data. The code pack declares TypeScript, TSX, JavaScript, Python, Go, Rust, YAML and JSON; the Ruby pack declares `ruby`. The universal `files` extractor reads the merged table and sets each file's `language`. A repo's languages are the union over its files.
  - The declarative runner blanks comment spans by those markers before it matches a regex, and keeps line numbers.
  - `[[image_roles]]` (`image`, `role`, `kind`) is pack data, merged across packs and read by `compose`. The code pack maps database and queue images. The observability pack maps the collector, Prometheus and Grafana to `observability.infra` with role `observability`. Every container carries a `role`, its image rule's or else its kind's name, and the truth comparator matches a TRUTH container's `kind` against it. `unmonitored` filters runtime containers: `role` is `service`, `datastore` or `queue`.
  - Emit props on nodes and edges are TOML values. A capture is coerced to the type the kind's or relation's prop schema declares, and `RelationDecl` gains a prop schema.
  - A kind declares its `id_prefix`, the kind-short in its IDs. Kinds of one pack may share one; two packs may not. Templates refer to a node by `@id:KIND:KEY`.
  - Extractor and linker names are pack-qualified, such as `ruby/ruby-http`, and unique at load.
- **Alternatives rejected:**
  - An extension map in `extract::files`: it puts `.rb` in the engine.
  - String props everywhere: the boolean never matches.
  - A unique prefix per kind: a reclassified container would change its ID.
- **Consequences:**
  - Contracts: `LanguageDecl`, `ImageRole`, `KindDecl.id_prefix`, `RelationDecl.props`, typed `EmitTemplate` props.
  - Tests: the conformance kit gains a boolean prop that a `when` selects and a commented URL that emits nothing; `extract/tests/contract.rs::files_language_from_the_languages_table`, `emit_props_coerce_to_the_prop_schema`, `regex_skips_comment_spans`; `extract/tests/formats.rs::compose_kind_from_image_roles`; `packs.rs::id_prefixes_are_owned_by_one_pack`, `extractor_names_are_pack_qualified`.
  - Users: a new language is one `[[languages]]` entry and its extractors.
- **Sources (checked 2026-10-26):** `examples/TRUTH.json` repos, containers, questions.unmonitored and scenarios.S10; review round 1 findings above and owner decisions D11, D13 and D21.

## DR-47: Deterministic scan bounds and a per-blob fact cache

- **Status:** accepted (decided by the main agent in review round 1, D21).
- **Starting point:** none; replaces the extractor contract's 2 s wall-clock budget.
- **Origin:** review round 1, unasked-4.
- **Context:** a 2 s cut per extractor made a large repo's facts depend on machine load, which breaks "the same commit gives byte-identical facts". A dropped edge then read as a regression or a scan-changed-under-edit. Every head move also re-extracted every file.
- **Decision:**
  - Each extractor takes files in sorted path order, up to 50,000 files and 256 MiB per repo. A cut is the same on every machine and reports `partial_scan` with the uncovered paths.
  - Per-file extractors cache facts by `(blob sha, extractor, packs_hash)` in `blob_facts`, so a commit re-extracts only the blobs it changed.
  - A fact missing because a partial scan didn't cover its file raises no regression and no scan-changed-under-edit.
- **Alternatives rejected:**
  - A wall-clock budget: nondeterministic.
  - A persisted fold cache now: no scenario needs it, and bet 2 keeps it as the fallback.
- **Consequences:**
  - Store: a `blob_facts` table.
  - Contracts: warning `partial_scan`.
  - Tests: `extract/tests/contract.rs::bounds_cut_in_sorted_path_order`, `unchanged_blobs_reuse_cached_facts`; `kernel/tests/reconcile.rs::a_partial_scan_raises_no_regression`.
- **Sources (checked 2026-10-26):** [rust-stack §4] (690 files/s); review round 1 unasked-4 and owner decision D21.

## DR-48: Workspace trust, user-level credential settings and policy-bearing packs

- **Status:** accepted (decided by the main agent in review round 1, D7 and D19).
- **Starting point:** extended "Packs. Declarative files ..." and the constraint "The engine reads its gateway key from environment or config."
- **Origin:** review round 1: security-1, security-5 and security-11.
- **Context:**
  - Workspace discovery loaded `.planroom/config.toml` from the root with no check of where it came from. A cloned repo could commit one that chose the Claude host, the variable that supplies the key, and extra packs. The first Claude call would then send any environment secret, and repo content, to any host.
  - An out-of-tree pack runs no code, but it supplies the trusted prompt prefix, decision kinds with effects, and its own eval result file. A hand-written `auto_enabled: true` could apply effects with no measured precision, because CI recomputes results only for in-tree packs.
  - KICKOFF lets the engine read its gateway key from the environment or config, while the design said "never from config" with no record.
- **Decision:**
  - A `.planroom/` is trusted only when the data root's `workspaces.json` records that the engine created it and git tracks nothing inside it. An untrusted one keeps its config unloaded, with one card ("Trust it?") and a `workspace_untrusted` warning. `cargo xtask fixture` records its fixture as trusted.
  - Credential settings (the Claude and Jev backends, `base_url`, `api_key_env`, the key) come only from the environment or the user-level config, `${XDG_CONFIG_HOME:-$HOME/.config}/planroom/config.toml`. The gateway key may sit there as `[claude] api_key` at mode 0600, which keeps KICKOFF's "or config". A workspace's file never holds them; the engine ignores them there with `config_key_ignored`.
  - `api_key_env` is one of `AI_GATEWAY_API_KEY`, `VERCEL_OIDC_TOKEN` or `TYPESAFE_API_KEY`. A credential goes only to `ai-gateway.vercel.sh`, `api.typesafe.ai`, or the host of a user-level `base_url`.
  - An out-of-tree pack is data but policy-bearing. Its `auto_enabled` counts only after a person enables auto-action for that kind with a person-only card that shows the holdout numbers.
- **Alternatives rejected:**
  - Loading any `.planroom/` config: the attack above.
  - Environment-only keys: a departure from KICKOFF's "or config", which a 0600 user-level file meets safely.
  - Re-running an out-of-tree pack's evals at load: needs live calls and the pack's recorded responses.
- **Consequences:**
  - Contracts: the user-level config and the "Where" column in [Configuration](contracts.md#configuration); warnings `workspace_untrusted` and `config_key_ignored`.
  - Tests: `crates/planroom/tests/trust.rs::a_committed_planroom_config_is_not_loaded`, `credential_settings_come_only_from_user_config`, `api_key_env_outside_the_allow_list_is_refused`, `credentials_go_only_to_allow_listed_hosts`, `an_out_of_tree_pack_loads_without_a_card` (DR-60), `an_out_of_tree_pack_cannot_enable_its_own_auto_action`.
  - Scenarios: the Ruby pack's first-load card is dealt on every fixture; S7's warm phase keeps it before the rescan.
  - Users: a cloned repo with Planroom state asks once before its config counts.
- **Sources (checked 2026-10-26):** KICKOFF `<constraints>` Claude; review round 1 security-1, security-5, security-11 and owner decisions D7 and D19.

## DR-49: Model calls are opt-in, consented and redacted

- **Status:** accepted (decided by the main agent in review round 1, D7 and D8).
- **Starting point:** none; serves the constraints "Claude" and "Spend" and the security gap KICKOFF leaves to the builder.
- **Origin:** review round 1: security-6, unasked-3 and cost-1.
- **Context:**
  - With a credential in the environment, the default config sent repo slices to third-party providers on the first decision, with no disclosure. Extractors keep secret-bearing values, such as compose `environment` values and DSN user info, and `kernel::redact` covered only logs, error details and the snapshot.
  - The `auto` backend counted the build container's proxy as a credential, so any product run in the build session spent live money outside `xtask live`, against KICKOFF's $3 cap and its "gate every live test behind an env flag".
- **Decision:**
  - An ambient proxy is never a credential, unless `PLANROOM_LIVE_GATEWAY=1` (Claude) or `PLANROOM_LIVE_JEV=1` (Jev) is set. `cargo xtask` and this repo's `.claude/settings.json` set `PLANROOM_REPLAY=strict`. In a duty, `replay_missing` counts as no model reachable.
  - Before the first billable call per workspace and provider, one card names the provider, what is sent and the expected cost. Yes is pre-selected for the session backend and for a credential the user-level config names; No for a credential found only in the environment. Until Yes, decisions keep their defaults and a model-only command returns `consent_required`. Strict replay counts as consented.
  - `kernel::redact` runs on every Jev state, Claude prompt, session-backend request file and mod tool output (tool handlers set `PLANROOM_REDACT=1`). It masks DSN user info, values of `*_KEY`, `*_SECRET`, `*_TOKEN` and `*_PASSWORD` variables, private keys and high-entropy strings, deterministically. The local CLI `--json` keeps raw `contains`, so the truth comparator matches.
  - `SECURITY.md` lists each provider and what it receives.
- **Alternatives rejected:**
  - Pre-selecting Yes for every credential: an ambient key set for another project would ship code unasked.
  - Redacting the local CLI too: the truth comparator needs the exact lines.
  - `PLANROOM_MODELS=off` as the dev default: it would hide replay drift that strict replay reports.
- **Consequences:**
  - Contracts: error `consent_required`; `deterministic_fallback` covers "not allowed yet".
  - Tests: `crates/planroom/tests/live_gate.rs::the_proxy_alone_makes_no_model_request` (`scan`, `decide --run` and `intent next` with the proxy present and no flag); `privacy.rs::the_first_billable_call_waits_for_consent`, `a_planted_repo_secret_reaches_no_request_or_tool_output` (a fake key planted in a temporary fixture repo).
  - Agents: `AGENTS.md` says live calls need a flag and land in the spend log.
- **Sources (checked 2026-10-26):** KICKOFF `<constraints>` Claude and Spend, `<verified_facts>` (the proxy injects `*.vercel.sh` credentials); `examples/TRUTH.json` (the fixture's DSN); review round 1 security-6, unasked-3, cost-1 and owner decisions D7 and D8.

## DR-50: Decisions prefill before they interrupt

- **Status:** accepted (decided by the main agent in review round 1, D9 and D20).
- **Starting point:** extended "Jev policy. Each decision kind declares its question, thresholds and escalation."
- **Origin:** review round 1: principles-2, unasked-6, cost-4 and cost-7; cost-3 (downgraded).
- **Context:** the policy had two outcomes, act or card. Every not-yet-enabled kind dealt a card per subject, and a spent budget or a missing model dealt cards with no pick. A real repo's first scan could deal hundreds. `part_relation` asked about every pair of parts. Cards-only kinds paid Claude up front for a pre-selection.
- **Decision:**
  - Each kind declares `stakes` and a `default`. A `low` kind prefills: Jev's pick applies as an inferred fact with its confidence, listed in Decisions as one collapsed batch per kind. A `person` kind deals a card, batched per kind with a pick per item. A prefill never retracts: a `retract_edge` pick marks the edge doubted.
  - The `default` leaves the deterministic result as it is. With no consent, no budget or no model, it stands and the request stays pending for a later duty run within budget. No card ever has no pick, and an unbound role lists its item in Diffs.
  - The first scan deals at most 3 cards, batched by cause, plus at most one Jev setup card when no backend is configured.
  - Only a `low`-stakes auto candidate (one with `policy.auto_at` set) asks Claude up front. A `person` card asks Claude when it reaches the top of the stack or the person taps "Ask Claude".
  - The `markdown` built-in proposes part-relation candidates as `artifacts.mentions` edges (an explicit link, a heading named in another part, a shared defined term), at most 3 per part, and `part_relation` is a `low` kind over them.
  - Deterministic tiers emit confidence 1.0, so `dependency_real` asks only about genuinely inferred edges.
- **Alternatives rejected:**
  - Dealing a card when a budget runs out: a spend cap isn't a decision only a person can make.
  - A separate cold-scan budget: overflow already defers, so a second cap adds a setting and no behavior.
  - Every pair of parts with a cap on calls: still quadratic cards.
- **Consequences:**
  - Contracts: `DecisionKind.stakes`, `DecisionKind.default`, `Policy.auto_at: Option`, no `on_unavailable`; `Outcome::Prefilled`.
  - Tests: `kernel/tests/policy.rs::low_stakes_prefill_and_person_stakes_card`, `a_budget_past_the_cap_keeps_the_default`, `a_prefill_never_retracts`, `person_stakes_escalate_only_lazily`; `first_run.rs::first_scan_deals_at_most_three_cards` (the fixture replayed and with models off), `a_large_repo_first_scan_deals_at_most_three_cards`; `s05_anatomy.rs::part_relation_requests_are_at_most_three_per_part`; `s07_decisions.rs::a_kind_version_bump_raises_no_card_burst`.
  - Users: a first scan shows a map with defaults and a few cards, not a pile of "Pick one".
- **Sources (checked 2026-10-26):** KICKOFF principle 1 and R9; [jev §4, §5, §9]; `examples/TRUTH.json` prompt_snapshot (13 parts); review round 1 findings above and owner decisions D9 and D20.

## DR-51: One spend record, a user-level ledger and a measured always-on surface

- **Status:** accepted (decided by the main agent in review round 1, D20).
- **Starting point:** none; serves the constraint "Spend" and principle 2's size budgets.
- **Origin:** review round 1: cost-2 and cost-5; the minor findings cost-8, cost-9 and cost-10.
- **Context:**
  - The only ledger was a table in each workspace's store, and every test run builds a fresh fixture, so the build-wide $3 and the daily cap reset on every run. `sample` and session usage had no price.
  - The always-on gate read `claude plugin details`, a static inventory that may miss tools registered at runtime and the composed prompt section. Typed tool unions would cost about 1,300 tokens on their own.
  - Budgets measured CLI envelopes, not the tool results an agent receives, and no live Claude spend was planned.
- **Decision:**
  - The ledger moves to `DATA_ROOT/ledger.db`, one per user, shared by every workspace; the daily cap reads it. Session tiers, `sample` calls and spawned agents' `turn.complete` usage are priced from a pinned table. The curator spawns only after a person's card or `/room curate`.
  - With `PLANROOM_SPEND_LOG` set (by `cargo xtask` and this repo's settings), the engine appends every live call to `docs/evidence/spend.jsonl` and refuses one that would pass `live_tests_total_usd` less $0.25. `cargo xtask live record` prints its estimate from the live spend plan in [Architecture](architecture.md#live-spend-plan) and refuses past what's left.
  - `tests/plugin/surface_tokens.sh` measures the exact bytes the model receives: the non-deferred tool specs, the composed section and the frontmatter, at `ceil(bytes / 4)` times a ratio `count_tokens` measured. It gates 1,500 tokens: tools 900, section 300, skills and agents 300. WS8 records a headless first-turn delta. Tool inputs are flat: a `command` enum, CLI arguments as strings, `base` and `idem`.
  - CI gates envelope tokens, tool-result tokens (`packages/mod/src/tools.test.ts`), replayed Claude usage, the S3 cache-hit floor and the fold. `budgets.json` `recorded` holds what only evidence runs measure: S02 chat usage and the token ratio.
- **Alternatives rejected:**
  - A ledger in each store plus a user-level copy: two records of one call.
  - `count_tokens` in CI: CI has no secrets.
  - Deferring `room_change` to fit the budget: S3's proposals would first need a tool search.
- **Consequences:**
  - Store: ten tables; `ledger.db` with its own migrations.
  - Contracts: the flat tool input; `budgets.json` `recorded`; the spend log.
  - Tests: `store/tests/ledger.rs::the_daily_cap_spans_workspaces`; `live_gate.rs::the_spend_log_stops_live_calls_at_the_cap`; `packages/mod/src/tools.test.ts::tool_results_within_budget`; `packages/mod/src/agents.test.ts::curator_spawns_only_after_the_card`, `agent_usage_goes_to_the_ledger`.
  - It amends DR-22, which read the number from `claude plugin details`.
- **Sources (checked 2026-10-26):** KICKOFF `<constraints>` Spend; [claude-gateway §2] (prices); [mod-api §6, §9]; [plugin-packaging §1]; review round 1 findings above and owner decision D20.

## DR-52: What agents can reach: confined reads, person-only imports, and guarded worktrees

- **Status:** accepted (decided by the main agent in review round 1, D15's import part, D16 and D18).
- **Starting point:** extended "Events carry the workspace, project, actor, cause and an idempotency key."
- **Origin:** review round 1: security-2, security-3, security-4 and principles-1; the minor findings security-8 and security-9.
- **Context:**
  - `import` was in `room_change`, took its actor from a local directory path, and let the owner's uid run person-only commands. Any agent could write a row file and merge as a person.
  - `planroom open` was outside the catalogue and printed a login code into model-visible text. The implementer's cwd sat inside `.planroom/`, and the guard matched path strings, so `../../store.db` slipped past.
  - `ask breakdown PATH` on the auto-allowed `room_query` read any file the user could read, with no prompt.
- **Decision:**
  - `import` leaves every tool. Inbox rows and event exports need a person; `/room sync` runs the import with the person key after the model's `ArtifactData list`. Other viewers' rows apply as their attributed edits. The owner's own rows and any person-only command become one card the local person confirms. A model response imports only for a request the engine derives as pending, and runs an agent imports are kept with `trusted = false` and never mined.
  - `open` is a person-only catalogue command. It launches the browser itself and never prints the code.
  - Hand-off specs and worktrees live in `DATA_ROOT/handoffs/<workspace_id>/KEY/`. The `tool.check` guard resolves paths before it matches them. It keeps every agent out of the stores, `.planroom/` and the data root, and denies Bash naming a person-only command, even inside a pty wrapper. It keeps a spawned implementer's Edit and Write inside its worktrees and denies its `sqlite3`, `script`, `git push` and `planroom handoff`.
  - Read commands read only the workspace's repos and `file:` snapshots. An outside file goes through `scan PATH`, behind the permission prompt; a credential path needs a person; `--workspace` on a read needs an existing store.
  - A wake from the Artifact means only "sync requested", the hint goes in per-turn context, and only the owner reads `inbox`. The first-publish line comes from the snapshot's sections, and presence is left out unless asked for.
  - Curation names its source runs and shows in full any draft with a URL, command substitution, a shell pipe or `allowed-tools`.
- **Alternatives rejected:**
  - A per-sync nonce in the ack: the model writes the inbox files, so it would see the nonce too.
  - Hiding person-only rows from import entirely: a phone pick would be lost.
  - Matching only path strings: one `..` defeats it.
- **Consequences:**
  - Contracts: `ActorRule::Mixed`; `Open`; error `path_refused`; `inbox` read by the owner only; `Snapshot.who` optional.
  - Tests: `actor.rs::agent_cannot_import_inbox_rows`, `agent_cannot_import_an_event_export`, `model_responses_only_for_pending_requests`, `open_is_person_only_and_prints_no_code`, `a_read_outside_the_workspace_is_path_refused`, `a_credential_path_needs_a_person`, `workspace_flag_on_a_read_needs_an_existing_store`; `guard.test.ts::implementer_cannot_reach_the_store_by_a_relative_path`, `implementer_writes_only_in_its_worktrees`, `a_pty_wrapped_person_command_is_denied`; `s11_remote.rs::owner_rows_and_person_only_rows_become_one_card`; `s12_curation.rs::agent_imported_runs_are_not_mined`, `a_draft_with_a_url_or_shell_is_shown_in_full`.
  - Scenarios: S5 and S6 scan the snapshot copy before they ask about it; S5 and S12 import as a person.
  - Assumptions: A-12 (`tool.check` sees a spawned agent's `agentId`) and A-13 (the owner can't write another viewer's `inbox/{self}`).
- **Sources (checked 2026-10-26):** KICKOFF R5 and R8; [artifact §2], [artifact-spike] (session writes need no prompt); [mod-api §5, §8]; review round 1 findings above and owner decisions D15, D16 and D18.

## DR-53: Automatic event backups and safe migrations

- **Status:** accepted (decided by the main agent in review round 1, D6).
- **Starting point:** extended "A deterministic `.planroom/` export per repo", which DR-34 replaced, with an automatic backup outside every working tree.
- **Origin:** review round 1, principles-4, with the same gap in operability-4, unasked-1 and unasked-2.
- **Context:** every person-authored event lived only in `.planroom/store.db`, which for a single-repo workspace sits inside the repo's working tree behind a `.gitignore` of `*`. `git clean -xfd` deleted every plan, override and decision, and `planroom export` was manual. A migration had no copy to fall back on, and an older engine meeting a newer store crash-looped through the resolver.
- **Decision:**
  - After each person-authored commit, debounced to one write per 5 s, the engine appends the new events to `DATA_ROOT/backups/<workspace_id>/events.ndjson`. When `.planroom/` is missing but a backup exists for the root, `scan` deals one card, "Restore 214 edits from 2026-10-26?", with Yes pre-selected. A restore replays into a store with no authored events.
  - Before any migration, the engine writes `VACUUM INTO DATA_ROOT/backups/<workspace_id>/store-vN.db`, checks it with `PRAGMA integrity_check`, keeps the last 3, and runs the migration and a post-migration fold in one transaction, rolling back on failure. A migration reports `store_migrated` with the copy's path, which the mod shows once.
  - `planroom doctor` warns when the newest backup is older than the last person-authored event.
  - An older engine meeting a newer store exits with `schema_mismatch`, and the mod shows "Restart this session to use Planroom vX" once.
  - `planroom doctor --restore BACKUP` (person only) is the downgrade path. It appends the current events to the backup first.
- **Alternatives rejected:**
  - The store outside the tree by default: the WAL check already moves it when it must, and an in-tree store keeps the workspace self-contained.
  - A backup inside `.planroom/`: deleted with it.
- **Consequences:**
  - Contracts: the backup uses `planroom.event/v1`; warning `store_migrated`.
  - Tests: `crates/planroom/tests/backup.rs::git_clean_then_scan_offers_one_restore_card`, `a_failed_migration_rolls_back`, `an_older_engine_says_restart_once`, `doctor_restore_is_person_only`, `doctor_warns_when_the_backup_is_behind`.
  - It amends DR-1 (migrations) and DR-34 (backups).
- **Sources (checked 2026-10-26):** KICKOFF principle 1 ("Never lose or overwrite what someone did"); review round 1 principles-4, operability-4, unasked-1 and unasked-2, and owner decision D6.

## DR-54: Surfaces act through catalogue verbs, reversibly, in real keystrokes

- **Status:** accepted (decided by the main agent in review round 1, D15).
- **Starting point:** none; implements R13, S9 and principle 1 ("Everything is reversible, which is what makes bold defaults safe").
- **Origin:** review round 1: ux-2, ux-3 and ux-4; the minor findings ux-8 and ux-10.
- **Context:**
  - A bare digit at the empty prompt presses the band's primary Button [mod-spike], and `PaneModel.next` could name any command. After `handoff collect`, a message that starts with "1." would merge.
  - The pane's `y` and `n`, accepting one ghost of a batch, and the canvas's Confirm and Reject had no catalogue leaf to call.
  - Pane actions acted on the first row, so the Nth row took N more keys, and the S9 test counted key-map entries, not real presses. `ctrl+x tab` focuses the band, not the pane [mod-spike].
  - The pane's rows didn't add up at 49×34. The canvas gave the next action, `Anatomy` and `Machine` no home among the five concepts.
- **Decision:**
  - `review ID accept|reject` is a catalogue leaf, mixed: an agent may reject only its own ghosts. The pane's `y` and `n`, the canvas's evidence popovers and a batch card's items run it. A batch card's accept stays `decide CARD accept`. `open` is the `e` key [DR-52].
  - `PaneModel.next`, every band Button and every pack suggestion name only a leaf whose undo class isn't `NotUndoable` and whose class isn't `handoff`. A hand-off start or merge shows on the band as a label that opens its card, where confirming takes a second key. The pack loader refuses a suggestion that breaks the rule.
  - Every row an action can target carries a row key from a pool of 12: `5` to `9`, `0`, `b`, `j`, `k`, `q`, `r` and `v`. A row key selects; an action key acts on the selected row. Lists page at 9 rows and rank the row the next action needs first.
  - S9 counts real key presses over scripted main flows (S1 to S4, a hand-off and a sync) in `claude plugin test`. `/room` is the entry point to the pane.
  - At 34 body rows the diff hunk shows a header and 2 lines, the mini-map shows only at 46 rows or more (else `m` swaps it with the list), and the list keeps at least 10 rows.
  - On the canvas, `PaneModel.next` is the face-up Decisions card when no decision is open. `Anatomy` and `Machine` are what the `anatomy` and `machines` lenses show on a focused block, and the breadcrumb's search takes a path.
- **Alternatives rejected:**
  - Moving `c`, `t`, `w` and `f` to `/room` to free letters: the six free letters and six free digits already make a pool of 12.
  - A confirmation on every band press: it costs a key on the common, reversible case.
- **Consequences:**
  - Contracts: `Command::Review`; the leaf table; the rule on `PaneModel.next` and `SuggestionDecl`.
  - Tests: `packages/mod/src/keys.test.tsx::main_flows_within_two_keys` (real presses), `band_next_is_undoable`, `key_map_snapshot`; `packages/mod/src/surfaces.test.tsx::rows_fit_at_49x34`; `crates/planroom/tests/packs.rs::a_suggestion_cannot_name_an_irreversible_command`; `kernel/tests/undo.rs` covers `review`; `tests/e2e/surface.spec.ts::exactly_five_concepts` counts the next-action card under Decisions.
  - Users: a stray digit can't merge, and the third diff is accepted with `7 y`.
- **Sources (checked 2026-10-26):** [mod-spike] (bare digits, `ctrl+x tab`, the 49×34 dock, diff hunk headers); [mod-api §2] (at most 36 one-key actions per pane); KICKOFF S9, S15 and principle 1; review round 1 findings above and owner decision D15.

## DR-55: Hand-off revert and an enumerated irreversible list

- **Status:** accepted (decided by the main agent in review round 1, D15).
- **Starting point:** none; implements principle 1 ("Everything is reversible") for R8.
- **Origin:** review round 1, principles-3; the hand-off part of the minor finding unasked-9.
- **Context:** `handoff merge` changes the user's default branches, yet it had no reverse in Planroom, and the loop ended at `reconciled`. Cancel and drop didn't say what happened to the implementer's branch. `Undo::NotUndoable(&str)` let any command opt out of reversibility, with no list and no test.
- **Decision:**
  - `handoff revert KEY` (person only) preflights every repo: a clean tree, and each recorded merge commit in `HEAD` and reverting cleanly. It then commits a `reverting` step, runs `git revert -m 1` on each merge commit with hooks off, and commits `reverted`. A retry resumes from the step.
  - The loop gains `merged -> reverting`, `reconciled -> reverting` and `reverting -> reverted`. The `merging` step records each repo's head before the merge and its merge commit.
  - The merge's reply and card say "Undo: /room handoff revert KEY".
  - `handoff cancel` and a drop remove the worktrees and move each `planroom/KEY` branch to `refs/planroom/archive/KEY`, so no commit is lost.
  - Every leaf has an undo class: `Edit`, `Compensate` (with the reversing command), `Query` or `NotUndoable`. `NotUndoable` is only `share`, because a viewer may already have seen what was published. `catalogue.rs::undo_classes_snapshot` pins every class, so a new irreversible leaf needs a record.
- **Alternatives rejected:**
  - Leaving the revert to `git revert -m 1` by hand in each repo.
  - Deleting an unmerged branch on cancel: it loses the implementer's work.
  - `NotUndoable` for `serve`, `watch`, `export` and `open`: they change nothing a person authored, so they are `Query`.
- **Consequences:**
  - Contracts: `Command::HandoffRevert`; `HandoffState::{Reverting, Reverted}`; `HandoffStep.repos`; the undo classes.
  - Tests: `crates/planroom/tests/handoff_preflight.rs::a_revert_undoes_the_merge_in_every_repo`, `a_revert_preflight_failure_leaves_every_repo_merged`, `a_crash_mid_revert_resumes_on_retry`, `cancel_archives_the_branch`; `actor.rs::agent_revert_is_person_required`; `catalogue.rs::undo_classes_snapshot`; S6's own-loop check covers the new states.
  - Docs: `docs/concepts/hand-off.md` draws the new states, and the "Remove Planroom" guide lists the archive refs.
  - It amends DR-25.
- **Sources (checked 2026-10-26):** KICKOFF principle 1 and R8; review round 1 principles-3 and unasked-9; owner decision D15.

## DR-56: The Artifact declares what it serves and shows synced edits

- **Status:** accepted (decided by the main agent in review round 1, D15).
- **Starting point:** kept and refined "Remote mode: ... the Artifact shows cached decisions and queues new ones."
- **Origin:** review round 1, ux-5; the minor finding ux-7, and the remainder of ux-1.
- **Context:**
  - The canvas claimed views couldn't tell its transports apart, but the snapshot holds only the default-lens maps, cards, diff summaries, plans, machines and the pane. `PathView`, `Anatomy`, evidence text, hunks, the heat ramp, other lenses and `intent next` had no data on a phone.
  - `live/head` carried only summary frames, so an imported edit stayed invisible until a republish, which happened only above 200 KB.
  - The manifest lacked the `comments` capability that Send to Claude needs, and a queued tap had no visible state.
- **Decision:**
  - `Transport` declares `capabilities`. The snapshot names what it serves and carries an engine-authored empty state, "Needs your computer: open this workspace there with /room open", which the page shows for any other view.
  - `live/head` carries the data delta since the snapshot as a `Diff`, and the page applies it. A delta that would take `live/head` past 256 KiB republishes the snapshot instead.
  - The page draws its own rows at once and marks them, and the cards it tapped, as queued until `acks/<uid>` says imported, conflict or refused.
  - The manifest declares `comments`. Send to Claude works for viewers with edit access, the owner among them; other viewers' rows wait for the owner's next sync.
  - An inbox row may name only `edit`, `review`, `decide` on a card in the published snapshot, or `intent propose`. Import refuses any other.
  - The empty-state audit runs through both transports.
- **Alternatives rejected:**
  - Queuing a phone question as an inbox `ask` that the next sync resolves into a results document: a second request path, and the empty state already names what to do.
  - Republishing the snapshot on every sync: a new version and a reload for every viewer each time.
- **Consequences:**
  - Contracts: `Snapshot.capabilities`, `Snapshot.unavailable`, `LiveHead.delta`, the rows in `acks/<uid>`, the `comments` capability, the inbox command list.
  - Tests: `tests/e2e/artifact.spec.ts::views_it_cannot_serve_show_an_empty_state`, `a_synced_edit_shows_on_the_map`, `a_tapped_card_shows_queued_until_acked`; `tests/e2e/surface.spec.ts::empty_state_audit` on both transports; `s11_remote.rs::an_unlisted_inbox_command_is_refused`. WS8 checks that `comments.sendToClaude` wakes the session's watch.
  - It amends DR-31.
- **Sources (checked 2026-10-26):** [artifact §2] (`comments.sendToClaude` is for writers only; `db` writes don't wake the session); [artifact-spike]; KICKOFF R13 and principle 1; review round 1 ux-5, ux-7 and ux-1; owner decision D15.

## DR-57: The pane before the engine and outside a workspace

- **Status:** accepted (decided by the main agent in review round 1, D15).
- **Starting point:** none; implements principle 1 ("Never show a blank state") for S14's first run.
- **Origin:** review round 1, principles-5; ux-6 (downgraded); the minor findings unasked-7 and operability-8.
- **Context:** every pane row came from `focus`, which needs an engine. So the pane had nothing to draw while the engine downloaded or built, and the rule that the engine authors every string left the mod no text for it. `focus` in a directory with no repos was an error. `watch` started in every session, so a session in a parent of 70 repos would create a store or poll for nothing. Nothing said what works with mods off.
- **Decision:**
  - The mod draws a fallback `PaneModel` for each state before an engine responds: `fetching`, `build_offered`, `building`, `failed`, `unsupported_platform`, `version_mismatch` and `store_busy`. `cargo xtask gen` writes them from `en.toml` into `catalogue.ts`, so the engine still authors every string. Each band action runs the resolver or names `/reload-plugins`.
  - A directory becomes a workspace only on `scan`. Before that, nothing creates `.planroom/`, `watch` doesn't start, and `focus` returns `ok` with an empty state, "Map these 5 repos? About 2 s", whose next action is `scan DIR`.
  - With mods off, the skills still work through Bash and the resolver. `plugin/README.md` and the `room` skill say in one line that the pane and `/room` need mods, and how to turn them on.
  - The audit mounts every fallback pane, the not-yet-a-workspace state and every error frame on all four surfaces.
- **Alternatives rejected:**
  - Strings written in the mod for these states: two string sources.
  - Scanning the session's directory on start: work and spend nobody asked for.
  - A repo picker for a directory with many repos: new surface, when `scan PATH` maps any one repo.
- **Consequences:**
  - Contracts: the `engine` atom's statuses; `FALLBACK_PANES`; `focus`'s empty state, while other reads keep `workspace_not_found`.
  - Tests: `packages/mod/src/surfaces.test.tsx::no_blank_pane`; `packages/mod/src/engine.test.ts::no_watch_until_a_store_exists`; `crates/planroom/tests/first_run.rs::focus_outside_a_workspace_creates_nothing`; `tests/plugin/install.sh` with mods off, where the skill path resolves the engine and `focus --json` returns a non-empty envelope.
  - Users: the first screen always names one next step.
- **Sources (checked 2026-10-26):** KICKOFF principle 1, S14 and S15; [mod-api §7]; review round 1 principles-5, ux-6, unasked-7 and operability-8; owner decision D15.

## DR-58: One data root, a self-locating resolver, an embedded canvas and visible background work

- **Status:** accepted (decided by the main agent in review round 1, D16), verify A-14 in WS5.
- **Starting point:** kept and refined "plugin/ ... engine bootstrap"; extends the constraint "the local server serves the canvas's built assets".
- **Origin:** review round 1: operability-1, operability-2 and operability-3; the minor findings operability-5, operability-6, operability-9, unasked-8 and unasked-11.
- **Context:**
  - The resolver read `$CLAUDE_PLUGIN_ROOT` and `$CLAUDE_PLUGIN_DATA`, which the Bash tool doesn't set [plugin-packaging §3]. The skills' path missed the dev checkout and the cache, and the mod cached the engine where skills never looked.
  - `serve` read the canvas from `<exe dir>/canvas/`, which only the release archive creates. `target/release/` and `cargo install` have none.
  - The source fallback was pinned to a tag the agent may not push and the owner may not have pushed yet. Its card named no command, process, preflight or lock recovery.
  - A newer mod could reuse an older `serve`, and the dev checkout matched by version alone. `release.yml` was one line, and the owner's clicks weren't listed. Windows and a missing toolchain ended in a card that fails. A detached process's failures went nowhere.
- **Decision:**
  - One data root, `${PLANROOM_DATA:-${XDG_DATA_HOME:-$HOME/.local/share}/planroom}`, for the engine, the resolver, the mod, the Bash tool and agents. The engine cache is `DATA_ROOT/engine/VERSION/`, pruned on each successful resolve.
  - The resolver finds `engine.env` and the dev checkout from its own path. The mod learns its root with `$.env.get` [A-14] and gives spawned agents the resolver's path in their prompt.
  - `crates/planroom/build.rs` embeds `target/canvas/` in the binary. Without it, `serve` shows one engine-rendered fix page.
  - The source build runs from the checkout when there is one, else `cargo install --tag`, which says so when the tag doesn't exist yet and offers `--branch main`. A preflight checks cargo, a C compiler and the network first, and names what's missing. Prefetch and build run detached under `setsid` with a log and a pid file. The fetch lock holds a pid and a time and breaks when stale.
  - `version --json` reports the catalogue hash. The mod shows one line on a mismatch, `serve --detach` drains an older or other-catalogue `serve`, and the dev checkout picks the newer of `release` and `debug` whose hash matches.
  - On native Windows the mod runs nothing and shows one line pointing to WSL 2.
  - `release.yml` follows Iris: `verify` (the tag against every manifest's version, and the changelog section), `checks`, `build`, and `publish` with an exact asset set. `x86_64-apple-darwin` builds on `macos-15-intel`, and the musl targets record their C settings.
  - A departure from Iris: the owner tags the release branch and fast-forwards `main` after publishing, so `main` never names an unreleased version. The owner checklist lists every click, and the final report reads it.
  - `watch`, `serve` and duties log to `DATA_ROOT/logs/`, capped and redacted. A failed duty is a `duty_failed` warning and one status-line entry, and `doctor --json` holds what a bug report needs.
- **Alternatives rejected:**
  - `$CLAUDE_PLUGIN_DATA` as the cache: unset in the Bash tool, and kept per plugin rather than per user.
  - Searching `<exe dir>/canvas` and `<exe dir>/../canvas`, plus a canvas archive: three places to get wrong.
  - Iris's order of merging before tagging: `main` names an unpublished version until the tag lands.
  - A `doctor --report` flag: `doctor --json` already is the report.
- **Consequences:**
  - Layout: `plugin/scripts/planroom` gains `prefetch`, `build` and `status`; `scripts/check-tag-version.sh`; `crates/planroom/build.rs` embeds the canvas.
  - Contracts: `VersionInfo`; `serve.json` `catalogue_hash`; warning `duty_failed`; the `engine` atom's statuses.
  - Tests: `tests/installer/run.sh` adds a run under `env -i HOME=... PATH=...` with no `CLAUDE_*` variable, a killed prefetch's stale lock, a missing tag and a missing toolchain; `routes.rs::serves_the_embedded_canvas`, `serves_the_fix_page_without_a_canvas`; `server_auth.rs::serve_drains_for_another_catalogue`; `packages/mod/src/engine.test.ts::a_catalogue_mismatch_shows_one_line`, `windows_shows_one_line`; `duties.rs::a_failed_duty_warns_until_it_succeeds`; `docs.rs::releasing_has_the_owner_checklist`. The release smoke test runs `serve` from `target/release/` and from a `cargo install --path` root and fetches `/`.
  - Assumptions: A-14.
  - It amends DR-1 and DR-28.
- **Sources (checked 2026-10-26):** [plugin-packaging §3, Design implications]; [mod-api §7]; [iris-release-bar §1, §3]; KICKOFF constraints on packaging, the release bar and pushing tags; review round 1 findings above and owner decision D16.

## DR-59: Repo gates that fire

- **Status:** accepted (decided by the main agent in review round 1, D14).
- **Starting point:** extended "Repo layout: a crate-graph test or `cargo-deny` bans in Rust, and `dependency-cruiser` in TypeScript" and the one task entry point.
- **Origin:** review round 1: repo-1, repo-2, repo-3, repo-4, repo-6 and repo-7; the minor findings operability-7, operability-10, operability-12, repo-8, repo-9, repo-10 and repo-11.
- **Context:**
  - The decision-record gate diffed against the merge base, which on this branch is the Initial commit, so it could never fire. The schema guard compared with `origin/main`'s moving tip, and CI's checkout was shallow.
  - The boundary self-test ran in a worktree with no `node_modules` and no clean control, and never planted a declared workspace dependency. So the TypeScript rule could break unseen.
  - Two CI jobs ran tools outside `cargo xtask`, so a green local `check` didn't mean a green CI.
  - `cargo xtask gen --budgets` could rebaseline S7's tier A gate with no record.
  - `claude plugin test packages/mod` exits 1 on a package with no `modules` entry, and plugin tests can't read JSON files.
  - S15 listed a test that needs a live session.
  - The kernel's "no I/O" and the adapters' "must not" rules were conventions: `std::fs` or `reqwest` would pass every check.
  - Conventions and area contracts were written twice, and the gate watched one copy. Area files weren't loaded into an agent's context. pnpm wasn't pinned exactly, and nothing installed JS dependencies. Dependabot ignored pnpm's 7-day age. The Playwright browser and Claude Code had no stated install.
- **Decision:**
  - `layout.lock` holds each gated surface's sha256 and the record that approved it. `check layout` fails on a changed hash, and `gen --layout-lock DR-n` refuses unless that record exists. In CI, with `fetch-depth: 0`, each commit in `BASE..HEAD` that changes the lock must add the record it names. A missing base fails loudly.
  - The breaking-change guard compares with a committed `schema/baseline/` and with the last tag's schema.
  - The self-test installs offline, runs every check on the clean copy first, and plants a declared `workspace:*` crossing and a relative import. It expects exactly the rule's name and no unresolvable import.
  - The kernel's "no I/O" is a lint, not a convention: `crates/kernel/clippy.toml`, generated from `boundaries.toml`, disallows `std::fs`, `std::net`, `std::process`, `std::env` and `SystemTime::now`, and `store` and `extract` may not depend on a network client. The self-test plants a `std::fs` call.
  - Every CI job runs `cargo xtask <command>`. `cargo deny` moves into `check`, the network audit into `check audit`, and the installer suite into `test --installer`. `ci_runs_only_xtask` fails on any other `run:` step except a listed setup step.
  - `budgets.json` is hand-ratcheted. `gen --budgets` writes a measured file, and a commit that raises a gated number adds a record.
  - `cargo xtask gen --mod` also builds `target/mod-test/`, with the golden outputs inlined, for `claude plugin test`.
  - Tests that need a live session live on evidence paths, and `feature_list_is_complete` enforces it.
  - `AGENTS.md` alone holds the conventions, and `repo-layout.md` links to it. Each area's contract block is generated from `repo-layout.md`, with a drift check. A one-line `CLAUDE.md` importing `@AGENTS.md` sits beside each area's `AGENTS.md`.
  - `packageManager` pins pnpm exactly. Every xtask command first runs `pnpm install --frozen-lockfile --offline`, falling back to online, with one fix line on failure. Dependabot waits 7 days. `cargo xtask e2e` installs Playwright's Chromium, and CI installs Claude Code at the pinned version with auto-update off.
- **Alternatives rejected:**
  - A merge-base diff: inert on this branch.
  - Generating the conventions from `repo-layout.md` into `AGENTS.md`: KICKOFF writes them once, in `AGENTS.md`.
  - The generic drift check over `budgets.json`: it would fail on a one-token improvement.
- **Consequences:**
  - Layout: `layout.lock` and `schema/baseline/` at the root; `target/mod-test/`; an area `CLAUDE.md` beside each area `AGENTS.md`.
  - Tests: `cargo xtask check boundaries --self-test`, with its control and a layout-gate plant; `cargo xtask check layout`; `ci_runs_only_xtask`; `feature_list_is_complete`; `area_contracts_match_repo_layout`; `release_pins.rs::dependabot_cooldown_matches_minimum_release_age`.
  - Agents: one command reproduces any CI failure.
  - It amends DR-6, DR-30, DR-36, DR-38 and DR-40.
- **Sources (checked 2026-10-26):** KICKOFF R16, S7, S15 and the repo-tooling constraint; [plugin-packaging §1] (`claude plugin test` needs a `modules` entry); [mod-api §1, §9]; [ts-stack §6.1]; [iris-release-bar §5, §7]; review round 1 findings above and owner decision D14.

## DR-60: Owner follow-ups after review round 1

- **Status:** accepted (main agent, 2026-10-26).
- **Starting point:** Store: events carry the actor; Packs: declarative files.
- **Context:** the round 1 fix agents raised four gaps: a CLI process as a writer instance makes an agent conflict with its own earlier write; under DR-4 a `scan` from CI with no actor would be `actor_required`; nothing says how the person key's hash is first registered, so an agent could register its own key first; and DR-48's first-load card for out-of-tree packs would appear on every fixture scan, interfering with S1's one next action, S7's at most one card and S15's empty-state audit.
- **Decision:**
  - An agent's client ID is its own ID from `PLANROOM_ACTOR` (`agent:<id>:<role>`), so one agent is one writer instance across processes. Only a person at a terminal is `cli:<pid>`.
  - Writes that only record observations, `scan` and the duties a live process runs, are `system:<part>` writes and need no actor. Every write that authors something still needs one.
  - The mod generates the person key on its first `session.start` in a workspace and registers its hash with `planroom person init`, which succeeds only when no hash is registered. Re-registering needs the old key or a `/dev/tty` confirmation. The `tool.check` guard denies agent calls to `person init`, and `planroom doctor` shows when the key was registered and from which surface. This stays a guardrail, not a boundary between processes of one OS user.
  - A pack that a person lists in config or `PLANROOM_PACKS` loads with no card: listing it was the person's choice. `planroom doctor` lists every loaded pack with its kinds and the sha256 of its prompts. The safety that matters stays: an out-of-tree pack's `auto_enabled` counts only after a person enables auto-action per kind.
- **Alternatives rejected:** a card on every first load of a pack (it interrupts for a choice the person already made, against principle 1); exempting all CLI writes from the actor rule (it would let unnamed agents author).
- **Consequences:** contracts gain `Command::PersonInit` (person-only by construction, mod-only through the guard); `actor.rs` gains `scan_needs_no_actor` and `an_agent_does_not_conflict_with_itself`; `trust.rs` gains `an_out_of_tree_pack_loads_without_a_card`; `guard.test.ts` gains `agents_cannot_init_the_person_key`.
- **Sources:** `docs/contributing/research/arch-review/round-1/fixes.md` (the fix agents' notes), checked 2026-10-26.

