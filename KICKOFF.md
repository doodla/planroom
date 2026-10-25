# Planroom kickoff prompt

Paste everything below the line into a fresh Claude Code session on this repo. Put the word `ultracode` in the same message to turn on multi-agent workflows (see `<process>`).

---

<context>
I'm the only user of what you're about to build, and I want it built right: a complete, releasable open-source project on GitHub, not a demo. You have full autonomy. Make every product, design and technical call yourself, build it end to end, and ship it. Don't stop to ask me questions.

This prompt has six kinds of content. Treat each differently:
- **`<requirements>` and `<acceptance_scenarios>`** say *what* Planroom must do and *why*. They are binding. The scenarios are the real specification.
- **`<constraints>`** are binding too: choices I've made, and limits you must stay within.
- **`<verified_facts>`** are things I checked in this environment. Rely on them instead of rediscovering them.
- **`<design_starting_points>`** are my current best guesses at *how*. They are not binding. Override any of them when you find something better, and record why in a decision record.
- **`<process>`** says how to plan, review and build.

Environment: a cloud Claude Code container with this repo (`planroom`, MIT, owner Sridhar Vadlamani) checked out. Read `CLAUDE.md` first: it sets rules for subagents, and a hook enforces them. Commit and push to the branch you were assigned.
</context>

<principles>
Two ideas shape every decision. When a requirement is unclear, resolve it in their favour.

1. **Respect human attention.** Human attention is the scarcest resource in the system. A person should be glad Planroom exists, because it gives them understanding and control at a fraction of the effort.
   - Interrupt a person only for decisions that only a person can make.
   - Make those decisions cheap: pre-decided, batched, explained in one line, one tap to confirm.
   - Never make someone read what a summary, a diagram or a diff could show them instead.
   - Never show a blank state. Never ask a question a good default could answer. Never lose or overwrite what someone did.
   - Everything is reversible, which is what makes bold defaults safe.
2. **Agent-native.** Planroom's code is written by agents, for agents, and run mostly by agents. People steer through the mod and the canvas, while agents do the reading, extracting, reconciling and implementing.
   - Every capability is reachable by an agent through a stable, typed, documented interface: CLI commands with `--json`, the local API, and the mod's tools. Each has stable error codes.
   - The repo itself is easy for agents to work in:
     - `AGENTS.md` invariants;
     - small modules with clear contracts;
     - deterministic commands;
     - fast tests;
     - project skills for repeat procedures;
     - context that is kept current.

   Planroom does the same for the repos it is pointed at.
</principles>

<requirements>
Each requirement must be traceable to a design element and to a test.

**R1. Point it at work, get understanding.** Point Planroom at a repo, or a workspace of several repos, and it builds a map of the system that someone can understand at a glance. They can then move up and down levels of abstraction, from intent down through systems, deployables and components to code, and every higher level is a faithful roll-up of the levels below it. It works on any language. Precision may vary by language, but no repo gets nothing, and adding a language never requires changing the core.

**R2. Understand relationships.** People can see and question how things relate: dependencies, dependents, data flow, deployment, co-change, ownership, and the plan's own links. They can ask "how does A reach B?", "what does this touch?" and "why do you think A depends on B?". Every inferred fact shows its evidence and confidence, and a person can confirm or reject it.

**R3. Break down any artifact.** Prompts, docs, decision records, specs, skills, agent configs, issues, PRs, commits, code reviews, review threads, CI workflows and modules can all be broken into their parts. Each part gets its purpose, why it exists, and how it relates to the other parts (constrains, verifies, refines, contradicts, addresses), plus what's missing. The model for this is a Claude session answering "what are the components of that prompt?" with a clear table. For PRs and reviews, the breakdown shows which threads are addressed and which are open, and which parts of the system the change touches.

**R4. Extract state machines.** Planroom finds the state machines implicit in code, config and prose, and makes them explicit and explorable: states, transitions, events and guards, with evidence. It checks them for unreachable states, dead ends, missing transitions and nondeterminism, and lets someone step through a machine. When prose and code describe the same machine differently, that drift is flagged.

