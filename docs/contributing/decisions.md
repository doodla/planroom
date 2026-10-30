# Decisions

This page holds Planroom's numbered decision records. Each one gives the context, the decision, the alternatives it rejected, its consequences for the layout, contracts, tests, release, agents and users, and its sources. Each record names the item of KICKOFF `<design_starting_points>` it kept, replaced or extended. Changing the layout, a boundary or a convention needs a new record here before it lands (R16), and `cargo xtask check layout` enforces that.

## How to read a record

- **Status** is `accepted`, `accepted, verify in WS` (it rests on an assumption the walking skeleton checks), `needs confirmation` (it binds only once it is confirmed), or `superseded by DR-n`. A record a later review amended says so in its Status, naming the rounds [DR-79].
- **An amendment overrides.** An "Amended in review round N" bullet changes the record: where it differs from the bullets above it, the amendment holds, and a later round's holds over an earlier one's. An "Amended by DR-n" bullet does the same for a later record's decision, and the Status names that record. An "Extended by DR-n" bullet adds to the record without changing what it decided, and the Status names that record too. The design pages state the current rule, and `docs.rs::an_amended_record_says_so_in_its_status` checks the Status [DR-79].
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
| 101 | Workspace trust by record, a pinned person key, confined reads, and the credential paths a scan checks | DR-48, DR-52, DR-74, DR-78, DR-88, DR-89 | extended, amends DR-48 and DR-88 | the merge of M1's workspace trust and confined reads |
| 102 | Every directory's purpose checked, generated paths marked from one table, and golden reads caught by literal | DR-38, DR-59, DR-79, DR-81, DR-83, DR-89 | extended, amends DR-81 | the merge of M1's layout and docs gates |
| 103 | Five languages' formats and queries: queries compiled only to extract, every built-in bounded, and hint keys masked | DR-9, DR-12, DR-46, DR-47, DR-83, DR-91, DR-100 | extended, amends DR-83 and DR-91 | the merge of M1's formats and queries for five languages |
| 104 | Git history, an owner check before every git call, and another user's repo skipped with no command | DR-12, DR-16, DR-47, DR-74, DR-78, DR-79, DR-85, DR-86, DR-92 | extended, amends DR-16, DR-79, DR-86 and DR-92 | the merge of M1's git history adapter |
| 105 | Pane sections with row keys and a 7-row page, the mod's lines from the string catalog, and one budget for `focus` | DR-54, DR-57, DR-65, DR-77, DR-82 | extended, amends DR-54 and DR-57 | the merge of M1's pane sections |
| 106 | The plan pack's stages and lint, the deterministic `intent` leaves, and `ask stage` | DR-15, DR-23, DR-52, DR-66, DR-89, DR-101 | extended, amends DR-5, DR-23, DR-66, DR-92 and DR-94 | the merge of M1's plan pack |
| 107 | The link snapshot and `runtime_wiring`, with test code kept out of every runtime join | DR-12, DR-42, DR-86, DR-87, DR-100 | extended, amends DR-86 and DR-87 | the merge of M1's link snapshot |
| 108 | Imports join workspace packages, and a wrapper's call sites bind its method and path | DR-12, DR-89, DR-107 | extended, amends DR-12 and DR-89 | the merge of M1's imports linker |
| 109 | The pack set as workspace state, the changed-source card, and the Ruby pack | DR-62, DR-89, DR-101 | extended, amends DR-62, DR-89 and DR-101 | the merge of M1's pack set |
| 110 | Event backups written through on every person-authored commit, a temporary host's redacted ref, and the plain `doctor` | DR-53, DR-75, DR-89 | extended, amends DR-53, DR-75 and DR-89 | the merge of M1's event backups |
| 111 | Levels are abstraction levels that packs declare, under the kernel's intent | DR-42, DR-45, DR-46, DR-59, DR-85, DR-109 | extended, amends DR-42, DR-45, DR-46, DR-59, DR-85 and DR-109 | the owner's request of 2026-10-29 |
| 112 | Diagrams for people and agents: pictures the engine derives from its views, and diagrams agents draw that stay ghosts until a person accepts them | DR-3, DR-4, DR-5, DR-11, DR-22, DR-54, DR-111 | extended, amends DR-5 | the owner's request of 2026-10-29 |
| 113 | `intent next` drafts a plan stage from replay, as the model's ghosts, and `schema --commands` pages | DR-18, DR-23, DR-49, DR-65, DR-93, DR-106, DR-109, DR-111 | extended, amends DR-18, DR-23, DR-93 and DR-109 | the merge of M1's `intent next` from replay |
| 114 | Every read and write loads the workspace's recorded pack set | DR-48, DR-62, DR-73, DR-106, DR-109, DR-111, DR-113 | extended, amends DR-106, DR-109 and DR-111 | the merge of M1's recorded-set reads |
| 115 | Starting from nothing: a plan-only workspace, intended blocks that next actions target and that resolve when code lands, and a hand-off that starts a new repo | DR-23, DR-25, DR-48, DR-57, DR-64, DR-73, DR-74, DR-75, DR-80, DR-85, DR-86, DR-87, DR-101, DR-106, DR-110, DR-113 | extended, amends DR-25, DR-57, DR-74, DR-85, DR-87 and DR-106 | the owner's request of 2026-10-29 |
| 116 | Questions on a sticky line, assumptions you can see, a one-line read-back, and corrections that feed the next draft | DR-23, DR-50, DR-54, DR-73, DR-106, DR-111, DR-112, DR-113 | extended, amends DR-4, DR-23, DR-54, DR-64, DR-75, DR-105 and DR-106 | the owner's request of 2026-10-29 |
| 117 | Context packs: the map as a ranked, budgeted reading list for a plan item or a block, built into every hand-off | DR-4, DR-15, DR-27, DR-52, DR-64, DR-75, DR-112, DR-115, DR-116 | extended, amends DR-4 and DR-27 | the owner's request of 2026-10-29 |
| 118 | Ratify a plan and walk away: one signed approval of a bounded batch of hand-offs, an integration branch per repo, and slice POCs first | DR-23, DR-25, DR-26, DR-54, DR-55, DR-64, DR-66, DR-71, DR-73, DR-75, DR-77, DR-80, DR-106, DR-110, DR-115, DR-116, DR-117 | extended, amends DR-23, DR-25, DR-64, DR-65, DR-66, DR-71, DR-75, DR-77, DR-80 and DR-106 | the owner's request of 2026-10-29 |
| 119 | One room: boards in areas, every view a link, repos and boards attached from anywhere, and plans that run side by side | DR-1, DR-29, DR-58, DR-77, DR-101, DR-115, DR-116, DR-118 | extended, amends DR-25, DR-29, DR-51, DR-65, DR-66, DR-71, DR-77, DR-78 and DR-101 | the owner's request of 2026-10-29 |
| 120 | Ontologies as packs, chosen per board: domain concepts aligned to blocks at every level, and release management as the first domain | DR-9, DR-62, DR-109, DR-111, DR-114, DR-115, DR-116, DR-117, DR-119 | extended, amends DR-23, DR-36, DR-45, DR-46, DR-62, DR-74, DR-106 and DR-109 | the owner's request of 2026-10-29 |
| 121 | The band offers undo, the pane's key map, and `/room` and the tools from one synopsis per leaf | DR-8, DR-14, DR-21, DR-54, DR-65, DR-66, DR-77, DR-85, DR-89, DR-105, DR-111, DR-113 | extended, amends DR-8, DR-54, DR-66, DR-77, DR-85, DR-89 and DR-105 | the merge of M1's band, keys, `/room` and tools |

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

- **Status:** accepted; A-3 settled: the Bash tool has no TTY. Amended in review rounds 1, 2 and 3. Amended by DR-116 and DR-117.
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
- **Amended by DR-116:** a chat answer whose quote the mod finds in the person's prompts applies at once, as `from_chat`.
- **Amended by DR-117:** a proposal an engine miner writes is a ghost whoever runs it.
- **Sources (checked 2026-10-25):** [mod-api §3] (`CommandRunInput.origin`), [mod-api §8] (`tool.check` sees every tool); [plugin-packaging §3] (Bash environment lacks plugin variables); the design reviews (fatal flaws in the simplicity-first and extensibility-first designs).

## DR-5: Base required; field-level authored revisions

- **Status:** accepted. Amended in review rounds 1, 2 and 3. Amended by DR-106 and DR-112.
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
- **Amended by DR-106:** `intent new` and `intent version` take no base: a new project is refused when its ID is taken and a tag when its label is live; each stores its commit's own snapshot `seq` as the edit's base and, with no `--idem`, mints a fresh key.
- **Amended by DR-112:** a person's accept of a `diagram` ghost writes the `review` event alone, with no copying `edit`, since a diagram sets no field (M3).
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

- **Status:** accepted. Amended in review rounds 1, 2 and 3, and by DR-86 and DR-121.
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
- **Amended by DR-121:** the outbox atom is `OutboxEntry`: a write's `idem`, its argv, the names of the variables it set (never their values), its stdin, its cause and its session. It is kept from before the process starts until any reply, and a reload resends it once, before the watch loop, as the same actor and client, so the store writes one event; a leaf that takes no `--idem` stays out of it.
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

- **Status:** accepted, verify in M1 (S1's M1 subset) and M3 (the full S1 comparator). Amended in review rounds 1, 2 and 3. Amended by DR-89 and DR-108.
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
- **Amended by DR-108:** a wrapped `route_call` binds at most 64 call sites, the first by path and line, by position or keyword with literal defaults and a receiver skipped; an unbound method or path never joins; a route call's host may come from any compose-set read in its container, confirmed by a served endpoint.
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

- **Status:** accepted. Amended in review rounds 2 and 3, and by DR-104.
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
- **Amended by DR-104:** every git call pins the repo's own git directory and working tree, so git's own `safe.directory` check doesn't run; the engine's owner check, which runs before any git call there and covers the working tree, the git and common directories and every alternate object directory, is the one that counts.
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

- **Status:** accepted; A-9 settled. Amended in review rounds 1, 2 and 3. Amended by DR-113.
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
- **Amended by DR-113:** in M1, `claude.backend` `auto` and `gateway` both reach the gateway adapter, whose gate picks the recording, the recorded live call or the `replay_only` refusal; `session` waits for M2's broker and `off` turns calls off, each `model_unavailable`.
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

- **Status:** accepted. Amended in review round 1. Amended by DR-106, DR-113, DR-116, DR-118 and DR-120.
- **Starting point:** extended "Structural lint runs in code; semantic lint goes to Jev."
- **Origin:** the concurrency-first design (§6.1); a design review noted the simplicity-first design reduced R5's methods to a sentence.
- **Context:** R5 names the methods per stage (Jobs-to-be-Done and laddering, SMART and non-goals, MECE and the WBS 100% rule, GTD and HTN, MoSCoW and an MVP line, pre-mortem, lint), and S3's lint depends on them.
- **Decision:** the plan pack declares seven stages, each with its method, kinds and fields, structural lint rules (in kernel rule types) and semantic Jev kinds, as in [Architecture](architecture.md#planning-stages-ghosts-and-lint-r5). In chat, the planning partner proposes items itself as ghosts; with no chat, `intent next` asks Claude with the plan pack's one shared cached prefix and the stage's method in the tail. Version 1 is tagged when a project's first batch is accepted; later versions are a person's.
- **Alternatives rejected:** a generic "stage with a prompt" (the simplicity-first design).
- **Consequences:** contracts gain the `covers` rule type for the 100% rule; tests `s03_plan.rs`, `kernel/tests/lint.rs`.
- **Amended in review round 1:** stages declare the `kernel.item` types they write, with prop schemas, instead of plan-pack kinds [DR-45]. Pack `[[rules]]` lose `forbid_edge`: boundary rules are intended-layer edits only, so one violation has one report path (extensibility-11). every stage shares one plan-pack prefix, the planning rules and the union `PlanProposal` schema, with a 1-hour TTL. The stage's method goes in the uncached tail, so a plan's later stages read the cache. The budget test asserts a hit-rate floor of 0.6 after the first stage (cost-6, principles-6).
- **Amended by DR-106:** the rule types `covers` (the WBS 100% rule) and `disjoint` (MECE) count a subject's children of the rule's kinds that pass its prop tests; the four checks no rule type can express yet (a ladder under 2 rungs, no non-goal, no MVP line, a Must below the line) wait for M2.
- **Amended by DR-113:** the shared prefix is `packs/plan/prompts/plan.md` and the union `DraftReply` schema, `PlanProposal`'s strict form with props as name-value pairs, since structured output refuses an open object; it is at least 16,000 bytes and the same for every stage and project. A reply that can't be used is `model_reply_invalid` and writes nothing until M2's card.
- **Amended by DR-116:** pack data gains `[[items]]` (item types no stage owns, any pack may declare), a stage's `readback`, and the rule type `referenced`.
- **Amended by DR-118:** an eighth stage, `derisk`, a stage's `needed_when`, the `poc` item type, `risk`'s `stakes` and `rests_on`, and the rule type `max_descendants`.
- **Amended by DR-120:** the rule type `ordered`; `[[items]]` entries' `concept`, `summary`, `stages` and `checklist`; and the plan pack's `moscow_order`.
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

- **Status:** accepted. Amended in review rounds 1, 2 and 3. Amended by DR-115, DR-118 and DR-119.
- **Starting point:** none; replaces the research suggestion of `planroom:implementer` registered with `isolation: 'worktree'` [mod-api §5].
- **Origin:** the simplicity-first design (§6.6, D17); the merge preflight from the extensibility-first design (§6.6).
- **Context:** S4's change spec touches `worker` and `platform` [TRUTH scenarios.S4.implementer_patches]; Claude Code's worktree isolation covers only the session's repo. A merge that fails in the second repo must not leave the first merged.
- **Decision:** the engine makes one worktree per touched repo on `planroom/KEY`. Two implementers share one interface: `agent` (`planroom:implementer`, spawned by the mod with `cwd` at the hand-off directory) and `patch` (applies the spec's patches with `git apply --3way`; the fixture implementer and curation). `handoff merge` is person-only and preflights every repo before merging any.
- **Alternatives rejected:** agent-type isolation; merging repo by repo with no preflight.
- **Consequences:** contracts `ChangeSpec`, `ImplementerRun`; tests `s04_handoff.rs`, `handoff_preflight.rs::a_conflict_in_the_second_repo_leaves_both_untouched`.
- **Amended in review round 1:** `handoff merge` commits a `merging` step with each repo's planned target before touching git, records each repo's result, then commits `merged`. A retry resumes from the step and skips a repo whose `HEAD` already contains `planroom/KEY` (correctness-10). Contracts gain `HandoffState::Merging`; test `handoff_preflight.rs::a_crash_between_repos_resumes_on_retry`. the hand-off directory, its spec and its worktrees live in `DATA_ROOT/handoffs/<workspace_id>/KEY/`, outside `.planroom/`, and the mod's guard keeps the implementer's Edit and Write inside its worktrees (D16, security-3) [DR-52]. `handoff revert` undoes a merge in every repo behind the same kind of preflight and resume, the `merging` step records each repo's head before the merge, and cancel or drop moves the branch to `refs/planroom/archive/KEY` (D15, principles-3) [DR-55].
- **Amended in review round 2:** `handoff merge` and `handoff revert` run under a `handoff:KEY` lease and key each step, so a retry resumes and a second invocation joins (E1) [DR-8]. The preflight also checks that each repo's `HEAD` is on the spec's base branch, and before each repo's merge the command checks that `HEAD` is still the recorded `before`. On a mismatch, or a merge that fails after the preflight, it runs `git merge --abort`, returns `handoff_conflict` and deals one card, "Revert worker's merge, or retry?", with revert pre-selected, and the hand-off loop gains `merging --> reverting`. Contracts gain `SpecRepo.base_branch` (correctness-11). Test `handoff_preflight.rs::a_commit_after_preflight_aborts_cleanly`. The engine makes a separate local clone per touched repo instead of a linked worktree, and `handoff collect` fetches only each clone's `HEAD` into `planroom/KEY` in the real repo, so the implementer's Bash can't move or delete the person's refs. The title changes to match (E15, security-2) [DR-64]. The mod runs `collect` right after `report`, and `collect` deals the review card (principles-7) [DR-66].
- **Amended in review round 3:** a hand-off also reaches `merged` when a scan finds its change on the tracked branch, landed by a pull request, a squash or a rebase (F20, unasked-6) [DR-80].
- **Amended by DR-115:** a hand-off may create a repo at start; its merge into an unborn branch is `--ff-only`, with its commits as change units.
- **Amended by DR-118:** `merge`, `revert` and `cancel` gain a `--plan` form under `plan:<project>`.
- **Amended by DR-119:** in a repo two workspaces hold, Planroom's branch names gain the first 8 characters of the workspace ID.
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

- **Status:** accepted. Amended in review rounds 1 and 2. Amended by DR-117.
- **Starting point:** none; implements R12.
- **Origin:** the simplicity-first design's templates (§6.8); the concurrency-first design's Claude drafting and its two CI paths (§6.5, §10.2).
- **Context:** S12 must pass deterministically; drafts should read well when a model is reachable.
- **Decision:** miners (gotchas, procedures, stale lines) produce template drafts. When a model is reachable, a cheap-tier Claude call refines each draft with the target file in its cached prefix; in a live session the hidden curator agent can rework drafts that span files. Every draft must mention the cluster's key tokens before it's proposed. CI runs S12 twice: with replayed Claude and with models off.
- **Alternatives rejected:** Claude-only drafts; template-only drafts.
- **Consequences:** contracts `CurationDraft`; tests `s12_curation.rs` (both paths).
- **Amended in review round 1:** curation mines runs from `handoff report` and from a person's import only; agent-imported runs are kept untrusted. Cards name their source runs and show in full any draft with a URL, command substitution, a shell pipe or `allowed-tools` (security-9) [DR-52]. The curator spawns only after a person's card or `/room curate`, and its usage goes into the ledger (cost-10) [DR-51].
- **Amended in review round 2:** a refinement call caches the shared curation prompt and puts the target file in the uncached tail, with the 5-minute TTL, since no two drafts share a target (cost-7) [DR-65]. The curator submits drafts on stdin with `edit propose --file -`, and the guard allows that and `curate --json` with no prompt (principles-3) [DR-66].
- **Amended by DR-117:** curation may propose a workspace edit, a block's `context` hints, as well as a repo patch.
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

- **Status:** accepted. Amended in review rounds 1, 2 and 3. Amended by DR-119.
- **Starting point:** extended "A single-writer local server per workspace" (its exposure).
- **Origin:** the concurrency-first design (§12.2); a design review flagged the simplicity-first design's `?t=` query-string token.
- **Context:** a token in a query string lands in logs and history.
- **Decision:** `planroom open` (person only) makes a 128-bit one-time code, registers its sha256 with `serve` through the control token, and opens the URL with the code in the fragment in the browser itself; it never prints the code. The code works once, within 60 s, for that `serve` process. The page exchanges it for a random bearer token, kept in the tab's memory and sent as `Authorization: Bearer`, the stream included, which the canvas reads through `fetch`. Every API request needs that token, an exact `Origin` (a missing one is refused) and a matching `Host`; no CORS; strict CSP. `.planroom/` is 0700 and its files 0600.
- **Alternatives rejected:** a bearer token in the query string (the simplicity-first design); an `HttpOnly`, `SameSite=Strict` cookie, the first design, which a browser sends to every other server on `127.0.0.1`, since cookies ignore the port and every local port is one site.
- **Consequences:** tests `crates/planroom/tests/server_auth.rs` (no token, a missing or foreign `Origin`, a wrong `Host`, a reused or expired code); `store/tests/fs_check.rs::store_files_are_0600_in_a_0700_directory`. Users: a reload of the tab asks for a new code, one `e` press.
- **Amended in review round 1:** an in-memory bearer token sent as a header replaces the cookie, and every request needs an exact `Origin` (D17, security-7). The code has 128 bits and a 60 s life, reaches `serve` through the control token, and is never printed (security-3). `.planroom/` is 0700 and its files 0600.
- **Amended in review round 2:** `open` writes the URL into a 0600 redirect page under `DATA_ROOT/open/` and opens that file, so the code is in no process's argv, and a second try at a code revokes the token the first exchange issued (E15, security-4) [DR-64]. The tab keeps its bearer token in `sessionStorage`, scoped to its origin and port, so a reload within one `serve` stays logged in. `serve` prefers the port its last `serve.json` recorded, so the origin and the tab's storage survive a restart. A tab with no valid token shows the engine-authored "This tab needs a fresh link" state with its pending edits counted, which the empty-state audit visits (E10, principles-4) [DR-8]. The consequence that a reload asks for a new code no longer holds. Tests `server_auth.rs::the_code_is_never_in_an_argv`, `a_code_exchanged_twice_revokes_its_token`, `serve_prefers_its_last_port`. Where no browser can be seen (no display on Linux, an SSH session, a container host, or an opener that fails), `open` opens nothing and returns `opened: false` with `/room share` as the next action, and on WSL it opens through `wslview` (ux-4, unasked-7).
- **Amended in review round 3:** `serve` never moves off its workspace's port. `workspaces.json` gives each workspace its own, another program holding it is `port_in_use`, and `serve --port N` is the deliberate move, so a tab's pending edits never strand in another origin. `server_auth.rs::serve_port_moves_only_when_asked` replaces `serve_prefers_its_last_port` (F8, unasked-7) [DR-78].
- **Amended by DR-119:** one room server and port serves every workspace, with workspace routes under `/w/{id}`; a new tab takes the token from an open tab.
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

- **Status:** accepted. Amended in review rounds 1 and 2. Amended by DR-120.
- **Starting point:** kept and combined "a crate-graph test or `cargo-deny` bans in Rust, and `dependency-cruiser` or `eslint-plugin-boundaries` in TypeScript."
- **Origin:** the concurrency-first design (§9.3) for one source; the simplicity-first design (§9.3) for the kernel allow-list, the pack-literal scan, the three-way self-test and the decision-record gate.
- **Decision:** `boundaries.toml` is the single source. `cargo xtask check boundaries` reads it with `cargo metadata`; `cargo xtask gen` writes `.dependency-cruiser.cjs` from it. Also: the kernel's external-dependency allow-list, a scan that no string literal in `crates/kernel/src` equals a built-in pack kind or relation name, with or without its `<pack>.` prefix, and a self-test that plants a Rust, a TypeScript and a pack crossing in a temporary copy and expects each to fail.
- **Alternatives rejected:** `eslint-plugin-boundaries` (missed 1 of 5 planted imports, mid-migration [ts-stack §6.1]); two hand-kept configs.
- **Amended in review round 1:** the literal scan matches bare kind and relation names too, and the self-test plants a bare relation name (D10, extensibility-1) [DR-45]. The self-test installs JS dependencies offline in its copy, runs every check on the unmodified copy as a control, and plants a declared `workspace:*` crossing and a relative import, expecting exactly the rule's name and no unresolvable import. It also plants a layout change without a record (D14, repo-2) [DR-59].
- **Amended in review round 2:** `boundaries.toml` also holds the kernel's no-I/O list as item paths and each TypeScript package's `third_party` allow-list, so the generated checks allow only what it names (repo-8, repo-9) [DR-70].
- **Amended by DR-120:** the literal scan also refuses built-in concept IDs.
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

- **Status:** accepted. Amended in review rounds 1 and 3. Amended by DR-111.
- **Starting point:** kept "Levels. C4 (System, Container, Component, Code) with Intent on top. One parent per node, and edges roll up with counts" and "Stable, namespaced IDs such as `<repo>:<kind>:<key>`"; kept "`layer` is actual or intended."
- **Decision:** five fixed levels; one parent; roll-ups with counts, an evidence sample and child handles, with faithfulness as a tested invariant. IDs `<namespace>:<kind-short>:<key>`, where kind-short is the kind's declared `id_prefix`, owned by one pack [DR-46]; edge IDs from a hash of kind, ends and `via`. When two extractors emit the same node ID, evidence unions and scalar fields take the more specific tier's value.
- **Consequences:** tests `kernel/tests/rollup.rs` (property), `kernel/tests/fold.rs::fact_merge_by_tier`.
- **Amended in review round 1:** kind-short is defined as `KindDecl.id_prefix`; two packs may not declare one prefix (extensibility-8).
- **Amended in review round 3:** roll-up lifts each end to its nearest ancestor at or above the level. When both ends meet one ancestor, the edge is that node's `internal`; only an end whose ancestor holds the other end steps down one child on its own path. So the lift rule and the internal count agree (correctness-6). Test `kernel/tests/rollup.rs::same_ancestor_edges_are_internal`. A repo's namespace is its directory name, suffixed for the later of two same-named repos, and the renames git reports carry a vanished target (F5; unasked-1, unasked-2) [DR-74].
- **Amended by DR-111:** the levels are no longer a closed set of five. The kernel owns `intent`; the base pack declares the ladder `system`, `container`, `component`, `code`; another pack may put a level directly below one it can see, never below the base pack's finest, 9 levels in all. Every rule that named a level reads the ladder through `kernel::ladder::Ladder`, and every stored spelling stays.
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

- **Status:** accepted (decided in review round 1, D10). Amended in review rounds 2 and 3. Amended by DR-111 and DR-120.
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
- **Amended by DR-111:** the literal scan also refuses a built-in pack's level ID in the kernel's and the contract's sources and in the TypeScript surfaces, outside tests and the uses `boundaries.toml`'s `level_words` lists, and the surface rule reads "no built-in kind, lens or level ID".
- **Amended by DR-120:** the kernel's vocabulary gains `kernel.concept`, `kernel.realizes` and `kernel.is_a`.
- **Sources (checked 2026-10-26):** KICKOFF R11, R14, R16; review round 1 extensibility-1 and owner decision D10.

## DR-46: Pack data tables and typed emits

- **Status:** accepted (decided in review round 1, D11, D13 and D21). Amended in review rounds 2 and 3. Amended by DR-111 and DR-120.
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
- **Amended by DR-111:** the kinds that use the kernel's `container` prefix share one level and every kind at that level uses it, so no rule names a level; an image role names a kind that uses the prefix, and a tone is any kind's block color.
- **Amended by DR-120:** pack data gains `[[concepts]]`, `concepts/*.toml`, `LensDecl.concepts` and `LensDecl.explains`; a built-in `ontology` pack joins the v0.1 packs.
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

- **Status:** accepted (decided in review round 1, D7 and D19). Amended in review rounds 2 and 3, and by DR-101.
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
- **Amended by DR-101:** trust is a record in the data root's `workspaces.json` (`trusted`), matching the canonical root, the state directory's path, its inode and, where both sides have one, its birth time. The engine records the directory it creates, which also needs git to track nothing in it; a person's accept of the trust card records it whatever git tracks. A repo-root `.planroom/` gets no card and nothing in it loads.
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

- **Status:** accepted (decided in review round 1, D20). Amended in review rounds 2 and 3. Amended by DR-119.
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
- **Amended by DR-119:** `ledger.db` gains the `running` table and the `project` column; `room.max_running_handoffs` caps implementers across workspaces, and `room.max_daily_agent_usd` their priced usage.
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

- **Status:** accepted (decided in review round 1, D6). Amended in review rounds 2 and 3. Amended by DR-110.
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
- **Amended by DR-110:** the write-through appends every event above the backup file's own last `seq`, scan events included, not above `meta.backup_seq`, and then sets the mark in a short transaction of its own, so a kill between the fsync and the mark duplicates nothing. The lock is on `events.ndjson.lock` beside the file, for at most 5 s. The file's first line is an `ExportHeader` that also names the state directory it backs up; a header for another workspace, store version or state directory, or a last seq past the store's head, sets the file aside as `events-<unix seconds>.ndjson`. `doctor` warns when the mark or the file is behind.
- **Sources (checked 2026-10-26):** KICKOFF principle 1 ("Never lose or overwrite what someone did"); review round 1 principles-4, operability-4, unasked-1 and unasked-2, and owner decision D6.

## DR-54: Surfaces act through catalogue verbs, reversibly, in real keystrokes

- **Status:** accepted (decided in review round 1, D15). Amended in review rounds 2 and 3. Amended by DR-105, DR-116 and DR-121.
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
- **Amended by DR-105:** the surface tests named under `packages/mod/src/surfaces.test.tsx` live in `packages/mod/plugin-tests/surfaces.test.tsx`, where `claude plugin test` draws all four surfaces; the pure layout's tests stay in `packages/mod/src/layout.test.ts`.
- **Amended by DR-116:** the pane gains the question line below the who line.
- **Amended by DR-121:** the key tests named under `packages/mod/src/keys.test.tsx` live in `packages/mod/plugin-tests/keys.test.tsx`, where presses reach the host. A band press whose reply names an undo makes that undo the band's primary for 10 s, and a stray digit never presses a leaf that can spend.
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

- **Status:** accepted (decided in review round 1, D15). Amended in review rounds 2 and 3. Amended by DR-105 and DR-115.
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
- **Amended by DR-105:** each fallback pane is a `FallbackPane { headline, detail, action }`, not a `PaneModel`, for `resolving`, `fetching`, `build_offered`, `building`, `version_mismatch`, `person_key_mismatch`, `unsupported_platform`, `failed` and `no_focus`; there is no `store_busy` pane and no `/reload-plugins` action, and `building` has no action, since one source build runs at a time. `no_blank_pane` lives in `packages/mod/plugin-tests/surfaces.test.tsx` and `packages/mod/src/layout.test.ts`; the all-surfaces audit is the surface sweep's, at M1's end.
- **Amended by DR-115:** `intent new` also makes a workspace, a plan-only one, in a folder with no repos.
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

- **Status:** accepted (decided in review round 1, D14). Amended in review rounds 2 and 3. Amended by DR-89 and DR-111.
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
- **Amended by DR-111:** the breaking-change guard counts an old enum value as kept when the new definition still accepts it, in an `enum`, in an `anyOf` or `oneOf` branch's `enum`, or by a `pattern`; after a release, opening a closed set needs a bump.
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

- **Status:** accepted (decided in review round 2, E6). Amended in review round 3. Amended by DR-109 and DR-120.
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
- **Amended by DR-109:** a copy is named by the sha256 over its files (`tests/` left out), in 0700 directories at 0600, read with a bounded walk; the config's list counts once, at the first trusted scan; the record also names the engine that wrote it (`packs_engine`, with its commit for a `-dev` build); only an engine older than that one refuses writes, and a newer one, or the same version at another commit, records its own built-ins at its first write, so no process reloads.
- **Amended by DR-120:** `ontology add` and `ontology save` also change the pack set, person-only.
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

- **Status:** accepted (decided in review round 2, E15); verify A-15 in WS8. Amended in review round 3. Amended by DR-116 and DR-118.
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
- **Amended by DR-116:** a chat answer is an agent write that applies, never a person action; person-only commands stay person-only.
- **Amended by DR-118:** `handoff start` also runs with no press when a live ratification's signed envelope covers its spec.
- **Sources (checked 2026-10-26):** KICKOFF R5, R8 and principle 1; [mod-api §7] (`$.store` is a JSON file under the Claude Code configuration directory) and its module rules (`crypto.subtle`); review round 2 security-1, security-2, security-4 to security-8, security-10 and unasked-6, and owner decision E15.

## DR-65: Cost gates that can fail

- **Status:** accepted (decided in review round 2, E16). Amended in review round 3. Extended by DR-89. Amended by DR-118 and DR-119.
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
- **Amended by DR-118:** a ratification's own spend and time caps stop further starts; the daily cap keeps its meaning.
- **Amended by DR-119:** `handoff advance` starts nothing once today's priced agent usage across the room reaches `room.max_daily_agent_usd`; a person's `handoff start` press still runs, its card naming the sum.
- **Sources (checked 2026-10-26):** KICKOFF `<constraints>` Spend, S7 and R9; [claude-gateway §4] (a prefix below the minimum fails silently); [mod-api §6] (the session cache lasts 5 minutes); review round 2 cost-1 to cost-10 except cost-4, and owner decision E16.

## DR-66: An agent surface that reports errors, takes proposals inline and brings the diff back

- **Status:** accepted (decided in review round 2). Amended in review round 3. Amended by DR-106, DR-118, DR-119 and DR-121.
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
- **Amended by DR-106:** plan items are written through `intent propose`, whose `PlanProposal` fixes their fields; `edit set`, `text` and `propose` wait for a later milestone.
- **Amended by DR-118:** under a ratification, `collect` builds a verified candidate instead of dealing a review card.
- **Amended by DR-119:** `room_query` and `room_change` gain a `board` property that reaches attached boards only.
- **Amended by DR-121:** `band_never_names_a_spending_leaf` lives in `packages/mod/plugin-tests/keys.test.tsx`. Each tool's spec is generated into `catalogue.ts` as `TOOL_SPECS`, its description the `[tools]` intro from en.toml and one synopsis line per leaf. The tools refuse `--client` as they refuse `--workspace`, and their own refusal carries the leaf's synopsis, schema handle and hint.
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

- **Status:** accepted (decided in review round 2, E17). Amended in review round 3, and by DR-85, DR-92, DR-118 and DR-119.
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
- **Amended by DR-118:** a ratified hand-off's base branch is the plan's integration branch.
- **Amended by DR-119:** a workspace may hold repos a person attaches from anywhere.
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

- **Status:** accepted (decided in review round 3, F5 and F6). Amended by DR-97, DR-115 and DR-120.
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
- **Amended by DR-115:** a `.planroom/` at a repo's root is its state directory when the data root records it as trusted and git tracks nothing in it; `intended` is a reserved namespace.
- **Amended by DR-120:** `onto` is a reserved namespace.
- **Sources (checked 2026-10-26):** KICKOFF R1, R6, R15 and principle 1; `git-diff(1)` (`-M`), `git-rev-parse(1)` (`--absolute-git-dir`); review round 3 unasked-1, unasked-2, unasked-4 and owner decisions F5 and F6.
- **Amended by DR-97:** a repo named `ws` or `plan` takes the suffix too, the suffix grows 4 hex at a time while the suffixed name is still taken, and a root's namespace is fixed at the first scan that maps the repo, inside that scan's transaction.

## DR-75: Authority stays with a person's own action

- **Status:** accepted (decided in review round 3, F7, F11 and F15); verify A-18 in WS8. Amended by DR-88, DR-98, DR-110, DR-116 and DR-118.
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
- **Amended by DR-110:** the ref's copy redacts every line, its header too, through `redact::value_with_open`, which masks a stored record's marked fields and every string under its open values. It is kept in 1,000-line segments under its own lock and committed as one parentless, unsigned commit through git plumbing, with no working tree, index or branch touched.
- **Amended by DR-116:** a signed owner row may also apply `intent answer`.
- **Amended by DR-118:** a ratified hand-off's installs run unsandboxed only from manifests byte-identical to the envelope's; from a changed or new lockfile they run sandboxed, reaching only the registry hosts the envelope names, or, where the host can't limit them, wait for a person's press.
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

- **Status:** accepted (decided in review round 3, F9 and F10); verify A-22 in WS5. Amended by DR-118, DR-119 and DR-121.
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
- **Amended by DR-118:** under a live ratification, a POC fail's redraft runs under the ratify press, whose card named its cost, and `h` deals the run's card, whose Stop takes a second key.
- **Amended by DR-119:** `p` deals the board picker.
- **Amended by DR-121:** the key tests live in `packages/mod/plugin-tests/keys.test.tsx`. The list's page Button takes the next free row key, and when the pool would be spent the list holds one row fewer, so every shown row keeps its key.
- **Sources (checked 2026-10-26):** KICKOFF R13, R14, S9, S11 and principle 1; [mod-api §2] (36 one-key actions per pane), [mod-api §3] (`immediate`); [mod-spike]; [artifact §2]; review round 3 ux-1 to ux-10 and principles-2, and owner decisions F9 and F10.

## DR-78: Releases users install, detached work in its own session, a canvas port that never moves, and machine-wide files across versions

- **Status:** accepted (decided in review round 3, F8, F16 and F17); verify A-20, A-21 and A-23 in WS8. Amended by DR-119.
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
- **Amended by DR-119:** the room's port, recorded in `room.json`, replaces the port per workspace; `workspaces.json` entries gain `attached`.
- **Sources (checked 2026-10-26):** [plugin-packaging §5] (real git and GitHub installs unverified; version resolution); [mod-api §7] (process groups on reload unverified); `/bin/sh -> dash` in this container, and the round-3 verifier's `set -m` check; KICKOFF's packaging constraint and S14; review round 3 operability-1 to operability-8, unasked-7, unasked-8 and unasked-10, and owner decisions F8, F16 and F17.

## DR-79: Gates that fire only where they mean something

- **Status:** accepted (decided in review round 3, F18). Amended by DR-104.
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
- **Amended by DR-104:** a step a run's own scope leaves out, such as vitest in `cargo xtask test -p CRATE`, reports `not run`, which neither passes nor fails, apart from `not built` and `skipped`.
- **Sources (checked 2026-10-26):** KICKOFF R16, S14, S15 and the repo-tooling constraint; `crates/*/Cargo.toml` for the allow-lists' current entries; `crates/xtask/src/boundaries.rs` and `main.rs` (skips that return `Ok`); review round 3 repo-1 to repo-9 and operability-4, and owner decision F18.

## DR-80: Hand-off results land however the change reaches the tracked branch

- **Status:** accepted (decided in review round 3, F20). Amended by DR-118.
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
- **Amended by DR-118:** a plan's integration branch may land by pull request, `landed_by` `Plan`.
- **Sources (checked 2026-10-26):** KICKOFF R7, R8 and principle 1; `git patch-id --stable` (git documentation); review round 3 unasked-6 and owner decision F20.

## DR-81: What the walking skeleton's engine and plugin work settled

- **Status:** accepted (decided at the merges of the engine and plugin work, 2026-10-26). Amended by DR-85, DR-86 and DR-102.
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
- **Amended by DR-102:** `golden_readers` is the mod's tests and the e2e stand-in, `tests/e2e/src/standin.ts`, and a string literal that reaches `tests/golden` counts as a read, as an import does.
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

- **Status:** accepted (decided at the merges of the gates and remote-slice work, the CI repairs and the WS7 and WS8 checks, 2026-10-26). Amended by DR-87, DR-89, DR-91 and DR-103.
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
- **Amended by DR-103:** `Hint.key` carries the redaction mark: a `code.route_call` key is a URL's source text, which can hold a secret. Joins read the raw value; `Edge.via` and `Node.aliases` stay unmarked.
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

- **Status:** accepted (decided at the merge of the review's fixes, 2026-10-26). Amended by DR-86, DR-87, DR-111, DR-115 and DR-121.
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
- **Amended by DR-111:** blocks nesting like boxes, and a lid lifting on a level zoom, are how the table draws an abstraction level; every user-facing word speaks of abstraction levels, and no surface names a level.
- **Amended by DR-115:** `no_repos`'s next action is `schema --command "intent new"`, "Start a plan here".
- **Amended by DR-121:** a mod tool's own refusal of a call carries `field`, `synopsis` and `schema` as every CLI `invalid_argument` does, its `synopsis` the leaf's line in the tool's description. A leaf's synopsis is `CommandSpec::synopsis`, read off its request's clap arguments, and a leaf's own flags are global to its subcommands.
- **Sources (checked 2026-10-26):** `docs/contributing/research/skeleton-review/` (round 1's findings and verdicts); the four fix branches' reports; the merged tree's `cargo xtask check`, `test`, `test --installer` and `e2e`.

## DR-86: What the walking skeleton's review settled in round 2

- **Status:** accepted (decided at the merge of round 2's fixes, 2026-10-26). Amended by DR-87, DR-100, DR-104 and DR-107.
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
- **Amended by DR-104:** a repo another user owns, or one that borrows another user's object store, is skipped with `repo_skipped`, reason `other_owner`, and a `null` fix: its line says who can scan it, since no copy command works in every layout.
- **Amended by DR-107:** the join across repos is the link snapshot's, so a repo's snapshot key holds no workspace repo names again: adding or removing a repo re-keys only the link, and a late scan is never superseded because the repo set changed.
- **Sources (checked 2026-10-26):** `docs/contributing/research/skeleton-review/round2-*.json`; the four fix branches' reports; the merged tree's `cargo xtask check`, `test`, `test --installer` and `e2e`.

## DR-87: What the walking skeleton's review settled in round 3

- **Status:** accepted (decided at the merge of round 3's fixes, 2026-10-26). Amended by DR-88, DR-97, DR-107 and DR-115.
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
- **Amended by DR-107:** with no repo names in a repo's snapshot, a late scan no longer re-derives its inputs under the write lock; `scan_superseded`'s `repos_changed` means only that another scan fixed the repo's root under another namespace, or this namespace for another root.
- **Amended by DR-115:** `plan_only` wins over `repos_unborn` when a plan exists; `repos_unreadable` still wins over both.
- **Sources (checked 2026-10-26):** `docs/contributing/research/skeleton-review/round3-*.json`; the two fix branches' reports; the merged tree's `cargo xtask check`, `test`, `test --installer` and `e2e`.
- **Amended by DR-97:** an unborn repo writes no snapshot and no scan event; `repo_unborn` and the scan's summary say it maps after its first commit, and the live frame's unborn line remains only for empty-commit events an older engine wrote.

## DR-88: The guard's residual risks, decided

- **Status:** accepted (decided at round 3's close, 2026-10-26). Amended by DR-90 and DR-101.
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
- **Amended by DR-101:** the pin is `workspaces.json`'s `person_keys`, written by `person init` only while the workspace is trusted. A token counts under the pinned key, or, with no pin, in a trusted workspace, which covers a key registered before pins existed; otherwise `person_required` with reason `key_not_pinned`.
- **Sources (checked 2026-10-26):** `packages/mod/src/guard.ts`, `register.tsx` and `person.ts`; `crates/planroom/src/app/data.rs`; `crates/planroom/src/server/routes.rs` (what the control token unlocks); `plugin/scripts/planroom`; [accepted risk](research/skeleton-review/accepted-risk.md).

## DR-89: What M1 builds, compares and leaves to later milestones

- **Status:** accepted (decided before M1's first change, 2026-10-26). Amended by DR-91, DR-95, DR-108, DR-109, DR-110 and DR-121.
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
- **Amended by DR-108:** storefront's call to orders joins with its host taken from the `ORDERS_URL` read in `config.ts`, a compose-set read in storefront's container that a served endpoint confirms, not from the calls' own file.
- **Amended by DR-109:** `scan`, not a freshness check, deals the changed-source card, with its pack's ID, changed file count and the copy's and source's short hashes as vars, and an accept takes the source as it reads at the pick.
- **Amended by DR-110:** M1 also builds the plain `doctor`, which reports git, the store, its migrations, the pack set and the backup; `doctor --restore` and the restore card land with the restore, still in M1. The write-through's tests leave the feature list's due map for its tests.
- **Amended by DR-121:** the `keys.test.tsx` tests live in `packages/mod/plugin-tests/keys.test.tsx`. `tool_results`, `surface.tools`, `surface.section` and `room_text` enter `budgets.json` as new keys, not raises.
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

- **Status:** accepted (decided at M1's first change, 2026-10-26). Amended by DR-92 and DR-103.
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
- **Amended by DR-103:** `Hint.key` carries the redaction mark: a `code.route_call` key is a URL's source text, which can hold a secret. Joins read the raw value; `Edge.via` and `Node.aliases` stay unmarked.
- **Sources (checked 2026-10-26):** `crates/contract/src/{card.rs, event.rs, requests.rs, model.rs, view.rs, schema.rs}`; `crates/kernel/src/redact.rs`; [Contracts](contracts.md) (CLI JSON envelope's redaction paragraph, Event model, the requests sketch); [CLI](architecture.md#one-command-catalogue) and [Design language](design-language.md#edit-by-hand) for the edit ops.
- **Amended by DR-92:** `set`, `text` and `propose` arrive with the plan pack's intent leaves, which fix their fields, rather than with the `edit` leaf; `forbid` with no kind waits for a stored form of a rule over every kind.

## DR-92: The truth comparator's floor, edit's first ops, the dirty count's git call, and test-only seams

- **Status:** accepted (decided at the merges of M1's truth comparator, discovery, person authority and edit work, 2026-10-27). Amended by DR-104 and DR-106.
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
- **Amended by DR-104:** `PLANROOM_TEST_EUID` joins the test-only seams: in a debug build it stands in for the effective user ID, release builds compile it out, and the mod sets it empty for every engine run.
- **Amended by DR-106:** of edit's later ops, `set`, `text` and `propose` stay out of M1: plan items are written through `intent propose`.
- **Sources (checked 2026-10-27):** `crates/testkit/src/truth.rs`; `crates/planroom/src/app/{edit, open, dirty}.rs`; `crates/planroom/src/server/auth.rs`; `crates/extract/src/git.rs`; [Testing strategy](architecture.md#testing-strategy); [Contracts](contracts.md).

## DR-93: The model adapter's crate edges, its transport's crypto provider, and an environment variable's own hint

- **Status:** accepted (decided at the merge of M1's Claude adapter, 2026-10-27). Amended by DR-113.
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
- **Amended by DR-113:** an `invalid_argument` whose `details` carry `next`, the handle of the command to run instead, keeps that exact command as its hint too, such as `intent next` on a stage that drafts nothing, which hands off to the plan's `intent lint`. `PLANROOM_REPLAY_DIR` joins the variables whose relative path is refused with the export that sets it.
- **Sources (checked 2026-10-27):** `crates/models/{Cargo.toml, src/}`; `crates/planroom/src/cli.rs` (`with_usage`); `deny.toml`; [Contracts](contracts.md#error-codes); DR-39's library table.

## DR-94: Declared IDs at most 64 characters, and the listing's own left-out count

- **Status:** accepted (decided at the merge of M1's questions and `ask`, 2026-10-27). Amended by DR-106.
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
- **Amended by DR-106:** the 64-character bound covers stage, rule and item-type IDs too, and a plan item's key is `[a-z0-9][a-z0-9_-]{0,63}`, never `project`.
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

## DR-101: Workspace trust by record, a pinned person key, confined reads, and the credential paths a scan checks

- **Status:** accepted (decided at the merge of M1's workspace trust, 2026-10-28). Amended by DR-109 and DR-119.
- **Starting point:** extended DR-52, DR-74, DR-78 and DR-89; amends DR-48 and DR-88.
- **Origin:** the merge of M1's workspace trust, confined reads and guarded places, read against the docs by independent checks over three rounds.
- **Context:**
  - DR-48 trusted a state directory the engine created, with git tracking nothing in it. "The engine created it" had no record behind it: a `.planroom/` deleted and unpacked again, or carried in by a clone, looked the same as the engine's own.
  - DR-89 says accepting the trust card trusts the workspace, which a directory git tracks could never pass under DR-48's git check.
  - A crafted `.planroom/` can carry its own store and person key, and so sign the one pick that would trust it, unless something outside the workspace says which key is the person's.
  - The credential list the `scan` catalogue row gives was read two ways: "a dotfile" in the row, and "a final component that starts with `.`" in the build plan. The second makes every hidden directory a credential path: a temporary root such as `/tmp/.tmpXXXX`, a `.github`, a `~/.dotfiles` repo. A plain `scan` and `scan .` read the same directory, but only the second was checked.
- **Decision:**
  - **Trust is a record in the data root.** `workspaces.json` gains `trusted: [{root, state_dir, ino, created, by, at}]`, one item per `(root, state_dir)`. A record matches only the directory it was written for: its canonical root, its state-directory path, its inode and, where both sides have one, its birth time, which a freed inode reused at once never shares. `by: engine` is written when the engine creates the state directory, and also needs git to track nothing in it. `by: person` is written by an accept of the trust card and waives the git check, so accepting trusts a tracked directory too. No data root means untrusted. A remount that renumbers inodes on a filesystem with no birth time asks again.
  - **A repo-root `.planroom/`** is never a state directory (DR-74): no card is dealt for it and nothing in it loads.
  - **The trust card** is dealt by `scan` for an untrusted state directory when no card for the workspace is open and none stands rejected, with `reject` preselected. A pick records or removes the trust, an `edit undo` of an accept removes it, and an undo of that undo records nothing, so an agent's undo never trusts a workspace. `workspace_untrusted` now says "the state directory is untrusted"; a pick whose record fails to write stands and carries the new warning `trust_not_recorded`.
  - **The person key is pinned.** `workspaces.json` gains `person_keys: [{root, fingerprint, at}]`, the SHA-256 of the registered key, written by `person init` and its rekey only while the workspace is trusted. A token counts only under the pinned key, or, with no pin, in a trusted workspace; otherwise `person_required` with reason `key_not_pinned`. Older engines keep both lists in `extra`, so `min_reader_version` stays.
  - **Confined reads.** The global `--workspace DIR` runs a leaf in DIR; on a read it needs a store there, and `focus` and `map` give their empty state only without it. A relative DIR is placed from the process's own directory. `ask`'s path-shaped arguments are checked as given and as resolved, and anything outside the workspace's repos is `path_refused` (`outside_workspace`), with no hint until `scan FILE` makes a snapshot in M4.
  - **Credential paths.** A credential path is a dotfile (a file whose name starts with `.`), anything under `~/.ssh`, `~/.aws`, `~/.gnupg` or `~/.password-store`, `~/.config/*/credentials`, or a final name matching `*.pem` or `.env*`. A hidden directory is no credential path by its name alone: the stores kept as directories are named in the list, and a directory holds what the rest of the list names. `scan` checks the directory it reads: the one it's named, the `--workspace` given, and for a plain `scan` the one it runs in. A non-person gets `path_refused` (`credential`).
  - **The guard** reads `X+=word` as the name it makes; refuses an archive whose names may be absolute or rewritten (`pax -r`, `cpio -i` without `--no-absolute-filenames`, `tar -P` or `--transform`, bsdtar `-s`, `unzip -:`) as an unpack at `/`; and treats `gio trash`, `remove` and `move` as `rm` and `mv`. These extend the guard rules at architecture.md's guard bullets; they add no new kind of rule.
  - **The fixture records its own trust.** `testkit::fixture` writes no `.planroom/` and has no data root of its own, and `cargo xtask fixture` records the fixture workspace's trust with a small writer, so neither testkit nor xtask depends on the engine crate. This departs from the M1 plan's fixture line, which had the testkit call the engine.
- **Alternatives rejected:**
  - Trust from the `workspaces.json` entry alone: `serve` writes an entry for any root, so an entry can't mean the engine made the directory.
  - A device number in the record: it changes across remounts and btrfs subvolumes.
  - Every hidden directory a credential path: it refuses temporary roots, `.github` folders and dotfiles repos for no secret, and an empty map's own `next` handle (`scan <root>`) would be refused in a `~/.dotfiles` workspace.
  - Leaving a plain `scan` unchecked: an agent running inside `~/.ssh/keys` would map what `scan .` there may not.
- **Consequences:**
  - Contracts: `workspaces.json`'s `trusted` and `person_keys`; `person_required`'s reason `key_not_pinned`; `path_refused`'s `credential` and `outside_workspace` with no hint; the warnings `workspace_untrusted` (reworded) and `trust_not_recorded`; the global `--workspace`; `scan`'s actor is `mixed`, as the catalogue already said.
  - The mod's tools refuse `--workspace` in every call, since `room_query` and some `room_change` leaves are auto-allowed.
  - Known gap: a store planted over a workspace whose key is already pinned leaves a person no way to re-register under their own key after accepting its trust card, since `person init` refuses the mismatch and `--rekey` needs a token under the planted key. Deleting the planted state directory clears it. M2 lets `person init` re-register when the store's key doesn't match the root's pin, with the pinned key's token as the authority.
  - Tests: `trust.rs`; `confine.rs::a_credential_path_needs_a_person`, `a_read_outside_the_workspace_is_path_refused` and `workspace_flag_on_a_read_needs_an_existing_store`; `tools.test.ts::a_tool_call_never_names_another_workspace`; `guard.test.ts::an append, an archive's own names and gio hide no guarded place`.
- **Amended by DR-109:** `planroom-testkit`'s `fixture_with_packs` builds the fixture workspace as `cargo xtask fixture` does, its Ruby pack config recorded as trusted, for every test that scans the fixture as CI paths do; the plain `fixture` writes no state directory, so the engine's first scan creates and records it.
- **Amended by DR-119:** the tools still refuse `--workspace` and reach another workspace only through an attached board.
- **Sources (checked 2026-10-28):** `crates/planroom/src/app/{trust, confine, data, cards, review, person, actor}.rs`; `packages/mod/src/{guard, tools}.ts`; `crates/xtask/src/fixture.rs`; [Where the workspace lives](architecture.md#where-the-workspace-lives); [Contracts](contracts.md).

## DR-102: Every directory's purpose checked, generated paths marked from one table, and golden reads caught by literal

- **Status:** accepted (decided at the merge of M1's layout and docs gates, 2026-10-28).
- **Starting point:** extended DR-38, DR-59, DR-79, DR-83 and DR-89; amends DR-81.
- **Origin:** the merge of M1's layout and docs gates, read against the docs by independent checks over two rounds.
- **Context:**
  - AGENTS.md says every directory has a stated purpose and every generated path is marked, but no check held either: a new directory needed no line anywhere, and the generated-headers step read a glob list in `boundaries.toml` that had drifted from repo-layout.md's Generated directories table.
  - DR-81 let only the mod's tests read `tests/golden/*.json`, enforced on imports. The e2e stand-in reads one golden by path, and a read by a string literal passed the import rule unseen.
- **Decision:**
  - **Every directory has a purpose, checked.** `every_directory_has_a_purpose` fails on a tracked directory that neither the Directory map nor its area's `## Layout` list names, on an area with subdirectories and no Layout list, on an entry naming a path that doesn't exist without the `(Mn)` tag of the milestone that builds it, and on a tag left on a path that exists. An entry names a directory when it is that directory, a path below it, or a data tree written with a trailing `**`; `**` stands only on `docs/contributing/research/**`, `packs/code/tests/**` and `.claude/skills/**`, and a single `*` names no directory. Placeholders `<name>` and `sNN` match one segment.
  - **Generated paths are marked from one table.** repo-layout.md's Generated directories table is the one list. `cargo xtask gen` writes a `GENERATED.md` stub into each generated directory and `.gitattributes` from it, and `generated_headers` checks that every path it names carries its stub, header or markers, and that a generated directory holds only what its generator writes. `boundaries.toml`'s `generated_read_only` names that table rather than a list of its own. `plugin/hooks/hooks.json` joins the mod bundle's group, so `gen` writes it and its drift check covers it; the stubs under `plugin/` pass `claude plugin validate --strict`.
  - **Golden reads by literal (amends DR-81).** `golden_readers` is the mod's tests and the e2e stand-in, `tests/e2e/src/standin.ts`. A string or template literal that reaches `tests/golden` from any other TypeScript or JavaScript file fails as `golden-only-in-mod-tests`, as an import does; the boundaries self-test plants one. A path split across concatenated literals isn't caught, as repo-layout.md's Boundaries table says.
  - **The area contracts table** starts every How to test it cell with its `cargo xtask` command, names files and never a test function, and carries `(Mn)` on an area that doesn't exist yet.
  - **Cargo.lock** is checked as committed and current, and the `cargo xtask` alias stays without `--locked`, since `--locked` would leave no `cargo xtask` command that can update the lock after a manifest edit.
- **Alternatives rejected:**
  - Keeping a glob list in `boundaries.toml` beside the table: two lists of generated paths drift.
  - A file-row exemption for the plugin's generated files: validate passes with the stubs.
  - `**` anywhere a layout list wants it: one line would silence the check for a whole crate.
- **Consequences:**
  - Layout: every area's `AGENTS.md` gains a `## Layout` list; `GENERATED.md` stubs in each generated directory; `.gitattributes` generated; `boundaries.toml`'s `generated_read_only` and `golden_readers`; the area contracts table's How to test it cells. `layout.lock` is regenerated with this record.
  - A change that adds a directory adds its Layout or map line, and a change that creates a tagged path drops its tag. The link change adds `crates/extract/src/link/`; five languages adds `packs/code/queries/` if it creates it.
  - Known gaps: a How to test it cell can still name a test function joined to its file by a separator other than `::`, and a golden path written root-relative with `./` or `//` segments passes the literal scan.
  - Tests: `crates/xtask/tests/gates.rs`'s `every_directory_has_a_purpose`, `a_changed_cargo_lock_fails_under_ci`, `a_golden_read_by_any_literal_path_fails`, `a_subtree_wildcard_names_only_a_data_tree` and `a_test_cell_naming_a_test_function_with_its_file_fails`; `crates/xtask/tests/docs.rs::generated_headers`; the boundaries self-test's golden-literal plant.
- **Sources (checked 2026-10-28):** `crates/xtask/src/{areas, docs, boundaries, generate, modgen, layout}.rs`; `boundaries.toml`; [Repo layout](repo-layout.md#area-contracts) and its Generated directories table; AGENTS.md's Boundaries and Working in this repository sections.

## DR-103: Five languages' formats and queries: queries compiled only to extract, every built-in bounded, and hint keys masked

- **Status:** accepted (decided at the merge of M1's formats and queries for five languages, 2026-10-28).
- **Starting point:** extended DR-9, DR-12, DR-46, DR-47 and DR-100; amends DR-83 and DR-91.
- **Origin:** the merge of M1's formats and queries for five languages, read against the docs by independent checks over two rounds.
- **Context:**
  - The code pack now reads package manifests and OpenAPI specs with two built-ins, and TypeScript, JavaScript, Python, Go and Rust through declarative queries over the runner's five grammars.
  - Compiling the code pack's twelve tree-sitter queries at every pack load cost every command about 0.3 s in a release build, read commands included, though only extraction runs them.
  - A `code.route_call` hint's key is the URL argument's source text, which can hold a query string's secret, while DR-91 kept `Hint.key` unmarked as a join key.
  - The new built-ins held a few hundred times their input in memory with no bound but the walk's, which DR-100 had closed for the runner alone.
- **Decision:**
  - **Queries compile only to extract.** A pack's regex extractors compile at load; its query extractors compile once per process, and only in a process that extracts (`scan`, `doctor --pack`, `pack add`). Read commands never compile them. A test compiles every built-in query, so a shipped pack can't carry a broken one; an out-of-tree pack's broken query is `pack_invalid` wherever a process extracts. contracts.md states it.
  - **Every extractor is bounded as the runner is.** The manifest and OpenAPI built-ins charge each fact's measured heap against DR-100's per-file bound and per-repo cut, with the same partial reasons and `unparsed_files` reporting, through the runner's own charging code.
  - **`Hint.key` is marked (amends DR-91 and DR-83).** It carries `x-redact`: joins read the raw value, and outputs mask what looks like a secret in it. `Edge.via` and `Node.aliases` stay unmarked.
  - **An OpenAPI operation is its own hint:** its ID includes its method, so operations of one path item never collapse, whatever line they sit on. A route's method stays as the source spells it, and joins compare it without case.
  - **A package name no ID can hold** (one with a newline or backslash) gives no package node; the file is reported as `partial_parse`. A leading UTF-8 byte order mark is read past.
- **Alternatives rejected:**
  - Compiling every query at load, which made every pane refresh pay for extraction.
  - Raising DR-100's 256 MiB repo cut for the call-hint query, which over the Go and Rust standard libraries cuts about two fifths of the files: splitting calls into their own extractor gives imports and definitions their own budget, when M3's linkers need full call coverage.
  - A new `PartialReason` for an unnameable package name: a contract addition for a case `partial_parse` already describes.
- **Consequences:**
  - Contracts: `Hint.key`'s mark in the generated redaction list; the OpenAPI hint ID; contracts.md's sentence on when a pack's extractors compile.
  - Known gaps: in a repo whose compose service name differs from the repo's name, the manifest's root container and compose's service are two containers until M3's linkers join them; the compose built-in has no heap charge of its own, finds lines per evidence item in quadratic time, and puts a service name holding a newline or backslash into a node ID, and the runner's node emits take such a captured key too, which the cross-repo links change closes; two Rust `use` forms (`self as x` in a group, a comment inside a plain path) give wrong import keys; a dense `Cargo.toml` can be refused whole by the per-file budget.
  - Tests: `crates/extract/tests/formats.rs`; `contract.rs::imports_and_calls_for_each_language`; `packs.rs::every_built_in_query_compiles`, `a_broken_query_loads_and_is_pack_invalid_wherever_a_process_extracts` and `read_commands_and_watch_frames_compile_no_query`; `privacy.rs::a_route_call_key_holding_a_secret_is_masked_where_hints_are_shown`.
- **Sources (checked 2026-10-28):** `crates/extract/src/{manifest, openapi, declarative/charge}.rs`; `packs/code/{languages, queries}/`; `crates/planroom/src/packs/mod.rs`; [The extractor contract](architecture.md#the-extractor-contract); [Contracts](contracts.md).

## DR-104: Git history, an owner check before every git call, and another user's repo skipped with no command

- **Status:** accepted (decided at the merge of M1's git history adapter, 2026-10-28).
- **Starting point:** extended DR-12, DR-47, DR-74, DR-78 and DR-85; amends DR-16, DR-79, DR-86 and DR-92.
- **Origin:** the merge of M1's git history adapter, read against the docs by independent checks over nine rounds.
- **Context:**
  - The `git` extractor reads a snapshot's history for co-change, ownership and churn, and a repo chooses what that history and its git directory hold: replace refs, a graft file, a shallow cut, alternates into another user's object store, special files where git opens a regular one, submodules whose git directories live elsewhere.
  - DR-78 has the engine refuse a repo whose git directory another user owns. Git itself still ran in places the check didn't reach: listing submodules ran git inside each one, and a repo borrowing another user's object store let that user stall a scan.
  - DR-86 had every `repo_skipped` name a fix. For another user's repo, each printed copy command failed in some layout (symlinks, nested workspaces, special files carried into the copy, clashing names).
- **Decision:**
  - **History is the commit's own.** Every git call pins the repo's own git directory and working tree (`GIT_DIR`, `GIT_WORK_TREE`), reads no graft file and no replace refs, and never looks above a workspace repo. A shallow clone's boundary is reported in `partial_scan` as DR-47 reports its 5,000-commit cut, one `history before <sha>` per boundary the read reached.
  - **The owner check runs before any git call there (amends DR-16).** It compares, with the effective user ID, the owner of the working tree, the `.git` file and the git directory it names, the common directory, and every object directory git would read through `objects/info/alternates`, as deep as git reads them. A repo that fails it is skipped with `repo_skipped`, reason `other_owner`. With the git directory named, git's own `safe.directory` check doesn't run; the engine's is the one that counts, on every git version from 2.34.1.
  - **Submodules are listed without running git in them.** A root repo's submodules are the gitlinks of its own index (`git ls-files -s -z`) whose paths its `.gitmodules`, read as a file and never through git, names. Each is then a repo with its own owner check before any git call in it. The root's own check runs first, so a root another user owns, or one borrowing another user's objects, is skipped like any repo, never `git_failed`.
  - **Special files fail before git opens them.** A file git opens in a git or common directory (`HEAD`, `config`, `config.worktree`, `packed-refs`, `index`, `shallow`, `objects/info/alternates`, `info/exclude`, `info/attributes`) that exists and is neither a regular file nor `/dev/null`, which git reads as empty, gives `git_failed` with `ls -l` of its path, so nothing blocks on a FIFO.
  - **Another user's repo names no command (amends DR-86).** An `other_owner` skip's `fix` is `null`: its one line says who owns what and that the owner can scan it, or the person can scan a copy they own. `path_refused`'s `other_owner` reason stays in the registry, reserved for a later read that names a path in another user's repo; no M1 path produces it.
  - **A step a run's own scope leaves out reads `not run` (amends DR-79).** `cargo xtask test -p CRATE` reports vitest and the other suites it doesn't run as `not run`, which neither passes nor fails, apart from `not built` and `skipped`.
  - **`PLANROOM_TEST_EUID` joins the test-only seams (amends DR-92).** In a debug build only, it stands in for the effective user ID, so a test plants another user's repo without root; release builds compile it out, and the mod sets it empty for every engine run.
  - **The oldest-git CI job** runs the adapter's tests on git 2.34.1 in an `ubuntu:22.04` container, which has neither git nor Rust. `ci_runs_only_xtask`'s setup steps gain the lines that install them: `apt-get update`, `apt-get install` of git, a build toolchain, `ca-certificates`, `curl`, `pkg-config` and `python3.11` (with `python3` linked to it), the rustup installer over HTTPS, and the cargo path line.
  - The plan's test `git_hardening.rs::the_other_owner_fix_runs_without_root` is renamed `the_other_owner_skip_names_no_command`, since that fix no longer exists, and `feature_list.json`'s git-checks step now says the scan skips such a repo with `repo_skipped`, reason `other_owner`, fix `null`.
- **Alternatives rejected:**
  - Printing a copy command for another user's repo: every form tried failed in some layout, and a copy carries that user's special files.
  - A wall-clock deadline on git calls: DR-47's bounds are deterministic.
  - `git submodule status`: it runs git inside each submodule before that submodule's owner check.
- **Consequences:**
  - Contracts: `repo_skipped`'s `other_owner` with a `null` fix; `partial_scan`'s shallow-clone `uncovered`; `git_failed` for a special file; the reserved `path_refused` reason.
  - Known gaps: a group-shared repo (`core.sharedRepository`) lets a member of its group plant a FIFO that stalls a scan; a copy of another user's repo can carry their special files; loose refs, loose objects, working-tree `.gitignore` and `.gitattributes`, and files config names aren't checked for special files, since only someone who can write there can plant one; a `git_failed` fix's `ls -l` path under `.git/modules/<name>/` or `.git/worktrees/<name>/` can come out partly masked when the name is short; `focus` and `map` show `no_repos` for a root whose submodule listing fails; the empty-state line says another user owns a working tree when only its git directory is theirs.
  - Tests: `crates/extract/tests/git.rs`; `crates/planroom/tests/git_hardening.rs`; the oldest-git job.
- **Sources (checked 2026-10-28):** `crates/extract/src/git.rs`; `crates/planroom/src/app/{workspace, scan}.rs`; `crates/xtask/src/{report, workflows}.rs`; `.github/workflows/ci.yml`; [Adapters](architecture.md#adapters); [Contracts](contracts.md).

## DR-105: Pane sections with row keys and a 7-row page, the mod's lines from the string catalog, and one budget for `focus`

- **Status:** accepted (decided at the merge of M1's pane sections, 2026-10-28). Amended by DR-116 and DR-121.
- **Starting point:** extended DR-54, DR-57, DR-65, DR-77 and DR-82; amends DR-54 and DR-57.
- **Origin:** the merge of M1's pane sections and fallback panes, read against the docs by an independent check.
- **Context:**
  - The pane drew its sections from a `PaneModel` but took its hotkeys from literal strings, wrote its own lines around the engine's, and fit `focus` to a byte size that a workspace's masked names and warnings could push past its token budget.
  - DR-57 listed the fallback panes as `PaneModel`s with `store_busy` and a band action that names `/reload-plugins`; the mod has never had either, and it draws a smaller shape.
  - The pane's own test file moved: the drawn surfaces are tested in `claude plugin test`, and only the pure layout stays in vitest.
- **Decision:**
  - **Sections and keys.** `packages/mod/src/layout.ts` lays out the pane and `register.tsx` draws it; every hotkey comes from `@planroom/ui` (`ROW_KEYS` through `keyRows`, `CARD_KEYS`, `BAND_KEYS`). The card's choices take `1` to `4`; the diff rows, the drift line's top item and the who line's top row take row keys in that order, starting at `5`. A row key only selects its row. The block list pages at 7 rows, with one plain page Button that wraps from the last page to the first; the key map gives paging a key.
  - **The fallback panes (amends DR-57).** Each is a `FallbackPane { headline, detail, action }`, not a `PaneModel`, for `resolving`, `fetching`, `build_offered`, `building`, `version_mismatch`, `person_key_mismatch`, `unsupported_platform`, `failed` and `no_focus`. There is no `store_busy` pane (a busy store answers `focus` with its error, which `no_focus` shows with a retry) and no `/reload-plugins` action, which the mod can't run. `building` and `unsupported_platform` have no action: one source build runs at a time.
  - **The mod's lines.** Every line the mod draws around the engine's own comes from `en.toml`'s `[mod]` table, which `cargo xtask gen` writes into `catalogue.ts` as `MOD_STRINGS`; `@planroom/ui`'s `fill()` fills each template before the mod sanitizes it.
  - **One budget for `focus`.** `focus` keeps to its catalogue budget, 1,000 tokens on the CLI, redacted or raw, by dropping mini-map blocks from the end with their edges, never by a byte fit. Over HTTP the pane is whole, its mini-map at most 60 blocks. `focus --detail summary` drops the mini-map, which the tools ask for; both details keep the top 3 presence rows.
  - **Overlays.** `packages/mod/plugin-tests/overlays.ts` holds the test-only overlays on the golden `focus.json`, one per milestone that makes its state real (drift's top item, diffs), as `tests/e2e/overlays/` does for the canvas.
  - **Where new code goes (AGENTS.md).** A pane section or key goes in `packages/mod/src/layout.ts`, which `register.tsx` draws, and the key map snapshot.
  - **Tests (amends DR-54 and DR-57).** The surface tests that DR-54 and DR-57 name under `packages/mod/src/surfaces.test.tsx` live in `packages/mod/plugin-tests/surfaces.test.tsx`; `packages/mod/src/layout.test.ts::no_blank_pane` lays out every fallback pane within the dock.
- **Alternatives rejected:**
  - A `store_busy` fallback pane: `focus` already reports a busy store, and a second pane for it is a second string source for one state.
  - A Build press while a build runs: a second build would race the first over the same source tree.
  - One choice per row in a pane under 48 columns: docks are at least 49 columns and inline panes are full width.
- **Consequences:**
  - Contracts: `FocusReq.detail` (`summary` or `full`, additive); `PaneModel.minimap` omitted at `summary`; `FALLBACK_PANES` and `MOD_STRINGS` in `catalogue.ts`.
  - Known gaps: below about 48 columns two long choice labels side by side can overflow the pane; the guard's verdict reasons, the tool descriptions, `room-args.ts`'s usage line, `live.ts`'s line, three of `engine.ts`'s lines and `/room`'s "+N more" are still written in the mod; no row is selected until a row key selects one, though architecture.md says an action key acts on the first row until then (the key map's action keys land with it); `room_query` asks for the full `focus` until the band and tools pass `--detail summary`; the resolver's source build takes no lock, so a reload mid-build can offer Build again; the all-surfaces sweep of every fallback pane, the not-yet-a-workspace state and every error frame is the surface sweep's, at M1's end.
  - Tests: `packages/ui/src/rows.test.ts`; `packages/mod/plugin-tests/surfaces.test.tsx` (`no_decision_shows_next_on_every_surface`, `drift_and_who_top_rows_carry_row_keys`, `compact_keeps_the_top_diff_line`, `pane_shows_the_engines_summary_line`, `a_check_again_press_refreshes_the_pane_with_no_way_back`, `a_running_source_build_offers_no_second_build`); `crates/planroom/tests/budgets.rs::focus_keeps_to_one_budget`; `crates/planroom/tests/focus.rs::focus_detail_summary_drops_the_minimap_and_keeps_the_rest`; the goldens `focus-no-repos.json` and `map-container.json`.
- **Amended by DR-116:** the list takes the row keys left in the pool, at most 7, so it pages at 6 when every keyed section shows.
- **Amended by DR-121:** the list's page Button takes the next free row key, and the list holds one row fewer when the pool would be spent; the first keyed row is selected from the start; the tools pass `focus` `--detail summary`; the tool descriptions, `room-args.ts`'s usage line and `/room`'s "+N more" come from `catalogue.ts` and en.toml. The key tests are in `packages/mod/plugin-tests/keys.test.tsx`.
- **Sources (checked 2026-10-28):** `packages/mod/src/{layout, register, engine, fallback}.ts`; `packages/ui/src/{rows, text}.ts`; `crates/planroom/src/app/focus.rs`; `crates/xtask/src/generate.rs`; `crates/planroom/text/en.toml`; [The mod](architecture.md#the-mod); [Contracts](contracts.md).

## DR-106: The plan pack's stages and lint, the deterministic `intent` leaves, and `ask stage`

- **Status:** accepted (decided at the merge of M1's plan pack, 2026-10-28). Amended by DR-114, DR-115, DR-116, DR-118 and DR-120.
- **Starting point:** extended DR-15, DR-23, DR-52, DR-66, DR-89 and DR-101; amends DR-5, DR-23, DR-66, DR-92 and DR-94.
- **Origin:** the merge of M1's plan pack, lint, the deterministic `intent` leaves and `ask stage`, read against the docs by independent checks over two rounds.
- **Context:**
  - S3 plans a change in stages, each with a method and lint, and attaches its next actions to the map. M1 builds the deterministic part: the plan pack's data, structural lint, writing items, versions and listings, and the stage checklist the chat agent reads. `intent next`, where the engine asks Claude, comes from recordings next.
  - DR-5 required a base on every write to authored state, but a new project and a version tag have nothing to compare a base against. DR-23 named rule types that can't express every stage's check. DR-66 and DR-92 left `edit set`, `text` and `propose` open for writing plan items.
- **Decision:**
  - **The plan pack.** `packs/plan/` declares seven stages, each with its method, the item types it writes with their fields, its checklist (`prompt`, one step per non-blank line) and its lint rule IDs, and nine rules of the types `requires_prop`, `requires_edge`, `min_children`, `covers` and `disjoint`. A stage, method or rule lives only in pack data; the kernel reads it.
  - **Rule types (amends DR-23).** `covers` is the WBS 100% rule: a subject whose counted children don't cover its own `prop` values. `disjoint` is MECE: a subject two of whose counted children share a value of `prop`. A child counts when it is of the rule's `kinds` and passes its prop tests. The four checks no rule type can express yet (a ladder under 2 rungs, no non-goal, no MVP line, a Must below the line) wait for M2.
  - **No base for a new project or a tag (amends DR-5).** `intent new` and `intent version` take no base: one puts a new project, refused when its ID is taken, and the other tags the head, refused when its label is live. Each stores its commit's own snapshot `seq` as the edit's base and, with no `--idem`, mints a fresh key, so a retry meets that refusal instead of replaying.
  - **Items are written through `intent propose` (amends DR-66 and DR-92).** `PlanProposal` fixes an item's key, type, name, parent, props and touches; re-proposing a key sets only the fields that differ and adds new targets, and an absent parent keeps an existing item where it is. `edit set`, `text` and `propose` wait for a later milestone. A proposal that changes nothing is `invalid_argument` with `field` `items`, since a commit needs an event and an undo handle. A person's proposal into a project that waits as a ghost names `field` `project`; one onto a ghost parent names `items[i].parent`, and either carries the ghost and its exact `review` in `ghosts`. A parent change that closes a cycle names the proposal's item the cycle runs through.
  - **IDs (amends DR-94).** A project is `plan:<slug>:project` and an item `plan:<slug>:<key>`, each key `[a-z0-9][a-z0-9_-]{0,63}` and never `project`. A slug is the text as lowercase words, at most 48 characters, used only when redaction leaves the text as it is, since an ID is never masked; otherwise it is `p-` and 8 hex characters of the text's sha256. DR-94's 64-character bound covers stage, rule and item-type IDs too.
  - **The proposal file (beside DR-52 and DR-101).** `intent propose --file F` reads a plain file of at most 64 KiB from anywhere under the workspace root, links followed, but never its state directory or a git directory; `--file -` reads stdin, so a chat proposal writes no file.
  - **Listings.** `intent list` with several projects and none named lists every project without items. A page cuts an entry too large for it alone (its touches, pending edits and review handles) and counts those entries in `entries_cut`, even on the last page. Version 1 is tagged automatically only once M2 accepts a batch; in M1 a person tags each version.
  - **`ask stage S [PROJECT]`** returns the stage's checklist, the item types it writes with their fields, and for the named or only project its items of those types, the stage's lint findings and the blocks the plan touches.
- **Alternatives rejected:**
  - A base on `intent new` and `intent version`: there is no prior state of a new project or a tag to compare it against, and a retry would replay a second project or tag.
  - `edit set` and `text` for plan items now: two ways to write an item's fields, one of which skips the proposal's checks.
  - An ok reply for a proposal that changes nothing: it would carry no event and no undo handle.
  - Listing a project's items when several projects exist and none is named: which project's is a guess.
- **Consequences:**
  - Contracts: `IntentNewReq`, `IntentVersionReq`, `IntentListReq`, `PlanProposal`, `ProposedItem`, `IntentListResult`, `PlanItem`, `LintResult`, `LintFinding`; `RuleType::Covers` and `Disjoint`; `StageDecl.prompt`; the empty states `no_intent`, `no_plan_items` and `no_findings`; `truncated`'s `entries_cut` for `intent list`; `path_refused`'s proposal-file reach.
  - Known gaps: `ask stage` and `intent lint` read the built-in packs only, as other reads do until the pack set loads a workspace's; the four M2 stage checks; an automatic version 1.
  - Tests: `crates/planroom/tests/s03_plan.rs`; `crates/planroom/tests/intent.rs`; `crates/planroom/tests/packs.rs`'s plan-pack validation tests; `budgets.rs::an_intent_list_page_keeps_to_its_budget_however_long_its_touch_ids`; the kernel's plan and lint unit tests.
- **Amended by DR-114:** `ask stage`, `intent lint` and every other `intent` leaf read the workspace's recorded pack set, so that known gap is closed.
- **Amended by DR-115:** a proposal may put intended blocks, and a next action may target one.
- **Amended by DR-116:** the plan pack gains the `question` item type, its checklist line and its rules; `answer` is settable through `intent propose`.
- **Amended by DR-118:** an eighth stage, `derisk`, a stage's `needed_when`, the `poc` item type, `risk`'s `stakes` and `rests_on`, and the rule type `max_descendants`.
- **Amended by DR-120:** the rule type `ordered`; `[[items]]` entries' `concept`, `summary`, `stages` and `checklist`; and the plan pack's `moscow_order`.
- **Sources (checked 2026-10-28):** `packs/plan/`; `crates/kernel/src/{plan, lint}.rs`; `crates/planroom/src/app/{intent, ask, confine, edit}.rs`; `crates/contract/src/plan.rs`; [Planning](architecture.md#planning-stages-ghosts-and-lint-r5); [Contracts](contracts.md).

## DR-107: The link snapshot and `runtime_wiring`, with test code kept out of every runtime join

- **Status:** accepted (decided at the merge of M1's link snapshot, 2026-10-28).
- **Starting point:** extended DR-12, DR-42 and DR-100; amends DR-86 and DR-87.
- **Origin:** the merge of M1's link snapshot, read against the docs and the truth by independent checks over seven rounds.
- **Context:**
  - DR-12 put the join across repos in a linker over hints, so each repo's snapshot can hold only its own facts. Until now compose joined services to repos itself, which put the workspace's repo names in every repo's snapshot key, and no relation between containers was mapped.
  - Linking reads every repo's hints at once, so a linker's work, its evidence and what counts as runtime code decide whether the map invents relations or stalls a scan.
- **Decision:**
  - **The link snapshot.** The `ws` link row is keyed by the active set and the packs, written inside the scan's transaction only when that commit creates it, and holds the facts `runtime_wiring` derives from the active set's hints. A repo's snapshot holds no other repo's name, so adding or removing a repo re-keys only the link (amends DR-86); a scan that read an older repo set is never superseded for that alone, and the input re-check under the write lock is gone (amends DR-87). Compose's service containers are now Linked, its hints scoped to the compose file, and constants are `code.const` hints.
  - **`runtime_wiring`** joins `route_call`s to endpoints, `url`s and `env_read`s through compose's `env_set`s to hosts, `queue_op`s to queues and `db_open`s to databases: `http`, `queue_publish`, `queue_consume` and `db`. It never guesses: a join hint whose repo holds two or more containers, or whose host reaches two or more targets, joins nothing, and the scan that builds the link warns once with `unresolved_ref`. When exactly one compose service joins a repo, that service's container is the repo's for joins, and the manifest's Universal-tier root container yields to it (DR-42). An edge cites its anchor hint, its callers two hops up, and at most three wiring items per join hint, the first by path and line.
  - **The link's work grows with the hints, never their product**: endpoints indexed by path and method, each host resolved once a link and shared, a variable's first three reads. The one disclosed exception is a union of distinct target sets, built once per combination.
  - **Test code never anchors or cites a runtime join.** A `[[languages]]` entry's `tests` globs name its test files; a `code.test_scope` hint (attributes `end`, and for a module a file declares at its top level `module`, `path` and `inline`) marks a span of test code inside a file, which the code pack's Rust query emits for every item `#[cfg(test)]`, `#[test]` or an attribute ending in `test` marks; and the files Rust reads a `#[cfg(test)]` module from are test code in whole. A module declared inline has no file of its own: only the files of the modules it declares out of line are its. The declarative runner's templates gain `$end`, a match's last line. Inline test code in other languages, such as Vitest's in-source block, is read as runtime code until a pack names it.
  - **Compose is bounded as the runner is (DR-100):** each fact charged as it is made, its file's lines indexed once.
  - **The map's top level pages** on the fixture now that lifted edges carry evidence: 9 of 10 blocks on the CLI's first page, as the paging rule says. No budget changed.
- **Alternatives rejected:**
  - Keeping the repo set in every snapshot key: every repo re-scanned when one is added.
  - Joining a hint with several candidates by a heuristic: an invented relation is worse than a missing one.
  - Leaving test code in the joins: a test that pushes to a queue mapped a publisher the system doesn't have.
  - Column spans for test scopes: they change the Evidence and Hint contracts; whole lines need formatting rustfmt never writes.
- **Consequences:**
  - Contracts: `LanguageDecl.tests`; the `code.test_scope` and `code.const` hint kinds; the template `$end`; `unresolved_ref`'s `details {count, items}`; `scan_superseded`'s narrowed `repos_changed`; the linker options table.
  - Known gaps: storefront's call to orders, whose URL is read in another file, waits for wrapper binding; the healthcheck edges wait for the Ruby pack in the fixture's set; the deploy, scrape and telemetry linkers come with their packs; a queue key that is an identifier no same-file constant resolves keeps its raw text until imported constants resolve; `cfg(all(test, ...))` marks no scope; an item overlapping a parse error gives no scope; runtime code on a test item's first or last line is dropped; a removed repo's service stays joined until removed repos leave the active set; the link runs inside the scan's write transaction, which lengthens how long a scan holds the lock.
  - Tests: `crates/extract/tests/link.rs`; `crates/planroom/tests/{link_snapshot, duties}.rs`; `crates/extract/tests/formats.rs`'s Rust test-scope cases; the recall floor's new checks.
- **Sources (checked 2026-10-28):** `crates/extract/src/link/`; `crates/extract/src/compose.rs`; `crates/store/src/commit/scan.rs`; `packs/code/`; [Hints and linkers](architecture.md#hints-and-linkers); [Contracts](contracts.md).

## DR-108: Imports join workspace packages, and a wrapper's call sites bind its method and path

- **Status:** accepted (decided at the merge of M1's imports linker, 2026-10-29).
- **Starting point:** extends DR-12 and DR-107; amends DR-12's round-3 amendment and DR-89's "through a wrapper call".
- **Origin:** the merge of M1's imports linker, read against the docs and the truth by independent checks over two rounds.
- **Context:**
  - Imports were Syntactic edges, so a TypeScript import of another repo's package mapped nothing the truth asks for.
  - storefront calls orders through `OrdersClient`, whose base URL is the `ORDERS_URL` read in `config.ts`, another file than the calls. DR-107's link left that join as a known gap, so storefront → orders stayed unmapped.
  - Queue keys that name an imported constant kept their raw text.
- **Decision:**
  - **Imports are hints a linker joins.** Each language's `[[languages]]` entry emits `code.import` hints. The `imports` linker matches each import spec to a workspace package through one index of package nodes keyed by name, at most one lookup per segment, and emits `depends_on_package` with the import as evidence. An import of a name two or more repos hold joins nothing and is counted in `unresolved_ref`. In M1 only imports of another workspace package join. A `depends_on_package` edge that carries import evidence reads as Linked, through DR-42's merge.
  - **One selection.** The scan and `link_snapshot.rs`'s relink read the same `Selection` of facts: what each loaded linker's roles name, plus the file-to-container map (`file_containers`) that the Ruby pack's healthcheck joins need.
  - **Wrapper binding** (amends DR-12 round 3). A `route_call` whose method or path is a parameter of its enclosing definition binds at each call site that passes a literal or a template, by position or keyword, with literal defaults, skipping a receiver (a first `self` or `cls`, Rust's `self`, TypeScript's `this`). A wrapped hint reads at most 64 call sites, the first by path and line. An unbound method or path never joins, replacing DR-107's method-less join. A queue key that names a parameter and resolves to no constant never joins.
  - **A route call's host** may come from any compose-set read in its container, confirmed by a served endpoint. This amends DR-89's "storefront to orders through a wrapper call": the join takes its host from the `ORDERS_URL` read in `config.ts`, not from the calls' own file.
  - **Imported constants** resolve queue keys, at most 32 dotted segments per import, each file's module keys worked out once. Test files still never anchor a join (DR-107).
  - **Linear work.** Caller evidence keeps one site per (path, line), so a line with many calls of one function is one caller; an import of a name many repos hold reads the index's list in place. Each bound has a wall-clock test, and a gate fails any test that asserts a wall-clock budget unless nextest runs it alone.
- **Alternatives rejected:**
  - Joining an import by its spec alone: two repos with one package name would invent an edge.
  - Joining an unbound wrapper call by its path alone: DR-107's method-less join mapped calls the system doesn't make.
  - Binding every call site of a wrapper: the work grows with the product of call sites and hints.
- **Consequences:**
  - Contracts: the `code.import` hint kind; the `imports` linker and its options; the linker block's `Reads`, edges and `file_containers` (contracts.md). No new error, warning or contract type.
  - The fixture: storefront → orders `http` reaches the map, so the golden map's first page now leaves otel-collector and prometheus out, and the tones overlay adds them whole.
  - Known gaps: binding reads at most 64 call sites per wrapped hint; a dotted import of more than 32 segments resolves no constant; `fmt.Sprintf`, `format!` and bound queue keys make no join in M1; relative and same-package imports wait for M3's module and file joins; the lifted storefront → orders edge's 3-item sample comes from its `GET /orders/{id}` child, so `orders.ts:26` and the POST route's lines don't show through the map (DR-92's owed read).
  - Tests: `crates/extract/tests/link.rs` (wrapper binding, imports, imported constants, and the linear-time cases, which run alone under nextest); `crates/xtask/tests/gates.rs::every_test_that_asserts_a_wall_clock_budget_runs_alone`; the units in `link/{mod, bind, runtime}.rs`; `privacy.rs`'s masked container on scan's own page; five S1 recall checks.
- **Sources (checked 2026-10-29):** `crates/extract/src/link/`; `packs/code/`; [Hints and linkers](architecture.md#hints-and-linkers); [Contracts](contracts.md).

## DR-109: The pack set as workspace state, the changed-source card, and the Ruby pack

- **Status:** accepted (decided at the merge of M1's pack set, 2026-10-29). Amended by DR-111, DR-113, DR-114 and DR-120.
- **Starting point:** extends DR-62; amends DR-62, DR-89 and DR-101.
- **Origin:** the merge of M1's pack set, read against the docs and the truth by independent checks over two rounds.
- **Context:**
  - DR-62 made the pack set workspace state, but nothing read the config's `packs`, copied a pack or recorded a set, so an out-of-tree pack could change under a running workspace and two processes could extract with different sets.
  - The truth lists the `healthcheck` container and its two `http` edges, which only the Ruby pack's extractor can find.
- **Decision:**
  - **Copies.** `pack add` (person-only, undone by Compensate) validates a pack and copies it into `.planroom/packs/<sha256>/`, named by the sha256 over its files with `tests/` left out, in 0700 directories at 0600, written aside and renamed into place. Reading a pack stops with `pack_invalid` past 1,000 entries, 16 directories deep, 1 MiB in one file or 4 MiB in all. Every process loads an out-of-tree pack only from its copy; a copy that is missing or no longer hashes to its name is copied again from a source that still does, and is otherwise set aside as `.<sha256>.<uuid>.stale` and the scan stops with `pack_invalid` naming the command that fixes it. `pack remove` drops a pack from the set.
  - **The record.** The store's `meta` holds the resolved list (each pack's ID, version, source and copy's sha256), `packs_hash`, `packs_engine` (the engine that recorded it, with its commit for a `-dev` build) and `packs_config_read`. The config's `packs` counts once, at the workspace's first trusted scan, which copies and records each pack it names; after that only `pack add` and `pack remove` change the set. A changed `packs_hash` makes every repo's snapshot stale.
  - **The write guard.** Every authored write checks the record under the write lock. Only an engine older than the one that recorded the set refuses, with `packs_mismatch`; a newer engine, or the same version at another commit, records its own built-ins at its first write, in that write's transaction, so no process reloads. `put_layout` and `touch_presence` are exempt.
  - **The changed-source card.** Each scan of a trusted workspace reads every recorded source before its commit. One that no longer hashes to its copy deals one person-only card for its pack, with Keep pre-selected, unless one is open or a Keep of the source as it reads now stands. Update takes the source as it reads at the pick, and its undo records the copy it replaced.
  - **The Ruby pack** (`examples/packs/ruby`) requires only `code` and carries no code: a `[[languages]]` entry and a `regex` extractor that emits the existing hint kinds, so `healthcheck` maps as a tooling container under `platform` with its two `http` edges. The fixture workspace loads it: `cargo xtask fixture` writes its config and records the trust, and `planroom-testkit`'s `fixture_with_packs` builds the same state for every test that scans the fixture as CI paths do.
- **Alternatives rejected:**
  - Loading a pack from its source each time: an edit outside the workspace would change the set under a running process.
  - Refusing writes from any engine whose identity differs from the recorder's: every rebuild of a `-dev` engine would lock the store until a reload.
  - Reading the config's `packs` on every scan: a hand edit would change the set with no person's action.
- **Consequences:**
  - Contracts: `PackAddReq`, `PackRemoveReq` and `PackSetResult`; the `packs_mismatch` row and its two hints; the `pack_not_updated` warning; `meta.packs`, `packs_hash`, `packs_engine` and `packs_config_read`, with `packs_hash`'s meaning widened and `schema_version` kept at 1 before any release (as DR-87); an extractor ID may hold `-`.
  - Decided at the merge: `find` matches words fuzzily against names and IDs, so a component's repo-qualified truth path is searched as two words (`orders cmd/orders`); scoring a whole path would change `find`'s documented matching. `schema --def Command` has about 190 bytes of margin after its trim: the leaf that passes it moves a type's referenced types behind one `schema --commands` handle, not a raised budget.
  - Known gaps: reads (`focus`, `ask`) use the built-ins; copies are never pruned; two `-dev` builds at different commits re-record and re-extract in turn; in an untrusted workspace with recorded packs, `packs_hash` names a set its snapshots weren't extracted with until a person trusts it; an open changed-source card outlives `pack remove` until it is picked; `doctor` doesn't read `PLANROOM_PACKS` yet; an error reply isn't held to its leaf's budget.
  - Tests: `crates/store/tests/pack_record.rs`; `crates/planroom/tests/packs.rs`'s pack-set cases; `trust.rs::{an_agent_cannot_add_a_pack, planroom_packs_loads_nothing_outside_doctor, an_out_of_tree_pack_loads_without_a_card}`; `s10_extensibility.rs::{ruby_absent_without_pack, healthcheck_edges_through_the_ruby_pack}`; `s01_first_look.rs::{search_finds_every_truth_entity, repo_languages_match_truth}`.
- **Amended by DR-111:** the known read gap covers every read, `map` and `find` among them: until reads load the workspace's recorded pack set, `map --level` refuses an out-of-tree pack's level with `invalid_argument` naming the built-in levels, and that level's blocks count at the finest.
- **Amended by DR-113:** the decided-at-the-merge hand-off is `Command`'s alone: `schema --def Command` keeps every variant and its `$ref` and hands off the leaf request types it leaves out behind one `schema --commands` handle, while every other type keeps its per-type `schema --def` handles. `schema --commands` itself pages, with an opaque cursor bound to the catalogue.
- **Amended by DR-114:** the known read gap is closed. Every read and write of a workspace loads its recorded set from the copies, read-only; a copy that is missing or changed, or a set that doesn't load, gives the built-ins and `packs_not_loaded`. A process keeps one set by its hash for scans and reads alike.
- **Amended by DR-120:** a pack's recorded source may be the room's library, `DATA_ROOT/ontologies/<id>/`.
- **Sources (checked 2026-10-29):** `crates/planroom/src/{packs, app/pack.rs}`; `crates/store/src/meta.rs`; `examples/packs/ruby/`; [What a pack is](architecture.md#what-a-pack-is); [Contracts](contracts.md).

## DR-110: Event backups written through on every person-authored commit, a temporary host's redacted ref, and the plain `doctor`

- **Status:** accepted (decided at the merge of M1's event backups, 2026-10-29).
- **Starting point:** extends DR-53; amends DR-53, DR-75 and DR-89.
- **Origin:** the merge of M1's event backups, read against the docs by an independent check and a re-check.
- **Context:**
  - DR-53 says nothing a person authored lives only in `store.db`, but no command wrote a backup, so a lost state directory lost every edit.
  - A cloud session, a Codespace or a dev container loses the state directory and the data root together when its container goes.
  - Nothing told a person or an agent whether the workspace was healthy: git, the store, its migrations, the pack set and the backup.
- **Decision:**
  - **The write-through.** After a commit with a person-authored event, and after the live loop hears of one, the store appends every event above the backup file's own last `seq` to `DATA_ROOT/backups/<workspace_id>/events.ndjson`, in export's format: an `ExportHeader` line (`planroom.event/v1`, the workspace ID, the store version, the state directory the file was written for, masked, and any later fields kept), then one event per line, scan events included. It locks `events.ndjson.lock` for at most 5 s, cuts a torn last line, fsyncs, then advances `meta.backup_seq` in a short transaction of its own, since the mark is bookkeeping, not an authored write. The file's own last seq decides what to append, so a kill between the fsync and the mark duplicates nothing and a later process fills a gap. A header naming another workspace, store version or state directory, or a last seq past the store's head, sets the file aside as `events-<unix seconds>.ndjson` and starts a new one. A failure leaves the commit a success with `backup_behind`.
  - **A temporary host's ref.** When `CLAUDE_CODE_REMOTE`, `CODESPACES` or `REMOTE_CONTAINERS` is set, a step after each append commits a redacted copy of the file to `refs/planroom/backup/<workspace_id>` in one workspace repo (the root when it is a repo, else the first recorded repo by namespace, fixed in `meta.backup_ref` at the first write): every line, the header included, goes through `redact::value_with_open`, which masks a stored record's marked fields and every string under its open values. The copy is kept in 1,000-line segments under its own lock and committed as one parentless, unsigned commit through git plumbing; no working tree, index or branch changes, and nothing pushes the ref. A failure gives `backup_ref_failed`, and the next person edit tries again.
  - **The plain `doctor`** reports the git on PATH against its floor, the store's head and event count, its version and the migration copies kept, the recorded pack set against this engine's, and the backup's mark against the last person-authored event, with `backup_behind` when it is behind. It writes nothing and keeps to 1,500 tokens whatever the paths. It counts the backup file by the write-through's own test (`planroom_store::held_seq`), so a file the write-through would set aside, or a workspace ID that names no backup directory, holds nothing; the latter shows the file as `DATA_ROOT/backups` and is `backup_behind`, whose line says the next edit tries to catch it up. `--pack DIR` keeps its conformance kit; the result is the contract's untagged `DoctorResult = Pack | General`.
- **Alternatives rejected:**
  - A debounced or periodic backup: a one-shot CLI process would exit before it ran.
  - Deciding what to append from `meta.backup_seq`: a kill between the fsync and the mark would duplicate lines.
  - Keeping the ref's header raw: its state directory is repo-derived path text, and the ref can leave the machine with the repo (DR-75).
- **Consequences:**
  - Contracts: `ExportHeader` with `state_dir`; `DoctorResult`, `DoctorReport` and its parts; the `backup_behind` and `backup_ref_failed` warnings; six newly marked path fields; `redact::value_with_open`; `meta.backup_seq` and `meta.backup_ref`.
  - Known gaps: the restore that reads the backup and the restore card come in the next wave; `doctor --restore` too; `doctor` doesn't read `PLANROOM_PACKS` yet; old set-aside files are kept and never pruned in M1.
  - Tests: the unit tests in `crates/store/src/backup.rs`; `crates/planroom/tests/backup.rs`; the doctor tests that traceability.md's R10 row names; `kernel::redact`'s `value_with_open` cases.
- **Sources (checked 2026-10-29):** `crates/store/src/backup.rs`; `crates/planroom/src/app/{backup.rs, doctor/}`; `crates/extract/src/git.rs`'s `write_backup_ref`; [Where the workspace lives](architecture.md#where-the-workspace-lives); [Contracts](contracts.md).

## DR-111: Levels are abstraction levels that packs declare, under the kernel's intent

- **Status:** accepted (decided on the owner's request of 2026-10-29; recorded at the merge of M1's levels change). Amended by DR-114.
- **Starting point:** kept KICKOFF's "Packs. Declarative files that add kinds, levels, roll-up rules, extractors, lenses, decision kinds and a default view" (KICKOFF.md:383). "Levels. C4 (System, Container, Component, Code) with Intent on top" (:384) stops being a closed set in the contract: C4 becomes the code pack's ladder under the kernel's `intent`, and every stored spelling stays. Amends DR-42 (five fixed levels), DR-45 (the domain-free scan), DR-46 (the container prefix, image roles and tones), DR-59 (the breaking-change guard), DR-85's wording of the lid and DR-109's known read gap.
- **Origin:** the owner, 2026-10-29: "I consider the box metaphor to be abstraction levels essentially. I don't really want it hardcoded everywhere like that."
- **Context:**
  - `contract::Level` was a closed enum of five names deriving `Copy`, `Ord` and `clap::ValueEnum`, its declaration order the level order (`crates/contract/src/model.rs:59-81`). Its schema was a closed `enum`, and its TypeScript a closed union.
  - The kernel, the app, the linkers, the truth comparator, `@planroom/ui`, the canvas and the docs spelled the names. `System` was the top of the map and the pane's focus; `Intent` the plan's level; `Container` the linkers' join and the fallback for a question's level and a path's; `Component` a hand-made group's fallback; `Code` the finest level and `intent`'s target level. Each level's neighbours were match arms (`app/map/focus.rs:53-59`, `app/map/page.rs:347-353`).
  - KICKOFF lists levels among the things packs add, and nothing in it asks for exactly five. R1 asks for "several levels of abstraction, from intent at the top to code at the bottom", each a faithful roll-up of the one below.
  - Every stored level is one of five snake_case strings: in fact bodies, edit events, opaque cursors, layout keys, snapshot capabilities, inbox rows and pack files.
  - The breaking-change guard read only a definition's top-level `enum` (`crates/xtask/src/schema_guard.rs:108-113`), so any open level type read as five removed values.
  - The literal scan checked kind and relation names only (`crates/xtask/src/boundaries.rs:365-440`), so nothing would stop a level name coming back into the kernel or a surface.
- **Decision:**
  - **A level is an abstraction level, and levels are data.** A level says how far down the map you've zoomed, not what kind a thing is.
    - The kernel owns one level, `intent`, the plan's, which sits apart from the containment ladder (DR-45).
    - The base pack, `packs::BASE` (`code`), built in and loaded first, lists the ladder top first in `[[levels]]`. Each level has an `id`, the `label` that "the {level} level" reads, and the count words `one` and `many`, each holding `{count}` once.
    - The code pack's ladder is `system`, `container`, `component`, `code`, so every stored value keeps its spelling.
  - **Other packs insert below.** A pack may put a level directly `below` a ladder level it can see: the base pack's, its own, or a required pack's.
    - At most one inserted level sits directly below any one level, whichever pack inserts it. The base ladder's own steps don't count, and a pack may chain its second level below its first.
    - Nothing goes above the base pack's top, and nothing below the base pack's finest. So the map's top and bottom stay the base pack's whatever else is loaded, and `intent`'s targets and R1's "code at the bottom" keep their meaning.
    - The ladder holds at most 8 levels below intent, 9 in all.
    - A kind's `level`, a lens's `level` and a suggestion's `map --level` must name a level their pack can see.
  - **Nothing names a level but `intent` (amends DR-42).** `contract::LevelId` is a string with no order of its own. Every comparison goes through `kernel::ladder::Ladder`: each level's rank, the top, the finest level, and the next level up or down. The rules that named a level now read:
    - the map with no level, and the pane's focus, show the top;
    - a focus opens the level below; a cut's drill-down names the level above;
    - a hand-made group takes its coarsest member's level;
    - `intent`'s targets prefer the finest level;
    - a question with no lens, and the scan summary's count, take the default lens's level, else the top;
    - a node at a level no loaded pack declares resolves at the finest level;
    - a `--level` no loaded pack declares is `invalid_argument`, naming the levels; a malformed one stays clap's usage error.
  - **Containers by their prefix (amends DR-46).**
    - The kinds that use the kernel's `container` prefix share one level, and every kind at that level uses the prefix. So a reclassified container keeps its ID, and the rule names no level.
    - An image role names a kind that uses the prefix.
    - The linkers take the containers to be those kinds' nodes, and read the top level's kinds beside them.
    - A tone is any kind's block color.
  - **The contract (amends DR-59).**
    - `Level` keeps its schema name as an open set: the stable value `intent` plus the code pattern. `schema_version` stays 1: every stored value reads unchanged, and no release exists (DR-85's precedent).
    - The guard counts an old enum value as kept when the new definition still accepts it: in an `enum`, in an `anyOf` or `oneOf` branch's `enum`, or by a `pattern`. A value the new pattern refuses is still removed. After a release, opening a closed set needs a bump.
    - A `MapView` at graph detail carries the ladder in `levels`, the not-scanned reply included. The canvas reads it on both transports, since the Artifact's snapshot map is read at graph detail.
    - `ask` alone lists the levels, in ladder order, before the kinds.
  - **Guarded (amends DR-45).** AGENTS.md's surface rule reads "no built-in kind, lens or level ID". The literal scan also refuses a string literal equal to a built-in pack's level ID in the kernel's and the contract's sources, the generated `@planroom/contract`, `@planroom/ui`, the canvas and the mod, outside tests. It allows only the uses `boundaries.toml`'s `level_words` lists file by file: an error's `code` field, the actor kind `system`, the login link's `code` parameter and `/room`'s code row. An entry that matches nothing fails the check. The scans' comments in `boundaries.toml` now say exactly what each checks.
  - **The drawing stays (amends DR-85's wording).** Blocks nest like boxes, and a lid lifts on a level zoom: that is how the table draws an abstraction level. Every user-facing word speaks of abstraction levels, no surface names a level, and intent sits beside the ladder rather than beside `system`.
- **Alternatives rejected:**
  - **A depth number per level:** two packs' numbers can clash by chance, and a pack's level above `system` would move every workspace's top map.
  - **Raw tree depth,** the owner's words taken literally: components nest to any depth (`examples/TRUTH.json:7`), so each directory would be a level. "The container level shows every one of the 11 truth containers" and roll-up's "nearest ancestor at or above the level" would lose their meaning.
  - **Declared roles in pack data (`top`, `unit`, `finest`):** a second role mechanism for one of the five ideas, which DR-45 rejected for Intent. The ladder's ends, the default lens and the container prefix cover every rule that named a level.
  - **A level below the base pack's finest:** `intent`'s targets, the focus clamp and the resolve fallback would move onto another pack's level, and R1's bottom would no longer be code.
  - **A check only on `LevelId::from(..)` and comparisons with `.level`:** a literal reaches a surface by many other routes, a closed list in the generated TypeScript among them.
  - **A level picker on the pane's `z`:** "back up a level" would take two keys instead of one, and every level is already reachable.
  - **Renaming the lid classes, selectors and tests:** they name the drawing, not a level.
  - **`schema_version` 2:** no stored byte changes, and no release exists.
  - **The ladder on `PaneModel` or the snapshot:** the canvas reads it from the map it draws.
  - **A warning for a node at an undeclared level:** it happens only after a pack that inserted a level is removed, and the node still shows.
- **Consequences:**
  - **Contracts:**
    - `LevelId` (schema name `Level`, an open set); `LevelDecl` and `PackFile.levels`; `LevelVocab` and `Vocabulary.levels`; `MapView.levels` at graph detail;
    - `invalid_argument`'s `levels` and `argument.unknown_level`;
    - the `[pack]` reasons `level_intent`, `level_below`, `levels_over`, `level_words` and `level_unknown`, with `prefix_container`, `prefix_reserved` and `image_kind` reworded;
    - en.toml's `[levels]` table for intent's words; the scan summary's `{units}`; the map summary's counts in the ladder's words, in ladder order, such as "16 components";
    - `ask`'s listing of levels; the usage error's synopsis reads `--level LEVEL`, and the TypeScript `Level` is `("intent") | string`.
  - **Layout:** gated surfaces, ratified with `cargo xtask gen --layout-lock DR-111` in this merge commit:
    - AGENTS.md's Boundaries (the kernel's and the surfaces' literal rules) and Conventions ("Where new code goes" gains "a level | `[[levels]]` in a pack, `below` a level the pack can see");
    - `boundaries.toml` (the scans' comments, `level_words` in `[rust.kernel]` and `[ts.rules]`, and `contract` in `[ts.rules] domain_free`);
    - the area contracts table (the kernel's and the contract's Must-not cells).
  - **Budgets:** this merge raises no gated number. `budgets.json` gates no golden at this merge, and the band's `tool_results` arrive with its merge as new keys measured on this tip (DR-89). The fold benchmark shows no slowdown: measured together in one container, base then levels, `fold.20k`'s medians are 116 ms and 106 ms and `fold.50k`'s 812 ms and 684 ms.
  - **Decided at the merge:** the top-level map's first graph page holds 6 of its 10 blocks, since each graph page now carries the ladder; the rest come on the next page, as `meta.next` says.
  - **Known gaps:**
    - reads (`map`, `find`, `focus`, `ask` and the rest) still load the built-in pack set alone, so an out-of-tree pack's level is checked at load and takes its place on its set's ladder, but `map --level` refuses it with `invalid_argument` naming the built-in levels, and its blocks count at the finest, until reads load the workspace's recorded pack set (DR-109's gap, widened here);
    - a node at a level no loaded pack declares resolves at the finest level, with no warning;
    - the mod's own lines fill `{level}` with the level's ID, which equals its label in the code pack; `/room`'s count rows keep the raw IDs, in key order;
    - the mod's typeahead offers no `--level` values; `ask` alone lists them;
    - no pack can replace the base ladder, or add a level above its top or below its finest;
    - the TypeScript scan finds a kind only by its `<pack>.` prefix, so a bare lens ID isn't scanned;
    - the app, extract, store and testkit aren't in the level scan: the missing `Ord` and the tests on an invented ladder guard them;
    - the scan's test exemption reads only a bare `#[cfg(test)]`.
  - **Tests:**
    - `crates/kernel/tests/ladder.rs`; `crates/kernel/tests/rollup.rs::rollup_is_faithful_on_any_ladder`; `crates/kernel/src/fold/state.rs::a_put_with_any_level_string_keeps_its_node`;
    - `crates/contract/tests/schema.rs::{level_ids_read_and_write_the_old_names_byte_for_byte, the_level_schema_is_an_open_set_holding_intent}`;
    - `crates/xtask/tests/schema_guard.rs::{an_enum_opened_to_an_open_set_that_keeps_its_values_passes, a_value_the_new_pattern_refuses_is_still_removed}`; `crates/xtask/src/boundaries.rs::{cfg_test_items_are_skipped_whole, a_brace_in_a_string_or_char_never_ends_a_test_item}` and the boundary self-test's level plants;
    - `crates/planroom/tests/packs.rs`'s ladder, insertion, cap, words, visibility and prefix checks, `a_level_below_the_base_packs_finest_is_pack_invalid` and `two_inserted_levels_below_one_level_are_pack_invalid` among them;
    - `crates/planroom/tests/map_read.rs::{an_unknown_level_is_invalid_argument_naming_the_levels, level_strings_take_labels_and_handles_keep_ids}`; `map_graph.rs::graph_detail_carries_the_ladder_and_summary_does_not`; `usage.rs`'s clap-path cases on a malformed level; `s10_extensibility.rs::{ask_alone_lists_the_levels_in_ladder_order, a_packs_level_sits_on_the_ladder_and_the_fold_places_its_blocks}`;
    - `packages/ui/src/map.test.ts::block_counts_follow_the_ladder`; `packages/canvas/src/levels.test.ts::crumbs_come_from_the_ladder_with_the_apart_level_beside`;
    - `feature_list.json` C18, "Levels are pack data".
  - **Users:** the same map, words and links as before. A pack can add a level, and the zoom rail, the breadcrumb and the phone's levels sheet follow the loaded ladder.
  - **Milestones:** M1 gains the slice "Levels as pack data" (8 h), taking it from 122 h to 130 h.
- **Amended by DR-114:** the read gap is closed: `map --level` takes a recorded pack's level and its blocks count there, and `ask` and the graph's ladder list it. The S10 test `a_packs_level_sits_on_the_ladder_and_the_fold_places_its_blocks` is now `a_packs_level_sits_on_the_ladder_and_every_read_shows_it`.
- **Sources (checked 2026-10-29):** KICKOFF.md:58, :383-384; `crates/contract/src/{model, pack, vocab, view, requests, command, errors, open_set, card}.rs`; `crates/kernel/src/{tree, rollup, resolve, find, plan}.rs`, `query/{mod, lookup, forest}.rs`, `fold/state.rs`; `crates/planroom/src/app/{map/*, focus, scan, edit, intent, ask, find, import, share}.rs`; `crates/planroom/src/packs/{mod, validate}.rs`; `crates/planroom/src/cli.rs`; `crates/planroom/tests/usage.rs`; `crates/extract/src/{lib, link/mod, link/runtime, declarative/mod}.rs`; `crates/testkit/src/truth.rs`; `crates/xtask/src/{boundaries, rustlex, schema_guard, bench}.rs`; `boundaries.toml`; `packs/code/pack.toml`; `packages/ui/src/map.ts`; `packages/canvas/src/{levels, strings}.ts`, `shell/zoom-rail.tsx`, `views/map-view.tsx`, `transport/artifact.ts`; `examples/TRUTH.json`; [Levels and roll-ups](architecture.md#levels-and-roll-ups); [Versioning rules](contracts.md#versioning-rules).

## DR-112: Diagrams for people and agents: pictures the engine derives from its views, and diagrams agents draw that stay ghosts until a person accepts them

- **Status:** accepted (decided on the owner's request of 2026-10-29, at the merge of M1's levels change). M3 and M4 build it; M1 builds none of it.
- **Starting point:** extended KICKOFF's principle "Never make someone read what a summary, diagram or diff could show them instead" (KICKOFF.md:38), R4 (:64) and the canvas kit's "statechart view with a simulator" (:418). Kept DR-3 (a view is a value; auto-layouts are a cache), DR-4 (an agent's write is a ghost), DR-11 (one machine model), DR-22 (no `--format`) and DR-54 (a view is picked by its result type). Amends DR-5 for one case: accepting a diagram ghost writes the `review` event alone, with no copying `edit`. Builds on DR-111, the levels record.
- **Origin:** the owner, 2026-10-29: "Additionally, if we could have a concept of cached, generated diagrams that help convey information to the user and the agent, that'd be nice. Can be agent generated I guess. Things like state machines and different systems in play would be nice."
- **Context:**
  - The engine already computes pictures, as typed views. A map at a level through a lens shows the systems in play; the `runtime` lens at its level is a container diagram built from evidence (architecture.md:870-877). M3 adds the path view and M4 the machine view. The canvas draws views with ELK and caches positions by view and structure (`crates/store/src/layouts.rs:8-33`).
  - Some pictures the map can't derive: a flow an agent traced through code, a lifecycle's states before M4 extracts machines, a sequence across systems. An agent can draw these, but R5 says nothing the AI invents counts until a person accepts it.
  - Agents read views as JSON rows; Mermaid is text they read and write well. Every surface of the mod host has a `Markdown` element; `Svg` needs a laid-out drawing and isn't on the terminal (`packages/mod/vendor/claude-code.d.ts:3919-3927`). `/room` rows are text or code (`packages/mod/src/room.ts:173-176`).
  - Derived state is never stored (architecture.md:297). No surface package may load Mermaid: the canvas's third-party list names `elkjs` and no Mermaid library (`boundaries.toml:43`), and the Artifact page is the embedded canvas build (`crates/planroom/src/app/share.rs:4`, `:44`).
  - `ask` returns `AskResult` (question, summary, items, empty; `crates/contract/src/requests.rs:194-201`); no contract type marks an answer as a path or a machine.
  - `show ID` is documented (architecture.md:1397) but is in neither the catalogue nor the band's branch, so nothing may hang on it.
  - An event body of a kind a reader doesn't know decodes as `EventBody::Unknown` (`crates/contract/src/event.rs:151`); `review` takes an `EventId` (`:111-112`); `edit undo EVENT` reverses the whole command whose first event that is (`crates/contract/src/requests.rs:85-86`, `:109`); the store keeps each event's `kind` and `target`, indexed by target (`crates/store/migrations/0001_init.sql:7-23`).
  - DR-5 has accepting a ghost write a new `edit` that copies its fields, under compare-and-set, with the `review` event as the audit trail (decisions.md:244; architecture.md:1408).
- **Decision:**
  - **One shape, two sources.** A diagram is Mermaid in a safe subset (`flowchart`, `sequenceDiagram`, `stateDiagram-v2`), a legend from each alias to the node ID it cites, and a content hash. Both kinds reply as `DiagramView`. The engine derives one from a view it computes; an agent draws one the map can't derive.
  - **Derived: a picture of a view, derived on read and stored nowhere.** It can't go stale, and it is never a second model of the graph (DR-11's reasoning). The source's type picks the form (DR-54): a map is a flowchart; a path a sequence (M3); a `MachineView` a state diagram (M4).
    - `diagram map [--level L] [--lens X] [--focus ID] [--known HASH]` pictures that map: its blocks as nodes, each lifted edge labelled with its relation's pack label, and `×N` when N > 1. `diagram map --lens runtime` shows the systems in play.
    - `diagram ask QUESTION [ARGS]... [--known HASH]` pictures the answer of a question whose pipeline's last step is a `path` (a sequence: participants in hop order, one message per hop) or, from M4, a `machine` (a state diagram: `[*]` to the initial state and from the terminal ones, one `A --> B: event` per transition, with guards, actions and the checks' marks as notes). The form comes from that step's shape in the vocabulary, and `AskResult` doesn't change. Any other question is `invalid_argument` with `argument.no_diagram_form`.
    - Both are leaves of their own with flat flags and positionals, so their HTTP and inbox bodies are flat; there's no nested optional subcommand.
    - The canvas, the pane's mini-map and the Artifact keep drawing the views themselves. A derived diagram is for agents and `/room`.
  - **Drawn: an agent's diagram, stored, checked against the map, and a ghost until a person accepts it.**
    - `diagram draw SUBJECT TOPIC --file F|-` takes a `DiagramDraft { title, mermaid, legend }`, inline through `room_change`'s `payload` (DR-66). SUBJECT is a block or a project, named as every command names one. TOPIC is a handle, `^[a-z0-9][a-z0-9-]{0,47}$`, such as `checkout-flow` or `order-states`.
    - The engine parses the Mermaid in the safe subset and resolves every legend entry (a full ID, an ID suffix or a name, as commands resolve one; `not_found` or `ambiguous_ref` names the alias). It refuses a draft that cites no node. It passes the title and every label through `kernel::redact` and `display::sanitize`, cuts each label to 60 characters and the title to 80, and draws the diagram again in its own canonical form with generated aliases. The stored and served Mermaid is always the engine's, never the agent's bytes.
    - It is stored as a `diagram` event, a tenth event kind: the subject in `events.target`, and in the body the topic, the form, the title, the parsed model, the cited IDs, the hash, and a fingerprint of the cited nodes' facts. The fingerprint covers each cited node's kind, level, parent and name, and its evidence's paths and line texts, line numbers left out, as the fold gives them. There's no table and no migration. Older readers keep the event as `Unknown`, and undo, the event backups, export and the stream carry it as they carry every event.
    - **A ghost until a person accepts it (R5, DR-4).** An agent's or a model's draw commits with status `Ghost` and is drawn translucent. A person's draw applies. Decisions lists diagram ghosts in their cause's ghost batch, read from the events by kind and target (the `events_target` index), not from the fold.
    - **Review and undo reach the event by its ID.** `review EVENT accept|reject` takes the `diagram` event's ID. A person's accept writes the review alone: a diagram sets no field, so DR-5's copying `edit` has nothing to copy (this record amends DR-5 for diagrams). `reject` hides it. `edit undo EVENT` reverses the draw, since the `diagram` event is its command's first event. An agent may reject or `edit undo` only its own (DR-4's mixed rule).
    - **Stale when a cited node changes.** On read, the fingerprint is taken again over the current model. Any difference makes the diagram stale (`fresh: false`), and the reply names the changed cited IDs (at most 10, with a count of the rest). An acceptance stays, and a stale diagram is drawn with the amber `~` tape until an agent redraws it.
    - **What shows.** Per subject and topic: the newest diagram a person drew or accepted, and the newest ghost after it as a proposal beside it. Older ones count in `alternatives`. A newer ghost never displaces what a person accepted.
    - `diagram show TARGET [TOPIC] [--known HASH] [--detail summary|graph] [--cursor C]` reads them. TARGET is a subject (one row per topic: title, form, status, `fresh`, hash and handles) or a draw's event ID; with TOPIC, it returns that diagram whole.
    - **The fold reads no diagram body.** A diagram changes no graph, so the fold's own event read leaves diagram bodies out. `diagram show`, `export`, the event backups and their ref, and the stream read them whole.
  - **Kept by content; agents read either kind by hash.** Every `DiagramView` carries `hash`, the sha256 of its Mermaid and legend after `kernel::redact`, which the engine computes whether or not `PLANROOM_RAW` is set, so the hash never changes with it. With `--known HASH` equal to it, the reply is `unchanged: true` with no Mermaid, legend or model, and it still carries the title, the status, `fresh` and the hash. An agent pays for a picture only when it changed.
  - **The safe subset,** parsed for drawn diagrams and the only form any diagram is written in:
    - One statement per line, blank lines allowed. The headers: `flowchart LR|RL|TB|TD|BT`, `sequenceDiagram`, `stateDiagram-v2`.
    - Flowchart: a node `A["label"]`, `A("label")`, `A[("label")]` or `A{"label"}`; a link `A --> B`, `A --- B`, `A -.-> B` or `A ==> B`, each with an optional `|"label"|`; `subgraph A["label"]` … `end`.
    - Sequence: `participant A as label`, `actor A as label`; `A->>B: label`, `A-->>B: label`, `A->B: label`, `A-->B: label`; `Note over A: label`, `Note over A,B: label`, `Note left of A: label`, `Note right of A: label`; `loop label`, `alt label`, `else label`, `opt label`, `end`.
    - State: `state "label" as A`; `[*] --> A`, `A --> B`, `A --> B: label`, `A --> [*]`; `note left of A: label`, `note right of A: label`.
    - An alias in a draft is `^[A-Za-z_][A-Za-z0-9_]{0,31}$` and no Mermaid keyword, matched without case: `graph`, `flowchart`, `subgraph`, `end`, `direction`, `click`, `call`, `href`, `style`, `linkStyle`, `class`, `classDef`, `state`, `note`, `participant`, `actor`, `loop`, `alt`, `else`, `opt`, `par`, `and`, `rect`, `critical`, `break`, `autonumber`, `activate`, `deactivate`, `create`, `destroy`, `box`, `title`, `accTitle`, `accDescr`, `default`, `left`, `right`, `of`, `over`, `as`, `fork`, `join`, `choice`, `LR`, `RL`, `TB`, `TD`, `BT`.
    - Refused anywhere, with the line's number in `invalid_argument`'s details: `%%` (comments and directives), `click`, `call`, `href`, `style`, `classDef`, `class`, `linkStyle`, `:::`, `@{`, a backtick, a chained link (`A --> B --> C`), `&`, and any other line.
    - Caps: 16 KiB of Mermaid; 60 nodes; 120 links, messages or transitions.
  - **Text that can't inject.**
    - Written aliases are always generated (`n1`, `p1`, `s1`, …): never names, never node IDs (which hold `:` and `/`), and never repo text, so no unmarked field carries repo text. A state diagram writes each state as `state "Name" as sN`, its name in a marked label.
    - Labels are redacted, sanitized and cut to 60 characters. A flowchart quotes them and writes every character outside `[A-Za-z0-9 _.,/@()+-]` as `#<decimal>;`. A sequence or state diagram writes every character outside `[\p{L}\p{N} _.,()=!+-]` as `_`.
    - No `classDef`, `class`, `style`, `linkStyle`, `click`, directive or comment is ever written.
    - `title`, `mermaid` and every label are marked `x-redact`; the legend's IDs, the aliases, the topics and the hashes aren't, and none of them holds repo text.
  - **M4's machines pack reads the state form back.** Its Mermaid pattern reads `[*]` as the initial or terminal pseudo-state, `state "Name" as sN` as that state's name and `A --> B: event` as a transition, and it skips `note` lines. So a rendered machine committed to a repo doc reads back to itself.
  - **Where people see diagrams (no sixth concept, R14).**
    - **Canvas:** a block's diagrams sit in M3's focus view (Levels), a project's beside its shelf entry (Intent). One opens in a diagram view that the `DiagramView` result type picks (DR-54). The canvas's own ELK worker lays it out from the engine's parsed model (`DiagramView.model`, sent only at `--detail graph`, the canvas's read), and its positions go in the layouts cache under `diagram:<event ID>`. A cited node takes its kind's `tone`; a ghost draws translucent; a stale diagram carries the tape. No Mermaid library (`boundaries.toml:43`).
    - **The pane:** `/room diagram …` draws its summary line and a `markdown` row holding a mermaid fence, a new `RoomRow` kind. A host that draws Mermaid shows a picture; any other shows the text. Its `text` is the summary line and handles, within `/room`'s 300 tokens. No new pane section or key.
    - **The Artifact page** is the canvas build, so it draws the diagram itself with the canvas's diagram view, laid out by ELK from its snapshot. That is how this record reads "the Artifact page draws Mermaid itself": `boundaries.toml:43` allows no Mermaid library, so the page draws the parsed model, not Mermaid text. `Snapshot.diagrams` holds the accepted diagrams, models included, of the snapshot map's blocks and of each project, after the maps in the 1 MB order, with capability keys `diagram:<subject>`.
    - **Agents** read both kinds through `room_query` (`diagram map`, `diagram ask`, `diagram show`) and draw through `room_change` (`diagram draw`). The `planroom` skill gains three lines: ask `diagram map` or `diagram ask` for a picture of a map or an answer; draw what the map can't show with `diagram draw`, citing block IDs, and expect a ghost until a person accepts it; pass `--known` with the hash you hold.
  - **The leaves:**

    | Leaf | Idea | Class | Undo | Tool | Budget |
    |---|---|---|---|---|---|
    | `diagram map` | Lenses | read | query | `room_query` | 1,500 |
    | `diagram ask` | Lenses | read | query | `room_query` | 1,500 |
    | `diagram show` | Levels | read | query | `room_query` | 1,500; its list form pages with `--cursor` |
    | `diagram draw` | Levels | change (an agent's is a ghost) | `edit undo` | `room_change`, inline `payload` | 400 |

    Each is actor `any`, `needs_base` false and `spends` false.
  - **Budgets.**
    - A diagram never pages. Past its budget, a derived one keeps the most-connected blocks (ties by ID), counts the rest in `omitted` and warns `truncated` with `map`'s drill-down. A draft whose `diagram show` reply would pass 1,500 tokens is refused at draw time, so every stored diagram fits its read.
    - `budgets.json` gains `tool_results["diagram-map"]`, `["diagram-path"]`, `["diagram-show"]` and `["diagram-draw"]` from the M3 goldens: new keys, not raises (DR-89). `tools.test.ts::tool_results_within_budget` gates them, and `tool_schemas_equal_catalogue` holds the four new synopsis lines inside `surface.tools`' 900 tokens; a raise there needs its own record. `tests/plugin/surface_tokens.sh`, planned for M1's surface sweep, measures the same surface once it lands.
    - If a derived diagram costs an agent more tokens than `map --detail summary` on the fixture, the skill doesn't point agents to it.
  - **No Planroom model call draws a diagram in M3.** The agent in the person's session draws with its own tokens. A Claude-drafted diagram would be a decision kind behind M2's consent card, with its own record.
- **Alternatives rejected:**
  - **Derived pictures only, with agents' drawings kept as repo docs:** that answers "systems in play" and M4's machines, but not the owner's cached, agent-generated diagrams of what the map can't derive.
  - **An agent's drawing as a description that applies at once:** R5 says nothing the AI invents counts until a person accepts it. A ghost needs no new authority class and no change to DR-4.
  - **A structured JSON draft of nodes, edges and groups instead of Mermaid:** agents write Mermaid fluently, and one parser for the subset serves both the check and the read; the engine still stores the parsed model.
  - **Grounding each drawn edge with `why`:** it would refuse useful drawings of flows the map doesn't hold yet. A drawing is the agent's claim, a ghost until accepted, and its cited nodes still make it stale.
  - **A diagrams table, or saved selections as a `kernel.diagram` node:** a table needs its own live path, review target and backup story, and a node kind would be one every node read skips. An event kind brings ghosts, review, undo, backups, export and the stream as they are.
  - **Pack-declared `[[diagrams]]` recipes:** lenses and levels already name every view a derived diagram pictures.
  - **A Mermaid library in the canvas or on the Artifact page:** `boundaries.toml:43` allows none, and adding one is a gated change; the canvas lays out the parsed model with ELK.
  - **State names as aliases in a derived state diagram:** they are repo text, and aliases sit in unmarked fields that redaction never masks.
  - **Planroom's own Claude call to draw (`diagram draft`):** a paid step needs M2's consent card and daily cap.
  - **Discovery through `show ID`:** that leaf isn't built; `diagram show SUBJECT` and the focus view list a block's diagrams.
  - **A `--format mermaid` flag on `map`:** DR-22.
  - **A diagram in `focus`:** the pane keeps to 1,000 tokens (DR-105).
  - **A 2,000-token read for drawn diagrams:** above `map`'s 1,500; a draft that wouldn't fit is refused instead.
  - **Generating architecture.md's diagrams or the concept pages' loops:** `own_loops_have_no_drift` would then check nothing.
- **Consequences:**
  - **Contracts (M3):**
    - `Command::DiagramMap`, `DiagramAsk`, `DiagramDraw` and `DiagramShow`, with their requests;
    - `DiagramView`, `DiagramForm` (`flowchart`, `sequence`, `state`), `DiagramModel` (nodes with alias, label, cited ID and tone; links, messages or transitions with label; subgraph groups), `DiagramDraft`, `DiagramRecord` as `EventBody::Diagram`, and `DiagramRow`;
    - `Snapshot.diagrams`; `argument.no_diagram_form`; `invalid_argument`'s details gain `line`, the refused Mermaid line's number; en.toml's `[diagram]`; the marks above.
    - All additive (a new event kind is an open-set value), so `schema_version` stays 1. Event kinds become ten; architecture.md's moving-parts budget and Events section change when M3 builds it, and DR-5's accept bullet gains the diagram exception.
  - **Milestones:** M3's slices summed to exactly 30 h (architecture.md:1810-1816). M3 grows by 17 h, to 47 h: the derived leaves 5 h (‖), and the drawn diagrams 12 h (the contract 1 h, the parser and checks 2.5 h, the event and the fold's skip 1 h, the two leaves with review and undo 2.5 h, the canvas view 2.5 h, the pane's fence 0.5 h, the Artifact's snapshot 1 h, tests and docs 1 h). M4 grows by 1.5 h, to 36.5 h, in its machines-pack slice. The total goes from 382 h to 408.5 h with the levels record's M1 slice.
  - **Known gaps:**
    - no Mermaid parser is on an allow-list, so CI checks the subset and the escaping, not that Mermaid draws them;
    - whether a host draws a mermaid fence is unverified;
    - a drawn state diagram isn't paired with code machines for drift unless an agent commits it to a repo doc, which the machines pack reads;
    - a drawn diagram's links are the agent's claim;
    - diagram events are never pruned, though the same content replays;
    - the Artifact shows accepted diagrams only;
    - the derived diagram's token saving over `map --detail summary` is measured when the leaf lands.
  - **Tests:**
    - `crates/kernel/tests/diagram.rs` (M3): `a_map_diagram_keeps_the_maps_blocks_and_lifted_edges`, `an_edge_label_carries_its_lifted_count`, `aliases_are_generated_never_names`, `a_state_is_written_with_a_generated_alias_and_its_name_as_a_label`, `labels_cannot_break_out` (a property test planting `"]-->x`, `%%{init:{}}%%`, `click n1 call x()`, a backtick, a newline, `<svg onload=x>`, U+202E, NUL, and states named `end` and `state`), `the_hash_covers_the_redacted_bytes_not_seq`, `a_cut_keeps_the_most_connected_blocks`, `rendering_is_deterministic`, `a_path_diagram_keeps_the_hops_in_order`, `the_safe_subset_parses_and_renders_back_to_itself`, `a_line_outside_the_subset_is_refused_with_its_number`, `a_changed_cited_node_changes_the_fingerprint_and_a_line_shift_does_not`;
    - `crates/planroom/tests/diagram.rs` (M3): `diagram_map_alone_pictures_the_top_of_the_map`, `a_diagram_holds_every_block_and_lifted_edge_of_its_map`, `a_known_hash_returns_unchanged`, `an_answer_with_no_diagram_form_is_invalid_argument`, `a_cut_diagram_warns_truncated_with_a_drill_down`, `diagram_output_is_redacted` (a planted secret, in a node name and in a state name, is masked and appears in no alias or legend entry; the hash equals the sha256 of the redacted fields, the same across two runs and under `PLANROOM_RAW=1`), `diagram_ask_path_draws_the_hops_in_order`, `an_agents_drawing_is_a_ghost_until_a_person_accepts_it` (the accept writes one `review` event and no `edit`), `an_agent_cannot_accept_a_drawing`, `an_agent_may_reject_or_undo_only_its_own_drawing`, `every_cited_id_must_resolve`, `a_drawing_goes_stale_when_a_cited_node_changes`, `a_newer_ghost_never_displaces_an_accepted_drawing`, `a_draft_whose_reply_passes_the_budget_is_refused`, `a_redraw_of_the_same_content_replays`, `the_fold_reads_no_diagram_body_and_an_export_and_a_backup_keep_it_whole`, `diagram_requests_round_trip_through_the_schema`;
    - `packages/mod/src/room.test.ts::room_diagram_draws_a_mermaid_fence_and_keeps_text_within_budget` (M3);
    - `tests/e2e/src/diagram.spec.ts` (M3): `a_blocks_diagram_opens_from_its_focus_view_laid_out_without_mermaid`, `a_ghost_diagram_draws_translucent_and_a_stale_one_taped`, `the_artifact_page_draws_an_accepted_diagram_from_its_snapshot`; `surface.spec.ts::exactly_five_concepts` visits the diagram view;
    - `crates/planroom/tests/s06_machines.rs::{a_rendered_machine_reads_back_through_the_mermaid_pattern, a_machine_diagram_notes_unreachable_and_dead_end_states}` (M4);
    - the goldens `diagram-map.json`, `diagram-path.json`, `diagram-show.json` and `diagram-draw.json`, through `cargo xtask gen --golden`;
    - `feature_list.json` C19, "Diagrams for people and agents" (tier C; R2, R4, R5, R13), added by M3's change with these tests.
  - **Users:** an agent can ask for a picture of any map, path or machine, and keep it by its hash. It can draw a flow, a sequence or a state machine the map can't show; a person sees it as a ghost on the block or project it is about, and can accept it; a cited block's change marks it stale.
- **Sources (checked 2026-10-29):** KICKOFF.md:38, :64, :66-69 (R5), :418; `crates/contract/src/{view, command, requests, event}.rs`; `crates/store/src/layouts.rs`; `crates/store/migrations/0001_init.sql`; `crates/kernel/src/{redact, display}.rs`; `crates/kernel/src/query/mod.rs` (`Answer`); `crates/planroom/src/app/share.rs`; `boundaries.toml`; `packages/mod/vendor/claude-code.d.ts`; `packages/mod/src/room.ts`; DR-5 (decisions.md:244); [Layers, overrides and ghosts](architecture.md#layers-overrides-and-ghosts); [Events](architecture.md#events); [Lenses and questions](architecture.md#lenses-and-questions); [The v0.1 packs](architecture.md#the-v01-packs); [The canvas](architecture.md#the-canvas); [Milestones](architecture.md#milestones).

## DR-113: `intent next` drafts a plan stage from replay, as the model's ghosts, and `schema --commands` pages

- **Status:** accepted (decided at the merge of M1's `intent next` from replay, 2026-10-29).
- **Starting point:** extends DR-23 and DR-106; amends DR-18, DR-23, DR-93 and DR-109. Builds on DR-111, the levels record.
- **Origin:** the merge of M1's `intent next` from replay, with its recordings, read against the docs by independent checks.
- **Context:**
  - DR-23 has surfaces with no chat run `intent next`, where the engine asks Claude with the plan pack's one shared cached prefix and the stage's method in the tail. The leaf wasn't built: the catalogue had no `intent next`, and S3's stage tests had no recordings.
  - DR-23's prefix sends "the union `PlanProposal` schema". Structured output refuses an open object, and `PlanProposal`'s props are one.
  - The Prompt injection row and R-10 gave each request a random fence. A random fence changes the request's bytes on every call, so no replay key would hold.
  - DR-18's session backend needs the mod's broker, which comes in M2, and so does the invalid-reply card architecture.md promises for a reply that fails its schema.
  - With 24 leaves, `schema --commands` measured 1,581 tokens. contracts.md and DR-21 say the listing pages, but the code returned it whole. `schema --def Command` passed its 1,500 tokens too, and DR-109 decided at its merge to hand off types behind one `schema --commands` handle rather than raise the budget.
  - The levels change (DR-111) merged first. `intent next`'s pools of touch targets read the old closed level enum.
- **Decision:**
  - **The leaf.** `planroom intent next [--stage S] [--project P]` drafts one plan stage. It takes the named stage, or the next stage with no items. It writes the reply's items as ghosts whose writer is `Actor::Model` with the pinned model's ID and the caller's client, whoever runs it, so only a person's review accepts them (DR-4). A key the store already holds (`Snapshot::holds_key`) replays its reply, or meets `idempotency_mismatch`, before any call. `meta.models` reports the call, and `cache_not_applied` a prefix that was neither written nor read.
  - **A stage that drafts nothing.** The lint stage, an unknown stage, or no stage left is `invalid_argument` on `stage`. For lint or none left, the line names the exact `planroom intent lint --project ID`, the details carry it as the `next` handle, and the hint is that command. The CLI keeps a hint whose details carry `next`, as it keeps an environment variable's (DR-93) and a card's step (DR-89); `details` still carry `synopsis` and `schema`.
  - **The request.** Its stable prefix is the plan pack's one stage prompt, `packs/plan/prompts/plan.md` (the stages in depth, the item rules, the reply's working limits and a worked example), then the reply's schema. The prefix is at least 16,000 bytes (21 KB as built), the same for every stage and project, on the capable tier with a 1-hour cache. The tail is the stage's slice, `StageSlice`, and the stage's question from en.toml: its method, its checklist as the pack writes it, and the item types it writes.
  - **The slice** holds the project, the stage and every item, live and ghost, in `intent list`'s order. For a stage that writes an item type a rule requires a `kernel.touches` edge of, it adds the blocks `find` ranks best for the live items' words and every block the plan touches. It holds no seq, ID, key, time, path or evidence, so the plan and the map alone fix the replay key. It is masked by its marks, then cut to 8,000 bytes: targets first, then text props to 200 characters, then items, each cut counted. So a secret is never cut short of the shape that masks it.
  - **The fence comes from the content.** `request_tail` wraps every model request's slice in `<repo-data id="r<first 16 hex of the masked bytes' sha256>">`, with every `<` inside written `<`. Repo text can't close the fence, and the same slice gives the same bytes, so its replay key holds. This replaces the random fence in architecture.md's Prompt injection row and R-10.
  - **The reply is `DraftReply`**, `PlanProposal`'s strict form: props as `{name, value}` pairs, the names an enum. The engine maps it back to a `PlanProposal`. `contract::draft_reply_schema` shapes the derived schema into structured output's strict form: every subschema inlined, every object closed, every property required, an `Option` as `anyOf [T, null]`, and an item's type and a prop's name each an enum of the sorted names given.
  - **An unusable reply is `model_reply_invalid`** (failed, exit 8, retry `never`), in place of the card until M2's decision runner. A reply is unusable when it is cut off or refused, isn't JSON in its schema's shape, holds items the stage's checks refuse, or changes nothing. It writes nothing and keeps the recording. Once the stage and project are known, a model refusal (`model_reply_invalid`, `model_unavailable`, `replay_missing`, `budget_exceeded`) hints the exact `planroom ask stage S PROJECT`, which drafts the stage in chat instead.
  - **Dropped targets.** In an engine draft, a touch target that isn't one block on the map is dropped, not refused. The item is still written as a ghost, and the summary counts what was dropped (`intent.targets_dropped`). Lint's `next_action_target` flags the item only when no target remains. `intent propose` still refuses such a target.
  - **Backends in M1.** `claude.backend` `auto` and `gateway` both reach the gateway adapter, whose gate picks the recording, the recorded live call or the `replay_only` refusal. `session` is M2's and `off` turns calls off; each is `model_unavailable`.
  - **`PLANROOM_REPLAY_DIR`** names the recordings' folder by an absolute path. A relative one is `invalid_argument` whose hint is the export that sets it (DR-93). With none set, strict replay finds no record and reads nothing relative to the folder a command runs in. Every test that replays a call sets it to the checkout's `tests/replay/`. Records are filed by purpose, `tests/replay/claude/plan.stage.<S>/`. No default works (R14): a shipped binary can't know where a checkout's recordings are, and a path relative to the working directory would answer model calls from whatever files the folder a command runs in holds, a scanned repo's included.
  - **S03's recordings and spend.** `cargo xtask live record S03` recorded the six engine-run stage calls (intent, outcome, breakdown, next actions, priorities and premortem) for "Add retries to the worker" in one run, cold, at $0.0877. The build-wide spend log then held $0.4032 of the $3 cap. The first call wrote the shared prefix and the five after it read it, at a 0.81 hit rate. `budgets.json` gains `"s3.plan"`: `prompt_total` 11,115 and `output` 1,113, the largest of the six calls, and `cache_hit_rate` 0.6, the floor. It is a new key, not a raise (DR-89). A stage call reserves $0.10 in the spend log before it is sent, and takes at most 6,000 output tokens. The live spend plan's S03 row states these bounds.
  - **`meta.models` after a paid call that fails** isn't set: an error envelope carries no `meta`. The spend log still records the call, and M2's ledger and invalid-reply card report it.
  - **`schema --commands` pages** under its 1,500 tokens, each leaf whole, in catalogue order, naming the next page in `meta.next`. `--cursor C`, only with `--commands`, reads it, and the pages join to the whole listing the catalogue hash covers. The cursor is opaque, as every list's is: hex-encoded JSON of seq 0, the page's last leaf and the catalogue hash. It reads only where this catalogue cuts a page after that leaf; any other cursor, such as a leaf's index or one from another build, is `invalid_argument` on `cursor`. The other `schema` forms stay whole replies.
  - **`schema --def Command`** keeps `Command`, every variant and its `$ref`, without descriptions. It hands off the leaf request types it leaves out behind one `schema --commands` handle, whose listing names each leaf's `schema --command LEAF`. Every other type, and every other reply, keeps its per-type `schema --def` handles. The budget is unchanged.
  - **The level sites take the levels record's forms.** In `app/draft.rs`, the slice's target pool and the reply's touch pool keep only the nodes the ladder doesn't hold apart (`!ladder.is_apart`), `find::find` takes the ladder, and `lookup::lookup` resolves a target at `ladder.finest()`. The ladder comes from the loaded vocabulary. In `app/intent.rs`, the touch pool, its lookup and `ambiguous`'s `level` detail (`LevelId::INTENT`) take the same forms. No level literal is added, and `level_words` doesn't change. The stage request holds no level, and `rank` reproduces the old level order, so the recordings replay unchanged. None of the levels record's decisions change.
- **Alternatives rejected:**
  - A random fence per request: no replay key would hold, so CI couldn't replay a single call.
  - `PlanProposal`'s own schema in structured output: it refuses the open props object.
  - Refusing a whole draft for one target the map doesn't hold, as `intent propose` does: one bad target would discard a paid stage, and lint still flags an action with no target.
  - Recordings read from a compiled-in path or from the working directory: a shipped binary has no checkout, and a scanned repo could plant replies.
  - A raised budget for `schema --commands` or `--def Command`: every new leaf would raise it again.
  - A bare page index as the listing's cursor: any integer in range read a page no page gave, and a cursor saved against another build read pages that don't join this one.
  - The schema handle as the lint refusal's hint: it describes the leaf's arguments, while the fix is one exact command.
  - Error envelopes carrying `meta.models`: a wider envelope change than M1 needs, and M2's ledger reports the call.
- **Consequences:**
  - Contracts: `Command::IntentNext` and `IntentNextReq` (`--stage S`, `--project P`); `StageSlice`, `SliceItem`, `SliceTarget`, `DraftReply`, `DraftItem`, `DraftProp`, `DraftValue` and `draft_reply_schema`; the error `model_reply_invalid`; `SchemaReq.cursor`; the marks on `StageSlice.name`, `SliceItem.name`, `SliceItem.props` and `SliceTarget.name`; `invalid_argument`'s `next` detail and its hint; a relative `PLANROOM_REPLAY_DIR` in `invalid_argument`. All are additive, so `schema_version` stays 1.
  - Behaviour: the plan pack's new prompt file changes `packs_hash`, so every store rescans once. The catalogue gains `intent next`, and its hash changes. Every model request's slice is now fenced. `schema --commands` returns pages.
  - Known gaps: `kernel::redact` doesn't mask a secret glued to the text before it with no word boundary, which is existing behaviour; a stage's reply may write any declared item type, each checked against its type's schema; S03's recordings are keyed on the fixture's map, so a change to the fixture, `find`'s ranking, the slice, `plan.md` or the schema re-records all six; `meta.models` is absent from an error envelope after a paid call until M2.
  - Tests: `crates/planroom/tests/s03_plan.rs::{stages_produce_ghosts_until_accepted, a_stage_that_drafts_nothing_is_refused_before_any_call}`; `budgets.rs::{s3_cache_hit_rate_at_least_0_6_after_the_first_stage, every_built_leafs_reply_on_the_fixture_keeps_to_its_budget}`; `live_gate.rs::{intent_next_without_a_record_flag_is_budget_exceeded, a_relative_replay_dir_is_refused, the_proxy_alone_makes_no_model_request}`; `privacy.rs::a_planted_repo_secret_reaches_no_request_or_tool_output`; `crates/planroom/src/app/draft.rs`'s unit tests (the shared prefix, the prop cut, the slice's marks, a fifty-item draft); `crates/contract/src/plan.rs::the_draft_schema_is_strict_and_order_free`; `crates/models/src/gate.rs::a_relative_replay_dir_is_refused`; `version.rs::{schema_commands_pages_join_to_the_whole_catalogue, a_schema_commands_cursor_no_page_gave_is_refused}`; `crates/planroom/src/app/version.rs::{schema_def_command_hands_its_leaf_requests_off_by_one_handle, a_schema_commands_cursor_from_another_catalogue_or_off_a_page_edge_is_refused}`; `catalogue.rs::undo_classes_snapshot`.
  - Users: with no chat, a person or a CI job drafts a plan's next stage with one command and reviews the model's ghosts. An agent reads the command list a page at a time.
- **Sources (checked 2026-10-29):** `crates/planroom/src/app/{draft, intent, models, version}.rs`; `crates/planroom/src/cli.rs` (`with_usage`); `crates/contract/src/{plan, command, errors, requests}.rs`; `crates/models/src/gate.rs`; `packs/plan/{pack.toml, prompts/plan.md}`; `tests/replay/claude/plan.stage.*/`; `budgets.json`; `docs/evidence/spend.jsonl`; [Claude](architecture.md#claude); [Live spend plan](architecture.md#live-spend-plan); [Contracts](contracts.md#error-codes).

## DR-114: Every read and write loads the workspace's recorded pack set

- **Status:** accepted (decided at the merge of M1's recorded-set reads, 2026-10-29).
- **Starting point:** extends DR-62 and DR-109; amends DR-106, DR-109 and DR-111. Builds on DR-48 (an untrusted workspace's packs) and DR-73 (a read in one transaction).
- **Origin:** a gap found while levels and `intent next` were built: scans and the write guard used the workspace's recorded pack set, while every read still ran on the built-ins alone.
- **Context:**
  - DR-109 made the pack set workspace state. Scans extract with the recorded set, and every authored write checks its record. Its known gaps said reads (`focus`, `ask`) use the built-ins, and DR-106's said the same of `ask stage` and `intent lint`.
  - DR-111 widened the gap. `map --level` refused an out-of-tree pack's level with `invalid_argument` naming the built-in levels, and that level's blocks counted at the finest.
  - So a workspace that recorded a pack held blocks extracted with it, yet `map`, `find`, `ask` and `focus` couldn't name its kinds, levels or relations, and a person's `edit relate --kind` with the pack's relation was refused.
  - Two sites in `app/draft.rs` were outside the change while `intent next` was being built there: `proposal`, which reads a stage's reply back, and `stage_call`, the seam the privacy test reads. Both still loaded the built-ins.
- **Decision:**
  - **The read set.** `packs::state::for_read(root, record)` is the one place a command of a workspace gets its pack set:
    - no record, or no out-of-tree pack in it: the built-ins, with no I/O and no trust check;
    - an untrusted workspace: the built-ins with no warning, as its scans (DR-48);
    - otherwise this engine's built-ins, then each recorded out-of-tree pack from its copy, in record order, whatever engine recorded it.
    - It is read-only: it never copies, sets aside or reads a source, and it compiles no query.
  - **One transaction.** A read resolves the set from the record its own snapshot reads (`Snapshot::recorded_packs`), so a reply folds with the set its snapshot records (DR-73). A write resolves its set before its transaction opens, so trust's git call never runs under the write lock.
  - **The sites.** `map`, `find`, `focus`, `ask` (its listing, its stages, its questions and its budget), `intent lint`, `list`, `propose` and `next`, `edit`, `review`, `import`, the live overlay, and scan's own summary, which reads the set the scan extracted with. Outside a workspace, before a store, `schema`, `version` and the live loop's repo kind read the built-ins.
  - **`intent next`'s draft (decided at this merge).** `draft::proposal` looks up the reply's touches on the ladder of the set the map was folded with, and `draft::stage_call` folds and drafts with the recorded set. Before, a touch that named a block at a pack's own level by its name was dropped, because the built-in ladder's lookup order never reaches that level. A full ID was found either way, and `is_apart` agreed, since only `intent` is apart in any set.
  - **The fallback.** A copy that is missing (`copy_missing`), one that no longer hashes to its name (`copy_changed`), or a set this engine can't load (`invalid`) gives the built-ins alone and the warning `packs_not_loaded {pack, reason}`. For `invalid`, the message names `planroom doctor --pack` on the copy. Every reply of a workspace's read or write carries it once, and text mode prints it as a line. Only `scan` stops instead, with `pack_invalid`. A failed set is never kept.
  - **The error envelope (decided at this merge).** A command that fails after its set fell back, such as `map --level zone` when the pack that declares `zone` lost its copy, has no reply to carry the warning. `for_read` notes the warning in a process-wide list, each once and at most 16, and the CLI puts the list in its error envelope's `meta.warnings` after the store's notices. Text mode prints it after the error. This is the store notices' pattern (`planroom_store::take_notices`), and it uses the existing warning code. `serve`'s error replies carry no warnings, as they carry no store notices. The canvas asks only for levels its replies listed, so it doesn't meet this refusal.
  - **One load per process.** A process keeps one recorded set, by its hash, shared by scans and reads, so `watch`, `serve` and a scan load it once. A scan compiles the queries of a set a read kept.
  - **What a read costs (decided at this merge).** In a debug build, each one-shot read of a workspace that records any out-of-tree pack costs about 70 to 100 ms more than a read of the built-ins:
    - median of 10 runs, the fixture, which records Ruby: `map --json` 313.0 ms and `focus --json` 297.7 ms; with no out-of-tree pack (`fixture --no-packs`), 211.8 ms and 206.2 ms;
    - the same workspace trusted against untrusted: `map` 321 ms against 232.5 ms, `focus` 300.5 ms against 229 ms;
    - the S10 workspace (the fixture plus a pack with a level) differs from the plain fixture by 3 to 26 ms.
    - No budget gates a one-shot read's wall-clock. The fold's 50 ms ([Performance budgets](architecture.md#performance-budgets)) times the kernel's fold alone, which this change doesn't touch.
    - The bar it bears on is commit to rendered (p99 under 1 s): the pane runs `focus --json` once per burst of frames, and `serve` loads the set once. That bar's gates, `tests/e2e/src/s08-tab.spec.ts::rendered_within_1_s` and `packages/mod/src/refresh.test.ts::pane_within_1_s_under_s8_frames`, arrive in M2. The canvas's interactive gate measured 406 to 435 ms over 5 cold loads on this tree.
    - Nothing changes now. The cost is the built-ins resolved a second time in one process: once for `builtin_records`, through `packs::loaded()`, and again inside `checked`. Building the read set from the built-ins the process already parsed is the cheap fix. It lands with M2's S8 harness if that harness shows the pane near its bound. A cache of the resolved set on disk, keyed by the recorded hash, follows only if that isn't enough.
  - **The fold benchmark (checked at this merge).** `cargo xtask bench fold` misses its budgets in this container on the base too, so the miss isn't this change's. Measured alternately in one container, two runs each, while another build ran:
    - the base: `fold.20k` 78.54, 86.91 and 79.95 ms; `fold.50k` 580.06, 573.52 and 578.01 ms;
    - this change before the merge: 82.86 and 92.79 ms; 595.46 and 566.68 ms;
    - after the merge: 88.16 and 85.45 ms; 601.0 and 586.4 ms.
    - The change touches nothing the benchmark builds but one `WarningCode` variant. DR-111's merge measured 116 and 106 ms here, and CI's runners time `fold.20k` between 38 and 61 ms. `budgets.json` is unchanged.
- **Alternatives rejected:**
  - Reading a recorded source when its copy is missing, as a scan does: a read would follow a path outside the workspace and write a copy.
  - Refusing a read when the set doesn't load, as a scan does: the whole map would go dark over one copy that the next scan restores.
  - Resolving the set inside a write's transaction: trust's git call would run under the write lock.
  - An `AppError` that carries warnings through every handler: a wider change than one drain point in the CLI, for the same envelope.
  - A new error code for a request refused after the fallback: the request's own refusal is right, and the warning says why it was made.
  - Leaving `app/draft.rs` on the built-ins as a known gap: the fix is one parameter, one call and a test.
  - A cache on disk now: a format to keep before the gate that would need it exists.
- **Consequences:**
  - Contracts: the warning `packs_not_loaded` (`details {pack, reason}`), now also in a CLI error envelope's `meta.warnings`; the store's `Snapshot::recorded_packs`. Both are additive, so `schema_version` stays 1.
  - Behaviour: a recorded pack's levels, kinds, relations, lenses and questions show in every read and write of a trusted workspace. `ask` with no question now opens the store when there is one, so a store that fails to open fails it too.
  - Known gaps:
    - a write resolves its set before its lock, so a `pack add` or `pack remove` that commits in between leaves that write on the previous set;
    - `serve`'s error replies carry no warnings;
    - the per-process cost above, until M2's S8 harness measures the pane.
  - Tests:
    - `crates/planroom/src/packs/state.rs::{no_record_or_a_built_in_record_reads_the_built_ins, a_read_set_compiles_no_query}`;
    - `crates/planroom/src/app/draft.rs::a_drafted_touch_names_a_block_at_a_packs_own_level`;
    - `crates/planroom/tests/packs.rs::{every_read_and_write_loads_the_recorded_set, a_missing_or_changed_copy_reads_the_built_ins_and_writes_nothing, an_older_engine_reads_the_recorded_copies_with_its_own_built_ins, a_request_refused_after_the_fallback_says_which_pack_didnt_load}`;
    - `trust.rs::an_untrusted_workspace_reads_with_the_built_ins_as_it_scans`;
    - `s10_extensibility.rs::a_packs_level_sits_on_the_ladder_and_every_read_shows_it`, renamed from `a_packs_level_sits_on_the_ladder_and_the_fold_places_its_blocks`.
  - `feature_list.json`: C18, "Levels are pack data", passes.
  - Users: a pack a person adds shows everywhere: its level on the map, its kinds in `find`, its questions in `ask`, its relations in `edit`.
- **Sources (checked 2026-10-29):** `crates/planroom/src/packs/{mod, state}.rs`; `crates/planroom/src/app/{map/mod, map/read, focus, find, ask, scan, edit, review, import, intent, draft}.rs`; `crates/planroom/src/cli.rs` (`finish_shrunk`); `crates/store/src/{meta, fs_check}.rs`; `crates/contract/src/errors.rs`; `crates/planroom/text/en.toml`; [What a pack is](architecture.md#what-a-pack-is); [Performance budgets](architecture.md#performance-budgets); [Contracts](contracts.md#error-codes).

## DR-115: Starting from nothing: a plan-only workspace, intended blocks that next actions target and that resolve when code lands, and a hand-off that starts a new repo

- **Status:** accepted (decided on the owner's request of 2026-10-29).
- **Starting point:** extends DR-23 and DR-106 (the plan pack and its proposals), DR-57, DR-85, DR-86 and DR-87 (empty states), DR-25 and DR-64 (hand-off clones), DR-48 and DR-101 (trust), DR-73 (a person's field wins), DR-80 (landed commits as change units), DR-110 (event backups) and the `intent next` draft (DR-113). DR-117, DR-118, DR-119 and DR-120 build on its intended blocks and plan-only workspaces. Keeps KICKOFF's "AI proposes, person disposes", every person-only gate of DR-64 and DR-75, and the intended layer as architecture.md:343 states it. Amends DR-57 (only `scan` makes a workspace), DR-74 (where the state directory is, and the reserved namespaces), DR-85 (`no_repos`'s next action), DR-87 (`repos_unborn` when a plan exists), DR-106 (what a proposal writes and what a next action may target) and DR-25 (a hand-off clones only repos that exist, and merges with `--no-ff`).
- **Origin:** the owner, 2026-10-29, on starting from nothing: "it has to be supported". The owner accepted the proposals that follow: "Yeah I like these. Get them planned properly."
- **Context:**
  - A folder with no repos is no workspace. `focus` and `map` show "No git repos here" with `focus` as the next action (architecture.md:593, contracts.md:181), and `scan` refuses with `workspace_not_found` when it finds no repos (`crates/planroom/src/app/scan.rs:89-92`).
  - `intent new` opens an existing store only, so it fails with `workspace_not_found` before any scan (`crates/planroom/src/app/intent.rs:174-180`). No plan can start in an empty folder.
  - A `.planroom/` at a repo's root is never that repo's state directory (architecture.md:590; `crates/planroom/src/app/workspace.rs:110-121`). So a person who starts a plan and then runs `git init` in the same folder would lose sight of it.
  - The intended layer names intended nodes (architecture.md:343), and `Node` already carries a `layer` (`crates/contract/src/model.rs:254`). But no command puts an intended block: `EditOp` writes renames, groups, pins and intended relations only (`crates/contract/src/requests.rs:30-70`), and plan items sit at the kernel's `intent` level, never on the ladder (`crates/kernel/src/plan.rs:101-116`).
  - Attachment refuses a target that matches no block (architecture.md:1229). An engine draft drops such a target and counts it in `intent.targets_dropped`, and lint flags an item left with none (the `intent next` change, DR-113). With an unborn or README-only repo, the map is near empty, so next actions have almost nothing to target.
  - A hand-off clones each touched repo and checks out its base commit (architecture.md:1311). `handoff merge` runs `git merge --no-ff` (architecture.md:1344), and `SpecRepo.base_commit` is required (contracts.md:1135-1137). So the first implementation has nowhere to go. Git refuses `--no-ff` into a branch with no commit ("Non-fast-forward commit does not make sense into an empty head"; git 2.43.0, checked 2026-10-29), and `--ff-only` into one works.
  - The mod starts `watch` once `focus` answers from a store (`packages/mod/src/watch.ts:169-171`). The card kinds are an open set (`crates/contract/src/card.rs:47-53`). Swift and Kotlin have no built-in grammar; the code pack's languages are TypeScript, JavaScript, Python, Go and Rust, with Ruby as a pack (architecture.md:944).
- **Decision:**
  - **A plan-only workspace (amends DR-57).** A folder becomes a workspace when `scan` or `intent new` runs there.
    - `intent new TEXT` in a folder with no store creates the state directory and the store where `scan` would, records the directory as trusted by the engine, writes its `workspaces.json` entry as `scan` does (so the room lists its boards, DR-119), and puts the project. The reply's summary says so in one line: "Started a plan in newthing/. No repos yet."
    - The project follows DR-106: an agent's `intent new` puts it as a ghost, so nothing the agent starts counts until a person accepts it. The state directory and the store hold no authored state of their own, as with an agent's `scan`.
    - It refuses the home directory and the filesystem root with `path_refused`, reasons `home_directory` and `filesystem_root`. A store there would make every folder below it one workspace, since the root is the nearest ancestor that holds one. The hint is the one line that makes a folder for the plan.
    - `focus`, `map` and the other reads work in it. `scan` in a workspace with a store and no repos returns ok, maps nothing and writes no event: "No repos to map yet; your plan is kept." With no store and no repos, `scan` still refuses with `workspace_not_found` and creates nothing.
  - **Empty states (amends DR-85 and DR-87).**
    - `no_repos` (no repos, and no store or a store with no plan) says "No git repos here. Start a plan, or add a repo." Its next action is `schema --command "intent new"`, "Start a plan here", as `no_intent`'s is. It was `focus`, which only looped.
    - New `plan_only`: `focus` or `map` with nothing at the top, where a plan exists and no repo has a commit. The line says the map fills from the plan's intended blocks and from each repo's first commit. Its next action is `intent list`, "Show the plan".
    - `plan_only` wins over `repos_unborn`, since the plan is the work in hand and the line still says a first commit maps the repo. `repos_unreadable` keeps winning over both, since git can't read a repo until a person fixes it.
  - **A plan begun before `git init` (amends DR-74).** A `.planroom/` at a repo's root counts as that repo's state directory only when the data root records that exact directory as trusted (its root, path, inode and birth time, DR-101) and git tracks nothing inside it. A clone or a copied tree can't carry such a record, so a committed `.planroom/` still loads nothing. Every other `.planroom/` at a repo's root stays ignored, as before. The engine never moves the directory into `.git/planroom/`.
  - **Intended blocks (extends DR-106).** An intended block is a node on the intended layer at a ladder level: a repo, service, module or file the plan means to exist.
    - `PlanProposal` gains `blocks`. Each `ProposedBlock` has a `key`, a `kind` that a loaded pack declares at a ladder level, a `name`, an optional `parent` and a required `expect`.
    - Its ID is `intended:<id_prefix>:<key>`, the key following DR-106's rule. `intended` joins `ws` and `plan` as a reserved namespace, so a repo directory named `intended` takes the 4-hex suffix and an intended block never shares an ID with a fact.
    - The ID carries no project. The intended layer is shared by the workspace's projects (architecture.md:529), so one intended block can serve two plans. A proposal that names an intended key that already exists re-proposes that block, as DR-106 re-proposes an item: a person's sets only the fields that differ, and an agent's is a ghost against the block, which every project then sees.
    - `expect` says where its code will live: `repo`, a namespace that exists or a new repo's directory name (same character rule as a key, left unchanged by redaction, since it becomes a namespace), an optional repo-relative `path`, and an optional `branch`, the new repo's first branch, which defaults to `main` and is read only when `repo` names no repo. A block with no `expect` is refused, because a scan could never find it. When the agent can't know, it guesses, and the person corrects it before accepting.
    - `parent` is a block at a coarser ladder level, intended or actual, or none at the top. A proposed block may name another block in the same proposal as its parent.
    - Blocks follow the ghost rules: an agent's blocks are one ghost batch with the proposal's items, and a person accepts them, block by block or as a batch.
    - The plan pack's `breakdown` and `next_actions` stages declare `blocks = true`, so `intent next` drafts may propose blocks and `ask stage` lists the ladder's levels with the kinds at each.
  - **Next actions target intended blocks (amends DR-106).**
    - A `kernel.touches` target may be an intended block, accepted or a ghost. An engine draft drops only a target that is neither an actual block nor an intended one in the model or in the same draft.
    - A person's accept of an item whose target is still a ghost block is `invalid_argument` with `field` `touches`, carrying the ghost and its exact `review` in `ghosts`, as DR-106 does for a ghost parent. Accepting the batch accepts both.
    - Lint reads an accepted intended block as a found target.
    - An intended block holds no evidence, so the impact radius lists it and adds nothing around it.
  - **On the map.** `MapNode` gains `intended`. An unresolved intended block draws as a dashed outline in its kind's tone, beside the actual blocks at its level, and a ghost one is also translucent. A resolved block isn't drawn: its actual block is. A plan-only workspace's top level is therefore the plan's intended repos.
  - **Resolution: an intended block meets its code.** `Node` gains `resolved`, the actual block's ID, set by an ordinary edit on the intended block.
    - **Candidates.** The candidates are the actual blocks at the intended block's level, in the namespace `expect.repo` names, whose location is `expect.path` or lies under it. A block's location is its first evidence path, or the repo's root when its evidence names none.
    - **At scan, once.** Inside the scan's transaction, after the snapshot commits, each accepted, unresolved intended block with exactly one candidate of its own kind is resolved by an edit with the `system` actor, `cause` set to the scan. Diffs lists it once: "Intended service api found: api:container:api. Undo." The engine resolves a block automatically at most once. After a person undoes an automatic resolution, only a person resolves that block. So a scan never fights an undo.
    - **The resolve card.** When the candidates are several, or one of another kind, the scan deals one `resolve` card per intended block: "Which block is the intended api?", with up to three candidates by nearest location and "Keep it intended" as the default. A rejected card is dealt again only when the candidate set changes.
    - **By hand.** `edit resolve INTENDED ACTUAL` resolves any intended block to any actual block at its level. A person's applies at once. An agent's is a ghost.
    - **What follows a resolution.** The fold reads a resolved block as its actual block, as it reads a renamed target: `kernel.touches` targets, intended `require`, `forbid` and `allow-only` edges, the impact radius, the reconciler and lint all see the actual block.
    - **When the code goes away.** If the actual block vanishes and no recorded rename carries it, the intended block reads as unresolved again, draws as an outline, and Diffs says so once. It deals no card.
    - Why this is safe: the rule matches only what a person accepted (the block, its kind and its `expect`), it runs only on code a scan maps from the tracked branch, it is one undoable edit, and it never sets a field a person holds (DR-73).
  - **A hand-off can start a new repo (amends DR-25).** A next action whose target is an intended block whose `expect.repo` names no repo puts a created repo in the spec.
    - **At start.** `handoff start` creates the repo at `<root>/<repo>` with `git init --template= --initial-branch=<branch>`, hooks off and the cleared environment, `<branch>` being the intended block's `expect.branch`. The start is a person's press (DR-64), or a live ratification whose envelope names the repo, its directory and its branch (DR-118); nothing else creates a repo. The start card says so: "Creates the repo api/ here on main, then runs cargo test and git commit in it, sandboxed."
    - **The clone.** The engine clones the empty repo locally as for any repo and switches it to an orphan `planroom/KEY`. The implementer's first commit is the branch's root.
    - **One path for both.** A repo that already exists with no commit, such as one a person made with `git init`, takes the same path with no creation step.
    - **Refusals.** `handoff start` refuses with `invalid_argument`, `field` `expect.repo`: a workspace whose root is a repo (a new repo needs a folder of repos), and a name that is a reserved namespace or an existing repo's. A directory in the way that isn't empty fails as `handoff_conflict`, naming its files, with nothing created.
    - **`collect` and cancel** work unchanged, since the real repo exists from the start: `collect` fetches into `refs/heads/planroom/KEY`, and a cancel or drop archives the branch at `refs/planroom/archive/KEY`.
    - **Merge.** The preflight checks that the working tree is empty of everything but ignored files, and that `HEAD` names the unborn base branch. Then the merge runs `git merge --ff-only planroom/KEY`. With no merge commit, the hand-off records the branch's commits as its change units, as DR-80 does for a landed pull request, and `handoff revert` reverts them, newest first. The repo stays, with its history, so nothing anyone committed is lost.
    - **Reconciling.** A created repo's reconcile window starts at the empty tree, so its first scan classifies the hand-off's commits as a later scan would, and the item can reach `done`.
    - **Proof in the spec.** `SpecRule` gains `Exists`, one per intended block the hand-off's items target. `collect` checks it against the proposed snapshot by the candidate rule, so the review card reads "rules 2/2 met, blocks 1/1 found".
  - **Languages with no grammar.** Starting from nothing in Swift or Kotlin maps files and directories only, through the `files` built-in. Intended blocks at those levels resolve against them. A finer level waits for a pack's `regex` extractor, the path AGENTS.md's table gives for any other language. This record adds no grammar.
- **Alternatives rejected:**
  - **Letting `scan` start an empty workspace:** a scan maps repos. One with none would write a scan event that names nothing, the case DR-97 removed for unborn repos.
  - **Giving an intended block the ID its code will have:** an extractor's key isn't knowable before the code exists. A wrong guess leaves the block unresolvable, and a right one puts an intended and an actual node on one ID.
  - **A relation from the intended block to its actual block:** the fold would follow a second edge on every read of either node. A field on the intended node reads the same and follows the fold's rename path. DR-120's `kernel.realizes` joins a concept to the blocks that build it, a different fact; a concept that realizes an intended block follows this record's read-through to the actual block.
  - **A model decision kind to match blocks:** a model would choose what the plan meant. A deterministic rule over what the person accepted, then a person's card, keeps "AI proposes, person disposes".
  - **Resolving at every scan, even after an undo:** the engine would undo the person's undo.
  - **Creating the repo at merge instead of at start:** `collect`, cancel and the archive ref would need a second path with no real repo to hold refs.
  - **An engine-authored root commit, so `--no-ff` works:** the engine has no git identity of its own and never fabricates one. Every merge commit it makes carries a person's identity: the person's own on their `handoff merge` press, or the one a ratification's envelope names (DR-118). An empty root commit that no person asked for is neither.
  - **Making the workspace root itself the new repo:** the state directory would have to move into its git directory.
  - **Moving `.planroom/` into `.git/planroom/` after `git init`:** `watch` and `serve` may hold the store open, the trust record would need rewriting, and the move can cross filesystems. Recognising the trusted directory where it is costs one rule.
  - **Allowing a plan-only workspace in the home directory:** every later session below home would write into it.
- **Consequences:**
  - **Contracts:**
    - `PlanProposal.blocks`, `ProposedBlock`, `Expect { repo, path, branch }` (`path` marked `x-redact`); `Node.expect` and `Node.resolved`, set only on intended-layer nodes; `MapNode.intended`.
    - `EditOp::Resolve` and the `edit resolve` leaf, with its `CommandSpec` and the reversible undo class.
    - The card kind `resolve`.
    - `SpecRepo.created`, with `base_commit` optional and absent only on a repo with no commit; `SpecRule` kind `Exists`; `collect`'s blocks-found count.
    - `StageDecl.blocks` in the pack format.
    - The empty reason `plan_only`, and `no_repos`'s new line and next action.
    - `path_refused` reasons `home_directory` and `filesystem_root`.
    - The reserved namespace `intended`.
    - Strings in `en.toml`: `empty.no_repos` reworded, `empty.plan_only`, `intent.workspace_started`, `scan.plan_only`, `card.resolve.*`, `diffs.intended_resolved`, `diffs.intended_unresolved`, `handoff.creates_repo`.
    - No error or warning code is added. `schema_version` stays 1: the widened `base_commit` and the changed next action meet no released reader (the precedent of DR-85 and DR-111).
  - **Milestones:**

    | Slice | Milestone | h |
    |---|---|---|
    | A plan-only workspace: `intent new` creates the state directory, the store and the trust record; the home and root refusals; `scan` ok with no repos; `no_repos`'s new next action and `plan_only`; a trusted `.planroom/` at a repo's root; the golden `focus` reply | M1 | 5 |
    | Intended blocks: `PlanProposal.blocks`, IDs and the reserved namespace, the ladder and parent checks, ghost batches, `kernel.touches` to intended blocks with the ghost-target accept rule, `StageDecl.blocks` and `ask stage`'s kinds by level, `intent next` drafts with blocks, and the breakdown and next-actions recordings recorded again with blocks (`cargo xtask live record`, under $0.05 in the spend log), `MapNode.intended` | M2 | 8 |
    | Resolution: candidates, the once-only resolution at scan, the `resolve` card, `edit resolve` and its undo, the fold's read-through to the actual block, falling back on a vanished block, Diffs lines | M2 | 7 |
    | A hand-off into a new or unborn repo: creation at start, the orphan clone branch, the refusals, the empty-tree preflight and `--ff-only` merge, commits as change units and their revert, the empty-tree reconcile window, `SpecRule::Exists` and the blocks-found count; after M2's hand-off slice | M2 | 8 |
    | Surfaces: the dashed outline and ghost translucency on the canvas and in the pane's block rows, the `resolve` card on both, the start card's create line | M2 | 3 |
    | The greenfield CI path: an empty folder to a mapped, reconciled repo through one `patch` hand-off | M2 | 2 |

    M1 grows by 5 h. M2 grows by 28 h.
  - **Tests** (all offline: temporary folders and data roots, strict replay, the `patch` implementer with a patch each test writes in its temporary folder, fake keys):
    - M1, `crates/planroom/tests/greenfield.rs`: `intent_new_in_an_empty_folder_starts_a_plan_only_workspace`, `an_agents_intent_new_in_an_empty_folder_waits_as_a_ghost_project`, `intent_new_refuses_the_home_directory_and_the_filesystem_root`, `scan_in_a_plan_only_workspace_maps_nothing_and_keeps_the_plan`, `a_plan_begun_before_git_init_stays_the_workspace`, `a_committed_planroom_dir_at_a_repo_root_still_loads_nothing`.
    - M1, `crates/planroom/tests/empty_states.rs`: `no_repos_offers_to_start_a_plan`, `a_plan_only_workspace_says_the_map_fills_from_the_plan`, `a_plan_wins_over_repos_unborn_but_not_over_repos_unreadable`. `focus.rs::focus_outside_a_workspace_is_an_empty_state_and_creates_nothing` stays as it is.
    - M1, `packages/mod/src/watch.test.ts`: "a plan-only workspace's store starts the watch"; `tests/golden/cli/focus-plan-only.json` from `cargo xtask gen --golden`.
    - M2, `crates/planroom/tests/intended.rs`: `a_proposal_puts_intended_blocks_as_one_ghost_batch`, `a_block_without_expect_is_refused`, `a_repo_named_intended_takes_a_suffix`, `a_proposal_naming_an_existing_intended_key_re_proposes_that_block`, `a_next_action_may_target_an_intended_block`, `accepting_an_item_whose_target_is_a_ghost_block_names_its_review`, `the_map_draws_intended_blocks_and_not_resolved_ones`.
    - M2, `crates/planroom/tests/intent.rs`: `a_breakdown_draft_proposes_intended_blocks_from_replay`, `a_draft_keeps_a_target_that_names_a_block_in_the_same_draft`.
    - M2, `crates/planroom/tests/intended_resolve.rs`: `one_candidate_of_the_same_kind_resolves_at_scan`, `two_candidates_deal_one_resolve_card`, `an_undone_resolution_is_never_redone_by_a_scan`, `an_agents_edit_resolve_is_a_ghost`, `touches_and_rules_follow_a_resolution`, `a_vanished_block_reads_as_intended_again`.
    - M2, `crates/kernel/tests/intended.rs`: `a_resolved_block_reads_as_its_actual_block`, `an_intended_block_adds_nothing_to_the_impact_radius`.
    - M2, `crates/planroom/tests/handoff_new_repo.rs`: `handoff_start_creates_the_repo_with_no_hooks`, `the_patch_implementer_commits_on_an_orphan_branch`, `merge_into_an_unborn_branch_fast_forwards_and_records_the_commits`, `revert_of_a_created_repo_keeps_its_history`, `cancel_archives_a_created_repos_work`, `a_root_that_is_a_repo_refuses_a_new_repo`, `a_directory_in_the_way_is_a_handoff_conflict`, `collect_counts_intended_blocks_found`.
    - M2, `crates/planroom/tests/greenfield.rs`: `an_empty_folder_reaches_a_mapped_repo_through_one_handoff`. An empty folder, `intent new`, a proposal of an intended repo and a next action on it through `intent propose --file` (drafting blocks from replay is `intent.rs`'s test, on the fixture's recordings, so this path needs no recording of its own), accept, `handoff start` with the `patch` implementer, `collect`, `merge`, the rescan that resolves the block, and the item `done`.
    - M2, `packages/ui/src/map.test.ts`: "an intended block carries its tone and the intended mark"; `tests/e2e/surface.spec.ts::empty_state_audit` gains `plan_only`.
    - `feature_list.json` gains a tier A entry, "Start from nothing" (R5, R6, R8), with `greenfield.rs::an_empty_folder_reaches_a_mapped_repo_through_one_handoff` and the M1 `greenfield.rs` and `empty_states.rs` tests above.
  - **Known gaps:**
    - No remote is created. A person pushes a new repo where they want it.
    - A workspace whose root is a repo can't take a new repo.
    - `git init` at a plan-only root after hand-offs created child repos makes the root the only repo, so discovery stops listing the children.
    - A `.planroom/` kept at a repo's root is not safe from `git clean -x`. The event backup (DR-110) and its restore card bring the plan back.
    - Resolution compares level, kind and location only. A block that moved during implementation goes to the card.
    - Two plans that pick one key for a block share it. Two plans that pick different keys for one block make two intended nodes of one kind and one `expect`. DR-119's duplicate check pairs the items that target them; both blocks stay until a person rejects one.
    - Languages with no grammar map files and directories only.
  - **Users:**
    - In an empty folder, the pane says "No git repos here. Start a plan, or add a repo." You describe what you want to build in chat, and the agent starts the plan as a ghost for you to accept.
    - The map shows the planned repos and services as dashed outlines, and the next actions point at them.
    - The hand-off card says it will create the repo, and on what branch.
    - After the merge, each outline fills in with the real block, with one line you can undo. Where the engine can't tell which block is meant, you get one card to pick it.
- **Sources (checked 2026-10-29):** the owner's design session of 2026-10-29; [Main flows](architecture.md#main-flows); [Where the workspace lives](architecture.md#where-the-workspace-lives); [Layers, overrides and ghosts](architecture.md#layers-overrides-and-ghosts); [Planning](architecture.md#planning-stages-ghosts-and-lint-r5); [The change spec](architecture.md#the-change-spec-r8); [The hand-off loop](architecture.md#the-hand-off-loop); [Contracts](contracts.md#cli-json-envelope) and its [Change spec](contracts.md#change-spec); `crates/planroom/src/app/{intent, scan, workspace}.rs`; `crates/planroom/src/app/map/read.rs`; `crates/contract/src/{model, requests, view, card}.rs`; `crates/kernel/src/plan.rs`; `packages/mod/src/watch.ts`; git 2.43.0's `merge` and `revert` on a branch with no commit.

## DR-116: Questions on a sticky line, assumptions you can see, a one-line read-back, and corrections that feed the next draft

- **Status:** accepted (decided on the owner's request of 2026-10-29). M1 builds questions as plan items; M2 builds the sticky line, chat answers, the read-back, the assumption lint, corrections and the intent-capture evals; M3 adds diagrams to questions.
- **Starting point:** extends R5 ("nothing the AI invents counts until a person accepts it"), DR-23 and DR-106 (the plan pack's stages and lint), DR-50 (a batch accepts only the items its card showed), DR-73 (a person's field wins over automatic writes), DR-111 and DR-112 (drawn diagrams), and DR-113 (`intent next` drafts and their recordings). DR-117 reads its questions into context packs, DR-118 adds POC answers and stop pins through it, DR-119 ranks its questions across boards, and DR-120 declares item types through its `[[items]]` table and adds `concept`, `summary`, `stages` and `checklist` to its `ItemTypeDecl`. Keeps every stage, method and rule of `packs/plan/pack.toml`, ghosts until a person accepts them, and the band's no-spend, reversible rule (DR-54). Amends:
  - DR-4 and DR-64: one more agent write applies at once instead of as a ghost, a chat answer, within the bounds below;
  - DR-23 and DR-106: a pack table of item types that no stage owns and any pack may declare, a stage's read-back, and one more rule type, `referenced`;
  - DR-54 and DR-105: one more pane line, and a list page that takes the row keys left in the pool;
  - DR-75: a signed owner row may also answer a question.
- **Origin:** the owner, 2026-10-29: "it's a visual layer. I'm fine with the agent making a reasonable first guess at the plan. As long as I can correct it. Questions is good, no max, but assume the user may stop at any point. I'd curate one or two questions and keep it sticky somewhere. They get updated too along with the plan in case chat or something answers it." On answers given in chat: "Yeah, but make them visible-ish while relevant so the user can correct if needed."
- **Context:**
  - The plan pack has seven stages, each with a method checklist and structural lint (`packs/plan/pack.toml:16-121`), and nine rules of the types `requires_prop`, `requires_edge`, `min_children`, `covers` and `disjoint` (`crates/contract/src/pack.rs:473-479`). Lint checks shape, not meaning; semantic lint is M2's Jev work (architecture.md:1216-1224).
  - An agent's items are ghosts until a person accepts them, one by one or as a batch (architecture.md:347, :1227). Accept or reject is the only feedback, and a reject carries no reason (`crates/contract/src/requests.rs:116-124`).
  - Nothing in a plan asks the person anything. A draft rests on guesses that no item states.
  - `kernel::plan::next_stage` picks the first stage with no live item of a type it writes, and `stage_of` the first stage that writes a type (`crates/kernel/src/plan.rs:249-256`). So an item type that every stage declared would make every stage look drafted.
  - A proposed item has a key, a type, a name, a parent, props and touches (`crates/contract/src/plan.rs:39-56`). The plan pack declares no relations; `kernel.touches` means what an item touches on the map.
  - `PaneModel` carries focus, the open decision, progress, diffs, drift, who and the next action (`crates/contract/src/view.rs:97-113`). The pane's fixed sections take 18 of 34 rows, and the list keeps at least 10 (architecture.md:1468-1479).
  - Every letter and digit of the pane is spoken for. The 24 action keys and the pool of 12 row keys (`packages/ui/src/keys.ts:16-29`) make the 36 one-key actions a pane may have. The pool goes to the top 3 diff lines, the drift line's top item, the who line's top row, then 7 list rows (`packages/ui/src/rows.ts:10`, `:45-58`).
  - Cards are dealt by writes, one open card per kind and subject (`crates/contract/src/card.rs:17-52`); from M2, `decide ID CHOICE` picks them, and only a person may (contracts.md:436, :447).
  - The mod's `prompt.submit` hook sees each prompt's text and its origin: `composer` for the person's own Enter, `bridge` for the person's message through Remote Control, and other kinds for peers, schedules and the SDK (`packages/mod/vendor/claude-code.d.ts:9288-9300`, `:9503-9570`). The mod doesn't hook it today.
- **Decision:**
  - **The agent always drafts; nothing waits for an answer.**
    - Every stage drafts on its best guess. Where it had to guess, it writes a question that states the guess as the assumption the plan is built on.
    - No stage, lint rule, version or hand-off waits for an answer. A question nobody answers keeps its assumption, and the plan reads the assumption as the answer.
    - There is no maximum count of questions. Surfaces show the top ones and page the rest.
  - **A question is a plan item.**
    - Pack data gains an `[[items]]` table: item types any stage may write, which no stage owns. `next_stage` and `stage_of` read stage items only, so a question never makes a stage look drafted. An `[[items]]` ID that a stage also declares is refused at load (`items_stage_clash`). Any pack may declare `[[items]]`; an item type ID that two loaded packs declare is `pack_invalid` (`item_claimed`).
    - The plan pack's `[[items]]` has one entry, `question`. The item's `name` is the question, as a person would be asked it. Its props:

      | Prop | Type | Holds |
      |---|---|---|
      | `assumption` | string | the answer the plan is built on until someone answers |
      | `changes` | string | what a different answer would change |
      | `stakes` | `low`, `medium` or `high` | how costly a wrong assumption is |
      | `rests_on` | list of item keys | the items built on the assumption |
      | `options` | list of up to 3 strings | other likely answers |
      | `answer` | string | the answer, once someone gives one |

    - `intent propose` resolves each `rests_on` entry as it resolves a parent: a key in the batch, a key of the project's items, or a full item ID of the project. It stores full item IDs. An entry that resolves to nothing is `invalid_argument` with `field` `items[i].props.rests_on[j]`.
    - Each stage's checklist gains one line: "Where you had to guess, write a question: the guess as its assumption, what another answer changes, its stakes, and the items that rest on it." It is pack data, so `ask stage` and `intent next` carry it as they carry the rest. It changes every stage's request, so the six stage recordings are recorded again with `cargo xtask live record` (about $0.09, as they cost $0.0877), logged in `docs/evidence/spend.jsonl`.
    - Two structural rules, expressible today: `question_assumption` (`requires_prop` `assumption`) and `question_rests_on` (`requires_prop` `rests_on`).
  - **What a question's state is, derived on every read.** Nothing about a question's state is stored, so every plan change re-evaluates every question, and the sticky slot moves on by itself.
    - A question **rests on** the live items its `rests_on` names, with their descendants, each counted once. Live means accepted, or a ghost still pending, and not done. Done counts from M2, when the reconciler marks items done.
    - Its **status** is the first that holds:
      - `dropped`: nothing live rests on it;
      - `open`: no `answer`;
      - `from_chat`: its `answer` came from a chat answer (below) and no person has confirmed it;
      - `from_poc`: its `answer` came from a passed proof of concept (DR-118 decides when) and no person has confirmed it;
      - `answered`: anything else.
    - **Kept or corrected.** An answered question is `kept` when its `answer` equals its `assumption`, and corrected otherwise. A later re-proposed assumption that differs from the answer reads as corrected, so the items resting on it show up to redraft.
    - **Rank.** Score is the stakes weight times the count of what rests on it: `low` 1, `medium` 3, `high` 9. Ties go to higher stakes, then to the lower item ID. A high-stakes question with two items resting on it (18) outranks a low one with ten (10). The weights are a first guess; the evals don't measure them, so a change to them needs a record.
    - **The sticky slot** holds two `open` questions: first any that a paused hand-off's stop names (DR-118), newest stop first, then the top by score. A `from_chat` or `from_poc` question doesn't take a slot; it shows beside it until it drops.
    - **Ghosts count.** The sticky slot, the rank and `ask stage` read the project's pending draft as well as its accepted items, because the questions are about the draft you're reviewing. Lint keeps reading accepted items only (R5).
    - The engine computes all of this in `kernel::plan`, reads it from the pack's prop names, and names no item type, so the kernel stays domain-free.
  - **Answering.** `intent answer ITEM` with exactly one of `--keep` (the answer is the assumption), `--option N` (one of its `options`), `--text TEXT` (at most 200 characters) or `--confirm` (a person takes a chat or POC answer as their own). It is actor `mixed`, `needs_base` true, undone with `edit undo`, and never spends.
    - **A person's answer applies.** On a question that is still a ghost, it also accepts that one item, in the same command: two events, one undo. Only that item is accepted, not its batch (DR-50).
    - **An agent's answer is a ghost**, like any agent write, unless it is a chat answer. A ghost answer that a person accepts answers the question; so does an accepted re-proposal that sets `answer`. That is how an accepted item resolves a question.
    - Anyone may set `answer` through `intent propose` too, under the same rules. `intent answer` is the narrow leaf the pane, the canvas, the Artifact and the chat use.
  - **A chat answer applies at once (amends DR-4 and DR-64).** When you answer a question in chat, the agent records it with `intent answer ITEM --text TEXT --from-chat --quote QUOTE`. It applies at once with status `from_chat`, and it stays visible while it matters. Here is how that stays safe and reversible:
    - **Only your words.** The only route is `room_change`. The mod keeps your last three prompts whose origin is `composer` or `bridge`, in module memory only: never in `$.state`, never logged, never sent to the engine. Its handler applies the call only when `QUOTE`, at least 2 characters and whitespace-normalized, occurs in one of them. Otherwise it refuses with `invalid_argument` and `argument.quote_not_in_chat`, and the agent may propose the answer as a ghost instead. A prompt from a peer, a schedule, a channel or the SDK never counts. The handler strips `--quote`, so your words never reach the engine, an event or a log.
    - **No other route.** The guard denies any agent Bash call that names `intent answer`, as it does a person-only command. The engine refuses `--from-chat` from any client but `mod:*` with `invalid_argument` and `argument.from_chat_outside_mod`.
    - **Bounded.** A chat answer sets one field, `answer`, on one question. It accepts nothing, changes no other item, starts nothing, and can't confirm a read-back. It is no person action: every person-only command stays person-only.
    - **Never over yours.** A chat answer never sets an `answer` a person holds (DR-73). It commits as a ghost instead and warns `answer_held`, so you see the proposed change and decide.
    - **Visible while it matters.** A `from_chat` question shows beside the sticky line in the pane, and as an "answered in chat" chip beside each item resting on it, on the plan board and the project's shelf. It shows until nothing live rests on it, or until you confirm it.
    - **Two keys from pane focus to correct.** Its row key selects it, then `n` deals its card with the chat answer marked and the other answers beside it. `y` confirms it as yours (`--confirm`). `edit undo` reverses it; an agent may undo only its own.
    - **Seen before you walk away.** A chat answer to a `high`-stakes question counts as answered, so the plan builds on it. Lint keeps flagging it until you confirm it (below), so it can't waive a POC (DR-118), and DR-118's ratify card lists such answers, so you see them before you approve a bounded batch.
    - **A POC's answer shows the same way.** A `from_poc` question shows a "confirmed by POC" chip where a `from_chat` one shows "answered in chat", with the same row, card, `--confirm` and undo. DR-118 decides when a pass writes one and what evidence it carries.
    - This is a guardrail, as DR-4 says of the actor rules, not a boundary between processes of one OS user. An agent that quotes your earlier words out of context can still record a wrong answer. The chip, the card and the undo are the containment.
  - **The sticky line in the pane (amends DR-54 and DR-105).**
    - One row, below the who line and above the mini-map and the list, drawn only when the focused project has an `open` or a `from_chat` question. It shows the top open question and its assumption, cut to width, then "+N" for the other open ones, then "✓N from chat" and "✓N by POC" for the unconfirmed answers. With no open question, it shows the newest chat answer: "✓ from chat: {question} → {answer}".
    - It takes the row key after the who line's. The list takes the keys left in the pool, at most 7, and pages at that size. So every visible row keeps a direct key: with three diffs, a drift item, a who row and a question line, the list pages at 6.
    - **Keys on the selected question line.** `y` keeps the assumption (`intent answer --keep`), or confirms a chat answer. `o` or `n` deals the question card.
    - **The question card** comes from `focus --picker questions [--entry ID]`, face up in place of the open decision, as any picker is. Its question line is the question; its why line is "Assumed: {assumption} · {count} items rest on it". Its choices are Keep, the first two `options`, and Other…, which opens an `Input`. The list holds the other open questions and the chat answers still in play; a row key on one deals its card. A pick runs `intent answer` as your press.
    - At 34 body rows the fixed sections take 19 rows, so the list keeps 15. The compact layout keeps the question line after the top diff line. The mobile mod has no `Input`, so Other… says "Answer in chat".
    - No new letter key. `y`, `n` and `o` keep their meanings: accept, reject and open, applied to a question.
  - **The canvas.** The top two open questions sit as pinned notes in the Intent concept, on every view, so there is no sixth concept (R14). A tap deals the question card in the Decisions stack. Chat-answer chips sit beside items on the plan board and on the project's shelf.
  - **The Artifact (amends DR-75).** A signed owner row may apply `intent answer`, which is reversible, beside `edit`, `review` and `intent propose`. An unsigned row stays a ghost on the Apply card. The snapshot carries `PaneModel.questions`, so the page shows the sticky questions.
  - **A one-line read-back, one key to confirm.**
    - The intent stage declares its read-back in pack data: its parts in order, each a prop of the intent item and its fragment ("You want {want}", " for {who}", ", so that {so_that}"), and a fragment for the non-goals ("; not {names}"). The intent item gains the prop `who`: who the job is for. A part whose prop is unset is left out. Non-goals list up to two names, then "+N". The kernel composes the line and hashes the values it read.
    - A write that changes the read-back deals a `readback` card, whose subject is the project ID and the read-back's hash. It closes any open read-back card of that project as superseded. Its question is the read-back; its why is "Is this what you want?"; its choices are That's it, Not quite and Later, with Later pre-selected, so nothing waits on it.
    - **That's it** (`decide CARD yes`) accepts the pending ghosts the read-back read, and only those (the card's `BatchInfo.items`, DR-50). It records the pick. The read-back is confirmed while a `yes` pick exists for its current hash; any later change to what it reads makes it unconfirmed and deals a new card.
    - **Not quite** (`decide CARD no [--note TEXT]`, at most 200 characters) records your note as a correction. In chat, the agent reads the note through `ask stage` and redrafts the intent. On the canvas and in the mod, the choice opens a one-line field for the note. On the mobile mod, it records the pick and says "Say what's off in chat".
    - `decide` is person-only, so no agent can confirm a read-back.
  - **Lint for high-stakes guesses.** A new rule type, `referenced`, domain-free, read from pack data. Its subject passes when it holds the prop `unless`, and, with `unless_held`, when a person holds that field (DR-73) or the engine set it from a passed POC, so an unconfirmed agent or chat write doesn't count; or, with `live`, when its list prop `live` names no live item; or, with `by`, when a node of `by.kinds` passing `by`'s field tests names it in its list prop `by.prop`. Otherwise it is a finding.
    - Its finding lists the live items its `live` prop names, the first 5 and a count.
    - The plan pack's rule `question_high_stakes_open` uses it with `unless` `answer`, `unless_held`, `live` `rests_on`, `props.type` `question` and `props.stakes` `high`, and no `by`. So a `from_chat` answer to a high-stakes question stays a finding until you `--confirm` it. Its message: "The plan rests on a high-stakes guess nobody has confirmed." It is a finding, never a gate. DR-118's `derisk_question_tested` uses the same type with `by`.
  - **Corrections feed the next draft as "not this".**
    - `review ID reject` takes an optional `--why TEXT`, at most 200 characters, stored in the `review` event's body. The canvas's reject of a plan ghost offers the field; the pane's `n` sends none.
    - `ask stage` returns `not_this`: the project's newest 10 corrections. Each has the item, its stage, what was proposed (each value cut to 160 characters), what you did, and your why or note. What you did is one of:
      - rejected an agent's ghost;
      - changed a field an agent had set;
      - answered a question with something other than its assumption;
      - said Not quite to a read-back.
    - `ask stage` also returns `redraft`: items resting on a corrected answer that haven't been re-proposed since the answer.
    - `intent next` puts the same two lists in the stage request's uncached tail, so the shared prefix stays cached. An empty list is left out, so a request with no corrections keeps its bytes and its recording.
    - Outside a ratification, no Planroom model call redrafts on its own. A redraft is the chat agent's turn, or `intent next` from the stage picker's cost card. The one redraft that runs unasked is a failed POC's, which the ratify press confirmed with its cost (DR-118).
  - **`ask stage`** also returns the project's questions in rank order, with status, assumption and what rests on each: the first 10 open, then the rest counted. It keeps to its leaf's budget, cutting `not_this` before questions.
  - **Diagrams in questions (M3).** A question may carry `diagram`, a topic of a diagram drawn on its project (DR-112). An unknown topic is `invalid_argument`. The question card's why line names it, and `o` on the card shows it. Once `diagram draw` exists, the breakdown and next-actions checklists gain the line: "Where the work changes a flow or a lifecycle, draw it with `diagram draw` on the project, cite its blocks, and name its topic in the question or item it explains." The read-back stays text.
  - **Evals for intent capture.**
    - 24 cases in `packs/plan/evals/intent_capture.cases.jsonl`: a request text and a map, from the fixture or a synthetic one. The case writer writes them, with at least two injection canaries (a request that says "ask nothing" or "assume X"). The independent labeller labels each without seeing any draft: the job's key terms, the ambiguities a good draft asks about (each with its terms), and what the request already settles. The split is by hash, as for Jev kinds.
    - A deterministic scorer reads each recorded intent-stage draft:
      - **coverage:** the share of labelled ambiguities that a question's name or assumption covers by one of their terms;
      - **false asks:** questions about what the request already settles;
      - **read-back match:** the share of the job's key terms in the read-back.
    - `cargo xtask live eval intent_capture`, behind `PLANROOM_LIVE_GATEWAY=1`, records one intent-stage call per case into `tests/replay/claude/intent_capture/`. It stops at $0.45, logged in `docs/evidence/spend.jsonl`.
    - CI recomputes `intent_capture.result.json` from the replays and fails if any byte differs. The holdout floors are coverage 0.75 and false asks at most 0.15 per case; below either, CI fails.
- **Alternatives rejected:**
  - **A gate: no next stage until the questions are answered.** The owner chose "never a gate"; you may stop answering at any point.
  - **A cap on questions.** The owner said "no max". Curation happens in what the surfaces show, not in what the plan holds.
  - **Separate `question` and `assumption` item types.** Every question carries its assumption, and an assumption with nothing to ask is still a question with one, so one type holds each fact once.
  - **Questions as decision kinds or cards.** Cards show one at a time and rank by stakes and staleness. A question has to stay answerable at any time beside the plan, and change with it.
  - **Storing a question's status or the sticky pick.** Derived state is never stored (architecture.md:297), and deriving it on every read is what re-evaluates questions on every plan change.
  - **A `rests_on` relation.** The plan pack declares no relations, and `kernel.touches` means touching the map. A prop of item IDs, resolved like a parent, needs neither.
  - **Ranking by count alone or by stakes alone.** The owner named both, and a product lets a high-stakes question with little resting on it outrank a trivial one with much.
  - **Chat answers as ghosts.** You would press a key for every answer you had just typed, which the owner's "resolves immediately" rules out.
  - **A chat answer with no quote check.** An agent could record an answer you never gave.
  - **Sending your prompt to the engine to check the quote.** Your words would cross into argv or stdin and an engine process; the mod already holds them, and prompts are never logged.
  - **A new pane letter for questions.** All 24 action keys and 12 row keys are in use, at the host's cap of 36.
  - **Taking the row key from the who line or a diff line.** Either breaks a key people already use; shortening the list's page by one only when every section is full breaks nothing.
  - **The read-back as a pack suggestion.** A suggestion's text is static, and it has one action; the read-back changes with the plan and needs Not quite and a record of the pick.
  - **A read-back prop on the intent item, or an `intent readback` leaf.** Cards, `decide` and the pick record already do this, person-only.
  - **An automatic redraft when you correct an answer.** It would spend without a press; the band never spends (DR-54). A failed POC's redraft is different: the ratify card named it and its cost, and the ratify press confirmed it (DR-118).
  - **Judging drafts with a model in CI.** CI is offline. The labeller's labels and a deterministic scorer over replays make the result recompute byte for byte.
- **Consequences:**
  - **Contracts** (all additive; `schema_version` stays 1; `planroom.pack/v1` and `planroom.event/v1` keep their tags):
    - pack: `PackFile.items` (`ItemTypeDecl { id, props }`); `StageDecl.readback` (`ReadbackDecl { parts: [{ prop, text }], not: { item_type, text } }`); `RuleType::Referenced` with params `unless`, `unless_held`, `live`, `by` (`kinds`, `prop` and field tests) and prop tests; the `[pack]` reasons `items_stage_clash`, `item_claimed` and `readback_prop`;
    - plan: `PlanItem.question` (`QuestionState { status, kept, rests, score, sticky }`, status `open`, `answered`, `from_chat`, `from_poc` or `dropped`); `IntentAnswerReq { item, keep, option, text, confirm, from_chat }` with `Command::IntentAnswer`; `AskStageResult.questions`, `.not_this` (`Correction { item, stage, was, now, why, by }`), `.redraft` and `.readback` (`Readback { text, hash, confirmed }`);
    - view: `PaneModel.questions` (`QuestionLine { top, open, from_chat, from_poc }`, `top` at most 2 `QuestionRow { item, question, assumption, stakes, rests, status, answer }`); the picker name `questions`;
    - cards and review: the card kind `readback`; `DecideReq.note`; `ReviewReq.why`, stored in the `review` body;
    - `invalid_argument` reasons `argument.quote_not_in_chat`, `argument.from_chat_outside_mod`, `argument.not_a_question`, `argument.answer_one_of`, `argument.nothing_to_confirm` and `argument.unknown_diagram_topic` (M3); the warning `answer_held`;
    - redaction marks on every new free-text field: `question`, `assumption`, `answer`, `text`, `was`, `now`, `why` and `note`. Item IDs, the hash and the scores aren't marked;
    - en.toml: `[questions]` for the pane line, the card's why line, Keep, Other… and "Answer in chat"; `card.readback.*`; the chips' "answered in chat" and "confirmed by POC";
    - the guard's denial `intent_answer_in_bash`; one more `room_change` synopsis line, which `tool_schemas_equal_catalogue` holds within `surface.tools`' 900 tokens. A raise there needs its own record.
  - **Milestones:**

    | Milestone | Slice | Requirement and scenario | h |
    |---|---|---|---|
    | M1 | Questions as plan items: the `[[items]]` table and its loader checks, `question` with its props, `rests_on` resolved at propose, the derived status, rank and sticky pick in `kernel::plan`, the checklist line and the six stage recordings recorded again, `question_assumption` and `question_rests_on`, `ask stage`'s questions, a person's `answer` through `intent propose` | R5, S3 | 5 |
    | M2 | Sticky questions and answers: `intent answer` for a person (1.5 h); chat answers with the mod's quote check, the guard, the held-field rule and `answer_held` (2 h); `PaneModel.questions`, the pane line, the row-key rule, the compact layout and the question card (2.5 h); the canvas's pinned notes (1.5 h); chips on the plan board and shelf (1 h); signed owner rows and the snapshot (0.5 h); tests and docs (1 h) | R5, R13, S3, S9 | 10 |
    | M2 | The read-back: the stage's `readback` and the composed line and hash (1 h); the `readback` card dealt and superseded (1.5 h); `decide` yes and no with `--note` (1 h); surfaces (0.5 h); tests (1 h) | R5, R13, S3 | 5 |
    | M2 | Assumption lint and corrections: `referenced` and `question_high_stakes_open` (1.5 h); `review --why` (1 h); `not_this` in `ask stage` and `intent next`'s tail (2 h); `redraft` (1 h); tests (1 h) | R5, S3 | 6.5 |
    | M2 | Intent-capture evals: the cases and labels, the scorer, the live record behind its gate, CI's recompute and floors, the report row | R5, S7 | 5 |
    | M3 | Diagrams in questions: the `diagram` prop and its check, the card's line, the two checklist lines | R4, R5 | 1.5 |

    - M1 grows by 5 h, M2 by 26.5 h and M3 by 1.5 h: 33 h in all.
    - M1's slice depends on the plan pack and `ask stage` (DR-106) and on `intent next` (DR-113), whose recordings it records again.
    - M2's slices need its decision runner and `decide` for cards, batch accepts, the plan board, and `intent next`'s recordings.
    - The M3 slice follows DR-112's drawn diagrams.
  - **Tests** (offline, with replays and the `patch` implementer; none needs a live call):
    - M1, `crates/kernel/tests/plan_questions.rs`: `a_question_with_nothing_live_resting_on_it_is_dropped`, `the_score_is_the_stakes_weight_times_what_rests`, `a_descendant_counts_once_however_many_named_ancestors`, `a_rejected_resting_item_no_longer_counts`, `an_answer_equal_to_the_assumption_reads_as_kept`, `the_sticky_slot_holds_the_top_two_open_questions`, `a_reproposal_that_moves_the_resting_items_moves_the_sticky_slot_on`, `pending_ghosts_count_toward_the_rank_and_not_toward_lint`;
    - M1, `crates/planroom/tests/intent.rs`: `rests_on_resolves_keys_and_stores_item_ids`, `an_unknown_rests_on_entry_is_invalid_argument_with_its_path`, `a_question_never_makes_a_stage_look_drafted`, `ask_stage_lists_open_questions_in_rank_order`, `a_persons_proposed_answer_resolves_the_question`;
    - M1, `crates/planroom/tests/packs.rs`: `an_items_type_a_stage_also_declares_is_refused`, `an_item_type_two_packs_declare_is_pack_invalid`; `crates/kernel/tests/lint.rs::a_question_without_an_assumption_or_resting_items_is_flagged`;
    - M2, `crates/planroom/tests/questions.rs`: `a_persons_answer_applies_and_undoes`, `answering_a_ghost_question_accepts_that_item_alone_and_one_undo_reverses_both`, `an_agents_answer_without_from_chat_is_a_ghost`, `from_chat_outside_the_mod_is_invalid_argument`, `a_chat_answer_applies_and_reads_as_from_chat`, `a_chat_answer_over_a_persons_answer_is_a_ghost_with_answer_held`, `a_confirm_makes_a_chat_answer_the_persons`, `exactly_one_answer_flag_is_taken`, `a_chat_answer_fades_when_nothing_live_rests_on_it`, `the_pane_carries_at_most_two_sticky_questions_within_the_focus_budget`;
    - M2, `packages/mod/src/tools.test.ts`: `a_chat_answer_whose_quote_is_not_in_the_persons_recent_prompts_is_refused`, `the_quote_never_reaches_the_engine`, `a_peer_or_scheduled_prompt_is_not_the_persons_words`; `packages/mod/src/guard.test.ts::intent_answer_from_bash_is_denied`;
    - M2, `packages/ui/src/rows.test.ts`: `the_question_line_takes_the_key_after_who`, `the_list_takes_the_keys_left_so_every_visible_row_keeps_one`; `packages/mod/src/layout.test.ts::rows_fit_at_49x34` and `rows_fit_compact_at_80x24_and_100x30` with the question line;
    - M2, `packages/mod/plugin-tests/surfaces.test.tsx`: `a_question_is_kept_in_two_keys_from_pane_focus`, `a_chat_answer_is_corrected_in_two_keys_from_its_row`, `other_on_mobile_says_answer_in_chat`; the key map snapshot;
    - M2, `tests/e2e/src/surface.spec.ts`: `question_notes_pin_under_intent_on_every_view`, `a_chat_answer_chip_sits_beside_its_resting_items`; `exactly_five_concepts` still passes;
    - M2, `crates/planroom/tests/s11_remote.rs::a_signed_owner_row_may_answer_a_question`;
    - M2, `crates/planroom/tests/readback.rs`: `the_readback_is_one_line_from_the_intent_and_its_non_goals`, `a_change_to_the_readback_deals_a_new_card_and_supersedes_the_old`, `yes_accepts_exactly_the_items_the_card_read`, `a_later_change_unconfirms_the_readback`, `a_note_on_not_quite_reaches_not_this`, `an_agent_cannot_confirm_a_readback`, `later_leaves_the_plan_going`; `crates/planroom/tests/packs.rs::a_readback_part_names_a_prop_its_stage_writes`;
    - M2, `crates/kernel/tests/lint.rs`: `an_open_high_stakes_question_with_live_resting_items_is_flagged`, `a_dropped_or_answered_question_is_not_flagged`, `a_high_stakes_chat_answer_stays_flagged_until_confirmed`, `referenced_passes_a_subject_a_by_node_names`;
    - M2, `crates/planroom/tests/questions.rs::a_poc_answer_reads_as_from_poc_and_confirms_with_one_key`; `crates/kernel/tests/plan_questions.rs::a_question_a_stop_names_takes_the_sticky_slot_first`;
    - M2, `crates/planroom/tests/intent.rs`: `not_this_lists_each_kind_of_correction_newest_first`, `a_reject_why_is_masked_on_every_output`, `items_resting_on_a_corrected_answer_stay_in_redraft_until_reproposed`, `ask_stage_cuts_not_this_before_questions_to_keep_its_budget`; `crates/planroom/tests/budgets.rs::the_stage_tail_with_not_this_keeps_the_prefix_hit_rate`;
    - M2, `crates/planroom/tests/evals.rs`: `intent_capture_recomputes_from_replays`, `an_ambiguity_counts_as_covered_only_by_a_question_naming_its_terms`, `a_question_about_what_the_request_settles_is_a_false_ask`;
    - M3, `crates/planroom/tests/diagram.rs`: `a_question_names_a_diagram_topic_of_its_project`, `an_unknown_diagram_topic_is_invalid_argument`;
    - the goldens `ask-stage.json` and `focus.json` with questions, and `intent-answer.json`, through `cargo xtask gen --golden`;
    - `feature_list.json` gains a tier C entry, "Getting intent right" (R5, R13), with these tests.
  - **Known gaps:**
    - the quote check is a guardrail: it proves the words were yours, not that they answer this question;
    - the mod forgets your recent prompts on a reload, so a chat answer to a prompt from before the reload is refused, and the agent proposes it as a ghost;
    - the canvas and the Artifact have no chat, so they have no chat answers;
    - until M2's reconciler marks items done, a chat answer shows until its resting items are rejected or re-proposed away;
    - the rank's weights aren't measured by the evals; the evals measure what the intent stage asks, not how it ranks;
    - only the intent stage declares a read-back;
    - a change counts toward `not_this` only when an agent wrote the value it replaced;
    - meaning stays with M2's semantic lint (`plan_item_vagueness`, `next_action_concrete`); the evals score Claude's drafts, not Jev.
  - **Users:**
    - You get a full first draft at once, built on stated guesses. You see them as questions, the top one on a line in the pane and the top two pinned on the canvas.
    - Two keys keep a guess, or open its card to choose another answer. Stop whenever you like: every unanswered guess stands as drafted.
    - Answer in chat, and the question resolves at once. It shows as answered in chat beside the work it touches until that work is done, and its row key and `n` correct it. A high-stakes one stays on lint's list until you confirm it.
    - After the intent stage, one line reads back what you asked for, and `1` confirms it.
    - What you reject or change, and why, shapes the next stage's draft.
- **Sources (checked 2026-10-29):** `packs/plan/pack.toml`; `packs/plan/README.md`; `crates/kernel/src/{plan, lint, next}.rs`; `crates/contract/src/{plan, pack, view, card, requests}.rs`; `packages/ui/src/{keys, rows}.ts`; `packages/mod/src/{layout, guard}.ts`; `packages/mod/vendor/claude-code.d.ts` (`PromptSubmitInput`, `PromptOrigin`); DR-4, DR-7, DR-23, DR-50, DR-54, DR-63, DR-64, DR-73, DR-75, DR-105, DR-106, DR-111, DR-112; [Layers, overrides and ghosts](architecture.md#layers-overrides-and-ghosts); [Planning](architecture.md#planning-stages-ghosts-and-lint-r5); [Pane](architecture.md#pane); [The canvas](architecture.md#the-canvas); [Evals and thresholds](architecture.md#evals-and-thresholds-s7); [Milestones](architecture.md#milestones); contracts.md:434-447.

## DR-117: Context packs: the map as a ranked, budgeted reading list for a plan item or a block, built into every hand-off

- **Status:** accepted (decided on the owner's request of 2026-10-29). M2 builds most of it, M3 to M5 the rest; M1 builds none of it.
- **Starting point:** extends R8's change spec, whose `context` field the pack now fills (architecture.md:1300; contracts.md:1140); R12's curation (DR-27), which gains two miners; and DR-112's staleness by fingerprint, which the pack reuses. Builds on DR-115 (intended targets, and workspaces with no repos) and DR-116 (questions, their statuses and their score). DR-118 starts the hand-offs whose specs the pack fills, and DR-120 declares a section through `[[context]]`. Keeps DR-15 (a fixed set of shapes, and the two-pack bar for a new one), DR-52, DR-64 and DR-75 (no change to who may do what), the rule that tool results carry pointers, never repo text (architecture.md:1162, :1600), and R5 (ghosts never count). Amends DR-4 with one rule: a proposal an engine miner writes is a ghost whoever runs it, since the engine guessed it. Amends DR-27: curation can also propose a workspace edit, not only a repo patch.
- **Origin:** the owner, 2026-10-29: "I like the context pack idea. How could we bring it to planroom?" On delivery: "I expect to work in the plan room, ratify the plan and walk away basically."
- **Context:**
  - The map already knows what an agent needs for a piece of work: a plan item's parents (packs/plan/pack.toml:4-8: every item is a `kernel.item` under its parent), its `kernel.touches` targets, the evidence lines behind each block and edge (`crates/contract/src/model.rs:210-234`), the edges' `via` nodes such as endpoints (`:259-272`), and the intended layer's rules (architecture.md:343).
  - Nothing turns that into a reading list. The change spec's `context` holds excerpts within 4,000 tokens, including each target repo's `AGENTS.md` and `CLAUDE.md`, fenced and untrusted, but no rule says which excerpts (architecture.md:1300; contracts.md:1140). The spec isn't built yet: hand-offs are M2.
  - Evidence is a single line (`Evidence::Line`), not a range. Tool results carry pointers and at most 200 characters of a line (architecture.md:1162, :1600).
  - `ask` returns `AskResult`: a question, a summary and items of ID, name, why and handles (`crates/contract/src/requests.rs:194-215`). It has no room for ranges, sections or a hash.
  - The kernel serves five of its twelve shapes (`crates/kernel/src/query/mod.rs:1-8`; `crates/contract/src/pack.rs:322-335`). None walks a node's parents. DR-15 lets a new shape in only with two packs that use it.
  - Curation mines only what Planroom observed of a run: the Bash calls in the mod's `observed.json` and the branch's commits (architecture.md:1360; DR-75). It doesn't see what an implementer read.
  - Ghosts are excluded from questions, lint and reconciliation until a person accepts them (architecture.md:347).
- **Decision:**
  - **What a context pack is.** The map turned into a reading list for one subject: a plan item, or a block at any level of the ladder. It is derived on read and stored nowhere, except as a copy inside a change spec.
    - A project isn't a subject: its pack would be the whole plan. A ghost isn't one either: accept it first. Both are `invalid_argument`, with `argument.context_subject` or `argument.context_ghost`.
    - The pack reads the model as questions do: ghosts never enter it (R5). Questions enter by DR-116's statuses. An `open` question is marked `unconfirmed`, since the plan rests on its assumption; a `from_chat` or `from_poc` one carries its status as its mark. None of them is a ghost, so the implementer sees them and knows what the plan rests on. `answered` and `dropped` questions stay out.
  - **One leaf of its own, `context ID`.** It isn't an `ask` question. Its reply needs sections, ranges, costs and a hash, and `ask` keeps one result type (DR-54; DR-112 kept `AskResult` the same way).

    | Leaf | Idea | Class | Undo | Tool | Budget |
    |---|---|---|---|---|---|
    | `context ID [--version V] [--budget TOKENS] [--known HASH]` | Lenses | read | query | `room_query` | 1,500 |

    It is actor `any`, `needs_base` false and `spends` false. `--version` pins a plan item's plan version (default: the project's head) and is `invalid_argument` on a block. `--budget` is the reading budget, below.
  - **Sections, in reading order.** The engine serves five; packs declare the rest.

    | Section | Source | What it holds |
    |---|---|---|
    | `intent` | engine | For a plan item: its parents from the intent down, the item, its direct children and the project's non-goals, as one line each. For a block: the open, accepted items whose `kernel.touches` reach it, each with its intent's line. |
    | `rules` | engine | The intended layer's required edges and boundary rules that name the subject or a target, in the change spec's `SpecRule` shape. |
    | `questions` | engine | The questions whose `rests_on` reaches the subject or a parent of it (for a block: an item that touches it), with status `open`, `from_chat` or `from_poc` (DR-116): the question, its assumption or answer, its stakes and its mark. |
    | `instructions` | engine, from pack data | The repo instruction files that apply to each target, as pointers. |
    | `targets` | code pack | Each target's own evidence, as ranges, and the evidence of the edges the subject's rules name. |
    | `interfaces` | code pack | The `runtime` neighbours of each target, depth 1, both ways: the evidence of the joining edges and of their `via` nodes (endpoints, schemas, queues). |
    | pack sections | other packs | Whatever a pack declares, in pack load order: the ontology pack's `definitions`, for one. |
    | `diagrams` | engine | Accepted diagrams of the subject, its targets and its project (DR-112), as `diagram show` handles with their hashes; a stale one carries `fresh: false`. |

  - **The intent chain needs no new shape.** The chain is the kernel's own plan structure (`kernel.item` under `kernel.project`), which the kernel already walks for lint. So the engine serves it, as it serves the built-in `stage` and `lint` (DR-15's round-3 amendment). A pack shape that walks parents would have one user, and DR-15 asks for two.
  - **Packs declare sections as data.** A `[[context]]` table in a pack has:
    - `id` and `label` (the heading the reply and the canvas show);
    - `from`: `subject` or `targets`, the start set;
    - `steps`: zero to three query shape steps, checked as a question's are (architecture.md:889); zero means the start set itself;
    - `cite`: `nodes` (the result nodes' own evidence), `edges` (the evidence of the edges joining the start set to the result, and of their `via_ref` nodes) or `props` (the named props of the result nodes, inline, each cut to 200 characters);
    - `props`: the prop names, with `cite = "props"` only;
    - `max_entries`: 1 to 20, default 10.
    - An `id` equal to an engine section, or one another loaded pack declares, is `pack_invalid` (`context_reserved`, `context_duplicate`).
    - The code pack declares `targets` and `interfaces`, and its `instruction_files = ["AGENTS.md", "CLAUDE.md"]`. A pack may add file names; the loaded packs' lists merge. So the kernel names no file and no lens.
  - **Pointers, not text.** A pointer is `{ path, lines: [first, last], commit, cost, why, source }`.
    - `path` is `<repo>/<repo-relative path>`, as in evidence. The reply's `roots` maps each repo to its checkout, so an agent can open it. A root is an absolute path, so `roots`' values are marked `x-redact`, as DR-119 marks `BoardRow.root`; its keys, the namespaces, aren't.
    - A range is the evidence line from 5 lines before to 20 after, clamped to the file. Ranges in one file merge when 10 lines or fewer apart, and a merged range stops at 120 lines.
    - `cost` is `ceil(bytes / 4)` of the range at `commit`, from the line index the scan verified evidence against. No file is read for the reply.
    - `why` is one engine-authored line from en.toml, such as "the `http` edge from worker to orders (POST /orders/{id}/events)". It can hold a name or a `via`, so it is marked `x-redact`.
    - `source` is `evidence`, `instructions` or `curated`.
    - An intended target, one not built yet, cites its nearest built ancestor's evidence and is marked `intended`. A resolved one reads as its actual block (DR-115) and cites that block's evidence. In a workspace with no repos, the pack holds `intent`, `rules`, `questions` and `diagrams`, and no pointers.
    - Instruction files are cited by their headed sections: the repo root's file, then each one in a directory above a target's path, nearest last. The scan indexes each instruction file's ATX headings (`#` lines outside code fences) in the line index, so no file is read for the reply. A file is cited by its first section, and by each section whose heading names a target's repo-relative path, its directory or its name, each section at most 120 lines; a file with no heading is one pointer of at most 400 lines. A `CLAUDE.md` that only imports `AGENTS.md` (`@AGENTS.md`) is cited once, as `AGENTS.md`. From M4, when the artifacts pack is loaded, a section is also cited when its text, not only its heading, names a target.
    - Plan text (the intent, rules and questions lines) is inline, each line cut to 160 characters, marked `x-redact` and passed through `display::sanitize`. Repo text is never inline in the reply. The tool result is marked `untrusted: true`, as every tool result is.
  - **Ranked and budgeted.** Every entry has a tier. When the pointers' summed `cost` passes the reading budget, the cut drops whole entries from the lowest tier first, and within a tier from the end of its section.

    | Tier | Entries | Cut |
    |---|---|---|
    | 1 | `intent`, `rules` | never |
    | 2 | `questions`; `instructions`; the first 5 `targets` pointers | fourth |
    | 3 | the other `targets` pointers, and curated includes | third |
    | 4 | `interfaces`, `diagrams` | second |
    | 5 | pack sections | first |

    - Within a section the order is fixed: `questions` by DR-116's score, then ID; `targets` in the item's touch order, then path and line; `interfaces` and pack sections by lifted-edge count, then ID; `instructions` root first.
    - The reading budget defaults to 6,000 tokens, and `--budget` takes 1,000 to 24,000. Tier 1 is inline text, so it counts against the reply, not the reading budget. It holds at most 15 lines, about 600 tokens: past that, non-goals and then rules beyond the first 10 are counted in `omitted`, never cut by the reading budget.
    - The reply keeps to the leaf's 1,500 tokens by the same cut, with the `truncated` warning (DR-86). Each section counts what it dropped in `omitted`. Nothing pages: the pack is a list to read in order, and its tail is what to read last.
  - **Hashed, and stale when a cited node changes (DR-112's mechanics).**
    - `fingerprint` is DR-112's: each cited node's kind, level, parent and name, and its evidence's paths and line texts, line numbers left out. The pack's also covers each instruction file's blob ID at the scanned commit. M2 builds this function in the kernel, and DR-112's M3 slice reuses it.
    - `hash` is the sha256 of the redacted entries and the fingerprint, computed whether or not `PLANROOM_RAW` is set. With `--known HASH` equal to it, the reply is `unchanged: true` with no sections. An agent pays for a pack only when it changed.
  - **Delivered to implementers automatically.** Building a pack is a read, so delivery needs no new authority. The person approved the hand-off, by a `handoff start` press or by the ratification that starts it (DR-118); the pack only fills its spec.
    - `handoff start` builds the subject item's pack at the spec's base commit. Entries become excerpts in `context`, in cut order, while they fit the field's 4,000 tokens after what the hand-off loop puts first (a retry's failure excerpt, DR-118). An excerpt that doesn't fit is never cut in half: it goes to the spec's new `reading` list as a pointer, up to the 6,000-token reading budget. Excerpts stay fenced, marked untrusted and redacted, as today.
    - A range is read at the base commit. When the scanned commit differs, each range is re-anchored by its evidence line's text, searching 50 lines either way, nearest first. A range that can't be re-anchored goes to `reading` as a pointer marked `anchored: false`.
    - The spec keeps `context_hash` and `context_fingerprint`. `handoff collect` takes the fingerprint again over the current model. On any difference it warns `context_stale`, naming the changed cited IDs (at most 10, with a count of the rest), and the review card shows the line.
    - A stale pack doesn't stop a running hand-off. The implementer worked from the commit the pack was built at, and the spec's rules and checks catch a real conflict. A hand-off that hasn't started builds its pack when it starts, so work sequenced behind another hand-off or a proof of concept reads the result.
  - **On demand in chat, secondarily.** The `planroom` skill gains two lines: before working on a plan item or a block, run `context ID` and read what it lists, in order, with your own Read; pass `--known` with the hash you hold. The chat agent's Read stays under the mod's guard, and no pointer can name a path in a store, `.planroom/` or the data root, since every pointer is a workspace repo file.
  - **Curation learns from what implementers read.**
    - The mod writes each Read call it saw the implementer make to `observed.json`, beside its Bash calls: the path inside its clone, as `<repo>/<path>`, and the line range, or the whole file.
    - `handoff report` writes two new fields on the run: `context`, the entries the spec gave (path, lines, section, excerpt or pointer), and `reads`, the observed reads. Only Planroom writes them; the run's agent-written steps stay `claimed` and are never mined (DR-75). Runs an agent imports teach nothing (DR-52).
    - Two new miners run in the `curate` duty, deterministic, with no model:

      | Miner | Rule | Proposal |
      |---|---|---|
      | `context_missing` | a range read, or a file edited, outside what the spec gave, in 3 or more runs whose subjects touch the same block | include that range in every pack whose targets hold that block |
      | `context_unread` | a pointer given, not as an excerpt, in 5 or more runs and read in none | skip that range in that block's packs |

    - A proposal is an `edit set` of the block's `context` field: `{ include: [{ path, lines, why }], skip: [{ path, lines }] }`, kept in the fold beside the node and read only by the pack builder. It is an override, so a rescan keeps it.
    - The `curate` duty writes it as a ghost, by the rule this record adds to DR-4: a proposal an engine miner writes is a ghost whoever runs it. DR-120's concept miner follows the same rule. One batch per `curate` run, listed in Decisions: "Context hints from 4 runs: give 2 ranges, skip 1". It counts only after a person accepts it. Reject hides it, and `edit undo` reverses an accepted one. An agent can't accept it.
    - Accepting checks each included path against the read confinement (DR-52): a path outside the workspace's repos, or a credential path, is refused. A rejected hint's cluster isn't proposed again until 3 more runs show it.
    - An accepted include enters `targets` or `interfaces`, wherever its block appears, marked `curated`. A skip removes the pointer from that block's packs.
  - **What a person sees.**
    - On the canvas, a plan item's detail on the plan board (Intent) shows one line: "Reading for this item: 14 ranges in 6 files, about 5,200 tokens; 2 open questions." Opening it lists the sections, each pointer with its evidence popover. No new view or concept.
    - The hand-off review card adds one line: "Given 9 ranges; read 7 of them, and 2 files outside them." It adds the `context_stale` line when there is one.
    - `/room context ID` draws the summary line and the handles, within `/room`'s 300 tokens. No new pane section or key.
  - **Why these numbers.** 4,000 tokens is the spec's existing excerpt budget. 6,000 for reading caps what an implementer reads before it starts, excerpts included, at 10,000 tokens, a small part of its window; 24,000 bounds an explicit `--budget`. The window of 5 before and 20 after an evidence line holds a call with its function's opening. The 3-run rule is curation's own (architecture.md:1364), and 5 for skips is stricter because a skip removes reading.
- **Alternatives rejected:**
  - **`ask context ID` returning `AskResult`:** its items have no ranges, costs, sections or hash. Widening `AskResult` for one question would change every question's result.
  - **A new `ancestors` shape for the intent chain:** one pack would use it, short of DR-15's two, and the chain is the kernel's own plan structure.
  - **The whole pack as excerpts for the chat agent:** repo text would enter tool results, against architecture.md:1600, and an agent would pay for text it doesn't read.
  - **Pointers only in the spec:** the implementer would spend turns reading what the spec can carry. The spec's 4,000-token excerpt field already exists for this.
  - **Storing packs, or pack events:** a pack is a view of the model. Stored, it would be a second model that goes stale. The spec keeps a copy only because the implementer must read exactly what it was given.
  - **Curated hints as `CLAUDE.md` lines:** they would change the repo for every agent, not just Planroom's packs, and need a merge for each. Gotchas that belong there stay with curation's existing miners.
  - **Learning by a model's ranking:** it would cost money on every pack and be hard to check. The counts above are deterministic and testable offline.
  - **Stopping a hand-off when its pack goes stale:** every merge into a shared branch would stop neighbouring work. DR-118 owns the stop conditions, and a stale pack isn't one.
  - **A pack for a project:** it would be the whole plan; each item has its own.
- **Consequences:**
  - **Contracts:**
    - `Command::Context` and `ContextReq { id, version, budget, known }`;
    - `ContextPack { subject, version, roots, sections, reading: { budget, used, omitted }, hash, unchanged, empty }` with `roots`' values marked `x-redact`, `ContextSection { id, label, entries, omitted }`, `ContextEntry { tier, node, text, pointer, marks, handles }` with `text` marked `x-redact`, `ContextPointer { path, lines, commit, cost, why, source, anchored }` with `why` marked, and `ContextMark` (`unconfirmed`, `from_chat`, `from_poc`, `intended`, `curated`, `stale`);
    - `ContextDecl` and `PackFile.context`, and `PackFile.instruction_files`; the `[pack]` reasons `context_reserved`, `context_duplicate` and `context_cite`;
    - `ChangeSpec.reading`, `ChangeSpec.context_hash` and `ChangeSpec.context_fingerprint`, all additive in `planroom.change-spec/v1`; `ContextExcerpt` gains `section`;
    - `ImplementerRun.context` and `ImplementerRun.reads`, optional and additive, like `denied` and `claimed`, so `fixture_format_is_unchanged` still holds;
    - the `context` override field and `ContextHints`; the `context_stale` warning with `{ key, changed, more }`; `argument.context_subject` and `argument.context_ghost`; en.toml's `[context]`.
    - All additive: `schema_version` stays 1. A reader that doesn't know the `context` override field ignores it.
  - **Milestones:**
    - M1: none. M1 has no hand-off to deliver a pack to, and the owner's path ("ratify the plan and walk away") needs M2's hand-offs before a pack does anything.
    - M2 grows by 18.5 h:
      - the `context` leaf: contract 1 h, the kernel's sections, ranges, cut, fingerprint and hash 3 h, the app's line-index costs, chain, rules, questions and instruction files 2.5 h, instruction files' headings in the line index and their sections 1 h, `[[context]]` in the loader and the code pack's sections 1.5 h, `/room context` and the skill lines 0.5 h, tests, goldens and docs 1.5 h (11 h);
      - ‖ the spec's context: excerpts and `reading` from the pack, re-anchoring at the base commit, `context_hash` and the fingerprint, `context_stale` at `collect`, and the review card's line, after the hand-off slice (4 h);
      - the observed reads in `observed.json`, and the run's `context` and `reads` at `handoff report` (1.5 h);
      - the plan board's reading line and list, with the canvas slice (2 h).
    - M3 grows by 1 h: the `diagrams` section, with the drawn diagrams slice.
    - M4 grows by 1 h: instruction-file sections cited by their text as well as their headings, with the artifacts pack slice.
    - M5 grows by 5 h: the two miners, the `context` override field in the fold, the ghost batch in Decisions, and the builder reading accepted hints, with the `curate` slice.
    - The total grows by 25.5 h.
  - **Tests** (all offline in CI, on the fixture, with the `patch` implementer and replays; none needs a model):
    - `crates/kernel/tests/context.rs` (M2): `the_chain_runs_from_the_intent_down_to_the_item_and_its_children`, `a_block_lists_the_open_items_that_touch_it`, `ranges_merge_within_ten_lines_and_stop_at_120`, `the_cut_drops_whole_entries_from_the_lowest_tier_first`, `intent_and_rules_are_never_cut`, `ghosts_never_enter_a_pack`, `the_hash_changes_with_a_cited_line_text_and_not_with_seq_or_a_line_shift`, `the_order_is_deterministic`;
    - `crates/planroom/tests/context.rs` (M2): `context_on_a_next_action_lists_its_chain_rules_targets_and_interfaces` (the S4 item `worker-status-via-api`), `context_on_a_block_lists_its_evidence_and_its_neighbours_interfaces`, `a_project_or_a_ghost_is_not_a_subject`, `a_known_hash_returns_unchanged`, `the_reply_holds_pointers_and_no_repo_text` (a planted secret and a planted "ignore previous instructions" line appear in no reply field), `the_reading_budget_bounds_the_summed_cost`, `an_intended_target_cites_its_nearest_built_ancestor`, `a_workspace_with_no_repos_gets_intent_rules_and_questions`, `instruction_files_come_from_pack_data_and_an_import_only_claude_md_is_cited_once`, `instruction_files_are_cited_by_their_first_section_and_sections_whose_heading_names_a_target`, `roots_are_masked_and_namespaces_are_not`, `question_marks_follow_their_status`, `context_output_is_redacted` (the hash is equal across two runs and under `PLANROOM_RAW=1`), `context_requests_round_trip_through_the_schema`;
    - `crates/planroom/tests/packs.rs` (M2): `a_context_section_with_a_reserved_id_is_pack_invalid`, `two_packs_declaring_one_context_id_are_pack_invalid`, `a_context_sections_steps_are_checked_like_a_questions`;
    - `crates/planroom/tests/handoff_context.rs` (M2): `the_spec_context_is_filled_from_the_pack_within_4000_tokens`, `an_excerpt_that_does_not_fit_becomes_a_reading_pointer`, `excerpts_are_fenced_untrusted_and_redacted`, `a_range_is_re_anchored_at_the_base_commit_by_its_line_text`, `collect_warns_context_stale_naming_the_changed_ids`, `a_handoff_builds_its_pack_when_it_starts`, `report_records_the_context_given_and_the_reads_observed` (the testkit writes the `observed.json` the mod would);
    - `packages/mod/src/agents.test.ts::observed_reads_are_written_before_report` and `packages/mod/src/room.test.ts::room_context_keeps_text_within_budget` (M2);
    - `tests/e2e/src/plan.spec.ts::a_plan_items_reading_list_opens_with_evidence_popovers` (M2);
    - `crates/planroom/tests/diagram.rs::a_context_pack_lists_accepted_diagrams_and_marks_stale_ones` (M3); `crates/planroom/tests/s05_artifacts.rs::instruction_sections_whose_text_names_a_target_are_cited` (M4);
    - `crates/planroom/tests/s12_curation.rs` (M5): `a_range_read_in_three_runs_and_never_given_is_proposed_as_a_ghost_hint`, `a_pointer_unread_in_five_runs_is_proposed_for_skip`, `an_accepted_hint_enters_the_next_pack_and_undo_removes_it`, `agent_imported_runs_teach_no_hint`, `an_agent_cannot_accept_a_hint`, `a_hint_outside_the_workspace_is_refused_at_accept`, `a_rejected_hint_waits_for_three_more_runs`;
    - the goldens `context-item.json` and `context-block.json`, through `cargo xtask gen --golden`; `budgets.json` gains `tool_results["context-item"]` and `["context-block"]`, new keys, not raises (DR-89), gated by `tools.test.ts::tool_results_within_budget`; the leaf's synopsis line stays inside `surface.tools`' 900 tokens, and a raise there needs its own record;
    - a new tier C entry in `feature_list.json`, "Context packs" (R8, R12), added by M2's change with these tests.
  - **Known gaps:**
    - the fingerprint covers each cited line, not the window around it, so a change inside a range that leaves its cited line alone doesn't make the pack stale;
    - a chat agent's reads aren't observed or mined; only implementer runs teach curation;
    - a chat agent's Read reads the working tree, which can differ from the scanned commit a pointer names;
    - until M4, an instruction file's section is cited only when its heading names a target, so a section that names one only in its text is left out; a file with no heading is cited whole, up to 400 lines;
    - `tokens_est` is `bytes / 4`, so `cost` is an estimate, as every budget is.
  - **Users:** you plan in the plan room and ratify; each hand-off's implementer gets exactly what the map says it needs, and you see on its review card what it read beyond that. An agent in chat can ask for any item's or block's reading list and pay for it only when it changed. Over time, Decisions offers you hints from what implementers needed, and nothing changes until you accept one.
- **Sources (checked 2026-10-29):** architecture.md:343, :347, :870-926, :1159-1170, :1296-1311, :1358-1371, :1590-1600, :1761-1860; contracts.md:1122-1183; decisions.md DR-4, DR-15, DR-27, DR-52, DR-64, DR-75, DR-111, DR-112; `crates/contract/src/{model.rs, pack.rs, requests.rs, event.rs, command.rs}`; `crates/kernel/src/query/mod.rs`; `packs/plan/pack.toml`; KICKOFF.md:99-102 (R12).

## DR-118: Ratify a plan and walk away: one signed approval of a bounded batch of hand-offs, an integration branch per repo, and slice POCs first

- **Status:** accepted (decided on the owner's request of 2026-10-29). M2 builds all of it; M1 is unchanged.
- **Starting point:** extends DR-23 and DR-106 (the plan pack's stages and lint), DR-25 (clones and the merge preflight), DR-55 (archived branches), DR-64 and DR-75 (approved commands, the sandbox, observed runs), DR-66 (report, then collect), DR-71 (the tracked branch) and DR-80 (landing by pull request). Builds on DR-115 (a hand-off may create a repo), DR-116 (questions, their statuses, score and sticky slot, and the `referenced` rule type) and DR-117 (context packs fill each spec). DR-119 adds overlap choices, take-in and room caps on top of it. Keeps DR-26 (the touch-set guard), DR-73 (a person's field wins over automatic writes), DR-77's second key for a repo-changing choice, and DR-110 (backups). Extends DR-54: the pane's Progress row shows a ratified run, with no row or key added. Amends DR-64 and DR-75 (`handoff start` is no longer only a press, and installs from a changed manifest may run sandboxed), DR-65 (no spawn is refused by a cap), DR-66 (collect always deals a review card), DR-71 (a hand-off's base branch is the tracked branch), DR-25 and DR-80 (merge and landing are per hand-off), DR-77 (a POC fail's redraft runs under the ratification's press, and under a live ratification `h` deals the run's card), and DR-23 and DR-106 (an eighth stage, `derisk`, a stage's `needed_when`, and the rule type `max_descendants`).
- **Origin:** the owner, 2026-10-29: "I expect to work in the plan room, ratify the plan and walk away basically." On de-risking: "Could we also have a concept for slice pocs so very large plans are derisked?" and "The pocs for design work would be valuable, so maybe we keep them." The owner accepted the proposal set ("Yeah I like these. Get them planned properly"), and raised no objection to the integration-branch default.
- **Context:**
  - `handoff start ITEM` is person-only, because it is the approval of one spec and its command list (architecture.md:1298; decisions.md:1413). It refuses the terminal path, so it needs a mod press or a canvas tab (decisions.md:1414). A press signs one token for one command, valid for 30 s and spent once (DR-64). So a plan of nine next actions costs nine presses, one at a time.
  - The mod runs the spec's installs outside the sandbox, in each clone at its base commit, as part of the person's start. That is safe only because the base is the person's own tracked branch, which no implementer has touched (architecture.md:467; decisions.md:1755).
  - After the implementer reports, the mod runs `handoff collect`, which deals one review card per hand-off (architecture.md:1343; decisions.md:1480). Merge is person-only and lands on the tracked branch (architecture.md:1344, :1354). A hand-off's base branch is the repo's tracked branch (architecture.md:1300; decisions.md:1648).
  - The implementer's own report is a claim. Only the Bash calls the mod observed, in `observed.json`, count as what ran (architecture.md:1308; decisions.md:1755).
  - The daily cap counts gateway and TypeSafe dollars only. Spawned agents' usage is priced and reported, and no spawn is refused by a cap (decisions.md:1450).
  - The plan pack has seven stages. The premortem rates failure stories and gives each high one a mitigation; nothing tests the riskiest guess before the work starts (packs/plan/pack.toml:97-109). Lint has five rule types (crates/contract/src/pack.rs:468-479). A field test can't test that a prop is absent (crates/kernel/src/next.rs:87-101).
  - Plan versions are `version` events, tags on `seq`, and a later version is a person's tag (architecture.md:394).
  - `HandoffState` has no state between `reported` and a person's review (contracts.md:710-711).
- **Decision:**
  - **Ratify is one signed person action over a plan version.**
    - `intent ratify [--project P] [--version LABEL] [--spend-usd N] [--hours N] [--parallel N] --digest SHA` is person-only, from a mod press or a canvas tab, never the terminal path. With no `--version`, it tags the head as the next version in the same commit.
    - `intent ratify --dry-run` is a read anyone may run. It returns the envelope below, its digest and the card. The press signs the digest, because the token covers the command's canonical arguments (DR-64). So the signature covers the exact batch the card showed.
    - **The envelope** (`planroom.ratify-envelope/v1`) is built from the version's fold, so it holds accepted items only:
      - the hand-offs: one per accepted `next_action` rated `must` or `should`, and one per POC that isn't waived. Each has its key, its plan items, its rules, its repos, the components of its touch points, its commands (the checks, `git add`, `git commit`), its installs with the sha256 of each manifest and lockfile they read, and the keys it waits for;
      - per repo: the tracked branch, its head now (the plan's base), the integration branch, and the git identity (`user.name` and `user.email`) that its merge commits carry, as git resolves it in that repo; for a repo a hand-off creates (DR-115), its directory, its first branch and the identity git resolves at the workspace root, and no head;
      - the sandboxed installs: per repo, the registry hosts that installs from a changed or new lockfile may reach (below);
      - the redraft: for each POC, one breakdown draft of its gated items if it fails, with the estimator's cost of one such draft (DR-65);
      - the caps: spend in dollars, hours, and how many hand-offs run at once;
      - the implementer (`agent`, or `patch` with its patch directory's digest);
      - the waived POCs, and the chat answers to high-stakes questions the plan builds on;
      - each overlap with another live plan and the choice made for it (DR-119).
    - `could` and `wont` items are listed on the card and left out. A person who wants one rates it `should` and ratifies again.
    - A next action with no rating, as when you stopped before the priorities stage, runs as `should`. The card counts them: "3 unrated, run as should".
    - **Whose name is on a merge commit.** A merge commit the engine makes on an integration branch (a candidate, below, or a take-in, DR-119) carries the identity the envelope names for that repo, passed as `-c user.name=… -c user.email=…`, and a `Planroom-Ratification: <event ID>` trailer. The card shows each identity, and the digest covers it. A repo where git resolves no identity gets no Ratify: the card says which repo and the line that sets one, and `intent ratify` returns `ratify_blocked` with `details.no_identity`. The engine has no identity of its own and never fabricates one (DR-115); the person whose ratification causes a merge is its committer.
    - At commit the engine rebuilds the envelope. A digest that differs is `ratify_stale`: the plan or a head moved since the card. The card deals again with the new batch.
    - The ratification is a `ratify` event, an event kind of its own beside DR-112's `diagram`. Its body is the envelope, whole. Export, backups and the stream carry it.
    - **Defaults.** Hours default to 12 and parallel runs to 3. The spend cap defaults to the estimator's figure (DR-65): each agent hand-off at the median priced usage of this workspace's last 20 agent runs, or $2 with no history, plus 50% for one retry each; one checker run per integration attempt at its own median, or $0.05; and each POC's redraft. The card shows each, and the person can change them before the press. They are command arguments, not settings, because each plan needs its own.
    - **The card says what the run needs.** It adds two lines: "Runs while a Claude Code session with Planroom stays open here; the 12 h keep counting while none is", and, for a repo whose later installs can't run sandboxed, "Later hand-offs in api/ will wait for you once one adds a lockfile".
    - **The ratify press is the confirming press for each redraft it names (amends DR-77).** DR-77 runs a paid step only on a press made while its card is face up. The ratify card names each POC's redraft and its cost, so the ratify press confirms them, as the stage picker's digit confirms a stage draft. A redraft is a gateway call: it counts against the daily cap and the ratification's spend cap, and runs only while both have room.
  - **What the ratification authorizes, and nothing more (amends DR-64 and DR-75).**
    - `handoff start KEY` runs with no press when a live ratification covers KEY. The engine builds the spec as it would for a press, then checks it against the envelope: same items, same rules, repos and touch-point components within the envelope's, commands and installs a subset of its, except installs from a changed or new lockfile, which run sandboxed within the envelope's registry hosts (below). Anything outside is `person_required`, as today.
    - The authority is the person's signed event, not the caller. So it doesn't matter which process asks: the engine starts only what the ratification already approved, when the schedule below allows it. The spec's `by` names the person who ratified, and the `handoff` event names the ratification as its cause.
    - A ratification is live until its time cap passes, the person revokes it, a newer ratification of the same project replaces it, or every hand-off it covers is closed.
    - It never extends. An item accepted after the version isn't covered, and an agent's ghost never enters a version. New work needs a new ratification.
    - **Revoke.** `intent ratify --revoke [--project P]` is person-only. Its press is the Stop choice on the run's card (below), which turns into a confirm, "Stop gift-wrap? Nothing new starts; runs in flight finish and wait", and a second key revokes (DR-77). Nothing new starts or integrates. Runs in flight finish and wait for the plan review, with every branch kept.
    - **Installs outside the sandbox only from the base's manifests.** A ratified hand-off's installs run outside the sandbox only when every manifest and lockfile they read is byte-identical, by sha256, to the envelope's (DR-75's reason, kept: no lifecycle script an implementer wrote runs unsandboxed).
    - **Installs from a changed or new lockfile run sandboxed (amends DR-75).** When a lockfile in the base differs from the envelope's, or is new, as in a repo the plan creates once its first hand-off adds one, the installs it calls for run inside the sandbox instead: the mod runs them through `planroom:checker` in the clone, its guard allowing exactly those install commands, with the sandbox's network open only to the registry hosts the envelope names for that repo. The code pack names each install command's registry hosts as data (`registry.npmjs.org` for `npm ci`), and the envelope lists, per repo, the hosts of the install commands its lockfiles call for, or for a repo the plan creates, of every install command the code pack names; the card shows them. An install that fails is a failed attempt. Where the host's sandbox can't limit the network by host, the hand-off waits for a person's `handoff start` press instead (`install_changed`), and the ratify card said so.
    - **A sandbox is required.** Where the host can't sandbox the implementer, the ratify card says so and offers no Ratify choice. Per-hand-off presses work as before.
  - **Work lands on one integration branch per repo, never the tracked branch (amends DR-25, DR-66 and DR-71).**
    - The integration branch is `planroom-plan/<project key>`, made at ratification from the tracked branch's head. It sits outside `planroom/` so no hand-off key can clash with it as a ref directory.
    - A ratified hand-off's base branch is the integration branch, and its base commit is that branch's head when it starts.
    - **A repo the plan creates.** Its integration branch is made when DR-115's start creates the repo, unborn. The first candidate there is `planroom/KEY` itself, with no merge, and integrating it moves the branch with `update-ref` from nothing. The plan merge into its unborn tracked branch is DR-115's `--ff-only` merge, behind DR-115's preflight.
    - **Collect builds a candidate instead of dealing a card.** It fetches the clone's `HEAD` into `planroom/KEY`, as today. Then it makes a verify clone at the integration head, merges `planroom/KEY` with `--no-ff`, hooks and signing off, under the envelope's identity for that repo (above), records the candidate commit, and scans it into the `proposed:KEY` snapshot. A merge conflict counts as a failed attempt.
    - **Rules on the candidate.** The hand-off's rules, and every rule of the hand-offs already integrated under this ratification, must hold on the candidate. So a later hand-off can't undo an earlier one.
    - **Checks on the candidate, by a sandboxed checker.** The mod spawns `planroom:checker` in the verify clone: Bash only, the Bash sandbox on, a fixed cheap model. Its guard allows exactly the spec's check commands in that clone and denies everything else. The mod writes `observed.json` with each command it saw and its exit code. Only those exit codes count; the checker's own words don't.
    - **Integrate.** `handoff integrate KEY` moves the integration branch to the recorded candidate commit with `git update-ref NEW OLD`, so it fails if anyone moved the branch. Nothing the checks wrote in the clone can change what lands. The hand-off moves to `integrated`.
    - The tracked branch, the working tree and the person's other refs never change during the run.
  - **The run.**
    - `handoff advance [--project P]` starts every hand-off the live ratification allows now, under the `ratify:<project>` lease, and returns the started keys for the mod to spawn. Anyone may run it; a second call joins the first. The mod runs it after every `handoff` event it hears from `watch`, and at `session.start`.
    - **Order.** POCs start first, in parallel. A hand-off waits for every POC that gates it. Hand-offs whose touch-point components overlap run one after another, `must` before `should`, then by item ID. The rest run in parallel, up to the parallel cap.
    - **One retry.** A hand-off whose rules don't hold, whose checks fail, whose merge conflicts or whose implementer gives up or times out restarts once from the current integration head. Its spec's `context` gains the failure as an untrusted excerpt: the unmet rules, or the failing check's output excerpt, redacted. It goes first and is cut to 800 tokens, and DR-117's excerpts fill the rest of the field's 4,000. A second failure stops it.
    - **Stop conditions.** Each stops only the work that depends on it:

      | Stop | What pauses |
      |---|---|
      | `poc_failed`: a POC fails after its retry | the items it gates |
      | `rules_unmet`: a hand-off fails after its retry | it, and hand-offs queued behind it on overlapping touch points |
      | `question`: a run's result names a new `high`-stakes question | the hand-offs whose items the question rests on |
      | `outside_touch_points` or `hidden_characters` at collect | it; it waits for a person's review |
      | `install_changed`: a lockfile differs from the envelope's, and the host can't sandbox its installs by host | it, until a person's press |
      | `integration_moved`: `update-ref` found the branch moved, and no take-in recorded the move (DR-119) | the whole plan |
      | `take_in_stopped`: a take-in needs judgment (DR-119 defines it) | the whole plan |
      | `spend_cap` or `time_cap` | every start not yet made |
      | `budget_exceeded` from a model call the run needs | the hand-off that needed it |

    - A stop moves the hand-off to `paused` with its reason, deals one stop card, and pins its question: DR-116's sticky slot takes a question a stop names first. Everything that doesn't depend on it keeps going.
    - **Stopping may rest on an agent's say-so; going on never does.** A new question in an implementer's result is a ghost, as any agent write is. It still pauses the hand-offs resting on it, because pausing costs nothing and loses nothing. Integration rests only on the engine's rules and the observed exit codes.
    - **The caps.** Ledger rows gain the hand-off key and the ratification's event ID. `advance` sums the ratification's rows before each start and starts nothing past the spend cap. It starts nothing after the time cap. A run in flight at the time cap is stopped where the host allows it; otherwise it finishes and isn't integrated. This amends DR-65 for a cap the person set on their own run; the daily cap keeps its meaning, and a gateway call, such as a POC's redraft, still counts against both. DR-119's room caps, on running implementers and on agent spend across workspaces, hold automatic starts too.
    - The run advances only while a Claude Code session with the mod is open in the workspace, since only the mod spawns implementers. The next session resumes it. The ratify card says so before you walk away.
    - **Where the run shows in the pane (extends DR-54 and amends DR-77; no row or key is added).** While the focused project has a live ratification, the Progress row, which already shows hand-offs running and owns `h`, shows the run instead of the stage: "run 5/9 · $4.10/$12 · paused: POC stripe-webhooks", cut to width. `h` then deals the run's card in place of the hand-off picker: its question line is that summary, its why line the time left and the first stop, and its choices are Stop, then the paused hand-offs to retry or review, then the items ready to hand off, as the picker lists them. So the fixed sections stay at DR-116's 19 rows at 34 body rows, and the list's page doesn't change.
  - **Slice POCs.**
    - **A derisk stage** follows the premortem: intent, outcome, breakdown, next actions, priorities, premortem, derisk, lint. Its method is "riskiest assumption first". It writes `poc` items.
    - **A plan that needs no POC skips it (amends DR-23).** `StageDecl` gains `needed_when`, a list of rule IDs of the same pack. `kernel::plan::next_stage` passes over a stage with `needed_when` while none of those rules has a finding, as if it were drafted. The derisk stage's is `["derisk_question_tested", "derisk_large_plan"]`, so a plan of 12 or fewer next actions with no untested high-stakes question goes from the premortem to lint, and the stage picker and the next action never offer a paid draft with nothing to do. A rule ID that the pack doesn't declare is `pack_invalid` (`stage_needed_when`).
    - **Its candidates, ranked.** `ask stage derisk` returns the project's open questions (DR-116) and its premortem risks, ranked by stakes × uncertainty × how much rests on them:
      - stakes weigh as DR-116's score weighs them;
      - uncertainty is 3 for an open question, 2 for one answered in chat, and a risk's likelihood (`low` 1, `medium` 2, `high` 3); a question a person or a passed POC answered isn't a candidate;
      - what rests on it is counted as DR-116 counts it, at least 1.
      - The premortem stage's `risk` gains `stakes` and `rests_on`, and its checklist one line asking for them.
    - **A POC item's props:**

      | Prop | Holds |
      |---|---|
      | `hypothesis` | what must be true, in one sentence |
      | `tests` | the question and risk items it tests |
      | `slice` | the thinnest end-to-end path that tests it, touching each layer once; its `kernel.touches` targets are those layers |
      | `check` | the pass/fail command, one of its spec's checks |
      | `budget_usd`, `budget_minutes` | its own caps; passing either is a fail |
      | `on_pass`, `on_fail` | what the plan does either way |
      | `gates` | the work and next-action items that wait for it, with their descendants |
      | `status` | `planned` or `waived`; the engine writes `passed` or `failed` |
      | `waiver` | why it is waived, when it is |

    - **When a plan needs POCs.** Two rules in the derisk stage's lint, each a finding until it's met:
      - `derisk_question_tested`: a `high`-stakes question with no answer a person holds or a POC wrote, and something live resting on it, needs a POC whose `tests` names it. It is DR-116's `referenced` with `unless` `answer`, `unless_held`, `live` `rests_on` and `by` the `poc` items' `tests`, so an unconfirmed chat answer doesn't waive a POC.
      - `derisk_large_plan`: a project with more than 12 `must` or `should` next actions needs at least one POC. Twelve is past two days of hand-offs, where one wrong guess wastes most of the run. It is pack data, so another pack can set its own.
    - **A waiver is a POC with `status` `waived` and its reason.** An agent's waiver is a ghost and doesn't count until a person accepts it. A waived POC runs nothing and gates nothing.
    - **Ratify refuses the full work without them.** Any finding from the derisk stage's rules makes `intent ratify` return `ratify_blocked`, with the findings, and a hint naming `intent next --stage derisk`. Other lint findings show on the card and don't block.
    - **One new rule type,** domain-free, which the kernel reads from pack data:
      - `max_descendants`: a subject with more than `max` descendants of `kinds` passing the `count` field tests, unless one descendant passes the `unless` field tests.
    - **A pass is kept (the owner's choice).** A POC whose rules hold and whose `check` exits 0 integrates like any hand-off, so later work builds on it.
    - **A pass answers what it tests.** Each question it `tests` gets its `assumption` as its `answer`, written by `system:ratify` with the integration as its cause. Its evidence is the repo, the branch, the candidate commit, the check command, its exit code and its redacted output excerpt. The question's status reads `from_poc` (DR-116), so it shows a "confirmed by POC" chip beside the items resting on it while they are in play, two keys from correcting, as DR-116 shows a chat answer. A risk it tests gains the evidence; its likelihood stays as a person set it.
    - **Unless the implementer changed the check.** When the POC's diff touches what its `check` runs (a file its command names, a file under a directory it names, or the manifest that defines the script it calls, such as `package.json` for `npm test`), the pass still integrates, but each answer is written as a ghost, not as `from_poc`. The plan card lists it under "check changed by the implementer", and the question stays open until a person accepts the answer.
    - **A fail pauses its part.** The branch moves to `refs/planroom/archive/KEY`, as a cancel's does (DR-55), so nothing is lost. The items it gates pause. Its questions pin first. If the workspace allows Claude and both the daily cap and the spend cap have room, `intent next --stage breakdown` redrafts the gated items as ghosts, with `on_fail` and the failure in the uncached tail, as "not this" (DR-116). The ratify press confirmed this redraft and its cost (above). Otherwise the redraft waits for the person's press on the stage picker.
  - **The person returns to one review (amends DR-25 and DR-80).**
    - `diff --plan P` maps the integration branch's diff from the plan's base onto the map: each hand-off's rules, each POC's hypothesis and result, the stops, the denied commands, and every `outside_touch_points` and `hidden_characters` warning, with those hunks expanded.
    - When the last hand-off integrates or stops, or the person opens the plan, one card deals: "gift-wrap: 7 of 9 integrated, 2 POCs passed, rules 14/14, +812 −120 in 31 files in 2 repos. Merge the plan, ask for changes, or keep it?" Merge takes the second, deliberate key (DR-77).
    - `handoff merge --plan P` (person only) preflights every repo as `handoff merge` does, under the `plan:<project>` lease and the step protocol. Then it runs `git merge --no-ff planroom-plan/<project>` on each tracked branch with hooks off. Every integrated hand-off moves to `merged`, `landed_by` `Plan`. A preflight conflict with the tracked branch returns `handoff_conflict` with the files.
    - `handoff revert --plan P` (person only) reverts the plan's merge commit in every repo, behind the same preflight and resume.
    - **By pull request.** The card names the integration branch. A person may push it and land it by a pull request, as for one hand-off (DR-80): the patch identity is taken over the integration branch's diff from the plan's base.
    - **Ask for changes** starts a person's hand-off on the integration branch, with the ordinary per-hand-off review. **Keep it** leaves everything as it is.
    - `handoff cancel --plan P` (person only) moves the integration branch to `refs/planroom/archive/plan/<project>/<n>` and archives each open hand-off's branch.
    - Paused hand-offs stay paused. A person can retry one (`handoff start KEY`), review it alone, or drop it.
    - **A re-ratification keeps what's done.** Ratifying a later version keeps every integrated hand-off whose items and rules are unchanged, and continues on the same integration branch.
  - **The loop gains three states.** `reported → verifying` (collect under a ratification), `verifying → integrated`, `verifying → paused`, `failed → paused` (after the retry), `paused → started` (retry), `paused → reviewing` (a person reviews it alone), `paused → dropped`, `integrated → merging` and `integrated → merged` (the plan merge, or the plan's pull request). Each is a `handoff` event, with its cause.
  - **How the two exceptions stay safe and reversible.** The owner chose two places where something counts before a person accepts it item by item:
    - **The ratification approves a bounded batch.** It is the person's signed press over a digest of every spec's rules, repos, touch points, commands and installs, the registry hosts sandboxed installs may reach, each POC's redraft and its cost, the identity on its merge commits, and the caps. The engine checks each start against it, and nothing outside it runs. The only branch it changes is the integration branch; the tracked branch changes only on the person's merge, with its second key, and `handoff revert --plan` undoes that. Revoke stops it at once; cancel archives everything.
    - **A POC pass answers its questions.** The person approved each POC's hypothesis, `check` and `on_pass` in the batch. The answer rests on the engine's rules and the exit codes the mod observed, never on an agent's report. The check itself may run code the implementer wrote, so a pass is evidence, not proof (Known gaps), and a pass whose check the implementer changed answers only as a ghost. It never sets an answer a person holds (DR-73). It shows as "confirmed by POC" with its evidence, and `edit undo` or `intent answer` reverses it. Reopening it re-pins the question; work already integrated waits for the plan review.
    - The chat answer that resolves a question at once is DR-116's. The ratify card lists such answers to high-stakes questions, from the envelope.
- **Alternatives rejected:**
  - **The mod signs each start with the person key.** The key would sign without a press, so any code in the mod's process could start anything.
  - **A long-lived delegated token.** A bearer secret whose leak is the person's authority. A ratify event holds no secret, and its authority is bounded by its envelope.
  - **Bounds alone, such as "any change in repos X and Y".** Work an agent chose would run unapproved. The envelope binds each hand-off's rules and touch points.
  - **Merging each hand-off into the tracked branch.** The owner's default is the integration branch, and the tracked branch should change only on a person's press.
  - **No integration branch, each hand-off reviewed alone on return.** Nine reviews instead of one, and nothing checks the hand-offs together.
  - **Rebasing hand-offs onto the integration branch.** It rewrites the implementer's commits, which breaks patch identity and the archive's record.
  - **Running checks through `process.run`.** It would run code an implementer wrote, unsandboxed, with the network and the person's rights.
  - **Trusting the implementer's reported checks.** They are claims (DR-75), and they ran before the merge with the other hand-offs.
  - **Installs from the integration head.** An implementer's manifest would run its lifecycle scripts outside the sandbox (DR-75).
  - **Stopping the whole plan on any stop.** The owner asked to pause only the dependent work.
  - **Throwaway POC branches.** The owner chose to keep the POCs that pass.
  - **POCs as a separate kind of run.** A second loop; the hand-off loop already brings clones, the sandbox, rules, archives and review.
  - **The POC threshold as a setting.** Pack data lets a domain pack set its own, and needs no new key.
  - **`could` items in the batch.** They would spend the cap on what the plan says it can do without.
  - **A per-hand-off drop on the plan review that reverts its integration merge.** Every later hand-off would need its checks again; a follow-up hand-off does the same through the ordinary path.
  - **A run line and a Stop key of its own in the pane.** Every key and the row budget are spent (DR-116). The Progress row already shows hand-offs running and owns `h`.
  - **A redraft after a POC fail with no named cost.** It would spend without a confirming press (DR-77). The ratify card names it, so the one press covers it.
  - **Merge commits under an engine identity, or the person's global config read at merge time.** The first fabricates an identity; the second signs with whatever config the run's environment holds, unseen. The envelope's identity is shown and signed.
  - **Every install after a manifest change waiting for a person.** A repo the plan creates has no lockfile at ratification, so every later hand-off there would wait, and walking away from a new project would not work.
- **Consequences:**
  - **Contracts:**
    - `RatifyEnvelope` (`planroom.ratify-envelope/v1`), `RatifiedHandoff`, `RatifyBase` (with `created` for a repo a hand-off creates, `identity` `{name, email}` and `registries`), `RatifyRedraft` `{poc, est_usd}`, `RatifyCaps`; `EventBody::Ratify` and `EventKind::Ratify`, the body holding the envelope, its digest and `revokes` for a revoke;
    - `HandoffState::Verifying`, `Integrated` and `Paused`; `HandoffStep.ratify` and `HandoffStep.stop`; `StopReason` (`poc_failed`, `rules_unmet`, `question`, `outside_touch_points`, `hidden_characters`, `install_changed`, `integration_moved`, `spend_cap`, `time_cap`, `budget_exceeded`, and DR-119's `take_in_stopped`); `LandedBy::Plan`; `SpecInstall.manifests` (path and sha256) and `SpecInstall.sandboxed`; `StageDecl.needed_when` and the `[pack]` reason `stage_needed_when`; the code pack's install table gains each command's registry hosts;
    - `PlanRun`, the read of a plan's run: state (`drafting`, `ratified`, `running`, `paused`, `done`), each hand-off's state and stop, spend against the cap, the time left and the pinned question;
    - `RuleType::MaxDescendants` with its parameter table;
    - the plan pack's `derisk` stage and `poc` item type; `risk` gains `stakes` and `rests_on`; rules `poc_status`, `poc_hypothesis`, `poc_check`, `poc_gates`, `poc_budget`, `poc_slice_target`, `derisk_question_tested` and `derisk_large_plan`;
    - errors `ratify_blocked` (conflict, exit 4, retry `never`; `details {findings, no_identity, card}`; the hint names `intent next --stage derisk`, or the `git config` line for a repo with no identity) and `ratify_stale` (conflict, exit 4, retry `after_reread`; `details {digest, current}`);
    - warning `plan_paused` on `focus` and `intent run`: `details {project, keys, reason, card}`;
    - redaction marks: `ratify_blocked`'s registry entry redacts `findings`, and `plan_paused`'s nothing; `RatifiedHandoff.title`, each touch point's `why`, `SpecInstall.manifests[].path` and `RatifyBase.identity` are marked `x-redact`; `PlanRun.question` is DR-116's `QuestionRow`, already marked; the `ratify` event's body goes through the event backup's redaction as every body does; no key, digest, commit, event ID or registry host is marked;
    - catalogue leaves, with their budgets:

      | Leaf | Idea | Actor | Class | Undo | Tool | Budget |
      |---|---|---|---|---|---|---|
      | `intent ratify [--project P] [--version LABEL] [--spend-usd N] [--hours N] [--parallel N] [--overlap …] --digest SHA` | Intent | person; refuses the terminal path | handoff | `Edit` (a revoke ends it) | none | 600 |
      | `intent ratify --dry-run [same flags] [--cursor C]` | Intent | any | read | `Query` | `room_query` | 1,500, paged by hand-off; the digest covers the whole envelope, not the page |
      | `intent ratify --revoke [--project P]` | Intent | person | handoff | `Edit` | none | 300 |
      | `intent run [--project P]` | Intent | any | read | `Query` | `room_query` | 600 |
      | `handoff advance [--project P]` | Diffs | any | handoff | as `handoff collect` | `room_handoff` | 300 |
      | `handoff integrate KEY` | Diffs | any | handoff | as `handoff collect` | `room_handoff` | 300 |
      | `diff --plan P [--cursor C]` | Diffs | any | read | `Query` | `room_query` | 1,500, paged by repo |
      | `handoff merge\|revert\|cancel --plan P` | Diffs | person; `merge` and `revert` refuse the terminal path | handoff | each leaf's own | none | 600 |

    - `budgets.json` gains `tool_results["ratify-dry-run"]`, `["intent-run"]`, `["diff-plan"]`, `["handoff-advance"]` and `["handoff-integrate"]`, new keys, not raises (DR-89); the new synopsis lines of `room_query` and `room_handoff` stay inside `surface.tools`' 900 tokens with DR-116's and DR-119's, and a raise there needs its own record;
    - ledger columns `handoff` and `ratify`, additions under DR-78's rule;
    - `plugin/agents/checker.md`: `model: haiku`, `effort: low`, tools `Bash`, the Bash sandbox on.
    - `schema_version` stays 1: every addition is additive, and no release exists (DR-111's precedent).
  - **Milestones:** M1 gains nothing; every rule here acts on hand-offs, the model calls' consent and caps, or the plan board, which M2 builds. M2 grows by 62 h:

    | Slice | Milestone | h |
    |---|---|---|
    | The derisk stage, the `poc` item type and its rules, `risk`'s new props, `StageDecl.needed_when` and the skipped stage, the `max_descendants` rule type and `referenced`'s `by` in `derisk_question_tested`, waivers, the ranked candidates in `ask stage derisk`, the derisk stage's recording and the premortem's recorded again through `cargo xtask live record` (about $0.03) | M2 | 5 |
    | Ratification: the envelope and its digest, `--dry-run` paged by hand-off, the identities and the redraft lines, unrated next actions as `should`, the `ratify` event through the fold, export and backups, the derisk and identity gates, `ratify_blocked` and `ratify_stale`, revoke, and `handoff start` under a ratification with its envelope check | M2 | 9.5 |
    | The ratified run: `handoff advance` and its order, the integration branch, the install gate and sandboxed installs from a changed or new lockfile, collect's candidate and verify clone, merge commits under the envelope's identity, rules on the candidate, `handoff integrate` with `update-ref`, one retry with its failure context, stop reasons, stop cards and `plan_paused`, the three new states, the caps from tagged ledger rows, and a created repo's unborn integration branch | M2 | 18 |
    | POC outcomes: POCs first and their gates, a pass kept with its `from_poc` answers and evidence, a pass whose check the implementer changed answering as ghosts, a fail archived with its part paused, its questions pinned and its redraft as ghosts, with the redraft's recording (a breakdown request with `on_fail` and the failure in its tail, about $0.015) | M2 | 6.5 |
    | The plan review and landing: `diff --plan` paged by repo, the plan card, `--plan` on `merge`, `revert` and `cancel` under `plan:<project>`, the plan landed by pull request, re-ratification's carry-over, `intent run`, the new `budgets.json` keys | M2 | 7.5 |
    | ‖ The mod: `advance` on every `handoff` event and at `session.start`, the `planroom:checker` spawn and its guard for checks and sandboxed installs with their registry hosts, priced runs tagged in the ledger, the ratify card with its caps, identities, redraft and session lines and its second key, no Ratify without a sandbox, the run on the Progress row and the run's card under `h` with its Stop confirm | M2 | 7.5 |
    | ‖ The canvas: a POC lane above the breakdown on `PlanBoard`, each POC a card from hypothesis to result with its gated items below it, run states as tints, and the plan review in `DiffTray` | M2 | 5 |
    | The walk-away CI path: a fixture plan with two POCs, one passing and one failing, and three next actions, through ratify, the run and one plan merge | M2 | 3 |

  - **Tests** (offline, strict replay, the `patch` implementer with patches the test writes under its temporary root and names with `--patches`; in Rust tests the testkit's checker stand-in writes `observed.json` as the mod does; the CI paths set `user.name` and `user.email` in each fixture repo's own config and none globally):
    - `crates/kernel/tests/lint.rs`: `referenced_finds_a_high_stakes_question_no_poc_tests`, `max_descendants_passes_with_a_poc_or_a_waiver`.
    - `crates/kernel/tests/plan_stages.rs`: `a_small_plan_skips_derisk_to_lint`, `a_stage_whose_needed_when_rules_find_something_is_next`.
    - `crates/planroom/tests/ratify.rs`: `ratify_needs_a_person_press`, `ratify_refuses_the_terminal_path`, `unrated_next_actions_run_as_should_and_the_card_says_so`, `a_repo_with_no_git_identity_blocks_ratify`, `the_dry_run_pages_by_handoff_and_its_digest_covers_every_page`, `the_envelope_names_each_pocs_redraft_and_its_cost`, `ratify_refuses_a_plan_with_an_untested_high_stakes_question`, `ratify_refuses_a_large_plan_without_a_poc_or_waiver`, `an_accepted_waiver_lets_ratify_through`, `an_agents_waiver_stays_a_ghost_and_blocks`, `could_items_are_listed_and_left_out`, `a_changed_plan_makes_the_digest_stale`, `a_ratified_handoff_starts_without_a_press`, `a_spec_outside_the_envelope_needs_a_person`, `an_item_accepted_after_the_version_is_not_covered`, `revoke_stops_new_starts_and_integrations`.
    - `crates/planroom/tests/handoff_integration.rs`: `work_lands_on_the_integration_branch_never_the_tracked_branch`, `a_handoff_integrates_only_when_its_rules_and_checks_hold`, `a_later_handoff_that_breaks_an_earlier_rule_does_not_integrate`, `what_the_checks_write_never_lands`, `overlapping_touch_points_run_one_after_another`, `a_failure_retries_once_with_its_output_then_pauses_its_dependents`, `an_outside_touch_points_change_waits_for_a_person`, `a_changed_lockfile_installs_sandboxed_without_a_press`, `a_created_repos_second_handoff_installs_sandboxed_without_a_press`, `without_host_limits_a_changed_lockfile_waits_for_a_person`, `candidate_merges_use_the_identity_the_envelope_names_with_its_trailer`, `a_moved_integration_branch_pauses_the_plan`, `the_spend_cap_stops_new_starts`, `the_time_cap_stops_new_starts`, `a_crash_mid_integration_resumes`, `the_plan_merges_with_one_person_press_after_preflight`, `a_plan_merge_reverts_in_every_repo`, `a_pull_request_of_the_integration_branch_lands_the_plan`, `cancel_archives_the_integration_branch`, `re_ratifying_keeps_unchanged_integrated_handoffs`, `a_created_repos_integration_branch_starts_unborn_and_lands_by_fast_forward`, `a_retry_puts_its_failure_first_in_the_spec_context`.
    - `crates/planroom/tests/poc.rs`: `pocs_start_first_and_gate_their_dependents`, `a_passed_poc_is_kept_and_answers_its_questions_with_evidence`, `undo_reopens_a_poc_answer_and_pins_it`, `a_poc_never_overwrites_a_persons_answer`, `a_failed_poc_pauses_only_its_gated_items`, `a_failed_poc_branch_is_archived_not_lost`, `a_failed_poc_redrafts_its_gated_items_as_ghosts` (strict replay of the redraft's recording), `a_redraft_waits_when_the_spend_cap_has_no_room`, `a_pass_whose_check_the_implementer_changed_answers_as_a_ghost`, `ask_stage_derisk_ranks_by_stakes_uncertainty_and_what_rests`.
    - `crates/planroom/tests/walk_away.rs::ratify_then_return_to_one_review`: the CI path above.
    - `crates/planroom/tests/budgets.rs`: the five new `tool_results` keys, gated by `tools.test.ts::tool_results_within_budget`.
    - `crates/kernel/tests/handoff_loop.rs::ratified_transitions_follow_the_loop_table`.
    - `crates/store/tests/ledger.rs::ratified_runs_are_tagged_and_summed`.
    - `packages/mod/src/agents.test.ts`: `advance_runs_on_each_handoff_event_and_at_session_start`, `the_checker_spawns_sandboxed_with_bash_only`.
    - `packages/mod/src/guard.test.ts`: `the_checker_runs_only_the_spec_checks_in_its_verify_clone`, `sandboxed_installs_reach_only_the_envelopes_registry_hosts`, `without_a_sandbox_the_ratify_card_offers_no_ratify`, `the_ratify_card_says_the_run_needs_an_open_session`.
    - `packages/mod/src/layout.test.ts`: `the_progress_row_shows_the_run_within_49_columns`, and `rows_fit_at_49x34` and `rows_fit_compact_at_80x24_and_100x30` unchanged with a run live; `packages/mod/plugin-tests/surfaces.test.tsx::h_deals_the_runs_card_and_stop_takes_a_second_key`; the key map snapshot unchanged.
    - `packages/canvas/src/views/poc-lane.test.ts::pocs_sit_above_the_breakdown_with_their_gated_items_below`.
    - `feature_list.json` gains a tier A entry, "Ratify and walk away" (R5, R7, R8, R10), with `walk_away.rs::ratify_then_return_to_one_review` and the `ratify.rs`, `handoff_integration.rs` and `poc.rs` tests above.
  - **Known gaps:**
    - The run advances only while a session with the mod is open. Closing it pauses the run, and the time cap keeps counting; the ratify card says both.
    - Spend is summed before each start, so runs in flight can pass the cap by up to the parallel count of runs. A run in flight at the time cap finishes where the host can't stop a spawned agent; it isn't integrated.
    - A check runs code its implementer may have written, so a POC's pass is evidence, not proof. The rules are the engine's own, a pass whose check files the implementer changed answers only as a ghost, and the plan review shows each POC's diff first. A change that defeats the check from a file it doesn't name still passes.
    - A sandboxed install's lifecycle scripts still run, inside the sandbox, with their registry reachable; they can read and write the clone.
    - Each integration costs one cheap checker run, which the default spend cap counts.
    - Until DR-119's take-in lands in M3, the integration branch doesn't take in later moves of the tracked branch, and a plan merge that conflicts returns `handoff_conflict`.
    - Nothing notifies the person away from the pane, the canvas or the Artifact when a plan pauses.
  - **Users:**
    - You plan in the room, then press Ratify. The card reads: "Ratify gift-wrap v3: 9 hand-offs, 2 POCs first, in storefront and orders. Runs npm ci, then pytest, npm test and git commit, sandboxed. Up to $12.00 and 12 h, 3 at a time, including one redraft per POC that fails. Merges as Ana Ruiz <ana@example.com>. Lands on planroom-plan/gift-wrap, never main. Runs while a Claude Code session with Planroom stays open here." A second key confirms it.
    - While you're away, the pane's Progress row reads "run 5/9 · $4.10/$12 · paused: POC stripe-webhooks". `h` opens the run's card. The canvas shows the POC lane above the breakdown, each card going from its hypothesis to its result.
    - A stop pauses only the work that depends on it, pins its question, and leaves the rest running.
    - When you return, one card shows the whole plan's result. One deliberate key merges it into main; one command reverts it.
    - Passed POCs are kept in the plan. Each question a POC answered shows "confirmed by POC", two keys from correcting.
    - Stop on the run's card, then a second key, revokes the ratification at any time, and nothing is lost.
- **Sources (checked 2026-10-29):** the owner's design session of 2026-10-29; [The change spec](architecture.md#the-change-spec-r8), [Implementers](architecture.md#implementers) and [The hand-off loop](architecture.md#the-hand-off-loop) (architecture.md:1296-1356); the reconciler's classes (:1280-1295); the guard's framing (:466-480); [Milestones](architecture.md#milestones); DR-25, DR-64, DR-65, DR-66, DR-71, DR-72, DR-75, DR-80, DR-89, DR-106 and DR-111; `packs/plan/pack.toml`; `crates/contract/src/pack.rs`; `crates/kernel/src/{lint.rs, next.rs}`; `crates/contract/src/event.rs`; [Contracts](contracts.md#change-spec).

## DR-119: One room: boards in areas, every view a link, repos and boards attached from anywhere, and plans that run side by side

- **Status:** accepted (decided on the owner's request of 2026-10-29).
- **Starting point:** keeps KICKOFF R10, "It holds several projects (plans) that share one model of the system" (KICKOFF.md:89), and architecture.md:529: projects in one workspace share facts, overrides and the intended layer. Keeps KICKOFF's "A single-writer local server per workspace: HTTP commands and queries, plus a scoped live stream" (KICKOFF.md:394) in substance: each workspace keeps one store, SQLite stays its single writer (DR-1), and its commands and stream keep a route of their own; one process now hosts every workspace's routes. Builds on DR-115 (plan-only workspaces get boards), DR-116 (the question score and `QuestionRow`) and DR-118 (ratification, integration branches, the `ratify:<project>` lease, the checker and `PlanRun`). Extends DR-29, DR-58, DR-77 and DR-101. DR-120 adds summary props to its board rows. Amends DR-29 and DR-78 (one canvas port per workspace), DR-71 (which repos a workspace holds), DR-66 and DR-101 (the tools' input and their refusal of `--workspace`), DR-77 (the `p` picker), DR-51 and DR-65 (the ledger, and a room cap on automatic starts by agent spend) and DR-25 (branch names in a shared repo). It adds `take_in_stopped` to DR-118's `StopReason`. It fills DR-118's known gap: taking later moves of the tracked branch into a plan's integration branch.
- **Origin:** the owner, 2026-10-29. On parallel plans: "Does planroom effectively support multiple parallel plans at once? I can imagine wanting to work on a bunch of things". On the room: "Ideally a single planroom for everything. But any layer/view should be bookmarkable. Just different boards for each project. In the mod, it would be whatever repo/project but with the option to attach a different board/repo as well. An abstraction and organization layer above would be nice." The owner accepted the proposals ("your recs are fine"; "Yeah I like these. Get them planned properly").
- **Context:**
  - The workspace is the unit. `workspaces.json` maps each root the engine created to its `workspace_id` and one canvas port, distinct across workspaces (architecture.md:599; `crates/planroom/src/app/data.rs:88-97`). `serve` binds that one workspace's port and never moves it (architecture.md:1423; `crates/planroom/src/server/mod.rs:116-149`, `:304-351`), so each workspace has its own origin, login and tab.
  - A workspace's repos are its root, its submodules, or its children or grandchildren that are repos (architecture.md:591). A repo elsewhere can't join, so a linker can't join a service to a library kept in another folder.
  - The mod works in the session's directory (`packages/mod/src/register.tsx:633`). Its tools refuse `--workspace` in every call (`packages/mod/src/tools.ts:103-107`; DR-101), so a session reaches one workspace.
  - The canvas keeps its view in memory: the level only (`packages/canvas/src/store.ts:57-58`). No URL names a view, so nothing can be bookmarked. A handle is an exact invocation, `{command, args, label}` (contracts.md:147), and the Artifact's snapshot already keys its precomputed views by handle (DR-77).
  - Many projects per workspace work today: project-scoped items, versions and views; claims and presence (architecture.md:385-387); safe concurrent writes. Nothing checks two plans against each other. DR-118 runs each ratified plan on its own integration branch, and leaves taking in later moves of the tracked branch to this record.
  - DR-116 ranks a plan's questions by stakes weight times what rests on them. Nothing ranks them across plans.
- **Decision:**
  - **The room: areas, boards, bookmarks.** The room is your organization layer above every workspace. It lives in the data root and never in a repo.
    - **A board is one project's plan and its map.** Every project in a listed workspace is a board, `<workspace_id>~<project slug>`. Every listed workspace also has its map board, `<workspace_id>`, which shows the map with no plan. Boards are derived from `workspaces.json` and each store's projects, so a new project is a board at once and nothing has to be kept in step. A plan-only workspace gets its boards the same way, since DR-115's `intent new` writes its `workspaces.json` entry.
    - **An area groups boards.** One level, no nesting: the breadcrumb then holds at most two lids above a board, which fits the phone's `⋯` lid, and areas can nest later as an addition. A board sits in one area or in none. A new project's board joins the area that holds its workspace's map board; a new workspace's boards sit in none, shown last as "Not in an area".
    - **You can hide a board.** A hidden board leaves the lists and stays one press away under "Hidden". Nothing about its workspace changes.
    - **A bookmark is a board and a handle.** `{id, board, handle, name, at}`. Only a read can be a bookmark: a leaf of class `read` that never spends and isn't in Setup, such as `map`, `focus`, `show`, `find`, `ask`, `diff`, `who`, `intent list` and `intent lint`. A bookmark of a write or a paid read would act when opened.
    - **A plan version is a handle too.** `map` gains `--version LABEL`: the map folded at that plan version's `seq`, whose snapshots stay while the version refers to them (architecture.md, Versions and revisions). With `--board`, a bookmark can hold a board, a level, a lens, a focus and a version.
    - **The file.** `DATA_ROOT/room.json` (`planroom.room/v1`): areas in order, each board's area or hidden mark, bookmarks, and the room's port. It follows `workspaces.json`'s rules: one lock, write and rename, `min_reader_version`, unknown fields kept (DR-78, DR-85). Its type lives in `crates/planroom`, since surfaces read the room through commands, never the file.
    - **Only a person writes the room** (`area`, `board`, `bookmark` leaves, actor `person`). It is your own organization, and an agent's guess at it would need a review loop worth less than the change. An agent gives you a link instead (`link`, a read), and you save it with one press. The token is checked as for any person write in the workspace the press comes from; a canvas tab's bearer token stands for it, as for every canvas write. Each write's undo class is `Compensate`, naming its reverse, and each reply carries the handle that undoes it.
  - **Every view is a link.**
    - **One encoding.** A view path is `/b/<board>/<command words joined by />` with each argument as a repeated `a` query parameter, in order: `/b/01JC~gift-wrap/map?a=--level&a=component&a=--lens&a=flow`. It encodes any handle exactly, with no per-leaf table. It holds IDs and flags only, never a label or a token, and is at most 2,000 bytes. `contract::room::view_path` and its parser are pure; `@planroom/ui`'s `view-path.ts` is their port, and both run one table of cases, as `display` does.
    - **Three forms of one link.** The local canvas URL is `http://127.0.0.1:PORT` plus the path. The `/room` argument is `/room go PATH` or `/room go BOOKMARK`. The Artifact anchor is `#` plus the path. `link BOARD -- COMMAND ARGS` returns all three.
    - **The login code stays apart.** It still rides in the fragment, `#code=...` (DR-29), and the view rides in the path and query. A link holds nothing secret, so a bookmark can sit in a browser or a note.
  - **One room server (amends DR-29 and DR-78).** `serve` starts or reuses the one server of your room, whatever directory it runs in.
    - **One port.** `room.json` records the room's port. The first room server takes the port of the workspace first recorded in `workspaces.json`, else a free one. So the common case, one workspace used before, keeps its origin, and with it that tab's storage and pending edits. After that the port never moves unasked: another program on it is `port_in_use`, and `serve --port N` is the deliberate move, as DR-78 decided for a workspace. New `workspaces.json` entries record the room's port, so an older engine that reads them meets the room server on the same origin and defers to it through DR-69's handshake.
    - **Routes.** A workspace's routes move under `/w/{workspace_id}`: `POST /w/{id}/api/v1/{command}`, `GET /w/{id}/api/v1/stream` and `PUT /w/{id}/api/v1/layout`. `POST /api/v1/{command}` serves only the leaves that need no workspace (`boards`, `area`, `board`, `bookmark`, `link`, `inbox`, `version`, `schema`); any other is `invalid_argument` naming its `/w/` route. `GET /api/v1/stream` is the room stream. `GET /` and `GET /b/*` serve the canvas page. `code`, `session` and `drain` are unchanged. A workspace `workspaces.json` doesn't list is `not_found`.
    - **One login for the room.** A tab's bearer token reaches every board, since every board is yours and `open` stays person-only. Auth, `Origin`, `Host`, CSP and the limits apply to every route as before (DR-29, DR-82).
    - **Bounded work.** The server folds a workspace on its first request and keeps at most 16 folded, closing the least recently used after 10 minutes idle. A board stream polls only its workspace's `data_version`, as today. The room stream polls each listed store once a second, only while a room client is connected, and sends a `board` frame when a board's state changes. The 64-client cap counts every stream.
    - `serve.json` is written into each state directory the server serves, without the control token (DR-88).
    - `open [LINK]` (person-only, as before) logs a tab in and lands it on LINK: a view path or a bookmark ID. With none, it opens the session's board.
  - **The canvas in the room.** The room adds no sixth concept (R14).
    - **Levels.** The room is the topmost lid in the breadcrumb: room, then area, then board, then the board's levels. A bookmark is found by name in the breadcrumb's search.
    - **Intent.** The project shelf becomes the room's shelf: board cards by area, each with its run row (below).
    - **Decisions.** At room scope, the card stack holds the room's inbox (below), the top question face up.
    - **Lenses and Diffs** stay in place and apply to the board you open.
    - **The URL follows the view.** Each change of board, level, lens or focus pushes the view path, so back and forward walk your views and any view can be bookmarked from the browser. A view's Bookmark action runs `bookmark add` as your press.
    - **A new tab logs in from an open one.** A tab with no token asks the open tabs of its origin over a `BroadcastChannel` for theirs. Every same-origin page is the canvas build, under a CSP that allows only its own scripts, so this shares nothing beyond the origin that already holds the token. With no open tab, the needs-login state keeps the view, and `open LINK` lands on it.
    - Pending edits stay keyed by workspace ID (architecture.md:1528), so the edits of every board live in the one origin's storage, and any signed-in tab flushes them.
  - **The mod: the session's boards (amends DR-66, DR-77 and DR-101).**
    - **A session opens on its directory's board.** One live project: its board. Several, or none: the map board, where the board picker lists the projects.
    - **`/room attach BOARD`** adds a board from the room to this session, and `/room detach BOARD` removes it. These are typed person actions, kept in a new `$.state` atom, `boards`. Bare `/room attach` deals the room's board picker. `/room attach PATH` attaches a repo (below), or, for a path that is a workspace root, that workspace's map board.
    - **The board picker.** `p` deals `focus --picker board`: the session's own boards, then its attached ones, then "Attach a board…". A board is a key and a digit (DR-77). `focus --picker project` stays and lists this workspace's project boards.
    - **`PaneModel.board`** names the board in view and how many the session holds. The focus line shows the board's name only when the session holds more than one.
    - **The tools reach attached boards only.** `room_query` and `room_change` gain an optional `board` string. The handler maps it to `--board` itself, and refuses a board the session hasn't attached with `invalid_argument` naming those it has. `--workspace` and `--board` stay refused inside `args`. The property is a plain string, not an enum of attached boards, because the tool schemas sit in the cached prefix and an enum would change it at every attach. With more than one board, the per-turn context names them in one line, at most 60 tokens; the composed section stays byte-identical (DR-20).
    - The pane's `watch` follows the board in view, and the session's presence row is written to that board's store.
    - `/room go LINK|BOOKMARK` shows the view as `/room` text, with `e` to open it on the canvas. A bookmark on a board the session hasn't attached attaches it for the session, since the typed command is your press.
  - **A repo from anywhere (amends DR-71).**
    - `repo attach PATH` and `repo detach NAMESPACE` are person-only, from a press or a typed `/room`, never the terminal path, because an attached repo becomes readable to every agent working in the workspace (DR-75's rule for repo-changing commands, applied to what widens agents' reads).
    - The list lives in the workspace's `workspaces.json` entry, `attached: [{path, at}]`: in the data root, outside every repo, where the guard keeps agents out. A planted store or a committed config can't add a repo to a scan.
    - An attached repo is one of the workspace's repos: scans, linkers, confined reads, hand-offs and the reconciler see it. Its namespace follows DR-74 and DR-97, keyed in `meta.repos` by its canonical absolute path.
    - **Refusals.** A path that isn't a repo's root, or is already one of the workspace's repos, is `invalid_argument`. A credential path (DR-101) or a repo another user owns (DR-78) is `path_refused`, for a person too, since attaching would hand it to agents.
    - **A detached repo's edits wait.** The next scan leaves the repo out, and the fold treats its namespace as one the scan didn't cover (DR-47), so no retarget card deals and Diffs stays quiet. Attaching it again brings every edit back. A path that has gone is `repo_skipped` with reason `attached_missing`, and its last map stays.
    - **A repo two workspaces hold.** A branch Planroom makes in such a repo, a hand-off's or an integration branch, gains `-` and the first 8 characters of the workspace ID, so two workspaces' plans of one name never share a ref. The name is fixed in the spec when the branch is made, so a repo shared later keeps the names it has.
  - **Plans side by side: the overlap check.**
    - **What overlaps.** Two live plans in one workspace overlap when one's touch set or impact radius (M2's impact over via evidence) meets the other's touch set. A plan is live from its first accepted item until it lands or is cancelled.
    - **At ratification.** `intent ratify --dry-run` lists every overlap with a ratified, running or paused plan, and the ratify card shows each with its choice. `intent ratify` gains `--overlap side-by-side|after|merge`, which DR-118's envelope and digest cover:
      - **Side by side** (the default): both run; this plan lands after the other in the landing order, and take-in (below) brings the other's work in when it lands. It is the default because it blocks nothing and loses nothing.
      - **After:** this plan's hand-offs whose touch-point components meet the other plan's wait until that plan lands or is cancelled. The rest run.
      - **Merge:** this plan isn't ratified. Its live items are proposed into the other plan as one ghost batch, each carrying an alias of its old ID. Accepting the batch retracts this plan's items in the same commit, and the board reads "merged into gift-wrap". Rejecting it leaves both plans as they were.
    - **Later overlaps.** When an accepted batch grows a running plan's touch set into another's, the `plan.overlap` card deals for the new overlap only. It is deterministic, stakes `person`, with the same three choices and side by side as its default. Until you pick, the default holds.
    - **Shared blocks show both tints.** `MapNode.plans` lists each live plan whose touch set holds the block, with its tint: the project's creation order modulo 8. The canvas stripes a block two plans touch. CLI text adds a plans column when any row has two.
  - **Duplicate work.** The `plan.duplicate_work` decision kind lives in the plan pack, stakes `person`, so it always deals a card and never acts alone (DR-50).
    - **Candidates** are pairs of accepted work items or next actions in two live plans of one workspace whose `kernel.touches` targets are one block, or one inside the other. Two intended blocks of one kind whose `expect` names one repo, with one `path` inside the other's (DR-115), count as one block. At most 20 pairs per check, nearest first.
    - Jev scores whether the two aim at the same change. The check runs at ratification and after each accepted batch in a plan that has a live sibling.
    - **One card per pair, batched by cause** (DR-20): keep both (the default), keep the first plan's, or keep the second plan's. Keeping one rejects the other item through `review`, which `edit undo` reverses. Past a spend guard, both stay and the card stays pending (AGENTS.md, Money and credentials).
    - Name similarity alone would flag unrelated work on one block, so a model reads the intent; the person decides every time.
  - **Landing order and take-in (fills DR-118's known gap).**
    - **The order.** Live plans land in ratification order unless a project's `land_after` field names another plan: an ordinary `edit set` on the project node, so an agent's is a ghost and `edit undo` reverses a person's. The ratify card's side-by-side choice sets it. A cycle is `invalid_argument`.
    - **The order sequences review cards, never your hand.** A plan's unasked plan card (DR-118) deals once its run is done and every plan before it has landed, been cancelled, or been kept. Until then its run row reads "lands after gift-wrap". You may open any plan and merge it at any time; the others take it in.
    - **Take-in.** When a repo's tracked branch moves forward, whatever moved it, every live ratification whose integration branch in that repo lacks the new head takes it in. The `take_in` duty runs under the plan's `ratify:<project>` lease, in the plan's verify clone at the integration head: `git merge --no-ff` of the tracked head, hooks and signing off, under the identity the live ratification's envelope names for that repo, with its `Planroom-Ratification` trailer (DR-118's rule for merge commits). It then checks every integrated hand-off's rules on the result and runs the plan's checks through the checker, as for a candidate. When all hold, `update-ref NEW OLD` moves the integration branch. A `take_in` event records the repo, the tracked head, the integration heads before and after, and the result.
    - **It stops only for judgment.** A merge git can't finish is aborted, leaving the branch as it was. Rules or checks that fail leave the branch as it was too. Either way the plan stops with `take_in_stopped`, a `StopReason` this record adds to DR-118's, whose scope is the whole plan: nothing more integrates, runs in flight finish and wait, and one card deals: hand off a fix on the integration branch, keep the plan where it was (the default, which changes nothing), or cancel the plan.
    - **In-flight work follows.** `integrate` and take-in hold the same lease. A candidate built on a head that a recorded take-in replaced is built again at the new head; DR-118's `integration_moved` stop counts only a move no take-in recorded.
    - **Reversible.** A take-in is one merge commit on the integration branch, never on the tracked branch. `handoff revert --take-in EVENT` (person-only) archives the take-in's head at `refs/planroom/archive/plan/<project>/take-in-<seq>` and moves the branch back with `update-ref`, while no hand-off has integrated on top; after that, it is `invalid_argument` naming those hand-offs, and the plan's own cancel or "ask for changes" applies. `diff --plan` shows each take-in as "taken in from main".
    - Like the run itself, take-in advances only while a session with the mod is open in the workspace, since only the mod spawns the checker.
  - **Room caps (amends DR-51 and DR-65).**
    - **Running hand-offs.** `room.max_running_handoffs` (default 3) caps implementers running at once across every workspace. Each plan's own `--parallel` cap still applies (DR-118). Machines and appetites differ, so one fixed number serves no one (R14); 3 keeps a laptop responsive and the spend rate visible. It is a user-level key only: a workspace's file can't raise a machine-wide cap, and a key there gets `config_key_ignored`.
    - **The count** is the `running` table in `ledger.db`, one row per running implementer, `{workspace_id, handoff, project, started_at, lease_until}`, renewed every 60 s by the process that tracks the run, freed at collect, cancel or 5 minutes past its lease. A crashed holder frees its slot.
    - `handoff advance` starts nothing past either cap, and its reply carries `handoff_waiting`. Your own `handoff start` press runs past the room cap: its card names the count ("4 running; your room cap is 3"), because you dispose.
    - **Agent spend.** The daily cap spans every workspace but counts gateway and TypeSafe dollars only, and no spawn is refused by it (DR-51, DR-65). Implementer and checker usage, most of a plan's spend, is priced and counted nowhere across plans. So `room.max_daily_agent_usd` (default 50) caps the automatic starts of the whole room: `handoff advance` sums today's priced agent rows in the user-level ledger, across every workspace, implementers and checkers alike, and starts nothing once the sum reaches the cap. Each plan's own spend cap still applies (DR-118), and gateway calls stay under the daily cap. No single default fits every budget (R14); $50 is about two nine-hand-off plans at the no-history estimate, so a second plan isn't held on day one and a third waits for you. It is a user-level key only, as `max_running_handoffs` is. Your own `handoff start` press runs past it, its card naming the sum ("$52.10 of agent spend today; your room cap is $50").
    - Ledger rows gain `project`, filled by every call made for a plan, drafting included, so each plan's spend can be shown.
  - **The portfolio and one inbox.**
    - **`boards`** lists the room: areas in order, then boards not in an area by last activity, hidden boards counted, and bookmarks. Each project board's row carries its run: DR-118's `PlanRun` state (`drafting`, `ratified`, `running`, `paused`, `done`), hand-offs integrated of total, stops, spend against its cap and its drafting spend, "lands after", the top open question (DR-116's `QuestionRow`), and a handle to open it. Above them, the room's caps: implementers running against `room.max_running_handoffs`, today's agent spend against `room.max_daily_agent_usd`, and today's gateway spend against the daily cap.
    - **`inbox`** lists open questions across every listed board, ranked together by DR-116's score, then by stakes, then by the board's place in the room, then by item ID. Each row is a `QuestionRow` with its board, and its handles carry `--board`, so you answer it where you read it.
    - Reads open each store read-only and never migrate one. A board whose root is gone, whose store can't be read, or whose store a newer engine wrote stays listed, with `board_unreachable` and its reason.
  - **What counts before a person accepts it.** This record grants no agent new authority. Room writes, repo attach and detach, `land_after`, every overlap choice, every duplicate pick and every take-in reversal are a person's. The one automatic act is take-in, and it runs only under a live ratification, which already approved changing that plan's integration branch (DR-118, amending DR-64 and DR-75). It never touches a tracked branch, a working tree or a ref a person made. It moves a branch only after the rules and checks hold, with `update-ref` guarding against a race. It records the head it replaced, and one press moves it back.
- **Alternatives rejected:**
  - **One store for the whole room:** facts and trust stay per workspace, as the owner chose. A planted store could then reach every workspace, and every store would need a migration.
  - **The room in a repo:** a clone would carry your organization into other people's checkouts, and a committed room could add repos to scans.
  - **Boards as records you create:** they would drift from projects. Derived boards need no syncing.
  - **Nested areas:** deeper breadcrumbs for a handful of boards; they can come later as an addition.
  - **A port per workspace, with the room as an index page:** a login per port, bookmarks that break across origins, and not the one server the owner asked for.
  - **The view in the local URL's fragment:** the fragment carries the login code, and a path lets the server serve a direct link.
  - **A per-leaf URL scheme (`?level=component`):** a table to keep in step with every leaf in two languages. Repeated `a` parameters encode any handle exactly.
  - **The tab token in `localStorage`:** it would outlive the tab and authorize any later load of the origin. A `BroadcastChannel` shares it only between tabs alive now.
  - **Agents writing bookmarks and areas:** a review loop on your own filing system; a link in chat costs you one press.
  - **An enum of attached boards in the tool schema:** it changes the cached prefix on every attach.
  - **Attached repos in the store or the workspace config:** a planted store or a committed config could make a scan read outside the workspace.
  - **Refusing a repo two workspaces share:** a library used by two projects is common; a suffix on Planroom's branch names settles the one clash.
  - **Rebasing an integration branch when another plan lands:** it rewrites commits a person may have reviewed. A merge commit keeps them and can be moved back.
  - **A strict landing order that refuses an out-of-order merge:** the order is a default; you dispose.
  - **Duplicate detection by name similarity alone:** it would flag unrelated changes to one block.
  - **The daily cap as the room's spend cap:** it counts gateway and TypeSafe dollars only (DR-65), so walk-away runs in several plans would spend on agents with nothing across plans bounding them.
  - **A cross-workspace landing queue:** plans in two workspaces meet only in a repo both hold, and take-in reacts to any move of its tracked branch.
- **Consequences:**
  - **Contracts:**
    - types in `crates/contract/src/room.rs`: `BoardId`, `AreaId`, `BookmarkId`; `RoomView` `{summary, caps, areas, unplaced, hidden, bookmarks, empty}`, `RoomCaps`, `AreaRow`, `BoardRow` (with `run: Option<PlanRun>`, `drafting_usd`, `lands_after`, `question: Option<QuestionRow>`, `tint`, `reachable`, `handle`), `BookmarkRow`, `InboxRow` `{board, row: QuestionRow, score}`, `LinkResult` `{path, url, room_arg}`, `view_path` and `parse_view_path`; `BoardRef` and `PaneModel.board`; `PlanMark` `{project, tint}` and `MapNode.plans`; `StreamFrame::Board {board, seq, state}`; `EventKind::TakeIn` and `EventBody::TakeIn`; the picker name `board`; `EmptyReason::RoomEmpty` ("Nothing in your room yet. Map a folder of repos, or start a plan in an empty one.", next `focus`, "Show this folder");
    - redaction marks: `AreaRow.name`, `BoardRow.name`, `BookmarkRow.name` and `BoardRef.name` (a name you type can be pasted from a repo), and `BoardRow.root`, a path; never a board, area or bookmark ID, a view path or a handle;
    - catalogue leaves: `boards [--cursor C]` and `link BOARD -- COMMAND ARGS` (reads, Levels), `inbox [--cursor C]` (a read, Decisions), each within 1,500 tokens; `area add|rename|remove|move`, `board place|hide|show`, `bookmark add|rename|remove` (person, Levels, `Compensate`); `repo attach PATH` and `repo detach NAMESPACE` (person, press only, Levels, `Compensate`); `map --version LABEL`; the global `--board BOARD`, which on a read needs a store, as `--workspace` does; `open [LINK]`; `intent ratify --overlap`; `handoff revert --take-in EVENT`; `/room boards|go|attach|detach|inbox`;
    - the tools' `board` property; the `boards` atom in `$.state`;
    - warnings `board_unreachable` `{board, reason}` (`root_gone`, `store_unreadable`, `store_newer`), `handoff_waiting` `{reason, running, max, spent_usd, max_usd}` (reason `running` or `spend`), `take_in_stopped` `{repo, project, files, card}` (`files` masked); `repo_skipped`'s reason `attached_missing`; `invalid_argument`'s reasons `argument.not_a_view`, `argument.view_path_too_long`, `argument.not_a_repo_root`, `argument.already_in_workspace`, `argument.board_not_attached`, `argument.land_after_cycle`, `argument.take_in_built_on`; no new error code;
    - decision kinds `plan.overlap` (deterministic) and `plan.duplicate_work` (Jev), each with `stakes = "person"`, a default and `evals/`;
    - formats: `room.json`, `planroom.room/v1`, internal, in Persisted documents; `workspaces.json` entries gain `attached`; `ledger.db` gains the `running` table and the `project` column, additions under DR-78's rule;
    - the HTTP route table above; the room stream;
    - configuration: `[room] max_running_handoffs = 3` and `max_daily_agent_usd = 50`, user-level only, `PLANROOM_ROOM_MAX_RUNNING_HANDOFFS` and `PLANROOM_ROOM_MAX_DAILY_AGENT_USD`;
    - `StopReason::TakeInStopped`, added to DR-118's enum;
    - `schema_version` stays 1: every change is additive, except the move of a workspace's routes under `/w/`, which lands before any release (DR-85's precedent);
    - design tokens `--plan-tint-0` to `--plan-tint-7` in both themes.
  - **Layout:** `@planroom/ui` exports `viewPath` and `parseViewPath`, a gated package export, ratified with `cargo xtask gen --layout-lock` in this record's merge commit; new files inside existing areas only (`crates/planroom/src/app/room.rs`, `crates/planroom/src/server/room.rs`, `packages/canvas/src/views/room.tsx`, `packages/ui/src/view-path.ts`).
  - **Milestones.** Nothing goes into M1. M1's paths run in one workspace, and every shape this record changes, the per-workspace port among them, changes before any release. Parallel plans need ratification and integration branches, which land in M2 (DR-118), so they follow in M3, beside M3's multi-repo work.

    | Slice | Milestone | h |
    |---|---|---|
    | The room and its links: `room.json`, derived boards, areas, placing and hiding, bookmarks, `boards`, `link`, `--board`, `map --version`, the view-path encoding and its `@planroom/ui` port, `room_empty` and `board_unreachable` | M3 | 9 |
    | One room server: the `/w/` routes, the workspace-free routes, `/b/*` pages, lazy folds with the 16-workspace bound, board and room streams, the room port taken from the first workspace's, handshake, drain and `port_in_use` on it, `serve.json` per workspace, `open [LINK]` | M3 | 9 |
    | ‖ The canvas in the room: the room lid, board cards by area with run rows, the URL following the view with history, bookmark actions, the token from an open tab, plan tints and their tokens | M3 | 8 |
    | ‖ The mod's boards: `PaneModel.board`, the board picker, `/room attach`, `detach`, `go`, `boards` and `inbox`, the tools' `board`, the per-turn line, `watch` following the board | M3 | 6 |
    | ‖ A repo from anywhere: `repo attach` and `detach`, `attached` in `workspaces.json`, discovery, namespaces and confinement, detached namespaces left uncovered, the refusals, branch names in a shared repo | M3 | 6 |
    | The overlap check: touch sets and impact radius, `--overlap` on ratify, the `plan.overlap` kind and its cases, holding the after choice's hand-offs, `MapNode.plans` | M3 | 7 |
    | Merging one plan into another: the ghost batch with aliases, retraction on accept, the merged board's state | M3 | 4 |
    | ‖ Duplicate work: the `plan.duplicate_work` kind, its candidates, replayed Jev, 60 eval cases (about $0.02 live, recorded once) | M3 | 6 |
    | Landing order and take-in: `land_after`, the order of plan cards, the `take_in` duty, its checks, the stop card, the `take_in` event, rebuilt candidates, `handoff revert --take-in` | M3 | 9 |
    | ‖ Room caps and plan spend: the `running` table with leases, `room.max_running_handoffs`, `room.max_daily_agent_usd` summed over priced agent rows, `handoff_waiting` and its reasons, a person's start past either cap, the ledger's `project` | M3 | 5 |
    | The portfolio and the inbox: run rows in `boards`, drafting spend, `inbox`, the room's Decisions stack, `/room inbox` | M3 | 5 |
    | The parallel-plans CI path: two overlapping fixture plans through ratify, one landing, the other's take-in and its landing | M3 | 3 |
    | Links in the Artifact: anchors, and each bookmarked view of the shared workspace precomputed in the snapshot within its 1 MB, the rest read requests (DR-77) | M5 (remote) | 3 |
    | Docs: the room's concept page and the guide to organizing boards | M5 (docs) | 2 |

    M3 grows by 77 h and M5 by 5 h. The overlap, take-in, caps and portfolio slices need DR-118's M2 slices; the duplicate-work slice needs M2's decision runner and evals.
  - **Tests** (offline, strict replay, the `patch` implementer, fake keys):
    - `crates/planroom/tests/room.rs`: `every_project_is_a_board_in_the_room`, `a_workspace_without_a_plan_has_its_map_board`, `a_new_board_joins_its_workspaces_area`, `room_writes_need_a_person`, `each_room_write_returns_its_undo`, `removing_an_area_keeps_its_boards`, `a_bookmark_of_a_write_or_a_spend_is_refused`, `a_moved_workspace_keeps_its_boards_and_bookmarks`, `an_unreachable_board_stays_listed_with_its_reason`, `room_reads_never_migrate_a_store`, `room_json_keeps_unknown_fields`, `a_newer_room_file_is_refused`, `the_board_flag_reads_another_workspace`, `the_board_flag_on_a_read_needs_a_store`, `map_at_a_version_folds_at_its_seq`, `link_returns_the_three_forms`;
    - `crates/contract/tests/view_path.rs::view_paths_round_trip` and `::the_ui_port_runs_the_same_cases`; `packages/ui/src/view-path.test.ts::view_paths_round_trip`;
    - `crates/planroom/tests/server_auth.rs`: `one_room_server_routes_every_board`, `a_board_route_needs_the_bearer_token`, `an_unlisted_workspace_is_not_found`, `a_workspace_leaf_on_the_room_route_names_its_route`, `the_room_port_is_the_first_workspaces_port`, `the_room_port_moves_only_when_asked`, `a_board_page_path_serves_the_canvas`, `open_lands_on_its_link_and_keeps_the_code_in_the_fragment`; `stream.rs`: `a_board_stream_sends_only_its_workspaces_frames`, `the_room_stream_sends_a_boards_new_state`; `serve_limits.rs::idle_workspaces_are_closed_past_sixteen`;
    - `crates/planroom/tests/attach.rs`: `an_attached_repo_maps_with_the_workspace`, `linkers_join_across_an_attached_repo`, `attach_needs_a_press`, `a_credential_path_cannot_be_attached_even_by_a_person`, `another_users_repo_cannot_be_attached`, `a_detached_repos_edits_wait_without_cards`, `reattaching_brings_the_edits_back`, `confined_reads_reach_an_attached_repo`, `a_shared_repo_gets_suffixed_branch_names`;
    - `crates/planroom/tests/parallel.rs`: `ratifying_an_overlapping_plan_lists_the_overlap`, `side_by_side_lands_after_the_earlier_plan`, `after_holds_only_the_shared_handoffs`, `merging_proposes_one_ghost_batch_with_aliases`, `accepting_the_merge_batch_retracts_the_old_items`, `a_later_overlap_deals_one_card`, `two_plans_intending_one_block_are_duplicate_candidates`, `shared_blocks_carry_both_plans_marks`, `land_after_orders_the_plan_cards`, `a_land_after_cycle_is_refused`, `a_landed_plan_is_taken_in_by_the_others`, `a_take_in_conflict_stops_only_that_plan`, `a_take_in_with_failing_checks_leaves_the_branch`, `a_take_in_never_writes_the_tracked_branch`, `a_candidate_built_before_a_take_in_is_rebuilt`, `a_take_in_reverts_to_its_recorded_head`, `take_in_merges_use_the_ratifications_identity`, `a_take_in_stop_reads_as_take_in_stopped_in_the_plan_run`, `automatic_starts_wait_for_a_room_slot`, `a_persons_start_runs_past_the_cap_with_its_card`, `the_room_spend_cap_holds_automatic_starts_across_workspaces`, `a_persons_start_runs_past_the_room_spend_cap_with_its_card`, `a_dead_holders_slot_frees_after_its_lease`, `a_workspace_config_cannot_raise_either_room_cap`, `spend_is_attributed_to_its_plan`, `the_portfolio_shows_each_plans_run`, `the_inbox_ranks_questions_across_boards`;
    - `crates/planroom/tests/duplicate_work.rs`: `two_plans_aiming_at_one_block_deal_one_card` (strict replay), `past_the_spend_guard_both_items_stay`, `keeping_one_rejects_the_other_reversibly`; the every-kind contract test covers both new kinds; `cargo xtask eval` recomputes their results;
    - `crates/planroom/tests/parallel_plans_ci.rs`: the CI path above;
    - `feature_list.json` gains a tier C entry, "One room and plans side by side" (R10, R13), with `parallel_plans_ci.rs` and the `room.rs`, `attach.rs` and `parallel.rs` tests above;
    - `packages/mod/src/tools.test.ts`: `a_tool_call_reaches_only_an_attached_board`, `a_tool_call_never_names_another_workspace` (kept); `room.test.ts`: `room_attach_adds_a_board_to_the_session`, `room_go_opens_a_bookmark`; `keys.test.tsx::a_board_is_a_key_and_a_digit`; `prompt.test.ts::attached_boards_leave_the_composed_section_unchanged`; `guard.test.ts::an_attached_boards_state_directory_is_guarded`; `surface_tokens.sh` keeps the tools within 900 tokens with `board`;
    - `tests/e2e/src/room.spec.ts`: `the_room_lists_areas_and_boards`, `a_bookmark_url_reopens_its_view`, `back_and_forward_walk_the_views`, `a_new_tab_logs_in_from_an_open_tab`, `pending_edits_of_two_boards_flush_on_one_origin`; `surface.spec.ts::exactly_five_concepts` covers the room lid and the room's shelf; `empty_state_audit` visits the empty room; `tokens.test.ts::contrast_meets_the_bar` covers the tints.
  - **Known gaps:**
    - The overlap and duplicate checks look within one workspace. Plans in two workspaces that hold one repo meet only when one lands, where take-in covers them.
    - The landing order is per workspace.
    - The Artifact holds one workspace. A room-wide Artifact is later.
    - On a temporary host, `room.json` goes with the container. Boards come back from the stores; areas and bookmarks don't.
    - A dev build's tab left at another workspace's old port keeps its unsent edits there. No release ever served one.
    - Take-in and the run advance only while a session with the mod is open in the workspace.
  - **Users:**
    - The canvas opens on your room: areas of boards, each board's state, what it's waiting on and what it has spent, and the top question across every plan. Any view you reach has a link you can bookmark, paste into `/room go`, or open on a phone's Artifact.
    - In Claude Code, a session opens on its folder's board. `p` and a digit switch boards; `/room attach` brings in another board, or a repo from anywhere, so its services join your map.
    - When you ratify a plan that touches what another is changing, the card says so and offers side by side, after, or merge. Shared blocks show both plans' colors.
    - When one plan lands, the others take it in and check themselves. You hear about it only when a conflict needs you.
- **Sources (checked 2026-10-29):** KICKOFF.md:89 (R10) and :394; [Claims, presence and projects](architecture.md#claims-presence-and-projects) (architecture.md:385-387, :529); [Where the workspace lives](architecture.md#where-the-workspace-lives) (architecture.md:587-608); [Local server and live stream](architecture.md#local-server-and-live-stream) (architecture.md:1420-1440); [The canvas](architecture.md#the-canvas) (architecture.md:1518-1535); the Milestones section of architecture.md; [CLI JSON envelope](contracts.md#cli-json-envelope) (`Handle`, contracts.md:147), [HTTP API and live stream](contracts.md#http-api-and-live-stream), [Mod tool schemas, state and the room family](contracts.md#mod-tool-schemas-state-and-the-room-family); DR-1, DR-20, DR-25, DR-29, DR-47, DR-48, DR-50, DR-51, DR-54, DR-58, DR-71, DR-74, DR-77, DR-78, DR-85, DR-97, DR-101, DR-111; DR-115, DR-116, DR-118; `crates/planroom/src/app/data.rs`, `crates/planroom/src/server/mod.rs`, `packages/mod/src/{register.tsx, tools.ts}`, `packages/canvas/src/store.ts`.

## DR-120: Ontologies as packs, chosen per board: domain concepts aligned to blocks at every level, and release management as the first domain

- **Status:** accepted (decided on the owner's request of 2026-10-29). M3 builds all of it, after the release pack it extends; formal imports (OWL, RDF, schema.org) stay out of v0.1.
- **Starting point:** extends KICKOFF's "Packs. Declarative files that add kinds, levels, roll-up rules, extractors, lenses, decision kinds and a default view" (KICKOFF.md:383) and R11's "domains plug in with no core change". Keeps DR-9 (packs are data, never code), DR-62 and DR-109 (the pack set is workspace state, changed only by a person), DR-111 (levels are pack data; a kind may sit at the kernel's `intent`) and KICKOFF's "AI proposes, person disposes". Builds on DR-114 (reads load the workspace's recorded pack set), the intended blocks and their resolution of DR-115, the `[[items]]` table and questions of DR-116, the `[[context]]` sections and the mined-ghost rule of DR-117, and the boards and portfolio of DR-119. Amends:
  - DR-45: the kernel's vocabulary gains one kind and two relations;
  - DR-46: pack data gains `[[concepts]]`, `concepts/*.toml`, and two lens fields; a new built-in pack, `ontology`, joins the v0.1 packs;
  - DR-62 and DR-109: two more person-only commands change the pack set, and a pack's source may be the room's ontology library;
  - DR-74: `onto` joins the reserved namespaces;
  - DR-23 and DR-106: a new rule type, `ordered`, four fields on DR-116's `[[items]]` entries, and one new plan pack rule;
  - DR-36: the literal scan also refuses built-in concept IDs.
- **Origin:** the owner, 2026-10-29: "I feel like there will be multiple in play, appropriate at each abstraction layer. I'd like a pack around that." And: "I imagine different ontologies being needed for every project/domain. For example, I'm tackling release management for multiple things." The owner accepted domain concepts and a methodology per level as pack data first, with formal imports later ("your recs are fine"), and asked for the set to be planned ("Yeah I like these. Get them planned properly"). Whether the owner's releases run on independent cadences or as coordinated trains is still open with the owner, so this record serves both. The accepted proposal put `concept`, `term` and one relation, `realizes`, in an ontology pack. This record moves the concept kind and its relations into the kernel instead, because the kernel's own algorithms (the fold, lint, `find` and the miner) must read them and the kernel may name no pack's kind (below); it adds `kernel.is_a`, since the `concepts` lens draws is-a as edges; and it keeps the ontology pack for everything else: the lens, the glossary question, the draft prompt, the context section and the suggestion. Terms are a prop, not nodes.
- **Context:**
  - A pack declares kinds, relations, lenses, questions, rules, decision kinds, suggestions, prompts and levels (architecture.md:631-647; `crates/contract/src/pack.rs:21-70`). Every kind it declares describes something an extractor finds or a plan holds. No pack can say what a thing means in the domain.
  - The kernel owns `kernel.group`, `kernel.external`, `kernel.project`, `kernel.item` and one relation, `kernel.touches` (architecture.md:318; AGENTS.md Conventions). Nothing aligns a domain idea with the blocks that build it: an "order" is a service, an aggregate, a table and a file, and the map shows four unrelated blocks.
  - A pack may give a kind the `intent` level: it is visible to every pack (`crates/planroom/src/packs/validate.rs:343-346`), and only a pack's own `[[levels]]` may not claim it (:369-371).
  - A lens is a set of relations, a flow, its kinds and a level (architecture.md:872). Several lenses may share a level. A lens can't narrow to some nodes of a kind.
  - The rule types are `requires_prop`, `requires_edge`, `min_children`, `covers` and `disjoint` (architecture.md:652-658). None compares two items through what their targets depend on.
  - The plan pack writes items only as the stages declare them (`packs/plan/pack.toml:16-110`), and its `priorities` stage sets `moscow` on work and next actions (:79-95). DR-116 adds `[[items]]`, item types any stage may write, which any pack may declare.
  - The release pack (M3, 5 h; architecture.md:1815) maps tags, release workflows and images, with `release-order`, `stale` and `bump-breaks` (architecture.md:950). It knows versions and deploys, but not products, channels, environments, gates, freezes, rollouts or release trains.
  - Every pack a workspace loads applies to every project in it. Out-of-tree packs come from `pack add DIR` alone (architecture.md:662-667), and the data root holds no shared library of them (architecture.md:599).
  - The domain-free scan refuses built-in kind, relation and level names in the kernel and the contract (`boundaries.toml:26-34`).
- **Decision:**
  - **The kernel gains the words for meaning (amends DR-45).** The kernel's algorithms must read concepts: the fold derives them, lint checks them, `find` matches their terms, and the miner proposes them. DR-45 put intent in the kernel for the same reason, so concepts follow it.
    - `kernel.concept`, prefix `concept`, at the kernel's `intent` level: apart from the ladder, because one concept is realized at every level.
    - `kernel.realizes`, from a concept to a block at any ladder level, actual or intended (DR-115).
    - `kernel.is_a`, from a concept to a more general concept.
    - "Part of" is the concept's parent, so containment, roll-up and `edit move` work unchanged.
    - A concept's props: `definition` (at most 280 characters), `terms` (its synonyms: at most 12, each at most 48 characters), `realized_in` (`code`, the default, or `plan`), and `ontology` (the ID of the pack it belongs to).
    - A concept's ID is `onto:concept:<ontology>/<key>`, the key following plan keys' rule. `onto` joins `ws`, `plan` and DR-115's `intended` as a reserved namespace (amends DR-74), so a repo named `onto` takes the 4-hex suffix.
    - Everything else lives in packs: lenses, rules, prompts, questions, item types and the domain's concepts themselves.
  - **An ontology is a pack that declares concepts (amends DR-46).**
    - `[[concepts]]` in `pack.toml`, or in `concepts/*.toml` merged at load, as `languages/*.toml` is. Each entry has `key`, `name`, `definition`, `terms`, `is_a` (keys of the pack's own concepts or `<pack>/<key>` of a pack it requires), `part_of` (one such key), `realized_in`, and an optional `realized_by`.
    - `realized_by` is a `when` predicate, the `filter` shape's (DR-63, round 3), over the kinds the pack can see: `realized_by = { kinds = ["release.tag"] }` makes every tag a realization of `version`.
    - Loading refuses, with `pack_invalid`: a key twice; an `is_a` or `part_of` naming no concept in scope (`concept_unknown`); a cycle through either (`concept_cycle`); one term on two concepts of one pack (`concept_term`).
    - The built-in packs that declare concepts are always loaded, but a board views through none of them until it selects one (below). An out-of-tree ontology arrives through `pack add` or `ontology add`, as person-only as before.
  - **The ontology pack.** A new built-in pack, `packs/ontology/`, requiring nothing, holds the tooling every ontology uses: the `concepts` lens, the `glossary` question, the `ontology_draft` prompt, the `definitions` context section and the "Name the domain" suggestion. It declares no concepts, so it is no ontology a board selects. It speaks only the kernel's `kernel.concept`, `kernel.realizes` and `kernel.is_a`, so it needs no other pack.
  - **Pack concepts are derived, not stored.** The fold builds each loaded pack's concepts, their `is_a` edges and their `realized_by` edges from the pack set, beside the snapshots' facts. They change only when `packs_hash` does, which already refolds.
    - A `realized_by` edge is inferred, with confidence 1.0 and the pack as its provenance. You confirm or reject it as any inferred edge (architecture.md:345-346). Its ID hashes its kind and ends, so a rejection holds across refolds.
    - Your corrections are ordinary overrides and rejections on those IDs: rename, move, hide, `definition`, `terms`. An authored put on an ID a pack declares reads as overrides of its fields, and your field wins (DR-73).
    - After `pack remove`, overrides on its concepts wait for it to come back. They raise no `rescan.retarget` card, since the `onto` namespace of an unloaded ontology is not orphaned, only absent.
  - **A board chooses the ontologies it views through.**
    - The workspace loads its packs. A project's `ontologies` prop, a list of loaded ontology IDs, chooses which of them its board shows. A board is one project's plan and map (DR-119), so the project is the selection's home.
    - `ontology use ID... [--project P]` sets it; `ontology use --default` clears it. Unset means every out-of-tree ontology the workspace loaded, which a person added, and no built-in one. So a board views through the release ontology only after `ontology use release`, and a workspace with one ontology it added needs no step. An ID no loaded pack declares concepts in is `invalid_argument`, naming the loaded ones.
    - A person's `ontology use` applies; an agent's is a ghost, since it changes what the board shows and what drafts are written from.
    - The selection scopes five things: concept chips on blocks, the board's lens list, the board's ontology lint, the glossary in prompts, and the context pack's `definitions`. It never refuses a read: `map --lens L` naming an unselected ontology's lens still answers, with the warning `ontology_unselected`.
  - **A methodology per level is a profile lens.**
    - `LensDecl` gains `concepts`, keys of the pack's own concepts. A lens with `concepts` shows those concepts beside the blocks that realize them, at its level, as the plan lens shows items beside their targets. Several profiles may share a level.
    - `LensDecl` gains `explains` (default false). On a profile with it, every block of the lens's `kinds` at its level that none of its concepts realizes is a lint finding, `code_unexplained`. It is opt-in, so a profile flags only the level whose method says every block has a meaning.
    - The ontology pack's `concepts` lens shows every selected concept with its `is_a` edges and parents, at `intent`.
  - **Every block shows what it means.** `MapNode` gains `concepts`: up to two `ConceptTag { id, name }` from the selected ontologies, by the `kernel.realizes` edges that count (inferred or accepted, never a ghost), with `more_concepts` counting the rest. Surfaces draw them as chips. Under a profile lens, `diagram map` labels each block "Concept: block name" (DR-112).
  - **The kernel checks meaning.** `ask lint` and each board's lint add four built-in checks over the selected ontologies, beside every pack's `[[rules]]`:
    - `concept_unrealized`: a `code` concept that no `kernel.realizes` edge that counts joins to an actual block. The edges that count are those chips count: inferred from a pack's `realized_by`, or accepted, never a ghost, so a mined edge counts only once a person accepts it. Its line says "planned, unbuilt" when an unresolved intended block realizes it, and "not planned" otherwise. A resolved intended block reads as its actual block (DR-115), so a concept that realizes one counts as built. A `plan` concept is never flagged: items instantiate it, and code doesn't.
    - `code_unexplained`: as above, only on a profile with `explains`.
    - `term_ambiguous` (one term, two meanings): one word, case-folded, that is a name or term of two selected concepts.
    - `concepts_overlap` (two terms, one meaning): two selected concepts whose realized actual blocks are the same set, of at least two.
  - **Words reach the map by `find` and `glossary`.** `find` also matches a concept's terms. The ontology pack's question `glossary WORD` runs `find`, then `filter` to `kernel.concept`, then `neighbors` over `kernel.realizes` forward, depth 1: what the word means and what builds it.
  - **Proposals are ghosts, and only a person makes them count.**
    - `ontology propose ID --file F|-` writes an agent's proposal (`OntologyProposal`: concepts with key, name, definition, terms, `is_a`, `part_of`, and `realizes` targets by node ID) as one ghost batch. In chat, Claude drafts it itself, as it does plan proposals (architecture.md:1226), so it spends nothing.
    - `ontology propose ID --mine` runs the deterministic miner, `kernel::ontology::mine`, and writes what it finds as one ghost batch, even when a person runs it, by DR-117's rule that a proposal an engine miner writes is a ghost whoever runs it. The miner:
      - splits every block name and plan item name into words (camelCase, `snake_case`, kebab-case, path parts), lowercases them and strips a plural `s`, `es` or `ies`;
      - drops a word that names more than 30% of the nodes at any one level, so `src`, `lib` and `main` fall out without a stop list;
      - proposes a concept for each word that names at least 3 nodes of at least 2 kinds, ranked by kinds, then levels, then nodes, then the word, top 20 per run, each with ghost `realizes` edges to at most 20 nodes it names (types, tables, API resources, routes, screens);
      - for a word that already names a selected concept, proposes only `realizes` edges to blocks not yet joined to it;
      - for each `concepts_overlap` finding, proposes the merge: the second concept's name added to the first's `terms`, and the second hidden. Rejecting it keeps both.
    - `ontology draft ID [--project P]` is the engine-run draft. A capable-tier Claude call, the ontology pack's `ontology_draft` prompt, reads the project's items, the miner's candidates and the selected glossary, and writes an `OntologyProposal` as one ghost batch by the `model` actor. It spends, so it carries the consent card and the daily cap exactly as `intent next` does (DR-89), and tests replay it.
    - `ontology realize CONCEPT BLOCK [--remove]` adds an authored `kernel.realizes` edge, as a plan item's `kernel.touches` is authored, never on the intended layer, so drift never reads it. `--remove` takes back an authored edge, or rejects an inferred one.
    - `ontology define CONCEPT [--name N] [--definition T] [--term W]... [--parent C] [--is-a C]...` puts a new concept or sets an existing one's fields.
    - A person's `realize` and `define` apply; an agent's are ghosts. Accepting a ghost batch, item by item or whole, is the existing review (DR-50). This record adds no exception to "nothing an agent writes counts until a person accepts it".
  - **Saving an ontology makes it portable (amends DR-62 and DR-109).**
    - The room's ontology library is `DATA_ROOT/ontologies/<id>/`, user-level and never in a repo. Each version is a directory of its own, `versions/<n>/`, never changed once written, and the file `current` names the newest. Version directories are only ever added (DR-78); `current` is the one file rewritten, by write and rename.
    - `ontology save ID` is person-only, with a press, a typed `/room` or a `/dev/tty` confirmation, as `pack add` is. It writes a new directory, `versions/<n+1>/`: every file of the loaded version byte for byte, except that `concepts/` holds one file, `concepts/saved.toml`, with the ontology's current concepts (pack concepts with your overrides, plus authored concepts in `onto:concept:ID/`, their `is_a` edges and parents). The library's version is the directory's number; `pack.toml`, its `version` included, is copied as written. So no key is declared twice, and nothing is overwritten.
      - A pack whose `pack.toml` holds `[[concepts]]` is refused with `invalid_argument`, `argument.concepts_in_pack_toml`: its keys would be declared twice, and `pack.toml` isn't rewritten (below). The hint is to move them to `concepts/*.toml`.
      - It writes under `ontologies/<id>.lock`: the version directory aside, then renamed into place, then `current`.
      - It refuses with `ontology_changed` when the library's current version isn't the one this workspace loaded, so two workspaces never race to one version number. It is a conflict, exit 4, retry `after_reread`; the hint is the changed-source card's Update, or `planroom ontology add ID`.
      - In the same commit, it records the library copy as the workspace's source for that pack, copies it as `pack add` does, and retracts the authored concepts it saved, so each concept has one home. One undo restores the old record and the authored concepts. The library keeps the version, since an undo changes the workspace and never deletes a file.
      - It never saves `realizes` edges: they name this workspace's blocks. A new workspace aligns through `realized_by`, the miner and you.
      - A built-in ontology's ID is `invalid_argument`: the binary owns it, and your corrections to it stay this workspace's overrides.
    - `ontology add ID` is `pack add` of the version `current` names: person-only, undone by `pack remove`. Each workspace still records its own copy (DR-109), with the library ID and version as its source.
    - When another workspace saves a newer version, every workspace whose recorded source is that library ID gets DR-109's changed-source card at its next scan, with Keep pre-selected. No workspace's ontology changes without its person's pick.
    - `ontology list` lists the loaded ontologies, with the board's selection marked, and the library's, each with its version, its concept count and the workspaces it knows of. It is a read, paged under 1,500 tokens.
  - **Plans, questions, context packs and diagrams speak the domain's words.**
    - An engine-run stage (`intent next`) gets the board's glossary in its uncached tail: each selected concept's name, its definition cut to one line and its terms, at most 600 tokens, most-realized first. An empty glossary is left out, as an empty `not_this` is (DR-116), so a board with no selection sends the same bytes as before, and no recording made without one is invalidated.
    - `ask stage` lists the same glossary, so a drafting agent in chat reads it too. Questions (DR-116) name concepts through the same words.
    - **A stage learns the domain's item types.** DR-116's `[[items]]` entries gain `stages`, the stage IDs that may write the type, and an optional `checklist` line. `ask stage` and the stage request list, for the stage they serve, the `[[items]]` types whose `stages` name it, from the plan pack and from the board's selected ontologies: each with its props, its `concept` and its checklist line. An unselected ontology's types aren't listed, so the request keeps its bytes.
    - The ontology pack declares the `[[context]]` section `definitions` (DR-117): from the targets, `neighbors` over `kernel.realizes` backward, depth 1, citing `definition` and `terms`.
  - **Release management is the release pack's ontology.** The release pack (built-in, requiring `code`) is the first worked domain. Release management needs the release pack's kinds, and a built-in pack may require only `code`, so the concepts go into that pack rather than into a pack of their own.
    - Concepts, with what realizes them:

      | Concept | `realized_in` | `realized_by` |
      |---|---|---|
      | `product` | code | `code.workspace`, `code.repo` |
      | `component` | code | `code.service`, `code.library`, `code.package` |
      | `version` | code | `release.tag` |
      | `version_constraint` | code | none; you or the miner align it |
      | `release` | code | `release.release_workflow` |
      | `channel`, `environment` | code | none; the miner finds them in workflow and deploy names |
      | `release_train`, `gate`, `freeze`, `rollout`, `rollback` | plan | none |

    - Item types, through DR-116's `[[items]]`, each with `stages = ["breakdown", "next_actions"]` and one checklist line: "On a release board, draft each train, its releases, gates and freezes as these types." DR-116's `ItemTypeDecl` gains `concept`, the key of the concept it instantiates, and `summary`, at most three prop names a portfolio row shows, beside `stages` and `checklist`.
      - `release_train`: `cadence` (`independent` or `coordinated`), `channel`, `next_date`; summary `cadence`, `next_date`.
      - `release`: touches the component it releases; `version`, `channel`, `date`.
      - `gate`: touches what it guards; `owner`, `environment`, `check`.
      - `freeze`: `from`, `until`, `scope`. `rollout`: `stages` (percentages). `rollback`: `trigger`, `steps`.
    - Rules: `gate_owner` (`requires_prop` `owner` on gates: a gate with no owner), `release_has_component` (`requires_edge` `kernel.touches` on releases), and `release_order` (`ordered`, below: a component released ahead of what it depends on).
    - A profile lens, `releases`, at `container`, about `component`, `version`, `channel` and `environment`, over `kernel.realizes`, `release.deploys` and `code.depends_on_package`.
    - **Both cadences, one model.** A coordinated release is one project whose train is `coordinated`, with one `release` item per component; `release_order` orders them along dependencies. Independent cadences are one project per product or component, each with its own train, and every board can select the release ontology. Two trains releasing one component meet in DR-119's overlap check at ratification. DR-119's `boards` lists every train with its summary props: `BoardRow` gains `summary`, the `summary` props of the project's open items whose type declares one, at most 3 items, each value cut to 60 characters.
  - **One new rule type, `ordered` (amends DR-23).**
    - Parameters: `prop` (the prop that orders), `values` (an explicit order; without it, values compare as text, so ISO dates order by time), `along` (relations; without it, the default lens's relations, so a pack names no code relation), and `props.<name>` (field test).
    - It finds a subject whose `kernel.touches` target depends, through one edge of `along` lifted to the targets' level, on another subject's target, while the subject's `prop` comes before the other's. The finding names both items and the edge.
    - The release pack uses it in `release_order`: `prop = "date"`, along `code.depends_on_package` and `release.deploys`. The plan pack uses it too, on its own merits, in `moscow_order`: `prop = "moscow"`, `values = ["must", "should", "could", "wont"]`, no `along`. It finds a must-do next action that rests on a could-do one.
  - **Guarded (amends DR-36).** The literal scan also refuses, in the kernel's and the contract's sources and the TypeScript surfaces, a string literal equal to a built-in pack's concept ID (`release/gate`), outside tests. AGENTS.md's naming convention lists the kernel's new kind and relations. Both are gated surfaces, ratified with this record and `cargo xtask gen --layout-lock` in its merge commit.
- **Alternatives rejected:**
  - **Concepts as pack kinds only, with the engine naming them:** the domain-free scan forbids it, and declared roles in pack data are the second role mechanism DR-45 and DR-111 rejected.
  - **One node kind per concept** (`release.gate` as a kind): a domain's vocabulary would need a pack per word, and an agent's draft couldn't add a concept without a new pack and a person's `pack add`.
  - **Terms as nodes with a `means` relation:** it doubles the nodes for no gain. "One term, two meanings" is a word on two concepts, and a prop answers it.
  - **Pack concepts stored as events at `pack add`:** two homes for one concept, and a pack update would conflict with its own copy.
  - **Authored concepts and `realizes` edges on the intended layer:** drift would read a domain idea as a missing block forever.
  - **Selection per workspace:** the owner asked for an ontology per project or domain, and a workspace holds several projects.
  - **Hiding unselected ontologies' answers:** a bookmarked view would break when a board's selection changes. A warning keeps the read working.
  - **Mining at every scan:** each scan would deal a ghost batch nobody asked for. A suggestion offers `--mine` instead.
  - **A model to find synonyms and overlaps:** it costs money on every run and can't be tested offline. Shared realizations are a deterministic signal.
  - **Release management as an out-of-tree example pack:** releases wouldn't ship it, and it would duplicate the release pack's kinds. Inside the release pack it ships with the binary, and a board turns it on with `ontology use release`.
  - **Unset meaning every loaded ontology:** built-in packs always load, so every board in every workspace would view through release management, with its chips and its unrealized `channel` and `environment`.
  - **`is_a` as a prop of concept keys, so the kernel gains one relation:** the `concepts` lens draws is-a as edges, and a lens is a set of relations (architecture.md:872), so a prop would need a second lens mechanism.
  - **The concept kind and `realizes` in the ontology pack, as first proposed:** the fold, lint, `find` and the miner are the kernel's and must read them, and the kernel may name no pack's kind (`boundaries.toml:26-34`).
  - **The ontology pack's declarations in the plan pack:** the lens, the glossary and the context section serve every board, plan or none, and the owner asked for a pack around ontologies.
  - **Saving `realizes` edges into the library:** they name one workspace's blocks, which another workspace doesn't have.
  - **Saving a built-in ontology's corrections into the library:** the copy would shadow the binary's pack and drift from it at the next release.
  - **Rewriting `pack.toml` on save:** `toml` drops comments, and the planroom crate's allow-list has no format-preserving editor. One generated `concepts/saved.toml` in a new version directory keeps every hand-written file intact.
  - **Saving over the library's current files:** a workspace that loaded the old version would lose its source; a new directory per version overwrites nothing.
  - **OWL, RDF or schema.org import now:** the owner put formal imports later. `[[concepts]]` is a subset an importer can write.
  - **Semantic-version checks in `release_order`:** satisfying a constraint is domain logic, and packs carry no code. The rule orders by date along the dependency edges.
- **Consequences:**
  - **Contracts:**
    - The kernel's `kernel.concept`, `kernel.realizes` and `kernel.is_a` in `Kind` and the relation list; the reserved namespace `onto`.
    - `ConceptDecl` and `PackFile.concepts`; `RealizedIn`; `Vocabulary.concepts`; `LensDecl.concepts` and `LensDecl.explains`; `ItemTypeDecl.concept`, `.summary`, `.stages` and `.checklist`; `AskStageResult.glossary` and `.item_types`; `BoardRow.summary` (marked `x-redact`); `RuleType::Ordered` and its row in `RuleType::params`.
    - `MapNode.concepts` and `more_concepts`; `ConceptTag`, whose `name` is marked `x-redact` because an authored or mined name comes from repo text.
    - `Command::Ontology` with the leaves `list`, `use`, `propose`, `draft`, `realize`, `define`, `save` and `add`; `OntologyProposal`, `OntologyListResult`, `OntologySaveResult`.

      | Leaf | Idea | Class | Undo | Actor | Budget |
      |---|---|---|---|---|---|
      | `ontology list [--cursor C]` | Lenses | read | query | any | 1,500 |
      | `ontology use ID...\|--default [--project P]` | Lenses | write | reversible | any; an agent's is a ghost | 300 |
      | `ontology propose ID --file F\|-\|--mine` | Intent | write | reversible | any; always ghosts | 600 |
      | `ontology draft ID [--project P]` | Intent | write, spends | reversible | any; always ghosts | 600 |
      | `ontology realize CONCEPT BLOCK [--remove]` | Lenses | write | reversible | any; an agent's is a ghost | 300 |
      | `ontology define CONCEPT ...` | Lenses | write | reversible | any; an agent's is a ghost | 300 |
      | `ontology save ID` | Lenses | write | compensate | person | 300 |
      | `ontology add ID` | Lenses | write | compensate | person | 300 |

    - Error `ontology_changed` (conflict, exit 4, retry `after_reread`; `details`: `id`, `library_version`, `loaded_version`; no key masked), with its row in the regenerated `docs/reference/errors.md`. Warning `ontology_unselected` (`details`: `lens`, `ontology`).
    - `pack_invalid` reasons `concept_unknown`, `concept_cycle`, `concept_term`, `concept_key`, `lens_concepts`. `invalid_argument` gains `ontologies` in `details`, and the reason `argument.concepts_in_pack_toml`.
    - Lint findings `concept_unrealized`, `code_unexplained`, `term_ambiguous`, `concepts_overlap`, and the packs' `gate_owner`, `release_has_component`, `release_order` and `moscow_order`.
    - Formats: `planroom.pack/v1` gains the optional `[[concepts]]` table and `concepts/*.toml`, an additive change with `schema_version` kept at 1. The library is pack directories and a `current` file holding one number, and adds no format of its own.
    - Layout: the new `packs/ontology/` with its `README.md`, and its line in `packs/AGENTS.md`'s Layout list, ratified with this record's layout lock.
  - **Milestones:** M1 gains nothing. The ontology needs reads that load the recorded pack set (DR-114, finishing in M1), the consent card and daily cap (M2), the context packs (M2) and the release pack (M3), and the owner's release work begins with M3's release pack. Its slices follow M3's release pack slice. The selection's home is a project prop, so only `BoardRow.summary` and the trains' portfolio wait for DR-119's portfolio slice. M3 grows by 45.5 h:

    | Slice | Milestone | h |
    |---|---|---|
    | Concepts as kernel vocabulary and pack data: the three kernel names and `onto`; `[[concepts]]` and `concepts/*.toml` with their checks; the fold's derived concepts and `realized_by` edges; overrides, rejections and the absent-ontology rule; `find` over terms; the built-in `packs/ontology/` with its README and Layout line; the literal scan and the layout lock | M3 | 10 |
    | Boards and checks: `ontology use` and `list`, the project's `ontologies` prop and the default of no built-in ontology, the board's lens list and `ontology_unselected`; profile lenses with `concepts` and `explains`; `MapNode.concepts`; the four kernel checks; `glossary` | M3 | 8 |
    | Authoring and the library: `propose` with `--file` and `--mine` and the miner, `realize`, `define`; `save` into a new version directory with its lock and `current` file, the `pack.toml` refusal, `ontology_changed`, the record switch and one undo; `add`; the changed-source card from the library | M3 | 9 |
    | ‖ Release management in the release pack: concepts, `ItemTypeDecl`'s `concept`, `summary`, `stages` and `checklist`, the release item types listed in `ask stage` and the stage request on a release board, the `ordered` rule type with `release_order` and `moscow_order`, `gate_owner`, the `releases` profile; a coordinated and an independent train in the fixture's tests; `BoardRow.summary` after DR-119's portfolio slice | M3 | 8.5 |
    | The domain's words: `ontology draft` with its prompt, replay and consent, and its one recording (about $0.02); the glossary in stage tails and `ask stage`, left out when empty, with one breakdown recording on a release board (about $0.015) that the glossary and release-item tests replay; the `definitions` context section; profile labels in `diagram map` | M3 | 5.5 |
    | Surfaces: concept chips on blocks; the `concepts` lens in the canvas; the lens list by board; the glossary in search; `/room ontology` typeahead; the "Name the domain" suggestion, an ordinary `[[suggestions]]` entry, with no pane line of its own | M3 | 4.5 |

  - **Tests** (all offline, through the CLI's `--json`, with replays and the `patch` implementer where a hand-off is involved):
    - `crates/kernel/tests/ontology.rs`: `pack_concepts_fold_under_onto_with_their_parents`, `a_realized_by_predicate_derives_inferred_realizes_edges`, `a_rejected_realizes_edge_stays_rejected_across_refolds`, `an_authored_put_on_a_pack_concept_reads_as_its_overrides`, `overrides_on_a_removed_ontology_wait_and_raise_no_card`, `the_miner_finds_words_recurring_across_kinds_and_levels`, `the_miner_drops_a_word_naming_most_of_a_level`, `the_miner_output_is_deterministic`, `an_unrealized_code_concept_is_flagged_and_an_intended_realization_reads_as_planned`, `a_plan_concept_is_never_unrealized`, `one_word_on_two_selected_concepts_is_ambiguous`, `two_concepts_realized_by_the_same_blocks_overlap`, `only_selected_ontologies_are_checked`, `an_inferred_realization_counts_as_built_and_a_mined_ghost_does_not`.
    - `crates/kernel/tests/lint.rs`: `ordered_flags_an_item_dated_before_what_its_target_depends_on`, `ordered_follows_an_explicit_value_order`, `ordered_without_along_reads_the_default_lens`.
    - `crates/planroom/tests/packs.rs`: `a_concept_naming_an_unknown_concept_is_pack_invalid`, `an_is_a_cycle_is_pack_invalid`, `one_term_on_two_concepts_of_a_pack_is_pack_invalid`, `a_lens_naming_another_packs_concept_is_pack_invalid`, `concepts_toml_merges_into_its_pack`, `the_ontology_pack_loads_and_declares_glossary_and_definitions`.
    - `crates/planroom/tests/ontology.rs`: `ontology_use_sets_the_projects_ontologies`, `a_board_with_no_selection_shows_no_release_concepts_or_findings`, `ontology_use_default_clears_the_selection`, `an_agents_ontology_use_is_a_ghost`, `an_unknown_ontology_is_invalid_argument_naming_the_loaded_ones`, `an_unselected_lens_leaves_the_board_and_still_answers_with_a_warning`, `mined_proposals_are_ghosts_even_for_a_person`, `an_agents_proposal_is_one_ghost_batch`, `nothing_proposed_counts_in_lint_until_accepted`, `save_needs_a_person`, `save_writes_the_library_and_switches_the_record_in_one_undo`, `save_refuses_when_the_library_moved_on`, `save_keeps_every_other_pack_file_byte_for_byte`, `save_writes_a_new_version_directory_and_overwrites_nothing`, `save_refuses_a_pack_with_concepts_in_pack_toml`, `a_built_in_ontology_cannot_be_saved`, `undo_of_a_save_restores_the_authored_concepts_and_keeps_the_library`, `another_workspace_gets_the_changed_source_card_after_a_save`, `ontology_add_records_a_copy_as_pack_add_does`, `glossary_finds_a_concept_by_its_term`.
    - `crates/planroom/tests/ontology_draft.rs`: `a_replayed_draft_writes_one_ghost_batch`, `a_draft_without_consent_spends_nothing`.
    - `crates/planroom/tests/release_mgmt.rs`: `a_gate_with_no_owner_is_flagged`, `a_component_released_before_its_dependency_is_flagged`, `a_coordinated_train_orders_its_releases_along_dependencies`, `independent_trains_keep_their_own_cadence`, `tags_realize_versions_through_the_pack`, `the_releases_profile_shows_components_with_their_concepts`, `a_breakdown_on_a_release_board_lists_release_item_types`, `a_replayed_release_breakdown_writes_a_train_and_a_gate` (strict replay of the release-board recording), `a_board_without_the_release_ontology_sends_unchanged_stage_bytes`.
    - `crates/planroom/tests/s03_plan.rs::a_must_action_resting_on_a_could_action_is_flagged`; `crates/planroom/tests/map_read.rs::blocks_carry_only_the_selected_ontologies_concepts`; `crates/planroom/tests/intent_next.rs::the_stage_tail_carries_the_boards_glossary`; `crates/planroom/tests/context.rs::definitions_carry_the_targets_concepts`; `crates/planroom/tests/diagram.rs::a_profile_lens_labels_blocks_with_their_concepts`; `crates/planroom/tests/room.rs::a_train_boards_row_carries_its_summary_props`.
    - `packages/ui/src/map.test.ts::concept_chips_show_two_and_a_count`; `packages/canvas/src/lens-switcher.test.ts::lenses_follow_the_boards_ontologies`; `tests/e2e/src/surface.spec.ts::concept_chips_sit_on_blocks_under_a_profile_lens`, with `exactly_five_concepts` still passing.
    - `cargo xtask check boundaries --self-test` plants `"release/gate"` in the kernel and expects the scan to refuse it.
    - `feature_list.json` gains two tier C entries: "Ontologies chosen per board" (R11, R13), with the `ontology.rs` tests above, and "Release management concepts" (R11), with the `release_mgmt.rs` tests above.
  - **Known gaps:**
    - No OWL, RDF or schema.org import in v0.1.
    - `release_order` compares dates along dependency edges; it doesn't check that a release satisfies a version constraint.
    - Corrections to a built-in ontology stay in their workspace; only out-of-tree ontologies travel through the library.
    - `realizes` edges never travel. A new workspace re-aligns through `realized_by`, the miner and you.
    - The miner splits and singularizes English names only.
    - The library is never pruned, and an older version can be restored only by `pack add` of its `versions/<n>/` directory.
  - **Users:**
    - Blocks carry chips that say what they mean ("Version", "Component"). A lens per method shows each level in the domain's words.
    - The pane suggests "Name the domain: 14 words recur across your code". One key runs `--mine`, and the proposals arrive as one ghost batch you accept, edit or reject.
    - Lint says when a concept is planned and unbuilt, when code means nothing in the domain, and when one word means two things or two words mean one.
    - Plans, questions and hand-off reading lists use your words.
    - For release management, you select the release ontology on a board (`ontology use release`), draft a train, which the breakdown now offers as a release item type, and see a gate with no owner or a component released ahead of what it depends on before you ratify.
    - `ontology save` puts your ontology in your room's library. Every other workspace that uses it asks you before taking the new version.
- **Sources (checked 2026-10-29):** KICKOFF.md:383; [What a pack is](architecture.md#what-a-pack-is) (architecture.md:617-680); [Lenses and questions](architecture.md#lenses-and-questions) (:870-938); [The v0.1 packs](architecture.md#the-v01-packs) (:940-953); [Layers, overrides and ghosts](architecture.md#layers-overrides-and-ghosts) (:338-349); [IDs and aliases](architecture.md#ids-and-aliases) (:312-318); [Claims, presence and projects](architecture.md#claims-presence-and-projects) (:529); [Where the workspace lives](architecture.md#where-the-workspace-lives) (:599); [Milestones](architecture.md#milestones) (:1761-1847); DR-9, DR-15, DR-23, DR-36, DR-45, DR-46, DR-62, DR-63, DR-73, DR-74, DR-106, DR-109, DR-111, DR-112; `crates/contract/src/pack.rs:21-70, :104-160, :390, :589-612`; `crates/contract/src/vocab.rs:19-28`; `crates/contract/src/view.rs:45`; `crates/contract/src/requests.rs:28-89`; `crates/planroom/src/packs/validate.rs:138-145, :343-371`; `crates/kernel/src/plan.rs:27-30`; `packs/plan/pack.toml:1-14, :79-95`; `boundaries.toml:26-34`.

## DR-121: The band offers undo, the pane's key map, and `/room` and the tools from one synopsis per leaf

- **Status:** accepted (decided at the merge of M1's band, keys, `/room` and tools, 2026-10-29).
- **Starting point:** extends DR-14, DR-21, DR-54, DR-65 and DR-66; amends DR-8, DR-54, DR-66, DR-77, DR-85, DR-89 and DR-105, and the contract's area row. Builds on DR-111, the levels record, and DR-113, which pages `schema --commands`.
- **Origin:** the merge of M1's band, keys, `/room` and tools, read against the docs at the merge.
- **Context:**
  - The tool descriptions, `/room`'s help and typeahead and the CLI's usage line each spelled a leaf's arguments their own way, and they drifted (drift-r2-4): a tool's description read `map: The map at a level through a lens.`, with no flags.
  - The mod put `--base` and `--idem` after every write's arguments. The CLI read a leaf's own flags only before its subcommand and refused a flag the leaf doesn't take, so an `edit` through `room_change`, a typed `/room edit`, the band's undo and `/room share` came back `invalid_argument`.
  - DR-8's outbox had no shape, so a write in flight when the mod reloaded was lost.
  - DR-105 left gaps: the tool descriptions, `room-args.ts`'s usage line and `/room`'s "+N more" were written in the mod; no row was selected until a row key selected one; `room_query` asked for the full `focus`; the list's page Button had no key.
  - `share --out` was required, and the mod can't know where a workspace's state directory is.
- **Decision:**
  - **One synopsis per leaf.** Every `CommandSpec` carries its request's clap arguments (`args`, the request's `augment_args`), and `CommandSpec::synopsis` and `CommandSpec::flags` read them. A leaf whose request has subcommands names them after its path: `planroom edit move|group|...|undo [--project PROJECT] ...`. `LeafListing` carries the synopsis, so the catalogue hash covers a leaf's flags, and the CLI's `invalid_argument` takes it from the spec. The listing pages as DR-113 decided, with its opaque cursor bound to the catalogue; a second copy of the paging, built beside the synopses with a bare index for a cursor, is dropped. `CommandSpec`'s `PartialEq` and `Eq` compare by path, since `args` is a function pointer. `intent next`, `pack add` and `pack remove` carry `args` too.
  - **A leaf's own flags are global to its subcommands.** `WriteCommon`'s `--base`, `--idem`, `--project` and `--client` are clap globals, so `edit` reads them on either side of its subcommand, as its synopsis writes them. This only loosens the parser: every line it read before reads the same. The tool handler and the person's runs still put them right after the leaf's path, and only where `LEAF_OWN_FLAGS` lists them, so `share` and the reads get neither.
  - **Generated for the mod.** `cargo xtask gen` appends to `catalogue.ts`: `TOOL_SPECS` (each tool's flat input, `isDeferred` on `room_handoff` alone, and a description made of its `[tools]` intro from en.toml and one synopsis line per leaf), `ROOM_COMMANDS` (each `/room` leaf with its synopsis, subcommands and flags), `LEAF_OWN_FLAGS` and `MOD_COMMANDS`, with their types. Every generated line drops `planroom`, `[--json]`, `[--base BASE]`, `[--idem IDEM]` and `[--client CLIENT]`, which the handler or the person's run supplies. `@planroom/contract` re-exports them, and `@planroom/ui` re-exports `ACTION_KEYS`, `KEY_MAP` and their types.
  - **The tools** take their specs from `TOOL_SPECS`. They refuse `--client` as they refuse `--workspace`, in either form, and give `focus` `--detail summary` unless the arguments name a detail. Their own refusal carries the leaf's synopsis, its schema handle and the hint, as the engine's `invalid_argument` does.
  - **`/room`** parses, helps and offers typeahead from `ROOM_COMMANDS`, in this order: the command words, a subcommand, the flags not yet typed, then, after a flag that takes a value, its fixed values. A flag whose value is open, such as `--level`, offers nothing, since `ask` alone lists the levels (DR-111). The text says at most two warnings and counts the rest. Its handles come from the result, its empty state and its warnings; a page's `meta.next` becomes the Next page handle, the command that ran with `--cursor`. A map page's summary counts the whole map from the `truncated` warning's total. `/room share` hands the model the publish's arguments as context.
  - **The band** never presses a leaf that can spend on a stray digit. A press whose reply names an undo makes that undo the band's primary for 10 s (`1 undoes it` in the toast, or `u undoes it` after a pane press, where `1` is a card key). An undo's own reply offers none, and a failed one, such as a stale write, clears it.
  - **The pane's keys.** `KEY_MAP` lists all 36 keys with their action and section: the card's `1` to `4`, the 12 row keys and the 20 action letters. `e` opens the canvas as the person's press, and the first keyed row is selected from the start. The list's page Button takes the next free row key; when the pool would be spent, the list holds one row fewer, so every shown row keeps its key. Keys whose leaf or data isn't there yet stay unwired.
  - **The outbox** is `OutboxEntry` in `$.state`: the write's `idem`, its argv, the names of the variables it set (never their values), its stdin, its cause and its session. It is kept from before the process starts until any reply, and a reload resends it once, before the watch loop, as the same actor and client, so the store writes one event. A leaf that takes no `--idem` is no replayable write and stays out of it.
  - **The prompt section** is one stable entry with the id `planroom:map`, by the host's `<plugin>:<name>` convention, from en.toml's `mod.prompt_section`, appended last. Its levels and lenses read the levels record's words. The hand-off sentence joins it when hand-off lands in M2.
  - **`share --out` is optional.** Without it the bundle goes to the state directory's `share/` (`.planroom/share`, or `.git/planroom/share` for a one-repo workspace), so `/room share` needs no argument in any layout.
  - **Budgets.** `budgets.json` reaches the mod's vitest files through `vitest.config.js`'s define `__PLANROOM_BUDGETS__`, declared in `packages/mod/src/budgets.d.ts`, not through an import edge, so `boundaries.toml` doesn't change. `tool_results` gates each golden CLI output, but the watch frames, as the tool result the handler returns, measured on the merged tree: `focus` 390, `focus-no-repos` 175, `invalid-argument` 126, `map-container` 724, `map-empty` 227 and `map-graph` 881 tokens, each within its leaf's budget. `surface.tools` 900 and `surface.section` 300 are architecture.md's shares of the always-on surface (the non-deferred tools measure 403 and the section 194), and `room_text` 300 is `/room`'s cap. All four are new keys, not raises (DR-89).
  - **Tests.** The key tests live in `packages/mod/plugin-tests/keys.test.tsx`, where presses reach the host. The host stand-in refuses a `--base` or `--idem` the leaf doesn't take, as the engine's parser does. `every_golden_handle_runs_ok` fails a handle whose reply says `ok` from a process that exited nonzero. The golden `invalid-argument.json` is `map --detail nope --json`, so it keeps clap's own path now that a level is pack data.
- **Alternatives rejected:**
  - Hand-written tool descriptions and a hand-written `/room` grammar: they drifted from the CLI (drift-r2-4).
  - Moving the supplied flags in the mod alone: the decided synopsis and a typed `/room edit move A B --project P` would still be refused.
  - A raised `schema` budget for a listing with synopses: DR-113's pages hold it.
  - Importing `budgets.json` in the mod's tests: an import edge out of the package that `boundaries.toml` would have to allow.
  - Offering the flags after a flag whose value is open: the next word is that flag's value.
  - The section id `planroom`: the host names a plugin's entries `<plugin>:<name>`.
- **Consequences:**
  - Contracts: `CommandSpec.args`, `CommandSpec::synopsis` and `CommandSpec::flags`, `LeafFlag` and `LeafListing.synopsis`; `OutboxEntry`; `ShareReq.out` optional; `WriteCommon`'s flags global; in `catalogue.ts`, `TOOL_SPECS`, `ROOM_COMMANDS`, `LEAF_OWN_FLAGS` and `MOD_COMMANDS`, with `ToolSpecData`, `RoomCommand`, `LeafFlag` and `ToolName`. All are additive, so `schema_version` stays 1. The catalogue hash changes.
  - Ratified by this record, as gated surfaces: repo-layout.md's contract area row, which says the contract reads a leaf's derived clap arguments for its synopsis and flags and shapes the schemas it generates (`schema::generate`, `draft_reply_schema`); and `@planroom/contract`'s new exports `LEAF_OWN_FLAGS`, `ROOM_COMMANDS`, `TOOL_SPECS` and `MOD_COMMANDS`, and the types `LeafFlag`, `RoomCommand`, `ToolName` and `ToolSpecData`.
  - The level sites take the levels record's forms: `catalogue.ts` lists no `--level` values, and every synopsis reads `[--level LEVEL]`; the golden's wrong argument is `--detail`; `room-args.test.ts`'s fixed-values case is `--detail`; en.toml's `[tools] args` example names `--focus` and `ID`, and the prompt section's levels and lenses follow `ask`. No level literal is added, and `level_words` doesn't change.
  - DR-105's gaps closed: the tool descriptions, `room-args.ts`'s usage line, "+N more", `focus`'s summary for the tools, the first row selected and the page key.
  - Known gaps: the guard's verdict reasons and `live.ts`'s line are still written in the mod; the resolver's source build takes no lock; `countsOf` has no plural; `/room map --level ` offers no levels (DR-111); pickers, presence, the block row keys and `x` and `z` land with MD3, and `main_flows_within_two_keys` with MD4; the mini-map's yarn from `MapView.yarn` comes later.
  - Tests: `crates/contract/src/command.rs::a_leafs_own_flags_parse_after_its_subcommand`; `crates/planroom/tests/usage.rs::{a_leafs_synopsis_is_the_one_schema_commands_lists, a_leafs_own_flags_run_after_its_subcommand}`; `crates/planroom/tests/golden.rs::{every_golden_handle_runs_ok, an_edit_and_its_undo_handle_run_as_the_round_trip_runs_them, an_ok_reply_with_a_nonzero_exit_is_no_successful_run}`; `packages/mod/src/tools.test.ts::{tool_schemas_equal_catalogue, tool_results_within_budget, a_wrong_argument_returns_the_leaf_synopsis, a_tool_refuses_client}`; `packages/mod/src/room.test.ts::{room_text_within_budget, room_commands_snapshot, room_shows_per_repo_warnings}`; `packages/mod/src/room-args.test.ts`; `packages/mod/src/prompt.test.ts::section_is_byte_identical_across_states`; `packages/mod/plugin-tests/keys.test.tsx::{key_map_snapshot, band_never_names_a_spending_leaf, a_band_press_from_the_prompt_offers_undo, e_opens_the_canvas_with_a_person_token}`; `packages/mod/src/outbox.test.ts::reload_mid_write_makes_one_event`; `packages/mod/plugin-tests/tools.test.tsx`; `packages/ui/src/rows.test.ts::the_page_button_takes_the_next_free_row_key`.
  - Users: an agent reads each leaf's arguments in its tool's description and gets the same line back from a wrong argument; a person types `/room` with typeahead, undoes a band press with one key, and keeps a write across a reload.
- **Sources (checked 2026-10-29):** `crates/contract/src/{command, requests, view}.rs`; `crates/planroom/src/cli.rs` (`with_usage`); `crates/xtask/src/{generate, modgen}.rs`; `packages/mod/src/{tools, room, room-args, prompt, catalogue, layout}.ts` and `register.tsx`; `packages/mod/plugin-tests/host.ts`; `packages/ui/src/{keys, rows}.ts`; `crates/planroom/text/en.toml`; `budgets.json`; `vitest.config.js`; [The mod](architecture.md#the-mod); [Contracts](contracts.md#mod-tool-schemas-state-and-the-room-family).
