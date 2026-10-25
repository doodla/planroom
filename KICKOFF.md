# Planroom kickoff prompt

Paste everything below the line into a fresh Claude Code session on this repo.

---

<context>
I'm the only user of what you're about to build, and I want it built right: a complete, releasable open-source project on GitHub, not a demo. You have full autonomy. Make every product, design and technical call yourself, build it end to end, and ship it. Don't stop to ask me questions.

Environment: a cloud Claude Code container with this repo (`planroom`, MIT, owner Sridhar Vadlamani) checked out.
- Chromium and Playwright are pre-installed: `require('playwright')` works and `chromium.launch()` needs no options. Don't reinstall them.
- Read `CLAUDE.md` first. It sets rules for subagents (always set `model` and `effort`, and verify their output), and a hook enforces them.
- Outbound calls to `*.vercel.sh` (Vercel AI Gateway) have credentials injected by the proxy.
- `OPENAI_API_KEY` and `GEMINI_API_KEY` are set, for Iris (see below).
- Commit and push to the branch you were assigned.
</context>

<product_vision>
**Planroom turns a repo into a living, malleable plan.** Point it at a repo and you get a canvas where you can understand the system (its architecture, components, flows and dependencies), say what you intend to change, and shape that intent with Claude until it's a plan you believe in. Planroom then keeps the plan honest against the code as both evolve.

There are three surfaces over one shared model:

1. **The Claude Code mod is the guide.** It lives beside the chat and walks me through the work. Claude does the hard part, clarifying my intent: it checks, curates and extends the plan with me. The mod makes every step obvious and cheap. Max out this UX. It should have:
   - a **pane**: the current stage, the one open question with pre-answered choices, plan outline and progress, pending diffs, and reconcile status (drift since last check);
   - a **band above the prompt** with the single most useful next action;
   - status-line and toast feedback;
   - **slash commands**, such as `/plan`, `/plan map`, `/plan diff`, `/plan reconcile`, `/plan open`;
   - **tools the model calls** to read and change the model, so Claude in the chat drives the interview;
   - an **SVG mini-map** on surfaces that support `Svg`, and a clean text outline on the terminal;
   - **keyboard-first** controls everywhere the surface allows.
2. **The canvas web app is the surface for understanding and tweaking.** Zoom from the whole system down to modules and files. Overlay the plan on the architecture. Edit both directly: drag, re-parent, rename, re-scope, re-wire dependencies, mark the intended state. Everything is tweakable and undoable. It should be beautiful and fast on desktop and on my phone.
3. **The CLI / local server is the engine.** `npx planroom` (or the installed binary), pointed at one repo or a workspace of several: it scans, owns the state store (see `<state_store>`), serves the canvas locally with live updates, and exposes the API that every mod instance, agent and canvas tab uses at once.

   Cloud sessions can't reach localhost, so also publish the canvas as a **Claude Artifact** for remote viewing and editing. The local and remote modes share one codebase. Sync works like this:
   - The workspace store is the source of truth.
   - The Claude session pushes snapshots into the Artifact `db` with the ArtifactData tool.
   - Edits made in the Artifact are written as events to a `db` collection. The session imports them as user-provenance events, and any conflict becomes a diff item.
   - Sync runs on `/plan open`, at the start of each turn, and when an Artifact edit notice arrives.
   - The Artifact shows Jev decisions that are already cached, and queues new ones as pending until the next sync.
</product_vision>

<core_model>
Design this first, as a versioned contract (in the style of Iris; see `<release>`).

**The kernel is tiny and domain-free.** Everything Planroom knows is one typed property graph built from four primitives:
- **Entity**: `{ id, kind, level, parent?, props, layer, provenance }`.
- **Relation**: `{ id, kind, from, to, props, evidence[], confidence, layer, provenance }`.
- **Event**: an append-only change to entities or relations. Every other state is a projection of events.
- **View**: a saved query plus layout and overrides. A lens, a plan's canvas and a diff are all views.

`layer` separates **actual** (observed by extractors) from **intended** (what I and Claude say), so one graph holds both, and drift is a diff between layers. IDs are stable and namespaced (`<repo>:<kind>:<path-or-key>`), so the same entity is recognized across scans, repos and sessions.

**Domains are packs, not core code.** A pack is a small declarative file that adds entity kinds, relation kinds, the level each kind sits on, aggregation rules, extractors, lenses, Jev decision kinds and a default view. The core treats every pack the same.
- **`code`**: the repo, packages, services, modules, files and symbols; imports, calls, co-change.
- **`plan`**: the intent graph: goals, outcomes, deliverables, actions, risks, decisions; `breaks_down`, `touches`, `blocks`.
- **`observability`**: signals (metric, log, trace span), instrumentation points, dashboards, alerts and SLOs. Relations: `emits` (code → signal), `monitors` (dashboard or alert → signal), `guards` (SLO → service), `pages` (alert → owner). Extractors read the OpenTelemetry and logging calls in code, collector configs, Prometheus rules, and dashboard JSON. The pack answers questions like "how does observability work here?", "what's unmonitored?" and "what fires if this breaks?"
- **`release`**, across repos: repos, packages, versions, releases, environments and pipelines. Relations: `depends_on` (with version range and resolved version), `publishes`, `deploys_to`, `promotes`. Extractors read manifests, lockfiles, tags, changelogs and CI and CD workflows. The pack answers questions like "what must release, in what order, for this change to ship?", "who's on a stale version?" and "what breaks if I bump X?"