**R5. Clarify intent, top down.** With Claude as a partner, a person turns a fuzzy intent into a plan they believe in. Planning always runs top down: intent, then outcome, then breakdown, then next actions, then priorities, then risks. It uses established methods (for example Jobs-to-be-Done, laddering, SMART outcomes, MECE or WBS breakdown, GTD next actions, MoSCoW, pre-mortem), and the person can jump between stages. Claude checks, curates and extends the plan. The AI proposes and the person disposes: nothing the AI invents counts until a person accepts it. Plans attach to the real system, and next actions name what they touch.

**R6. Tweak everything.** The map, the plan and any artifact breakdown can be shaped directly, by hand on the canvas or by asking Claude: regroup, rename, re-scope, re-wire, hide noise, pin a layout, edit a breakdown. Corrections survive every rescan. A scan never silently overrides a person's edit; a conflict becomes a diff for them to resolve.

**R7. Diffs and reconciliation.** There are three kinds of diff, with one diff engine and one visual language:
- **plan and model versions**, like git for the plan;
- **intended vs actual**, where what people say the system should be and what the code is disagree;
- **proposed changes**, code or text, mapped onto the map.

A reconciler compares intent with reality on demand and on change, and classifies each mismatch as expected, in progress, done, unplanned or regression.

**R8. Hand-off.** An approved change that needs code or text edits becomes a self-contained change spec, handed to a **separate implementer agent** that works in isolation. Its result comes back as a diff for review, and reconciliation closes the loop after merge. The planning partner in the chat never silently implements.

**R9. Cheap decisions.** Keeping the map and plan current must be nearly free, or it rots. Small, frequent decisions go to Jev, a cheap decision model:
- above a measured confidence bar, Planroom acts or pre-fills;
- in the uncertain band, it shows a one-tap card with Jev's pick pre-selected;
- only beyond that does it escalate to Claude.

Every decision kind has a declared policy and an eval behind its threshold. Every decision is recorded and can be inspected and overridden.

**R10. Many agents, sessions and projects at once.** One workspace can serve several Claude sessions, the mod in each, implementer and curator agents, automation, and canvas tabs on several devices at the same time. It can hold several projects (plans) that share one model of the system. No write is ever lost, retries are safe, every surface updates live, and everyone can see who (person or agent) is working on what. User-authored state is never last-writer-wins.

**R11. Extensible domains.** The core data model is simple and domain-free, and extends into a graph. New domains plug in without core changes. v0.1 must cover, as domains on that model:
- code;
- plans;
- observability ("how does observability work here? what's unmonitored? what fires if this breaks?");
- release management across repos and dependencies ("what must release, in what order? who's on a stale version? what breaks if I bump X?");
- artifacts (R3);
- workflows and state machines (R4).

**R12. Context curation and skills.** Planroom keeps the agent context of the repos it's pointed at accurate and lean, so repeat agent work gets simpler and cheaper. It proposes updates to `CLAUDE.md` and `AGENTS.md`, and new or updated project skills for procedures agents keep repeating. It spots those from the history of implementer runs. Proposals arrive as diffs for approval.

**R13. Surfaces.** There are three, sharing one model:
- **A Claude Code mod that guides.** It lives beside the chat. Max out its UX: the current focus, the one open decision with pre-answered choices, progress, pending diffs, drift, and who's working on what. It also has a band above the prompt with the next best action, slash commands, tools the model calls, and a mini-map where the surface can draw one. It is keyboard-first.
- **A canvas web app** for understanding and tweaking. It's beautiful and fast on desktop and on a phone. It runs locally, and is also published as a Claude Artifact for when localhost isn't reachable, for example from a cloud session.
- **A CLI and local server, the engine**, which agents use directly.

