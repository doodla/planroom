# Decisions

This page holds Planroom's numbered decision records. Each one gives the context, the decision, the alternatives it rejected, its consequences for the layout, contracts, tests, release, agents and users, and its sources. Each record names the item of KICKOFF `<design_starting_points>` it kept, replaced or extended. Changing the layout, a boundary or a convention needs a new record here before it lands (R16), and `cargo xtask check layout` enforces that.

## How to read a record

- **Status** is `accepted`, `accepted, verify in WS` (it rests on an assumption the walking skeleton checks), `needs confirmation` (it binds only once it is confirmed), or `superseded by DR-n`. A record a later review amended says so in its Status, naming the rounds [DR-79].
- **An amendment overrides.** An "Amended in review round N" bullet changes the record: where it differs from the bullets above it, the amendment holds, and a later round's holds over an earlier one's. An "Amended by DR-n" bullet does the same for a later record's decision, and the Status names that record. An "Extended by DR-n" bullet adds to the record without changing what it decided, and the Status names that record too. The design pages state the current rule, and `docs.rs::an_amended_record_says_so_in_its_status` will check the Status (M1) [DR-79].
- **Starting point** names the `<design_starting_points>` item and whether this record keeps, replaces or extends it, or says "none" when the record serves a requirement or constraint the starting points don't cover.
- **Origin** says where the idea came from: one of the three candidate designs (the simplicity-first, extensibility-first and concurrency-first designs, in `research/arch/candidate-*.md`) or the design reviews that compared them; the Phase 1 research; architecture review round N; the walking skeleton or its review round N; milestone planning; or an owner decision.
- Sources are the Phase 1 notes in `docs/contributing/research/`, checked on 2026-10-25, plus `examples/TRUTH.json` and `examples/README.md`.

## Index

| DR | Title | Starting point | Effect | Origin |
|---|---|---|---|---|
| 1 | SQLite is the single writer; no engine daemon | Store: single-writer local server | replaced | simplicity-first; FULL sync and the version handshake from concurrency-first |
| 2 | Store facts and events; derive the rest on read | Kernel: everything derived from events; Store: SQLite for derived state | replaced | simplicity-first |
| 3 | Three edit operations and eight event kinds; a view is a value | Kernel: four primitives | extended (View dropped as a primitive) | simplicity-first |
| 4 | Actor identity and actor rules | Store: events carry the actor | extended | concurrency-first; fixes a fatal flaw in simplicity-first |
| 5 | Base required; field-level authored revisions | Store: optimistic concurrency on entity versions | replaced | simplicity-, concurrency- and extensibility-first combined |
| 6 | The mod reaches the engine through the CLI | none (research suggested `http.fetch` over a socket) | replaced the research suggestion | simplicity-first |
| 7 | A stale write is stored; only a person's raises a card | Store: optimistic concurrency | extended | concurrency-first over simplicity-first |
| 8 | Idempotency with request hashes, replays and a mod outbox | Store: events carry an idempotency key | extended | concurrency-first |
| 9 | Packs are data; built-in extractors and linkers are declared by packs | Packs: declarative files | kept, narrowed | simplicity-first over extensibility-first |
| 10 | A pack conformance kit | Packs | extended | extensibility-first |
| 11 | Scope cuts for v0.1 | Extractors tier 3; canvas kit dependency matrix; XState | replaced | simplicity-first |
| 12 | Hints, built-in linkers, endpoints as via nodes, and full evidence recall | Extractors in tiers; Kernel primitives | extended | concurrency- and extensibility-first; fixes a fatal flaw in extensibility-first |
| 13 | Ruby is an out-of-tree data pack the fixture workspace loads | Extractors tier 2 language list | kept (no Ruby built in) | the design reviews disagreed; out of tree chosen |
| 14 | One command catalogue; no MCP server | `rmcp` if there's an MCP server | replaced | simplicity-first |
| 15 | Lenses with flow, a dedicated path lens, and question pipelines | Store: typed query builder plus named queries | replaced | simplicity- and extensibility-first |
| 16 | The git CLI is the only git adapter | Libraries: `gix` or `git2` | replaced | simplicity-first |
| 17 | Kernel roles | Jev policy | extended | extensibility-first |
| 18 | Claude backends: the gateway, and the session's model through the mod | none (constraint "Claude") | added | extensibility- and concurrency-first; fixes a fatal flaw in simplicity-first |
| 19 | Eval protocol for thresholds | Jev policy | extended | simplicity-, extensibility- and concurrency-first |
| 20 | One card at a time, batched by cause | Jev policy; Remote mode (queued decisions) | extended | simplicity- and extensibility-first |
| 21 | Agent-first envelope and engine-authored strings | none (principle 2) | added | simplicity-, extensibility- and concurrency-first |
| 22 | A tiny surface with measured always-on cost | none (R14) | added | simplicity- and concurrency-first |
| 23 | Plan stages with their methods and structural lint | Jev policy: structural lint in code | extended | concurrency-first |
| 24 | Duties under leases instead of a jobs API | none | added | simplicity-first; duties off the critical path from concurrency-first |
| 25 | Engine-made clones, two implementers, a cross-repo merge preflight | none (research: an agent type with worktree isolation) | replaced the research suggestion | simplicity- and extensibility-first |
| 26 | The hand-off guard asks the person | none | added | the design reviews disagreed; extensibility- and concurrency-first combined |
| 27 | Curation drafts: templates, then Claude, then the curator agent | none | added | simplicity- and concurrency-first |
| 28 | Engine bootstrap without a SessionStart hook | Repo layout: engine bootstrap | kept, refined | simplicity- and extensibility-first |
| 29 | Browser login with a one-time code | Store: local server | extended | concurrency-first |
| 30 | Skills hidden from the slash menu | none (R14) | added | fixes a fatal flaw in all three designs |
| 31 | Remote mode: a published snapshot, a small live head, an attributed inbox | Remote mode | kept, refined | simplicity- and concurrency-first |
| 32 | The S8 concurrency harness | none | added | concurrency-first |
| 33 | Freshness on read instead of a file watcher | Libraries: `notify` | replaced | simplicity-first |
| 34 | No per-repo export directory | Store: deterministic `.planroom/` export | replaced | simplicity-first |
| 35 | Crates and packages | Repo layout sketch | replaced | simplicity- and concurrency-first |
| 36 | One boundaries file for both languages | Repo layout: crate-graph test or `cargo-deny`; `dependency-cruiser` | kept, combined | concurrency- and simplicity-first |
| 37 | One schema in two views, one version | Repo layout: `planroom-schema` as contract source; schema-to-TS generator | kept, refined | simplicity-first |
| 38 | The xtask runner is the one task entry point | Repo layout: `justfile` or `cargo xtask` | kept (xtask) | all three designs |
| 39 | Rust library choices | Libraries to evaluate first (Rust) | kept mostly | research |
| 40 | TypeScript library choices | Libraries to evaluate first (TypeScript) | kept mostly | research |
| 41 | Jev adapter and pins | Jev policy | kept | research |
| 42 | Levels, IDs and fact merging | Kernel: levels, IDs | kept, refined | simplicity-first |
| 43 | How S13's 1 s bar is measured in CI | none | added | confirmed: strict |
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
| 60 | Owner follow-ups after review round 1 | Store: events carry the actor; Packs: declarative files | extended | review round 1's fixes |
| 61 | Writes that keep the tree, undos and imports whole | Store: optimistic concurrency on entity versions; Kernel: everything derived from events | extended | review round 2 (E4) |
| 62 | The pack set is workspace state | Packs: declarative files | extended | review round 2 (E6) |
| 63 | Decision kinds express v0.1's kinds as data | Jev policy | extended | review round 2 (E7) |
| 64 | Agents never hold the person's authority | Store: events carry the actor | extended | review round 2 (E15) |
| 65 | Cost gates that can fail | none (constraint "Spend", S7) | added | review round 2 (E16) |
| 66 | An agent surface that reports errors, takes proposals inline and brings the diff back | none (principles 1 and 2, R8) | added | review round 2 |
| 67 | Paired owner devices, and inbox rows only from the mod | Remote mode | kept, refined | review round 2 (E8) |
| 68 | The canvas edits by hand and answers to the keyboard | none (R6, R13, R14) | added | review round 2 (E9) |
| 69 | Releases from main, a portable detach and a resolver contract | Repo layout: engine bootstrap | kept, refined | review round 2 (E11, E12, E13) |
| 70 | Repo gates over the pushed commits and every boundary statement | Repo layout: boundary checks; `justfile` or `cargo xtask` | extended | review round 2 (E14) |
| 71 | Everyday git moves: a tracked branch, the working tree and every repo layout | none (R1, R7, principle 1) | added | review round 2 (E17) |
| 72 | Owner follow-ups after review round 2: signed person tokens, Git LFS filters, S7's fast-forward | DR-64; DR-71 | amended | review round 2's fixes |
| 73 | A person's work outlasts automatic writes, flushes and restores | Store: optimistic concurrency on entity versions; Jev policy | extended | review round 3 (F1, F3, F4) |
| 74 | Repo namespaces that survive duplicates and renames, and a single repo's state in its git directory | Kernel: stable, namespaced IDs | kept, extended | review round 3 (F5, F6) |
| 75 | Authority stays with a person's own action | Store: events carry the actor; Remote mode | extended | review round 3 (F7, F11, F15) |
| 76 | The mod checks its host | none (S9, S14, principle 1) | added | review round 3 (F19) |
| 77 | Every pane action within two keys, and an Artifact that serves a cloud session | Remote mode | kept, refined | review round 3 (F9, F10) |
| 78 | Releases users install, detached work in its own session, a canvas port that never moves, and machine-wide files across versions | Repo layout: engine bootstrap; Store: local server | kept, refined | review round 3 (F8, F16, F17) |
| 79 | Gates that fire only where they mean something | Repo layout: boundary checks | extended | review round 3 (F18) |
| 80 | Hand-off results land however the change reaches the tracked branch | none (R7, R8) | added | review round 3 (F20) |
| 81 | What the walking skeleton's engine and plugin work settled | Store: events carry an idempotency key; Repo layout: boundary checks; Remote mode | extended | walking skeleton (engine and plugin work) |
| 82 | What the walking skeleton's serve, canvas and mod work settled | Actors: person actions are signed; Surfaces read one contract; Repo layout: package dependencies | extended | walking skeleton (serve, canvas and mod work) |
| 83 | What the walking skeleton's gates, remote slice and platform checks settled | Repo layout: boundary checks; Remote mode; the release model | extended | walking skeleton (gates and remote-slice work, CI, WS7, WS8) |
| 84 | The umbrella skill is `planroom`, so the mod can own `/room` | Plugin: skills, agents and the mod ship as one plugin | extended | walking skeleton (live mod run) |
| 85 | What the walking skeleton's review settled | Extractors read the repo; One contract source; Surfaces read one contract; the release model | extended | walking skeleton review round 1 (scan, contract, engine and surfaces fixes) |
| 86 | What the walking skeleton's review settled in round 2 | Extractors read the repo; Actors: agents never act as a person; Surfaces read one contract | extended | walking skeleton review round 2 (scan, budget, guard and canvas fixes) |
| 87 | What the walking skeleton's review settled in round 3 | Extractors read the repo; Actors: agents never act as a person; Surfaces read one contract; Remote mode | extended | walking skeleton review round 3 (engine and guard fixes) |
| 88 | The guard's residual risks, decided | Actors: agents never act as a person | extended | walking skeleton review round 3's residual risks |
| 89 | What M1 builds, compares and leaves to later milestones | none (Milestones; S1, S3, S15 and the feature list's gate) | added | milestone planning (M1's scope, read against the tree) |
| 90 | The guard lets everyday work through, and closes three more doors | Actors: agents never act as a person | extended | a usability review of the guard after DR-88 |
| 91 | The contract M1 builds on: cards on the wire, the edit ops, and the redaction marks | none (Contracts; the Logging convention) | added | M1's first change, read against the docs |
| 92 | The truth comparator's floor, edit's first ops, the dirty count's git call, and test-only seams | DR-71, DR-73, DR-75, DR-86, DR-89, DR-91 | extended | the merges of M1's truth comparator, discovery, person authority and edit work |
| 93 | The model adapter's crate edges, its transport's crypto provider, and an environment variable's own hint | DR-39, DR-48, DR-66, DR-85 | extended | the merge of M1's Claude adapter |
| 94 | Declared IDs at most 64 characters, and the listing's own left-out count | DR-15, DR-46, DR-86 | extended | the merge of M1's questions and `ask` |
| 95 | The check gates' edges: what a budgets raise is, how the baseline and the release are found, and what the secret scan exempts | DR-59, DR-79, DR-83, DR-85, DR-89 | extended, amends DR-89 | the merge of M1's check gates |
| 96 | Where a store lives on an unsafe filesystem, its pointer's type, migration copies, and the engine's libc calls | DR-1, DR-53, DR-69, DR-78, DR-85 | extended | the merge of M1's store open and migrations |
| 97 | Namespaces fixed at the first mapping scan, reserved names, and an unborn repo that writes no event | DR-47, DR-74, DR-87, DR-89 | extended, amends DR-74 and DR-87 | the merge of M1's namespaces, renames and fold benchmark |
| 98 | Review, one open Apply card, a viewer's row that keeps its viewer, and schema types by handle | DR-5, DR-21, DR-67, DR-75, DR-83, DR-86, DR-89 | extended, amends DR-75 | the merge of M1's review leaf, card store and inbox edits |
| 99 | One hook for a model request's tail, every invisible character marked, and evidence paths left whole | DR-49, DR-64, DR-75, DR-91 | extended | the merge of M1's redaction on every path and the sanitizing pass |
| 100 | The declarative runner's bounds: per file, one repo cut, and templates capped at load | DR-9, DR-46, DR-47, DR-86, DR-89 | extended, amends DR-47 and DR-86 | the merge of M1's declarative runner |

## Where the design reviews disagreed

| Disagreement | Positions | Resolution | Record |
|---|---|---|---|
| Which candidate design is best overall | Two reviews ranked simplicity-first highest; one ranked concurrency-first highest | Start from simplicity-first (best feasibility, core simplicity, traceability, R14 and R16 scores) and add concurrency-first's agent-native and attention mechanisms, the reasons for that one ranking | all |
| Where Ruby support lives | One review: out of tree, loaded in the S1 and S2 CI paths; another: a pattern file in the base `code` pack | Out of tree, loaded by the fixture workspace's config on every path, because `examples/README.md` says Ruby has no built-in extractor and S10 is a stronger proof with a pack that isn't built in | DR-13 |
| Stale writes | Simplicity-first: every stale write is a card; concurrency-first: an agent's stale write is only an error | Store every stale write as a conflict (nothing lost), raise a card only for a person | DR-7 |
| The hand-off guard | Extensibility-first: deny the chat's own edits on the touch set; concurrency-first: only warn; a review objected to blocking edits the person asked for | Ask the person through the permission prompt; deny with a one-line reason only if asking isn't possible | DR-26 |
| Daemon or not | Extensibility- and concurrency-first: a daemon over a Unix socket; simplicity-first: no daemon | No daemon: `process.run` has the timeout `http.fetch` lacks, and nothing has to survive a reload | DR-1, DR-6 |
| Remote sync loop | Concurrency-first: a mod timer calling `ArtifactData`, auto-allowed; a review flagged it as risky | No timer and no auto-allow; sync is model-driven or a person's key press | DR-31 |
| Number of tools | Simplicity-first: 3; extensibility-first: 11; concurrency-first: 9 | 3 tools, with a token snapshot | DR-22 |
| One schema or many | Simplicity-first: one; extensibility-first: one per contract | One schema in two views with one version, per the principle "one versioned schema behind them all" | DR-37 |

## Ideas from the other designs considered and rejected

| Idea | From | Why rejected |
|---|---|---|
| Executable pack extractors with a trust list, an SDK and an exec-versus-in-process parity test | extensibility-first | Packs that run code need a trust model and a sandbox v0.1 doesn't have; data packs plus built-in linkers meet S10 (DR-9). The conformance kit is kept (DR-10). |
| A jobs API with `job.wait` long-polling | concurrency-first | It exists to work around `http.fetch`'s missing timeout; the CLI path has `process.run` timeouts, and duties under leases already join duplicate work (DR-24) |
| `as_of` and `min_seq` read-your-writes tokens | concurrency-first | Every process reads the same SQLite file, so a read after a commit sees it by construction; `serve` checks `data_version` before each read (DR-8) |
| A `reconcile.routine` role and a `change_significance` kind | extensibility- and concurrency-first | Unplanned changes go to Diffs and never deal a card, so a routine commit needs no decision to stay quiet (DR-20) |
| A generic `planroom op NAME JSON` entry point | concurrency-first | The catalogue already gives every command one JSON request shape over HTTP and tools; `planroom schema --commands` lists them (DR-14) |
| An "Engine" sixth help heading | concurrency-first | Plumbing commands are grouped as "Setup", which is not a concept in the UI; S15's check is on the UI and the command family (DR-22) |
| XState for the statechart simulator | concurrency-first | The machine is already data; a small pure step function with golden parity tests avoids a second machine model (DR-11) |
| A per-repo `.planroom/` export with git-author import | concurrency-first | A second source of truth no scenario needs (DR-34) |

---

## DR-1: SQLite is the single writer; no engine daemon

- **Status:** accepted, verify in WS4. Amended in review rounds 1, 2 and 3.
- **Starting point:** replaced "A single-writer local server per workspace: HTTP commands and queries, plus a scoped live stream."
- **Origin:** the simplicity-first design (central idea, D1); `synchronous = FULL` and the version handshake taken from the concurrency-first design after a design review.
- **Context:** R10 needs serialized writes, compare-and-set and idempotency across several Claude sessions, agents, the canvas and CI. A daemon gives one writer, but it brings a lifecycle: stale locks, orphan processes, upgrades while it runs. The mod can't host a process across reloads: `$.process.spawn` children die on every reload and at session end, and `process.run` blocks on a child that keeps stdio open [mod-api §7, §10]. `http.fetch` has no timeout and no streaming [mod-api §7].
- **Decision:** every engine process writes through `store::commit`, one `BEGIN IMMEDIATE` transaction with a field-level compare-and-set and the idempotency check, in WAL mode with `synchronous = FULL` and a 5 s busy timeout. `planroom serve` exists only for browsers and exits after 30 idle minutes. `planroom watch` (one per mod session) is a reader that may run duties. A newer binary asks an older `serve` to drain before migrating, and an older `watch` or `serve` exits with `schema_mismatch` after a migration.
- **Alternatives rejected:**
  - A self-daemonizing server with a Unix socket (the extensibility- and concurrency-first designs, and mod-api's design implication 1): more moving parts, a lifecycle to manage, and the mod's calls would use `http.fetch` with a hand-made timeout.
  - `synchronous = NORMAL` (the simplicity-first design as written): can lose the last commits on power loss; a design review flagged it.
- **Consequences:**
  - Layout: no `daemon` module; `crates/planroom/src/live.rs` serves both `watch` and `serve`.
  - Contracts: `serve.json` holds `{port, pid, version, control_token}`; `meta.engine_version` in the store.
  - Tests: `store/tests/concurrency.rs` (3 processes × 500 writes), `store/tests/crash.rs`, the S8 harness (DR-32), and `serve_drains_for_newer_engine`.
  - Release: no service files and no lock cleanup in the installer.
  - Agents: the CLI is complete on its own, with no server to start first.
  - Users: nothing to keep running; the canvas starts `serve` on demand.
  - Fallback without a contract change: if WS4 or S8 shows contention, route CLI writes through `serve` when it runs.
- **Amended in review round 1:** multi-process WAL needs a local filesystem (unasked-5). At store open the engine checks the filesystem `.planroom/` is on: network mounts, WSL `/mnt/*`, FUSE, and macOS network volumes and sync folders. On one of those it keeps the store at `DATA_ROOT/stores/<workspace_id>/store.db` with a `.planroom/store.pointer`, and says so in one line. `doctor` reports it, and `store/tests/fs_check.rs::unsafe_filesystem_moves_the_store` tests it. Before a migration the engine writes a `VACUUM INTO` copy under the data root, keeps the last 3, and migrates and folds in one transaction. When an older `watch` or `serve` meets a newer store, the mod says "Restart this session to use Planroom vX" once instead of restarting it [DR-53] (principles-4). The drain token is now the control token, which also registers login codes [DR-29] (security-7). `serve.json` also records the catalogue hash, and `serve --detach` drains a `serve` of an older version, or of the same version with another catalogue hash, not only before a migration, and the mod shows one line when its engine's catalogue doesn't match (operability-5) [DR-58].
- **Amended in review round 2:** SQLite leaves `data_version` unchanged for a connection's own commits, so `watch` and `serve` poll it on a dedicated read-only connection that never writes, and each `store::commit` also hands its `seq` to the process's own live loop. `serve` applies its own committed `seq` to its fold before it replies (E2, correctness-2). Tests `stream.rs::two_sse_clients_on_one_serve_see_each_others_writes`, `a_serve_write_then_read_shows_the_write`, `a_watch_duty_rescan_sends_a_frame`.
- **Amended in review round 3:** every commit and duty transaction reads the store version after `BEGIN IMMEDIATE`, so an older `watch`, `serve` or one-shot process that waited on the lock refuses with `schema_mismatch` instead of writing into a migrated store (correctness-7) [DR-73]. Test `store/tests/migrations.rs::an_older_writer_refuses_after_a_concurrent_migration`. A live `watch` re-reads `user_version` whenever `data_version` changes, so an older one exits with `schema_mismatch` within 200 ms of a migration rather than at its next write (operability-6) [DR-78].
- **Sources (checked 2026-10-25):** [mod-api §7, §10, Design implications 1]; [rust-stack §5] (rusqlite, one writer); the design reviews (`synchronous = NORMAL` gap); candidate-simplicity §3.9, §16 R-1.

## DR-2: Store facts and events; derive the rest on read

- **Status:** accepted. Amended in review rounds 1, 2 and 3.
- **Starting point:** replaced "Event (append-only; everything else is derived from events)" and "SQLite in WAL mode for events, derived state and a decision cache."
- **Origin:** the simplicity-first design (D2).
- **Context:** extracted observations are rebuildable and large; authored actions are small and precious. Storing scans as events floods the log, and derived tables need migrations and repair.
- **Decision:** the store holds two truths: facts (per repo, per commit, rebuildable) and events (authored, append-only). Everything else is a pure fold computed on read; `serve` and `watch` keep the fold in memory and apply new events incrementally. The only caches are model responses, layouts, per-blob facts and the in-memory fold.
- **Alternatives rejected:**
  - Facts as events (the starting point's literal reading): thousands of fact events per scan.
  - Projections in SQLite (the extensibility- and concurrency-first designs): a closure table and derived rows to migrate and rebuild.
- **Consequences:**
  - Layout: `planroom-kernel` holds the fold; `planroom-store` holds ten tables, plus the user-level ledger in its own file.
  - Contracts: only `events` needs migration care in the store.
  - Tests: `cargo bench fold` in `budgets.json` (under 50 ms at 20,000 facts and 5,000 events); `incremental_fold_equals_full_fold`.
  - Release: migrations are rare and small; `export` and `import` are the upgrade escape hatch.
  - Agents and users: a corrupted cache can't corrupt meaning; deleting `store.db` facts and rescanning restores them.
- **Amended in review round 1:** one derived result is stored: the reconciler's outcome, on the `scan` event, so classes don't depend on when a scan ran [DR-44] (correctness-3). Per-file extractors' facts are cached per blob in an eleventh table [DR-47] (unasked-4). Claims and presence depend on the clock, so they are an overlay with an explicit `now`, outside the pure fold, and `now` is recorded in replays (correctness-12). The ledger moves to `DATA_ROOT/ledger.db`, shared by every workspace, so the store keeps ten tables [DR-51] (cost-2).
- **Amended in review round 2:** a `scan` event, or an `undo` or `review` of an event at or below the cached `seq`, refolds `serve`'s and `watch`'s model from scratch, and subtree refolds stay out of v0.1. A kernel proptest checks the incremental fold against a full one outside S8 (E4, correctness-6) [DR-61].
- **Amended in review round 3:** a one-shot read folds inside one read transaction and returns the highest `seq` it saw there as `meta.seq`, so a writer's base never covers a write it didn't read (correctness-8) [DR-73]. Test `store/tests/interleavings.rs::meta_seq_never_exceeds_the_folded_view`. The caches get a `gc` duty, and a second fold benchmark runs at 50,000 events (unasked-9) [DR-47].
- **Sources (checked 2026-10-25):** candidate-simplicity §3.1, §3.7; [ts-stack §1] (layout cache is the one expensive derived value).

## DR-3: Three edit operations and eight event kinds; a view is a value

- **Status:** accepted. Amended in review rounds 1 and 2. Amended by DR-89.
- **Starting point:** extended "A typed property graph of four primitives: Entity, Relation, Event, View" (Entity is `Node`, Relation is `Edge`, Event kept; View dropped as a primitive).
- **Origin:** the simplicity-first design (D4, §3.6).
- **Context:** R6 lists many edit types (regroup, rename, re-scope, re-wire, hide, pin, edit a breakdown), and S15 needs an undo test for every one. One mechanism per edit type multiplies conflict, ghost and attribution rules.
- **Decision:** every change to the graph is an `edit` event with one of `put`, `set`, `retract` on one target. The eight event kinds are `edit`, `review`, `undo`, `version`, `decision`, `claim`, `scan`, `handoff`. A view is a value, `ViewKey {level, lens, focus, project}`; pinned positions are `edit pin`; auto layouts are a cache.
- **Alternatives rejected:** typed override records per edit type (the concurrency- and extensibility-first designs); saved views as a primitive (the starting point), which no requirement needs.
- **Consequences:**
  - Contracts: `Edit { op, target, layer, fields, base, over }`.
  - Tests: `kernel/tests/undo.rs` generates apply-then-undo for every `Op × target kind`, plus review, decision pick, version and claim.
  - Users: pin a layout is undoable and conflict-checked like any edit.
- **Amended in review round 1:** `review` is a catalogue leaf, and `catalogue.rs::undo_classes_snapshot` pins every leaf's undo class, with `share` the only `NotUndoable` one (ux-3, principles-3) [DR-54, DR-55].
- **Amended in review round 2:** an undo reverses every event of the command that wrote its target, and it is a write under compare-and-set with its own authored revisions (E4, correctness-5) [DR-61].
- **Amended by DR-89:** a ninth event kind, `card`, holds a card a write deals before `decide` exists, and `review` picks it. The open cards are the `card` events no `review` names, so the store needs no new table.
- **Sources (checked 2026-10-25):** candidate-simplicity §3.6, §15 D4; the design reviews (three edit ops).

## DR-4: Actor identity and actor rules

- **Status:** accepted; A-3 settled: the Bash tool has no TTY. Amended in review rounds 1, 2 and 3.
- **Starting point:** extended "Events carry the workspace, project, actor, cause and an idempotency key."
- **Origin:** the concurrency-first design (§3.7, §3.11); fixes the fatal flaw two design reviews found in the simplicity-first design, where a CLI call with no `PLANROOM_ACTOR` was attributed to a person, so an agent's Bash calls could skip ghosting and approve its own merge. The extensibility-first design left the CLI's actor undefined.
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
  - `PLANROOM_ACTOR` defaulting to `person:<git email>` (the simplicity-first design): the fatal flaw.
  - Session tokens from a daemon (the concurrency-first design): needs DR-1's daemon.
  - An HMAC of each request with the key: the mod has no Node crypto, and the env var never leaves the mod's `process.run`.
- **Consequences:**
  - Layout: `crates/planroom/src/app/actor.rs`; `packages/mod/src/actor.ts`; `packages/mod/src/guard.ts`.
  - Contracts: `Actor`; `Client`; `ActorRule` on every `CommandSpec`; errors `person_required` and `actor_required`.
  - Tests: `actor.rs` (an unnamed write with no terminal is `actor_required`; a pty with no confirmation is no person; reads need no actor; a person claim without the key fails; tool handlers never send a person); `s03_plan.rs` (an agent's accept fails); `s04_handoff.rs` (an agent's merge fails).
  - Security: a guardrail for "AI proposes, person disposes", not a security boundary between processes of one OS user; `SECURITY.md` says so.
  - Users: no prompts in the mod or the canvas, where person actions are the keys and taps they already make; at a terminal, one confirmation per write.
  - A Bash tool that allocates a TTY changes nothing: a terminal alone never makes a person.
- **Amended in review round 1:** TTY-based person detection is dropped in favor of the `/dev/tty` confirmation; unnamed non-terminal writes are `actor_required`; the person key moves to the mod's `$.store` with only its hash in `store.db`; writes carry a client ID (owner decisions D1 and D2; correctness-2, security-3). `open` is person-only and never prints its code. Importing inbox rows and event exports is person-only and leaves `room_change`. The guard resolves paths, covers every file tool and pty-wrapped person-only commands, and keeps the implementer in worktrees under the data root (D15, D16; security-2, security-3, principles-1) [DR-52]. `handoff revert` is person-only, and `review` is a mixed catalogue leaf (ux-3, principles-3) [DR-54, DR-55]. A-3 settled by a live spike: the Bash tool runs with stdin and stdout not on a TTY, so an unnamed Bash write is always `actor_required`.
- **Amended in review round 2:** the mod passes a one-time action token signed through `crypto.subtle` over `command ‖ canonical(args) ‖ idem ‖ base ‖ at`, never the key; DR-72 made it an ECDSA P-256 signature, so `store.db` keeps only the public key. The rejected alternative above assumed the mod had no crypto, but the mod runtime has `crypto.subtle` [mod-api, module rules]. The guard denies reads of the mod's `$.store` file, and `handoff start`, `merge` and `revert` refuse the `/dev/tty` path, since anyone with a pty can confirm it (E15; security-1, security-8) [DR-64].
- **Amended in review round 3:** DR-72's key pair replaced the key and the HMAC token described above: the mod passes an ECDSA P-256 token, and `store.db` holds only the public key, in `meta.person_pubkey`. `open` and `person init --rekey` refuse the `/dev/tty` path and take a mod press, or for the rekey a token under the old key (F15; security-3, security-4) [DR-75].
- **Sources (checked 2026-10-25):** [mod-api §3] (`CommandRunInput.origin`), [mod-api §8] (`tool.check` sees every tool); [plugin-packaging §3] (Bash environment lacks plugin variables); the design reviews (fatal flaws in the simplicity-first and extensibility-first designs).

## DR-5: Base required; field-level authored revisions

- **Status:** accepted. Amended in review rounds 1, 2 and 3.
- **Starting point:** replaced "Optimistic concurrency on entity versions."
- **Origin:** field-level compare-and-set from the simplicity-first design (§3.9); `base_required` from the concurrency-first design (§3.7); separating fact changes from user revisions from the extensibility-first design (§3.10).
- **Context:** R10 says user-authored state is never last-writer-wins. The simplicity-first design's check `request.base ⊆ current` passes for an empty base, so a write with no base was last-writer-wins (a fatal flaw a design review found). Whole-entity versions make a rename and a regroup of the same node conflict for no reason, and a rescan that bumps an entity's version makes a person's pending edit stale.
- **Decision:**
  - Every write to user-authored state carries `base`, the `seq` of the view the writer acted on. HTTP and tool calls without it get `base_required`; a CLI with no terminal must pass `--base SEQ`; a person at a terminal gets a read-then-confirm.
  - The conflict test is per field: a field the edit sets has an authored revision above `base`, written by another writer instance `{actor, surface, client_id}`: a canvas tab, a mod session, an Artifact viewer or a CLI process. Two tabs of one person conflict like two people; a writer instance never conflicts with its own later writes.
  - Accepting a ghost, or choosing "yours" on a conflict, writes a new `edit` that copies the fields, with `base` from the accepting view and `cause` set to the reviewed event, under the normal compare-and-set. An accept over a newer write becomes a three-way card. The `review` event stays as the audit trail.
  - Scans never change authored revisions. A fact change under an override is a `ScanChangedUnderEdit` diff item, not a stale write.
  - The mod fills `base` for tool calls from the latest `meta.seq` it returned to the model in the turn, else the pane's `seq`.
- **Alternatives rejected:** entity versions (the starting point, the concurrency-first design); `base ⊆ current` (the simplicity-first design as written); optional base.
- **Consequences:**
  - Contracts: `base: Seq` on write requests; `Edit.over` records the fact value an override was made over.
  - Tests: `kernel/tests/cas.rs` (rename plus regroup apply; two renames conflict; an empty base fails; a rescan never stales an edit; two tabs of one person conflict; a tab and a mod press conflict; an accept or a resolution after an intervening write is a card); S8's never-last-writer-wins check.
  - Agents: one number to pass, from the envelope they just read; the error hint says so.
  - Users: no spurious conflicts from background scans.
- **Amended in review round 1:** compare-and-set compares writer instances, not actors (D1, correctness-2), and accepting is a new write (D3, correctness-4). Contracts gain `Event.client` and `WriteCommon.client`.
- **Amended in review round 2:** an applied `undo` sets the authored revision of every field it changes back, so a write from an older base conflicts with it. An Artifact writer instance is one page load, `artifact:<uid>:<instance>`, so a viewer's two devices conflict like two tabs (E4, correctness-5, correctness-7) [DR-61].
- **Amended in review round 3:** a person's field wins over any automatic write. An auto-enabled effect, a prefill or an `infer` output never sets a field whose current value comes from a person's edit, whatever its base, and its decision is recorded as `Superseded`, with no card (F1, correctness-1). A canvas edit that another tab flushes keeps its minting tab's client ID, so its compare-and-set is that tab's (F3, correctness-3) [DR-73].
- **Sources (checked 2026-10-25):** candidate-simplicity §3.9; candidate-concurrency-agents §3.7; candidate-extensibility §3.10; the design reviews (the fatal flaw in the simplicity-first design; field-level compare-and-set; fact version separate from user version).

## DR-6: The mod reaches the engine through the CLI

- **Status:** accepted; A-10 settled the mod's process APIs in a cloud session, and WS5 measures them in a terminal session. Amended in review rounds 1 and 2.
- **Starting point:** none; replaces the research suggestion of `$.http.fetch` over a per-workspace socket [mod-api, Design implications 1].
- **Origin:** the simplicity-first design (D6).
- **Context:** `process.run` has a timeout and a 10 minute cap; `http.fetch` has neither timeout nor streaming [mod-api §7]. Agents use the CLI anyway.
- **Decision:** the mod runs commands with `$.process.run` (argv and env in, one JSON envelope out) and streams with `$.process.spawn` of `planroom watch`, restarted from the last `seq` after a reload. Mod tests stub `process.run` with golden CLI outputs that the Rust tests produce.
- **Alternatives rejected:** `http.fetch` to a daemon (the extensibility- and concurrency-first designs), which also needs DR-1's daemon.
- **Consequences:**
  - Tests: `packages/mod` tests read `tests/golden/cli/*.json`, so a contract drift fails both sides.
  - Agents: every mod action is also a CLI call an agent can make.
  - Fallback: `http.fetch` over `serve`'s socket with the same catalogue, if a surface lacks `process.run`.
- **Amended in review round 1:** `claude plugin test` exits 1 on a package with no `modules` entry and gives tests no file system, so `cargo xtask gen --mod` also builds a harness in `target/mod-test/`: the mod and its tests bundled by esbuild, the workspace packages inlined, the golden CLI outputs inlined as a generated module, and a `hooks/hooks.json` naming the bundle. `claude plugin test target/mod-test` runs it (D14, repo-6) [DR-59]. A-10 settled by a live spike: `$.process.run` works in a cloud session, and `$.process.spawn` streams stdout as it's written.
- **Amended in review round 2:** `cargo xtask test` builds the `target/mod-test/` harness before it runs `claude plugin test`, and plain `cargo xtask gen` builds the committed mod bundle, so `gen --mod` is gone (repo-10) [DR-70].
- **Sources (checked 2026-10-25):** [mod-api §7]; candidate-simplicity §15 D6; the design reviews (golden CLI outputs for mod tests).

## DR-7: A stale write is stored; only a person's raises a card

- **Status:** accepted. Amended in review round 1.
- **Starting point:** extended "Optimistic concurrency on entity versions."
- **Origin:** the concurrency-first design (§3.9), combined with the simplicity-first design's rule that nothing is lost (§3.9). Resolves a disagreement between the design reviews: one flagged the simplicity-first design's cards for agent races as a principle 1 violation; another noted the concurrency-first design dropped the agent's write.
- **Context:** principle 1: interrupt a person only for decisions only they can make. Under S8's load, two agents race often. R10 and the principle "never lose what someone did" forbid discarding the losing write.
- **Decision:** a stale write is always stored as an `edit` with `status: conflict`. A person's stale write raises a three-way card with "theirs" pre-selected. An agent's or model's returns `stale_write` with a compact three-way diff (at most 1 KB) in `details`, is listed in Diffs under "agent retries", collapsed, and raises no card. A person's edit over a pending ghost is no conflict: it supersedes the ghost, which is listed in Diffs as superseded and raises no card. S8's stale write is person against person: the canvas tab against a mod press on the same field.
- **Alternatives rejected:** a card for every conflict (the simplicity-first design); an error with nothing stored (the concurrency-first design); an automatic merge (never).
- **Consequences:**
  - Contracts: error `stale_write` with `details {event, base, theirs, yours, card}`, where `card` is present only for a person's write.
  - Tests: S8 asserts the tab's stale rename over a mod press is a card, agent B's ghost is superseded, and an agent's stale write is a stored conflict with no card; `kernel/tests/cas.rs` pins the press, tab and ghost cases.
  - Agents: they get everything they need to retry in one reply.
- **Amended in review round 1:** a person's edit supersedes a pending ghost, and S8's stale write is person against person (D4, correctness-1); choosing "yours" writes a new edit under compare-and-set [DR-5].
- **Sources (checked 2026-10-25):** candidate-concurrency-agents §3.9; candidate-simplicity §3.9; the design reviews (a fatal flaw in the simplicity-first design, a concurrency gap in the concurrency-first design).

## DR-8: Idempotency with request hashes, replays and a mod outbox

- **Status:** accepted. Amended in review rounds 1, 2 and 3, and by DR-86.
- **Starting point:** extended "Events carry ... an idempotency key."
- **Origin:** the concurrency-first design (§3.8, §3.10, §7.5).
- **Context:** S8 requires safe retries. A reused key with a different payload silently replayed the wrong result in the simplicity-first design, as a design review found. Spawned children and module state die on a mod reload [mod-api §7], so an in-flight write could be lost.
- **Decision:**
  - Every event stores `idem` (UNIQUE) and `request_hash`. Same key and hash: the stored reply with `meta.replayed: true`. Same key, other hash: `idempotency_mismatch`.
  - A derived key when none is given: `sha256(actor ‖ command ‖ canonical(args) ‖ base)`. Tool calls use the `tool_use_id` that every `tool.call` input carries.
  - The mod keeps an outbox atom in `$.state` and resends it with the same keys after a reload.
  - The canvas keeps pending edits in `sessionStorage`, wrapped in try/catch, until confirmed.
  - Read-your-writes needs no token (the `as_of` and `min_seq` tokens were rejected).
- **Alternatives rejected:** client-minted keys only (agents forget them); `min_seq` tokens (unneeded with one SQLite file).
- **Consequences:**
  - Contracts: `meta.replayed`; error `idempotency_mismatch`; the `request_hash` column.
  - Tests: S8 retries and duplicates; `store/tests/idem.rs`; a mod test that reloads mid-write and checks one event.
- **Amended in review round 1:** a command that writes several events keys them `idem#1`, `idem#2` and so on and stores the whole reply on the first; a replay of a derived key sets `meta.replayed_from` and hints to pass a new `--idem`; a decision event's key is `sha256(kind ‖ kind_version ‖ subject ‖ input_hash)` (correctness-11, correctness-9). Test `store/tests/idem.rs::multi_event_replay_returns_the_whole_reply`. A-8 settled by the spike's diagnostics: every `tool.call` input carries `tool_use_id`, which tool calls use as their key.
- **Amended in review round 2:** a missing key is derived only for a `needs_base` command, where the base pins the intent. Every other command mints a fresh uuid v7 per invocation, and the mod, canvas and Artifact page mint one per user action. A scan keys each repo's event by `(repo, commit, packs_hash, previous active snapshot)`, so a move back to an earlier head records a new scan. `handoff merge` and `handoff revert` key each step and follow a step protocol: a retry with the same key resumes from the last committed step, a replay happens only after a terminal step, and a running detached duty is joined, never run twice (E1, correctness-1). Tests `retries.rs::base_less_commands_mint_a_fresh_key`, `a_rescan_after_a_head_move_writes_a_new_scan_event`, `a_move_back_to_an_earlier_head_records_a_new_scan`; `handoff_preflight.rs::a_retry_with_the_same_key_resumes_mid_merge`, `a_retry_joins_a_running_merge_duty`. Pending canvas edits move from `sessionStorage` to `localStorage`, keyed by workspace ID with their original keys, and any signed-in tab of the workspace flushes them, so a reload, a closed tab or a `serve` restart loses nothing (E10, principles-4) [DR-29]. Tests `tests/e2e/surface.spec.ts::reload_mid_edit_keeps_the_edit`, `serve_restart_keeps_pending_edits`.
- **Amended in review round 3:** a pending canvas edit stores its tab's client ID, and the flushing tab sends it. A request hash covers the command, its canonical arguments, `base`, the actor and the client, never transport fields. A decision event's hash covers only its inputs, so a duplicate run replays even when Jev's pick drifted (F3; correctness-3, correctness-9) [DR-73]. Tests `tests/e2e/surface.spec.ts::a_flush_by_another_tab_keeps_the_original_writer`, `a_flush_of_a_committed_edit_replays`.
- **Amended by DR-86:** a scan's key takes the snapshot's extraction-inputs hash where it took `packs_hash`, so a scan under other inputs at the same commit is another key.
- **Sources (checked 2026-10-25):** [mod-api §7, §10]; candidate-concurrency-agents §3.8; the design reviews.

## DR-9: Packs are data; built-in extractors and linkers are declared by packs

- **Status:** accepted. Amended in review rounds 1, 2 and 3.
- **Starting point:** kept and narrowed "Packs. Declarative files that add kinds, levels, roll-up rules, extractors, lenses, decision kinds and a default view."
- **Origin:** the simplicity-first design (D11); the extensibility-first design's executable packs rejected.
- **Context:** S10 loads a pack from outside the source tree; R11 says domains plug in with no core change. A pack that runs code needs a trust model; scanned repos are untrusted input.
- **Decision:** a pack is TOML, tree-sitter queries, prompts and evals, never code. It may declare built-in Rust extractors and linkers by name, and declarative regex or tree-sitter extractors that emit nodes, edges and hints. `requires` is `["code"]` or nothing. Roll-up rules are fixed in the kernel; packs choose what a lens lifts.
- **Alternatives rejected:** exec extractors over NDJSON with a trust list, an SDK and a parity test (the extensibility-first design, and the concurrency-first design's exec Ruby pack): code from anywhere, no sandbox.
- **Consequences:**
  - Security: nothing in a pack runs, and the loader rejects executable files. A pack is still policy-bearing, so an out-of-tree pack's `auto_enabled` waits for a person [DR-48]; loading it deals no card, because a person listed it [DR-60].
  - Layout: `packs/` holds data only; Rust logic a pack needs lives in `planroom-extract`.
  - Tests: `tests/packs.rs` (validation, the inward-only rule, no executables).
  - Users: a pack can't do what the declarative runner and built-ins can't; a format that needs a parser is a built-in.
- **Amended in review round 1:** pack data gains `[[languages]]` and `[[image_roles]]`, typed emit props and ID prefixes, and extractor and linker names are pack-qualified [DR-46]. An out-of-tree pack is data but policy-bearing: its `auto_enabled` counts only after a person enables auto-action per kind (D19, security-5) [DR-48]. Its first-load card was dropped by DR-60. `code` is the base ring every domain pack attaches to, and the other packs form the outer ring: a pack may require only a pack in a lower ring, which in v0.1 means `code` or nothing. That is R16's "packs depend inward" for packs, and `packs.rs::requires_is_inward_only` enforces it (repo-5).
- **Amended in review round 2:** rings are by origin. The built-in packs are the inner rings, and an out-of-tree pack may require any built-in pack but no other out-of-tree pack. A pack references only its own kinds, those of the packs it requires, and the kernel's (extensibility-7) [DR-62]. The self-test plants a built-in that requires an outer ring and an out-of-tree pack that requires another. Test `packs.rs::an_out_of_tree_pack_may_require_any_built_in`.
- **Amended in review round 3:** an out-of-tree pack's `[[hints]]` entry that no loaded linker consumes is `pack_invalid`, naming the alias and `@ext:` pattern for same-key joins, so a declared hint kind never silently joins nothing (extensibility-5, option b). Test `packs.rs::an_unconsumed_hint_kind_is_pack_invalid`.
- **Sources (checked 2026-10-25):** candidate-simplicity §4.1, §15 D11; candidate-extensibility §4.4, §4.11; the design reviews (exec packs run unsandboxed code).

## DR-10: A pack conformance kit

- **Status:** accepted. Amended in review round 2.
- **Starting point:** extended "Packs."
- **Origin:** the extensibility-first design (§4.10), adapted to data packs.
- **Context:** S10's out-of-tree path should be first-class and checked in CI, and pack authors outside the repo have no `xtask`.
- **Decision:** `planroom doctor --pack DIR` validates the manifest, runs every extractor over the pack's `tests/inputs/`, compares sorted facts with `tests/expected.jsonl`, runs twice to prove determinism, and validates eval files. CI runs it on `packs/*` and `examples/packs/*`.
- **Alternatives rejected:** a separate `pack` verb (more surface); the exec parity test (no exec packs).
- **Consequences:** contracts gain the pack test layout; docs gain "Write a pack" in guides.
- **Amended in review round 2:** a `pack` verb now exists, but only to change the workspace's pack set: `pack add DIR` and `pack remove ID`, both person-only. The conformance kit stays in `doctor --pack`, the only command that reads `PLANROOM_PACKS` (E6) [DR-62].
- **Sources (checked 2026-10-25):** candidate-extensibility §4.10; the design reviews.

## DR-11: Scope cuts for v0.1

- **Status:** accepted.
- **Starting point:** replaced "precise: SCIP/LSIF or a language server", "a dependency matrix synced with the graph", "XState for the simulator"; config formats k8s and Terraform deferred.
- **Origin:** the simplicity-first design (D10, D15, D16).
- **Context:** no requirement or scenario needs these, and each adds a library or a second model. Principle: keep the bar on quality, not scope.
- **Decision:** v0.1 ships no SCIP reader, no dependency matrix, no XState, and no k8s or Terraform extractors. Each is one more built-in or view later, with no kernel change. The canvas's statechart step-through is a pure function in `@planroom/ui`, checked against the Rust `ask machine --step` golden outputs, so the Artifact can step a machine with no engine.
- **Alternatives rejected:** shipping them (the concurrency-first design included the matrix and XState).
- **Consequences:** known gaps recorded at release; `feature_list.json` adds no entries for them.
- **Sources (checked 2026-10-25):** candidate-simplicity §15; [ts-stack §2] (xstate size, kept as a rejected option).

## DR-12: Hints, built-in linkers, endpoints as via nodes, and full evidence recall

- **Status:** accepted, verify in M1 (S1's M1 subset) and M3 (the full S1 comparator). Amended in review rounds 1, 2 and 3. Amended by DR-89.
- **Starting point:** extended "Extractors, in tiers" and "Relation {..., evidence[], confidence ...}".
- **Origin:** the concurrency-first design's hints and two-phase scan (§4.2), the extensibility-first design's link phase (§4.6). Fixes the extensibility-first design's fatal flaw (S4 step 4 missed the planted regression) and closes the simplicity-first design's gap (no extractor for the audit DSN, found by a design review).
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
  - Aliases only (the simplicity-first design): can't join code with compose wiring or a literal DSN.
  - A `code.db_open` hint with only an env var name (the extensibility-first design): misses the regression.
  - Requiring at least one truth evidence item per relation: a weakening of the frozen truth without the KICKOFF process.
- **Consequences:**
  - Contracts: `Hint`; `Edge.via_ref`; the `link` snapshot purpose; warning `evidence_dropped`.
  - Tests: `extract/tests/link.rs` (including `metric_mentions_add_evidence_only` and `otel_counter_joins_its_prometheus_name`); `extract/tests/formats.rs::changelog_sets_repo_version`; `extract/tests/contract.rs::git_facts_carry_ref_evidence`; `s01_first_look.rs` (every truth relation with every evidence item); `s04_handoff.rs` (post-merge regression from the code alone); a docstring-is-not-evidence test.
  - Risk: call-site evidence by name may over-include; extra verified evidence is allowed ("a scan may find more" [TRUTH about]).
- **Amended in review round 1:** comments may add evidence but never relations, `Evidence::Ref` covers git-derived facts, and a `changelog` extractor supplies Go repos' versions (D21; correctness-5, correctness-6). Hints carry IDs and provenance (extensibility-4). TRUTH needs `orders/internal/http/middleware.go:26` and `orders/CHANGELOG.md:7` [TRUTH questions.observability.alerts, questions.stale_versions].
- **Amended in review round 2:** a scan that changes any repo's active snapshot links inside its committing `IMMEDIATE` transaction, from the stored hints of the then-active per-repo snapshots. The link snapshot is keyed by the sha256 of its sorted member snapshot IDs and `packs_hash`, and its ID is on the `scan` event (`Scan.link`) and in the reconcile outcome, so `fold(at)` finds the one link of the active set (E3, correctness-3). Tests `duties.rs::concurrent_rescans_link_the_final_active_set`; `kernel/tests/fold.rs::the_link_snapshot_matches_the_active_set`.
- **Amended in review round 3:** a `route_call` whose method or path is a parameter of its enclosing function is bound at each call site in the same container that passes a literal or a template: one route call per call site, with both lines as evidence. A template's interpolated segment normalizes to `{}` and matches any `{param}` segment of a served route. A route call whose base isn't a literal takes its host from a same-file `env_read` that compose sets, as a database open does. That gives S4's post-hand-off `worker -> orders` edge all five truth evidence items (F2, correctness-2). Test `extract/tests/link.rs::a_wrapper_call_binds_method_and_path`: line 44 yields `POST /orders/{id}/events` and line 39 `GET /orders/{id}`.
- **Amended by DR-89:** full evidence recall is verified in two steps: M1's S1 comparator checks the subset DR-89 states, and M3's checks the whole truth.
- **Sources (checked 2026-10-25):** `examples/TRUTH.json` (relations, scenarios.S4); candidate-concurrency-agents §4.2; candidate-extensibility §4.6; the design reviews (the fatal flaw in the extensibility-first design; `*.db_dsn`).

## DR-13: Ruby is an out-of-tree data pack the fixture workspace loads

- **Status:** accepted. Amended in review rounds 1, 2 and 3.
- **Starting point:** kept "syntactic: tree-sitter queries and import resolvers for TS/JS, Python, Go and Rust" (no Ruby built in).
- **Origin:** a design review's proposal; another review proposed a Ruby pattern file in the base `code` pack instead. The extensibility- and concurrency-first designs kept Ruby in test-only packs, so their S1 truth match failed (fatal flaws two design reviews found).
- **Context:** `TRUTH.json` lists the `healthcheck` container and its two `GET /healthz` edges as things that must exist [TRUTH containers, relations]. `examples/README.md` says Ruby has no built-in extractor, and S10 needs a new language to work through the extractor contract alone.
- **Decision:** Ruby support is the data pack `examples/packs/ruby/`, not embedded in the binary. `cargo xtask fixture OUT` writes `OUT/.planroom/config.toml` with `packs = ["<repo>/examples/packs/ruby"]`, so every CI path and evidence run of the fixture loads it, and `feature_list.json` records that fixture state. S10 builds a fixture without that config, shows `healthcheck` absent, then loads copies of the Ruby and flags packs from a temporary directory outside the tree. Its no-core-change proof scans only core, the shipped engine crates (`contract`, `kernel`, `store`, `extract`, `models`, `planroom` with `build.rs`) and `packs/`, not `xtask` or `testkit`, for the toy packs' identifiers. It also shows that the same prebuilt binary, by sha256, runs S1 without the packs and S10 with them.
- **Alternatives rejected:**
  - Ruby in the base pack (a design review's proposal, and the simplicity-first design): contradicts the fixture contract and weakens S10's proof.
  - Ruby only in the S10 test (the extensibility- and concurrency-first designs): S1's truth match fails.
- **Consequences:**
  - Layout: `examples/packs/ruby/`, `examples/packs/flags/`.
  - Tests: `s01_first_look.rs` and `s02_questions.rs` use the configured fixture; `s10_extensibility.rs` asserts absence, then presence, then `no_engine_source_mentions_the_packs` and `same_binary_runs_with_and_without_the_packs`.
  - Users: a real workspace with Ruby files gets the universal tier for them until the Ruby pack is added; the release archive ships `examples/packs/ruby` for that.
  - Still open: add one line to `examples/README.md` saying the fixture workspace loads `examples/packs/ruby` through its config (not a `TRUTH.json` change).
- **Amended in review round 1:** the S10 check is restated: core is the shipped engine crates and `packs/`, matched on pack identifiers, plus a binary-identity proof in place of `git diff --stat crates/` (D12, extensibility-3). The Ruby pack declares its language in `[[languages]]` [DR-46].
- **Amended in review round 2:** S10 adds the copies with the person's `planroom pack add`, because `PLANROOM_PACKS` no longer loads packs (E6) [DR-62].
- **Amended in review round 3:** the no-core-change scan matches the toy packs' identifiers as quoted literals: `"flags.flag"`, `"flags.reads_flag"`, `"flag_stale"`, `"ruby/ruby-http"`, `"ruby/ruby-script"` and `"rb"`. Ordinary code such as `flags.push(...)` then passes, and the binary-identity proof covers the rest (extensibility-7).
- **Sources (checked 2026-10-25):** `examples/README.md` (scenario hooks, S10); `examples/TRUTH.json` (containers, relations, scenarios.S10); the design reviews (proposals and fatal flaws).

## DR-14: One command catalogue; no MCP server

- **Status:** accepted.
- **Starting point:** replaced "`rmcp` (if there's an MCP server)"; principle 2 lists "an MCP server if the architecture calls for one".
- **Origin:** the simplicity-first design (§7.1, D5); handler-coverage test from the concurrency-first design (§7.1).
- **Context:** the CLI, HTTP API, tools and `/room` must stay in step. Plugin tools already appear as `mcp__planroom__*` [mod-api §4]. `rmcp` shipped 3 majors in 5 months [rust-stack §8].
- **Decision:** `planroom-contract::Command` is one enum. Each variant carries its request and result types, idea, actor rule, permission class, default budget, error codes, undo and surfaces. It generates the clap tree, the HTTP routes, the three tool schemas, the `/room` parser and typeahead, and `docs/reference/cli.md`. A test asserts every variant has exactly one handler. No MCP server: sessions without mods use the skills, which teach the CLI.
- **Alternatives rejected:** an rmcp stdio server (a second wiring path and a fast-moving dependency); hand-written bindings per surface.
- **Consequences:**
  - Layout: one `app/` handler per command; no `.mcp.json`.
  - Tests: `catalogue_has_one_handler_per_command`; the CLI reference drift test; the tool schema drift test.
  - Agents: one request shape per command everywhere; `planroom schema --commands` lists them.
- **Sources (checked 2026-10-25):** [mod-api §4]; [rust-stack §8]; [plugin-packaging, Design implications] (one tool-wiring path).

## DR-15: Lenses with flow, a dedicated path lens, and question pipelines

- **Status:** accepted. Amended in review round 3.
- **Starting point:** replaced "A typed query builder plus named queries (neighbourhood, paths, roll-ups, diffs)."
- **Origin:** the simplicity-first design (§4.6, D14); `fires-if` and `bump-breaks` named from the extensibility-first design (§4.7), as a design review proposed.
- **Context:** S2's path rule follows only `http`, `queue_publish` and `queue_consume`, the last one backward [TRUTH questions.path_storefront_to_worker.rule]. R11 names "what fires if this breaks?" and "what breaks if I bump X?", which the simplicity-first design left out, as a design review noted. A query builder is a second API to version.
- **Decision:** a lens is data: relation kinds, a flow direction per kind, a default level. The default lens is `runtime` (with `db`); the `path` question uses the `flow` lens, which has exactly the truth's edge kinds. A question is a named pipeline of one to three kernel query shapes from a fixed set of twelve; `fires-if` and `bump-breaks` are pipelines. A new shape needs two packs that use it and a decision record.
- **Alternatives rejected:** a runtime lens with `db` and `deploys` for paths (the extensibility- and concurrency-first designs): goes beyond the truth's rule; a query builder (the starting point).
- **Consequences:** contracts gain `QuestionSpec { steps }`; tests `s02_questions.rs`, `kernel/tests/query.rs`, `kernel/tests/questions.rs::fires_if_lists_direct_and_indirect_alerts`, `bump_breaks_lists_consumers`.
- **Amended in review round 3:** `LensDecl` gains `kinds`, the node kinds a lens is about. Under a lens, `map` takes the lens's level and lists those kinds' nodes first, edges or none, so `map --lens flags` shows the edge-less `gift_wrap` (F13, extensibility-1). `kinds`, `stage`, `lint`, `missing` and `drift` are built-in questions the engine serves, not shape pipelines, and `ask` alone lists every loaded kind, lens and question with its arguments and owning pack. Each v0.1 shape ships because a v0.1 question uses it, and the two-pack bar applies to shapes added after v0.1 (extensibility-4). Tests `s10_extensibility.rs::an_edgeless_lens_kind_shows_on_its_map`, `ask_lists_what_is_loaded`.
- **Sources (checked 2026-10-25):** `examples/TRUTH.json` (questions); candidate-simplicity §4.6; candidate-extensibility §4.7; the design reviews.

## DR-16: The git CLI is the only git adapter

- **Status:** accepted. Amended in review rounds 2 and 3.
- **Starting point:** replaced "`gix` or `git2`."
- **Origin:** the simplicity-first design (D7).
- **Context:** the scan needs history with numstat, tags and branches; the hand-off needs `worktree add`, `apply --3way`, `commit` and `merge --no-ff`. gix has no porcelain merge and its worktree mutation was not tried [rust-stack §6]. The truth's post-hand-off state is defined by `git apply` [TRUTH scenarios.S4.post_handoff].
- **Decision:** `planroom-extract::git` shells out to `git` (at least 2.30) with `-c core.hooksPath=/dev/null -c core.fsmonitor=false --no-optional-locks`, `LC_ALL=C` and `-z` porcelain.
- **Alternatives rejected:** gix 0.90.0 (a new 0.x every 2 to 4 weeks, a 27 to 53 s build unit, no merge porcelain); git2 0.21.0 (C libgit2 and its CVEs).
- **Consequences:**
  - Release: git is a runtime requirement; `doctor` checks the version.
  - Tests: CI on Linux and macOS; a hook fixture that must never fire.
  - Performance: `git log --numstat` took 0.90 s against 1.67 s for gix on 2,023 commits [rust-stack §6].
- **Amended in review round 2:** git runs with `--no-ext-diff --no-textconv` on every `diff`, `show` and `log`, with `GIT_TERMINAL_PROMPT=0`, `GIT_LFS_SKIP_SMUDGE=1` and a cleared environment, at version 2.35.2 or later, so `safe.directory` refuses a repo another user owns. A checkout or merge in a repo whose local config names a command git would run (a filter, a diff command or textconv, a merge driver, an editor, a pager, an ssh command or an include) is refused with `repo_config_refused` and one card. The hand-off's writes are `clone --local --no-checkout` and `fetch`, not `worktree add` (E15; security-2, security-6) [DR-64]. Tests `git_hardening.rs::textconv_ext_diff_and_drivers_never_fire`, `a_repo_whose_config_names_a_command_is_refused_before_merge`.
- **Amended in review round 3:** the engine itself refuses a repo whose git directory another user owns, so the floor drops to git 2.34.1, Ubuntu 22.04's, whose backported fix a version check refused; below it `scan` and `doctor` return `git_too_old` (unasked-8) [DR-78].
- **Sources (checked 2026-10-25):** [rust-stack §6]; the design reviews.

## DR-17: Kernel roles

- **Status:** accepted. Amended in review rounds 1 and 2.
- **Starting point:** extended "Jev policy. Each decision kind declares its question, thresholds and escalation."
- **Origin:** the extensibility-first design (§5.2).
- **Context:** the kernel needs some decisions (an override whose target vanished, a change unit the reconciler can't place, a next action with no target) but must name no pack kind (R11, R16).
- **Decision:** three kernel roles: `rescan.retarget`, `reconcile.residual`, `attach.target`. A pack's decision kind binds one with `role = "..."`. `attach.target` is raised for an accepted `kernel.item` with no `kernel.touches` edge, narrowed by the bound kind's `subject` and `when` [DR-45]. An unbound role lists its item in Diffs and deals no card; two bindings are `pack_invalid`.
- **Alternatives rejected:** naming pack kinds in the kernel (fails the domain-free scan); `reconcile.routine` (DR-20 makes it unneeded); `intent.stage` (stages are pack data already).
- **Consequences:** contracts gain `Role`; tests `kernel/tests/roles.rs` and the domain-free literal scan.
- **Amended in review round 1:** roles still bind decision kinds only; the node kinds and relation the kernel's algorithms need are kernel vocabulary instead [DR-45] (D10, extensibility-1). An unbound role no longer deals a card with no pick; it lists its item in Diffs (D9, principles-2) [DR-50].
- **Amended in review round 2:** a kind bound to `rescan.retarget` or `reconcile.residual` takes the role itself as its subject, which the raiser supplies as `Subject::Role { role, key }`, and a `rescan.retarget` kind with `person` stakes may use the `retarget` effect (E7, extensibility-2) [DR-63].
- **Sources (checked 2026-10-25):** candidate-extensibility §5.2; the design reviews.

## DR-18: Claude backends: the gateway, and the session's model through the mod

- **Status:** accepted; A-9 settled. Amended in review rounds 1, 2 and 3.
- **Starting point:** none; implements the constraint "The mod uses the chat model and `$.model.complete`, the Artifact uses `sample`, and the engine uses Claude models on the gateway, with the model pinned in config."
- **Origin:** the extensibility-first design's session-model adapter (§5.1), the concurrency-first design's broker (§5.1); fixes the fatal flaw all three design reviews found in the simplicity-first design, which dropped `$.model.complete` with no record (a binding constraint can't be overridden by a decision record anyway).
- **Context:** most plugin users won't set `AI_GATEWAY_API_KEY`. `$.model.complete` uses the session's own account and needs an explicit model; behind a gateway, cache marks go in `prompt` [mod-api §6]. It isn't in the session's cost totals.
- **Decision:**
  - `planroom-models::claude` has a `gateway` backend (when a gateway credential resolves; `anthropic/claude-haiku-5.5` cheap and `anthropic/claude-sonnet-5.5` capable, pinned in config) and a `session` backend (no credential and a mod attached).
  - With `session`, a Claude call becomes a `ModelRequest` listed in `meta.model_requests` and written to `.planroom/models/requests/`. The mod runs `$.model.complete` with the stable prefix in `prompt` blocks, the last marked `cache: true`, and imports a `planroom.model-response/v1` file. The response lands in `model_cache` under the same input hash, so the command picks it up when re-run.
  - The first session request deals the session provider's consent card ("Use your Claude session for Planroom's Claude calls?") with "Yes" pre-selected, recorded as a decision [DR-49]. Usage goes into the user-level ledger with `provider = session`, priced from the pinned table, and shows in the status line, as does each spawned agent's usage [DR-51].
- **Alternatives rejected:** no `$.model.complete` (the simplicity-first design: violates the constraint); a broker over a daemon's stream (the concurrency- and extensibility-first designs: needs DR-1's daemon); the mod calling Claude for its own summaries (the engine authors every string, DR-21).
- **Consequences:**
  - Contracts: `ModelRequest`, `ModelResponse` (`planroom.model-response/v1`), `meta.model_requests`.
  - Tests: `models/tests/session_backend.rs`; a mod test with a stubbed `model.complete`; the S9 evidence run uses the session backend once.
  - Cost: the spend guards cover session usage too.
  - Still open: confirm that the session backend, used whenever the gateway isn't configured, satisfies "the mod uses `$.model.complete`", or add a second mod-local use.
- **Amended in review round 1:** the consent card is one case of the per-provider card before any billable call (D7, unasked-3) [DR-49]. The build container's proxy is a gateway credential only with `PLANROOM_LIVE_GATEWAY=1` (D8, cost-1). Session and spawned-agent usage is priced into the user-level ledger (D20; cost-2, cost-10) [DR-51]. A-9 settled by a live spike in a cloud session: `$.model.complete` with `cache: true` caches through the session's provider, a write of 14,403 tokens and then a read of 14,403.
- **Amended in review round 2:** the session backend's 5-minute cache can't hold across person-paced stages, so the S3 hit-rate floor applies to gateway recordings, the session backend's rate is evidence only, and the session card says the cache lasts 5 minutes (cost-10). Session and spawned-agent usage is priced and reported in the ledger but no longer gated by the daily cap, which counts real dollars (cost-8) [DR-65].
- **Amended in review round 3:** the broker re-runs a fulfilled request's handle itself, with the original cause and `idem` plus `#rerun`, as the tool call's actor or, for a press, as `agent:<session8>:broker`, since it never signs a person token; a leaf that needs a person gets one key on its card instead. Meanwhile the pane and the canvas show an engine-authored drafting line, so a stage started from the pane lands its ghosts with no second press (principles-5). Test `packages/mod/src/broker.test.ts::fulfilled_request_reruns_its_handle`. The mod sends a session request file only when its bytes match the `sha256` its `meta.model_requests` entry carries, and import refuses a response whose `request_sha256` isn't the engine's (security-8) [DR-75].
- **Sources (checked 2026-10-25):** [mod-api §6]; [claude-gateway §2, §7]; KICKOFF `<constraints>` Claude; the design reviews (the fatal flaw in the simplicity-first design).

## DR-19: Eval protocol for thresholds

- **Status:** accepted. Amended in review rounds 1 and 2.
- **Starting point:** extended "Jev policy" (thresholds come only from evals).
- **Origin:** hash split and byte-for-byte recompute from the simplicity-first design (§5.9); `--verify-order` and injection canaries from the extensibility-first design (§5.8); option shuffling from the concurrency-first design (§5.8).
- **Context:** S7 needs an independent labeller who hasn't seen Jev's outputs, cases held out from tuning, and auto-action only at observed precision of at least 0.95 on at least 60 cases. Jev reads instructions literally and is confidently wrong on some ambiguous cases [jev §5].
- **Decision:** a case writer makes states (with at least two injection canaries per kind); an independent labeller labels them and the labels are committed before any Jev call (`cargo xtask eval --verify-order` checks git order and the cases digest); the split is `sha256(case_id) mod 5 < 2` for tune; instructions and bands are fitted on tune only; two live runs on `typesafe-ai/jev-1.14.0`, the second with option order shuffled; `auto_enabled` only at holdout precision of at least 0.95 on 60 or more cases in both runs with at least 30 acted on; CI recomputes every result byte for byte.
- **Alternatives rejected:** a hand-chosen split; thresholds set in pack files; one run.
- **Consequences:**
  - Contracts: `EvalCase`, `EvalResult` (`planroom.eval/v1`).
  - Tests: `kernel/tests/evals.rs`, `decision_policy_matches_eval_results`.
  - Spend: about $0.02 to $0.05 live for a full pass [jev §9], within the build's live spend plan [DR-51].
- **Amended in review round 1:** eval runs append to the build's spend log and count against its $3 (D20, cost-2, cost-9) [DR-51]. An out-of-tree pack's `auto_enabled` waits for a person (D19, security-5) [DR-48].
- **Amended in review round 2:** the recompute test reads pack files, so it moves to `crates/planroom/tests/evals.rs` as `evals_recompute_byte_for_byte` and `decision_policy_matches_eval_results`; the kernel's own tests take their data as values, so its no-I/O lint holds for every target (repo-8) [DR-70].
- **Sources (checked 2026-10-25):** [jev §5, §8, §9, §10]; the design reviews.

## DR-20: One card at a time, batched by cause

- **Status:** accepted. Amended in review round 1.
- **Starting point:** extended "Jev policy" and the remote mode's "shows cached decisions and queues new ones."
- **Origin:** the simplicity-first design (§5.6); per-turn batching and the prompt section from the extensibility-first design (§3.3, §7.5).
- **Context:** principle 1: decisions cheap, pre-decided, batched, one tap. S7: a routine rescan raises at most one card.
- **Decision:**
  - Surfaces show the top card by `stakes × staleness` and a count of the rest.
  - Ghosts from one cause (one chat turn, one plan stage run, one curation run) batch into one card ("Apply Claude's 2 edits?"), and so do one kind's cards and prefills [DR-50].
  - Unplanned changes go to Diffs and never deal a card; only regressions do.
  - The mod's `prompt.compose` section is one stable block of at most 300 tokens, byte-identical across turns and states, so it stays in the cached prefix. Per-turn hints go in the user-side context through `prompt.submit`, and the tools are registered before the engine resolves, so the tool list never changes mid-session.
- **Alternatives rejected:** a card per ghost; a `reconcile.routine` decision kind (the extensibility- and concurrency-first designs).
- **Consequences:** contracts gain `Cause::Turn`; tests `s07_decisions.rs` (at most one card), `kernel/tests/policy.rs::cards_batch_by_cause`.
- **Amended in review round 1:** one kind's cards and prefills batch like ghosts (D9, principles-2) [DR-50]. Dynamic hints, such as the remote-sync line, never enter the composed section, and the tools register from the static catalogue before the engine resolves (cost-11, principles-9). Test `packages/mod/src/prompt.test.ts::section_is_byte_identical_across_states`.
- **Sources (checked 2026-10-25):** candidate-simplicity §5.6; candidate-extensibility §3.3, §7.5; the design reviews.

## DR-21: Agent-first envelope and engine-authored strings

- **Status:** accepted. Amended in review rounds 1, 2 and 3.
- **Starting point:** none; implements principle 2 ("structured, compact, paginated, with stable IDs and pointers to evidence").
- **Origin:** the simplicity-first design's envelope (§7.2); handles, `ambiguous_ref`, category and the retry flag from the extensibility-first design (§7.3); `empty.next` from the concurrency-first design (§7.2); the string catalog and `same_output` from the extensibility-first design (§7.1); evidence verification at ingest from the extensibility- and concurrency-first designs.
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
- **Amended in review round 2:** a mod tool returns one `ToolResult` shape, so an error reaches the agent whole, with its code, retry class, hint and details. `meta` omits empty and default fields except `seq` and `size`, and drops `models` when both call counts are 0. `consent_required` has retry `never`, and a code only a person can clear always has `never` (principles-1, principles-10, principles-11) [DR-66].
- **Amended in review round 3:** every agent-facing output has a size budget, discovery included. `schema --command LEAF` and `schema --def NAME` return one leaf's or one type's definitions with their `$defs` closure under 1,500 tokens, and `schema --commands` pages. `doctor --json` names its report file instead of carrying 50 log lines, and a pack question's `budget_tokens` is clamped to 3,000. Only `watch`, `serve` and `export` stream (F12, principles-4). Test `catalogue.rs::every_leaf_has_a_budget_or_streams`.
- **Sources (checked 2026-10-25):** [iris-release-bar §6]; candidate-simplicity §7.2, candidate-extensibility §7.1 and §7.3, candidate-concurrency-agents §7.2; the design reviews.

## DR-22: A tiny surface with measured always-on cost

- **Status:** accepted. Amended in review rounds 1, 2 and 3.
- **Starting point:** none; implements R14.
- **Origin:** the simplicity-first design (§7.2, §7.4, §9.6); the token snapshot from the concurrency-first design (§5.9).
- **Context:** R14: one CLI entry point, one command family, and check whether a better default removes a concept, flag or setting. Every always-on tool and skill costs tokens every turn [plugin-packaging §Design implications].
- **Decision:** three tools (two non-deferred), four global flags plus `--base` on writes, no `--quiet`, `--verbose` or `--format`, one optional config file per workspace, plus a user-level one that alone holds credential settings [DR-48], with a default for every key, and a rule that a new setting needs a decision record saying why no default works. The only user-facing settings in v0.1 are extra pack paths, the model backends, pins and credentials, the spend limits, and `models = off`, a privacy and offline choice no default can make for the person. A snapshot test fails when non-deferred tool descriptions and schemas, skill and agent frontmatter and the mod's prompt section exceed 1,500 projected tokens, measured from the exact bytes the model receives [DR-51]. CLI help groups commands by the five ideas plus "Setup".
- **Alternatives rejected:** nine or eleven tools (the concurrency- and extensibility-first designs); an "Engine" concept heading (the concurrency-first design).
- **Consequences:** tests `s15_surface.rs`, `tests/plugin/surface_tokens.sh`; the configuration reference is generated from `defaults.toml`.
- **Amended in review round 1:** the always-on number is measured from the exact bytes, because `claude plugin details` may miss runtime-registered tools. It has sub-budgets of 900 for tools, 300 for the section and 300 for skills and agents, and tool inputs are flat to fit. CI gates envelope and tool-result tokens, replayed Claude usage and the S3 cache-hit floor; `budgets.json` `recorded` holds the S02 chat usage and the token ratio (D20; cost-5, cost-8) [DR-51]. The user-level config is a second file of the same keys, for credential settings only [DR-48]. `room_change` is auto-allowed for its ghost-only leaves (`edit`, `intent`, `claim`, `curate`), while `scan` and `decide --run` keep the permission prompt (principles-7).
- **Amended in review round 2:** `room_change` gains an optional `payload` input, passed on stdin to a leaf run with `--file -`, so a chat proposal needs no Write call and no prompt (principles-3) [DR-66]. Each skill body gets its own budget beside the always-on 1,500, since the questions' paths load them (cost-2) [DR-65].
- **Amended in review round 3:** `planroom --help` shows the five ideas and one line naming `planroom help --all`, which alone lists the Setup leaves, and `scan`, `open` and `share` join Levels, so a person meets no sixth group (ux-10) [DR-77].
- **Sources (checked 2026-10-25):** [plugin-packaging §Design implications] (about 40 tokens per short skill); candidate-simplicity §7.4; the design reviews.

## DR-23: Plan stages with their methods and structural lint

- **Status:** accepted. Amended in review round 1.
- **Starting point:** extended "Structural lint runs in code; semantic lint goes to Jev."
- **Origin:** the concurrency-first design (§6.1); a design review noted the simplicity-first design reduced R5's methods to a sentence.
- **Context:** R5 names the methods per stage (Jobs-to-be-Done and laddering, SMART and non-goals, MECE and the WBS 100% rule, GTD and HTN, MoSCoW and an MVP line, pre-mortem, lint), and S3's lint depends on them.
- **Decision:** the plan pack declares seven stages, each with its method, kinds and fields, structural lint rules (in kernel rule types) and semantic Jev kinds, as in [Architecture](architecture.md#planning-stages-ghosts-and-lint-r5). In chat, the planning partner proposes items itself as ghosts; with no chat, `intent next` asks Claude with the plan pack's one shared cached prefix and the stage's method in the tail. Version 1 is tagged when a project's first batch is accepted; later versions are a person's.
- **Alternatives rejected:** a generic "stage with a prompt" (the simplicity-first design).
- **Consequences:** contracts gain the `covers` rule type for the 100% rule; tests `s03_plan.rs`, `kernel/tests/lint.rs`.
- **Amended in review round 1:** stages declare the `kernel.item` types they write, with prop schemas, instead of plan-pack kinds [DR-45]. Pack `[[rules]]` lose `forbid_edge`: boundary rules are intended-layer edits only, so one violation has one report path (extensibility-11). every stage shares one plan-pack prefix, the planning rules and the union `PlanProposal` schema, with a 1-hour TTL. The stage's method goes in the uncached tail, so a plan's later stages read the cache. The budget test asserts a hit-rate floor of 0.6 after the first stage (cost-6, principles-6).
- **Sources (checked 2026-10-25):** KICKOFF R5; candidate-concurrency-agents §6.1; the design reviews.

## DR-24: Duties under leases instead of a jobs API

- **Status:** accepted. Amended in review rounds 1, 2 and 3.
- **Starting point:** none.
- **Origin:** the simplicity-first design's duties (§2.4); the concurrency-first design's "decisions off the scan's critical path, keyed so duplicate triggers join", which a design review proposed adding.
- **Context:** Jev takes about 300 to 600 ms per call at the client [jev §3]; The simplicity-first design ran decisions inline in a cold scan. Concurrent triggers shouldn't duplicate work. `http.fetch` has no timeout.
- **Decision:** background work (rescan on a moved head, the decision run, curation) is a duty that a live process runs only while holding its lease (`scan:REPO`, `decide:HASH`, `curate`), so duplicate triggers join one run. A scan never waits on Jev; with no live process, `planroom decide --run` does the work. The server replies within 10 s; a command still running at 8 s detaches into its duty and completes on the stream.
- **Alternatives rejected:** a jobs API with `job.wait` (the concurrency-first design).
- **Consequences:** contracts gain `duty` stream frames and the `{running}` reply; tests `duties.rs::duties_join_on_lease`, `s07_decisions.rs`.
- **Amended in review round 1:** an inline rescan takes the same `scan:REPO` lease. A refresh read never waits on it and returns the current snapshot with `meta.stale_repos`; any other command waits up to 5 s and reuses the holder's snapshot. Leases carry an epoch that the committing transaction checks, and snapshot inserts are idempotent (correctness-7, correctness-9). Test `duties.rs::an_expired_holder_commits_nothing`.
- **Amended in review round 2:** a lease records its holder's pid, process start time and host. A contender on that host takes over at once, with an epoch bump, when that process is gone, and `watch` releases its leases on `SIGTERM`. Presence is keyed by `(actor, surface, client_id)` and merged per actor only for display (correctness-12). Tests `duties.rs::a_dead_holders_lease_is_taken_over_at_once`, `stream.rs::presence_keeps_a_row_per_surface`.
- **Amended in review round 3:** the decision duty's lease is one `decide` per workspace, not one per request set, so two runs never ask Jev twice about a subject they share (correctness-9) [DR-73]. A `gc` duty prunes the caches after a scan (unasked-9) [DR-47]. Test `duties.rs::concurrent_decide_runs_ask_each_subject_once`. A `scan` event triggers one `decide` duty run, which spends that scan's Jev budget. Requests it leaves pending wait for the next scan or an explicit `decide --run`, so duty runs never chain (cost-5) [DR-65].
- **Sources (checked 2026-10-25):** [jev §3]; [mod-api §7]; the design reviews.

## DR-25: Engine-made clones, two implementers, a cross-repo merge preflight

- **Status:** accepted. Amended in review rounds 1, 2 and 3.
- **Starting point:** none; replaces the research suggestion of `planroom:implementer` registered with `isolation: 'worktree'` [mod-api §5].
- **Origin:** the simplicity-first design (§6.6, D17); the merge preflight from the extensibility-first design (§6.6).
- **Context:** S4's change spec touches `worker` and `platform` [TRUTH scenarios.S4.implementer_patches]; Claude Code's worktree isolation covers only the session's repo. A merge that fails in the second repo must not leave the first merged.
- **Decision:** the engine makes one worktree per touched repo on `planroom/KEY`. Two implementers share one interface: `agent` (`planroom:implementer`, spawned by the mod with `cwd` at the hand-off directory) and `patch` (applies the spec's patches with `git apply --3way`; the fixture implementer and curation). `handoff merge` is person-only and preflights every repo before merging any.
- **Alternatives rejected:** agent-type isolation; merging repo by repo with no preflight.
- **Consequences:** contracts `ChangeSpec`, `ImplementerRun`; tests `s04_handoff.rs`, `handoff_preflight.rs::a_conflict_in_the_second_repo_leaves_both_untouched`.
- **Amended in review round 1:** `handoff merge` commits a `merging` step with each repo's planned target before touching git, records each repo's result, then commits `merged`. A retry resumes from the step and skips a repo whose `HEAD` already contains `planroom/KEY` (correctness-10). Contracts gain `HandoffState::Merging`; test `handoff_preflight.rs::a_crash_between_repos_resumes_on_retry`. the hand-off directory, its spec and its worktrees live in `DATA_ROOT/handoffs/<workspace_id>/KEY/`, outside `.planroom/`, and the mod's guard keeps the implementer's Edit and Write inside its worktrees (D16, security-3) [DR-52]. `handoff revert` undoes a merge in every repo behind the same kind of preflight and resume, the `merging` step records each repo's head before the merge, and cancel or drop moves the branch to `refs/planroom/archive/KEY` (D15, principles-3) [DR-55].
- **Amended in review round 2:** `handoff merge` and `handoff revert` run under a `handoff:KEY` lease and key each step, so a retry resumes and a second invocation joins (E1) [DR-8]. The preflight also checks that each repo's `HEAD` is on the spec's base branch, and before each repo's merge the command checks that `HEAD` is still the recorded `before`. On a mismatch, or a merge that fails after the preflight, it runs `git merge --abort`, returns `handoff_conflict` and deals one card, "Revert worker's merge, or retry?", with revert pre-selected, and the hand-off loop gains `merging --> reverting`. Contracts gain `SpecRepo.base_branch` (correctness-11). Test `handoff_preflight.rs::a_commit_after_preflight_aborts_cleanly`. The engine makes a separate local clone per touched repo instead of a linked worktree, and `handoff collect` fetches only each clone's `HEAD` into `planroom/KEY` in the real repo, so the implementer's Bash can't move or delete the person's refs. The title changes to match (E15, security-2) [DR-64]. The mod runs `collect` right after `report`, and `collect` deals the review card (principles-7) [DR-66].
- **Amended in review round 3:** a hand-off also reaches `merged` when a scan finds its change on the tracked branch, landed by a pull request, a squash or a rebase (F20, unasked-6) [DR-80].
- **Sources (checked 2026-10-25):** [mod-api §5]; `examples/TRUTH.json` scenarios.S4; the design reviews.

## DR-26: The hand-off guard asks the person

- **Status:** accepted; A-2 settled. Amended in review round 1.
- **Starting point:** none; implements R8 "The planning partner in the chat never silently implements."
- **Origin:** the extensibility-first design denied the chat's own edits on the touch set (§6.6); the concurrency-first design only added a notice (§6.4); a design review objected that the extensibility-first design blocks edits the person asked for.
- **Context:** "never silently" is a guarantee, so a notice is too weak; blocking a person's explicit request costs attention.
- **Decision:** while a hand-off is started and not closed, the mod's `tool.check` hook stops the main thread's `Edit`, `Write`, `MultiEdit` and `NotebookEdit` on any path in its touch set and asks the person through the permission prompt, with one line naming the hand-off. If `tool.check` can't ask, it denies with that line and names `/room handoff cancel KEY`.
- **Alternatives rejected:** deny always (the extensibility-first design); notice only (the concurrency-first design).
- **Consequences:** tests `packages/mod/src/guard.test.ts`; S9 evidence shows the prompt once.
- **Amended in review round 1:** A-2 settled from the 2.1.310 types (`ToolCheckResult`): `tool.check` returns `ask` with the reason, and where nobody can be asked Claude Code turns it into a deny with that reason. So the guard returns one value, `ask`, whose line names `/room handoff cancel KEY` for the deny case.
- **Sources (checked 2026-10-25):** [mod-api §4, §8]; the design reviews (the idea from the extensibility-first design, and the objection).

## DR-27: Curation drafts: templates, then Claude, then the curator agent

- **Status:** accepted. Amended in review rounds 1 and 2.
- **Starting point:** none; implements R12.
- **Origin:** the simplicity-first design's templates (§6.8); the concurrency-first design's Claude drafting and its two CI paths (§6.5, §10.2).
- **Context:** S12 must pass deterministically; drafts should read well when a model is reachable.
- **Decision:** miners (gotchas, procedures, stale lines) produce template drafts. When a model is reachable, a cheap-tier Claude call refines each draft with the target file in its cached prefix; in a live session the hidden curator agent can rework drafts that span files. Every draft must mention the cluster's key tokens before it's proposed. CI runs S12 twice: with replayed Claude and with models off.
- **Alternatives rejected:** Claude-only drafts; template-only drafts.
- **Consequences:** contracts `CurationDraft`; tests `s12_curation.rs` (both paths).
- **Amended in review round 1:** curation mines runs from `handoff report` and from a person's import only; agent-imported runs are kept untrusted. Cards name their source runs and show in full any draft with a URL, command substitution, a shell pipe or `allowed-tools` (security-9) [DR-52]. The curator spawns only after a person's card or `/room curate`, and its usage goes into the ledger (cost-10) [DR-51].
- **Amended in review round 2:** a refinement call caches the shared curation prompt and puts the target file in the uncached tail, with the 5-minute TTL, since no two drafts share a target (cost-7) [DR-65]. The curator submits drafts on stdin with `edit propose --file -`, and the guard allows that and `curate --json` with no prompt (principles-3) [DR-66].
- **Sources (checked 2026-10-25):** `examples/TRUTH.json` scenarios.S12; the design reviews.

## DR-28: Engine bootstrap without a SessionStart hook

- **Status:** accepted. Amended in review rounds 1 and 2.
- **Starting point:** kept and refined "plugin/ ... engine bootstrap."
- **Origin:** the simplicity-first design (§11.2, D20); the source-build card from the extensibility-first design (§11.2).
- **Context:** plugins can't hold a `bin/` folder; mixing `modules` with classic hooks in one `hooks.json` is unverified; an interactive SessionStart hook holds Claude's first reply [plugin-packaging §3].
- **Decision:** `plugin/scripts/planroom` resolves `$PLANROOM_ENGINE`, the dev checkout found from the script's own path, the cache under the data root, then a sha256-verified download for the fixed tag, then a source build after a person's card. The mod starts `prefetch --detach` in `session.start`; if the fetch fails and the toolchain preflight passes, it deals one card, "Build the engine from source, about 4 minutes?", with progress in the status line.
- **Alternatives rejected:** a SessionStart prefetch hook; `releases/latest` (403 through the proxy).
- **Consequences:** tests `tests/installer/run.sh`; release writes `plugin/scripts/engine.env`.
- **Amended in review round 1:** the resolver uses no plugin variable, since the Bash tool sets none: it finds `engine.env` and the dev checkout from its own path, and its cache is `DATA_ROOT/engine/VERSION/`, shared by the mod, the Bash tool and agents. The source build runs from a checkout when the plugin is loaded from one, preflights cargo, a C compiler and the network, runs detached with a log and a pid file, and says so when the release tag doesn't exist yet. A stale fetch lock breaks (D16; operability-1, operability-3, unasked-8) [DR-58].
- **Amended in review round 2:** `cargo xtask gen` writes `engine.env`, and `cargo xtask gen --version` its version, in the commit that bumps it; nothing writes it at release (operability-10) [DR-69].
- **Sources (checked 2026-10-25):** [plugin-packaging §3, Design implications]; [iris-release-bar §3]; the design reviews.

## DR-29: Browser login with a one-time code

- **Status:** accepted. Amended in review rounds 1, 2 and 3.
- **Starting point:** extended "A single-writer local server per workspace" (its exposure).
- **Origin:** the concurrency-first design (§12.2); a design review flagged the simplicity-first design's `?t=` query-string token.
- **Context:** a token in a query string lands in logs and history.
- **Decision:** `planroom open` (person only) makes a 128-bit one-time code, registers its sha256 with `serve` through the control token, and opens the URL with the code in the fragment in the browser itself; it never prints the code. The code works once, within 60 s, for that `serve` process. The page exchanges it for a random bearer token, kept in the tab's memory and sent as `Authorization: Bearer`, the stream included, which the canvas reads through `fetch`. Every API request needs that token, an exact `Origin` (a missing one is refused) and a matching `Host`; no CORS; strict CSP. `.planroom/` is 0700 and its files 0600.
- **Alternatives rejected:** a bearer token in the query string (the simplicity-first design); an `HttpOnly`, `SameSite=Strict` cookie, the first design, which a browser sends to every other server on `127.0.0.1`, since cookies ignore the port and every local port is one site.
- **Consequences:** tests `crates/planroom/tests/server_auth.rs` (no token, a missing or foreign `Origin`, a wrong `Host`, a reused or expired code); `store/tests/fs_check.rs::store_files_are_0600_in_a_0700_directory`. Users: a reload of the tab asks for a new code, one `e` press.
- **Amended in review round 1:** an in-memory bearer token sent as a header replaces the cookie, and every request needs an exact `Origin` (D17, security-7). The code has 128 bits and a 60 s life, reaches `serve` through the control token, and is never printed (security-3). `.planroom/` is 0700 and its files 0600.
- **Amended in review round 2:** `open` writes the URL into a 0600 redirect page under `DATA_ROOT/open/` and opens that file, so the code is in no process's argv, and a second try at a code revokes the token the first exchange issued (E15, security-4) [DR-64]. The tab keeps its bearer token in `sessionStorage`, scoped to its origin and port, so a reload within one `serve` stays logged in. `serve` prefers the port its last `serve.json` recorded, so the origin and the tab's storage survive a restart. A tab with no valid token shows the engine-authored "This tab needs a fresh link" state with its pending edits counted, which the empty-state audit visits (E10, principles-4) [DR-8]. The consequence that a reload asks for a new code no longer holds. Tests `server_auth.rs::the_code_is_never_in_an_argv`, `a_code_exchanged_twice_revokes_its_token`, `serve_prefers_its_last_port`. Where no browser can be seen (no display on Linux, an SSH session, a container host, or an opener that fails), `open` opens nothing and returns `opened: false` with `/room share` as the next action, and on WSL it opens through `wslview` (ux-4, unasked-7).
- **Amended in review round 3:** `serve` never moves off its workspace's port. `workspaces.json` gives each workspace its own, another program holding it is `port_in_use`, and `serve --port N` is the deliberate move, so a tab's pending edits never strand in another origin. `server_auth.rs::serve_port_moves_only_when_asked` replaces `serve_prefers_its_last_port` (F8, unasked-7) [DR-78].
- **Sources (checked 2026-10-25):** candidate-concurrency-agents §12.2; the design reviews.

## DR-30: Skills hidden from the slash menu

- **Status:** accepted; A-1 settled. Amended in review round 1.
- **Starting point:** none; implements R14 and S15 "one command family."
- **Origin:** a fatal flaw the design reviews found in all three candidate designs.
- **Context:** plugin skills appear as `/planroom:<name>` slash commands [plugin-packaging §3], outside the `/room` family.
- **Decision:** three skills (`room`, `room-plan`, `room-handoff`), each with `user-invocable: false` in frontmatter, so the model can load them and the slash menu doesn't list them. The S15 snapshot asserts the headless session's `slash_commands` list holds only `/room` from Planroom. If `user-invocable: false` isn't honored on Claude Code 2.1.310, ship one skill named `room`, whose `/planroom:room` entry is the same family.
- **Alternatives rejected:** four or five visible skills (all candidates).
- **Consequences:** tests `tests/plugin/slash_commands_snapshot.sh` (evidence path, needs a session) and a static frontmatter check in CI.
- **Amended in review round 1:** A-1 settled by a live spike (headless `claude -p` with a spike plugin): a skill with `user-invocable: false` is absent from `slash_commands` and the init `skills` list, and the model still invokes it with the Skill tool. The slash-command snapshot runs only on S15's evidence path, and S15's CI path has the static check `s15_surface.rs::skills_are_not_user_invocable` (D14, repo-7) [DR-59].
- **Sources (checked 2026-10-25):** [plugin-packaging §3]; the design reviews (a fatal flaw in all three).

## DR-31: Remote mode: a published snapshot, a small live head, an attributed inbox

- **Status:** accepted, verify in WS7 and WS8. Amended in review rounds 1, 2 and 3.
- **Starting point:** kept and refined "Remote mode: the workspace is the source of truth, the session pushes snapshots to the Artifact db; Artifact edits land as events in a db collection; the session imports them on `/room open`, at turn start, and on edit notices; the Artifact shows cached decisions and queues new ones."
- **Origin:** the simplicity-first design (§7.6); snippets excluded by default and the first-publish summary from the concurrency-first design (§12.5); the concurrency-first design's timer loop rejected after a design review.
- **Context:** `db` writes are last-writer-wins page-side, need `if_version` session-side, and don't wake the session; bulk data fits a published data file; a `{self}` path proves the author [artifact §2, §6; artifact-spike].
- **Decision:** the bulk model ships as `data/snapshot.json` (code text excluded by default); a small `live/head` document carries the top cards and recent changes; viewers write only `inbox/{self}/events/{docId}`, and only the owner reads `inbox`. A person's `planroom import` commits other viewers' rows as attributed person edits through the same compare-and-set. It turns the owner's own rows and person-only rows into one card [DR-52]. Sync runs on `/room sync`, `/room` opening, the page's Send to Claude, and a hint in the per-turn context; a wake carries only "sync requested". The mod never calls `ArtifactData` from a timer and never auto-allows it.
- **Alternatives rejected:** all data in `db` shards (25,000-document cap, many batches); a 5 s mod timer auto-allowed through `tool.check` (the concurrency-first design).
- **Consequences:** contracts `Snapshot`, `LiveHead`, `InboxRow`; tests `s11_remote.rs`, `tests/e2e/artifact.spec.ts` with a fake `db` that has the real rules.
- **Amended in review round 1:** Importing inbox rows is person-only and in no tool, and the owner's rows and person-only rows become one confirming card (D15; security-2, principles-1) [DR-52]. Only the owner reads `inbox`. A wake carries only "sync requested", and viewer text is data. The first-publish line comes from the snapshot's sections, and presence is left out unless `--include-presence` is passed (security-8). The sync hint is per-turn context, never the composed section (principles-9). The page logs each `sample` call for the ledger (cost-2) [DR-51]. The snapshot names what it serves and carries the empty state for the rest; `live/head` carries the data delta since the snapshot, which the page applies; the page marks its rows queued until `acks/<uid>` lists them; the manifest declares `comments` for Send to Claude; and an inbox row may name only `edit`, `review`, `decide` or `intent propose` (D15; ux-5, ux-7, ux-1) [DR-56].
- **Amended in review round 2:** an inbox row carries the page's per-load `instance` and a per-instance `counter` in place of `seq`. The client is `artifact:<uid>:<instance>`, and import applies rows in `(instance, counter)` order (E4, correctness-7) [DR-61]. Import takes rows on stdin from the mod, from the `ArtifactData list` result it observed, and a paired owner device's signed rows apply directly (E8; security-3, principles-8) [DR-67].
- **Amended in review round 3:** bare `/room` starts no sync; in a cloud session with remote mode on, each turn start syncs with no person token; and a page row may be a read request, which any sync answers into `answers/<docId>` (F10, ux-1, ux-5) [DR-77].
- **Sources (checked 2026-10-25):** [artifact §2, §5, §6]; [artifact-spike]; the design reviews (a fatal flaw in the concurrency-first design's timer; snippets).

## DR-32: The S8 concurrency harness

- **Status:** accepted. Amended in review rounds 1 and 2.
- **Starting point:** none; implements S8.
- **Origin:** the concurrency-first design (§10.3).
- **Context:** S8 needs proof of no lost writes, idempotent retries, live updates within 1 s, claims and presence, and a stale write that becomes a diff, not a merge.
- **Decision:** two agent CLI writers on two projects, an HTTP writer as a canvas tab plus an SSE reader, 10% of writes killed after commit and before the reply and retried with the same key, 5% duplicated on purpose, a claim, and one field renamed by agent B (a ghost), then by a mod press, then by the tab from a base read before the press. Live latency is measured from commit to rendered: the tab's `data-seq` marker and the pane's `seq`. Faults are compiled only in test builds (`PLANROOM_TEST_FAULTS`). A `writer_interleavings` proptest checks the same invariants in-process; `incremental_fold_equals_full_fold` checks `serve`'s fold.
- **Consequences:** layout `crates/testkit/src/load.rs`; tests `s08_concurrency.rs`.
- **Amended in review round 1:** S8's stale write is two person writer instances, with B's ghost superseded (D4, correctness-1), and the latency gate is commit to rendered (D21, correctness-7).
- **Amended in review round 2:** S8's agent writers commit constantly, which would hide a process that misses its own commits. So `stream.rs` adds two SSE clients on one `serve` with no other writer, and a kernel proptest checks the incremental fold outside S8 (correctness-2, correctness-6) [DR-1, DR-61].
- **Sources (checked 2026-10-25):** candidate-concurrency-agents §10.3; the design reviews.

## DR-33: Freshness on read instead of a file watcher

- **Status:** accepted. Amended in review rounds 1 and 2.
- **Starting point:** replaced "`notify`."
- **Origin:** the simplicity-first design (D8).
- **Context:** scans are of committed state; `notify` on macOS was not built or tried [rust-stack G13].
- **Decision:** each command compares each repo's `HEAD` and refs with its active snapshot and rescans what moved; live processes poll heads every second. A refresh read (`focus` and every surface's refetch) never rescans inline while a live process holds the repo's `scan:REPO` lease. Surfaces refresh on the first frame of a burst, after quiet, and at a maximum wait of 300 ms (canvas) or 500 ms (mod).
- **Alternatives rejected:** `notify` 8.2 with `notify-debouncer-full` (the concurrency-first design).
- **Consequences:** S7's commit-triggered rescan is a head change seen by the poll or the next command.
- **Amended in review round 1:** leading-plus-trailing refresh with a maximum wait, and no inline rescan in refresh reads (D21, correctness-7).
- **Amended in review round 2:** freshness follows each repo's tracked branch, and `focus` and `map` also report the repos whose working tree has uncommitted changes, with no rescan (E17, unasked-12) [DR-71].
- **Sources (checked 2026-10-25):** [rust-stack §2, G13]; candidate-simplicity §15 D8.

## DR-34: No per-repo export directory

- **Status:** accepted. Amended in review round 1.
- **Starting point:** replaced "A deterministic `.planroom/` export per repo (intended layer, overrides, plans, views), reviewable in PRs."
- **Origin:** the simplicity-first design (D3).
- **Context:** no requirement asks for per-repo files; two sources of truth need their own sync and merge rules.
- **Decision:** the workspace store is the one source; `planroom export` writes events as NDJSON for upgrade, and the engine appends the same format to a backup under the data root after each person-authored commit [DR-53]. `.planroom/` holds only the store and local state, with a `.gitignore` of `*`.
- **Alternatives rejected:** The concurrency-first design's export with git-author import.
- **Consequences:** teammates converge through the Artifact and hand-offs in v0.1; a shared export is a later feature with its own requirement.
- **Amended in review round 1:** with no per-repo export, nothing else held a person's events, so `git clean -xfd` lost them. An automatic, append-only backup under the data root and a restore card close that, without a second source of truth: the backup is read only to restore (D6, principles-4) [DR-53].
- **Sources (checked 2026-10-25):** candidate-simplicity §15 D3; the design reviews (the concurrency-first design's export is a second source of truth).

## DR-35: Crates and packages

- **Status:** accepted.
- **Starting point:** replaced the repo layout sketch `planroom-schema · -kernel · -store · -extract-* · -packs · -decide · -server · -cli` and `packages/ schema · canvas · mod · ui`.
- **Origin:** the simplicity-first design (D12, D13); `planroom-testkit` and `@planroom/ui` as a view-model package from the concurrency-first design.
- **Context:** R16 wants boundaries enforced by tooling; fewer crates make the rule one sentence. `docs/contributing/design-language.md` puts the tokens in `packages/ui`.
- **Decision:** crates `planroom-contract`, `-kernel`, `-store`, `-extract`, `-models`, `planroom` (the binary and composition root), `planroom-testkit` (dev-only), plus `xtask`. Packages `@planroom/contract` (generated only), `@planroom/ui` (design tokens, view-model rows, hotkeys, braille raster, the machine stepper; no React, DOM or Node), `@planroom/canvas`, `@planroom/mod`.
- **Alternatives rejected:** a crate per extractor tier or pack (the extensibility-first design's 27 crates); a hand-written theme inside the generated contract package (the simplicity-first design).
- **Consequences:** [Repo layout](repo-layout.md); `same_output` works because the mod and canvas share `@planroom/ui`.
- **Sources (checked 2026-10-25):** candidate-simplicity §9; candidate-concurrency-agents §9.3; `docs/contributing/design-language.md`.

## DR-36: One boundaries file for both languages

- **Status:** accepted. Amended in review rounds 1 and 2.
- **Starting point:** kept and combined "a crate-graph test or `cargo-deny` bans in Rust, and `dependency-cruiser` or `eslint-plugin-boundaries` in TypeScript."
- **Origin:** the concurrency-first design (§9.3) for one source; the simplicity-first design (§9.3) for the kernel allow-list, the pack-literal scan, the three-way self-test and the decision-record gate.
- **Decision:** `boundaries.toml` is the single source. `cargo xtask check boundaries` reads it with `cargo metadata`; `cargo xtask gen` writes `.dependency-cruiser.cjs` from it. Also: the kernel's external-dependency allow-list, a scan that no string literal in `crates/kernel/src` equals a built-in pack kind or relation name, with or without its `<pack>.` prefix, and a self-test that plants a Rust, a TypeScript and a pack crossing in a temporary copy and expects each to fail.
- **Alternatives rejected:** `eslint-plugin-boundaries` (missed 1 of 5 planted imports, mid-migration [ts-stack §6.1]); two hand-kept configs.
- **Amended in review round 1:** the literal scan matches bare kind and relation names too, and the self-test plants a bare relation name (D10, extensibility-1) [DR-45]. The self-test installs JS dependencies offline in its copy, runs every check on the unmodified copy as a control, and plants a declared `workspace:*` crossing and a relative import, expecting exactly the rule's name and no unresolvable import. It also plants a layout change without a record (D14, repo-2) [DR-59].
- **Amended in review round 2:** `boundaries.toml` also holds the kernel's no-I/O list as item paths and each TypeScript package's `third_party` allow-list, so the generated checks allow only what it names (repo-8, repo-9) [DR-70].
- **Sources (checked 2026-10-25):** [ts-stack §6.1]; [rust-stack §10]; the design reviews.

## DR-37: One schema in two views, one version

- **Status:** accepted. Amended in review round 3.
- **Starting point:** kept and refined "`planroom-schema` (contract source)" and "a schema-to-TS generator (`json-schema-to-typescript`, `ts-rs` or `specta`)."
- **Origin:** the simplicity-first design (§8.1, D18).
- **Context:** principle 2 asks for one versioned schema. schemars makes `#[serde(default)]` and `Option` fields optional in the default view [rust-stack G7].
- **Decision:** Rust types in `planroom-contract` are the source. `cargo xtask gen` writes `schema/planroom.v1.schema.json` (serialize view) and `schema/planroom.v1.input.schema.json` (deserialize view) with schemars 1.2.2, then `json-schema-to-typescript` 16.0.0 writes `packages/contract/src/generated/`. One integer `schema_version = 1` covers the whole contract; persisted documents also carry `format: "planroom.<name>/v1"`. Open sets use `anyOf: [{enum}, {pattern}]`.
- **Alternatives rejected:** one schema per contract with its own major (the extensibility-first design); ts-rs (`u64` as `bigint`); specta (a release candidate); schema-first with typify (draft-07 internals) [rust-stack §9].
- **Amended in review round 3:** the resolver's outputs are contract types too. `ResolverStatus` and the resolver's two envelopes are `$defs`, and `cargo xtask gen` writes `plugin/scripts/contract.sh`, which the POSIX sh resolver sources, so it spells no code and can't drift from the schema (F14, principles-1) [DR-69].
- **Sources (checked 2026-10-25):** [rust-stack §9, G7, G8]; [iris-release-bar §6]; the design reviews (a fatal flaw in the extensibility-first design).

## DR-38: The xtask runner is the one task entry point

- **Status:** accepted. Amended in review rounds 1 and 2.
- **Starting point:** kept "`justfile` (or `cargo xtask`)", choosing xtask.
- **Context:** `just` isn't installed [progress.md A2]; CI and agents must run the same commands.
- **Decision:** `cargo xtask {check, test, e2e, gen, release-dry-run, fixture, eval, live}`; nothing else is documented for agents. It never stops at the first failure and prints one line per check, like Iris's `check.sh`.
- **Alternatives rejected:** `just` (extra install); a `scripts/check.sh` (a second language for task logic).
- **Amended in review round 1:** every `cargo xtask` command sets `PLANROOM_REPLAY=strict`, `PLANROOM_SPEND_LOG=docs/evidence/spend.jsonl`, and for tests a temporary `PLANROOM_DATA`, and this repo's `.claude/settings.json` sets the first two for agents (D8, D20; cost-1, cost-2) [DR-49, DR-51]. `cargo xtask fixture` records its fixture as a trusted workspace [DR-48]. CI runs only `cargo xtask` commands, and `ci_runs_only_xtask` fails on any other `run:` step except a listed setup step. `cargo deny check` (licenses, bans, sources) joins `check`; advisories and `pnpm audit` become `check audit`; the installer suite becomes `test --installer`; the guardrails job runs `check guardrails`. Every command first installs JS dependencies from the lockfile, offline when it can (D14; repo-3, repo-11) [DR-59].
- **Amended in review round 2:** `cargo xtask` sets `PLANROOM_SPEND_LOG` to the absolute path of the main checkout's `docs/evidence/spend.jsonl`, through `git rev-parse --git-common-dir`, and passes it into nested evidence sessions; `.claude/settings.json` sets only `PLANROOM_REPLAY=strict` (cost-1) [DR-65]. The three release helpers in `scripts/` stay POSIX sh: `release.yml`'s `verify` job runs before any Rust toolchain is installed, the resolver suite already needs sh, and `cargo xtask` calls them locally, so CI keeps one entry point (repo-5). `cargo xtask test` takes `-p CRATE`, a test-name filter and `--ts PATH`, and plain `cargo xtask gen` writes every committed output (repo-10) [DR-70].
- **Sources (checked 2026-10-25):** [iris-release-bar §7]; `progress.md` A2.

## DR-39: Rust library choices

- **Status:** accepted.
- **Starting point:** kept mostly "Libraries to evaluate first: Rust."
- **Decision and rejected alternatives:**

| Need | Choice (exact pin) | Rejected | Why |
|---|---|---|---|
| Parsing | `tree-sitter =0.27.1`, `tree-sitter-language =0.1.9`, grammars TypeScript 0.23.2, JavaScript 0.25.0, Python 0.25.0, Go 0.25.0, Rust 0.24.2 | `syn`, `swc`, `oxc`, language servers | one runtime for all six grammars; 690 files/s; ABI floor test for TypeScript's ABI 14 |
| Graph algorithms | `petgraph =0.8.3` | hand-rolled adjacency | toposort, paths, SCC |
| SQLite | `rusqlite =0.40.2` (`bundled`) | `sqlx 0.9.0` | synchronous single writer; 12 s build against 48 s; the two can't share a workspace (G1) |
| HTTP server | `axum =0.8.9`, `tokio =1.54.1`, `tower-http =0.6.11`, `tokio-stream =0.1.19` | actix-web, warp | SSE and static files verified; one `tower-http` copy with reqwest |
| HTTP client | `reqwest =0.13.5` with `rustls` and the ring provider | hyper directly, ureq | no OpenSSL; ring builds 22 s faster |
| Serialization and schema | `serde =1.0.229`, `serde_json =1.0.154` (`preserve_order`), `schemars =1.2.2` (`preserve_order`) | schemars 0.8 | draft 2020-12, follows serde attributes |
| CLI | `clap =4.6.7` (derive) | argh, bpaf | |
| Diffs | `similar =3.2.1` | `diff`, `dissimilar` | text hunks and ratios |
| Logging | `tracing =0.1.44`, `tracing-subscriber =0.3.23` | log | stderr only |
| Walk and globs | `ignore =0.4.33`, `globset =0.4.20` | walkdir | respects `.gitignore` |
| Regex | `regex` (linear time) | fancy-regex | a pack can't stall a scan |
| YAML | `saphyr-parser =0.1.0`, with markers [A, WS3] | `serde_yaml` (unmaintained) | line markers for evidence |
| Markdown | `pulldown-cmark` | comrak | |
| PromQL | `promql-parser` | hand-rolled selectors | `job` and metric matchers |
| Fuzzy search | `nucleo-matcher` | | |
| Hashes and IDs | `sha2 =0.11.0`, `uuid =1.28.0` (`v7`) | blake3 (C build), ulid 3.0 (renamed API) | |
| Errors | `thiserror =2.0.21` in libraries, `anyhow =1.0.104` in the binary and tests | snafu | |
| Tests | `insta =1.49.0`, `assert_cmd =2.2.2`, `tempfile =3.27.0`, `proptest`, `jsonschema =0.58.6`, `wiremock =0.6.5` | expect-test | |
| CI tools | `cargo-deny 0.20.2`, `cargo-nextest 0.9.149`, `cargo-about 0.9.2`, downloaded with pinned sha256 | `cargo install` (2 min 30 s) | |
| Git | the git CLI (DR-16) | `gix =0.90.0`, `git2 =0.21.0` | |
| MCP | none (DR-14) | `rmcp =3.7.0` | |
| File watching | none (DR-33) | `notify =8.2.1` | |

- **Cool-off.** Several pins were released within 14 days of 2026-10-25 (rusqlite 0.41.0, tokio 1.54.1, uuid 1.29.0, schemars 1.2.3, sha2 0.11.1, similar 3.2.1, jsonschema 0.60.1). **Decided in WS1 (2026-10-26):** every pin is a version at least 14 days old. WS1 pinned `schemars =1.2.2` and `jsonschema =0.58.6`; later slices pin the others the same way when they first add them, and record the version here. WS3 and WS4 pinned `rusqlite =0.40.2`, `sha2 =0.11.0`, `uuid =1.28.0` and `saphyr-parser =0.1.0` (DR-81).
- **Consequences:** `deny.toml` allows MIT, Apache-2.0, Unicode-3.0, Unlicense, ISC, Zlib, BSD, CC0-1.0, MIT-0, BSL-1.0, CDLA-Permissive-2.0 and MPL-2.0; bans `openssl` and `openssl-sys`; `[profile.dev.package."*"] opt-level = 2` for tree-sitter speed (G4).
- **Sources (checked 2026-10-25):** [rust-stack §1, §2, §5, §6, §9, §10, §11].

## DR-40: TypeScript library choices

- **Status:** accepted. Amended in review round 1.
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
| Language and lint | `typescript 6.0.3`, `eslint 10.12.0`, `typescript-eslint 8.71.1` (both pinned in WS1: at least 14 days old), `eslint-plugin-react-hooks 7.1.1` | TypeScript 7.0.2 (no typescript-eslint support), ESLint 9 (deprecated) | |
| Format | `prettier 3.9.10` | Biome 2.5 and 2.6 | Markdown and YAML support too young |
| Boundaries | `dependency-cruiser 18.6.0` | `eslint-plugin-boundaries 7.x` | DR-36 |
| Tests | `vitest 5.0.3` (pinned in WS1: at least 14 days old), `@playwright/test 1.63.0` | | `cargo xtask e2e` installs the matching Chromium |
| Mod bundle | `esbuild` (ESM, no splitting) | Vite for the mod | the mod can't use `import()` [mod-api §10] |
| Fonts | `@fontsource-variable/*` 5.3.0: Bricolage Grotesque, Atkinson Hyperlegible Next; JetBrains Mono | Inter, Roboto, Arial, system-ui, Space Grotesk | design language |
| Statecharts | none (DR-11) | `xstate 5.33.3` | |

- pnpm 10.28.2 with `minimumReleaseAge: 10080` (7 days).
- **Amended in review round 1:** CI runners have no preinstalled browsers, so `cargo xtask e2e` runs `playwright install chromium` at the pinned version, with `--with-deps` in CI. pnpm is pinned exactly in `packageManager` (10.28.2, as WS1 committed it), and Dependabot waits 7 days like `minimumReleaseAge` (operability-10, operability-12, repo-11) [DR-59].
- **Sources (checked 2026-10-25):** [ts-stack §1, §2, §6, §7, §8]; [mod-api §1, §10].

## DR-41: Jev adapter and pins

- **Status:** accepted.
- **Starting point:** kept "Jev policy" and the constraint "Jev ... behind one adapter that supports both the Vercel AI Gateway and direct TypeSafe."
- **Decision:** one `JevBackend` trait and one normalized model with two codecs: `Gateway` (`/v1/evaluate`, `typesafe-ai/jev-1.14.0`) and `TypeSafe` (`/v1/systemone` with `noul`, on the direct API or the gateway's `/typesafe` path; `jev-1.14.0` if the docs list it, else `jev-latest` with `model_reported` recorded). One request per slice, many questions per request, never several slices per state. Retries only on 429, 529, 5xx and transport errors. Both codecs pass one contract test over fixtures built from the docs and the Phase 1 recordings. A key-gated smoke test records the direct API's reported version.
- **Alternatives rejected:** the unpinned `typesafe-ai/jev` alias (reports no version, `zdr: "some"`); gateway model fallbacks for escalation (escalation goes through Planroom's own Claude adapter so it's recorded and budgeted); the AI SDK (no Rust SDK).
- **Sources (checked 2026-10-25):** [jev §1, §2, §7]; KICKOFF `<verified_facts>`.

## DR-42: Levels, IDs and fact merging

- **Status:** accepted. Amended in review rounds 1 and 3.
- **Starting point:** kept "Levels. C4 (System, Container, Component, Code) with Intent on top. One parent per node, and edges roll up with counts" and "Stable, namespaced IDs such as `<repo>:<kind>:<key>`"; kept "`layer` is actual or intended."
- **Decision:** five fixed levels; one parent; roll-ups with counts, an evidence sample and child handles, with faithfulness as a tested invariant. IDs `<namespace>:<kind-short>:<key>`, where kind-short is the kind's declared `id_prefix`, owned by one pack [DR-46]; edge IDs from a hash of kind, ends and `via`. When two extractors emit the same node ID, evidence unions and scalar fields take the more specific tier's value.
- **Consequences:** tests `kernel/tests/rollup.rs` (property), `kernel/tests/fold.rs::fact_merge_by_tier`.
- **Amended in review round 1:** kind-short is defined as `KindDecl.id_prefix`; two packs may not declare one prefix (extensibility-8).
- **Amended in review round 3:** roll-up lifts each end to its nearest ancestor at or above the level. When both ends meet one ancestor, the edge is that node's `internal`; only an end whose ancestor holds the other end steps down one child on its own path. So the lift rule and the internal count agree (correctness-6). Test `kernel/tests/rollup.rs::same_ancestor_edges_are_internal`. A repo's namespace is its directory name, suffixed for the later of two same-named repos, and the renames git reports carry a vanished target (F5; unasked-1, unasked-2) [DR-74].
- **Sources (checked 2026-10-25):** candidate-simplicity §3.3, §3.4; `examples/TRUTH.json` about (component granularity).

## DR-43: How S13's 1 s bar is measured in CI

- **Status:** accepted (decided 2026-10-26). Amended in review round 1.
- **Starting point:** none; implements S13 "The canvas is interactive in under 1 s on the fixture."
- **Context:** shared CI runners vary. The concurrency-first design proposed asserting 1.5 s in CI, which a design review found a weakening without the KICKOFF process. The combined design first proposed the median of 5 cold loads.
- **Decision:** CI asserts that every one of 5 cold loads is interactive in under 1 s, records every value in the test output, and fails on any console error. A load is retried only when the runner itself fails (the browser or server didn't start), never because it was slow. A median would let two slow loads pass, which reads as a weakening of "interactive in under 1 s".
- **Alternatives rejected:** a 1.5 s CI bar (a weakening); the median of 5 (lets slow loads pass); a single load (one sample says little about the bar).
- **Consequences:** the canvas must stay well under the bar, not just near it: persisted layouts, folded first views and a small first-paint bundle (about 215 KB gzip measured) are requirements, not optimizations. If shared runners make the gate flaky, the fix is in the canvas or the harness, never the threshold.
- **Amended in review round 1:** every wall-clock gate, this one, the cold scan, the fold benchmark and S8's commit to rendered, runs only on ubuntu-24.04 and writes every sample to the job summary. On macos-15, `cargo xtask test` asserts the same functional invariants and records the timings without gating them. No threshold changes (operability-11).
- **Sources (checked 2026-10-25):** [ts-stack §3.4] (0.31 to 0.45 s at 200 nodes, 1.4 to 1.6 s at 4× throttling); the design reviews (a fatal flaw in the concurrency-first design).

## DR-44: The reconciler: non-merge change units, attributed fact deltas, a persisted outcome

- **Status:** accepted (decided in review round 1, D5). Amended in review rounds 2 and 3.
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
- **Amended in review round 2:** the outcome also records the link snapshot its delta ran to, and the scan reconciles inside its committing transaction against the link it just built (E3, correctness-3) [DR-12]. With no outcome yet, the first scan records a baseline that classifies no change units (correctness-10, in part). Test `kernel/tests/reconcile.rs::the_first_scan_is_a_baseline`. The window follows each repo's tracked branch: only a fast-forward move of it is reconciled, and any other head move re-anchors the map and skips the reconciler with no card (E17, unasked-2) [DR-71].
- **Amended in review round 3:** an item is done when its rules hold on the tracked branch and its hand-off reached it, however the change got there, and the commits that carried it are the hand-off's units, never residuals (F20, unasked-6) [DR-80].
- **Sources (checked 2026-10-26):** `examples/TRUTH.json` scenarios.S4; `examples/fixture/repos/worker/10-audit-rows/COMMIT.toml`; review round 1 correctness-3 and owner decision D5.

## DR-45: Intent is kernel vocabulary

- **Status:** accepted (decided in review round 1, D10). Amended in review rounds 2 and 3.
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
- **Amended in review round 2:** the domain-free scan also covers `crates/contract/src`, the kernel's one workspace dependency, so a constant there can't carry a pack name into the kernel. The self-test plants one, and the contract's area contract forbids such literals (E14, repo-3).
- **Amended in review round 3:** the domain-free scan matches every built-in kind and relation name with its `<pack>.` prefix, and a bare name unless `boundaries.toml`'s `kernel_vocabulary` list holds it. That list names the words the kernel and the contract must spell, such as `person`, `repo`, `file`, `commit` and `state`, so Actor parsing passes while `"watches"` still fails, and the self-test plants both (extensibility-7). `packages/ui`, `canvas` and `mod` get the same scan, since they style from `MapNode.tone`, never a kind ID (extensibility-6) [DR-46].
- **Sources (checked 2026-10-26):** KICKOFF R11, R14, R16; review round 1 extensibility-1 and owner decision D10.

## DR-46: Pack data tables and typed emits

- **Status:** accepted (decided in review round 1, D11, D13 and D21). Amended in review rounds 2 and 3.
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
- **Amended in review round 2:** every Container-level kind uses the kernel's prefix `container`, so a pack that only adds an image role re-IDs no container (extensibility-4). Merged tables refuse a duplicate key, overlapping globs take the first match in load order, two default lenses are `pack_invalid` unless one is the code pack's, and `Tier` gains `Precise` for the SCIP reader DR-11 deferred (extensibility-6, in part). A reference in `from`, `to`, `parent` or `scope` takes the same forms, and a bare path such as `$file` is `pack_invalid`: templates write `@id:code.file:$file` (extensibility-8). Tests `packs.rs::a_pack_that_only_adds_an_image_role_keeps_every_container_id`, `duplicate_table_keys_are_pack_invalid`, `two_default_lenses_are_pack_invalid`; `extract/tests/contract.rs::a_bare_path_reference_is_pack_invalid`.
- **Amended in review round 3:** `KindDecl.tone` (`service`, `library`, `store` or `infra`) picks a Container-level kind's block token, and `MapNode` carries it, so a surface styles an out-of-tree kind without naming a kind ID. A palette for lens yarn stays a known gap (extensibility-6, in part). Test `packages/ui/src/tokens.test.ts::every_tone_has_a_token`.
- **Sources (checked 2026-10-26):** `examples/TRUTH.json` repos, containers, questions.unmonitored and scenarios.S10; review round 1 findings above and owner decisions D11, D13 and D21.

## DR-47: Deterministic scan bounds and a per-blob fact cache

- **Status:** accepted (decided in review round 1, D21). Amended in review rounds 2 and 3, and by DR-85, DR-86 and DR-100.
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
- **Amended in review round 2:** the `git` extractor reads at most the last 5,000 commits of the repo's tracked branch and reports older history as `partial_scan`, and the mod's tool handler runs `scan` with `process.run`'s 10-minute timeout (unasked-3, downgraded). Committing each repo's snapshot as it finishes, and a generated large-repo test, are known gaps.
- **Amended in review round 3:** a `gc` duty, under its lease after a scan, prunes `blob_facts` rows of other pack sets and `model_cache` rows of models no config pins. `budgets.json` gates a second fold benchmark at 50,000 events, since every one-shot read folds every event. Events are never pruned (unasked-9).
- **Amended by DR-85:** the bounds apply to `HEAD`'s tree, which the walk lists with `git ls-tree` and reads with one `git cat-file --batch`, never to the working tree. The skipped "ignored paths" are only those a committed `.gitignore` ignores; `.git/info/exclude` isn't part of the commit, so it no longer applies.
- **Amended by DR-86:** the per-blob cache keys facts by `(blob sha, extractor, the extractor's options, packs_hash)`, since an extractor's facts depend on its options; `compose`'s include the workspace's repo names.
- **Amended by DR-100:** the per-blob cache key holds the file's repo-qualified path too, since a file's facts copy it into their evidence and IDs and its capture charge counts it; and every declarative bound is per file, with one coverage cut per repo in sorted path order.
- **Sources (checked 2026-10-26):** [rust-stack §4] (690 files/s); review round 1 unasked-4 and owner decision D21.

## DR-48: Workspace trust, user-level credential settings and policy-bearing packs

- **Status:** accepted (decided in review round 1, D7 and D19). Amended in review rounds 2 and 3.
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
  - Scenarios: a fixture's out-of-tree pack loads with no card [DR-60], so no CI path decides one.
  - Users: a cloned repo with Planroom state asks once before its config counts.
- **Amended in review round 2:** a pack loads only from the built-ins and a trusted config, which the person-only `pack add` and `pack remove` write. `PLANROOM_PACKS` no longer loads one, so an agent's environment can't add a policy-bearing pack (E6, extensibility-1) [DR-62]. Test `trust.rs::an_agent_cannot_add_a_pack`. The scenarios consequence no longer deals a first-load card on every fixture, which DR-60 removed, and S7's CI path no longer keeps one (principles-5).
- **Amended in review round 3:** trust covers both forms of the state directory. A clone can't bring a `.git/planroom/`, so a single repo's state directory needs only the `workspaces.json` record (F6, unasked-4) [DR-74].
- **Sources (checked 2026-10-26):** KICKOFF `<constraints>` Claude; review round 1 security-1, security-5, security-11 and owner decisions D7 and D19.

## DR-49: Model calls are opt-in, consented and redacted

- **Status:** accepted (decided in review round 1, D7 and D8). Amended in review rounds 2 and 3.
- **Starting point:** none; serves the constraints "Claude" and "Spend" and the security gap KICKOFF leaves to the builder.
- **Origin:** review round 1: security-6, unasked-3 and cost-1.
- **Context:**
  - With a credential in the environment, the default config sent repo slices to third-party providers on the first decision, with no disclosure. Extractors keep secret-bearing values, such as compose `environment` values and DSN user info, and `kernel::redact` covered only logs, error details and the snapshot.
  - The `auto` backend counted the build container's proxy as a credential, so any product run in the build container spent live money outside `xtask live`, against KICKOFF's $3 cap and its "gate every live test behind an env flag".
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
- **Amended in review round 2:** redaction is on by default on every output path: CLI text and `--json`, Bash and skill calls, tool results, `/room` text, and every `live/head` and `acks` batch. It runs only over the fields the contract marks `#[redact]`, never an ID, handle, cursor or hash, so handles survive it. `PLANROOM_REDACT` is gone. Only the truth comparator opts out, with `PLANROOM_RAW=1`, which the guard denies to agents, so the rejected alternative "redacting the local CLI too" now holds through that one opt-out (E15; security-5, principles-2) [DR-64]. `privacy.rs::a_planted_repo_secret_reaches_no_request_or_tool_output` covers the Bash path, `/room` text and `live/head`, and `packages/mod/src/tools.test.ts::handles_round_trip_after_redaction` re-issues every handle.
- **Amended in review round 3:** consent cards pending at once for one workspace batch into one card with a pick per provider. Picking Set it up on the per-user Jev setup card records that intent, so the gateway consent card that follows pre-selects Yes even for a key found only in the environment (principles-7). `PLANROOM_RAW` reaches only a read command's `--json` stdout (security-7) [DR-75]. Tests `privacy.rs::pending_consents_batch_into_one_card`, `the_setup_choice_preselects_yes_on_consent`.
- **Sources (checked 2026-10-26):** KICKOFF `<constraints>` Claude and Spend, `<verified_facts>` (the proxy injects `*.vercel.sh` credentials); `examples/TRUTH.json` (the fixture's DSN); review round 1 security-6, unasked-3, cost-1 and owner decisions D7 and D8.

## DR-50: Decisions prefill before they interrupt

- **Status:** accepted (decided in review round 1, D9 and D20). Amended in review rounds 2 and 3.
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
- **Amended in review round 2:** a `person` card asks Claude only when the person taps "Ask Claude", which it offers only in the escalate band, so no read spends and a confident card never asks (principles-6). Accepting a batch accepts only the items the card showed (principles-9). Strict replay counts as a configured Jev backend, so no CI path deals the setup card, and the setup choice is recorded once per user under the data root, since whether a key exists is a fact about the person (principles-5). The setup card's cost comes from the shared estimator, at most `jev_per_scan_usd` a scan (cost-9) [DR-65]. Tests `kernel/tests/policy.rs::a_confident_person_card_never_asks_claude`, `focus_never_spends`, `batch_accept_is_bounded_by_base`; `first_run.rs::strict_replay_deals_no_setup_card`, `the_setup_choice_is_per_user`.
- **Amended in review round 3:** a prefill, an auto-enabled effect or an `infer` output never sets a field a person holds. Its decision is `Superseded` and listed in Diffs with no card, and the runner skips such subjects, so a kind-version bump never undoes a person's override (F1, correctness-1) [DR-73]. Tests `kernel/tests/policy.rs::a_prefill_never_overwrites_a_person_field`, `a_kind_version_bump_keeps_person_overrides`.
- **Sources (checked 2026-10-26):** KICKOFF principle 1 and R9; [jev §4, §5, §9]; `examples/TRUTH.json` prompt_snapshot (13 parts); review round 1 findings above and owner decisions D9 and D20.

## DR-51: One spend record, a user-level ledger and a measured always-on surface

- **Status:** accepted (decided in review round 1, D20). Amended in review rounds 2 and 3.
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
- **Amended in review round 2:** the spend log's path is absolute, resolved by `cargo xtask` through `git rev-parse --git-common-dir`, so every worktree and nested session shares one file, and a lock covers sum, check and append. `.claude/settings.json` no longer sets it, and the engine refuses a relative path (cost-1). `live evidence S02` compares each question's chat usage with `recorded` and refuses a regression, and `budgets.rs` gates a chat proxy per S2 question and each skill body (cost-2). Imported `sample` calls reach the spend log (cost-6). The daily cap counts real dollars only (cost-8) [DR-65].
- **Amended in review round 3:** `ledger.db` changes only by additions and records a `min_reader_version`, so older engines keep reading it, and a ledger migration keeps a `VACUUM INTO` copy (operability-7) [DR-78].
- **Sources (checked 2026-10-26):** KICKOFF `<constraints>` Spend; [claude-gateway §2] (prices); [mod-api §6, §9]; [plugin-packaging §1]; review round 1 findings above and owner decision D20.

## DR-52: What agents can reach: confined reads, person-only imports, and guarded worktrees

- **Status:** accepted (decided in review round 1, D15's import part, D16 and D18). Amended in review rounds 2 and 3.
- **Starting point:** extended "Events carry the workspace, project, actor, cause and an idempotency key."
- **Origin:** review round 1: security-2, security-3, security-4 and principles-1; the minor findings security-8 and security-9.
- **Context:**
  - `import` was in `room_change`, took its actor from a local directory path, and let the owner's uid run person-only commands. Any agent could write a row file and merge as a person.
  - `planroom open` was outside the catalogue and printed a login code into model-visible text. The implementer's cwd sat inside `.planroom/`, and the guard matched path strings, so `../../store.db` slipped past.
  - `ask breakdown PATH` on the auto-allowed `room_query` read any file the user could read, with no prompt.
- **Decision:**
  - `import` leaves every tool. Inbox rows and event exports need a person; `/room sync` runs the import with a person token after the model's `ArtifactData list`. Other viewers' rows apply as their attributed edits. The owner's own rows and any person-only command become one card the local person confirms. A model response imports only for a request the engine derives as pending, and runs an agent imports are kept with `trusted = false` and never mined.
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
- **Amended in review round 2:** the guard also denies reads of the mod's `$.store` file and the login redirect pages, and any call that sets `PLANROOM_PERSON_TOKEN` or `PLANROOM_RAW`. The implementer works in separate clones, not linked worktrees, so `implementer_writes_only_in_its_worktrees` becomes `implementer_writes_only_in_its_clones`. It runs the commands the person approved with `handoff start` with no prompt, and any other Bash call is denied and listed on the review card, with a retry that allows it (E15; security-1, security-2, unasked-6) [DR-64]. Import takes inbox rows only on stdin from the mod, never from a folder any process could write (E8, security-3) [DR-67].
- **Amended in review round 3:** the guard also denies agent reads of the user-level config and an agent's `git push` that names `refs/planroom/` (security-11, unasked-3). A run from the `agent` implementer is mined only from the Bash calls the mod observed and the branch's commits, and every curation draft is shown in full (security-5) [DR-75].
- **Sources (checked 2026-10-26):** KICKOFF R5 and R8; [artifact §2], [artifact-spike] (session writes need no prompt); [mod-api §5, §8]; review round 1 findings above and owner decisions D15, D16 and D18.

## DR-53: Automatic event backups and safe migrations

- **Status:** accepted (decided in review round 1, D6). Amended in review rounds 2 and 3.
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
- **Amended in review round 2:** the backup is written through, not debounced, since most writers are one-shot CLI processes that can't flush a skipped write. Every person-authored commit appends the events above the high-water `meta.backup_seq` under a lock on the backup file, fsyncs and advances the mark, and any later process catches up a gap. A restore sorts by `seq` and dedupes by `idem`, and `doctor` warns when the mark is behind (E5, correctness-8). The ephemeral-host copy and the pre-restore copy in E5 answer other findings (operability-3, unasked-1, unasked-5). Tests `backup.rs::a_burst_of_three_edits_is_fully_backed_up`, `concurrent_appenders_keep_order`, `a_later_process_catches_up_a_gap`. These now apply. On a temporary host (`CLAUDE_CODE_REMOTE`, `CODESPACES`, `REMOTE_CONTAINERS`) the write-through also commits the backup file to `refs/planroom/backup/<workspace_id>` in one workspace repo, named in `meta.backup_ref`, with git plumbing and hooks off; the status line says once how to push it, and `scan` restores from it when the data root has none (unasked-1). A restore first writes a pre-restore copy, so restoring that copy undoes it, and sets the person-authored events above the restored copy aside as a replay set that the next successful migration replays through `store::commit` (unasked-5). Operability-3 is met by the write-through. The older engine's line now names its fix instead of always saying to restart (operability-8) [DR-69]. Tests `a_temporary_host_writes_the_backup_ref`, `scan_restores_from_the_backup_ref`, `a_restore_is_undone_by_its_pre_restore_copy`, `restore_then_upgrade_replays_set_aside_edits`.
- **Amended in review round 3:** a replay set's edits take the restored copy's last `seq` as their base, so a field written after the restore conflicts with them and becomes a person card (F4, correctness-4). A restore into an empty store keeps each event's stored status and skips `scan` events, and the next scan records a baseline that keeps the restored classes [DR-73]. A single-repo workspace keeps its state in its git directory, out of reach of `git clean`, so the rejected alternative "the store outside the tree" now holds for that case. `git_clean_then_scan_offers_one_restore_card` becomes `a_lost_state_directory_then_scan_offers_one_restore_card`, beside `git_clean_leaves_a_single_repos_state_alone` (F6, unasked-4) [DR-74]. On a temporary host the ref holds the redacted event export, the `temporary_host` warning carries only the repo and the ref, and only the mod's pane and status line say how to copy it out. Planroom never pushes it (F7; unasked-3, security-9) [DR-75]. `doctor --restore` also takes the ledger's pre-migration copy, and `workspaces.json` follows the data root's additions-only rule (operability-7) [DR-78].
- **Sources (checked 2026-10-26):** KICKOFF principle 1 ("Never lose or overwrite what someone did"); review round 1 principles-4, operability-4, unasked-1 and unasked-2, and owner decision D6.

## DR-54: Surfaces act through catalogue verbs, reversibly, in real keystrokes

- **Status:** accepted (decided in review round 1, D15). Amended in review rounds 2 and 3.
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
- **Amended in review round 2:** the band and `PaneModel.next` also never name a leaf that can spend (`CommandSpec.spends`), so a stray digit can't start a paid model call (principles-12). The review card that `collect` deals takes a second key to merge (principles-7) [DR-66]. With no open decision the pane draws `PaneModel.next` as its face-up card, so vscode and mobile, which have no band, still show the next action (ux-5). A band press from the empty prompt shows a toast of what it did, and for 10 s the band's primary is its undo (ux-6). Choices sit two to a row, and a fixed label holds 20 columns (ux-7). Below 34 body rows or 49 columns the pane draws a compact layout: a focus line, the card and the list (ux-8). Enter on a stage row under `i` runs that stage through its cost card, and an accepted stage makes `intent version` the next action (ux-11). Tests `surfaces.test.tsx::no_decision_shows_next_on_every_surface`, `rows_fit_compact_at_80x24_and_100x30`; `keys.test.tsx::a_band_press_from_the_prompt_offers_undo`. On the canvas, the action tray, lid drops, felt pins and chalk relations each run one catalogue `edit` leaf, so hand edits follow this record's rules too (ux-2) [DR-68].
- **Amended in review round 3:** `l`, `p`, `i`, `h` and `t` deal pickers whose first ten entries are a key plus a digit. The stage picker is the stage run's cost card, so its pick is the confirming key, which replaces round 2's "Enter on a stage row under `i` runs that stage through its cost card". Merging is the unasked review card's Merge, then a second press. The row-key pool covers the drift and who lines' top items and 7 list rows, and the compact layout keeps the top diff line. S9 counts keys with no list open, per flow in `feature_list.json` (F9, ux-2, ux-3, principles-2) [DR-77].
- **Sources (checked 2026-10-26):** [mod-spike] (bare digits, `ctrl+x tab`, the 49×34 dock, diff hunk headers); [mod-api §2] (at most 36 one-key actions per pane); KICKOFF S9, S15 and principle 1; review round 1 findings above and owner decision D15.

## DR-55: Hand-off revert and an enumerated irreversible list

- **Status:** accepted (decided in review round 1, D15). Amended in review rounds 2 and 3.
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
- **Amended in review round 2:** the implementer works in local clones under the hand-off directory, not linked worktrees, so `handoff cancel` and a drop first fetch each clone's `HEAD` into `planroom/KEY`, then remove the clones and archive the branch (E15) [DR-64]. The loop also gains `merging -> reverting`, for a merge that fails after the preflight when the person picks revert on its card (correctness-11) [DR-25].
- **Amended in review round 3:** `handoff revert` of a change landed by a pull request reverts its recorded `landed` commits, newest first (F20) [DR-80].
- **Sources (checked 2026-10-26):** KICKOFF principle 1 and R8; review round 1 principles-3 and unasked-9; owner decision D15.

## DR-56: The Artifact declares what it serves and shows synced edits

- **Status:** accepted (decided in review round 1, D15). Amended in review rounds 2 and 3.
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
- **Amended in review round 2:** the queued rows and the Send to Claude button make the page's face-up Decisions card, so neither adds a top-level concept, and the unavailable state's button is "Back to the map", which a phone can press (ux-10). Test `tests/e2e/artifact.spec.ts::queued_rows_are_the_face_up_card`, and `exactly_five_concepts` on the page.
- **Amended in review round 3:** the snapshot holds every lens's maps and precomputes the views S1 to S6 need, and any other view becomes a read request the next sync answers. The empty state that named `/room open` is gone, since a cloud session can't open one, and the rejected alternative's "the empty state already names what to do" no longer holds. `artifact.spec.ts::a_view_outside_the_snapshot_becomes_a_request` replaces `views_it_cannot_serve_show_an_empty_state` (F10, ux-1) [DR-77].
- **Sources (checked 2026-10-26):** [artifact §2] (`comments.sendToClaude` is for writers only; `db` writes don't wake the session); [artifact-spike]; KICKOFF R13 and principle 1; review round 1 ux-5, ux-7 and ux-1; owner decision D15.

## DR-57: The pane before the engine and outside a workspace

- **Status:** accepted (decided in review round 1, D15). Amended in review rounds 2 and 3.
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
- **Amended in review round 2:** a `person_key_mismatch` fallback pane shows the one terminal line that re-keys the workspace, and the mods-off text lists the presses that go missing and which person-only commands a terminal can still run (unasked-4, downgraded) [DR-60].
- **Amended in review round 3:** before registering anything, the mod checks its host's version and the `$` members it calls; on a mismatch it shows one line and leaves the skills and the CLI (F19, unasked-5) [DR-76]. The `person_key_mismatch` pane's action is a pane Button that runs `person init --rekey` with a press, not a terminal line, and `open` takes only a press, so neither runs with mods off (F15) [DR-75].
- **Sources (checked 2026-10-26):** KICKOFF principle 1, S14 and S15; [mod-api §7]; review round 1 principles-5, ux-6, unasked-7 and operability-8; owner decision D15.

## DR-58: One data root, a self-locating resolver, an embedded canvas and visible background work

- **Status:** accepted (decided in review round 1, D16), verify A-14 in WS5. Amended in review rounds 2 and 3. Amended by DR-88.
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
- **Amended in review round 2:** `main` is the integration branch with `-dev` versions between releases, which replaces the departure from Iris above. Prefetch and build detach without a `setsid` command, and `serve` daemonizes in Rust. A cached install's build clones the tag, or `main` for a `-dev` version, instead of `cargo install --git`, so it embeds the canvas. Pruning keeps every version used in the last 7 days (E11, E12, E13) [DR-69].
- **Amended in review round 3:** the engine cache and the source clone are keyed by version and catalogue hash, a non-default base's download is cached apart, and the checkout test needs `crates/planroom/Cargo.toml` outside Claude Code's plugins directory (operability-1, operability-3, operability-5) [DR-78].
- **Amended by DR-88:** the data root is only ever absolute: `PLANROOM_DATA`, `XDG_DATA_HOME` or `HOME/.local/share`, each only when absolute, else `invalid_argument` naming `PLANROOM_DATA`.
- **Sources (checked 2026-10-26):** [plugin-packaging §3, Design implications]; [mod-api §7]; [iris-release-bar §1, §3]; KICKOFF constraints on packaging, the release bar and pushing tags; review round 1 findings above and owner decision D16.

## DR-59: Repo gates that fire

- **Status:** accepted (decided in review round 1, D14). Amended in review rounds 2 and 3. Amended by DR-89.
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
- **Amended in review round 2:** the per-commit walk covers only the commits a push or pull request adds, a state check runs every time, and the first lock comes with a new record. The gated surfaces add the area contracts table, the Boundaries section of `AGENTS.md` and each package's dependency names. Dependabot waits 14 days, the direct-pin age. `cargo xtask test` builds `target/mod-test/` and plain `gen` writes every committed output, replacing `gen --mod`, `PLANROOM_UPDATE_DOCS` and `PLANROOM_UPDATE_GOLDEN` (E14, repo-1, repo-2, repo-10, repo-11) [DR-70].
- **Amended in review round 3:** every shipped crate has an allow-list, only `models` and `planroom` open sockets, `contract` carries the no-I/O lint, and a `skipped` check fails in CI unless declared (F18, repo-1, repo-2) [DR-79]. This replaces the forbid-list that kept a network client out of `store` and `extract`.
- **Amended by DR-89:** `feature_list_is_complete` passes when every listed test resolves to a test that runs under a CI `cargo xtask` command with nothing live, or carries a `due` tag naming the milestone that builds it; a test that resolves carries no `due`. A raise is a larger value for a key `budgets.json` already holds. A new key isn't a raise and needs no record.
- **Sources (checked 2026-10-26):** KICKOFF R16, S7, S15 and the repo-tooling constraint; [plugin-packaging §1] (`claude plugin test` needs a `modules` entry); [mod-api §1, §9]; [ts-stack §6.1]; [iris-release-bar §5, §7]; review round 1 findings above and owner decision D14.

## DR-60: Owner follow-ups after review round 1

- **Status:** accepted (decided 2026-10-26). Amended in review rounds 2 and 3.
- **Starting point:** Store: events carry the actor; Packs: declarative files.
- **Context:** the round 1 fixes raised four gaps: a CLI process as a writer instance makes an agent conflict with its own earlier write; under DR-4 a `scan` from CI with no actor would be `actor_required`; nothing says how the person key's hash is first registered, so an agent could register its own key first; and DR-48's first-load card for out-of-tree packs would appear on every fixture scan, interfering with S1's one next action, S7's at most one card and S15's empty-state audit.
- **Decision:**
  - An agent's client ID is its own ID from `PLANROOM_ACTOR` (`agent:<id>:<role>`), so one agent is one writer instance across processes. Only a person at a terminal is `cli:<pid>`.
  - Writes that only record observations, `scan` and the duties a live process runs, are `system:<part>` writes and need no actor. Every write that authors something still needs one.
  - The mod generates the person key on its first `session.start` in a workspace and registers its hash with `planroom person init`, which succeeds only when no hash is registered. Re-registering needs the old key or a `/dev/tty` confirmation. The `tool.check` guard denies agent calls to `person init`, and `planroom doctor` shows when the key was registered and from which surface. This stays a guardrail, not a boundary between processes of one OS user.
  - A pack that a person lists in config or `PLANROOM_PACKS` loads with no card: listing it was the person's choice. `planroom doctor` lists every loaded pack with its kinds and the sha256 of its prompts. The safety that matters stays: an out-of-tree pack's `auto_enabled` counts only after a person enables auto-action per kind.
- **Alternatives rejected:** a card on every first load of a pack (it interrupts for a choice the person already made, against principle 1); exempting all CLI writes from the actor rule (it would let unnamed agents author).
- **Consequences:** contracts gain `Command::PersonInit` (person-only by construction, mod-only through the guard); `actor.rs` gains `scan_needs_no_actor` and `an_agent_does_not_conflict_with_itself`; `trust.rs` gains `an_out_of_tree_pack_loads_without_a_card`; `guard.test.ts` gains `agents_cannot_init_the_person_key`.
- **Amended in review round 2:** a pack loads with no card because a person added it with `pack add`. `PLANROOM_PACKS` no longer lists packs, so the last bullet's "or `PLANROOM_PACKS`" no longer applies (E6) [DR-62]. `Command::PersonInit` joins the catalogue, the leaf table and the undo classes (`Query`), with `PersonPair` beside it [DR-67]. `person init` reads the key from stdin, and `store.db` keeps the registered key, which DR-72 made the public half of an ECDSA P-256 pair (principles-5, security-1) [DR-64, DR-72]. `person init` returns `person_key_mismatch` when another key is registered, and `person init --rekey` replaces the registered key with the mod's own (unasked-4); round 3 took its `/dev/tty` path away [DR-75].
- **Amended in review round 3:** re-registering takes a token under the old key or a press of the mismatch pane's button, never a `/dev/tty` confirmation, and `store.db` keeps only the public key (F15, security-3, security-4) [DR-72, DR-75].
- **Sources:** `docs/contributing/research/arch-review/round-1/fixes.md` (the notes from the fixes), checked 2026-10-26.


## DR-61: Writes that keep the tree, undos and imports whole

- **Status:** accepted (decided in review round 2, E4).
- **Starting point:** extended "Optimistic concurrency on entity versions" and "Event (append-only; everything else is derived from events)".
- **Origin:** review round 2: correctness-4, correctness-5, correctness-6 and correctness-7.
- **Context:**
  - Compare-and-set is per field, so two writers who each moved one node under the other both landed, and roll-up never ended on the cycle.
  - An `undo` set no authored revision, so a write from a base older than the undo overwrote it with no conflict. Undo also targeted one event, while a regroup writes a `put` and a `set parent` per member, so undoing the `put` left members under a missing parent.
  - The incremental fold refolded only a scanned repo's subtree. That misses the link snapshot and the workspace-level containers, and it can't express an undo or a review of an event it already applied.
  - All of an Artifact viewer's devices were one writer instance, and import set no order for a viewer's rows, so one viewer's edits were last-writer-wins.
- **Decision:**
  - `store::commit` refuses a parent change that would make a cycle, with `invalid_argument` naming the cycle. The fold breaks any cycle an older store or an import holds: the higher-`seq` edit loses and is listed in Diffs. A parent that doesn't resolve falls back to the fact parent, listed in Diffs.
  - An undo is a write. It carries `base`, passes the same compare-and-set, sets the authored revision of every field it changes back at its own `seq` and writer instance, and conflicts like any write.
  - An undo is per command: `Undo { of, events, base }` reverses every event the command that wrote `of` wrote, in one transaction.
  - A `scan` event, or an `undo` or `review` of an event at or below the cached `seq`, refolds `serve`'s and `watch`'s model from scratch. Subtree refolds stay out of v0.1.
  - The Artifact page mints an instance ID per load and a counter per row. The client is `artifact:<uid>:<instance>`, and import applies rows in `(instance, counter)` order.
- **Alternatives rejected:**
  - Storing a cycle as a conflict with a card: the fold would still have to break it, and a refusal tells the writer at once.
  - Undo by single event with a cleanup pass for orphans: a half-reverted regroup still shows in between, and the cleanup is a second mechanism.
  - Subtree refolds that track the link snapshot: more code for a fold that already fits its 50 ms budget from scratch.
  - One writer instance per viewer, ordered by `at`: two devices' clocks disagree, and their edits would still never conflict.
- **Consequences:**
  - Contracts: `Undo { of, events, base }`; `InboxRow.instance` and `InboxRow.counter` in place of `seq`; `invalid_argument` details gain `cycle`; the Artifact `Client` ID.
  - Tests: `kernel/tests/fold.rs::a_parent_cycle_breaks_by_seq`, `an_unresolved_parent_falls_back_to_the_fact_parent`, `incremental_fold_equals_full_fold` (a proptest over scans of random repos, link changes, undos and reviews of older events); `kernel/tests/cas.rs::a_parent_change_that_makes_a_cycle_is_refused`, `a_write_over_an_undo_since_base_conflicts`; `kernel/tests/undo.rs::regroup_undo_is_atomic`; cycle cases in `store/tests/interleavings.rs::writer_interleavings`; `s11_remote.rs::one_instances_rows_apply_in_counter_order`, `two_instances_of_one_viewer_conflict`.
  - Users: one undo after a regroup takes the whole regroup back, and a viewer's phone and laptop get the same three-way card two canvas tabs do.
  - It amends DR-2, DR-3, DR-5 and DR-31.
- **Sources (checked 2026-10-26):** KICKOFF R6, R10 and principle 1; review round 2 correctness-4 to correctness-7 and owner decision E4.

## DR-62: The pack set is workspace state

- **Status:** accepted (decided in review round 2, E6). Amended in review round 3.
- **Starting point:** extended "Packs. Declarative files that add kinds, levels, roll-up rules, extractors, lenses, decision kinds and a default view."
- **Origin:** review round 2, extensibility-1; the minor finding extensibility-7.
- **Context:**
  - Each process resolved its pack set from its own environment through `PLANROOM_PACKS`, while every process folds and scans one store. Two processes saw different models, the last scanner decided everyone's actual layer, and S10's later commands ran without the packs its scan had loaded.
  - Any agent's Bash could set `PLANROOM_PACKS`, and a pack carries policy: its low-stakes prefills and `infer` outputs apply with no person involved.
  - `requires` allowed only `[]` or `["code"]`, so an out-of-tree pack couldn't build on observability, release, artifacts or machines.
- **Decision:**
  - Packs come only from the built-ins and the trusted workspace config. `PLANROOM_PACKS` is read only by `planroom doctor --pack`.
  - `planroom pack add DIR` and `planroom pack remove ID` are person-only (a press, a typed `/room` or a `/dev/tty` confirmation) and write the config. Each one's undo is the other.
  - The store's `meta` records the resolved pack list and its `packs_hash`. A process whose resolved set differs refuses writes with `packs_mismatch`. A long-lived process reloads its packs once, an older engine says to restart, a newer engine records its own built-ins at its first write, and a hand edit of the config counts only after `pack add` or `pack remove`.
  - A changed `packs_hash` makes every active snapshot stale, so the next freshness check rescans.
  - Rings are by origin: a built-in pack requires `code` or nothing, and an out-of-tree pack may require any built-in pack but no other out-of-tree pack.
- **Alternatives rejected:**
  - Folding with the recorded set whatever a process resolved: the process would run extractors and prompts it never loaded.
  - `scan --pack DIR` with a `/dev/tty` confirmation: it ties a lasting setting to one scan, and a verb of its own names the change it makes.
  - Keeping `PLANROOM_PACKS` behind a `tool.check` deny: a guardrail for a variable no command needs.
- **Consequences:**
  - Contracts: `Command::PackAdd` and `Command::PackRemove`; error `packs_mismatch`; `meta.packs` and `meta.packs_hash`; no environment override for the `packs` key; `PackFile.requires` by origin.
  - Layout: `boundaries.toml` gains `out_of_tree_requires`, and the self-test's pack plants change.
  - Tests: `packs.rs::alternating_scans_with_different_environments_never_flip_the_snapshot`, `a_process_with_another_pack_set_refuses_writes`, `an_out_of_tree_pack_may_require_any_built_in`; `trust.rs::an_agent_cannot_add_a_pack`, `planroom_packs_loads_nothing_outside_doctor`; S10's CI path adds its packs with `pack add` from a directory outside the tree.
  - Users: adding a pack is one command, and every session, tab and agent sees it.
  - It amends DR-9, DR-10, DR-13, DR-48 and DR-60.
- **Amended in review round 3:** `pack add` copies the validated pack into the state directory's `packs/<sha256>/`, and every process loads only the copy, so editing, moving or deleting a pack's source refuses no write. A changed source deals one person-only card, update or keep the current copy, with Keep pre-selected, and `packs_mismatch` remains only for engine version skew (extensibility-3, downgraded). Tests `packs.rs::an_added_pack_loads_from_its_copy`, `editing_a_packs_source_deals_one_card_and_refuses_no_write`, `deleting_a_packs_source_changes_nothing`.
- **Sources (checked 2026-10-26):** KICKOFF R5, R10, R11, R16 and S10; review round 2 extensibility-1 and extensibility-7, and owner decision E6.

## DR-63: Decision kinds express v0.1's kinds as data

- **Status:** accepted (decided in review round 2, E7). Amended in review round 3.
- **Starting point:** extended "Jev policy. Each decision kind declares its question, thresholds and escalation."
- **Origin:** review round 2, extensibility-2.
- **Context:** `DecisionKind` had a node-or-edge subject, a field-value `when`, static choice options and five effects. Five of the eleven v0.1 kinds needed more. `machines_same` asks about a machine pair. `override_retarget` asks about an orphaned override and moves it. `file_component` and `attach_target` choose among candidates that differ per subject. The prechecks of `file_component` and `commit_addresses_thread` weren't field predicates. The way out was naming kinds in the engine, which DR-17 forbids.
- **Decision:**
  - A choice either lists static `options` or names `options_from = { shape, params, max }`, a query shape over the subject that yields its candidates. Jev gets them as the request's options, each eval case stores them as `options`, and an effect names the pick as `$pick`. `auto_choices` applies to static options only.
  - A subject is a node or edge kind, or a role (`subject = { role = "rescan.retarget" }`) whose raiser supplies typed `Subject::Role { role, key }` subjects.
  - `when` reads props that built-in extractors precompute. `files` sets `placed_by_rule`, `review_threads` sets `touched_after_anchor`, and the machines pack's `machine_pairing` linker emits `machines.describes` candidates with the Jaccard overlap as confidence and `paired` set at 0.8 or above.
  - A sixth effect, `retarget`, copies an orphaned override onto the picked node. It is valid only for a `rescan.retarget` subject in a `person`-stakes kind. `set_prop` may set `parent`.
  - A test checks that every v0.1 kind validates against the contract.
- **Alternatives rejected:**
  - Hard-coding the five kinds in `app`: it breaks DR-17 and R11, and a pack author couldn't copy the pattern.
  - An expression language for `when`: a parser and an injection surface, where a precomputed prop is enough.
  - Retargeting as the role's built-in effect: a hidden effect outside the closed set the injection tests check.
- **Consequences:**
  - Contracts: `SubjectKind`, `Subject`, `OptionsFrom`, `Effect::Retarget`, `EvalCase.options`.
  - Tests: `packs.rs::every_v0_1_decision_kind_validates_against_the_contract`; `kernel/tests/effects.rs::retarget_needs_a_person_stakes_role_subject`; `kernel/tests/policy.rs::options_from_gives_per_subject_candidates`; `extract/tests/link.rs::machine_pairing_scores_state_overlap`.
  - Pack authors: a "which owner?" kind is pack data.
  - It amends DR-17.
- **Amended in review round 3:** `when` is the `filter` shape's predicate, fields plus `with_edge` and `without_edge`, evaluated by the kernel over the folded model. Precomputed props remain only for facts an extractor alone sees. The toy `flag_stale` kind's `when` selects a disabled flag with no incoming `flags.reads_flag` edge, and its card asks "gift_wrap is off and unused on main. Remove it or keep it?" (F13, extensibility-2). Test `kernel/tests/policy.rs::when_reads_graph_structure`.
- **Sources (checked 2026-10-26):** KICKOFF R9 and R11; [jev §5]; review round 2 extensibility-2 and owner decision E7.

## DR-64: Agents never hold the person's authority

- **Status:** accepted (decided in review round 2, E15); verify A-15 in WS8. Amended in review round 3.
- **Starting point:** extended "Events carry the workspace, project, actor, cause and an idempotency key" (who counts as a person) and "A single-writer local server per workspace" (its exposure).
- **Origin:** review round 2: security-1, security-2, security-4, security-5, security-6, security-7 and unasked-6; the minor findings security-8 and security-10.
- **Context:**
  - Every person check rested on a key that sat in the mod's `$.store`, a plain JSON file no guard covered, and the CLI got the raw key in its environment on every press, where `/proc` and every git child could read it.
  - The implementer worked in linked worktrees, which share the real repo's refs, so its Bash could move `main` or delete a branch before any review.
  - `open` handed the login code to the browser opener's argv, which other local users and agents can read.
  - Redaction ran only on the mod's tool outputs. Bash and skill calls, `/room` text and the `live/head` delta that every Artifact viewer reads carried raw secrets.
  - Git disabled hooks and fsmonitor but still ran a repo's textconv, external diff, filter and merge drivers, and the minimum git predated `safe.directory`.
  - Repo text reached terminals and the review diff with control, ANSI and bidi characters intact.
  - A background implementer's tests, installs and commits each raised a permission prompt, or failed where nobody could respond.
- **Decision:**
  - **Action tokens.** The mod signs each person action over `command ‖ canonical(args) ‖ idem ‖ base ‖ at` with `crypto.subtle` and passes it as `PLANROOM_PERSON_TOKEN`, never the key. DR-72 made the signature ECDSA P-256, so `store.db` `meta` keeps only the public key. The engine refuses a token older than 30 s or for another command, and spends each one. It removes the token from its environment at start and runs git and the browser opener with a cleared environment and an allow-list. The guard denies agent reads of the mod's `$.store` file [A-15].
  - **Separate clones.** `handoff start` makes a local clone per touched repo under the hand-off directory, with hooks off. `handoff collect`, and a cancel or drop, fetch only each clone's `HEAD` into `planroom/KEY` in the real repo.
  - **The login code in no argv.** `open` writes the URL into a 0600 redirect page under `DATA_ROOT/open/` and opens that file. A second try at a code revokes the token the first exchange issued.
  - **Redaction everywhere, by field.** `kernel::redact` runs on every output path, over the fields the contract marks `#[redact]`, and never over an ID, handle, cursor or hash. Only the truth comparator opts out, with `PLANROOM_RAW=1`, which the guard denies to agents.
  - **Git that runs no repo command.** `--no-ext-diff --no-textconv` on every `diff`, `show` and `log`; git 2.35.2 or later with `safe.directory` on; and before a checkout or merge, a repo whose local config names a filter, a diff command or textconv, a merge driver, an editor, a pager, an ssh command or an include is refused with one card.
  - **One display-sanitizing pass.** `kernel::display::sanitize` and its port in `@planroom/ui` show C0 and C1 controls and ESC as visible escapes, and bidi and zero-width characters as visible markers, in every human-facing text. `collect` adds `hidden_characters` to a diff summary whose hunk holds them.
  - **Approved commands.** `handoff start` is the person's one approval of the spec's bounded command list: its checks, the installs its lockfiles call for, and `git add` and `git commit`. The guard allows the implementer's matching Bash inside its clones with no prompt and denies the rest with a reason. The mod lists the denied commands in the run, and the review card offers a retry that allows them (`handoff start KEY --allow CMD`).
  - **No terminal path for repo-changing commands.** `handoff start`, `handoff merge`, `handoff revert` and `person pair` need a mod press or a canvas tab, never a `/dev/tty` confirmation.
  - **Comment text is guidance.** The platform delivers a Send to Claude comment's text into the turn it wakes, so the docs state the no-quoting rule as guidance, and the containment is the person-only gates.
- **Alternatives rejected:**
  - An Ed25519 key pair, so `store.db` would hold only a public key: stronger at rest, but HMAC is one primitive that the mod, the page and the engine all share. DR-72 took this path with ECDSA P-256 after round 2, because a leaked backup would otherwise sign as the person.
  - Linked worktrees with a deny-list of ref-changing git commands: one encoded command defeats a name match.
  - Overriding each driver with `-c` flags: git can't be told to disable a driver whose name it doesn't know, so refusing the repo is simpler.
  - A prompt per implementer Bash call: the person gets prompts they can't judge, and a headless session fails the hand-off.
- **Consequences:**
  - Contracts: `PLANROOM_PERSON_TOKEN`; `meta.person_pubkey` (DR-72) and `meta.spent_tokens`; `#[redact]` on repo-derived fields; `SpecRepo.clone`, `ChangeSpec.commands`, `ImplementerRun.denied`; `handoff start --allow`; error `repo_config_refused`; warning `hidden_characters`.
  - Layout: `crates/kernel/src/display.rs`; `packages/ui/src/display.ts`; tests `crates/planroom/tests/git_hardening.rs` and `handoff_clone.rs`.
  - Tests: `actor.rs::a_token_works_once_and_only_for_its_command`, `repo_changing_commands_refuse_the_terminal_path`, `children_never_inherit_the_token`; `guard.test.ts::the_mod_store_file_is_guarded`, `implementer_writes_only_in_its_clones`, `implementer_runs_spec_checks_without_prompts`, `implementer_unlisted_bash_is_denied_and_listed_once`; `handoff_clone.rs::the_implementer_cannot_move_the_real_repos_refs`, `a_hidden_character_hunk_warns`; `server_auth.rs::the_code_is_never_in_an_argv`, `a_code_exchanged_twice_revokes_its_token`; `privacy.rs::a_planted_repo_secret_reaches_no_request_or_tool_output` on the Bash path, `/room` text and `live/head`; `tools.test.ts::handles_round_trip_after_redaction`; `git_hardening.rs::textconv_ext_diff_and_drivers_never_fire`, `a_repo_whose_config_names_a_command_is_refused_before_merge`; `kernel/tests/display.rs::controls_and_bidi_show_as_markers`; `packages/ui/src/display.test.ts::display_parity`.
  - Release: git 2.35.2 is the minimum `doctor` checks.
  - Conventions: `AGENTS.md` says to mark repo-derived fields `#[redact]`, to pass human-facing strings through the display-sanitizing pass, and that an agent never sets `PLANROOM_PERSON_TOKEN` or `PLANROOM_RAW`.
  - Users: a hand-off runs unattended within its approved commands; commands outside them cost one card. A repo with a per-repo filter, such as a local Git LFS install, is refused for a merge with one card naming the keys; a global install isn't affected.
  - It amends DR-4, DR-16, DR-25, DR-29, DR-49, DR-52 and DR-60.
- **Amended in review round 3:** the implementer's Bash runs under Claude Code's sandbox [A-18], its installs run before it starts, and without a sandbox its approved commands ask; `open` and `person init --rekey` take only a press; `PLANROOM_RAW` reaches only a read's `--json` stdout; and the browser opener gets the fixed system `PATH` (F15; security-1, security-4, security-7) [DR-75]. The 2.35.2 floor becomes 2.34.1, and the engine itself refuses a repo another user owns (unasked-8) [DR-78].
- **Sources (checked 2026-10-26):** KICKOFF R5, R8 and principle 1; [mod-api §7] (`$.store` is a JSON file under the Claude Code configuration directory) and its module rules (`crypto.subtle`); review round 2 security-1, security-2, security-4 to security-8, security-10 and unasked-6, and owner decision E15.

## DR-65: Cost gates that can fail

- **Status:** accepted (decided in review round 2, E16). Amended in review round 3. Extended by DR-89.
- **Starting point:** none; serves the constraint "Spend" and S7's token budgets.
- **Origin:** review round 2: cost-1, cost-2, cost-3 and cost-5; the minor findings cost-6, cost-7, cost-8, cost-9 and cost-10.
- **Context:**
  - The spend log's path was relative, so each worktree, fixture directory and nested evidence session summed its own copy, and two callers could sum and append at once. The $3 cap wasn't enforced.
  - S2's Claude tokens lived only in `budgets.json` `recorded`, which CI doesn't gate, and the command that re-measured them overwrote them. Skill bodies had no budget.
  - S7's per-scan guard equalled its bound and deferred silently, so "under $0.001" held by construction.
  - The decision input hash included evidence line numbers, so a commit that inserted a line above evidence re-asked decided items and re-dealt person cards.
  - `sample` calls never reached the spend log, a prefix below the cache minimum went unnoticed, curation cached prefixes it never reused, one daily cap mixed real dollars with notional session usage, card copy disagreed with the cap, and the S3 floor assumed a cache the session backend doesn't keep.
- **Decision:**
  - `cargo xtask` sets `PLANROOM_SPEND_LOG` to the main checkout's file through `git rev-parse --git-common-dir`, and passes it into nested sessions. The engine refuses a relative path, and a `PLANROOM_LIVE_*` flag with no log, and holds a lock across sum, check and append. Each evidence run checks that its live calls reached the file.
  - `budgets.rs` gates a CI proxy per S2 question: the always-on surface, the skill bodies its path loads and its tool results. Each skill body has its own budget. `cargo xtask live evidence S02` compares new chat usage with `recorded` and refuses a regression past 20%, or a hit-rate drop past 0.1, unless the commit adds a decision record.
  - S7 runs with `jev_per_scan_usd = 0.01` and asserts zero budget deferrals and zero pending requests after the rescan. The Jev budget counts per `scan` event and per duty run.
  - A slice serializes evidence `path` and `contains`, with an occurrence index for a repeated line, never `line`.
  - Import appends priced `sample` calls to the spend log; the page logs the applied model.
  - A marked prefix with no cache write and no read is `cache_not_applied`, which fails the budget test. `xtask live record` measures each prefix with `count_tokens`, needs 1,200 real tokens or `uncached`, and checks that a recording starts cold. Curation keeps the 5-minute TTL and puts the target file in the tail.
  - The daily cap counts gateway and TypeSafe dollars only; session and agent usage is priced and reported, never gated, and no spawn is refused by a cap.
  - Every cost figure on a card comes from one estimator, capped by its limit.
  - The S3 floor applies to gateway recordings; the session backend's hit rate is evidence, and its card says the cache lasts 5 minutes.
- **Alternatives rejected:**
  - Setting the path in `.claude/settings.json`: its values are static, so it can't name the main checkout from a worktree.
  - Per-worktree logs merged at the end: each worktree's check passes while the total doesn't.
  - Gating `recorded` in CI: it needs a live session and a credential.
  - Keeping `line` in the hash and carrying picks forward through a separate table: a second mechanism for what leaving `line` out does.
- **Consequences:**
  - Contracts: `invalid_argument` for a relative spend log; `budgets.json` `chat_proxy` and `surface.skill_bodies`; warning `cache_not_applied`; the meanings of `jev_per_scan_usd`, `claude_per_day_usd` and `live_tests_total_usd`; `inbox/<uid>` `sample` entries name their model; spend-log lines with `provider = sample`.
  - Tests: `live_gate.rs::the_spend_log_is_shared_across_worktrees_and_cwds`, `concurrent_live_calls_serialize_on_the_spend_log`, `a_relative_spend_log_is_refused`; `budgets.rs::s2_chat_proxy_within_20_percent`, `a_prefix_that_never_caches_fails`; `s07_decisions.rs::a_line_shift_above_evidence_hits_the_cache`, `nothing_deferred_by_budget_on_the_rescan`; `store/tests/ledger.rs::session_and_agent_usage_is_reported_not_gated`; `packages/mod/src/agents.test.ts::a_spawn_is_never_refused_by_a_cap`; `text.rs::card_costs_match_the_estimator`; `s11_remote.rs::sample_calls_reach_the_spend_log`.
  - Scenarios: S7's CI path writes `jev_per_scan_usd = 0.01` into its fixture's config.
  - It amends DR-18, DR-27, DR-50 and DR-51.
- **Amended in review round 3:** a `scan` event triggers one `decide` duty run that spends that scan's budget, and leftovers wait for the next scan or a `decide --run` (cost-5). S7's warm phase repeats `decide --run` until none is pending (cost-4). `xtask live record` refuses a warm prefix before it pays, from the ledger's last write of that `prefix_hash` (cost-2). The live spend plan adds development re-records, aborted recordings and Phase 1's spend, and the walking skeleton seeds both spend logs (cost-1, cost-3). Bare `/room` returns no `text`, and any other `/room` text holds at most 300 tokens, gated as `room_text` (cost-6). Tests `duties.rs::one_scan_spends_at_most_its_budget_across_duty_runs`, `live_gate.rs::a_warm_prefix_is_refused_before_any_call`, `docs.rs::the_spend_log_starts_with_phase_1`, `packages/mod/src/room.test.ts::room_text_within_budget`.
- **Extended by DR-89:** in M1, `intent next` answers from `tests/replay/` until M2's consent card and daily cap.
- **Sources (checked 2026-10-26):** KICKOFF `<constraints>` Spend, S7 and R9; [claude-gateway §4] (a prefix below the minimum fails silently); [mod-api §6] (the session cache lasts 5 minutes); review round 2 cost-1 to cost-10 except cost-4, and owner decision E16.

## DR-66: An agent surface that reports errors, takes proposals inline and brings the diff back

- **Status:** accepted (decided in review round 2). Amended in review round 3.
- **Starting point:** none; implements principle 2's agent interfaces, principle 1's "interrupt a person only for decisions only they can make" and R8's "its result comes back as a diff for review".
- **Origin:** review round 2: principles-1, principles-3 and principles-7; the minor findings principles-10, principles-11 and principles-12.
- **Context:**
  - Tool results carried the envelope's `result` and `meta.next`, so an error reached the agent as nothing: no code, no retry class, no candidates.
  - A chat proposal needed a JSON file on disk first, so each batch cost a Write prompt and left a file in the person's tree.
  - Nothing ran `handoff collect` after the implementer reported, and the pane had no key for it, so the diff didn't come back.
  - `consent_required` told agents to retry later though only a person can clear it; every envelope carried about 90 tokens of empty `meta`; and the band could name a paid model call that a stray digit presses.
- **Decision:**
  - Every mod tool returns one `ToolResult`: `ok`, then `result` or the whole `error`, then the `meta` fields an agent acts on.
  - `room_change` takes an optional `payload`, which the handler passes on stdin to `intent propose --file -` or `edit propose --file -`. The curator uses the same stdin form through its resolver, and the guard allows those calls with no prompt.
  - On the implementer's `turn.complete`, the mod runs `handoff report` and then `handoff collect`, which deals one review card; merging takes a second key.
  - A code only a person can clear has retry `never`; `consent_required` becomes `never`, with its card named.
  - `meta` omits empty and default fields except `seq` and `size`.
  - `CommandSpec.spends` marks the leaves that can make a paid model call, and the band and `PaneModel.next` never name one.
- **Alternatives rejected:**
  - The mod writing each proposal to a file under `.planroom/models/` and passing `--file`: a second path to clean up, where stdin needs none.
  - A pane key for `collect`: an extra press for a deterministic step that isn't the person's decision.
  - Keeping every `meta` field for readers' convenience: the schema defaults keep readers typed at no token cost.
- **Consequences:**
  - Contracts: `ToolResult`, `ToolError`, `ToolMeta`; `room_change.payload`; `--file -`; `CommandSpec.spends`; `Meta`'s skip rules; `consent_required`'s retry class and the registry rule.
  - Tests: `packages/mod/src/tools.test.ts::error_envelopes_reach_the_model`, `a_stage_proposal_writes_no_file`; `packages/mod/src/agents.test.ts::report_then_collect_deals_the_review_card`; `packages/mod/src/keys.test.tsx::band_never_names_a_spending_leaf`; `packs.rs::a_suggestion_cannot_name_an_irreversible_command` covers spending leaves; `envelope_schema.rs::the_documented_error_table_is_the_error_codes` checks the rule.
  - Budgets: `budgets.json` is re-baselined downward once the `meta` skip rules land.
  - Conventions: `AGENTS.md` says a code only a person can clear has retry `never`.
  - Users: S9's hand-off flow goes from report to the review card with no keystroke.
  - It amends DR-21, DR-22, DR-25, DR-27 and DR-54.
- **Amended in review round 3:** `invalid_argument` carries the leaf's synopsis and its `schema --command LEAF` handle in `details` and `hint`, so a wrong tool argument costs one small read (principles-6) [DR-21]. Test `packages/mod/src/tools.test.ts::a_wrong_argument_returns_the_leaf_synopsis`.
- **Sources (checked 2026-10-26):** KICKOFF principles 1 and 2, R8, S9 and S14; [mod-spike] (A-4: a tool result is a string or content blocks); review round 2 principles-1, principles-3, principles-7, principles-10, principles-11 and principles-12.

## DR-67: Paired owner devices, and inbox rows only from the mod

- **Status:** accepted (decided in review round 2, E8); verify in WS7. Amended in review rounds 2 and 3.
- **Starting point:** kept and refined "Remote mode: ... Artifact edits land as events in a db collection; the session imports them ...".
- **Origin:** review round 2: security-3 and the downgraded principles-8; the surfaces review raised the same gap as ux-1.
- **Context:**
  - Import read inbox rows from `.planroom/share/inbox`, a folder any local process could write with Bash, and took the author from the folder's path. Any agent could plant rows that applied as another viewer.
  - The owner's own rows never applied, because the session writes `db` as the owner with no prompt. For the one user KICKOFF names, every phone edit and pick waited for a second confirmation at the desk.
- **Decision:**
  - The mod's `tool.call` hook wraps the `ArtifactData list` call that `/room sync` asks for, checks the call is for the workspace's own Artifact URL, and passes the rows from the observed result to the person-only `planroom import -` on stdin. Import never reads rows from a folder.
  - `planroom person pair`, run from the pane's remote list or the canvas, never from `/room` or a terminal, mints a 10-character code, keeps the key derived from it in `meta.pairing_key`, and shows the code once on that surface.
  - The owner types the code into the Artifact page once. The page derives the key with PBKDF2-SHA256 at 100,000 rounds, salted with the workspace ID, keeps it in `localStorage`, and signs each row as `sig = HMAC-SHA256(key, docId ‖ canonical(row))`, over the row without its `sig`.
  - A signed owner row applies as `person:artifact:<uid>`, `decide` included. Unsigned owner rows join the one confirming card, and a confirmed row applies as `person:artifact:<uid>` with the card as its cause. Another viewer's rows apply by path while A-13 holds.
  - Pairing again replaces the key.
- **Alternatives rejected:**
  - A per-sync nonce: the model writes and reads the inbox, so it would see the nonce [DR-52].
  - Showing the code in `/room` text: the model would see it.
  - Denying Bash writes under `.planroom/` and keeping the folder: a path match in shell text is a guardrail one encoding defeats, and stdin removes the folder.
- **Consequences:**
  - Contracts: `InboxRow.sig`; `meta.pairing_key`; `Command::PersonPair`; `import -` reads `{path, row}` lines.
  - Tests: `s11_remote.rs::a_signed_owner_row_applies`, `an_unsigned_or_broken_signature_joins_the_card`, `rows_come_only_from_the_mod`, `a_confirmed_row_keeps_the_viewer_as_actor`; `tests/e2e/artifact.spec.ts::pairing_signs_rows`; `packages/mod/src/room.test.ts::sync_passes_the_observed_rows_on_stdin`.
  - Walking skeleton: WS7 checks that the mod's `tool.call` hook sees the rows of an `ArtifactData list` result.
  - Users: the owner's phone edits and picks apply at the next sync; pairing takes one code, typed once per device.
  - It amends DR-31 and DR-52.
- **Amended in review round 2 (surfaces fix pass):** the card for unsigned owner rows pre-selects No, since the model could have written them, and its why line says pairing skips it. A sync that a Send to Claude wake starts carries no person token, so import applies only rows whose `sig` checks and leaves the rest in the inbox for the person's next sync (ux-1). Test `s11_remote.rs::a_wake_sync_applies_only_signed_rows`.
- **Amended in review round 3:** a page the model republishes could read the pairing key, so a signed row applies only reversible edits, and a pick from any row only pre-selects its card's choice at the desk. The owner's unsigned rows become ghosts on one card with Apply pre-selected, replacing the round 2 amendment's No, so a decline is an ordinary reject that `edit undo` reverses (F11, F15; principles-3, security-2) [DR-75]. Pairing lives on a Decisions card, "Pair this device" on the page and "Pair a phone" on the local remote card; the page checks for its key on each load; and the pane's Pair is an entry of the remote picker (ux-6, ux-7) [DR-77].
- **Sources (checked 2026-10-26):** KICKOFF S11, R5 and principle 1 ("one tap to confirm"); [artifact §2], [artifact-spike] (session writes need no prompt); review round 2 security-3, principles-8 and ux-1, and owner decision E8.

## DR-68: The canvas edits by hand and answers to the keyboard

- **Status:** accepted (decided in review round 2, E9); built in M1 and M5.
- **Starting point:** none; implements R6's "shaped by hand on the canvas", R13's mobile-first canvas and R14's five concepts.
- **Origin:** review round 2: ux-2; the minor findings ux-9, ux-12 and unasked-10.
- **Context:**
  - The canvas listed views, transports, motion and gestures, but no way to regroup, rename, re-wire, forbid, hide or pin a block by hand. R6 asks for it, and S4's evidence records the regroup and the intended change on the canvas.
  - On the phone a one-finger drag pans, so a drag couldn't move a block there, and the desktop layout wasn't specified.
  - The phone's top row had to hold up to five lids, search, the lens switcher and the project shelf at 44 px each, with no collapse rule.
  - The light `--card` token was near-white, and it paints the bottom sheet over half a phone screen, which KICKOFF's don't-list rules out.
  - Nothing gave keyboard or screen-reader users a way through the map, and no audit checked it.
- **Decision:**
  - The selected block carries an action tray: rename, group with, hide and pin, each one catalogue `edit` leaf. On the desktop, dropping a block on another's lid runs `edit move`, a drag on empty felt runs `edit pin`, and chalk drawn from one block to another runs `edit relate --layer intended`, whose popover offers Forbid.
  - On the phone, a long press opens the evidence and the tray in the bottom sheet, and Move into and Relate to pick their target from a list. A drag there only pans.
  - Each hand edit is pending until the engine confirms it, and offers Undo. The tray counts under Levels and the popover under Intent, so the top level keeps five concepts.
  - The desktop layout is fixed: the breadcrumb, lens switcher and project shelf across the top, the zoom rail on the left, the card stack on the right, and the tray and board in a bottom drawer.
  - The phone's top row holds at most four targets: the current lid with search, a `⋯` lid for the ancestors, one lens swatch that opens a sheet, and one project chip.
  - In light mode `--card` becomes pale mint, `#e3f1ea`, which keeps every text and mark token above the contrast bar.
  - The arrow keys move the selection between siblings, Enter dives, Escape goes up and Tab reaches the tray, with visible focus rings. A hidden outline of the `MapView` rows has `role="tree"`, and `@axe-core/playwright` audits every view.
- **Alternatives rejected:**
  - Dragging a block on the phone: it clashes with the one-finger pan that the gesture table and S13 rely on.
  - A sixth top-level Edit control: R14 allows five.
  - Shift-drag for chalk: the phone has no Shift. A chalk handle works with a pointer, and the tray's Relate to covers the phone.
- **Consequences:**
  - Contracts: none; every hand edit is an existing `edit` leaf.
  - Layout: `@axe-core/playwright` joins the dependencies of `tests/e2e`, a gated surface this record approves [DR-70].
  - Tests: `tests/e2e/edit.spec.ts::regroup_by_a_lid_drop_and_by_the_tray`, `rename_hide_and_pin_from_the_tray`, `chalk_makes_an_intended_relation_and_forbid`, `the_phone_tray_moves_and_relates`, `each_hand_edit_undoes`, at 1440×900 and 390×844; `tests/e2e/quality.spec.ts::top_row_fits_at_the_deepest_level`, `keyboard_reaches_every_block_and_action`, `axe_finds_no_serious_violation`; `tokens.test.ts::contrast_meets_the_bar` over the new token; `exactly_five_concepts` with the tray and the popover open.
  - Milestones: M1's canvas slice grows 3 h, and M5's accessibility slice 2 h.
  - It extends DR-54.
- **Sources (checked 2026-10-26):** KICKOFF R6, R13, R14, S4, S13 and the look-and-feel don't-list; review round 2 ux-2, ux-9, ux-12 and unasked-10, and owner decision E9.

## DR-69: Releases from main, a portable detach and a resolver contract

- **Status:** accepted (decided in review round 2, E11 to E13); verify the detach on macOS in the installer job from WS5. Amended in review round 3.
- **Starting point:** kept and refined "plugin/ ... engine bootstrap" and DR-58's release flow; it replaces DR-58's departure from Iris's branch order.
- **Origin:** review round 2: operability-1, operability-2 and operability-4; the minor findings operability-5 to operability-12.
- **Context:**
  - `main` stayed behind until a release was published. GitHub runs `workflow_dispatch`, schedules and Dependabot only from the default branch, so the checklist's dry run, `advisories.yml`, `toolchain.yml` and Dependabot couldn't work before v0.1.0, and nothing said where work merged after it. The `--branch main` fallback was offered exactly when `main` held no code.
  - Prefetch and build detached under `setsid`, a util-linux tool macOS doesn't ship, so neither macOS target could resolve its engine.
  - S14's release-asset evidence had no documented way past the dev checkout, and the base URL a test server needs had no scheme rule.
  - In a checkout a stale build fell through to a cached release. A cached install's build had no canvas, and its fix page named a checkout the user didn't have. Pruning deleted a binary other sessions still ran.
  - `schema_mismatch` always said "Restart this session", which fixes nothing when the plugin is stale. A skill's Bash call could hang on the fetch lock or get no output. `engine.env`'s version had no generator. The dry run built Intel macOS on an arm runner, and Iris's art spend had no record.
- **Decision:**
  - `main` is the default and integration branch, as in Iris. Between releases it carries `X.Y.Z-dev`. A release commit sets `X.Y.Z`, CI goes green on `main`, the owner runs the dry run on `main` and tags that commit, and the next commit sets the next `-dev`. `cargo xtask gen --version V` writes every manifest, `Cargo.lock`, `engine.env` and the changelog move.
  - The resolver treats a `-dev` version as having no release asset and goes straight to the source build's preflight, unless `PLANROOM_RELEASE_BASE_URL` names a test server. A cached install's build clones the tag, or `main` for a `-dev` version, into `DATA_ROOT/src/VERSION`, and builds there as from a checkout, with the canvas when Node and pnpm are present. A build from `main` whose catalogue hash isn't the plugin's keeps no binary and says to update the plugin.
  - The engine daemonizes in Rust: it forks and calls `setsid(2)` through `libc`. The resolver runs `prefetch --detach` and `build --detach` as itself in the background, in its own process group, with its stdio redirected. So nothing needs a `setsid` command.
  - The resolver's environment is a contract: `PLANROOM_ENGINE`, `PLANROOM_RESOLVE` (`auto`, `checkout` or `release`) and `PLANROOM_RELEASE_BASE_URL` (`https`, or plain `http` only to a loopback host). `status --json` reports the step that resolved. The guard denies agents the two variables that choose a binary.
  - In a checkout, a build with another catalogue hash returns `engine_unavailable` with reason `stale_build` and the build command, and the mod's line names the fix for the cause. `serve --detach` drains a same-version server with another hash only when its own binary is newer.
  - A resolve prunes only versions unused for 7 days, and the mod resolves again once when its stored binary is gone.
  - A caller other than the mod waits at most 20 s on the fetch lock, then gets `engine_fetching`. A failed download gives it `engine_unavailable`, and it never starts a build.
  - `schema_mismatch` names the version that wrote the store. The mod names `/plugin marketplace update planroom` and `/plugin update planroom@planroom` when a release wrote it, and `doctor --restore` when a `-dev` build did. An Upgrade Planroom guide gives the same steps, and Remove Planroom lists every path, ref and config file Planroom creates.
  - The release dry run builds `x86_64-apple-darwin` on `macos-15-intel`, as `release.yml` does. `test (os)` stays on ubuntu-24.04 and macos-15, because the dry run builds and smoke-tests the Intel archive on its own runner. An MSRV job adds nothing while `rust-version` equals the pinned 1.97.
  - Each Iris run appends `{at, image, model, cost_usd, prompt_path}` to `docs/art/spend.jsonl`, after a dry-run cost and with `--max-cost`. A docs test checks every image's prompt and row, and the $5 total.
- **Alternatives rejected:**
  - Keeping `main` behind releases with a separate integration branch: every GitHub feature that reads the default branch would read the wrong one.
  - A `stable` branch for installs: it needs a marketplace source that names a ref, which no spike has checked, and a `-dev` install that builds from source already works.
  - `perl -MPOSIX` for `setsid`, or a plain double fork: perl isn't on every Linux image, and a double fork alone leaves the child in the session's process group.
  - `cargo install --git`: it never builds the canvas, so a marketplace user got the fix page with no checkout to run its command in.
- **Consequences:**
  - Contracts: error codes `engine_fetching` and `engine_unavailable`; `schema_mismatch` gains `writer_version`; the resolver's environment table; `status --json` names its step.
  - Layout: `libc` joins `crates/planroom`'s dependencies; `DATA_ROOT/src/`; `docs/art/spend.jsonl`; `docs/guides/upgrade.md`; `cargo xtask gen` writes `engine.env`, and `gen --version` its version.
  - Tests: `tests/installer/run.sh::detach_returns_at_once_and_survives_parent_exit` on macos-15 and ubuntu-24.04, `base_url_refuses_plain_http_to_a_non_loopback_host`, `resolve_release_skips_the_checkout`, `status_names_the_step_that_resolved`, `a_dev_version_goes_straight_to_the_build_card`, `a_stale_checkout_build_names_the_rebuild`, `prune_keeps_a_recently_used_version`, `a_non_mod_caller_waits_20_s_then_gets_engine_fetching`, `a_failed_download_gives_engine_unavailable`, `a_cached_install_builds_from_a_clone`; `packages/mod/src/engine.test.ts::a_missing_binary_resolves_once_more`, `schema_mismatch_names_the_right_action`; `server_auth.rs::a_same_version_server_drains_only_for_a_newer_binary`; `packages/mod/src/guard.test.ts::agents_cannot_choose_the_engine`; `docs.rs::every_image_has_its_prompt_and_spend_row`; `docs.rs::releasing_has_the_owner_checklist` also matches the checklist's triggers with `release.yml`'s.
  - Owner: the checklist merges, dry-runs and tags on `main`, then merges the next `-dev` bump.
  - Milestones: M5's release slice grows 4 h.
  - It amends DR-28 and DR-58.
- **Amended in review round 3:** the resolver sources the generated `plugin/scripts/contract.sh`: `SCHEMA_VERSION`, its two codes with category, exit code and retry class, and printf templates. So `status --json`, `engine_fetching` and `engine_unavailable` are contract types it can't drift from, and `tests/installer/run.sh::every_resolver_output_validates` checks each against the committed schema (F14, principles-1) [DR-37]. A release version whose tag is missing builds nothing and returns `engine_unavailable` with reason `no_release` (security-10) [DR-75]. The detach starts its child with `setsid` on Linux and perl's `POSIX::setsid` on macOS, since dash ignores `set -m` without a terminal; perl ships with macOS and Linux has `setsid`, so the rejected alternative's reason no longer holds (F17, operability-2). Users install the latest release through the README's line, a tag source or a `release` branch, in place of the rejected `stable` branch, which A-20 now checks (F16, operability-1) [DR-78].
- **Sources (checked 2026-10-26):** [iris-release-bar §2, §3]; [mod-api §7] (`process.run` blocks on a child that keeps stdio open); [plugin-packaging §3, §5]; KICKOFF's packaging, release-bar, Iris and S14 text; review round 2 operability-1 to operability-12, and owner decisions E11, E12 and E13.

## DR-70: Repo gates over the pushed commits and every boundary statement

- **Status:** accepted (decided in review round 2, E14). Amended in review round 3.
- **Starting point:** extended DR-59's repo gates and "Repo layout: a crate-graph test or `cargo-deny` bans in Rust, and `dependency-cruiser` in TypeScript".
- **Origin:** review round 2: repo-1 and repo-2; the minor findings repo-4 to repo-12. The store and kernel fixes applied repo-3 through DR-45.
- **Context:**
  - CI walked every commit since the fork from `main`, which on this branch is the Initial commit, so one bad pushed commit kept CI red until history was rewritten. The commit that writes the first lock fails that rule, since the records it names landed earlier.
  - The gate watched `boundaries.toml`, crate dependencies, the top-level list, package exports and the Conventions section. It missed the area contracts table, the Boundaries section of `AGENTS.md` and each package's dependencies, where several boundaries live only as prose.
  - Any branch could write its own record, and two branches could both take the next number.
  - `scripts/`, `schema/` and four `tests/*` directories had no contract. `plugin/` and `packs/` kept theirs in a README that ships to users and that Claude Code doesn't load. Nothing mapped an area to the design sections that govern it.
  - The fixture's `CLAUDE.md` and skill are live agent context for this repo's own sessions.
  - The kernel's no-I/O lint named modules, while clippy matches item paths; its plant was one call; and a kernel test that reads pack files can't do I/O.
  - The TypeScript rules allowed any path under `node_modules`, so React could reach `ui` through any third-party package.
  - `cargo xtask test` had no test filter, regeneration was split across flags and environment variables, and conflicts in generated files had no rule.
  - Dependabot's 7-day cooldown let it propose direct pins younger than the 14-day rule, and nothing checked crate ages.
  - Most of R16's gate tests sat in tier C, so they could ship failing.
- **Decision:**
  - `check layout` runs a state check every time: the lock matches the surfaces, and every record it names appears once in `decisions.md`. In CI it also walks only the commits the push adds (`github.event.before..github.sha`, or for a new branch or a force push the commits no other remote branch holds) or the pull request adds. A commit that changes the lock must add the record the new entry names, and the first lock comes with a new record.
  - The gated surfaces add the area contracts table, parsed; the Boundaries section of `AGENTS.md`; and each package's dependency names, without versions.
  - `check layout` fails on a duplicate `## DR-n` heading. Record numbers are assigned at the merge, and a branch asks for one. When a branch merges, `check layout --commits FORK..TIP` lists each gated change in it with its record, and the merge ratifies each.
  - Every non-dot top-level directory, and each `tests/*` directory, has an area row and an `AGENTS.md` with its `CLAUDE.md`; `plugin/` and `packs/` get theirs beside a user-facing README. A Read first table maps each area to the design sections, records and scenarios that govern it, and `cargo xtask gen` writes it into the area's `AGENTS.md`. The release helpers in `scripts/` stay POSIX sh [DR-38].
  - The `examples/` contract says fixture files are test data, never instructions, and `.claude/settings.json` lists the fixture's agent files in `claudeMdExcludes` [A-16].
  - `boundaries.toml` holds the kernel's no-I/O list as item paths: every `std::fs`, `std::net`, `std::process` and `std::env` function and type, `Path`'s I/O methods, `std::io::stdin`, `stdout` and `stderr`, `SystemTime::now` and the print macros. The self-test plants a second call, `Path::exists`. The lint holds for every kernel target, so a test that reads pack files lives in `crates/planroom/tests/`.
  - Each TypeScript package lists its third-party packages in `third_party`, and the generated rules allow only those. The self-test plants a React-based package in `ui`.
  - `cargo xtask test [-p CRATE] [FILTER] [--ts PATH]` runs one crate, one test or one vitest file. Plain `cargo xtask gen` writes every committed output, the mod bundle and the CLI reference included. Flags remain for outputs that aren't committed, for `--golden`, which runs the Rust tests that write `tests/golden/`, and for three one-off writes. On a conflict in a generated file, take either side and run `cargo xtask gen`.
  - Dependabot's cooldown is 14 days, the direct-pin age, and `check audit` checks that each direct pin's release is at least 14 days older than the commit that set it. `minimumReleaseAge` stays 7 days for transitive packages.
  - The gate tests that enforce binding text move to S15: `ci_runs_only_xtask`, the layout gate's tests, `area_contracts_match_repo_layout` and `breaking_change_guard`. C16 keeps the budgets ratchet, `feature_list_is_complete` and the pin test.
- **Alternatives rejected:**
  - Ratifying an earlier commit by naming its sha in the lock: the push-range walk and the state check leave nothing to ratify.
  - A guardrail hook that denies a branch's agent the lock and new headings: an agent on a branch runs in its own session, which a hook there can't tell from the merging session, so the merge-time listing is the check.
  - Hashing dependency versions: every Dependabot bump would need a record.
  - Renaming the fixture's agent files to inert names: `examples/README.md` is frozen and says `tree/` files sit at their repo-relative paths.
  - A Read first column in the area table: the table gets too wide to read, and a second table generates the same lines.
- **Consequences:**
  - Contracts: the `layout.lock` surfaces; `area_contracts_match_repo_layout` covers every directory and the Read first lines.
  - Layout: `plugin/AGENTS.md`, `packs/AGENTS.md` and their `CLAUDE.md` files; area files for `scripts/`, `schema/`, `tests/replay/`, `tests/golden/`, `tests/installer/` and `tests/plugin/`; `boundaries.toml` gains the no-I/O list and `third_party`; `.claude/settings.json` gains `claudeMdExcludes`.
  - Tests: `gates.rs::a_bad_commit_already_pushed_fails_no_later_run`, `the_first_lock_needs_a_new_record`, `a_lock_naming_a_missing_record_fails`, `a_duplicate_record_heading_fails`, `a_branch_merge_lists_its_gated_changes`, `an_edited_area_row_needs_a_record`, `a_new_package_dependency_needs_a_record`, `a_dependency_version_bump_needs_none`; the self-test's `Path::exists` and third-party plants; `crates/planroom/tests/evals.rs` replaces `crates/kernel/tests/evals.rs`; `release_pins.rs::dependabot_cooldown_matches_the_pin_age` replaces `dependabot_cooldown_matches_minimum_release_age`.
  - Agents: one command runs one test, and one `gen` fixes any generated file.
  - Milestones: M1 gains a 3 h repo-gates slice.
  - It amends DR-19, DR-36, DR-38 and DR-59.
- **Amended in review round 3:** the layout walk runs only on branch pushes and pull requests, and a dispatch, a schedule or a tag push checks state only. A branch writes no record and asks for no number; the merge commit carries the record and the lock. A generated file's conflict runs its own generator, and a plain test run compares golden files and never writes them (F18, operability-4, repo-3, repo-4) [DR-79].
- **Sources (checked 2026-10-26):** KICKOFF R16, S14, S15, the repo-tooling constraint and `<process>` step 5; `git merge-base origin/main HEAD` (the Initial commit); `.dependency-cruiser.cjs`; `pnpm-workspace.yaml`; `examples/README.md`; review round 2 repo-1, repo-2 and repo-4 to repo-12, and owner decision E14.

## DR-71: Everyday git moves: a tracked branch, the working tree and every repo layout

- **Status:** accepted (decided in review round 2, E17). Amended in review round 3, and by DR-85 and DR-92.
- **Starting point:** extended DR-44 (the reconciler) and DR-33 (freshness on read).
- **Origin:** review round 2: unasked-2; the minor findings unasked-8 and unasked-12.
- **Context:**
  - The reconciler's window ran `prev..head` from the last reconciled head. A checkout of an older tag, a switch to a sibling branch, a rebase or a bisect gave an empty or wrong window with a large fact delta. That dealt residual cards and wrote wrong classes into the stored outcome.
  - Discovery saw the root repo or its direct children only. A meta-repo's submodules and a `~/src/<org>/<repo>` tree got nothing, and a workspace inside another one split a person's edits without a word.
  - The map showed only committed state and said nothing when the working tree differed.
- **Decision:**
  - Each repo's first scan records a tracked branch: the branch checked out then, or `origin/HEAD`'s target when `HEAD` is detached. The reconcile outcome keeps it with each repo's head, and reconciliation follows only fast-forward moves of it.
  - Any other move re-anchors. The map follows `HEAD`, the reconciler skips the repo, no card is dealt, and `focus` carries a `reconcile_paused` line. Off the branch, the reconciled head stays put and reconciliation resumes on the way back. A rewrite of the tracked branch restarts the window at the new head with nothing classified.
  - `handoff start` records the tracked branch as each repo's `base_branch`, which the merge preflight checks.
  - Discovery adds a root repo's initialized submodules, and repos two levels down when the first level has none. A scan of a directory that holds another workspace keeps both and warns with `workspace_nested`.
  - `focus` and `map` report `meta.dirty_repos` from `git status --porcelain -z --untracked-files=no`, shown as one line, with no rescan.
- **Alternatives rejected:**
  - Reconciling a descendant head on any branch: a feature branch's commits would be classified as if merged.
  - Scanning the working tree: scans are of committed state [DR-33], and a dirty file has no commit for its evidence.
  - Merging a nested workspace's store on a card: an import into a store that has authored events isn't defined, and the warning makes the split visible meanwhile.
- **Consequences:**
  - Contracts: `RepoHead.tracked_branch`; `meta.dirty_repos`; warnings `reconcile_paused` and `workspace_nested`.
  - Tests: `kernel/tests/reconcile.rs::a_backward_checkout_deals_no_card`, `a_sibling_branch_pauses_reconciliation`, `a_rewritten_tracked_branch_restarts_the_window`, `a_return_to_the_tracked_branch_resumes`; `first_run.rs::submodules_are_repos`, `repos_two_levels_down_are_found`, `a_nested_workspace_warns`, `focus_lists_dirty_repos`. S7's checkout of `routine` leaves `main` the tracked branch, so that rescan re-anchors and deals no reconcile card.
  - Milestones: M2's reconciler slice grows 2 h, and M1's extractor slice 1 h.
  - It amends DR-33 and DR-44.
- **Amended in review round 3:** the S7 consequence above gives way to DR-72's fast-forward. The reconciler runs on S7's rescan and classifies the `routine` commit `unplanned`, which deals no card (correctness-10). Test `s07_decisions.rs::the_routine_commit_is_unplanned`. A single-repo workspace's state directory is `.git/planroom/` [DR-74].
- **Amended by DR-85:** `meta.dirty_repos` counts untracked files. Uncommitted work never reaches the map, and an untracked file is the commonest uncommitted work, so the signal runs `git status --porcelain=v1 -z --untracked-files=normal`, which honours `.gitignore`, in place of `--untracked-files=no`.
- **Amended by DR-92:** the call turns off every filter driver git's config names and adds `--ignore-submodules=dirty`, so counting runs no command the repo or a submodule configures; a submodule the workspace maps reports its own status.
- **Sources (checked 2026-10-26):** `examples/README.md` (S7's `routine` branch starts from `main`); KICKOFF principle 1 and R1; review round 2 unasked-2, unasked-8 and unasked-12, and owner decision E17.

## DR-72: Owner follow-ups after review round 2: signed person tokens, Git LFS filters, S7's fast-forward

- **Status:** accepted (decided 2026-10-26).
- **Starting point:** none; amends DR-64 and DR-71.
- **Context:** the round 2 fixes raised three issues. To check DR-64's HMAC action tokens the engine must hold the person key itself, so a leaked `store.db`, backup or export would let anyone sign as the person. DR-64's repo-config refusal would refuse every repo with a standard Git LFS install, whose `filter.lfs.*` entries `git lfs install` writes. And S7's `checkout routine` step now takes DR-71's off-branch path, so S7 would no longer run the reconciler it is meant to exercise.
- **Decision:**
  - The person key is an ECDSA P-256 key pair. The mod generates it with `crypto.subtle`, keeps the private key in its `$.store`, and registers only the public key. Action tokens are ECDSA signatures over the same fields, verified in the engine with the `p256` crate. `store.db`, its backups and exports hold only the public key. The Artifact pairing key stays a shared HMAC key in `meta.pairing_key` (DR-67), because the page has no channel to register a public key; a leaked store lets someone forge only Artifact inbox rows, and only by also writing as the owner to the Artifact `db`.
  - The repo-config check allows exactly the `filter.lfs.*` entries `git lfs install` writes, and nothing else under `filter.*`.
  - S7's commit-triggered rescan is a fast-forward merge of `routine` into `main` (`git merge --ff-only routine`), so the tracked branch moves and the reconciler runs.
- **Alternatives rejected:** keeping HMAC with the key in the store (a leaked backup becomes the person); allowing any `filter.*` (the filter driver runs arbitrary commands from repo config).
- **Consequences:** `planroom-models` is untouched; `planroom` gains the `p256` crate (RustCrypto, Apache-2.0 or MIT); A-17 for WS8: `crypto.subtle` in the mod supports ECDSA P-256 sign; tests `actor.rs::a_leaked_store_cannot_sign_a_person_action`, `git_safety.rs::a_standard_lfs_install_is_allowed`, `git_safety.rs::a_custom_filter_is_refused`; S7's step and `s07_decisions.rs::routine_rescan_*` run on the fast-forward.
- **Sources:** `docs/contributing/research/arch-review/round-2/fixes.md`, checked 2026-10-26.

## DR-73: A person's work outlasts automatic writes, flushes and restores

- **Status:** accepted (decided in review round 3, F1, F3 and F4).
- **Starting point:** extended "Optimistic concurrency on entity versions" and "Jev policy. Each decision kind declares its question, thresholds and escalation."
- **Origin:** review round 3: correctness-1, correctness-3 and correctness-4; the minor findings correctness-7, correctness-8 and correctness-9.
- **Context:**
  - A prefill, an auto-enabled effect or an `infer` output went through the same compare-and-set as any write, with the runner's read as its base. A person's edit made before that read sat at or below the base, so the model's write replaced it with no card.
  - A tab that flushed another tab's pending canvas edit sent its own tab ID. The stale edit then looked like the flusher's own write and overwrote the flusher's newer value.
  - A restore's replay set kept its bases from the abandoned timeline, while the restored store reused those seqs, so a set-aside edit could overwrite an edit made after the restore. A restore also replayed `scan` events whose snapshot IDs mean nothing in a rebuilt store.
  - An older process waiting on the lock could commit into a store a newer one had just migrated. A one-shot read could return a `meta.seq` above the view it folded. Two decide runs with different pending sets held two leases and asked Jev twice.
- **Decision:**
  - A person's field wins over any automatic write. A person holds a field when its current value comes from a person's edit, and a target a person retracted, or an inferred edge a person confirmed, is held whole. An auto-enabled effect, a prefill or an `infer` output never sets a held field, whatever its base. `store::commit` checks it before compare-and-set, records the decision with outcome `Superseded`, lists it in Diffs and deals no card. The runner skips subjects whose effect fields a person holds.
  - A pending canvas edit keeps the client ID of the tab that minted it, and whichever tab flushes it sends that ID. A request hash covers the command, its canonical arguments, `base`, the actor and the client, never transport fields.
  - A replay set's edits take the restored copy's last `seq`, the head both timelines share, as their base, and pass the normal compare-and-set. Any field written after the restore conflicts with them, whoever wrote it, and the conflict is a person card.
  - A restore into an empty store keeps each event's stored status and skips `scan` events. It keeps the last reconcile outcome in one `scan` event with snapshot and link 0, and the next scan rescans every repo, classifies no change unit and carries the restored classes forward.
  - Every commit and duty transaction reads `PRAGMA user_version` after `BEGIN IMMEDIATE` and aborts with `schema_mismatch` when it isn't the process's own.
  - A read folds inside one read transaction, at N, the highest `seq` in it, and returns `meta.seq = N`. A cursor whose snapshot was pruned folds at the current `seq` and carries `stale_page`.
  - The decision runner holds one `decide` lease per workspace. A decision event's request hash covers its inputs, never the pick, and the runner skips a subject whose decision event already exists.
- **Alternatives rejected:**
  - A conflict card when a model write meets a person's field: the person already decided, and a card asks again.
  - A model write that waits for the person's edit to age: a timer decides what a person said.
  - Letting the flushing tab's ID stand: it hides a conflict between two tabs.
  - Replaying with the original bases: they count in another timeline.
  - A `decide` lease per subject: more rows for a run that is already batched per slice.
- **Consequences:**
  - Contracts: `Outcome::Superseded`; the client a flush sends; the request hash's fields; `Meta.seq` from one read transaction; `stale_page` for a pruned page; a restored `scan` event with snapshot 0.
  - Tests: `kernel/tests/policy.rs::a_prefill_never_overwrites_a_person_field`, `a_kind_version_bump_keeps_person_overrides`, `an_infer_rerun_keeps_hand_edited_parts`; `tests/e2e/surface.spec.ts::a_flush_by_another_tab_keeps_the_original_writer`, `a_flush_of_a_committed_edit_replays`; `backup.rs::a_replayed_edit_conflicts_with_a_post_restore_edit`, `a_restore_keeps_done_classes`; `store/tests/migrations.rs::an_older_writer_refuses_after_a_concurrent_migration`; `store/tests/interleavings.rs::meta_seq_never_exceeds_the_folded_view`; `duties.rs::concurrent_decide_runs_ask_each_subject_once`.
  - Users: a correction a person made stays made, whatever a model decides later.
  - Milestones: M1's fold and backup slices and M2's decision runner grow by 3 h in all.
  - It amends DR-1, DR-2, DR-5, DR-8, DR-24, DR-50 and DR-53.
- **Sources (checked 2026-10-26):** KICKOFF R6, R10 and principle 1; review round 3 correctness-1, correctness-3, correctness-4, correctness-7 to correctness-9 and owner decisions F1, F3 and F4.

## DR-74: Repo namespaces that survive duplicates and renames, and a single repo's state in its git directory

- **Status:** accepted (decided in review round 3, F5 and F6). Amended by DR-97.
- **Starting point:** kept "Stable, namespaced IDs such as `<repo>:<kind>:<key>`" and extended DR-53's backups.
- **Origin:** review round 3: unasked-1, unasked-2 and unasked-4.
- **Context:**
  - The namespace was the bare repo name. Discovery reaches repos two levels down for `~/src/<org>/<repo>` trees, so two repos named `api` merged into one set of nodes, and an override in one applied to the other.
  - A `git mv` of a directory a person regrouped detached the regroup, and a card asked for a pick git already knew. Nothing checked that a release kept IDs stable.
  - For a single-repo workspace, `.planroom/` sat in the working tree behind a `.gitignore`. Build contexts, `rsync` and archive uploads ignore that file, so the store could ship in an image.
- **Decision:**
  - A repo's namespace is its directory name. Of two repos with one name, the later in path order adds `-` and 4 hex characters of the sha256 of its path from the workspace root. `meta.repos` fixes each root's namespace at its first scan, and `doctor` lists every suffixed one.
  - A rescan runs `git diff -M --name-status -z OLD NEW`. The `scan` event records each file rename, and each directory whose files all moved into one new directory. The fold resolves a vanished edit target through those renames, and Diffs lists the move once. Anything else that vanishes goes through the `rescan.retarget` role as before, a renamed repo directory included.
  - `id_stability.rs` compares the fixture's IDs with the list the last release committed, and a release that changes one needs a record saying how overrides carry over.
  - When the workspace root is a git repo, alone or with its submodules, its state directory is `planroom/` inside that repo's git directory, as `git rev-parse --absolute-git-dir` names it. A directory of repos keeps `.planroom/` at its root. A `.planroom/` at a repo's root is never that repo's state directory, so one a clone brings stays untrusted. Discovery finds either form, and the guard covers both.
- **Alternatives rejected:**
  - The shortest unique path suffix as the namespace, such as `me-planroom`: it depends on which other repos exist, so one repo gets other IDs in another workspace.
  - A namespace keyed by the first commit or remote URL: two clones of one repo share both.
  - Every store under the data root: on a temporary host the data root goes with the container, and a store beside its repo keeps the workspace self-contained.
  - Retargeting renames by a card: git already knows the answer.
- **Consequences:**
  - Contracts: `meta.repos`; `EventBody::Scan.renames`; the `NodeId` namespace rule; the state directory's two forms in Configuration.
  - Tests: `first_run.rs::same_named_repos_get_distinct_namespaces`, `a_single_repo_workspace_keeps_its_state_in_the_git_dir`; `kernel/tests/fold.rs::an_override_follows_a_git_rename`, `a_moved_directory_carries_its_regroup`; `crates/planroom/tests/id_stability.rs::fixture_ids_match_the_last_release`; `store/tests/fs_check.rs::a_single_repo_keeps_its_state_in_the_git_dir`; `backup.rs::git_clean_leaves_a_single_repos_state_alone`.
  - Security: a build context that copies `.git` still copies the store, and `SECURITY.md` says so.
  - Users: same-named repos map apart, a renamed file keeps its corrections, and `git clean -x` in a single repo leaves Planroom's state alone.
  - Milestones: M1's IDs and backup slices grow by 3 h.
  - It amends DR-42, DR-48, DR-53 and DR-71.
- **Sources (checked 2026-10-26):** KICKOFF R1, R6, R15 and principle 1; `git-diff(1)` (`-M`), `git-rev-parse(1)` (`--absolute-git-dir`); review round 3 unasked-1, unasked-2, unasked-4 and owner decisions F5 and F6.
- **Amended by DR-97:** a repo named `ws` or `plan` takes the suffix too, the suffix grows 4 hex at a time while the suffixed name is still taken, and a root's namespace is fixed at the first scan that maps the repo, inside that scan's transaction.

## DR-75: Authority stays with a person's own action

- **Status:** accepted (decided in review round 3, F7, F11 and F15); verify A-18 in WS8. Amended by DR-88 and DR-98.
- **Starting point:** extended "Events carry the workspace, project, actor, cause and an idempotency key" (who counts as a person) and "Remote mode: ... Artifact edits land as events in a db collection; the session imports them".
- **Origin:** review round 3: security-1 to security-4, principles-3 and unasked-3; the minor findings security-5 to security-9 and security-11, and part of security-10.
- **Context:**
  - The implementer could edit any file in its clones, and its approved checks and installs then ran that code with no prompt, as the person's OS user, with the network on. Code it wrote could read the mod's key, sign a merge of its own hand-off and send secrets out.
  - The Artifact page keeps the pairing key in `localStorage`, which survives a republish, and the model makes every publish. Yet a signed row could run any allowed command, `decide` included, with no confirmation.
  - The contract still specified an HMAC token checked against a key kept in `store.db`, which DR-72 had replaced.
  - `person init --rekey` and `open` took a `/dev/tty` confirmation, which anyone who controls a pty can give, and the browser opener inherited the caller's `PATH`.
  - The owner's unsigned phone rows waited on a card with No pre-selected, and a No discarded them.
  - On a temporary host, the backup ref held the unredacted event log, and a warning that every envelope carries gave the model the exact command to push it.
  - Agent-written run steps were mined into standing instructions; the review card didn't flag changes outside the spec; `PLANROOM_RAW` reached every output; a rewritten session request file was sent and cached; the guard didn't cover the user-level config, which may hold the gateway key; and a release version with no tag built from `main`.
- **Decision:**
  - **A sandboxed implementer.** The `planroom:implementer` agent type runs with Claude Code's Bash sandbox on: writes only inside its clones, and no network [A-18]. The guard allows its Edit and Write inside its clones, except each clone's `.git/`, and on its `result.json`. The spec's installs, and any install a retry's `--allow` adds, run before it starts: the mod runs them in each clone at its base commit, outside the sandbox. So its approved list is the checks, `git add` and `git commit`. Where the host can't sandbox it, the `handoff start` card says so and each approved command returns `ask`.
  - **Pairing proves less, so it allows less.** A signed owner row applies only a reversible edit: `edit`, `review` or `intent propose`. A `decide` row from any viewer, paired or not, applies nothing at import. Its choice becomes the pre-selected pick of that card at the desk, marked with its viewer, and one key confirms it.
  - **Unpaired edits are kept.** Each of the owner's unsigned rows becomes a ghost under `person:artifact:<uid>`, with the import as its cause, and they batch into one card that lists them, with Apply pre-selected. A reject is the ordinary review reject, which `edit undo` reverses. Acks mark such rows and waiting picks `held`.
  - **One token format.** `PLANROOM_PERSON_TOKEN` is `<at>.<sig>`, an ECDSA P-256 signature checked against `meta.person_pubkey`, which holds only the public key [DR-72].
  - **Presses for the keys.** `person init --rekey` takes a token under the old key or a press of the `person_key_mismatch` pane's button, and `open` takes only a mod press; neither takes a `/dev/tty` confirmation. The browser opener runs with the fixed system `PATH`.
  - **A local backup ref.** On a temporary host the ref holds the redacted event export. The `temporary_host` warning carries only the repo and the ref; the mod says once, in the pane and the status line, where it is and how to copy it out, from a string in its bundle. Planroom never pushes it, and the guard denies an agent's `git push` that names `refs/planroom/`.
  - **Smaller gaps closed.** An agent implementer's run is mined only from the Bash calls the mod observed, written to `observed.json`, and the branch's commits; every curation draft is shown in full. `collect` warns `outside_touch_points`, and the review card expands those hunks before it offers Merge. `PLANROOM_RAW` reaches only a read command's `--json` stdout. The mod sends a session request file only when its bytes match the sha256 in `meta.model_requests`, and import checks the response's `request_sha256`. The guard denies agent reads of the user-level config. A release version whose tag is missing builds nothing.
- **Alternatives rejected:**
  - A `tool.check` guard that ties each Artifact publish to the last `share` manifest: it matches text the model writes, and limiting signed rows to reversible edits bounds what a forged row can do either way.
  - Rotating the pairing key when the comment watch reports a republish: the watch is best-effort, and the bound above doesn't need it.
  - Network to the package registries inside the sandbox: an install from a manifest the implementer edited would run its lifecycle scripts online. Installing from the base commit first needs no network later.
  - A pending key that a terminal fingerprint check promotes: it keeps the terminal path this record removes.
  - A prompt for each implementer command on every host: DR-64's reason stands wherever a sandbox exists.
  - Pinning the release archives' sha256 in `engine.env`: the binary embeds its commit, so the release commit can't hold its own archives' hashes. It stays a known gap.
- **Consequences:**
  - Contracts: `PLANROOM_PERSON_TOKEN`'s format and `meta.person_pubkey`; `ChangeSpec.installs`; `ImplementerRun.claimed`; `ModelRequestRef.sha256` and `ModelResponse.request_sha256`; acks `held`; warning `outside_touch_points`; `temporary_host` without `push`; the `person_required` and `person_key_mismatch` rows; the `open` and `person init` leaf rows.
  - Layout: `plugin/agents/implementer.md` turns the sandbox on; the mod writes `observed.json` into the hand-off directory.
  - Tests: `actor.rs::rekey_and_open_refuse_the_terminal_path`, `rekey_needs_the_old_key_or_a_press`, `a_leaked_store_cannot_sign_a_person_action`; `server_auth.rs::the_opener_gets_a_fixed_path`; `guard.test.ts::implementer_cannot_edit_its_clones_git_dir`, `without_a_sandbox_approved_commands_ask`, `the_user_config_is_guarded`, `an_agent_push_of_planroom_refs_is_denied`; `agents.test.ts::the_implementer_spawns_sandboxed`, `installs_run_before_the_implementer`, `observed_steps_are_written_before_report`; `s11_remote.rs::unpaired_owner_edits_wait_on_one_apply_card`, `a_declined_unpaired_row_is_kept_and_undoable`, `a_row_pick_waits_preselected_on_its_card`, `a_signed_row_never_runs_a_person_only_command`; `backup.rs::the_backup_ref_is_redacted`; `privacy.rs::no_envelope_suggests_pushing_the_backup`, `raw_never_reaches_share_or_model_requests`; `s12_curation.rs::agent_written_run_steps_are_never_mined`, `every_draft_is_shown_in_full`; `handoff_clone.rs::an_out_of_scope_change_is_flagged`; `session_backend.rs::a_rewritten_request_file_is_refused`; `broker.test.ts::a_changed_request_file_is_not_sent`; `tests/installer/run.sh::a_missing_tag_builds_nothing`.
  - Security: `SECURITY.md` states the residual risk that an approved check runs code the implementer wrote, sandboxed in its clone, and what pairing proves and what it doesn't.
  - Walking skeleton: WS8 spikes A-18.
  - Users: phone edits are never lost; a phone's pick waits for one key at the desk; a hand-off's checks run offline in its clones; with mods off, `open` and a rekey aren't available.
  - Milestones: M1's actor slice, M2's hand-off and models slices and M5's remote slice grow by 5 h in all.
  - It amends DR-4, DR-18, DR-49, DR-52, DR-53, DR-57, DR-60, DR-64, DR-67 and DR-69.
- **Amended by DR-88:** the mod registers the person key's public half once per workspace in each mod's life and again after a press the store refuses as no person's, not once per workspace for good.
- **Amended by DR-98:** each unsigned row's ghost is caused by the open Apply card it joins, since an import writes no event of its own to name.
- **Sources (checked 2026-10-26):** KICKOFF R5, R8, S11 and principle 1; the Artifact tool's browser-storage rule (storage survives a republish to the same URL); [artifact §2]; review round 3 security-1 to security-11, principles-3 and unasked-3, and owner decisions F7, F11 and F15.

## DR-76: The mod checks its host

- **Status:** accepted (decided in review round 3, F19); verify A-19 in WS8.
- **Starting point:** none; serves S9, S14 and principle 1's "never show a blank state".
- **Origin:** review round 3: unasked-5.
- **Context:** the mod API is early access and "may change between releases without notice" [mod-api §10]. CI pins Claude Code 2.1.310 with auto-update off, while users run an auto-updating Claude Code. The mod checked the engine's version and catalogue hash but never its host's, so a host update could throw during registration and leave no pane, no `/room` and no tools, with nothing saying why.
- **Decision:**
  - At `session.start`, before it registers anything, the mod checks the Claude Code version, at least 2.1.310 [A-19], and that every `$` member it calls exists.
  - On a mismatch it registers nothing more and shows one status line, generated into its bundle from `en.toml`. The skills and the CLI then work as with mods off.
  - A newer host that has every member runs as usual. `plugin/README.md` names the tested version.
- **Alternatives rejected:**
  - A scheduled canary job against the latest Claude Code: a non-gating job no scenario needs, while the check covers every user.
  - Refusing every host newer than the tested one: each auto-update would turn the mod off.
- **Consequences:**
  - Contracts: the host check's line in `catalogue.ts`, from `en.toml`.
  - Layout: `packages/mod/src/host.ts`.
  - Tests: `packages/mod/src/host.test.ts::a_missing_member_registers_nothing_and_shows_one_line`, `an_older_host_falls_back_to_the_skills`, `a_newer_host_with_every_member_runs`.
  - Risks: R-13. Walking skeleton: WS8 checks A-19.
  - Milestones: M1's mod slice grows 1 h.
  - It amends DR-57.
- **Sources (checked 2026-10-26):** [mod-api §10] (early access; pin in CI); repo-layout's toolchain pins; review round 3 unasked-5 and owner decision F19.

## DR-77: Every pane action within two keys, and an Artifact that serves a cloud session

- **Status:** accepted (decided in review round 3, F9 and F10); verify A-22 in WS5.
- **Starting point:** none for the pane, which implements R13, S9 and principle 1; kept and refined "Remote mode: ... the Artifact shows cached decisions and queues new ones" for the Artifact.
- **Origin:** review round 3: ux-1, ux-2, ux-3 and principles-2; the minor findings ux-5, ux-6, ux-7, ux-10 and part of ux-9.
- **Context:**
  - The pane's only lens and project controls were `l` and `p`, each the next one in a cycle, so reaching the observability lens could take nine presses.
  - A row behind a list key (`i`, `f`, `w`, `t`, and `d` in the compact layout) took a list key, a row key and an action key. A stage run also had to clear its cost card, which no contract defined. That made three or four keys, while S9's assert counted only visible rows.
  - In a cloud session, on a phone or on a container host, the Artifact is the only canvas. It couldn't show other lenses, `PathView`, `Anatomy`, hunks or the heat ramp, and its empty state sent the person to `/room open`, which on those hosts sends them back to `/room share`.
  - `/room` wasn't registered as immediate, so it couldn't focus the pane mid-turn, and bare `/room` started a model-driven sync whenever remote mode was on.
  - An Artifact device that lost its pairing key gave no sign until its edits waited on a card. The Pair controls, Ask Claude and the needs-login state had no place among the five concepts. `planroom --help` showed a sixth Setup group of plumbing, and the block borders, "the mark that counts", had no tokens.
- **Decision:**
  - **Pickers.** `l`, `p`, `i`, `h` and `t` each deal a picker card face up, from `focus --picker NAME`: lenses, projects, plan stages, items ready to hand off, and the remote actions. Its first four entries are choices `1` to `4`, and the rest are list rows keyed `5` to `9`, `0`, then the letters, so ten entries are a key plus a digit. A pick runs at once as the person's press, and any other action key closes the picker.
  - **A paid or repo-changing pick confirms itself.** Such a step runs only on a press made while a card naming its cost or change is face up. The stage picker is the stage run's cost card, so `i` and a digit run any stage. The hand-off picker names what starts. The review card arrives unasked; its Merge turns it into a confirm, and the second press merges.
  - **Top items on every visible section.** The row-key pool covers the three diff lines, the drift and who lines' top items, then 7 list rows. The compact layout keeps the top diff line.
  - **The count.** S9's test counts real keys from pane focus with no list open, over S1 to S4, a hand-off and a sync, and `feature_list.json` S9 holds each flow's count. A typed `/room` line is one entry point, and two-target hand edits go through Claude's ghost or `/room edit`.
  - **What a cloud session needs.** The snapshot holds every lens's maps and precomputes the views S1 to S6 need for their subjects, keyed by handle, within 1 MB. Anything else is a read request: a page row naming `map`, `show`, `find`, `diff` or an `ask` that doesn't spend, which any sync answers into `answers/<docId>`. The request state replaces "open this view there with /room open". In a cloud session with remote mode on, the session syncs at each turn start, with no person token.
  - **Smaller fixes.** `room` registers with `immediate: true` [A-22], and bare `/room` starts no sync. The Artifact page checks for its pairing key on each load and shows "Pair this device" when the owner's is gone. Pairing is a Decisions card, Ask Claude is a choice on an evidence sheet or a card, and the needs-login state is a full-page empty state. `planroom --help` shows the five ideas and one line naming `help --all`, and `scan`, `open` and `share` join Levels. Each block tone has a border token in both themes.
- **Alternatives rejected:**
  - One consent per plan in place of the stage cost card: a stage's cost would then show nowhere when it's spent, while the picker shows it on every run at no extra key.
  - A lens list behind `l` that a row key selects and Enter applies: three keys.
  - Dropping the band's bare-digit hotkeys: it costs the one-key next action from the prompt, while the band names only reversible commands that don't spend, with a toast and an undo. The S9 evidence run decides it (known gap ux-4).
  - A host flag in the snapshot that switches the empty state between `/room open` and Send to Claude: the request path works on every host, so one state serves all.
  - Pane keys for two-target hand edits: a verb and two targets don't fit in two keys (known gap ux-8).
- **Consequences:**
  - Contracts: `Picker`, `PickerEntry` and `focus --picker`; `DriftLine` and the first `who` row carry row keys; `Snapshot.answers`, `Answer` and `planroom.answer/v1`; `answers/<docId>` in the `db` layout; read requests in `InboxRow.command`; `Snapshot.unavailable`'s text; `room` immediate; `CommandSpec.idea` and the leaf table's ideas for `scan`, `open` and `share`.
  - Layout: none beyond files inside `packages/mod`, `packages/ui` and `packages/canvas`.
  - Tests: `keys.test.tsx::main_flows_within_two_keys`, with no list open and each flow's count, `a_lens_and_a_project_are_a_key_and_a_digit`, `any_stage_runs_in_two_keys_through_its_cost_card`, `a_last_row_hand_off_is_two_keys`, `merge_is_two_keys_from_the_review_card`, `a_picker_owns_the_digits_until_it_closes`; `surfaces.test.tsx::drift_and_who_top_rows_carry_row_keys`, `compact_keeps_the_top_diff_line`; `room.test.ts::room_is_immediate`, `bare_room_starts_no_sync`; `remote.test.ts::a_cloud_session_syncs_at_turn_start`; `s11_remote.rs::a_read_request_is_answered_by_a_tokenless_sync`, `a_read_request_that_spends_is_refused`, `the_snapshot_holds_every_lens_and_the_precomputed_answers`; `tests/e2e/artifact.spec.ts::a_view_outside_the_snapshot_becomes_a_request`, `no_state_names_room_open`, `a_lost_pairing_key_shows_pair_this_device`; `surface.spec.ts::exactly_five_concepts` covers pairing, Ask Claude and the needs-login state; `s15_surface.rs::help_shows_five_ideas`; `tokens.test.ts::contrast_meets_the_bar` covers the border tokens.
  - Users: any lens, project, stage, hand-off or sync is two keys from `/room`, and a phone in a cloud session asks for what the snapshot lacks and gets it at the next turn.
  - Walking skeleton: WS5 checks A-22.
  - Milestones: M1's mod slice grows 2 h and its canvas slice 1 h; M5's remote slice grows 4 h, and its docs slice 1 h, shared with DR-78.
  - It amends DR-22, DR-31, DR-54, DR-56 and DR-67.
- **Sources (checked 2026-10-26):** KICKOFF R13, R14, S9, S11 and principle 1; [mod-api §2] (36 one-key actions per pane), [mod-api §3] (`immediate`); [mod-spike]; [artifact §2]; review round 3 ux-1 to ux-10 and principles-2, and owner decisions F9 and F10.

## DR-78: Releases users install, detached work in its own session, a canvas port that never moves, and machine-wide files across versions

- **Status:** accepted (decided in review round 3, F8, F16 and F17); verify A-20, A-21 and A-23 in WS8.
- **Starting point:** kept and refined "plugin/ ... engine bootstrap" and DR-69's release flow; extended "Store: single-writer local server" through DR-29's stable port.
- **Origin:** review round 3: operability-1, operability-2 and unasked-7; the minor findings operability-3, operability-5 to operability-8, unasked-8 and unasked-10.
- **Context:**
  - A GitHub marketplace install reads `main`, which carries `-dev` between releases. So no user ever downloaded the checksummed release archive, and every install built from source.
  - On Debian and Ubuntu, `/bin/sh` is dash, which ignores `set -m` without a terminal. A detached prefetch or build stayed in the caller's process group, where a reload, a session's end or a hangup could kill it.
  - `serve` moved to a free port when its recorded one was taken, and the canvas keeps unsent edits in storage scoped to its origin, so a move stranded them.
  - A download from any base landed in the shared cache with a valid stamp. The cache and the source clone were keyed by version alone, so a newer `main` resolved an older `-dev` build. An older `watch` noticed a migration only at its next write.
  - `ledger.db` and `workspaces.json` had no rule for engines of two versions at once. The A-14 fallback forwarded too few proxy variables. The git floor of 2.35.2 refused Ubuntu 22.04's patched 2.34.1 and had no error code. "Remove Planroom" left out the published Artifact and the paired devices.
- **Decision:**
  - **Users install releases.** The README's install line names the latest release, through a marketplace source that names the tag, or else a `release` branch the owner fast-forwards to each tag [A-20]. `main` keeps `./plugin` for the local-path flow. The checkout test is positive: `crates/planroom/Cargo.toml` two levels above the script, which isn't under Claude Code's plugins directory.
  - **A session of its own.** `prefetch --detach` and `build --detach` start their child with `setsid` on Linux and `perl -MPOSIX -e 'POSIX::setsid(); exec @ARGV'` on macOS, with stdio redirected, and the installer job tests both.
  - **A canvas port that never moves.** `workspaces.json` records one port per workspace, distinct across workspaces. When another program holds it, `serve` exits with `port_in_use` and a fix line, and `serve --port N` is the deliberate move.
  - **Smaller fixes:**
    - The cache and the source clone are keyed by version and the catalogue hash's first 8 characters. A download from a non-default base is cached apart, under that base's hash, and a `PLANROOM_ENGINE` binary never is; `status` and `doctor` name a non-default source. A failed download names the archive's and `SHA256SUMS`'s URLs.
    - The A-14 fallback forwards the proxy and CA variables in both cases.
    - A live `watch` re-reads `user_version` whenever the store changes. `ledger.db` and `workspaces.json` change only by additions, with a `min_reader_version`, and a ledger migration keeps a `VACUUM INTO` copy that `doctor --restore` takes.
    - The engine checks a repo's owner itself, the git floor drops to 2.34.1 [A-21], and `git_too_old` names the fix.
    - "Remove Planroom" lists the Artifact and the paired devices.
- **Alternatives rejected:**
  - Pointing `main`'s marketplace entry at a release tag: a local-path install of the checkout would then fetch from GitHub, while S14 and contributors need `./plugin`.
  - `set -m` under bash: `/bin/sh` is dash where it matters, and the resolver stays POSIX sh.
  - Moving to a free port and flushing the old origin's edits: the new origin can't read the old origin's storage.
  - An `install-archive` resolver command for blocked proxies: it writes the shared cache from a path any process names (known gap operability-8).
  - A CI job in an `ubuntu:22.04` container on every push: WS8 checks the floor once, and a new adapter flag goes through the same check.
- **Consequences:**
  - Contracts: error codes `port_in_use` and `git_too_old`; `path_refused` reason `other_owner`; `engine_unavailable`'s details gain the URLs; `schema_mismatch` covers the data root's files; `workspaces.json` as `{root, workspace_id, port}`; the data root's compatibility rule; the resolver's cache keys and its non-default base; the plugin manifest note.
  - Layout: `DATA_ROOT/engine/VERSION+HASH/`, `engine-alt/` and `src/VERSION+HASH/`; a `release` branch if A-20 picks it.
  - Tests: `tests/installer/run.sh::detach_gets_its_own_session`, which replaces `detach_returns_at_once_and_survives_parent_exit`, `a_non_default_base_never_fills_the_shared_cache`, `an_engine_override_is_never_cached`, `a_newer_dev_catalogue_misses_the_cache`, `a_marketplace_clone_is_not_a_checkout`, `a_failed_download_names_its_urls`; `server_auth.rs::a_taken_port_exits_with_port_in_use`, `two_workspaces_never_record_one_port`, `serve_port_moves_only_when_asked`; `store/tests/migrations.rs::an_older_watch_exits_after_a_migration`; `store/tests/ledger.rs::an_older_engine_reads_a_newer_ledger`, `a_ledger_migration_keeps_a_copy`; `git_hardening.rs::a_repo_another_user_owns_is_refused`, `an_old_git_gets_git_too_old`; `engine.test.ts::the_fallback_forwards_both_cases_of_proxy_variables`; `docs.rs::remove_planroom_lists_every_location`, `readme_installs_the_release`.
  - Owner: the checklist fast-forwards `release` when A-20 picks it, and installs from the README after each release.
  - Walking skeleton: WS5 checks the detach on both systems; WS8 checks A-20, A-21 and A-23 and grows 1 h.
  - Milestones: M1's canvas, extractors and backup slices grow 1 h each; M5's release slice grows 2 h.
  - It amends DR-1, DR-16, DR-29, DR-51, DR-53, DR-58, DR-64 and DR-69.
- **Sources (checked 2026-10-26):** [plugin-packaging §5] (real git and GitHub installs unverified; version resolution); [mod-api §7] (process groups on reload unverified); `/bin/sh -> dash` in this container, and the round-3 verifier's `set -m` check; KICKOFF's packaging constraint and S14; review round 3 operability-1 to operability-8, unasked-7, unasked-8 and unasked-10, and owner decisions F8, F16 and F17.

## DR-79: Gates that fire only where they mean something

- **Status:** accepted (decided in review round 3, F18).
- **Starting point:** extended DR-59's and DR-70's repo gates, and "Repo layout: a crate-graph test or `cargo-deny` bans in Rust, and `dependency-cruiser` in TypeScript".
- **Origin:** review round 3: repo-1, repo-2 and operability-4; the minor findings repo-3 to repo-9.
- **Context:**
  - Only the kernel had a dependency allow-list. `store` and `extract` forbade three named crates, `contract` and `planroom` had no rule, and nothing linted `std::net` outside the kernel. So "only `models` may use a network client", and the kernel's no-I/O rule, reachable through a `contract` helper, were conventions. The `planroom` row had drifted from DR-72.
  - Every check could print `skipped`, and only `check audit` said a skip fails in CI, so a self-test, a plant or the installer suite could go unrun on a green CI.
  - The layout walk defined ranges only for pushes and pull requests, and a range it can't resolve fails. So the release dry run on `workflow_dispatch` came out red.
  - Plain `gen` didn't write golden files, eval results or `layout.lock`, though the conflict rule said it did, and nothing compared golden values.
  - A branch couldn't make `check` pass after a gated change, and record numbers assigned ahead broke when branches merged out of order.
  - Generated files sat beside hand-written ones, with mixed files unmarked. The fixture's skill had no tool guard. Amended records kept superseded bullets under a plain "accepted". Two Read first names matched no heading, and only top-level directories needed a stated purpose.
- **Decision:**
  - **Allow-lists and sockets.** Every shipped crate has an `external_allow` in `boundaries.toml`, over normal and build dependencies. Of the shipped crates, only `models` and `planroom` may open sockets: the others' generated `clippy.toml` disallows `std::net`. `contract` gets its own allow-list and the kernel's no-I/O lint. The area table's "May depend on" cells must equal the allow-lists. The self-test plants `ureq` in `extract`, `extract -> models`, `testkit` as a normal dependency, a `std::net` call in `store` and a `std::fs` wrapper in `contract`.
  - **A skip in CI is a failure**, unless `crates/xtask/ci_skips.toml` lists the check with a reason and a record. CI installs nextest, so the fallback can't run there. BusyBox runs on Linux only, and macOS runs the installer suite under `/bin/sh` and dash. `feature_list_is_complete` refuses `passing` for an entry with a skipped test or step.
  - **The walk per event.** Branch pushes and pull requests walk their commits; `workflow_dispatch`, `schedule` and tag pushes run the state check only.
  - **Smaller fixes:**
    - Tests that own golden files compare by default and write only under `gen --golden`, and the Generated directories table names each output's conflict command.
    - In a linked worktree, a changed gated surface is `pending ratification`, not a failure, and the merge commit carries the record and the lock.
    - `plugin/hooks/` becomes wholly generated, mixed files carry BEGIN and END GENERATED markers, and the read-only list and `.gitattributes` are generated from the table.
    - `.claude/settings.json` denies `Skill(run-worker-tests)`.
    - An amended record's Status says so, and an amendment overrides the bullets above it.
    - Read first names are links that `gen` resolves. Each area's `AGENTS.md` lists its subdirectories, and `every_directory_has_a_purpose` checks tracked directories against the map and those lists.
- **Alternatives rejected:**
  - Longer forbid-lists of network crates: a new client crate passes until someone names it.
  - A failure for every `skipped`, everywhere: an offline local run needs `check audit` to skip.
  - A range from the last tag for a dispatch run: its commits were already walked, and no tag exists before v0.1.0.
  - Moving every generated file into a `generated/` directory: `clippy.toml` and `.dependency-cruiser.cjs` must sit where their tools read them, so they keep header lines.
  - Rewriting every amended record's bullets: each amendment names its round and finding, which a rewrite would lose, while the status and the reading rule point the reader to it.
- **Consequences:**
  - Contracts: drift tests `golden_outputs_equal_engine_output` and `every_directory_has_a_purpose`, and the wider `generated_headers` and `area_contracts_match_repo_layout`.
  - Layout: `boundaries.toml` gains allow-lists, `net` and `no_io_crates`; `crates/{contract,store,extract}/clippy.toml`; `crates/xtask/ci_skips.toml`; a generated `plugin/hooks/hooks.json`; the deny entry in `.claude/settings.json`; each area's Layout list.
  - Tests: the self-test's five new plants; `gates.rs::a_dispatch_run_checks_state_only`, `a_tag_push_checks_state_only`, `a_worktree_change_is_pending_not_failed`, `gen_layout_lock_refuses_in_a_worktree`; `crates/xtask/tests/skips.rs::a_skip_fails_under_ci`, `a_declared_skip_passes`; `docs.rs::an_amended_record_says_so_in_its_status`, `read_first_names_resolve`.
  - Agents: an agent on a branch reports `pending ratification` instead of stalling, and one table says how to resolve any generated conflict.
  - It changes gated surfaces: `boundaries.toml`, the area contracts table, and the Boundaries and Conventions sections of `AGENTS.md`. The lock that records them names this record.
  - Milestones: M1's repo-gates slice grows 3 h.
  - It amends DR-59 and DR-70.
- **Sources (checked 2026-10-26):** KICKOFF R16, S14, S15 and the repo-tooling constraint; `crates/*/Cargo.toml` for the allow-lists' current entries; `crates/xtask/src/boundaries.rs` and `main.rs` (skips that return `Ok`); review round 3 repo-1 to repo-9 and operability-4, and owner decision F18.

## DR-80: Hand-off results land however the change reaches the tracked branch

- **Status:** accepted (decided in review round 3, F20).
- **Starting point:** none; serves R7 and R8 ("reconciliation closes the loop after merge").
- **Origin:** review round 3: unasked-6.
- **Context:** the only route to `merged` was the person-only `handoff merge`, a local `git merge --no-ff`. A repo with a protected `main`, or a team used to pull requests, lands the branch by a pull request, often squashed. Then `planroom/KEY` was never an ancestor, the item stayed in progress, and the squash commit, which touches the item's targets, fell through to the residual card.
- **Decision:**
  - An item is done when its rules hold on the tracked branch and its hand-off reached it, however the change got there.
  - At `collect`, the hand-off records each repo's patch identity: `git patch-id --stable` over the branch's diff from its base commit.
  - A scan moves a `reviewing` hand-off to `merged` when the tracked branch holds the branch head, gains a commit or a run of commits with the same patch identity, or gains commits on its touch points after which every rule holds. Those commits are its change units and its `landed` list, never residuals, and `handoff revert` reverts them.
  - `handoff merge` stays the local path. The review card names the branch, so a person can push it for a pull request; Planroom never pushes it.
- **Alternatives rejected:**
  - A command that pushes the branch for a pull request: pushing is the person's own git action, and a Planroom push needs credentials and a remote policy no scenario needs.
  - Matching a commit message that names `KEY`: messages get edited, and patch identity with the rules decides without them.
- **Consequences:**
  - Contracts: `HandoffStep.patch_ids` and `landed_by`; `RepoStep.landed`; the loop gains `reviewing -> merged`.
  - Tests: `reconcile.rs::a_squash_merged_handoff_is_done`, `a_rebase_merged_handoff_is_done`, `an_edited_pull_request_whose_rules_hold_is_done`, `a_landed_change_is_no_residual`; `handoff_preflight.rs::a_revert_reverts_a_squash`; S6's own-loop check covers the new transition.
  - Users: a team with protected branches lands hand-offs the way it lands everything else.
  - Milestones: M2's hand-off slice grows 2 h and its reconciler slice 1 h.
  - It amends DR-25, DR-44 and DR-55.
- **Sources (checked 2026-10-26):** KICKOFF R7, R8 and principle 1; `git patch-id --stable` (git documentation); review round 3 unasked-6 and owner decision F20.

## DR-81: What the walking skeleton's engine and plugin work settled

- **Status:** accepted (decided at the merges of the engine and plugin work, 2026-10-26). Amended by DR-85 and DR-86.
- **Starting point:** extended "Store: events carry ... an idempotency key", "Repo layout: a crate-graph test or `cargo-deny` bans in Rust, and `dependency-cruiser` in TypeScript", and Remote mode.
- **Origin:** the walking skeleton's engine work (WS3 and WS4) and plugin work, from their merge notes in `progress.md`, plus the decisions taken at the merges.
- **Context:**
  - The engine work built the store, the scan path and the `files` and `compose` built-ins. Its notes found gaps: the stored reply DR-8 replays had no column, and a repo moving A, B, A, B met its own scan key.
  - The walk read the working tree while evidence named `HEAD`. A directory, a repo and an empty file had no line to cite. Compose `depends_on` and the repo node's ID had no stated form.
  - The plugin work found host rules the design didn't know. The host loads a hooks module only when `register` is exported on its own declaration, and `claude plugin validate --strict` warns on a `CLAUDE.md` in a plugin.
  - To pass its checks, the plugin work generated depcruise rules that let only the mod import `claude-code` and only the mod's tests read golden JSON. That is a boundary change, which needs a record.
  - Its code also differed from the design in places: `scan` marked `NotUndoable`, `CommandSpec` without `spends`, the `engine` atom type held in xtask, and hand-written fallback strings.
  - With mods off, `open` had no path at all, since DR-75 made it press-only. The cloud session's turn-start sync didn't say what its `ArtifactData` call asks of the person.
- **Decision:**
  - **Store and scan.**
    - `events` gains `reply`: the writing command's whole reply, on its first event only, which a replay returns.
    - A scan key whose event exists gets `#after:SEQ`, the `seq` of the scan event the repo moves away from. So every move records its own event.
    - The walk reads the working tree, and verification checks every evidence item against the snapshot's commit. A line changed since then drops, with its fact when no evidence is left.
    - A directory, the repo, and an empty or non-UTF-8 file carry `Evidence::Ref` with the commit and branch read.
    - Compose `depends_on` becomes a `code.depends_on` hint, keyed by the service depended on, with the dependent service in `service`.
    - A repo's own node is `<ns>:repo:<ns>`, such as `orders:repo:orders`.
    - The pins are `rusqlite =0.40.2`, `sha2 =0.11.0`, `uuid =1.28.0` and `saphyr-parser =0.1.0`, each at least 14 days old, recorded in DR-39.
    - `serde_json` in `store`, and `uuid` and `toml` in `planroom`, are already on the allow-lists DR-79 set, so they need no added rule.
  - **Boundaries, ratified.** Only `packages/mod` imports `claude-code` and `claude-code/testing`, the mod API types. Only the mod's tests read `tests/golden/*.json`. `boundaries.toml` states both, as `host_types` and `golden_readers`, and `cargo xtask gen` writes them as depcruise rules.
  - **The mod and its build.**
    - `gen` rewrites esbuild's export list to `export function register`, so the host loads the bundle.
    - Plain `gen` writes the bundle with every other output, and `gen --mod` writes the bundle alone.
    - The fallback strings move from `packages/mod/src/fallback.ts` into `en.toml` with the string catalog.
    - The atom types, `EngineState` and `EngineStatus` among them, move from a string in `crates/xtask/src/modgen.rs` to `crates/contract/src/view.rs`.
    - Claude Code 2.1.310's `claude-code.d.ts` is vendored in `packages/mod/vendor/`, matching the Claude Code pin.
    - eslint's `no-misused-promises` is off for `packages/mod/src/register.tsx` and `packages/mod/plugin-tests/` only. It took 11 minutes over the mod API types, and `tsc -b` still checks those files.
    - `plugin/` carries `AGENTS.md` and no `CLAUDE.md`.
  - **Catalogue and resolver.** The contract crate follows the design: `scan` is `Query`, so `share` stays the only leaf on DR-55's `NotUndoable` list, and `CommandSpec` has `spends`. `engine_unavailable` gains two reasons:
    - `override_missing`: `PLANROOM_ENGINE` names no executable file; the fix is `unset PLANROOM_ENGINE`.
    - `no_build`: `PLANROOM_RESOLVE=checkout` finds no build in the checkout; the resolver offers the source build.
  - **`open` with mods off.** With mods off there is no mod to press. So in a workspace with no registered person key, `open` takes a `/dev/tty` confirmation that names the action. Once a mod registers its key, `open` is press-only again, as DR-75 set.
  - **The turn-start sync's consent.** A cloud session's turn-start sync makes one `ArtifactData` call per turn. The first call's permission prompt is the person's chance to allow it for the session. Without that allow, the sync runs only on `/room sync`.
  - **Generated files and the first lock.** `plugin/hooks/hooks.json` is generated with the bundle, and each crate's `AGENTS.md` contract block comes from the area table under `area_contracts_match_repo_layout`, as DR-79 set. The first `layout.lock` names DR-79 for every surface and needs no record of its own; every later lock change adds one in the same commit.
- **Alternatives rejected:**
  - Rebuilding a replayed reply from the fold: later events change the state, so a retry would get a different reply than the first call.
  - Checking evidence against the working tree: a line nobody committed would reach snapshots and the Artifact, and differ on the next machine.
  - Letting any package import `claude-code`: the canvas and `ui` would compile against a host API they never run in.
  - Keeping `open` press-only with mods off: then a person with mods off can't open the canvas at all.
  - Auto-allowing the turn-start `ArtifactData` call: DR-31 rejected a mod auto-allow, and the person's own allow is the consent.
  - Exporting `register` by hand in the source: esbuild gathers every export into one list as it bundles, so only a rewrite after the build holds.
  - A record for the first lock: the surfaces it hashes were set by DR-79 and ratified here, so a third record would approve nothing.
- **Consequences:**
  - Contracts: `events.reply`; `Evidence::Ref` on directories, repos and empty or non-UTF-8 files; the `code.depends_on` hint; the repo node ID; `EngineState` and `EngineStatus` in `crates/contract/src/view.rs`; `engine_unavailable` reasons `override_missing` and `no_build`; `open`'s actor rule.
  - Layout: `host_types` and `golden_readers` in `boundaries.toml`; `packages/mod/vendor/claude-code.d.ts`; `plugin/` without `CLAUDE.md`; `gen --mod`; the eslint exception.
  - Tests: `crates/store/tests/commit.rs::moving_back_and_forth_records_every_move`, `a_retry_with_the_same_key_returns_the_stored_reply`; `crates/store/tests/idem.rs::a_retry_after_a_kill_after_commit_returns_the_stored_reply`; `crates/extract/tests/contract.rs::files_emit_the_repo_its_directories_and_its_files`, `evidence_that_differs_from_the_commit_is_dropped`, `compose_emits_containers_aliases_and_hints_on_their_lines`; planned `actor.rs::open_takes_the_terminal_path_only_with_no_mod_key`, `gates.rs::the_first_lock_names_dr_79`, and the self-test's `claude-code` and golden-JSON plants in `packages/canvas`.
  - Users: with mods off, a person at a terminal can open the canvas. In a cloud session, turn-start syncs run only after the person allows the first call.
  - Security: a pty that confirms `/dev/tty` can open the canvas in a workspace no mod has run in. That is R-8's guardrail, as for the other terminal-path commands.
  - It changes gated surfaces: `boundaries.toml`, the area contracts table (`packages/mod`, `tests/golden`) and the Boundaries section of `AGENTS.md`.
  - It amends DR-8, DR-12, DR-39, DR-42, DR-57, DR-69, DR-70, DR-75, DR-77 and DR-79.
- **Amended by DR-85:** the walk no longer reads the working tree. It lists `HEAD`'s tree and reads the files from the commit, so a snapshot's facts are a function of its key, and a directory or file node exists only for a path in the commit. Verification still checks every evidence item against the snapshot's commit, and `Evidence::Ref` still names no path.
- **Amended by DR-86:** `Evidence::Ref` and `Line` evidence from a head scan name the commit and no branch. The branch is on the scan event, and a head snapshot records none, since a branch can move or be deleted while the commit stays.
- **Sources (checked 2026-10-26):** `progress.md`, the 2026-10-26 entries for the engine and plugin merges; `docs/contributing/research/ws4-concurrency.md`; `crates/store/migrations/0001_init.sql`; `crates/store/src/commit.rs`; `crates/extract/src/{files,compose,verify}.rs`; `.dependency-cruiser.cjs`; `crates/xtask/src/{modgen,generate}.rs`; `plugin/scripts/planroom`; `eslint.config.js`; the decisions taken at the merges.

## DR-82: What the walking skeleton's serve, canvas and mod work settled

- **Status:** accepted (decided at the merges of the serve, canvas and mod work and the integration step after them, 2026-10-26).
- **Starting point:** extended "Actors: person actions are signed", "Surfaces: the canvas and the mod read one contract", and Repo layout's gated package dependencies.
- **Origin:** the walking skeleton's serve, canvas and mod work, from their merge notes in `progress.md`, plus what the integration found when the three met.
- **Context:**
  - The serve work and the mod work each had to choose the person token's wire format, which the docs left open, and chose differently: the mod signed `at` in milliseconds and left `--idem` and `--base` out of `args`, while the engine read seconds and signed every word but `--json`. Each side's tests passed alone; together the engine refused every mod token as stale.
  - In `claude plugin test` on Claude Code 2.1.310, `crypto.subtle` has only `digest`, so the mod can't sign with Web Crypto [A-17 fails]. The mod's `third_party` list is empty.
  - The serve work reports the Rust target triple in `version --json`, which `VersionInfo` didn't list. The mod work keeps `lastSeq` as `{ seq }`, not the bare `Seq` the docs gave, and widened the A-15 guard until WS8 found the `$.store` file, which turned out to sit inside the folder the widened rule exempted.
  - Every golden now comes from a real engine run, but the engine can't yet produce a decision card, diffs, drift, tones or an agent's ghost edits, which the surfaces' tests draw.
  - The canvas found that `elk.bundled.js` fails inside a module worker, that its `Transport` needs a login and a layout save, and that the palette has no lens colors yet.
  - Some code trails the docs: `PaneModel.minimap` is required, `PresenceRow` has no `client_id`, `StreamFrame` has only `Event`, `Reset` and `Ping`, the resolver's cache path ignores the catalogue hash, and `serve` lacks `code`, `drain`, its version handshake, the request timeout and the client cap.
- **Decision:**
  - **The person key and token, on the wire.**
    - The key `person init` reads on stdin is one line: the base64url, without padding, of the 65-byte SEC1 uncompressed point. Its fingerprint is the first 16 hex characters of the sha256 of those bytes.
    - `PLANROOM_PERSON_TOKEN` is `<at>.<sig>`. `at` is Unix time in whole seconds. `sig` is the base64url of the 64-byte `r ‖ s` ECDSA P-256 signature over SHA-256 of the message.
    - The message is `command ‖ args ‖ idem ‖ base ‖ at`, each field followed by one NUL byte. `command` is the leaf's path joined with spaces (`person init`). `args` is the canonical JSON array of the argv words after the command's path, less `--json`; `--idem`, `--base` and their values stay in it as well as in their own fields. `idem` and `base` are those flags' values, or empty. `at` is its decimal text.
    - The engine accepts a token within 30 s of its own clock, either way, and spends its digest once. The mod signs low-S; the engine accepts either form.
    - With no key registered, `person init`'s token must verify under the key on its own stdin, which proves the caller holds that key. A `--rekey` verifies under the old key or the new one; the new one is the mismatch pane's press, the guardrail that accepted risk security-4 names.
    - `crates/kernel/src/keys.rs::a_token_message_ends_each_field_with_a_nul` and `packages/mod/src/actor.test.ts`'s "the signed bytes match the engine's own example" pin the same bytes on both sides.
  - **The mod signs with `@noble/curves`** 2.4.0 (MIT, audited), bundled into `plugin/hooks/planroom-mod.js`; it is the mod's one `third_party` entry. A-17 is settled as failed.
  - **`VersionInfo`** is `{ version, git_commit, target, schema_version, catalogue_hash }`, `target` being the Rust target triple the binary was built for.
  - **The mod's `lastSeq` atom** is `{ seq: Seq | null }`, null before the first frame.
  - **The A-15 guard.** WS8 found the mod's `$.store` file: the host keeps every plugin's store at `<config>/plugins/store/<plugin>_<source>-<hash>.json`, mode 0600 (`planroom-spike_inline-d70c18432e39.json` for the spike mod). The mod work's interim rule exempted everything under `plugins/`, so it missed exactly that file. The guard now denies agent reads of `<config>/plugins/store/` itself and of every `planroom_*` file in it, and a Grep or Glob rooted at a folder that holds it; it no longer matches other paths that merely name `planroom`.
  - **Goldens from real runs only.** Every file under `tests/golden/cli/` is written by the engine's own tests on the built fixture, `cargo xtask gen --golden` rewrites them, and no generator builds stand-ins. `map-empty.json` is `map --detail graph` before the first scan, with the temporary root written as `/home/you/shop`. A surface test that needs a state the engine can't produce yet lays a test-only overlay over a real golden, kept beside the test and named for the milestone that makes the state real; nothing hand-edits `tests/golden/`. The golden test learns it should write from `PLANROOM_GOLDEN=write`, which only `gen --golden` sets and the engine never reads; DR-70's retired variables stay retired.
  - **The canvas.** ELK runs from ELK's own `elk-worker.min.js`, loaded by URL from `layout/elk.ts`, since `elk.bundled.js` fails inside a module worker [ts-stack §8]. Until packs bring lenses in M1, every edge draws in `--muted`; M1 adds one yarn token per lens to the palette, held to the 3:1 mark bar. The needs-login text lives in `packages/canvas/src/strings.ts` until M1 generates the canvas's strings from `en.toml`, as `FALLBACK_PANES` already is. Vite runs with `--configLoader runner`, so its config can import `@planroom/ui`'s TypeScript.
  - **Serve and the live stream, from the integration step.**
    - A browser sends no `Origin` on a same-origin `GET`, so a `GET` may prove its origin with `Sec-Fetch-Site: same-origin`; a foreign `Origin`, a `GET` with neither, and a `cross-site`, `same-site` or `none` fetch are refused. `POST /api/v1/code` and `drain`, called by local processes with the control token, may carry no `Origin`, never a foreign one.
    - The CSP is exactly what the canvas needs: `style-src` allows `'unsafe-inline'`, since React Flow places blocks with inline styles, and nothing else loosens.
    - `POST /api/v1/session` answers with the envelope, `result: {token}`. `POST /api/v1/code` takes the contract type `CodeReq`. The canvas sends a handle as its command's request JSON to `POST /api/v1/{command path}`, and puts layouts under the map's own `layout` key with the contract's `LayoutPut`.
    - HTTP `scan` takes a `dir` only when it names the served root.
    - `watch` replays from `--since` right after its first ping, sends `reset` to a cursor more than 5,000 events behind or past the store's last `seq`, and exits with `schema_mismatch` within one 200 ms poll plus its exit. `WatchReq` gains the optional `session` the mod passes. Watch and serve build frames with the same code.
    - `tests/e2e` also holds the vitest global setup that runs the built engine's `watch --json` for the mod's integration test, since the mod has no Node APIs, and the test-only overlays.
  - **Code that trails the docs keeps to the docs.** Each item below is built in the milestone named, and the docs stand as written:
    - `PaneModel.minimap` becomes optional with `focus --detail summary` (M1, mod slice).
    - `PresenceRow.client_id` and `StreamFrame::Presence` arrive with presence through `watch` (M1); `Duty` arrives with detached duties (M3) and `ModelRequest` with the session backend (M2).
    - The resolver's cache key takes the catalogue hash, and `serve` gains `drain`, the version handshake, reuse of a running server, the 10 s request timeout, the 64-client cap, a `reset` for an SSE client its broadcast drops, removing `serve.json` on a `schema_mismatch` exit, and a start in a workspace with no store yet, so the canvas's empty map comes from the real server (M1, serve and resolver slices).
    - A scan frame's `target` is the repo's node ID (`worker:repo:worker`), not its name (M1).
- **Alternatives rejected:**
  - Milliseconds for `at`: a token's window is 30 s, and seconds are what the engine's clock and its spent-token table already keep.
  - Leaving `--idem` and `--base` out of `args`: the engine would have to strip them the mod's way, two rules to keep in step where one says "every word but `--json`".
  - Web Crypto in the mod: the host doesn't offer `sign`, and a native helper would add a binary to the plugin.
  - Generated stand-in goldens: they drifted from the engine (repo IDs, `tone`, positions) and hid the token mismatch's kind of gap, which only a real run shows.
  - `elk.bundled.js` in a module worker: it fails at load.
- **Consequences:**
  - Contracts: the token and key formats; `VersionInfo.target`; `lastSeq`'s shape.
  - Layout: `@noble/curves` in the mod's `third_party` list in `boundaries.toml` and the area table; `layout/elk.ts` in place of `layout/elk.worker.ts`; `tests/golden/cli/map-empty.json` written by `crates/planroom/tests/golden.rs`; `crates/xtask/src/golden.rs` removed.
  - Tests: the shared token example; `golden.rs::golden_outputs_equal_engine_output` over every golden; the surfaces' overlays.
  - Security: unchanged; the rekey's press stays the guardrail security-4 accepted.
  - It changes a gated surface: the mod's dependency names.
  - It amends DR-58, DR-64, DR-70, DR-72 and DR-75.
- **Sources (checked 2026-10-26):** `progress.md`, the 2026-10-26 entries for the serve, canvas and mod merges; `crates/kernel/src/keys.rs`; `crates/planroom/src/app/{actor,person,version}.rs`; `packages/mod/src/{actor,person,atoms}.ts`; `packages/canvas/src/layout/elk.ts`; `crates/planroom/tests/golden.rs`; `docs/contributing/research/ts-stack.md` §8.

## DR-83: What the walking skeleton's gates, remote slice and platform checks settled

- **Status:** accepted (decided at the merges of the gates and remote-slice work, the CI repairs and the WS7 and WS8 checks, 2026-10-26). Amended by DR-87, DR-89 and DR-91.
- **Starting point:** extended "Repo layout: a crate-graph test or `cargo-deny` bans in Rust, and `dependency-cruiser` in TypeScript", Remote mode, and the release model.
- **Origin:** the walking skeleton's gates and remote-slice work, CI's first runs on the merged tree, and the platform checks in `research/ws7-remote.md` and `research/ws8-install.md`.
- **Context:**
  - **A-13 fails.** As the owner, the session wrote under a made-up viewer's `inbox/{self}`. The same write at `interact` level was refused. So only the owner, and the owner's model session with it, can write any viewer's inbox.
  - **Rows arrive in the text block.** `ArtifactData list` returns each document's data in its text block. The structured result the mod sees through `$.tool.call` carried `data: {}`.
  - **CI caught what local runs couldn't:**
    - cargo-deny failed on tokio 1.53.2's RUSTSEC-2026-0386, fixed in 1.53.3, which was 13 days old.
    - `subtle` and `matchit` carry BSD-3-Clause.
    - The workspace path dependencies had no `version`, which cargo-deny counts as wildcards in publishable crates.
    - A BusyBox built to prefer its applets broke the installer harness.
    - macOS reports `/private/var` for `/var` temp paths, and puts a trailing slash on `$TMPDIR`.
  - **`/plugin update` keys on the version string** (A-23 fails). `claude plugin validate` accepts any version string at all.
  - **The gates work** built the documented gates and reported where the documents left a choice open.
  - **The remote-slice work** built the thinnest remote slice and named what each part defers.
  - DR-81 settled `scan` as a `Query`, but the code still said `NotUndoable` until this merge.
- **Decision:**
  - **Unsigned inbox rows wait for the desk, whoever's inbox holds them** [A-13 fallback]. An inbox path no longer proves its author against the owner's own session. Every imported row that no paired device signed is held exactly as the owner's unsigned rows are:
    - an edit becomes a ghost on the one Apply card;
    - a pick waits pre-selected on its card.
    The actor is still `person:artifact:<uid>` from the path, named for display, never as authority. Read requests need no person and are answered either way. A paired device's signed reversible edits still apply directly.
  - **`/room sync` reads the rows from the text block.** The mod takes each JSON line between `BEGIN ARTIFACT DB` and `END ARTIFACT DB` as data, checks it against `InboxRow`, and passes `{path, version, row}` to `import -`.
  - **`import -` and its pins.**
    - Each stdin line may carry the document's `version`, and lines with no `row` give the versions of documents the session wrote earlier.
    - Import returns batches of at most 50 writes and 1 MiB, with `live/head` last. Each batch names its files under `.planroom/share/sync/<seq>-<µs>/` and pins every write to an existing document with `if_version`.
    - `Answer` carries `format` and an optional `error`. `acks/<uid>` is `planroom.ack/v1`, with `imported_through`, and an existing ack is written as an `update` so its rows merge.
    - Unknown fields in an inbox row are ignored, so a forged `actor` is ignored.
  - **`share`** writes the canvas built for the Artifact, with relative paths, and marks its root `data-transport="artifact"`, which `main.tsx` reads to pick the transport.
    - `Snapshot.capabilities` names view keys (`map:system:all`, `pane`).
    - Positions come from the layout cache when one exists; otherwise the page lays out with ELK.
    - `meta.share_snapshot_seq` records what the snapshot holds, for `live/head.snapshot_seq`.
    - The publish declares `db` and `user` until Ask Claude and Send to Claude land, which add `sample` and `comments` (M4).
  - **A release that fixes a RustSec advisory in the tree may be pinned before it is 14 days old.** The commit and the record name the advisory. tokio 1.53.3 for RUSTSEC-2026-0386 is the first, at 13 days. Then rustls 0.23.46 for RUSTSEC-2026-0349, at 13 days, and the transitive webpki-root-certs 1.0.10 for RUSTSEC-2026-0383 (2026-10-27). `cargo xtask check audit`'s age check reads the exceptions from this record's list. `deny.toml` allows BSD-3-Clause, which DR-39 already named.
  - **Path dependencies carry their version.** Each workspace path dependency names `version = "=X.Y.Z[-dev]"`, so the crates can be published and cargo-deny keeps denying wildcards. The release step (`gen --version V`, M5) rewrites these pins with the workspace version.
  - **The plugin's version is stamped from its own files** [A-23 fallback, refined]. Plain `gen` writes `plugin.json`'s version as `X.Y.Z-dev.g<first 8 hex of a sha256 over every file under plugin/ but plugin.json, in sorted path order>`. A release version is written as is. The `g` keeps the identifier alphanumeric, as semver requires. The resolver and `engine.env` compare the engine against `X.Y.Z-dev`, without the stamp. The plugin layout check holds the version to semver, since `claude plugin validate` doesn't. This replaces the catalogue-hash stamp the A-23 row named, which missed a change to a skill, an agent, the resolver or the mod alone.
  - **The gates, as built:**
    - `layout.lock` is TOML, one table per surface with `sha256` and `record`. The first lock names DR-79.
    - `[ts.rules] test_tooling` lets tests and configs import vitest, vite and `@vitejs/plugin-react` under the default-deny allow-lists.
    - `libc` is exempt from the socket scan, which finds callers through `libc::socket` instead.
    - CI installs its tools in xtask under `CI=true`:
      - cargo-deny from its sha256-pinned release archive;
      - nextest with `cargo install --locked` at an exact version;
      - ShellCheck, BusyBox and dash from the runner's package manager.
      A tool fetched as a release binary is sha256-pinned; one from a package manager or `cargo install --locked` is pinned by version.
    - The bare-name scan and the pack `requires` plants print as "not built", not as skips, until packs land in M1, which builds them.
  - **The test harnesses take paths as the engine reports them:** physical (`pwd -P`, `canonicalize`), and found by walking `PATH` rather than `command -v`. `cargo test` runs every binary (`--no-fail-fast`), so one failure doesn't hide the next.
  - **Code that trails the docs keeps to the docs,** by milestone:
    - `Snapshot` gains layouts, diffs, plans, machines, every level and lens, cards and the precomputed answers (M4).
    - `LiveHead` gains its `delta` with `Diff` (M2).
    - Read requests beyond `map` (M4); edits (M1) and decisions (M2) from inbox rows.
    - A republish's `if_version` and clearing `answers/` (M4).
    - `#[redact]` marks replace `kernel::redact`'s hand-written field list (M1).
    - `every_directory_has_a_purpose` and the pin-age audit (M1).
    - The testkit row's allow-list (M1).
- **Alternatives rejected:**
  - **Trusting another viewer's unsigned row:** the owner's session can write any inbox with no prompt, so the path proves nothing against a model.
  - **Ignoring the advisory until 1.53.3 turned 14 days old:** the age rule guards against new, untested releases, not against a fix to a known flaw. Holding a known unsound version for a day buys nothing.
  - **`publish = false` on the library crates:** `planroom` couldn't then be published to crates.io, which the release keeps open.
  - **Stamping the catalogue hash:** it misses plugin-only changes.
- **Consequences:**
  - Contracts: import's stdin and result shapes; `Answer` and the ack format; the share page's transport marker; the plugin version stamp.
  - Layout: `layout.lock`; `[ts.rules] test_tooling`; `crates/xtask/ci_skips.toml`; the generated `clippy.toml` per crate.
  - Tests: `crates/xtask/tests/{gates,skips,docs}.rs`; `crates/planroom/tests/s11_remote.rs`; `tests/e2e/src/artifact.spec.ts`; the installer suite under sh, dash and BusyBox.
  - Users: on a shared Artifact, a collaborator's edit waits for the owner's one Apply card unless the owner paired that device. Read requests from anyone are answered on the next sync.
  - Security: the A-13 fallback closes the path a model-written row would otherwise have to a person's authority.
  - Release: `gen --version V` rewrites the path-dependency pins; the published marketplace entry names the tag through `git-subdir` [A-20].
  - It changes no gated surface beyond what the first lock records.
  - It amends DR-39, DR-52, DR-55, DR-56, DR-67, DR-70, DR-75, DR-78, DR-79 and DR-81.
- **Amended by DR-87:** import's result carries `counts`, the first 5 rows' outcomes, `root` and `batch_files`, one file per `ArtifactData batch` call, in place of the batches inline; each batch keeps at most 50 writes and 1 MiB, with `live/head` last.
- **Amended by DR-89:** a `#[redact]` mark is `#[schemars(extend("x-redact" = true))]` on the field. `cargo xtask gen` writes the marked field paths into `crates/kernel/src/redact_fields.rs`, and `kernel::redact` masks exactly those paths.
- **Amended by DR-91:** the mark also sits on node and block names, diff hunks, presence `doing`, a hand-off step's `note` and a card's `vars`; join keys (`Hint.key`, `Edge.via`, `Node.aliases`) stay unmarked.
- **Sources (checked 2026-10-26):** `research/ws7-remote.md`, `research/ws8-install.md`; the gates and remote-slice merge notes in `progress.md`; CI runs 36 to 46 on the working branch; RUSTSEC-2026-0386; `crates/planroom/src/app/{share,import}.rs`; `crates/xtask/src/{layout,tools,plugin}.rs`.

## DR-84: The umbrella skill is `planroom`, so the mod can own `/room`

- **Status:** accepted (decided when the mod first ran live, 2026-10-26).
- **Starting point:** extended "Plugin: skills, agents and the mod ship as one plugin".
- **Origin:** the walking skeleton's live run of the mod.
- **Context:** Loaded live, the mod's `session.start` threw at `$.command.register({ name: "room" })`. The host refused the command because the plugin's own `room` skill already claims `/planroom:room`, and the refusal skipped the whole start: no engine, no pane, no tools that work. `claude plugin test` mocks `command.register`, so the harness never showed it.
- **Decision:**
  - The umbrella skill is named `planroom` (`plugin/skills/planroom/SKILL.md`). `/room` stays the mod's one command family [DR-77]. `room-plan` and `room-handoff` keep their names, since no mod command has them.
  - The plugin layout check refuses a skill whose name equals a command the mod registers. Its list of the mod's commands sits beside the check, with a pointer to `register.tsx`.
- **Alternatives rejected:**
  - Renaming the mod's command: `/room` is the design's one command family, in every key count and guide.
  - Registering the command under another name only where the skill exists: two names for one thing.
- **Consequences:**
  - Layout: `plugin/skills/planroom/`.
  - Tests: `cargo xtask check`'s plugin layout step.
  - Users: none see the skill, which is `user-invocable: false`.
  - It amends DR-30 and DR-57, where they name the `room` skill.
- **Sources (checked 2026-10-26):** the host's refusal during that live run, "`$.command.register`: "/room" refused: it is the plugin's /planroom:room"; `packages/mod/src/register.tsx`; `crates/xtask/src/plugin.rs`.

## DR-85: What the walking skeleton's review settled

- **Status:** accepted (decided at the merge of the review's fixes, 2026-10-26). Amended by DR-86 and DR-87.
- **Starting point:** extended "Extractors read the repo", "One contract source", "Surfaces read one contract" and the release model.
- **Origin:** the walking skeleton's review, round 1. Eight reviewers, one per focus and each checked by a separate verifier, found 18 material findings ([the review's index](research/skeleton-review/README.md)). Four fix branches fixed them (the scan, contract, engine and surfaces fixes), and the resolver and `check` items were fixed directly. This record settles each doc conflict the fixes raised.
- **Context:** The skeleton's slice worked end to end, live. Its review still found that a dirty first scan froze a wrong map, persisted documents dropped unknown fields, surfaces offered buttons that failed, the guard missed everyday shell forms, and some documented gates and checks had no milestone. Each fix met a place where the docs were silent or said something else.
- **Decision:**
  - **Scan reads the commit.** `scan` lists HEAD's tree with one `git ls-tree -r -z -l` per repo and reads the files with one `git cat-file --batch`, within the walk's old bounds (1 MiB a file, the binary check, 50,000 files and 256 MiB a repo, cut in sorted path order). The working tree never reaches the facts, so a snapshot's facts are a function of its key. "Ignored paths" means the committed `.gitignore` files only; `.git/info/exclude` isn't part of the commit, so it no longer applies. A Git LFS file maps by its path, and its content is the pointer text. A directory or file node exists only for a path in the commit; `Evidence::Ref` still names no path.
  - **The dirty signal counts untracked files.** Since uncommitted work never reaches the map, `meta.dirty_repos` (M1) is how a person learns of it, and an untracked file is the commonest uncommitted work. It runs `git status --porcelain=v1 -z --untracked-files=normal`, which honours `.gitignore`, in place of `--untracked-files=no`.
  - **Persisted documents follow the versioning rules.** `EventKind` is published as `anyOf [{enum}, {pattern}]`, and `EventBody::Unknown` keeps an unknown kind's raw body and writes it back unchanged. The remote documents (`Snapshot`, `LiveHead`, `InboxRow`, `Answer`, `Ack`) keep unknown fields in a flattened `extra`, as do `workspaces.json` and `serve.json`. One check, `contract::format`, loads older formats and refuses a newer one; `cargo xtask gen` writes its TypeScript port (`checkFormat`), which the Artifact transport uses. These changes only loosen what a reader accepts, before any release, so `schema_version` stays 1.
  - **`workspaces.json`** is read, changed and renamed under an exclusive lock on `workspaces.json.lock` (the standard library's `File::lock`). A file whose `format` is newer, or whose `min_reader_version` is above the engine's version, is refused with `schema_mismatch`, whose details gain `file`, and is never rewritten. The `workspaces.json` and `serve.json` types live in `crates/planroom`, since no surface reads them.
  - **The contract owns what crosses the language boundary.** `EngineState`, `EngineStatus`, `PaneState`, `LastSeq` and `TurnSeq` move to `crates/contract/src/view.rs`; `ToolResult`, `ToolError` and `ToolMeta` to `envelope.rs`; `ResolverStatus`, `ResolverStep` and `UnavailableReason` to a new `resolver.rs`. `engine_unavailable` (retry `never`) and `engine_fetching` (retry `later`) join `ErrorCode`. `ToolError` keeps `category`, since DR-66 sends the error whole. `cargo xtask gen` writes `plugin/scripts/contract.sh`: the schema version, the codes the resolver prints (`engine_unavailable`, `engine_fetching`, `invalid_argument`) with exit codes and retry classes, its reasons and steps, and envelope templates. The resolver sources it and spells no code or reason itself, which the installer suite checks. `boundaries.toml` lists `contract.sh` among the generated read-only files.
  - **The envelope carries a `summary`.** It's optional and additive: the engine-authored line text mode prints first, display-sanitized, present on success. `/room` shows it with the reply's handles, never JSON (DR-65).
  - **A usage error is one envelope.** A clap error under `--json` prints `invalid_argument` with `details.field`, `details.synopsis` (generated from clap) and `details.schema`, the `schema --command LEAF` handle, which is also the hint. An error that names no leaf uses `command: "planroom"` and the `schema --commands` handle. Every CLI `invalid_argument` carries them; errors over HTTP don't yet (M2's budgets slice).
  - **`map` keeps its budget.** Over its catalogue budget, `map` keeps what fits in the documented order and adds the `truncated` warning, whose details name what was kept and a drill-down that runs today: the next coarser level, or `focus` from the top. No `meta.next` cursor until M2's paging. `--detail graph` over serve's HTTP API and in `share`'s snapshot is the canvas's layout input, not agent-facing, so it stays whole until M2 pages it. Everything an agent can reach is capped: the CLI at any detail, and so `room_query`, the top-level map in `scan`'s reply (to `scan`'s own budget), a summary over serve, and import's read answers.
  - **No level is blank.** An empty level returns an `Empty`: `no_intent` (back to the top of the map), `empty_level`, or `nothing_mapped` (`scan ROOT`). Outside a workspace, `map` returns `focus`'s `no_repos` state, ok, not `workspace_not_found`, and that state's next action is `focus` ("check again"), never a `scan DIR` that fails.
  - **Display sanitizing is built.** `kernel::display::sanitize` marks control and bidi characters the way `@planroom/ui` does, on every text reply, every stderr line and every `en.toml` interpolation. The mod passes every string it draws through `@planroom/ui`'s `sanitize`.
  - **Surfaces offer only what works.** Until `map --focus` (M1), the pane's block rows are plain rows with no row key. A pressed next action shows its result with a way back; the pane keeps that in module state, so a reload forgets it. The canvas's breadcrumb follows the map's level; intent sits beside the system level, so its one ancestor is the top.
  - **The guard reads shell forms.** It denies a path word with glob characters whose prefix could expand into a guarded place, resolves relative paths against each `cd` on the line, compares variable names with quotes removed, denies a person-only command whose command word is an expansion, and denies `/proc/*/environ` and `/proc/*/mem`. It stays a guardrail: scripts run by path, variables set in an earlier call and `dotglob` aren't covered, and `SECURITY.md` (M5) says so.
  - **The source build builds the canvas.** With Node and pnpm present, the resolver's `build` runs `pnpm install --frozen-lockfile` and `cargo xtask gen --canvas` before `cargo build`, says so when it can't, and removes its clone once the cache holds the engine.
  - **`check` names what nobody built.** It prints `not built (M1)` for the breaking-change guard, `feature_list_is_complete`, the secret scan and `release_pins`, and `not built (M5)` for the docs tests and `scripts/check-tag-version.sh`. M1's repo-gates slice now builds the first four, and M1's backups slice the filesystem check at `Store::open` with `store.pointer`, `fs_check.rs` and `crash.rs`.
- **Alternatives rejected:**
  - Keeping the working-tree walk and keying snapshots by a hash of their facts: the map would still change with uncommitted edits, which architecture.md rules out.
  - Retrofitting `extra` and open enums after v0.1: that is itself a breaking change to the generated types.
  - Capping `--detail graph` for the canvas: it would draw a partial map with no way to page.
  - Removing the pane's block rows: they're the pane's main content; plain rows keep them readable.
- **Consequences:**
  - Layout: `crates/kernel/src/display.rs`, `crates/contract/src/format.rs` and `resolver.rs`, `plugin/scripts/contract.sh` (generated), `packages/mod/src/room.ts`, `packages/canvas/src/levels.ts` and `shell/next-action.ts`.
  - Contracts: `Envelope.summary`; `invalid_argument` details `field`, `synopsis` and `schema`; `schema_mismatch` details `file`; `engine_unavailable` details `reason`, `fix`, `est_minutes`, `canvas` and `missing`, with the download URLs still owed (DR-78, M5); the empty reasons `no_intent`, `empty_level` and `nothing_mapped`. Schemas and TypeScript regenerated; goldens regenerated, gaining `summary`.
  - Tests: `crates/planroom/tests/scan.rs` (three dirty-tree cases), `workspaces.rs`, `usage.rs`, `budgets.rs`, `empty_states.rs` and `display.rs`; `crates/kernel/tests/display.rs`; `crates/extract/tests/contract.rs::the_working_tree_never_reaches_the_facts`; the guard's 18 bypasses and 18 everyday commands; the canvas's way-back e2e specs; five new installer tests.
  - Users: a dirty or untracked file never changes the map; the install line works; every button does something.
  - It amends DR-81 where it says the walk reads the working tree, DR-47's bounds and the ignored paths they skip, DR-71's dirty signal (`--untracked-files=no`), and architecture.md's "The map holds only committed lines", which is now how it's built.
- **Amended by DR-86:**
  - `map` pages in M1, not M2: M1's S1 slice adds `--cursor` with `meta.next` at every detail, and the truth comparator follows it to the whole map. Until then `map` keeps the blocks that fit, as above.
  - The CLI checks every `--json` success reply against its leaf's budget, warnings included, not only `map`'s and `scan`'s. Replies over HTTP still aren't checked.
  - The guard also holds Bash to the Write tool's rule and refuses whole-tree reads of a secret's parents. Any word in the engine cache is denied, so listing it is no longer one of the everyday commands that pass. Its residual risks in `SECURITY.md` add programs that walk or write files themselves, a tree read rooted at or above the workspace root, a crafted tree copied above the data root, and quoted text that reads as a redirection.
  - `share`'s publish arguments name `root`, with `files` relative to it, and its result lists only the files it built from the workspace.
- **Amended by DR-87:** `UnavailableReason` is also a schema root, so the mod reads it from the generated types and its switch over reasons is checked at build time.
- **Sources (checked 2026-10-26):** `docs/contributing/research/skeleton-review/` (round 1's findings and verdicts); the four fix branches' reports; the merged tree's `cargo xtask check`, `test`, `test --installer` and `e2e`.

## DR-86: What the walking skeleton's review settled in round 2

- **Status:** accepted (decided at the merge of round 2's fixes, 2026-10-26). Amended by DR-87 and DR-100.
- **Starting point:** extended "Extractors read the repo", "Actors: agents never act as a person" and "Surfaces read one contract".
- **Origin:** round 2 of the walking skeleton's review. Its 16 material findings, 12 distinct defects, are in [the review's index](research/skeleton-review/README.md). Four fix branches fixed them (the scan, budget, guard and canvas fixes), and one was fixed in the plan.
- **Context:** Round 1's fixes held. Round 2 found what they left: inputs outside a snapshot's key, a late scan that could move a repo back, a partial clone that fetched, one unreadable repo failing a whole scan, warnings and two leaves over budget, a guard with no write rule for Bash and no rule for recursive reads, and two canvas controls that led nowhere.
- **Decision:**
  - **A snapshot's key holds every extraction input.** A head snapshot's key column, still named `packs_hash`, holds a hash of every input but the commit: the packs hash, each built-in's options (the sorted workspace repo names among them, since compose joins services to repos), and, for a partial clone, the files it lacks. `meta.packs_hash` is unchanged. Adding or removing a repo re-keys every repo's snapshot until M1's linkers move the cross-repo join into the link snapshot, as DR-12 intends; then a per-repo key holds per-repo inputs again.
  - **Evidence names the commit.** `Ref` and `Line` evidence from a head scan name the commit and no branch; the branch is on the scan event, and a head snapshot records no branch. The truth comparator (M1's S1 slice) matches TRUTH's evidence items that name a branch through the scan event that read them.
  - **A late scan never moves a repo back.** `commit_scan` re-reads a moving repo's `HEAD` inside its write transaction and leaves a repo whose scanned commit is no longer `HEAD` unchanged, with the warning `scan_superseded`. A repo's first scan records its commit even if `HEAD` moved meanwhile.
  - **A scan never fetches.** Every git call through the adapter runs with `-c protocol.allow=never` and `GIT_NO_LAZY_FETCH=1`, so a partial clone's missing blobs are never fetched and no transport it configures runs. M2's hand-off clones and fetches between local clones, which the `file` protocol carries, take an exception for exactly those calls. A missing blob is uncovered, reported by `partial_scan`.
  - **A scan's outcome is per repo.** A repo git refuses (another owner, a broken config) is skipped with `repo_skipped`, which names the repo, the reason and the fix, and the rest of the workspace maps. A repo with no commits yet maps nothing, since the map holds only committed content, and `repo_unborn` says it maps after its first commit. When git can read no repo and there's no map yet, `focus` and `map` return the empty reason `repos_unreadable`, whose next action is `focus`, never a scan that fails again; a workspace with a map keeps it, with `repo_skipped`. `git_failed` gets a hint.
  - **Every CLI reply keeps to its leaf's budget.** `partial_scan` and `partial_parse` carry a `count` and the first 10 paths in sorted order; `uncovered` and `paths` are now samples. The CLI's `finish` checks every `--json` success reply against its leaf's catalogue budget: over it, warnings are dropped in order behind one `truncated` warning with `{budget_tokens, left_out, codes}`, a leaf's own truncation is kept, and a result that alone passes the budget says so. Replies over HTTP aren't checked, so `--detail graph` stays whole for the canvas. `schema --command` drops descriptions to fit and keeps every shape whole. `share`'s publish arguments name `root`, with `files` relative to it, and its result lists only what it built from the workspace. These meaning changes come before any release, so `schema_version` stays 1.
  - **`map` pages in M1.** `--cursor` with `meta.next` at every detail moves from M2's budgets slice into M1's S1 slice, and the truth comparator follows it to the whole map. This amends DR-85's "no `meta.next` cursor until M2".
  - **The guard holds Bash to the write rule.** It splits a line into simple commands and their redirections, checks what each writes or deletes with the Write tool's rule, denies any word in the data root's engine cache (so listing it is no longer an everyday command), refuses deleting the current workspace's state directory, the data root, the mod's store, the user config or a folder above them, and refuses `git clean -d` with `-X`, and with `-x` unless `-e .planroom` keeps the state directory.
  - **The guard refuses whole-tree reads of a secret's parents.** `grep -r`, `find -exec`, `tar`, `zip -r`, `cp -r`, `rsync`, `ln -s` and the like may not name a folder above the mod's store, the user config or the data root's guarded files, nor a `.planroom` or `.git`. `rg`, `ag` and `fd` skip hidden names, so they're refused only with a flag that includes them or when no dot-folder sits in between; plain `rg foo` stays allowed. The Grep and Glob tools' containing-folder test covers the same places. It stays a guardrail: programs that walk or write files themselves, a tree read rooted at or above the workspace root, deleting another workspace's state directory, a crafted tree copied above the data root, and quoted text that reads as a redirection are named in `SECURITY.md` (M5).
  - **The resolver checks its cache.** It hands back a cached engine only when it's a regular file, not a link, whose sha256 matches `.ok`; anything else is no cache.
  - **The mod reads the contract's types.** `Envelope`, `ErrorObject`, `VersionInfo` and `ResolverStatus` replace the mod's hand-written reply shapes.
  - **A control with nothing to offer isn't a button.** A picker with nothing to pick, such as the project shelf before M1's `intent new` or a single lens, shows what it holds. Each ancestor lid is widened by the overlap the current lid paints over, so the phone's `⋯` lid keeps a 44 px target, which e2e measures with `elementFromPoint` at the top and one level down.
- **Alternatives rejected:**
  - Moving the cross-repo join into linkers now: that's M1's linker slice; keying by every input is correct today and simpler to undo then.
  - Mapping an unborn repo as a node with no evidence: it breaks "evidence or nothing".
  - Raising `share`'s budget: compact arguments fit within 800 tokens.
  - Denying every recursive read rooted at the workspace: it would refuse `grep -rn foo src/`-style everyday work, for a store an agent can already read through `planroom`.
- **Consequences:**
  - Contracts: warnings `repo_unborn`, `repo_skipped`, `scan_superseded`; the empty reason `repos_unreadable`; `partial_scan` and `partial_parse` details; the envelope-level `truncated` details; `PublishArgs.root`.
  - Tests: `crates/planroom/tests/scan.rs` (moving a repo out and back, a deleted branch, a late scan, unborn and unreadable repos), `tests/git_hardening.rs`, `budgets.rs` (every built leaf on the fixture, a scan past the file bound), the guard's write, recursive-read and Grep/Glob bypass lists, `a_swapped_cached_engine_is_refused`, and the canvas's 44 px and offers-only-what-works specs.
  - It amends DR-85 (paging moves to M1; the guard's everyday list and residual risks; share's publish arguments), DR-81 (`Ref` evidence and the branch), DR-8 (a scan's key) and DR-47 (the per-blob cache's key).
- **Amended by DR-87:**
  - A late scan also leaves a repo unchanged when another scan moved it after this one began and the repo set changed, not only when `HEAD` moved; `scan_superseded` names which.
  - Every git call also refuses each transport by name, so no call fetches even on a git without `GIT_NO_LAZY_FETCH`; a partial clone is told by reading its keys as git reads a boolean.
  - A workspace whose repos have no commits returns `repos_unborn`, and scan's summary counts no unborn or skipped repo.
  - The guard reads the shell's directory per call, checks every tool's paths and uploads, and protects the plugin's root.
- **Amended by DR-100:** the per-blob cache's key adds the file's repo-qualified path to the options this record added.
- **Sources (checked 2026-10-26):** `docs/contributing/research/skeleton-review/round2-*.json`; the four fix branches' reports; the merged tree's `cargo xtask check`, `test`, `test --installer` and `e2e`.

## DR-87: What the walking skeleton's review settled in round 3

- **Status:** accepted (decided at the merge of round 3's fixes, 2026-10-26). Amended by DR-88 and DR-97.
- **Starting point:** extended "Extractors read the repo", "Actors: agents never act as a person", "Surfaces read one contract" and Remote mode.
- **Origin:** round 3, the last, of the walking skeleton's review. Its 9 material findings, 8 distinct defects, are in [the review's index](research/skeleton-review/README.md), with what stays open in [its accepted risk](research/skeleton-review/accepted-risk.md). One fix repaired CI, and two fix branches fixed the rest (the engine and guard fixes).
- **Context:** Round 2's fixes held where they were aimed. Round 3 found their edges: a late scan could still move a repo back when the repo set changed rather than `HEAD`; a repo's own config could re-allow a transport on a git without `GIT_NO_LAZY_FETCH`; a workspace whose repos had no commit looped on a scan that mapped nothing; import's reply passed its budget with many rows; and the guard resolved paths against a directory the shell had left, checked only four tools' paths, and let an agent rewrite the mod that guards it.
- **Decision:**
  - **A late scan never moves a repo back, whatever moved.** A scan records the store's latest `seq` before it reads any repo (`ScanWrite.started`). Inside its transaction, `commit_scan` tells the caller when another scan moved the repo since then; the scan then re-derives its inputs under the write lock and leaves the repo unchanged unless they still match. `scan_superseded` gains `details.reason`: `head_moved` (round 2's case) or `repos_changed`, each with its own line. A late scan whose inputs are stale with no competing scan still commits; that isn't a move back, and the next scan corrects it.
  - **Every transport is refused by name.** Every git call through the adapter also passes `-c protocol.<name>.allow=never` for `ext`, `fd`, `file`, `git`, `ssh`, `http`, `https`, `ftp` and `ftps` (`extract::git::TRANSPORTS`), so a repo's own per-protocol allow can't beat `protocol.allow=never`, and "no call fetches" holds without `GIT_NO_LAZY_FETCH`. A repo is a partial clone when `extensions.partialclone` names a remote, or a `remote.<name>.promisor` is true, read as git reads a boolean (`git config --type=bool`). Only git's exit 1 means no key. Any other failure reading these keys skips the repo with `repo_skipped`, reason `git_failed`, never "not partial". M2's local clone and fetch take their exception for `file` alone.
  - **A workspace with no commits says to commit.** `focus` counts only repos with a commit as scannable. When none has one, `focus` and `map` return the empty reason `repos_unborn`, which names the repos and whose next action is `focus`, never a scan that maps nothing. When the top level is empty, every level takes the top's empty state. `scan`'s summary counts neither an unborn nor a skipped repo as mapped or moved, and names the unborn ones; an unborn repo's live frame says it maps after its first commit.
  - **Import's reply keeps to its budget.** `ImportResult` carries `counts` (`imported`, `held`, `conflict`, `refused`, `left`, `skipped`), the first 5 rows' outcomes (`IMPORT_ROWS_SAMPLE`), `root` (the sync's absolute directory) and `batch_files`: one `batch-N.json` per `ArtifactData batch` call, relative to `root`, each a `DbBatch` whose `file_path`s are absolute. `batches` is removed. Every row is still acked in `acks/<uid>`. This removes a field, which the versioning rules make a `schema_version` bump; no release has shipped, so it stays 1, as DR-86 did.
  - **The guard reads where the shell is.** It takes the shell's directory from `$.session.cwd()` on every call, and resolves relative words and its landing test against it; the places it derives from the environment stay cached. A `cd` or `pushd` into a guarded place is refused, so a host that didn't report the move still can't reach one through a relative word. When the shell's directory differs from the one the session started in, or the line runs `cd` or `pushd`, every word on the line is checked as a path.
  - **The guard checks every tool's paths.** Beyond Read, Grep, Glob, Edit, Write and Bash, it reads the path fields of Artifact (`file_path`, `file_paths`, `files` joined to `root`, `root`; `out_dir` as a write), ArtifactData (`file_path`, `writes[].file_path`, `batch[].file_path`; `out_dir` as a write), SendUserFile and its alias SendFile, LS, NotebookRead, LSP and Monitor (whose `command` is read as Bash). For a tool it doesn't know, every string that looks like a path is held to the read rule. A file sent off the machine must pass the read rule too, and from a state directory only `share/` may leave, where `share` and `import` write; nothing leaves from the data root.
  - **The guard protects the mod's own files.** The write rule also covers the plugin's root (`$.plugin.root` and the real path it resolves to). Deleting or moving the plugin's folder or a folder above it is refused, and so is a `cd` into the plugin's folder. A Bash word that lands there is denied unless it's the program a command runs, so the `planroom` skill's `"<plugin>/scripts/planroom" map --json` still runs, and the Read tool can still read the plugin's files.
  - **Developing Planroom with its own mod loaded.** When this repository's `plugin/` is loaded live as a mod, as `ws8-install.md` describes, it is the protected root: an agent can't edit `plugin/` until the mod is unloaded or a person makes the change. `plugin/AGENTS.md` says so.
  - **The mod reads the resolver's reason from the contract.** `UnavailableReason` becomes a schema root, and the mod switches on the generated type with a `never` default, so a reason the mod doesn't handle fails its build.
- **Alternatives rejected:**
  - Re-reading the whole workspace's inputs at every commit: the re-check runs only when another scan landed in between, which keeps a lone scan's cost unchanged.
  - Setting `GIT_ALLOW_PROTOCOL` instead of per-protocol flags: the flags are checked per call by a test that plants each transport, and the variable's interaction with the flags wasn't verified on every supported git.
  - Raising import's budget, or a `root` with relative paths inline: 40 rows still produce about 82 writes, near 1,500 tokens.
  - Denying every unknown tool: it would block MCP tools that have nothing to do with a secret.
  - Leaving the plugin root writable in dev mode: the guard can't tell a developer's agent from any other, and the person can still edit there.
- **Consequences:**
  - Contracts: `scan_superseded` `details.reason`; the empty reason `repos_unborn`; `ImportResult` loses `batches` and gains `counts`, `root` and `batch_files`, with the new type `ImportCounts`; `UnavailableReason` is a schema root. Schemas, TypeScript and the plugin's types are regenerated; `schema_version` stays 1.
  - Strings: `warning.scan_superseded_head_moved` and `_repos_changed`, `empty.repos_unborn`, `scan.unborn` and `frame.scan_unborn` in `en.toml`.
  - Tests: `crates/planroom/tests/scan.rs` (a late scan over a changed repo set, an unborn-only workspace), `crates/store/tests/commit.rs`, `tests/git_hardening.rs` (an `ext::` partial clone, a valueless `promisor`), `crates/extract/src/git.rs`'s transport plants with their controls, `budgets.rs` (import with 40 rows), `s11_remote.rs` (a 63-line sync), the guard's shell-directory, every-tool and plugin-root tests in `packages/mod/src/guard.test.ts` and `packages/mod/plugin-tests/guard.test.tsx`, and `engine.test.ts`'s unknown reason.
  - `SECURITY.md` (M5) adds the guard's residual risks listed in the accepted risk.
  - It amends DR-86 (the late scan, the transports, `repo_unborn` in scan's summary, the guard's reach), DR-83 (import's result) and DR-85 (the resolver's reason types).
- **Amended by DR-88:** a tool the guard doesn't know is held through every string it's given, not only strings that look like a path, and a string that reaches only a place the write rule holds is put to the person.
- **Sources (checked 2026-10-26):** `docs/contributing/research/skeleton-review/round3-*.json`; the two fix branches' reports; the merged tree's `cargo xtask check`, `test`, `test --installer` and `e2e`.
- **Amended by DR-97:** an unborn repo writes no snapshot and no scan event; `repo_unborn` and the scan's summary say it maps after its first commit, and the live frame's unborn line remains only for empty-commit events an older engine wrote.

## DR-88: The guard's residual risks, decided

- **Status:** accepted (decided at round 3's close, 2026-10-26). Amended by DR-90.
- **Starting point:** extended "Actors: agents never act as a person".
- **Origin:** the walking skeleton review's round 3: the guard's residual risks DR-85 to DR-87 left, and the minor security findings of rounds 2 and 3, each checked against the code before it was decided ([accepted risk](research/skeleton-review/accepted-risk.md#the-guards-residual-risks-decided)). The owner delegated these risk decisions on 2026-10-26.
- **Context:** Round 3 closed with a list of what the guard didn't cover. Checking each against the code confirmed them and found more: a deleting `find` (`find . -name '*.db' -delete`) removed the workspace's store, a quoted `'>'` turned a secret's parent into a redirect target and let a tree read pass, a tool the guard didn't know could write anywhere, and `cd $(...)` hid a guarded folder. With `HOME` unset the guard failed open, and the engine put its data root under the current directory.
- **Decision:**
  - **The guard sees every string a tool it doesn't know is given.** A `file:` URL is read as its path, a string with a space by the folder it spells and where it lands, a bare name where it lands. A string that reaches a guarded file is denied; one that only reaches a place the write rule holds is put to the person, unless the host calls the tool read-only. This replaces DR-87's "strings that look like a path, held to the read rule".
  - **It asks where it can't see.** Past 1,024 strings or 32 levels in a tool's input, or 1,024 paths to resolve, and with no absolute `HOME`, it answers ask, which a session nobody can be asked in turns into a deny.
  - **It reads a Bash line a second way,** with quoted or escaped operators as text and each command substitution as an unknown word, and denies what either reading denies. It follows `cd $(...)` by the words its substitution names, reads `~name` as this user's home, takes `dotglob` turned on in the line into account, and catches a `planroom` fed by `xargs`, `parallel`, `find -exec` or an expansion, and a guarded variable set by `+=`, `readonly`, `local`, `read` or `printf -v`.
  - **The plugin's code stays the plugin's.** The write rule covers the other versions in the host's plugin cache, the marketplace clone, `installed_plugins.json` and `known_marketplaces.json`. Bash names those folders only to run what's in them. A shell that sits in a protected folder runs only a line that starts with `cd DIR &&` out of it. The mod's area contract names the plugin's own files and files sent off the machine among what its guards hold.
  - **Clean-up never takes the state directory.** A deleting `find` is judged by what its tests may select; an `rm` or `mv` fed by `xargs` or a substitution by what feeds it; `git stash --all` is refused unless it keeps `.planroom`. Unpacking an archive, or a `mv`, onto the path to the data root, the mod's store, the user config or the plugin is refused.
  - **More tree readers count:** `git grep --no-index` and `--untracked`, `ugrep -r`, `bsdtar`, `pax`, the compressors with `-r`, `7z a` and `ditto`.
  - **Every engine reply to a tool call is marked untrusted.** `ToolResult` gains `untrusted: bool`, additive and omitted when false, so `schema_version` stays 1.
  - **The data root is absolute or nothing.** It is `PLANROOM_DATA`, else `XDG_DATA_HOME`, else `HOME/.local/share`, each only when absolute. Otherwise the engine and the resolver answer `invalid_argument` naming `PLANROOM_DATA`, with the hint `export PLANROOM_DATA=DIR`.
  - **The mod registers its key once per workspace in each mod's life,** and again after a press the store refuses as no person's, rather than trusting the list of workspaces it registered with.
  - **What M1 does for the rest:** its actor slice checks whether the session's Bash sandbox can deny every command reads of the person key, the gateway key and `serve.json`, and writes to the plugin, the data root and the state directory, while the engine is reached only through the mod's tools or one resolver path the sandbox excludes. The check covers Bash's `dangerouslyDisableSandbox`, `allowUnsandboxedCommands` and `excludedCommands`, and takes the sandbox settings from a source the agent can't write. It states the result per surface: a cloud container needs bubblewrap, macOS isn't run yet, Windows has no sandbox, and a call run on another machine through `_host` rests on the guard alone. Where the sandbox holds, it is the boundary, and the pane says when person actions rest on the guard alone. The same slice pins the registered person key's fingerprint under the data root and refuses a token when the store's key disagrees with it. Its mod slice checks live that `$.session.cwd()` follows a `cd`, and if it doesn't, the guard carries each call's `cd` forward. Its serve slice keeps the control token under the data root, not in `serve.json`, and removes a login page whose code expired unused.
  - **Open until M1:** a crafted tree unpacked into the workspace can replace `.planroom/`. No code yet trusts only a state directory the engine created: `workspace.rs`'s `find_root` takes any `.planroom/`. M1 lands DR-48's trust rule, and its actor slice pins the registered person key's fingerprint, so a planted store's key is refused. M1's backups restore the store.
  - **Accepted, with what limits each:** a script run by path and a program that walks or writes files itself (the default permission mode asks before each such command, but auto mode lets a classifier approve it and bypass mode asks nothing; every person action needs a token the person key signs); a variable or `dotglob` set outside the line; a tree read rooted at or above the workspace (until the control token moves); deleting a folder that holds another workspace (M1's backups restore the store); and no adversarial test of the guard yet, which is owed to a human security review before the first release. `SECURITY.md` (M5) states them to users.
- **Alternatives rejected:**
  - The OS keychain as the person key's place: the mod reaches it only through `security` or `secret-tool`, which any command the agent runs can call too; macOS prompts on each read only when the item trusts no app, so every press would prompt; libsecret hands an unlocked item to any process of the user; and cloud sessions have no keychain, so the file would stay where the agent runs most on its own. The better later option is a host-held signing handle: the host signs on the mod's behalf and the mod never reads the key, in the pattern of `$.session.authorize()`. A key that signs only with the person present, such as a passkey in the canvas, is the other. Both are left for a later record.
  - Refusing `python -c`, scripts and build tools: it would block everyday work, and no pattern can see what they do.
  - Refusing every recursive read rooted at the workspace: it would refuse `grep -rn foo .` and `grep -r foo`. A read rooted below it, such as `grep -rn foo src/`, already passes.
  - Denying every tool the guard doesn't know: it would block MCP tools that never touch a secret.
- **Consequences:**
  - Contracts: `ToolResult.untrusted`; `invalid_argument` with `details.field` `PLANROOM_DATA`; the resolver's `invalid_argument` names the field.
  - Layout: the mod's row in the area contracts table (gated; `layout.lock` names this record).
  - Tests: `packages/mod/src/guard.test.ts` (the unknown-tool test, the shell forms left after round 3, the protected-folder shell, security-r2-4 and security-r3-4 cases, tests-3-1, and the everyday list beside them); `packages/mod/plugin-tests/guard.test.tsx` and `actor.test.tsx`; `packages/mod/src/engine.test.ts`; `crates/planroom/tests/workspaces.rs::a_serve_with_no_home_writes_nothing_into_the_workspace`; `tests/installer/run.sh::a_data_root_with_no_absolute_place_is_refused`.
  - Milestones: the three M1 checks above join the actor, mod and serve slices, and the actor slice also pins the registered person key's fingerprint.
  - It amends DR-58 (an absolute data root), DR-75 (when the mod registers its key) and DR-87 (the guard's rule for tools it doesn't know).
- **Amended by DR-90:** the guard reads `**` as Bash does, takes a `sed` or `perl -i` script as no file it writes, lets a dry-run `git clean` and a `find -empty` delete pass, counts a bare `store.db` only in a state directory, asks on `_host` and `dangerouslyDisableSandbox`, and refuses a guarded variable in a start-up or settings file; the mod's runs set the engine variables empty.
- **Sources (checked 2026-10-26):** `packages/mod/src/guard.ts`, `register.tsx` and `person.ts`; `crates/planroom/src/app/data.rs`; `crates/planroom/src/server/routes.rs` (what the control token unlocks); `plugin/scripts/planroom`; [accepted risk](research/skeleton-review/accepted-risk.md).

## DR-89: What M1 builds, compares and leaves to later milestones

- **Status:** accepted (decided before M1's first change, 2026-10-26). Amended by DR-91 and DR-95.
- **Starting point:** none; it amends [Milestones](architecture.md#milestones) and serves S1, S3 and S15, and the feature list's gate (DR-59).
- **Origin:** milestone planning: M1's scope, read against the tree after DR-88.
- **Context:** M1's row in the Milestones table says it closes S1, S15 (most) and S3's deterministic part. Read against the code and `feature_list.json`, several things don't hold:
  - S1 can't pass in M1. `first_run.rs::first_scan_deals_at_most_three_cards` needs M2's first-scan card budget, and the full truth comparison needs M3's observability pack and cross-repo joins (the `deploys` linker, package versions), since TRUTH.json lists `deploys`, `scrapes`, `exports_telemetry` and `queries` relations and three `observability` containers.
  - Many tests `feature_list.json` lists don't exist at their paths: some exist under another name or file, some are planned for a later milestone, and the gate can't tell either from a typo.
  - S3's `ci_path` accepts ghosts with `decide CARD accept` (steps 5 and 6), but `decide` and its batch accepts arrive with M2's decision runner. M1's plan-pack slice still says "batch review".
  - M1 needs cards (workspace trust, a changed pack source, restore, and the Apply card for unsigned inbox edits) before `decide` exists, and no record says how a card is stored or picked.
  - `intent next` asks Claude, and M1 has neither the consent card nor the daily cap that M2's models row brings.
  - `budgets.json` has its shape in [Contracts](contracts.md) but no file yet; M2's budgets slice creates it, while M1's fold benchmark and surface numbers need it now. The rule that a raise needs a record doesn't say whether a new key is a raise.
  - DR-83 replaces `kernel::redact`'s hand-written field list with `#[redact]` marks, but no form for the marks exists, and the kernel can't include a file from outside its crate.
  - TRUTH.json's `healthcheck` container and its two `http` edges come only from the Ruby pack, which the Milestones table puts in M5.
  - Scanning this repository maps `examples/fixture/`'s compose services as its own containers ([Found by dogfooding](architecture.md#found-by-dogfooding)).
- **Decision:**
  - **S1 turns passing in M3.** M1 compares a stated subset of the truth and asserts what it leaves out:
    - **Containers:** all 11 by name, repo and every evidence item the code and Ruby packs declare; kind and role for the 8 whose kind the code pack declares (3 `service`, 1 `library`, 2 `tooling`, `queue` and `datastore`), with the `queue` and `datastore` roles from the code pack's `[[image_roles]]`.
    - **Components:** all 24.
    - **Relations:** 7 of the 23: `http` ×3 (storefront to orders through a wrapper call, and healthcheck to orders and to storefront through the Ruby pack), `db` ×2, `queue_publish` and `queue_consume`, each with every evidence item. An evidence item that names a branch matches through the scan event that read it (DR-86).
    - **Out of scope until M3:** `deploys` ×8, `scrapes` ×3, `exports_telemetry` ×3, `queries` ×1, `depends_on_package` ×1, the `observability` kind of `otel-collector`, `prometheus` and `grafana`, and the `collector.yaml` and `prometheus.yml` evidence items of `otel-collector` and `prometheus`, which only the observability pack reads. `s01_first_look.rs::every_truth_item_the_loaded_packs_declare_matches_with_evidence` asserts that the out-of-scope set is exactly this list, so nothing drops silently. `s01_first_look.rs::truth_recall_never_drops` holds recall at or above a committed floor that only rises.
    - **Still listed:** `every_truth_container_component_and_relation_with_evidence` is due M3, and `first_scan_deals_at_most_three_cards` is due M2. M1 emits `depends_on_package` without its `via`; M3 compares it.
  - **The feature list's tests move, are renamed, split and tagged, and none is weakened.** Every assertion stays; only a test's path, name or milestone changes.
    - **`due` tags:** each entry gains an optional `due` object from a listed test to the milestone that builds it, for example `"due": {"crates/planroom/tests/first_run.rs::first_scan_deals_at_most_three_cards": "M2"}`. The field is additive; the format stays `planroom.feature-list/v1`. A name that carried "(planned for M1)" or "(planned for M5)" becomes the bare name with a `due` tag. `tests/installer/run.sh::a_newer_dev_catalogue_misses_the_cache` moves from M5 to M1.
    - **What `feature_list_is_complete` means:** every listed test resolves to a test that runs under a CI `cargo xtask` command with no `PLANROOM_LIVE_*` flag and nothing live, or carries a `due`. A test that resolves carries no `due`, and an entry marked `passing` carries none. A milestone closes only when no listed test still carries its `due`.
    - **Moves:**

      | Listed now (feature list or traceability.md) | Moves to |
      |---|---|
      | `first_run.rs`: `submodules_are_repos`, `repos_two_levels_down_are_found`, `a_nested_workspace_warns`, `focus_lists_dirty_repos` | `discovery.rs` |
      | `first_run.rs::same_named_repos_get_distinct_namespaces` | `namespaces.rs` |
      | `first_run.rs::a_single_repo_workspace_keeps_its_state_in_the_git_dir` | `state_dir.rs` |
      | `store/tests/fs_check.rs::a_single_repo_keeps_its_state_in_the_git_dir` | merged into `state_dir.rs`'s test of the same assertion |
      | `first_run.rs::open_without_a_display_offers_share` | `open.rs` |
      | `actor.rs::an_agent_does_not_conflict_with_itself` | `edit.rs` |
      | `actor.rs`: `a_read_outside_the_workspace_is_path_refused`, `a_credential_path_needs_a_person`, `workspace_flag_on_a_read_needs_an_existing_store` | `confine.rs` |
      | `s01_first_look.rs::top_level_has_one_next_action` | `next.rs` |
      | `kernel/tests/fold.rs::the_link_snapshot_matches_the_active_set` | `crates/planroom/tests/link_snapshot.rs`, since it reads the store |
      | `trust.rs::credentials_go_only_to_allow_listed_hosts` | `crates/models/tests/hosts.rs` |
      | `privacy.rs::no_envelope_suggests_pushing_the_backup` | `backup.rs` |
      | `envelope_schema.rs::golden_outputs_equal_engine_output` | `golden.rs` |
      | `gates.rs`: `a_dispatch_run_checks_state_only`, `a_tag_push_checks_state_only` | `crates/xtask/src/layout.rs`, which holds them |
      | `crates/planroom/tests/docs.rs::every_image_has_its_prompt_and_spend_row` | `crates/xtask/tests/docs.rs` |
      | `tests/e2e/*.spec.ts` | `tests/e2e/src/*.spec.ts` |
      | `surfaces.test.tsx`, `keys.test.tsx` and `tools.test.ts` tests that already exist in `layout.test.ts` or `engine.test.ts` | the file that holds them |

    - **Renames to tests that already assert the step:**

      | Listed name | Existing test |
      |---|---|
      | `server_auth.rs::a_request_without_the_token_is_refused`, `a_missing_origin_is_refused` | `requests_without_a_token_or_from_another_origin_or_host_are_refused` |
      | `server_auth.rs::a_code_exchanged_twice_revokes_its_token` | `a_login_code_works_once_and_a_second_try_revokes_its_token` |
      | `server_auth.rs::a_taken_port_exits_with_port_in_use`, `serve_port_moves_only_when_asked` | `serve_keeps_its_port_and_another_program_on_it_is_port_in_use` |
      | `routes.rs::serves_the_embedded_canvas` (in `feature_list.json` and traceability.md), `routes.rs::serves_the_fix_page_without_a_canvas` | `server_auth.rs::the_canvas_or_its_fix_page_is_served_at_the_root` |
      | `first_run.rs::focus_outside_a_workspace_creates_nothing` | `focus.rs::focus_outside_a_workspace_is_an_empty_state_and_creates_nothing` |
      | `gates.rs::layout_gate_fails_without_a_record` | `a_changed_surface_fails_until_a_newer_record_ratifies_it` |
      | `gates.rs::a_missing_base_fails` | `a_range_that_cant_be_resolved_fails` |
      | `gates.rs::a_new_package_dependency_needs_a_record`, `a_dependency_version_bump_needs_none` | `a_dependency_version_bump_needs_none_and_a_new_dependency_needs_a_record` |
      | `gates.rs::an_edited_area_row_needs_a_record` | `an_edited_area_row_needs_a_record_and_its_formatting_does_not` |
      | `gates.rs::a_worktree_change_is_pending_not_failed`, `gen_layout_lock_refuses_in_a_worktree` | `a_worktree_change_is_pending_not_failed_and_gen_layout_lock_refuses_there` |

    - **A planned name changes:** DR-70 and traceability.md name the planned test `gates.rs::a_branch_merge_lists_its_gated_changes`; no test or feature list entry holds it yet.

    - **Stay where they are, due M1:** `gates.rs::ci_runs_only_xtask` and `gates.rs::area_contracts_match_repo_layout` name `cargo xtask check` steps today, not tests; M1's repo gates write them as tests under those names.
    - **Due M1 (repo gates), not moves:** `crates/planroom/tests/docs.rs::read_first_names_resolve` and `an_amended_record_says_so_in_its_status` exist nowhere yet. M1's repo gates slice writes them.
    - **Also due M1 (repo gates):** `crates/xtask/tests/gates.rs::every_directory_has_a_purpose`, `budgets_raise_needs_a_record` and `feature_list_is_complete` exist nowhere yet; the repo gates slice writes them under those names.

    - **Written under their listed names,** because no existing test asserts them: `server_auth.rs::the_code_is_never_in_an_argv` (a stub opener records its argv, which holds a file path under `DATA_ROOT/open/` and never the code) and `server_auth.rs::an_expired_or_reused_code_is_refused` (a code 61 s old and a reused code are both refused; the test moves the code's clock rather than waiting). `the_control_token_registers_a_login_code_by_its_digest` already asserts that a reused code is refused; no test asserts the 61 s expiry.
    - **The split:** `s03_plan.rs::lint_flags_vague_action_and_missing_done_test` becomes `lint_flags_a_missing_done_test` (M1, structural) and `lint_flags_a_vague_action` (M2, on Jev).
  - **`review` stays per item in M1, and M2 adds batch review.** `review ID accept|reject` keeps its catalogue row ([Command catalogue](contracts.md#command-catalogue)).
    - `s03_plan.rs::agent_accept_is_person_required` and `stages_produce_ghosts_until_accepted` land in M1 on `review GHOST accept`: an agent's accept is `person_required`, and a stage's ghosts stay ghosts until a person accepts them one by one.
    - M2 extends both onto `decide CARD accept`, one batch per stage, which are S3's steps 5 and 6. Their M1 assertions stay. S3's `ci_path` doesn't change.
    - `accepted_plan_is_version_2`, `next_actions_touch_the_three_targets`, `version_diff_renders_in_the_shared_view` and `tests/e2e/plan.spec.ts` are due M2, on batch accepts and the first version tag; `lint_flags_a_vague_action` is due M2, on Jev. S3 turns passing in M2, as before.
  - **`intent next` replays only, until M2's consent card and daily cap.** In M1 it answers from `tests/replay/`. A live call is made only in record mode, under a `PLANROOM_LIVE_*` flag and the build's spend log. Every other live call is refused with `budget_exceeded` before it is sent. The spend log works as [Spend guards](architecture.md#spend-guards) gives it: under its lock, the engine sums `docs/evidence/spend.jsonl`, refuses a call past `live_tests_total_usd` less $0.25, and appends the call; a relative path, or a live flag with no spend log, is refused. M2's consent card and the user-level daily cap lift the replay-only rule.
  - **Cards are an event kind.** A card is a `card` event (`EventBody::Card`) with `kind` (an open set: `apply`, `trust`, `pack_source`, `restore`), `subject`, `question` and `why` (their `en.toml` keys), `stakes`, `choices`, `default` and `dealt_by`. It fills `DecisionCard`: its question, its why line and its stakes come from the event, and `person_only` is true for every kind M1 deals.
    - **Versioning:** event kinds are an open set, and an older reader keeps an unknown kind as `EventBody::Unknown` and writes it back unchanged ([Versioning rules](contracts.md#versioning-rules)). So the kind is additive, and `schema_version` stays 1. The store needs no new table: the open cards are the `card` events no `review` names.
    - **Dealing:** only a write deals a card, never a read. `import` deals the Apply card when unsigned edit rows arrive. `scan` deals the trust card when the workspace's `.planroom/` is untrusted, the changed-source card when a loaded pack's source differs from its copy, and the restore card when the state directory is missing and a backup exists for the workspace root. No card is dealt twice while one of the same kind and subject is open. `scan` returns `workspace_untrusted` with the trust card it dealt, and `focus` carries it while that card is open.
    - **Picking:** `review CARD accept|reject` picks a card through the existing `Review { of }` body, with no change to the command or the event. A second pick returns `already_resolved`, and an agent's pick `person_required`. `focus` lists the open cards and puts the first face up in `PaneModel.decision`; M2's `decide ID CHOICE` picks the same events, read beside its `DecisionRecord`s.
    - **What each pick does:** accepting the trust card records the workspace as trusted and loads its config; rejecting keeps it unloaded. Accepting the changed-source card updates the pack's copy; rejecting keeps the old copy. Accepting the restore card restores from the backup, as `doctor --restore` does, which also resolves the card; rejecting starts empty. The Apply card is picked through its ghosts, one `review` each, and resolves with its last ghost; `review` on the card itself returns `invalid_argument`, whose hint names its ghosts, until M2's batch review picks it whole.
  - **`budgets.json` arrives in M1, in the shape Contracts gives it, and a new key isn't a raise.**
    - **The file:** M1 creates `budgets.json` at the repo root with the keys [Contracts](contracts.md) already defines (`fold.20k`, `fold.50k`, `surface`, `room_text`, `recorded` with its `token_ratio`, and the per-question keys as their questions land). `cargo xtask gen --budgets` stays in M2's budgets slice.
    - **What a raise is:** a larger value for a key that already exists. A commit that raises one adds a decision record, as DR-59 says. A new key isn't a raise and needs none. A person lowers a value by hand.
    - **The fold benchmark:** `crates/kernel/benches/fold.rs` builds its events in memory, so the kernel still does no I/O. `cargo xtask bench fold` runs `cargo bench fold` and compares the result with `fold.20k` and `fold.50k`; CI runs it on ubuntu-24.04 ([Performance budgets](architecture.md#performance-budgets)).
  - **Default excludes, with one setting** (R14). A scan skips directories named `fixture`, `fixtures`, `__fixtures__`, `testdata` or `test-data`, at any depth below a repo's root, so this repository no longer maps `examples/fixture/` as its own system. The setting `[scan] exclude` holds that list as globs relative to each repo's root (`**/fixture/**`, `**/fixtures/**`, `**/__fixtures__/**`, `**/testdata/**`, `**/test-data/**`), and a person's value replaces it. No default fits every workspace: the same directory name holds test data in one repo and product code in another, so a person needs a way to map one back in. The excludes are part of the scan's snapshot key (DR-86).
  - **The `#[redact]` form.** A repo-derived free-text field carries `#[schemars(extend("x-redact" = true))]`. `cargo xtask gen` writes the field paths into `crates/kernel/src/redact_fields.rs`, inside the kernel, and `kernel::redact` masks exactly those paths. AGENTS.md's Logging line changes to this form in M1's first change, with the layout lock, since its Conventions section is gated; the design pages describe it now.
  - **The Ruby pack comes into M1,** whole, as [Ruby through the extractor contract](architecture.md#ruby-through-the-extractor-contract) gives it: `ruby-http`, the `ruby` `[[languages]]` entry and `ruby-script`. M5's toy-pack slice keeps the flags pack and S10.
- **Alternatives rejected:**
  - Keeping S1 in M1 by dropping the truth items M1 can't reach: that weakens the entry, which only an infeasibility record may do.
  - Leaving unbuilt tests listed without a milestone: the gate couldn't tell an unbuilt test from a mistyped one.
  - Bringing `decide` and batch accepts into M1: they belong with M2's decision runner, its batches per kind and its card budget, and would land without them.
  - A new leaf that picks cards: a command and an undo class for a stopgap that `review` already covers.
  - A card table in the store: a migration for what the event log already holds, and one more thing the backup and the event export would have to carry.
  - Live `intent next` in M1 behind a per-call cap: without a consent card and a daily cap, a call could spend without a person's yes.
  - Counting a new `budgets.json` key as a raise: every new gate would need a record for being added, which discourages adding gates.
  - An `include!` of a generated list from outside the kernel: the kernel's path rule refuses it. A hand-kept list is the drift DR-83 removes.
  - A setting with no default excludes: every first scan of a repo with test data would map it, as this repository's did.
- **Consequences:**
  - **Contracts:** the event kind `card` (`EventBody::Card`), so the event model has nine kinds; the optional `due` object in `feature_list.json`; the `scan.exclude` setting with its default. `schema_version` stays 1.
  - **architecture.md, [Milestones](architecture.md#milestones):**
    - M1's row: "S15 (most), S3 (deterministic part)", plus S1's M1 subset, which M1 builds without closing S1.
    - M3's row: "S1, S2", and its cross-repo joins slice names S1.
    - M1's plan-pack slice: "ghosts and per-item review" in place of "ghosts and batch review", and `intent next` replays only until M2.
    - M2's decision runner slice: gains "batch review".
    - M5's slice "The toy pack, the Ruby pack and S10" becomes "The toy pack and S10", and its hour for the Ruby pack moves to M1's packs slice: M1 becomes 122 h and M5 75 h, so the total stays 382 h.
  - **traceability.md:** S1's row notes that S1 closes in M3, with M1's subset; S3's row moves batch review to M2.
  - **M1's first change:** the `due` tags, the moves, the renames and the split above land in `feature_list.json`, traceability.md and architecture.md's [Security and privacy](architecture.md#security-and-privacy) table together, so the three agree until then. The same change writes AGENTS.md's `#[redact]` line, with `cargo xtask gen --layout-lock DR-89`.
  - **contracts.md:** the Feature list row and the `feature_list.json` shape state the `due` rule and what a complete list means; the event model gains the `card` kind; the `Review` comment names cards beside ghosts, conflicts and inferred facts; `decide ID CHOICE` reads `card` events in M2.
  - **architecture.md, elsewhere:** the counts table and the event-model section say nine event kinds; the scan bounds and the snapshot key's inputs name the default excludes; redaction describes the `#[redact]` form; the fold row names `cargo xtask bench fold`.
  - **Elsewhere:** contracts.md's redaction paragraph and samples show the `#[redact]` form; repo-layout.md's task table gains `cargo xtask bench fold`, built in M1.
  - **How to read a record:** it gains the "Extended by DR-n" bullet and its Status form, which DR-65's Status uses for this record, and "milestone planning" as an origin.
  - **Tests:** the new and written-under-their-name tests above, among them `s01_first_look.rs::every_truth_item_the_loaded_packs_declare_matches_with_evidence` and `truth_recall_never_drops`; `trust.rs::accepting_the_trust_card_loads_the_config`; `packs.rs::accepting_the_changed_source_card_updates_the_copy`; `backup.rs::a_lost_state_directory_then_scan_offers_one_restore_card`; `cards.rs` (a dealt card listed face up, a second pick `already_resolved`, an agent's pick `person_required`, no second open card, the Apply card resolved by its last ghost); `config.rs::fixture_and_test_data_trees_are_excluded_by_default`; `live_gate.rs` (the cap, its margin, a relative path, a live flag with no spend log, `intent next` without a record flag); `gates.rs::a_new_budgets_key_is_not_a_raise`, `feature_list_is_complete`.
  - It amends DR-3 (a ninth event kind, `card`), DR-12 (full evidence recall is verified in M3, with a stated subset in M1), DR-59 (what `feature_list_is_complete` checks, and what a raise is) and DR-83 (the form of the `#[redact]` marks), and extends DR-65 (in M1, `intent next` answers from replay).
- **Amended by DR-91:** a card's own kind travels as `card_kind`, since `kind` tags the event body, and a card carries `vars`, the values its question and why lines name.
- **Amended by DR-95:** a budgets raise is a larger ceiling, a smaller `_rate` floor, a removed key, a number set back to null or a number replaced by another shape; a new key, a first number and `recorded` are none.
- **Sources (checked 2026-10-26):** `architecture.md` ([Milestones](architecture.md#milestones), trust in [Where the workspace lives](architecture.md#where-the-workspace-lives), [Ruby through the extractor contract](architecture.md#ruby-through-the-extractor-contract), `budgets.json` in [Token budgets](architecture.md#token-budgets-s7), [Spend guards](architecture.md#spend-guards), `intent next` in [Planning](architecture.md#planning-stages-ghosts-and-lint-r5), the server's exposure in [Security and privacy](architecture.md#security-and-privacy), the fold benchmark in [Performance budgets](architecture.md#performance-budgets), [Found by dogfooding](architecture.md#found-by-dogfooding)); `contracts.md` (open sets in [Versioning rules](contracts.md#versioning-rules), the Feature list row in [Every public contract](contracts.md#every-public-contract), `review` in [Command catalogue](contracts.md#command-catalogue), [Event model](contracts.md#event-model)); `feature_list.json` (S1, S3, C15); `examples/TRUTH.json` (`containers`, `relations`); `crates/contract/src/event.rs`; `crates/store/migrations/0001_init.sql`; `crates/planroom/tests/server_auth.rs`; `crates/planroom/src/server/auth.rs` (`CODE_LIFE`).

## DR-90: The guard lets everyday work through, and closes three more doors

- **Status:** accepted (decided 2026-10-26).
- **Starting point:** extended "Actors: agents never act as a person".
- **Origin:** a usability review of the guard after DR-88: 249 ordinary calls a coding agent makes, run through the guard, and a reading of DR-88's reasons against the code.
- **Context:** The guard refused 21 of 249 everyday calls, 15 of them unintended: a dry-run `git clean`, `**` globs, `sed -i` and `perl -pi` scripts, a deleting `find -empty`, the names `store.db` and `ledger.db` in an app's own files, and Glob patterns. The review also found that a call run on another attached machine (`_host`) or outside the sandbox passed unchecked, and that a Write or Edit could put `PLANROOM_ENGINE` in a shell start-up or Claude Code settings file, where a later press would inherit it.
- **Decision:**
  - **Everyday calls pass.** A `**` matches no hidden folder unless the line turns `dotglob` on, as in Bash. A `sed` or `perl -i` script is not a file it writes. In a deleting `find`, `-empty` and `-size 0` select only empty files (a WAL, a journal or the data root's lock among them) and, for `-size 0`, folders, and `-delete` binds to its own `-o` branch, so `find . -empty -type d -delete` passes. A dry-run `git clean` passes unless a later `--no-dry-run` turns it off. A bare `store.db` or `ledger.db` counts only in a state directory, or behind a glob or expansion that may stand for one.
  - **Three more doors close.** A call the guard would let through is asked when it sets `_host` or `dangerouslyDisableSandbox`. No Write, Edit, MultiEdit or Bash write may put a guarded variable in a shell start-up file or a Claude Code settings file. Every engine and resolver run the mod starts sets `PLANROOM_ENGINE` and `PLANROOM_RELEASE_BASE_URL` empty, which the resolver reads as unset.
  - **A backslash-newline joins a word as the shell joins it:** an unescaped one outside single quotes, and inside a quoted `sh -c` script as that shell would. A comment ends at a newline either way.
  - **The shell's indirections are read.** `env -C` and `sudo -D`, with long options abbreviated as getopt allows, move a command as `cd` does; `env -` and `-i` start none; `env -S` runs its value, and is asked when the value is attached. `${X:-word}` and the other default forms are read as `word` too, and a known variable's pattern operators are applied. Any other operator, two expansions joined in one name, and a variable the line sets by `read`, `mapfile`, `printf -v` or a substitution that prints a word may be any name, a hidden one included; a `for` variable takes its list.
  - **Filters and the data root.** A Grep or Glob filter that singles out a state file, has a stem that isn't a bare wildcard, or may lie in a state directory is refused. Any Bash write to a start-up or settings file is asked; a Write, Edit or MultiEdit there may not set `PLANROOM_DATA`, `XDG_DATA_HOME` or `HOME` either. Every run the mod starts sets `PLANROOM_DATA` to the data root the session's starting environment placed, or to `unplaced`, which the resolver refuses, when it gave no place, and waits for that pin before it starts. A `dangerouslyDisableSandbox` of any true value is asked.
  - **Accepted:** system-wide start-up files (`/etc/profile` and the like), and an Edit that assembles a guarded name from text already in a file; the mod's empty engine variables and pinned data root still keep these from picking what a press runs. A variable set outside the line still reads as one plain name. A `sed` script's own `w` and `e` commands write and run through their arguments, the accepted class of programs that do. And a usability cost: a loop that writes or removes under a variable the line sets and the guard can't place, such as `for f in a/x b/y; do rm $f; done` or `while read f; do rm "$f"; done < list`, is refused; M1's sandbox check decides whether it can relax.
  - **A side effect a person may notice:** exporting `PLANROOM_ENGINE` to try a local engine no longer reaches the mod's runs. Running the resolver by hand, and the dev checkout's own build, still use it.
- **Alternatives rejected:**
  - Refusing every write to a settings file: it would block an agent's legitimate project settings.
  - Holding the Glob tool to a separate rule: one `**` rule for Bash, Grep and Glob is simpler, and a Glob lists names, never contents.
- **Consequences:**
  - Tests: `packages/mod/src/guard.test.ts` ("everyday calls from a usability sweep pass the guard", the `**`, in-place script, `find -empty`, dry-run `git clean`, `store.db`, `_host` and sandbox, start-up and settings file, and backslash-newline cases); `packages/mod/plugin-tests/guard.test.tsx`; `actor.test.tsx` ("no run the mod starts takes its engine from the environment it inherited"); `tests/installer/run.sh::empty_overrides_count_as_unset`; the `review-*` and `review2-*` cases in `guard.test.ts`; `actor.test.tsx`'s data-root tests ("every run the mod starts names the data root the session started with", and the run with no place, which names `unplaced`).
  - It amends DR-88 (the guard's Bash reading and its read rule for store names).
- **Sources (checked 2026-10-26):** `packages/mod/src/guard.ts`, `live.ts` and `register.tsx`; `plugin/scripts/planroom` (an empty override is unset); [accepted risk](research/skeleton-review/accepted-risk.md#the-guards-residual-risks-decided).

## DR-91: The contract M1 builds on: cards on the wire, the edit ops, and the redaction marks

- **Status:** accepted (decided at M1's first change, 2026-10-26). Amended by DR-92.
- **Starting point:** none; it settles details of [Contracts](contracts.md) and AGENTS.md's Logging convention.
- **Origin:** M1's first change, read against the docs: the contract types M1 builds on (every documented error and warning code, the card event, the pack manifest, the vocabulary, M1's request shapes), the kernel's `resolve` and `fold` signatures, and the redaction marks DR-89 gave a form.
- **Context:**
  - DR-89 names a card's own field `kind`, but `EventBody` is tagged on `kind`, so the two would collide in one JSON object.
  - DR-89's card names its question and why lines by `en.toml` key, and has no place for the values those lines hold, such as an edit count or a date.
  - contracts.md's first sketch of `EditOp` named a `regroup` op and left out `unrelate`, while architecture.md's CLI table and the design language name `edit group` and `edit unrelate`.
  - The hand-written redaction list also masked block names, diff hunks and what people are doing, which contracts.md's list of marked fields left out, and a few free-text fields had no decision either way.
  - AGENTS.md's Logging convention still says `#[redact]`. DR-89 gave the mark its form and changes the convention at M1's first change, and that section is gated.
- **Decision:**
  - **A card travels as `card_kind`.** The Rust field stays `kind`; its JSON name is `card_kind`, as a decision record's is `decision_kind`. Its values stay DR-89's open set.
  - **A card carries `vars`,** a map from name to string holding the values its `question` and `why` lines fill in, left out when empty. The lines stay `en.toml` keys, so the engine renders and sanitizes them at display.
  - **`EditOp` follows architecture.md's CLI table:** `group`, not `regroup`, and `unrelate` beside `relate`. `set`, `text` and `propose` arrive with the `edit` leaf, which fixes their fields.
  - **Which fields carry the mark.** Besides contracts.md's list: node and block names, crumbs, mini-blocks, ask and find item names, diff hunks, presence `doing`, a hand-off step's `note`, since a note can quote a test's output, and a card's `vars`, which can name a repo or a pack. Join keys stay unmarked, as `Hint.key` already is: `Edge.via` and `Node.aliases` are keys a later read resolves. An edit's `fields` and `over` stay unmarked until the `edit` leaf gives them types, because a mark on a whole map would mask the IDs in it.
  - **AGENTS.md's Logging line names the mark and the generated list,** as DR-89 decided: `cargo xtask gen` writes the marked paths into `crates/kernel/src/redact_fields.rs`, and `kernel::redact` masks exactly those fields.
  - **`Stakes` and `ModelRequestRef` match contracts.md:** `Stakes` is `low | person`, and `ModelRequestRef` is `{ id, purpose, sha256, rerun }`. Nothing ever wrote the code's extra `medium` and `high` or its `est_tokens`, so `schema_version` stays 1.
- **Alternatives rejected:**
  - Renaming `EventBody`'s tag: every stored event carries `kind`, and a rename breaks every reader.
  - Storing a card's rendered question: the event would hold text the engine couldn't render in another language or sanitize at display.
  - Marking `Edge.via` and `Node.aliases`: they are join keys, and the edge ID hashes `via`.
  - Keeping `regroup` beside the CLI table's `group`: two names for one op.
- **Consequences:**
  - Contracts: `Card.card_kind` and `Card.vars`, marked; `EditOp::Group` and `EditOp::Unrelate`; the marked fields in the redaction paragraph; `evidence_dropped`'s `details {repo, count, items}`.
  - Conventions: AGENTS.md's Logging line, ratified with the layout lock.
  - Tests: `crates/contract/tests/schema.rs::every_redacted_field_is_listed`; `crates/contract/src/card.rs`'s round trips; `crates/contract/src/requests.rs::an_edit_parses_from_its_op_and_travels_with_it_flattened`.
  - It amends DR-89 (the card's wire name and `vars`) and DR-83 (the marked fields).
- **Sources (checked 2026-10-26):** `crates/contract/src/{card.rs, event.rs, requests.rs, model.rs, view.rs, schema.rs}`; `crates/kernel/src/redact.rs`; [Contracts](contracts.md) (CLI JSON envelope's redaction paragraph, Event model, the requests sketch); [CLI](architecture.md#one-command-catalogue) and [Design language](design-language.md#edit-by-hand) for the edit ops.
- **Amended by DR-92:** `set`, `text` and `propose` arrive with the plan pack's intent leaves, which fix their fields, rather than with the `edit` leaf; `forbid` with no kind waits for a stored form of a rule over every kind.

## DR-92: The truth comparator's floor, edit's first ops, the dirty count's git call, and test-only seams

- **Status:** accepted (decided at the merges of M1's truth comparator, discovery, person authority and edit work, 2026-10-27).
- **Starting point:** extended DR-71 (as DR-85 amended it), DR-73, DR-75, DR-86, DR-89 and DR-91.
- **Origin:** the merges of the truth comparator, the dirty-repo count, person authority and the `edit` leaf, each read against the docs by an independent check or at merge.
- **Context:**
  - The comparator lives in `planroom-testkit`, whose area contract lists `assert_cmd`, `tempfile` and `serde_json`, but whose `boundaries.toml` row had no allow-list, and `boundaries.toml` is gated.
  - The recall floor only fell: a change that raised recall passed with a note, so what it gained stayed unprotected.
  - No read shows a block's own evidence (`MapNode` has none), and a lifted edge carries at most 3 evidence items, so a container's evidence and a relation with 4 truth items can't match through the map.
  - `set`, `text` and `propose` take fields that only the plan pack's intent leaves fix, and `forbid` with no kind has no stored form: an edge needs one kind.
  - An HTTP edit with no client fell back to one shared ID, so two tabs of one person were one writer instance; and a base past the store's head passed compare-and-set as covering every write before it.
  - DR-71, as DR-85 amended it, names `git status --porcelain=v1 -z --untracked-files=normal` as the dirty signal, but plain `status` runs a clean filter the repo's config names, and a submodule's own filters in its working tree.
  - `open` and the login code need a way for tests to stand in for the system opener and to age a code past 60 s with no real wait, and neither may become a setting a person or an agent can reach (DR-75).
- **Decision:**
  - **The testkit's allow-list** is `assert_cmd`, `tempfile` and `serde_json`, as its area contract already says.
  - **The floor fails both ways:** `truth_recall_never_drops` fails when a floor check no longer holds and when a check holds that the floor doesn't name, so a change that raises recall raises the floor in the same change.
  - **Two reads are owed** before full evidence recall: a block's own evidence and a relation's whole evidence. Until they exist, those checks count as missing, never as passing.
  - **`edit`'s first ops** are `move`, `group`, `rename`, `hide`, `pin`, `relate`, `unrelate`, `forbid` with a kind, and `undo`. `set`, `text` and `propose` arrive with the plan pack's intent leaves, which fix their fields; `forbid` with no kind is `invalid_argument` until a rule over every kind has a stored form.
  - **An HTTP write names its tab's client,** or it is `invalid_argument` on `client`, so each tab is its own writer instance (DR-5, DR-73). **A base past the head** is `invalid_argument` on `base`, checked inside the write transaction.
  - **The dirty count's git call** turns off every filter driver git's config names, through `GIT_CONFIG_KEY_n`, and adds `--ignore-submodules=dirty`; a submodule the workspace maps reports its own status. The adapter's hooks, fsmonitor and transports stay off as for every git call.
  - **Test-only seams:** the opener is `PLANROOM_TEST_OPENER`, read only in builds with `debug_assertions`, so release builds compile it out, and the mod blanks it for every engine run as it blanks `PLANROOM_RAW`; the login code's clock is a constructor only tests call. Neither is documented as a setting.
- **Alternatives rejected:**
  - Lowering the floor when a change trades one check for another: a floor that can fall guards nothing.
  - Fixing `set`, `text` and `propose`'s fields now: they would be guessed, and the intent leaves would change them.
  - A shared default client over HTTP: it merges tabs a person keeps apart.
  - Plain `git status`: it runs repo-configured commands.
  - An environment variable for the clock: an agent could set it.
- **Consequences:**
  - Boundaries: `boundaries.toml`'s testkit row, ratified with the layout lock.
  - Contracts: `WriteCommon.client` required over HTTP; `EditOp::Forbid`'s `None` refused; the pending ops' comment.
  - Tests: `crates/testkit/tests/truth.rs`; `s01_first_look.rs::truth_recall_never_drops`; `edit.rs::a_base_past_the_head_is_refused`, `an_http_write_without_its_client_is_refused`; the git adapter's `counting_uncommitted_files_runs_no_filter_the_repo_or_a_submodule_names`; `server_auth.rs::an_expired_or_reused_code_is_refused`; `open.rs`.
  - It amends DR-71 (the dirty count's git call) and DR-91 (when `set`, `text` and `propose` arrive).
- **Sources (checked 2026-10-27):** `crates/testkit/src/truth.rs`; `crates/planroom/src/app/{edit, open, dirty}.rs`; `crates/planroom/src/server/auth.rs`; `crates/extract/src/git.rs`; [Testing strategy](architecture.md#testing-strategy); [Contracts](contracts.md).

## DR-93: The model adapter's crate edges, its transport's crypto provider, and an environment variable's own hint

- **Status:** accepted (decided at the merge of M1's Claude adapter, 2026-10-27).
- **Starting point:** extended DR-39, DR-48, DR-66 and DR-85.
- **Origin:** the merge of the Claude adapter, the replay store, the live gate and the build-wide spend log, read against the docs by an independent check.
- **Context:**
  - The engine now calls `planroom-models`, and `cargo xtask live` resolves the spend log through it, so the crate graph gains `planroom -> planroom-models`, `planroom-models -> planroom-contract` and `xtask -> planroom-models`. `boundaries.toml` already allows each edge, but the crate graph is gated.
  - reqwest 0.13's `rustls` feature turns on the aws-lc-rs provider, while DR-39 decides ring. Naming ring's provider takes `rustls` as a direct dependency of `planroom-models`, whose allow-list didn't list it.
  - Every CLI `invalid_argument` names the `schema --command LEAF` handle in its `hint` (DR-66, DR-85). For a bad environment variable (`PLANROOM_SPEND_LOG`, a `PLANROOM_LIVE_*` flag, `PLANROOM_REPLAY`), that handle describes arguments and can't help; the engine's own hint is the command that sets the variable.
  - The check that the transport carries no other TLS stack is a test the plan didn't name.
- **Decision:**
  - **The three crate edges** are ratified with the layout lock.
  - **ring, and nothing else:** `planroom-models` takes `rustls` (default features off; `ring`, `std`, `tls12`), reqwest takes `rustls-no-provider`, and the models crate installs ring's provider as the process default. `deny.toml` bans aws-lc. `rustls` joins the models row of `boundaries.toml` and the area contracts table.
  - **An environment variable keeps its own hint:** an `invalid_argument` whose `field` is a `PLANROOM_` variable keeps the engine's hint, the exact command that sets it, under `--json` too; its `details` still carry `synopsis` and `schema`.
  - **The transport's test** is `crates/models/tests/transport.rs::the_live_transport_is_rustls_with_ring_and_nothing_else`: `Cargo.lock` holds no OpenSSL, native-tls or aws-lc crate, and the process provider is ring's.
- **Alternatives rejected:**
  - aws-lc-rs, reqwest's default: DR-39 chose ring for its shorter build and no C toolchain.
  - The schema handle for an environment variable: a person or agent reading it learns nothing that fixes the variable.
- **Consequences:**
  - Boundaries: `boundaries.toml`'s models row; the area contracts table's models row; the crate graph; ratified with the layout lock.
  - Contracts: the `invalid_argument` row's `hint` sentence.
  - Tests: `live_gate.rs::a_live_flag_without_a_spend_log_is_refused` checks the `--json` hint; `models/tests/transport.rs`.
- **Sources (checked 2026-10-27):** `crates/models/{Cargo.toml, src/}`; `crates/planroom/src/cli.rs` (`with_usage`); `deny.toml`; [Contracts](contracts.md#error-codes); DR-39's library table.

## DR-94: Declared IDs at most 64 characters, and the listing's own left-out count

- **Status:** accepted (decided at the merge of M1's questions and `ask`, 2026-10-27).
- **Starting point:** extended DR-15, DR-46 and DR-86.
- **Origin:** the merge of the questions work, read against the docs by an independent check.
- **Context:**
  - A pack's IDs had no length bound. A reply that must keep to its leaf's budget (DR-86) and also echo a question's ID cut a long ID short, so `AskResult.question` and a listing entry could hold a value that is no ID and that a caller can't pass back.
  - Question IDs weren't checked at load at all, and built-in ones hold hyphens (`fires-if`), so the declared-ID syntax doesn't fit them.
  - The listing reused `truncated`'s `left_out`, which already counts the warnings the CLI's whole-reply check leaves out.
- **Decision:**
  - **Every declared ID is at most 64 characters:** a pack, kind, relation, hint, lens, language, extractor or question ID longer than that is `pack_invalid` at the line that declares it. Question IDs get the bound alone, not the `[a-z0-9_]` syntax. No reply cuts an ID.
  - **The listing's count** of entries too long for any page is `entries_left_out` in its `truncated` details.
- **Alternatives rejected:**
  - Echoing an ID whole past the budget: a reply over its leaf's budget breaks DR-86.
  - Cutting an ID only past the budget: an ID that sometimes isn't one can't be passed back.
- **Consequences:**
  - Contracts: the pack manifest's ID comment; the `truncated` row's `entries_left_out`.
  - Tests: `packs.rs::a_declared_id_past_64_characters_is_refused`.
  - Known gap, for M2's linkers: `path` misses a route that enters a repo through its root file and leaves toward a block inside it, since roll-up lifts an edge into that file to the repo and steps an edge out of it down to the file.
- **Sources (checked 2026-10-27):** `crates/planroom/src/packs/validate.rs` (`syntax`, `bounded`); `crates/planroom/src/app/ask.rs`; [Contracts](contracts.md#pack-manifest).

## DR-95: The check gates' edges: what a budgets raise is, how the baseline and the release are found, and what the secret scan exempts

- **Status:** accepted (decided at the merge of M1's check gates, 2026-10-27).
- **Starting point:** extended DR-59, DR-79, DR-83 and DR-85; amends DR-89.
- **Origin:** the merge of the check gates, read against the docs by an independent check over two rounds.
- **Context:**
  - DR-89 calls a raise "a larger value for a key that already exists", but `cache_hit_rate` is a floor, so a smaller value loosens it, and removing a key or setting it back to null drops its bound altogether.
  - The breaking-change guard compares with "the last release", which the docs don't say how to find, and a release commit dates its CHANGELOG section before the owner tags it.
  - `gen --schema-baseline` moves the baseline only with a `schema_version` bump, while no release exists yet.
  - The secret scan must never fail on a hash or an ID, yet a credential can be hex or digits.
  - The scan reaches the kernel's named shapes, a new crate edge `xtask -> planroom-kernel`; the crate graph is gated.
- **Decision:**
  - **A raise** is a larger ceiling, a smaller `_rate` floor, a removed key, a number set back to null, or a number replaced by another shape; a new key, a first number and `recorded` are none. Each needs a record in the same change. This amends DR-89.
  - **The last release** is the newest `## [X.Y.Z]` section of CHANGELOG.md other than the tree's own version; its `vX.Y.Z` tag must be present, or the check fails. Before CHANGELOG.md names one, only the baseline is compared.
  - **The baseline** moves only with a bump. Until v0.1.0 it is the schema of the tree that merged the guard; additive changes since pass the guard.
  - **The secret scan** exempts a hash, number or ID only under a handle field Planroom itself writes (`view_key`), and an already-masked value or a literal anywhere; under any other name every value is a finding. The list fails safe and grows only in a reviewed change.
  - **The `xtask -> planroom-kernel` edge** is ratified with the layout lock.
- **Alternatives rejected:**
  - DR-89's literal wording: it lets a lowered floor or a deleted ceiling through without a record.
  - The workspace version or `git tag -l` for the last release: one moves before the tag exists, the other depends on what was fetched.
  - A list of credential names: a credential named in no list passes.
- **Consequences:**
  - Contracts: the budgets.json paragraph's raise sentence; the breaking-change guard's test home in `crates/xtask/tests/schema_guard.rs`.
  - Accepted: the guard pairs an untagged variant with the new variant holding all its properties, so a change that removes a property and also adds a variant that keeps it can pass as additive.
  - Tests: `crates/xtask/tests/{schema_guard, gates}.rs`; `crates/planroom/tests/release_pins.rs`; xtask's secrets unit tests.
- **Sources (checked 2026-10-27):** `crates/xtask/src/{schema_guard, secrets, layout, testrun, report}.rs`; `crates/kernel/src/redact.rs`; [Contracts](contracts.md); [Repo layout](repo-layout.md).

## DR-96: Where a store lives on an unsafe filesystem, its pointer's type, migration copies, and the engine's libc calls

- **Status:** accepted (decided at the merge of M1's store open and migrations, 2026-10-27).
- **Starting point:** extended DR-1, DR-53, DR-69, DR-78 and DR-85.
- **Origin:** the merge of the store's open path, read against the docs by an independent check over two rounds.
- **Context:**
  - SQLite's write-ahead log needs a local filesystem (DR-1). The store crate may not read the filesystem type itself, since it has no `libc`; the planroom crate's `libc` line named only `fork` and `setsid` (DR-69), and the git owner check (DR-78) also needs the effective user ID.
  - `store.pointer` is an internal persisted file, and DR-85 keeps such files' types in the planroom crate, but the store crate, which may depend only on the contract, must read it.
  - A migration's `VACUUM INTO` copy is a whole store: it needs a place a person can find, a bound on how many stay, and a way to clear what a killed process left without touching a copy another store is writing.
- **Decision:**
  - **The planroom crate's `libc` calls** are `fork`, `setsid`, `statfs` (the filesystem type, read in `app/fs_type.rs` and registered with the store once at start-up) and `geteuid` (the git owner check). The area contracts row says so, ratified with the layout lock.
  - **An unsafe filesystem:** a new store whose state directory is on one is created under `DATA_ROOT/stores/<workspace_id>/` with `store.pointer` left in the state directory; an existing store stays where it is; an unsafe data root keeps a new store in place. Each case is one `unsafe_filesystem` warning per command.
  - **`StorePointer` lives in the contract crate,** read through `contract::format`, an exception to DR-85's rule for internal files, since the store crate reads it. A pointer is followed only to its one place under the data root, through no symbolic link, to a store of the same workspace; anything else is refused as `internal` naming the file, for now.
  - **Migration copies** go to `DATA_ROOT/backups/<workspace_id>/store-vN.db` at 0600 in a 0700 folder, checked with `integrity_check`, the last three kept; with no data root a migration from a store with a schema is refused rather than run without a copy. A temporary copy unwritten for ten minutes is a killed process's and is swept; a fresher one is left, since another store sharing the workspace ID may be writing it.
  - **Every write transaction checks the store version,** layouts and presence included.
- **Alternatives rejected:**
  - Moving an existing store off an unsafe filesystem: a move a person didn't ask for loses their place in the tree.
  - Sweeping every temporary copy under the write lock: the lock is the store's, not the folder's, so a copied state directory's store could lose its copy mid-write.
  - A dedicated `path_refused` reason for a bad pointer now: deferred until a pointer can come from anywhere but a person's own copy of a state directory.
- **Consequences:**
  - Boundaries: the planroom crate's `libc` line in the area contracts table, ratified with the layout lock.
  - Contracts: `planroom.store-pointer/v1`; the `unsafe_filesystem` warning; the `store_migrated` notice; `schema_mismatch`'s hint names the update commands.
  - Tests: `crates/store/tests/{fs_check, crash, migrations}.rs`; `crates/planroom/tests/backup.rs`.
- **Sources (checked 2026-10-27):** `crates/store/src/{lib, migrate, pointer, fs_check}.rs`; `crates/planroom/src/app/fs_type.rs`; [Where the workspace lives](architecture.md#where-the-workspace-lives); [Contracts](contracts.md).

## DR-97: Namespaces fixed at the first mapping scan, reserved names, and an unborn repo that writes no event

- **Status:** accepted (decided at the merge of M1's namespaces, renames and fold benchmark, 2026-10-27).
- **Starting point:** extended DR-47 and DR-89; amends DR-74 and DR-87.
- **Origin:** the merge of the namespaces work, read against the docs by an independent check over three rounds.
- **Context:**
  - DR-74 adds `-` and 4 hex to the later of two same-named repos and fixes a root's namespace at its first scan. A repo named `ws` or `plan` would start IDs in the namespaces of workspace nodes and plan items, and a repo literally named `api-3f2a`, or a 4-hex collision, could still give two repos one namespace. A repo with no commit has a first scan that maps nothing.
  - DR-87 says an unborn repo's live frame says it maps after its first commit, but live frames come only from stored events, and an event for an unborn repo names an empty commit, counts the repo as moved and made `focus` loop.
  - `repos_changed`, which DR-87 defined for a scan whose repo set moved, is also what a scan meets when another scan fixed a repo's namespace first.
- **Decision:**
  - **Reserved names and a growing suffix:** a repo named `ws` or `plan` takes the suffix, and the suffix grows 4 hex at a time while the suffixed name is still taken. This amends DR-74.
  - **A namespace is fixed at the first scan that maps the repo,** read and written inside that scan's transaction; a repo another scan fixed meanwhile stores nothing and the scan warns `scan_superseded` with `repos_changed`. This amends DR-74's "at its first scan".
  - **An unborn repo writes no snapshot and no scan event;** `repo_unborn` and the scan's summary carry the line that it maps after its first commit, and `frame.scan_unborn` remains only for empty-commit events an older engine wrote. This amends DR-87.
  - **The fold's budgets** are fold.20k 50 ms and fold.50k 500 ms, first numbers and so no raise (DR-89), held by the CI bench job.
- **Alternatives rejected:**
  - An empty-commit scan event for an unborn repo: it reopens the moved count and the `focus` loop.
  - A new event kind for the unborn line: a contract addition for a line the warning already carries.
  - Raising fold.20k: the fold was made to fit instead.
- **Consequences:**
  - Contracts: NodeId's namespace rule; `scan_superseded`'s `repos_changed` and `mapped`; `EventBody::Scan.commit` empty only in an older engine's event.
  - Tests: `namespaces.rs`; `id_stability.rs::fixture_ids_match_the_last_release`; `scan.rs::an_unborn_repo_writes_no_scan_event`; `xtask/tests/bench.rs`; the fold's rename and copy-on-write tests.
  - Known gap: the CI bench gate measures wall-clock time on a shared runner; if fold.20k lands near its budget there, the fold gets faster rather than its budget higher.
- **Sources (checked 2026-10-27):** `crates/planroom/src/app/workspace.rs`; `crates/store/src/commit/scan.rs`; `crates/kernel/src/fold/{state, prepass}.rs`; `crates/kernel/benches/fold.rs`; [Contracts](contracts.md); [Where the workspace lives](architecture.md#where-the-workspace-lives).

## DR-98: Review, one open Apply card, a viewer's row that keeps its viewer, and schema types by handle

- **Status:** accepted (decided at the merge of M1's review leaf, card store and inbox edits, 2026-10-27).
- **Starting point:** extended DR-5, DR-21, DR-67, DR-83, DR-86 and DR-89; amends DR-75.
- **Origin:** the merge of the review work, read against the docs by an independent check over three rounds.
- **Context:**
  - DR-75 says each unsigned row's ghost has "the import as its cause", but an import writes no event of its own: the only event a ghost can name is the Apply card its row joins, which architecture.md's same paragraph already makes the cause of the applied edit.
  - DR-67 says a confirmed inbox row applies as `person:artifact:<uid>`. A row that is already stale when imported is stored as a conflict on no card, and its accept went through the general rule, the reviewing person's, so one viewer row got a different actor depending on when it went stale.
  - An `edit undo` can reopen a resolved card. Reopening an Apply card while a newer one is open would leave two open cards of one kind and subject, and an undo of a last ghost's own edit would leave a card open with nothing waiting on it.
  - No pairing key exists before M5, so no row's `sig` can check.
- **Decision:**
  - **A ghost's cause is the open Apply card.** Each unsigned `edit` row's ghosts name the card the row deals or joins, which is written first, under a key of its own. This amends DR-75's "with the import as its cause".
  - **A viewer's row keeps its viewer.** A person's accept of an inbox row applies as `person:artifact:<uid>` with its client, whether the row waits on the Apply card (caused by the card) or was held as a conflict at import (caused by that conflict). Every other accept is the reviewing person's, caused by the item it copies (DR-5). The `review` itself is always the person's, so the authority is theirs either way.
  - **One open card per kind and subject.** A row that arrives while an Apply card is open joins it. An undo that would reopen a card beside an open one of the same kind and subject is refused with `invalid_argument`, whose `card` detail names the open card and whose hint reviews its first ghost. A card stays open exactly while a ghost waits on it: the review of its last waiting ghost, or an undo that leaves none waiting, writes the card's own review in the same write, and undoing that write reopens it. The store returns card events with the reviews and undos that name them; one kernel function, shared with the fold's prepass, decides which are open.
  - **`review` is a mixed leaf.** A person accepts or rejects any ghost or stored conflict; an agent may only reject its own ghosts (same session and role). Every other agent call is `person_required`, checked before the base, idempotency and existence checks, with the exact `planroom review` line as its hint and the card in its details when one is involved.
  - **A schema reply past its bare level hands types off by handle.** Merging the review leaf's types onto the questions' took `schema --command focus` to 1,550 tokens with every description gone, over `schema`'s 1,500. Past the bare level, `schema --command` keeps the request's types and the result's own, then takes the result's other types breadth first from it, keeping each that still fits and passing over each that doesn't, and gives a new `handles` field a `schema --def NAME` handle for each type a kept one references but the reply leaves out; `schema --def NAME`, which DR-21's amendment named and this change builds, fits one type's closure the same way. The budget stays at 1,500 and every shape a reply holds stays whole.
  - **A `sig` waits until pairing exists.** With no pairing key, a row that carries a `sig` is held on the Apply card like an unsigned row, never refused for its signature; DR-67's signed path starts when `person pair` does.
- **Alternatives rejected:**
  - An `import` event for ghosts to name: a new event kind for a cause the card already gives.
  - The reviewing person as the actor of an accepted stale row: it splits one viewer row's attribution by timing, against DR-67.
  - Allowing two open Apply cards after an undo: `focus` shows one card face up per kind, so the second would hide ghosts.
  - Raising `schema`'s budget: every command the catalogue gains lands in `CommandName`, and every pane section in `PaneModel`, so a raise would only move the day the bare level stops fitting.
  - Stopping at the first type that doesn't fit: a large type near the result would hand off every smaller one behind it.
- **Consequences:**
  - Contracts: `ReviewResult`; `invalid_argument`'s `ghosts`, `count` and `card` details; `person_required`'s `card`; the Unsigned rows rule; `SchemaReq.def`, `DefSchema` and `LeafSchema.handles`, all additive.
  - Tests: `review.rs`; `cards.rs`; `inbox_edits.rs`; `s11_remote.rs::a_confirmed_row_keeps_the_viewer_as_actor` and `a_stale_row_held_as_a_conflict_applies_as_its_viewer`; `kernel/tests/fold.rs`'s open-card checks; `version.rs`'s `every_leaf_holds_or_hands_off_each_type_it_references` and `every_type_fits_and_hands_off_what_it_leaves_out`; `budgets.rs` with `schema --def`.
  - Known gap: a type whose own body passes the budget can't be handed off. The largest, `Command`, has about 1,500 bytes of room, some seven more leaves; `every_type_fits_and_hands_off_what_it_leaves_out` fails before such a reply ships, and `Command` then splits by idea.
- **Sources (checked 2026-10-27):** `crates/planroom/src/app/{review, cards, import, edit, version}.rs`; `crates/store/src/cards.rs`; `crates/kernel/src/fold/{state, prepass}.rs`; [Contracts](contracts.md); [Remote mode through the Artifact](architecture.md#remote-mode-through-the-artifact-s11).

## DR-99: One hook for a model request's tail, every invisible character marked, and evidence paths left whole

- **Status:** accepted (decided at the merge of M1's redaction on every path and the sanitizing pass, 2026-10-28).
- **Starting point:** extended DR-49, DR-64, DR-75 and DR-91.
- **Origin:** the merge of the redaction work, read against the docs by an independent check over two rounds.
- **Context:**
  - Every engine output path already masked by field, but a model request's tail had no engine-side builder: the Claude transport assumed its caller had masked it. A slice of a type the generated list doesn't name, or a generic wrapper whose name leaves out its type argument, would have gone through unmasked.
  - The sanitizing pass marked bidi and zero-width characters, but line and paragraph separators, Unicode tag characters and other invisible format characters passed both ports raw.
  - An evidence path, `<repo>/<path>`, is repo-derived text that no mark covers, and it repeats the text of the file's own ID, which is never masked (security-8).
  - `hidden_characters` describes bidi controls and zero-width characters, and nothing emits it yet.
- **Decision:**
  - **One hook for a request's tail.** `app::models::request_tail` masks the slice by its own type's marks and the question as an engine line. It refuses with `internal` a slice whose type the generated list doesn't name, a generic type, and a listed type that holds an open value no mark covers (`redact_fields::OPEN`). It takes no raw flag and reads no environment, so `cli::raw_for` stays the one reader of `PLANROOM_RAW`.
  - **Every invisible character is marked, in both ports.** Every code point Unicode marks `Default_Ignorable_Code_Point`, every other format character (Cf), and the line and paragraph separators show as `⟨U+XXXX⟩` markers, the Tags block U+E0000 to U+E007F included. `display_parity` holds one table, written out in both ports' tests.
  - **Evidence paths stay whole.** `Evidence.path` and `PartialFile.path` are never masked, like the IDs whose text they repeat: masking the path alone would hide nothing and would break its match with the ID.
  - **A usage error's argv is masked** in text mode, as the error path already was in `--json`.
  - **`hidden_characters` keeps its meaning for now.** The change that first emits it widens it to every character the pass marks, and its record says why that isn't a `schema_version` bump while nothing has shipped the narrower meaning.
- **Alternatives rejected:**
  - Marking `Evidence.path`: the high-entropy rule would mask ordinary long paths unless paths took the ID's exemption, and the same text sits unmasked in the ID.
  - A shared golden table both sanitizing ports read: `packages/ui` may not read `tests/golden/`, and two literal copies of one table, checked against each other by their tests, need no new generator.
- **Consequences:**
  - Contracts: the redaction paragraph names the hook, its refusals and evidence paths.
  - Tests: `privacy.rs::a_planted_repo_secret_reaches_no_request_or_tool_output` (names and content planted, over CLI text and `--json`, HTTP, SSE and `watch`, `share`, `import`, the serve log and a model request, with fake keys in every process's environment) and `raw_never_reaches_share_or_model_requests`; `display_parity` in `crates/kernel/tests/display.rs` and `packages/ui/src/display.test.ts`; `redact.rs`'s `only_a_listed_contract_type_is_listed`.
  - Closes: redaction running only in share and import, line and paragraph separators and tag characters shown raw.
- **Sources (checked 2026-10-28):** `crates/kernel/src/{redact, display}.rs`; `crates/planroom/src/app/models.rs`; `crates/planroom/src/cli.rs`; `packages/ui/src/display.ts`; [Contracts](contracts.md); [Security and privacy](architecture.md#security-and-privacy).

## DR-100: The declarative runner's bounds: per file, one repo cut, and templates capped at load

- **Status:** accepted (decided at the merge of M1's declarative runner, 2026-10-28).
- **Starting point:** extended DR-9, DR-46 and DR-89; amends DR-47 and DR-86.
- **Origin:** the merge of the declarative runner, read against the docs by independent checks over ten rounds.
- **Context:**
  - A pack's regex and query extractors run over files a scanned repo chose, so every cost they meet has to be bounded the same way on every machine: nesting, parse and query work, the text predicates read, the facts and matches a run holds, and the templates it expands.
  - A budget shared across a repo's files made a file's facts depend on the files before it, which DR-47's per-blob fact cache can't replay; a per-file floor with no repo bound let many small files exhaust memory.
  - A file's facts copy its path and repo name into their evidence and IDs, so DR-47's and DR-86's cache key, which leaves the path out, can't be exact.
  - Expanding a template walks its parts for every match, which no work budget counts, and the pack loader took a template of any length.
- **Decision:**
  - **Per file, never shared.** Parse and query work, the text a predicate reads included, is at most 2 steps a byte plus 16,384, counted by progress calls, chunk reads and summed tree depth, never a clock. A file's facts and kept matches hold at most 128 bytes of heap a byte plus 16 KiB, charged as each fact's measured shape, attrs and props maps included. A file past either is partial with `work_limit`, gives that extractor no fact, and is reported in `unparsed_files`; one nested past 256 is partial with `nesting_limit`.
  - **One repo cut, like the walk's.** In sorted path order, each file that gives facts adds its charge; the first file whose charge would take an extractor's total past 256 MiB, and every file after it, are partial with `repo_limit`. The cut is computed every run from per-file charges; a cache stores each blob's facts and charge, never the cut.
  - **The cache key holds the path.** A per-file extractor's facts are cached by `(blob sha, repo-qualified path, extractor, packs_hash)`. This amends DR-47 and DR-86 before any cache exists.
  - **Templates are capped at load.** A template longer than 1,024 bytes or naming more than 64 variables refuses its pack with `pack_invalid` (`template_too_long`), so a template's cost per match is bounded and a pack can't stall a scan through it.
  - **`unparsed_files`** reports files a bound refused, apart from `partial_parse`, which keeps meaning files parsed with error nodes; one warning per bound, with the count and the first 10 paths in sorted order.
- **Alternatives rejected:**
  - A capture budget shared across a repo: a file's facts would depend on other blobs.
  - 16 bytes a byte for the capture budget: it refused the code pack's own URL hints on dense files, while the repo cut already bounds a repo's total.
  - A wall-clock bound: a result would depend on the machine.
  - Charging template expansion as work instead of capping templates: it bounds the same cost but lets a pack load that can never extract.
- **Consequences:**
  - Contracts: `PartialReason`'s `work_limit`, `nesting_limit` and `repo_limit`; the `unparsed_files` warning; `pack_invalid`'s `template_too_long`.
  - Known cost: a crafted repo at the walk's bounds, every file at its work budget, can take minutes to scan. It is bounded, linear in the walk and the same on every machine.
  - Tests: `crates/extract/tests/contract.rs`'s bound, heap, linear-time and per-file tests, among them `a_files_facts_never_depend_on_the_files_before_it`, `the_repo_bound_cuts_an_extractors_facts_in_sorted_path_order`, `a_runs_kept_matches_count_against_its_capture_budget` and `dense_everyday_files_keep_their_facts`; `packs.rs::a_template_past_its_bounds_is_refused_at_load` and `a_file_a_bound_refuses_is_reported_apart_from_parse_errors`.
- **Sources (checked 2026-10-28):** `crates/extract/src/declarative/{mod, work, template, nesting}.rs`; `crates/planroom/src/packs/validate.rs`; [The extractor contract](architecture.md#the-extractor-contract); [Contracts](contracts.md).