- **`artifacts`**: the things work is made of: prompts, docs, ADRs, specs, skills, agent configs, issues, PRs, commits, code reviews, review threads and CI runs.
  - Entities: document, section, instruction, constraint, requirement, check, example, step, trigger; and PR, commit, review, thread, comment, check-run.
  - Relations: `contains`, `requires`, `constrains`, `verifies`, `refines`, `contradicts`, `addresses` (commit or PR → thread or requirement), `reviews`, `blocks`.
  - Sources: local git always; GitHub via `gh` or the GitHub MCP tools when available. A skill is a document with triggers, steps, tools and scripts.
- **`workflow`**: state machines: machine, state (including nested and parallel), transition, event, guard and action. Relations: `transitions_to`, `triggered_by`, `guarded_by`, `performs`, `implements` (machine → code or artifact).

These six ship in v0.1 and prove the extension model. Writing a pack must not require touching the core, so add a guide, "Write a pack", plus a test that loads a toy pack from outside the source tree.

**Workspaces span repos.** A workspace is a set of repos, local paths or git URLs. When a manifest or lockfile in one repo names a package that another repo in the workspace publishes, the extractor resolves it to a cross-repo relation, so release and dependency views cross repo boundaries.

The sections below are the `code` and `plan` packs on this kernel.

- **System model.** Components, interfaces and dependencies, mapped to code (file globs, symbols). It comes in two layers:
  - **actual**: derived from scanning the repo with the language-agnostic extractor pipeline in `<language_agnostic_mapping>`;
  - **intended**: what I and Claude say it should be.
- **Abstraction levels.** Every system node lives on one level, and zoom moves between them:
  - **Intent**: goals and outcomes;
  - **System**: the repo and its external actors;
  - **Containers**: deployables, packages, services, apps;
  - **Components**: modules and features;
  - **Code**: files and symbols.

  This is the C4 model with intent on top. Each node has one parent on the level above. Edges carry a `kind` (imports, calls, data, deploys, co-change, owns), the `evidence` behind them (which extractor, file and line), and a `confidence`. When you zoom out, edges aggregate up the hierarchy, with counts and weights, so a higher level is always a faithful summary of the level below and never a separate drawing that can drift.
- **Intent graph.** Each plan node has a type (goal, outcome, deliverable, action, risk, decision), status, `doneWhen`, estimate, priority, provenance (user, Claude, scan or Jev) and an `accepted` flag. Nodes link to the system components they touch. Proposals from the AI stay ghosts until I accept them.
- **History.** The kernel's event log, with snapshots. Every change is an event with provenance, so any two versions can be diffed and any change undone.
- **Three diffs, one diff engine and one visual language:**
  - *plan versions*: what changed in the plan or model since version N, like git for the plan;
  - *intended vs actual*: where the intended architecture and the real code disagree;
  - *proposed code change*: the code diff an implementation would make, mapped onto canvas nodes.
- **Reconciler.** It re-scans, rebuilds *actual*, compares it to *intended* and to the plan, and produces drift items. Each item is classified as expected, in progress, done, unplanned or regression. It can run on demand, on git events, and when an agent finishes.
- **Hand-off.** When I approve a tweak that needs code, Planroom writes a self-contained change spec (intent, affected components, acceptance criteria, the diff of intended vs actual) and hands it to a **separate implementer agent**, behind an `Implementer` interface with two implementations:
  - the real one: an agent type the mod registers, with `isolation: 'worktree'`;
  - a deterministic fixture implementer that applies canned patches to `examples/`, for tests and CI.

  Either way, the result comes back as a proposed code diff for me to review. After merging, the reconciler closes the loop. Claude in the main chat stays the planning partner and never silently implements.
- **Context curator (a product feature).** Planroom does for the repo it's pointed at what you do for this one (see `<working_method>`). Using the system model and the history of implementer runs, it periodically proposes:
  - updates to the target repo's `CLAUDE.md` / `AGENTS.md`: stale facts, missing invariants, the component map in brief;
  - new or updated project skills for procedures agents keep repeating there. It spots these from repeated tool sequences in implementer runs and from the change specs.

  Proposals arrive as diffs for me to approve, like everything else. Jev decides cheaply whether a context entry is stale and whether a repeated pattern deserves a skill. This makes every future agent run on that repo cheaper, the implementer's included, which feeds back into keeping the canvas cheap to maintain.