**R14. A tiny surface to learn.** The whole mental model a person needs is five ideas: **Levels** (where I am), **Lenses** (which relationships I see), **Intent** (what I want, grouped into projects), **Diffs** (what's different) and **Decisions** (what needs me). There is one CLI entry point and one slash-command family. Everything else stays internal. Before adding any concept, flag or setting, check whether a better default would remove the need for it.

**R15. Releasable.** The repo is ready for a public v0.1 release, at the standard of `doodla/iris` (see `<constraints>`).
</requirements>

<acceptance_scenarios>
These define done. Each becomes a `must` entry in `feature_list.json` and an automated test where automation is possible. For the rest, record evidence (screenshots, logs). Build the fixture they run on first.

**The fixture.** `examples/` is a small polyglot workspace of two or three repos:
- a TS web app, a Python worker and a Go service, wired by docker-compose and an OpenAPI spec;
- a shared library that one repo publishes and another consumes at a stale version;
- OpenTelemetry instrumentation, a Prometheus alert rule and a dashboard JSON;
- an order or job lifecycle in code, plus a doc that describes it slightly differently;
- tags and changelogs, and a PR (local branches are enough) with review threads, one of which a later commit addresses.

Write the fixture's known truth down, so the answers can be checked against it.

**S1. First look.** Run `planroom` on the fixture. Within seconds, someone sees a map at the system level with a sensible default lens and one suggested next action. Zooming to code and back shows faithful roll-ups. Search jumps to any entity.

**S2. Questions.** Answer, correctly against the fixture's truth:
- "How does observability work here?"
- "What's unmonitored?"
- "What must release, in what order, to ship a change to the shared library?"
- "Who's on a stale version?"
- "How does the web app reach the worker?" (each hop with its evidence)

**S3. Top-down plan.** "Add retries to the worker" is taken from intent through all planning stages to next actions attached to real components. AI proposals stay ghosts until accepted. Lint flags a vague action and a missing done-test.

**S4. Tweak and hand off.**
1. Regroup two components, and change the intended architecture.
2. The intended-vs-actual diff appears, with its impact radius.
3. Approve it. A change spec goes to the implementer, the code diff comes back for review, and you merge it.
4. Reconciliation shows the item resolved.
5. A rescan keeps the regrouping.

**S5. Artifact anatomy.**
- Break down `KICKOFF.md` (this prompt). Its parts match its sections, with sensible purposes and relations.
- Break down the fixture PR. Addressed and open threads are correct, and the touched components are shown.
- Break down one skill and one CI workflow.
- Edit a breakdown, and see the result come back as a text diff to the source.

**S6. State machines.** Extract the fixture lifecycle from code and from the doc, and flag the drift between them. Extract the machines in this prompt (phases, autonomy). Planroom's own escalation and hand-off loops are modelled, and match its code. The checks and the step-through work.

**S7. Cheap decisions.** Every decision kind has a policy and an eval set: at least 25 labelled cases, with a rationale for each label. Automatic action is enabled only for kinds at 0.95 precision or above; the rest fall back to one-tap cards. Report the table at the end. Decisions can be inspected and overridden.

**S8. Concurrency.** Two agents and a canvas tab write at the same time to two projects in one workspace. Check that:
- no events are lost;
- retries are idempotent;
- live updates arrive within one second;
- claims and presence are visible;
- a stale write becomes a diff, not a merge.

**S9. Mod.** In a live session, every main flow can be driven from the mod: pane, band, commands and tools. Every action is reachable in two keys or fewer. The implementer hand-off runs once for real, and the output is recorded.

**S10. Extensibility.** A toy domain pack, loaded from outside the source tree, adds new kinds, a lens and a decision kind without any core change. A new language works through the extractor contract alone.

**S11. Remote.** The canvas is published as an Artifact and kept in sync with the workspace. An edit made there comes back as a user event.

**S12. Context curation.** On the fixture, Planroom proposes at least one `CLAUDE.md` diff and one skill, based on the history of implementer runs.

**S13. Quality bars.**
- The canvas is interactive in under 1 s on the fixture.
- Pan and zoom stay smooth (about 60 fps) at 200 nodes.
- Nothing overflows at 390 px.
- Light and dark themes are both designed.
- There are no console errors.
- Every edit type has an undo test.

**S14. Release.**
- CI is green on the pushed branch.
- A fresh clone of the fixture works with the packed CLI (`npm pack`, then `npx ./planroom-*.tgz`).
- The docs, art, changelog and release notes are complete per `<constraints>`.
</acceptance_scenarios>

<constraints>
- **Surfaces**: a Claude Code mod, a canvas web app also published as a Claude Artifact, and a CLI with a local server. The implementer is a separate agent.
- **Jev** (TypeSafe AI) is the cheap-decision model. Support both the Vercel AI Gateway and direct TypeSafe connections behind one adapter, with a pinned model version. Ignore thejevai.com: it isn't affiliated with TypeSafe.
- **Iris** (my CLI, `doodla/iris`) makes all images: logo, README hero, a 1280×640 social preview, and empty-state art. Total spend must stay under $5. Commit the prompts next to the images.
- **Look and feel**:
  - Bold and playful, with craft. The image is a tactile, toy-like planning table for engineers, not a corporate PM tool. Commit to one concept and carry it through the canvas, the mod and the art.
  - Use distinctive typefaces. Don't use Inter, Roboto, Arial, system-ui or Space Grotesk.
  - Use a cohesive palette defined as CSS variables, and a dark mode that is designed, not inverted.
  - Keep motion to a few high-impact moments, and respect `prefers-reduced-motion`.
  - Design mobile-first.
  - Don't use: a cream or off-white background, purple-on-white gradients, italic accent words in headlines, numbered "01/02/03" section labels, monospace labels as decoration, pill-shaped buttons everywhere, or a generic three-card hero layout.
- **Release bar**: match `doodla/iris`. Clone it with `GIT_LFS_SKIP_SMUDGE=1 git clone --depth 1 https://github.com/doodla/iris` and study its `README.md`, `AGENTS.md`, `CONTRIBUTING.md`, `SECURITY.md`, `CHANGELOG.md`, `docs/` (including the style guide in `docs/contributing/style-guide.md`) and `.github/`. That means:
  - versioned public contracts with committed schemas and drift tests;
  - docs split into guides, concepts, reference and contributing, with each fact in one place;
  - `AGENTS.md`, with `CLAUDE.md` importing it;
  - `CODE_OF_CONDUCT.md`, issue and PR templates, and Dependabot;
  - CI covering lint, format, typecheck, unit tests, end-to-end tests, mod validation and tests, and schema drift;
  - a release workflow;
  - a `marketplace.json` so the mod is installable.
- **Spend**: live Jev, gateway and `sample` calls during the build stay under $3 in total. Gate every live test behind an env flag.
- **Code**: TypeScript throughout, so contracts are shared by every surface. Keep it minimal: no speculative abstractions, no defensive code for impossible cases, and no hard-coding to pass tests. Never delete or weaken a test or a `must` entry to get green.
- **Don't, without me**: publish to npm, push tags, create GitHub releases, or change repo settings. Prepare all of these, and list exactly what I need to click.
</constraints>

<verified_facts>
I checked these in this container. Rely on them.

- **Tools**:
  - Chromium and Playwright are pre-installed. `require('playwright')` works and `chromium.launch()` needs no options. Don't reinstall them.
  - Claude Code is version 2.1.310.
  - `*.vercel.sh` calls get credentials injected by the proxy.
  - `OPENAI_API_KEY` and `GEMINI_API_KEY` are set. `TYPESAFE_API_KEY` is not set.
- **Jev through the gateway**:
  - The endpoint is `POST https://ai-gateway.vercel.sh/v1/evaluate` with body `{ model: "typesafe-ai/jev", state, questions }`. `state` can be a string, an object or an array.
  - Question types:
    - `boolean` → `{ probability }`;
    - `choice` (`criteria`: a record of option names to descriptions) → `{ choice, probabilities, confidence }`;
    - `score` (`criteria`: an array ordered low to high) → `{ score, probabilities }`.
  - Several questions can go in one call. Limits: 32k tokens for the state plus the longest question, and 64k per request.
  - The AI SDK equivalent is `experimental_decide`. There's also a TypeSafe-compatible API at `https://ai-gateway.vercel.sh/typesafe`. Docs: https://vercel.com/docs/ai-gateway/modalities/decision
  - A live call from here returned in about 50 ms and cost about $0.000016. Pinned versions `typesafe-ai/jev-1.13.0` and `typesafe-ai/jev-1.14.0` exist.
- **Jev direct**:
  - The endpoint is `POST https://api.typesafe.ai/v1/systemone` with `Authorization: Bearer $TYPESAFE_API_KEY` and `model: "jev-latest"`.
  - The boolean type is named `noul` there, its answer is `{ noul: <p> }`, and usage is snake_case.
  - Docs: https://docs.typesafe.ai/llms.txt. Read them before you code against this API.
  - Test with fixtures built from the docs, plus a gated live smoke test.
- **Iris**:
  - Install: `curl -fsSL https://raw.githubusercontent.com/doodla/iris/main/install.sh | sh` installs it into `~/.local/bin`, and `iris doctor` passes here.
  - It never picks a model: choose one from `iris models list` and pass `-m`.
  - `--dry-run --json` shows the cost first, and `--max-cost` caps it. Generate at a supported size, then crop.
- **Mods**:
  - Load the `plugin-authoring` skill before you write any mod code. Its types are the reference.
  - The API includes `$.http.fetch`, `$.process.spawn`, `$.fs`, `$.store` / `$.state`, `$.tool.register`, `$.agent.register` / `$.agent.spawn` (with `isolation: 'worktree'`), `$.model.complete`, `$.command.register` and `$.ui.*` (Pane, AbovePrompt, status, toast).
  - Panes draw from a fixed set of elements (Box, Text, Button, Markdown, and Svg on non-terminal surfaces), not a DOM.
  - The first mod file you write triggers a one-time "turn on mods" question, which I'll answer.
  - `claude plugin validate` and `claude plugin test` exist here.
  - Install from a repo is `/plugin marketplace add doodla/planroom`, then `/plugin install planroom@<marketplace name>`. There is no `--marketplace` flag.
- **Artifacts**:
  - Load `artifact-design` and `artifact-capabilities` before you write the page.
  - `sample` asks Claude, and `db` is shared storage that the session reads and writes with the ArtifactData tool.
  - Pages can't call arbitrary hosts, so Jev runs on the server and mod side.
- **CI**: Iris's CI triggers only on pushes to `main` and on PRs. Planroom's must also trigger on pushes to any branch.
- **npm**: the name `planroom` is unclaimed on npm.
</verified_facts>

<design_starting_points>
These are not binding. Each is a reasonable first guess. Validate it during architecture planning, keep it or replace it, and record the call in a decision record.

- **Kernel**:
  - A typed property graph built from four primitives:
    - Entity `{id, kind, level, parent?, props, layer, provenance}`;
    - Relation `{id, kind, from, to, props, evidence[], confidence, layer, provenance}`;
    - Event (append-only; all other state is computed from events);
    - View (a saved query plus layout and overrides).
  - `layer` (actual or intended) makes drift a diff between layers.
  - IDs are stable and namespaced, for example `<repo>:<kind>:<key>`.
- **Domain packs**: declarative files that add entity and relation kinds, which level each kind sits on, aggregation rules, extractors, lenses, decision kinds and a default view. Six ship in v0.1: code, plan, observability, release, artifacts, workflow.
- **Levels**: the C4 model (System, Container, Component, Code) with Intent on top. Each node has one parent on the level above, and edges aggregate upward with counts.
- **Extractors**, in tiers:
  1. universal: the file tree, git history (co-change, ownership, churn), manifests and lockfiles;
  2. syntactic: tree-sitter with per-language query files and import resolvers; TS/JS, Python, Go and Rust in v0.1;
  3. precise: SCIP/LSIF indexes or a language server, when present;
  4. config: Docker, compose, k8s, Terraform, CI, OpenAPI, protobuf, OpenTelemetry and Prometheus configs, dashboards;
  5. inference: Claude for groupings and purposes, Jev for cheap calls at scale, both marked as inferred.

  For artifacts: deterministic parsing first (headings, tags, front matter, YAML), then Claude for segmentation, then Jev for classification.
- **Store**:
  - One local server per workspace acts as the single writer. It takes HTTP commands and queries, and pushes a live stream filtered by scope.
  - Writes use optimistic concurrency against the entity versions the writer read.
  - Storage is SQLite in WAL mode: events, computed state and a decision cache.
  - A deterministic, git-friendly export in each repo's `.planroom/` holds the intended layer, overrides, plans and views, so they can be reviewed in PRs.
  - A registry tracks sessions and agents, with heartbeats and claims (time-limited leases that warn rather than block).
  - Every event carries the workspace, the project, the actor, what caused it, and an idempotency key.
  - Queries go through a typed builder plus named queries (neighbourhood, paths, roll-ups, diffs). No custom query language.
- **Remote mode**: the workspace is the source of truth. The session pushes snapshots into the Artifact `db`. Artifact edits are written as events to a `db` collection, and the session imports them on `/plan open`, at turn start and on edit notices. The Artifact shows cached decisions and queues new ones.
- **Implementer**: an interface with two implementations, the real mod-registered agent (worktree isolation) and a deterministic fixture implementer for tests and CI.
- **Jev policy**:
  - Each decision kind declares its question, its thresholds and its escalation path.
  - Answers are cached by a hash of the state and the question.
  - The state passed to Jev is a scoped slice from the query API.
  - Structural lint is computed in code; semantic lint goes to Jev.
- **Canvas component kit**:
  - an abstraction-zoom rail and breadcrumb, with semantic folding and edge bundling;
  - lenses;
  - a focus (ego) view;
  - a path finder;
  - an impact-radius heat ramp;
  - a dependency matrix kept in sync with the graph;
  - an anatomy outline;
  - a statechart view with a simulator;
  - evidence popovers;
  - a minimap and search;
  - a stable hierarchical layout (for example ELK) with pinnable positions.
- **Planning stages**: Intent (JTBD and laddering), Outcome (SMART and non-goals), Breakdown (MECE, the WBS 100% rule, or an impact or story map), Next actions (GTD and HTN), Prioritize (MoSCoW and an MVP line), Pre-mortem, Lint.
</design_starting_points>

<process>
Plan from the top down, review independently, build in thin vertical slices.

**If `ultracode` is on**, run each phase below as its own workflow, and read the results before you start the next one. Follow `CLAUDE.md`: set `model` and `effort` on every agent, and never use a checker weaker than what it checks. Otherwise, use subagents where the same shapes apply.

1. **Understand.**
   - Read the verified docs, the mod and Artifact skills, and Iris's repo.
   - Run spikes on the riskiest unknowns before committing to any design: how dense a mod pane can be, Artifact `db` sync, Jev latency and calibration on real plan-node slices, tree-sitter under Node, and edge aggregation performance at 200+ nodes.
   - Record what you learn in `progress.md`.
2. **Architecture, top down.**
   - Go from requirements to capabilities, to architecture, to modules and contracts, to the milestone plan.
   - Produce two or three independent candidate architectures from different angles: simplicity-first, extensibility-first, and concurrency- and agent-first.
   - Score them against the requirements and principles with parallel judges, then synthesize the best into one. Graft in the good ideas from the runners-up.
   - Write the result up as:
     - `docs/contributing/architecture.md`, with C4 diagrams;
     - decision records in `docs/contributing/decisions.md`, each naming the design starting point it kept or replaced, and why;
     - the schemas and the public contracts;
     - a traceability table (each requirement and scenario → the design element → the planned test);
     - `feature_list.json`, where every scenario is a `must` entry and you add `should` and `could` entries.
3. **Independent architecture review.**
   - Give the design documents, not your reasoning, to reviewers at your own model level or above. Give each a distinct lens: correctness and concurrency, extensibility and core simplicity, the principles (attention and agent-native), user surface and UX, security and privacy (secrets, the repos being scanned, prompt injection from scanned content), operability and release, and cost.
   - Each reviewer tries to refute the design. Fix every finding that survives a majority. Answer the rest in the decision records.
   - Repeat until a round finds nothing material.
4. **Walking skeleton.** Build one thin slice through every surface before deepening any of them: scan the fixture, then a node in the store, then the mod pane showing it, then the canvas showing it, then the Artifact publishing it. This raises the mod and Artifact consent prompts early, while I'm likely still around. Update the architecture with anything the skeleton teaches you.
5. **Milestones.** Order them so each one ends shippable and green:
   1. kernel, store, code and plan packs, mod and canvas basics;
   2. diffs, reconciliation, hand-off and Jev decisions;
   3. multi-repo, observability and release;
   4. artifacts, state machines and anatomy;
   5. context curation, polish and release.

   Build independent tracks in parallel, each in its own worktree. Every milestone ends with:
   - an adversarial review of its diff, by distinct lenses;
   - a check of the code against the architecture, where you fix drift or update the decision records;
   - a context review.
6. **Release.** Docs, Iris art, CI, the release workflow, the changelog and `v0.1.0` notes. Then a final independent review of the whole repo against this prompt, with a completeness critic asking what's missing, unverified or untested. Fix what it finds.

**Context review and skill acquisition**, at the end of every phase and whenever you repeat a procedure:
- Correct or cut stale agent context (`AGENTS.md`, `CLAUDE.md`, contributing docs, `progress.md`), and add the invariants you've learned.
- Distill any procedure you've done twice into a project skill in `.claude/skills/`, written and tested with `skill-creator`.
- Log what each review changed and what it will save.

**Working rules:**
- Work one feature at a time: verify it, flip its flag, commit, push.
- Context compacts automatically, so don't wind down early to save tokens. Keep `progress.md` good enough that any later session can resume from it alone.
- Verify by using the product. Drive the canvas with Playwright as a person would, screenshot every main view at 1440×900 and 390×844 in both themes, and critique each screenshot against the principles and the look and feel ("is the next action obvious? is any decision costlier than it needs to be?"). Fix, then re-shoot.
</process>

<definition_of_done>
- Every acceptance scenario passes, as an automated test or with recorded evidence. Every other `must` entry passes. List the failing `should` and `could` entries as known gaps.
- The final independent review finds nothing material.
- Everything is committed and pushed, and CI is green.
- Screenshots are committed under `docs/screenshots/`. The Iris art and its prompts are committed.
- The final message leads with the Artifact link and what works. It shows the evidence: test and CI output, the S7 decision-eval table, screenshot paths, Iris and Jev spend, and the review outcomes. Then it lists known gaps honestly, and the exact clicks I owe (npm name and token, tag, release, repo settings).
</definition_of_done>

<autonomy>
A standing instruction from the user, the person you are working for. It is about how your turns end. A message with no tool call in it ends your turn, and the work stops there until you are asked to continue. The user has seen you end turns in four ways while work they asked for was still owed, and does not want any of them. One: a long summary of what was done that closes by announcing the next step and has no tool call, so the next thing never starts. Two: an offer to carry on with something unless the user would prefer otherwise, which stops to wait for an answer the user was not going to give. Three: a list of decisions for the user when, by your own account, none of them blocks the rest of the work. Four: deciding that this is a good place to report, because the turn has been long or a milestone is done. Status notes are welcome, and so are your recommendations on open decisions, but put them in the same message as your next tool call and carry on with whatever does not depend on the user's answer. If you notice yourself inviting the user to redirect you or offering to wait, delete it and do the next thing. The stops the user does want are the ones where nothing can move without them, or where the thing blocking you is deliberately protected from you. This does not override the need for confirmation on risky or destructive actions.

Make reasonable assumptions and record them in `progress.md` instead of asking. The only prompts that need me are the Artifact consent dialogs and the "turn on mods" question. I'll answer them when they appear, so keep working on whatever doesn't depend on them.
</autonomy>