</core_model>

<state_store>
The store serves many writers at once from day one: several Claude sessions, the mod in each, implementer and curator agents, Jev-driven automation, and canvas tabs on my laptop and phone. Many projects can be open in one workspace.

- **Scopes.**
  - A **workspace** holds repos and one shared graph.
  - **Projects** (plans) are named scopes inside it. Each has its own intent graph, views, decisions and history, and points at shared entities rather than copying them. Two plans touching the same service see the same service.
  - Every event carries `workspace`, `project?`, `actor` (user, a session id or an agent id, plus its kind), a `causedBy` chain and an idempotency key.
- **One writer, many clients.** The local server is the single serialization point for appends. Clients talk to it over HTTP for commands and queries, and over a live stream (SSE or WebSocket) for subscriptions filtered by scope, so every surface updates live.
  - If the server isn't running, the CLI and the mod start it on demand. A lock keeps it to one per workspace.
  - Writes use optimistic concurrency: a command names the entity versions it read, and a stale write is rejected with the current state, never silently merged.
- **Storage.** An embedded, transactional store, for example SQLite in WAL mode: events, projections and a decision cache, under a workspace directory.
  - Each repo also keeps a deterministic, git-friendly export in `<repo>/.planroom/`: intended layer, overrides, plans and views, as sorted JSON. That makes the parts I care about reviewable in pull requests. On startup and on pull, the server imports changes to those files as events.
  - Record this split in an ADR.
- **Agents and sessions are first-class.**
  - A registry of active sessions and agents, with heartbeats, what each is doing, and their **claims**: time-limited leases on the entities or plan nodes they're working on.
  - A claim doesn't block anyone. It warns, and the canvas and mod show presence: who's on what, with avatars or agent glyphs.
  - Implementer and curator runs record their tool sequences and outcomes as events. That history is what the context curator mines.
- **Conflicts are diffs.** Concurrent edits to the same field, or an edit that contradicts an override or a claim, become a diff item for me, or for the owning agent, to resolve. Nothing is last-writer-wins on user-authored state.
- **Queries.** A small, typed query API over the graph: traverse by kind and level, neighbourhood, paths, aggregates up the hierarchy, and diff between two versions or layers. Every view, lens, mod pane and Jev state slice uses it. Don't invent a query language. A typed builder plus a handful of named queries is enough.
</state_store>

<artifact_anatomy>
Planroom should break down any artifact the way you'd explain it to me. The canonical example: I asked a Claude session "what are the components of that prompt?" and got back a table of its sections, what each covers, and why it's there. Planroom does that for any artifact (a prompt, a skill, a PR, a code review, a doc, a CI workflow, a module) and keeps the result live and editable.

- **Anatomy view.** An artifact becomes a graph:
  - its parts, each with a one-line purpose and why it exists;
  - how the parts relate: which constrains which, which verifies which, where they contradict, what's missing.

  It's shown as both a structured outline (the table-like reading) and a graph, at every zoom level. For a PR or a review, the anatomy is:
  - intent, changes grouped by component, review threads and their state, checks, what's addressed and what's still open;
  - all linked to the system model, so I can see which services and dependencies a PR touches, and what it risks.
- **State machine extraction.** A first-class output of any breakdown. Planroom finds the machines implicit in code and prose and makes them explicit:
  - **From code:** enums plus switch or transition tables, reducers, explicit FSM libraries, status fields with guarded updates, and job and queue lifecycles.
  - **From config:** CI and CD workflows, deployment promotions, alert lifecycles.
  - **From prose:** "a PR goes from draft to review to approved", an agent's escalation policy, a prompt's phases, a skill's steps.

  Each machine renders as a **statechart view** in the component kit, with:
  - states, transitions, events, guards and actions;
  - evidence linked to source lines;
  - checks: unreachable states, dead ends, missing transitions for known events, nondeterminism, and drift between the prose version and the code version of the same machine;
  - a step-through simulator.

  Planroom's own loops, such as Jev escalation, implementer hand-off and reconcile, are modelled this way, and their machines are kept in sync with the code that implements them.
- **Round-trip.** Edits to a breakdown flow back to the source as a diff: reorder sections, tighten an instruction, add a missing transition, resolve a contradiction. Code and prose changes go through the implementer. Small text edits to a doc, prompt or skill can be applied directly once I approve.
- **How extraction works.**
  - **Deterministic parsing first**, wherever structure exists: markdown headings, XML-style tags, front matter, YAML and JSON, tree-sitter ASTs, GitHub objects.
  - **Claude** for semantic segmentation, purposes and relations.
  - **Jev** for the cheap, high-volume calls, for example "Is this paragraph an instruction, a constraint or an example?", "Does this commit address this thread?", "Is this a transition of machine M?" and "Do these two statements contradict?"
  - Every extracted part carries provenance and confidence, and the evidence popover shows it.
</artifact_anatomy>

<language_agnostic_mapping>
Planroom must work on any repo. The core never knows a language. Extractors emit facts into one normalized graph (nodes, edges, evidence, confidence), and a builder assembles the levels from those facts. Extractors stack in tiers, each optional, and each makes the map more precise:

1. **Universal**, for every repo:
   - the file and directory tree;
   - git history: co-change coupling (files that change together are related, in any language), ownership, churn;
   - manifests and lockfiles (package.json, Cargo.toml, go.mod, pyproject, pom/gradle, and so on) for packages and external dependencies.
2. **Syntactic**: tree-sitter grammars with per-language query files for definitions, references and imports (the same `tags.scm` style GitHub code navigation uses).
   - Adding a language means adding a query file and a small import-path resolver, not core code.
   - Ship TS/JS, Python, Go and Rust in v0.1.
3. **Precise, when available**: SCIP/LSIF indexes or a language server, when the repo has them, for exact cross-references.
4. **System and config**: Dockerfiles, compose files, k8s manifests, Terraform, CI workflows, OpenAPI, protobuf and GraphQL schemas. These reveal containers, services and data edges that code imports can't.
5. **Inference**:
   - Claude proposes component groupings, names and one-line purposes.
   - Jev makes the cheap calls at scale, for example "Is this directory a component boundary?", "Which level is this?" and "Is this co-change edge meaningful?".
   - Inferred facts are marked as inferred, with their confidence.

Each extractor declares what it emits and how confident it is. The UI shows evidence on demand ("why do you think A depends on B?"). The reference docs list per-language support. A repo in an unsupported language still gets a useful map from tiers 1, 4 and 5.
</language_agnostic_mapping>

<user_surface>
Keep the set of things I have to know about tiny, and hold it there as the product grows. Write it down in `docs/concepts/` as the entire mental model, and treat it as a public contract:
- **Levels**: where I am on the zoom.
- **Lenses**: which relationships I'm looking at.
- **Intent**: what I want, grouped into projects inside a workspace.
- **Diffs**: what's different.
- **Decisions**: what needs me.

That's the whole vocabulary. One CLI entry point (`planroom`) and one slash command family (`/plan …`) with good defaults. Extractors, scanners, the event log, Jev thresholds and caches stay internal.

My corrections to the map, such as regrouping components, renaming, hiding noise or pinning a layout, are stored as overrides (events in the store, exported to `.planroom/` for review). They survive every rescan. I make them by direct manipulation on the canvas or by asking Claude, never by editing config. A rescan that contradicts an override becomes a diff, not an overwrite.

Before you add a concept, a flag or a setting, check whether a better default would remove the need for it.
</user_surface>

<cheap_decisions_with_jev>
Planroom has to stay cheap to maintain, or it rots. Its big lever is Jev, TypeSafe AI's decision model. Jev answers typed questions (boolean, choice, score) about a piece of state, with calibrated probabilities. It costs about $0.04 per million input tokens and answers in milliseconds.

**What Jev decides.** Route every small, frequent decision through Jev:
- Is this drift item expected?
- Does this node still match the code?
- Is this leaf actionable?
- Which stage or question should come next?
- Does this tweak need code, or is it plan-only?
- How should these suggestions rank?
- Is this auto-accept safe?
- The plan-lint checks.

**When to escalate.** Use the answer's probability and confidence:
- Above a threshold (configurable per decision kind), act or pre-fill.
- In the uncertain band, show it to me as a one-tap card with Jev's pick pre-selected.
- Only when that isn't enough, escalate to Claude.

Make this explicit in code, so every decision kind declares its question, its thresholds and its escalation path.

**How to call it:**
- One `decide()` adapter supporting both of these:
  - **Vercel AI Gateway.** `POST https://ai-gateway.vercel.sh/v1/evaluate`, with body `{ model: "typesafe-ai/jev", state, questions }`. The question types are `boolean`, `choice` (`criteria`: a record of option names to descriptions) and `score` (`criteria`: an array ordered lowest to highest). Auth is `AI_GATEWAY_API_KEY` locally; in this container the proxy injects it. The AI SDK equivalent is `experimental_decide`. The gateway also serves a TypeSafe-compatible API at `https://ai-gateway.vercel.sh/typesafe`. Docs: https://vercel.com/docs/ai-gateway/modalities/decision
  - **Direct TypeSafe.** `POST https://api.typesafe.ai/v1/systemone`, with `Authorization: Bearer $TYPESAFE_API_KEY` and `model: "jev-latest"`. The boolean type is named `noul` there, its answer is `{ noul: <p> }`, and usage is snake_case. Docs: https://docs.typesafe.ai/llms.txt
- Normalize both into one result type and pin a model version in config. Before you code against the direct API, read the official docs. Ignore thejevai.com: it is unaffiliated.
- Limits: 32k tokens for state plus the longest question, and 64k per request. So send scoped slices of the model, never the whole repo, and batch several questions per call against the same state.
- Cache answers by hash of state and question.
- Record every decision (inputs hash, answer, probability, what was done) in the history, so I can see and override why something happened.
- Build an eval set in `evals/decisions/*.json`, with at least 25 realistic cases per decision kind. You label them, with a one-line rationale per label.
  - Enable automatic action for a kind only if its precision is at least 0.95 at the chosen threshold.
  - Below that bar, the kind defaults to the one-tap card.
  - Report the table at the end.
- Test direct TypeSafe mode against fixtures built from its docs, plus a live smoke test that runs only when `TYPESAFE_API_KEY` is set (it isn't set here).
- Spend cap: keep total live Jev, gateway and `sample` spend during the build under $3. Gate every live test behind an env flag.

I verified this live call from this container. It returned a choice with probabilities and a confidence, in about 50 ms, for about $0.000016:

```sh
curl -sS https://ai-gateway.vercel.sh/v1/evaluate -H "Content-Type: application/json" -d '{"model":"typesafe-ai/jev","state":{"node":{"title":"Figure out auth","type":"action","doneWhen":""}},"questions":{"actionable":{"type":"boolean","instructions":"Is this leaf a concrete physical next action starting with a specific verb?"},"lens":{"type":"choice","instructions":"Which edit is most needed?","criteria":{"rewrite_verb":"title uses a vague verb","add_done":"missing done-when test","ok":"no change needed"}}}}'
```

Heavier reasoning goes to Claude:
- the interview and decomposition;
- writing change specs;
- explaining architecture.

How Claude is reached depends on the surface: in the mod, through the chat model and the mod's model APIs; in the Artifact, through `sample`; locally, through the gateway.
</cheap_decisions_with_jev>

<planning_philosophy>
Usability is the product. Every surface follows one rule: **always help me advance my intent with the least effort.**

- **Never a blank state.** Every view opens with a sensible default already made: a first map of the repo, a drafted goal, a proposed next step.
- **Decisions arrive pre-decided.** A card shows the recommended answer, why, and how confident Planroom is. Confirming takes one tap or key, and overriding takes one more. Ask open-ended questions only when no reasonable default exists.
- **One obvious next action** at a time, always visible. Everything else is a step away (progressive disclosure).
- **Everything is reversible** (undo, history, diff), which is what makes bold defaults safe.
- **Make reasonable choices on my behalf** when the cost of being wrong is low, and say what you chose. Escalate only when it matters.
- **Respect my edits.** A user edit is never silently overwritten by a scan or an AI. A conflict becomes a diff for me to resolve.

The interview uses established methods in stages I can jump between at any time:
1. **Intent**: Jobs-to-be-Done, Socratic questioning, laddering until my answer stabilizes (2–3 rounds of "why", not a forced five).
2. **Outcome**: SMART measures, plus non-goals.
3. **Breakdown**: issue tree / MECE, the WBS 100% rule, or an Impact Map or Story Map, whichever lens fits the task. Name the lens you used.
4. **Next actions**: GTD and HTN. Each leaf is a verb-first physical action with a `doneWhen`, sized at a day or less.
5. **Prioritize**: MoSCoW plus a walking-skeleton MVP line.
6. **Pre-mortem**: Klein's method. Each risk becomes a node with a mitigation.
7. **Lint**, in two kinds:
   - **Structural, computed exactly in code:**
     - each parent has 2–7 children;
     - there are no dependency cycles;
     - every leaf traces up to the outcome;
     - every Must branch has a risk;
     - every leaf has a done-test;
     - unconfirmed inferences are flagged by provenance.
   - **Semantic, as cheap Jev checks:**
     - children cover 100% of their parent's scope;
     - siblings are MECE;
     - every leaf is actionable;
     - the goal is SMART;
     - siblings are at a similar granularity.

For a repo, the system model grounds every stage. Breakdown nodes attach to real components, and next actions name the files they touch.
</planning_philosophy>

<design_direction>
Bold and playful, with craft: a tactile, toy-like planning table for engineers, not a corporate PM tool. Commit to one strong concept and carry it through the canvas, the mod and the README art.
- **Typefaces**: distinctive ones from Google Fonts. Don't use Inter, Roboto, Arial, system-ui or Space Grotesk.
- **Palette**: a cohesive palette as CSS variables, and a dark mode that is designed, not inverted.
- **Motion**: put the budget into a few moments: nodes blooming in, a ghost being accepted, a diff animating between versions, a branch completing, drift resolving. Respect `prefers-reduced-motion`.
- **Don't use**: a cream or off-white background, purple-on-white gradients, italic accent words in headlines, numbered "01/02/03" section labels, monospace labels as decoration, pill-shaped buttons everywhere, or a generic three-card hero layout.
- **Mobile first**: a bottom-sheet guide, pinch-zoom and pan, thumb-reachable controls, large touch targets, and no horizontal page scroll.
- **The mod**: within its element set it should feel as considered as the canvas, with tight layout, clear hierarchy, and the next action always one key away.
- **Components for understanding relationships.** Design these as a coherent kit, the heart of the canvas, each as considered as a Figma or Linear primitive:
  - **Abstraction zoom**: a level control (a slider or rail with a stop per level, and a breadcrumb such as `System › api › auth › session.ts`). It's driven by pinch, scroll or keys. Zooming is semantic, not just scale: children fold into their parent with a motion that shows where they went, and edges bundle into weighted aggregates with counts. "Expand here" opens one node a level deeper in place.
  - **Lens switcher**: imports, calls, data, deploy, co-change, ownership, plan. Lenses combine, each has a legend, and a sensible default is always on.
  - **Focus / ego view**: select a node to dim everything outside its N-hop neighbourhood, with dependencies on one side and dependents on the other.
  - **Path finder**: "How does A reach B?" Highlight the shortest paths, with evidence on each hop.
  - **Impact radius**: show what a tweak or diff touches, transitively, with a heat ramp by distance. It's used by the diff and hand-off views.
  - **Dependency matrix (DSM)**: an alternate view for dense graphs, with cycles and layering violations highlighted. It stays in sync with the selection on the graph.
  - **Anatomy outline**: the table-like breakdown of an artifact (part → purpose → why → relations), synced with the graph selection.
  - **Statechart view**: nested and parallel states, animated transitions, the simulator, and check results inline.
  - **Evidence popover**: on any edge or inferred node, show why it exists (extractor, file and line, confidence), and let me confirm or reject it.
  - **Minimap and search**: fuzzy search across every level that jumps and zooms to the result.

  Lay out with a hierarchical, stable algorithm (for example ELK layered or compound layout), so nodes don't jump around between scans or zoom levels. Positions I pin are stored as overrides.
- **Measurable bars:**
  - every mod action is reachable in 2 keys or fewer;
  - the canvas is interactive in under 1 s on the fixture;
  - pan and zoom stay smooth (about 60 fps) at 200 nodes;
  - no layout overflows at 390 px;
  - every edit type has an undo test.
</design_direction>

<runtime_notes>
- **Mod.** Load the `plugin-authoring` skill before you write any mod code. It defines the folder layout, the types (grep them; they are the reference), validation and tests, and the one-time "turn on mods" question that I'll answer.
  - The mod API includes `$.http.fetch` (Jev and the local server), `$.process.spawn` (git, the CLI), `$.fs`, `$.store` / `$.state`, `$.tool.register`, `$.agent.register` / `$.agent.spawn` (the implementer agent), `$.command.register` and `$.ui.*`.
  - Panes use a fixed set of elements (Box, Text, Button, Svg on non-terminal surfaces, Markdown, and so on), not a DOM. The full canvas is the web app's job, and the mod links to it.
  - Commit the mod's source under `mod/`, and also install it into this session's mods folder so it hot-loads here.
- **Artifact.** Load `artifact-design` and `artifact-capabilities` before you write the published page, and follow their contracts: allowed CDN hosts, theme tokens, `db`, `sample`, and graceful degradation when a capability is absent. Its pages can't call arbitrary hosts, so Jev runs on the mod and server side.
- **Stack.** Your call. Choose boring, well-supported tools, and use TypeScript end to end so the schema is shared by the CLI, web and mod. Keep the code minimal: no speculative abstractions, no defensive code for impossible cases, and no hard-coding to pass tests.
</runtime_notes>

<release>
Ship it the way `doodla/iris` is shipped. Clone it with `GIT_LFS_SKIP_SMUDGE=1 git clone --depth 1 https://github.com/doodla/iris` and read its `README.md`, `AGENTS.md`, `CONTRIBUTING.md`, `SECURITY.md`, `CHANGELOG.md`, `docs/` and `.github/`. Match its bar and its documentation style guide (`docs/contributing/style-guide.md`):
- Public contracts are versioned, with a JSON schema committed and enforced by a test: the event and entity schemas, the pack format, the `.planroom/` export format, the local API, the CLI, `--json` output, the mod's tool schemas and the local API.
- The README is a landing page: what Planroom is, a hero image, install, a first result in under 2 minutes, and links to the rest.
- Docs are split into guides, concepts, reference and contributing. Each fact has one home.
- `AGENTS.md` (with `CLAUDE.md` importing it) holds the invariants.
- Include `CONTRIBUTING.md`, `SECURITY.md`, `CODE_OF_CONDUCT.md`, a `CHANGELOG.md` in Keep a Changelog format, issue and PR templates, and Dependabot.
- **CI**:
  - Runs lint, format check, typecheck, unit tests, Playwright end-to-end tests (using the fixture implementer), `claude plugin validate` and the mod's tests, and a schema-drift check.
  - Triggers on push to any branch as well as on PRs. Watch it with `gh run list` / `gh run watch`.
  - Pin the Claude Code version that's installed here (`claude --version`) for the mod jobs. Confirm those commands run without credentials. If they can't, gate the job and record why in `progress.md`.
- **Release workflow**: a tag produces a GitHub release, and npm publishing runs as a dry run until I add a token. Write the `v0.1.0` release notes, but don't push the tag.
- **Mod install**: the mod is installable from the repo. `.claude-plugin/marketplace.json` lives at the repo root, and its plugin `source` points at `./mod`. The README install lines are `/plugin marketplace add doodla/planroom`, then `/plugin install planroom@<marketplace name>`.
- Add a demo fixture repo under `examples/` that the end-to-end tests and screenshots use.
- **Images come from Iris**, my CLI: logo, README hero, GitHub social preview (1280×640), and empty-state illustrations.
  - Install it with `curl -fsSL https://raw.githubusercontent.com/doodla/iris/main/install.sh | sh` (it goes into `~/.local/bin`), then run `iris doctor`.
  - Iris never picks a model. Choose one from `iris models list` and pass `-m`.
  - Generate at a supported size, then crop (for example to 1280×640).
  - Always `--dry-run --json` first to see the cost, and pass `--max-cost`.
  - Draft at low quality, then finalize the chosen ones.
  - Keep total image spend under $5. Commit the prompts next to the images, so they can be regenerated.
</release>

<working_method>
Build it right, in phases. Commit and push after each meaningful step.
1. **Design.**
   - Write `docs/contributing/architecture.md` and short ADRs in `docs/contributing/decisions.md`: stack, store format, diff engine, reconciler, Jev decision policy, mod ↔ server ↔ canvas protocol, and local vs Artifact modes.
   - Define the schemas, plus `feature_list.json` with one entry per user-visible feature, each with `passes: false` and a `tier` (`must`, `should` or `could`), and `progress.md` for decisions, assumptions and what's next.
   - The `must` tier includes at least everything in the Playwright flow under `<verification>`, plus:
     - the kernel plus the four packs (code, plan, observability, release), and a toy pack loaded from outside the source tree;
     - a multi-repo workspace with cross-repo dependency and release views;
     - multiple concurrent writers: two sessions or agents plus a canvas tab on two projects, with live updates, claims and presence, and a stale write turned into a diff;
     - artifact anatomy for a prompt, a skill, a PR with its review threads, and a CI workflow;
     - state machine extraction from code, config and prose, in the statechart view with checks and the simulator;
     - round-trip edits from a breakdown back to source as a diff;
     - abstraction zoom across all five levels, with edge aggregation;
     - lenses, focus view, path finder, impact radius and evidence popovers;
     - extractor tiers 1, 2 (TS/JS at least) and 4, working on the fixture;
     - overrides that survive a rescan;
     - the context curator proposing at least one `CLAUDE.md` diff and one skill on the fixture;
     - mod pane, band, commands and tools;
     - Jev decisions with escalation;
     - the three diff views;
     - reconcile;
     - undo;
     - Artifact publish with sync;
     - the release files.
   - Never merge, weaken or delete a `must` entry once it's written.
   - Before building, get an independent review of the design from a subagent at your own model level. Give it the documents, not your conclusions, and address its findings.
1b. **Walking skeleton.** Build one thin slice through every surface before deepening any of them: scan the fixture, then a node in the store, then the mod pane showing it, then the canvas showing it, then the Artifact publishing it from `db`. This raises the "turn on mods" and Artifact consent prompts early, while I'm likely still around, and it shows you the real constraints of each runtime before the design hardens. Adjust the design docs to match what you learn.
2. **Engine.** Schema, store, event log, diff engine, scanner, reconciler, the Jev adapter and its eval set, and the CLI. Use test-driven development where it pays.
3. **Canvas.** Map, plan overlay, direct editing, the three diff views, reconcile view, the guided interview, then polish and mobile.
4. **Mod.** Pane, band, commands, tools, and the implementer agent hand-off loop.
5. **Integration.** Local live mode end to end, then Artifact publish and `db` sync.
6. **Release.** Docs, Iris art, CI, release workflow, changelog, `v0.1.0` tag notes. Then a final independent review of the whole repo against this prompt, as with the design review, and fix what it finds.

- **Context review and skill acquisition, for your own work.** Do this at the end of every phase, and whenever you notice yourself repeating a multi-step procedure:
  - Review the repo's agent context (`AGENTS.md`, `CLAUDE.md`, `docs/contributing/`, `progress.md`). Correct anything stale, cut anything that no longer earns its tokens, and add the invariants you've learned.
  - Distill any procedure you've done twice into a project skill under `.claude/skills/<name>/SKILL.md`, with any scripts it needs. Examples: run the end-to-end tests and review screenshots, add a decision kind with its eval cases, add a language extractor, regenerate the schemas, make Iris art.
  - Use the `skill-creator` skill to write and test the skills.
  - Log each review in `progress.md`: what changed, which skills were added, and what they'll save next time.

  The goal is that repeat agent work on this repo gets simpler and cheaper over time, for you and for every later session.
- Work one feature at a time: verify it, flip its flag, commit. Never delete or weaken a test to make it pass.
- Use parallel tool calls for independent work. Use subagents for large independent chunks, following `CLAUDE.md`, and verify what they report before you rely on it.
- Context compacts automatically, so don't wind down early to save tokens. Keep `progress.md` current so any later session can resume.
</working_method>

<verification>
- Unit tests cover the schema, diff engine, reconciler classification, decision policy (with a mocked `decide()`), lint and interview state machine. A small live Jev smoke test is gated behind an env flag.
- **Anatomy and state machine checks.**
  - Run anatomy on this very prompt (`KICKOFF.md`). The parts should match its sections, with sensible purposes, and at least the phase and autonomy machines extracted.
  - On the fixture:
    - seed an order or job lifecycle in code, plus a doc that describes it slightly differently. Planroom must extract both and flag the drift;
    - make a PR with review threads, where a later commit addresses one of them. Planroom must show which threads are addressed and which are open.
- **Concurrency test.** Run two agents and a Playwright canvas tab against one workspace and two projects, writing at the same time. Assert:
  - no lost events;
  - idempotent retries;
  - live updates within a second;
  - claims and presence are visible;
  - a stale write becomes a diff.
- The real implementer hand-off is covered by `claude plugin test`. Also run it once live in this session, and record the output as evidence.
- Make `examples/` a small polyglot **workspace of two or three repos**, so the language-agnostic pipeline and the packs are actually exercised:
  - a TS web app, a Python worker and a Go service, wired by docker-compose and an OpenAPI spec;
  - a shared library one repo publishes and another consumes at a stale version;
  - OpenTelemetry instrumentation, a Prometheus alert rule and a dashboard JSON;
  - tags and changelogs.

  Ask the "how does observability work here?" and "what must release, in what order, to ship this change?" questions, and check the answers against the fixture's known truth. Run Planroom on it and on Planroom's own repo. In the fixture, introduce a real code change and confirm the reconciler reports the correct drift.
- Drive the canvas with Playwright as a person would. Do one full flow: open repo → map appears → set intent → break down → tweak the intended architecture → see the intended-vs-actual diff → hand off to the implementer → review its code diff → merge → reconcile shows the result as resolved.
- Screenshot every main view at 1440×900 and 390×844, in light and dark mode. Look at them, critique them against the design direction and the planning philosophy (is the next action obvious? is any decision costlier than it needs to be?), fix, and re-shoot. There should be no console errors.
- For the mod: `claude plugin validate`, a type-check, and `claude plugin test` covering the commands, tools, pane and band renders, and the hand-off.
</verification>

<definition_of_done>
- Every `must` entry in `feature_list.json` passes. List the `should` and `could` entries that don't pass as known gaps.
- CI is green on the pushed branch.
- The packed CLI works on a fresh clone of the fixture: test with `npm pack`, then `npx ./planroom-*.tgz`. The npm name isn't claimed yet, so list claiming it among my clicks.
- The canvas is published as an Artifact and I have its link. The mod validates, passes its tests, and is loaded in this session.
- The repo is release-ready per `<release>`. Screenshots are committed under `docs/screenshots/`, and the Iris art and its prompts are committed.
- Everything is committed and pushed.
- The final message leads with the Artifact link and what works, shows the evidence (test and CI output, screenshot paths, Iris spend, Jev decision eval results), then lists known gaps honestly.
</definition_of_done>

<autonomy>
A standing instruction from the user, the person you are working for. It is about how your turns end. A message with no tool call in it ends your turn, and the work stops there until you are asked to continue. The user has seen you end turns in four ways while work they asked for was still owed, and does not want any of them. One: a long summary of what was done that closes by announcing the next step and has no tool call, so the next thing never starts. Two: an offer to carry on with something unless the user would prefer otherwise, which stops to wait for an answer the user was not going to give. Three: a list of decisions for the user when, by your own account, none of them blocks the rest of the work. Four: deciding that this is a good place to report, because the turn has been long or a milestone is done. Status notes are welcome, and so are your recommendations on open decisions, but put them in the same message as your next tool call and carry on with whatever does not depend on the user's answer. If you notice yourself inviting the user to redirect you or offering to wait, delete it and do the next thing. The stops the user does want are the ones where nothing can move without them, or where the thing blocking you is deliberately protected from you. This does not override the need for confirmation on risky or destructive actions.

Make reasonable assumptions and record them in `progress.md` instead of asking. The only prompts that need me are the Artifact consent dialogs and the "turn on mods" question. I'll answer them when they appear, so keep working on whatever doesn't depend on them. Don't publish to npm, create GitHub releases, or change repo settings. Prepare them, and list in your final message exactly what I need to click.
</autonomy>
