# Planroom kickoff prompt

Paste everything below the line into a fresh Claude Code session on this repo. Put the word `ultracode` in the same message to turn on multi-agent workflows (see `<process>`).

---

<context>
I'm the only user of what you're about to build, and I want it built right: a complete, releasable open-source project on GitHub, not a demo. You have full autonomy. Make every product, design and technical call yourself, build it end to end, and ship it. Don't stop to ask me questions.

Here is how to treat each section of this prompt:

| Section | Status |
|---|---|
| `<principles>` | Binding. Each rule has a test hook (see S15). |
| `<requirements>` | Binding: *what* and *why*, never *how*. |
| `<acceptance_scenarios>` | Binding. They are the real specification, ranked into tiers. |
| `<constraints>` | Binding: choices I've made, and limits. |
| `<verified_facts>` | Checked in this environment. Rely on them instead of rediscovering them. |
| `<design_starting_points>` | Not binding: my best guesses at *how*. Override any of them when you find something better, and record why in a decision record. |
| `<process>` | How to plan, review, build and integrate. Follow it. |
| `<definition_of_done>` | Binding: when to stop. |
| `<autonomy>` | Binding: how turns end. |

Environment: a cloud Claude Code container with this repo (`planroom`, MIT, owner Sridhar Vadlamani) checked out. Read `CLAUDE.md` first: it sets rules for subagents, and a hook enforces them. Commit and push to the branch you were assigned.

Context will be compacted many times on a run this long. Write anything you need to survive a compaction to `.claude/carry.md`: where you are in the process, the current milestone and review round, in-flight decisions, gotchas. A hook replays it after every compaction or resume. Keep it under 4,000 characters, and keep `progress.md` as the full record.
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
   - Agents are the primary users of the engine. Every capability is reachable through stable, typed, documented interfaces, with stable error codes and one versioned schema behind all of them:
     - CLI commands with `--json`;
     - the local API;
     - tools agents can call (the mod's tools, and an MCP server if the architecture calls for one);
     - skills that teach agents when and how to use each capability.

     Output is designed for agents first: structured, compact, paginated, with stable IDs and pointers to evidence rather than prose. The human surfaces render that same output.
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

**R1. Point it at work, get understanding.** Point Planroom at a repo, or a workspace of several repos, and it builds a map of the system that someone can understand at a glance. They can then move up and down several levels of abstraction, from intent at the top to code at the bottom, and every higher level is a faithful roll-up of the levels below it. It works on any language. Precision may vary by language, but no repo gets nothing, and adding a language never requires changing the core.

**R2. Understand relationships.** People can see and question how things relate: dependencies, dependents, data flow, deployment, co-change, ownership, and the plan's own links. They can ask "how does A reach B?", "what does this touch?" and "why do you think A depends on B?". Every inferred fact shows its evidence and confidence, and a person can confirm or reject it.

**R3. Break down any artifact.** Prompts, docs, decision records, specs, skills, agent configs, issues, PRs, commits, code reviews, review threads, CI workflows and modules can all be broken into their parts. Each part gets its purpose, why it exists, and how it relates to the other parts (constrains, verifies, refines, contradicts, addresses), plus what's missing. The model for this is a Claude session answering "what are the components of that prompt?" with a clear table. For PRs and reviews, the breakdown shows which threads are addressed and which are open, and which parts of the system the change touches.

**R4. Extract state machines.** Planroom finds the state machines implicit in code, config and prose, and makes them explicit and explorable: states, transitions, events and guards, with evidence. It checks them for unreachable states, dead ends, missing transitions and nondeterminism, and lets someone step through a machine. When prose and code describe the same machine differently, that drift is flagged.

**R5. Clarify intent, top down.** With Claude as a partner, a person turns a fuzzy intent into a plan they believe in. Planning always runs top down: intent, then outcome, then breakdown, then next actions, then priorities, then risks. It uses established methods (for example Jobs-to-be-Done, laddering, SMART outcomes, MECE or WBS breakdown, GTD next actions, MoSCoW, pre-mortem), and the person can jump between stages. Claude checks, curates and extends the plan. The AI proposes and the person disposes: nothing the AI invents counts until a person accepts it. Plans attach to the real system, and next actions name what they touch.

**R6. Tweak everything.** The map, the plan and any artifact breakdown can be shaped directly, by hand on the canvas or by asking Claude: regroup, rename, re-scope, re-wire, hide noise, pin a layout, edit a breakdown. Corrections survive every rescan. A scan never silently overrides a person's edit; a conflict becomes a diff for them to resolve.

**R7. Diffs and reconciliation.** There are three kinds of diff, with one diff engine and one visual language:
- **plan and model versions**, like git for the plan;
- **intended vs actual**, where what people say the system should be and what the code is disagree;
- **proposed changes**, code or text, mapped onto the map.

The intended architecture can declare boundary rules (what may depend on what), and violations show up as intended-vs-actual drift. A reconciler compares intent with reality on demand and on change, and classifies each mismatch as expected, in progress, done, unplanned or regression.

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

**R12. Context curation and skills.** Planroom keeps the agent context of the repos it's pointed at accurate and lean, so repeat agent work gets simpler and cheaper. It proposes updates to `CLAUDE.md` and `AGENTS.md`, and new or updated project skills for procedures agents keep repeating. It spots those from the history of implementer runs. It runs periodically (on a trigger such as every N implementer runs, or after a merge) and on demand. Proposals arrive as diffs for approval.

**R13. Surfaces.** There are three, sharing one model:
- **A Claude Code mod that guides.** It lives beside the chat. Max out its UX: the current focus, the one open decision with pre-answered choices, progress, pending diffs, drift, and who's working on what. It also has a band above the prompt with the next best action, slash commands, tools the model calls, and a mini-map where the surface can draw one. It is keyboard-first.
- **A canvas web app** for understanding and tweaking. It's beautiful and fast on desktop and on a phone. It runs locally, and is also published as a Claude Artifact for when localhost isn't reachable, for example from a cloud session.
- **A CLI and local server, the engine**, which agents use directly.

All three ship as **one Claude Code plugin**. Installing it gives an agent or a person everything: the mod, Planroom's skills and agent types (the implementer and the curator), and the engine. Within the plugin's limits, the engine binary is resolved on first use.

**R14. A tiny surface to learn.** The whole mental model a person needs is five ideas: **Levels** (where I am), **Lenses** (which relationships I see), **Intent** (what I want, grouped into projects), **Diffs** (what's different) and **Decisions** (what needs me). There is one CLI entry point and one slash-command family (`/room …`; `plan` is already a Claude Code permission-mode name, so avoid `/plan`). Everything else stays internal. Before adding any concept, flag or setting, check whether a better default would remove the need for it.

**R15. Releasable.** The repo is ready for a public v0.1 release, at the standard of `doodla/iris` (see `<constraints>`).

**R16. A deliberately designed repo.** This is one long build in a box, so the main agent owns the repo's design and keeps it coherent, while subagents and parallel tracks follow it. The structure is designed on purpose for a Rust and TypeScript codebase that agents write and run:
- **One obvious place for everything.** Every directory has a stated purpose. Each top-level area has a short `AGENTS.md` or README giving its contract: what it owns, what it may depend on, how to test it.
- **Boundaries enforced by tooling, not convention:**
  - the core kernel depends on no domain;
  - packs and adapters depend inward, never sideways;
  - surfaces talk to the engine only through the published contracts;
  - generated code lives in marked directories and is never hand-edited.

  A check fails CI when a dependency crosses a boundary, in both Rust and TypeScript.
- **One set of commands.** A single task entry point runs the same commands locally and in CI: check, test, end-to-end, generate, release dry run. Agents use only those commands.
- **Pinned toolchains and reproducible builds:** pinned toolchain versions, committed lockfiles, and one Cargo workspace plus one JS workspace.
- **Shared conventions, written down once in `AGENTS.md`:** naming, error codes, logging, configuration, and where new code goes.
- **Structure changes are deliberate.** Changing the layout, a boundary or a convention needs a decision record, written by the main agent, before the change lands.
</requirements>

<acceptance_scenarios>
These define done. Each scenario is ranked into a tier:

| Tier | Scenarios | Rule |
|---|---|---|
| **A**, the core loop | S1, S2, S3, S4, S7, S8, S14, S15 | Must pass. |
| **B** | S5, S6, S9, S10, S11, S12, S13 | Must pass, or ship as a documented gap with evidence of how far it got. |
| **C** | Anything you add yourself | Nice to have. |

Each scenario becomes an entry in `feature_list.json` with its tier, and has **two declared paths**:
1. **A deterministic CI path**: named CLI `--json` queries compared against the truth file, the fixture implementer, and recorded or replayed responses for every Claude and Jev call. It needs no secrets and no live model.
2. **An evidence path**: run live once, with the output recorded under `docs/evidence/`.

**If a scenario turns out to be infeasible as written**, don't weaken it quietly. Write a decision record with the evidence, propose the closest testable variant, and have an independent reviewer approve the substitution. Never delete or weaken a scenario, test or `feature_list.json` entry in any other way.

**When a milestone has used much more effort than planned**, finish its tier A items, record what's left, and move on. Return to the rest only if time remains at the end.

**The fixture.** Nested git repos don't survive a commit (only a gitlink is stored), so commit these instead:
- plain source trees;
- a JSON file of review threads, in a format you document;
- a seeded history of implementer runs;
- a deterministic `examples/build-fixture` script that turns them into fresh git repos (commits, tags, branches) in a temp directory for each test run.

All merges in the scenarios happen in that copy, so the fixture is the same on every run. Together, the repos form a small polyglot workspace of two or three repos:
- a TS web app, a Python worker and a Go service, wired by docker-compose and an OpenAPI spec;
- a shared library that one repo publishes and another consumes at a stale version;
- OpenTelemetry instrumentation, a Prometheus alert rule and a dashboard JSON;
- an order or job lifecycle in code, plus a doc that describes it slightly differently;
- tags and changelogs, and a PR (branches plus the review-threads file) with review threads, one of which a later commit addresses;
- planted known positives, each with its exact expected name: an unreachable state, a dead end, the doc-vs-code lifecycle drift, one unplanned change and one regression.

**Freeze the truth first.** Write the fixture's known truth in `examples/TRUTH.json` before any extractor exists. Have an independent agent review it against the fixture. After that, change it only through a decision record.

**S1. First look.** Run `planroom` on the fixture. A cold scan finishes in under 5 s. Someone then sees a map at the top system level, with a sensible default lens and one suggested next action. Zooming to code and back shows faithful roll-ups. Search jumps to any entity.

**S2. Questions.** Answer, correctly against the fixture's truth:
- "How does observability work here?"
- "What's unmonitored?"
- "What must release, in what order, to ship a change to the shared library?"
- "Who's on a stale version?"
- "How does the web app reach the worker?" (each hop with its evidence)

**S3. Top-down plan.** "Add retries to the worker" is taken from intent through all planning stages to next actions attached to real components. AI proposals stay ghosts until accepted. Lint flags a vague action and a missing done-test. The accepted plan becomes version 2, and the diff from version 1 renders in the same diff view as S4.

**S4. Tweak and hand off.**
1. Regroup two components, and change the intended architecture.
2. The intended-vs-actual diff appears, with its impact radius.
3. Approve it. A change spec goes to the implementer, the code diff comes back for review, and you merge it.
4. Reconciliation shows the item resolved, and classifies the planted unplanned change and regression correctly.
5. A rescan keeps the regrouping.

**S5. Artifact anatomy.**
- Break down `KICKOFF.md` (this prompt). Its parts match its sections, with sensible purposes and relations.
- Break down the fixture PR. Addressed and open threads are correct, and the touched components are shown.
- Break down one skill and one CI workflow.
- Edit a breakdown, and see the result come back as a text diff to the source.

**S6. State machines.** Extract the fixture lifecycle from code and from the doc, and flag the drift between them. Extract the process-phase machine from this prompt, checked against a truth that you write and freeze in `TRUTH.json` before extraction. Planroom's own escalation and hand-off loops are modelled, and match its code. The checks find the planted unreachable state and dead end, by name, and the step-through works.

**S7. Cheap decisions.**
- Every decision kind has a policy and an eval set. An independent labeller, who has not seen Jev's outputs, writes the labels, with a rationale for each. The cases are held out from threshold tuning.
- Automatic action is enabled only for kinds that reach observed precision ≥ 0.95 on at least 60 cases. The rest fall back to one-tap cards.
- Both Jev adapters (gateway and direct) pass the same contract tests, against fixtures built from the docs.
- A commit-triggered rescan of the fixture makes 0 Claude calls on the routine path, spends under $0.001 on Jev, and raises at most one decision card.
- Report the table at the end. Decisions can be inspected and overridden.

**S8. Concurrency.** Two agents and a canvas tab write at the same time to two projects in one workspace. Check that:
- no writes are lost;
- retries are idempotent;
- live updates arrive within one second;
- claims and presence are visible;
- a stale write becomes a diff, not a merge.

**S9. Mod.** In a live session, every main flow can be driven from the mod: pane, band, commands and tools. Every action is reachable in two keystrokes or fewer from pane focus (a slash command counts as one entry point). The implementer hand-off runs once for real, and the output is recorded.

**S10. Extensibility.** A toy domain pack, loaded from outside the source tree, adds new kinds, a lens and a decision kind without any core change. A new language works through the extractor contract alone.

**S11. Remote.** The canvas is published as an Artifact and kept in sync with the workspace. An edit made there comes back to the workspace as an attributed user edit.

**S12. Context curation.** On the fixture, Planroom proposes at least one `CLAUDE.md` diff and one skill, based on the history of implementer runs.

**S13. Quality bars.**
- The canvas is interactive in under 1 s on the fixture.
- At 200 nodes, the p95 frame time stays under 20 ms. Measure it locally with a Playwright trace; it's evidence, not a CI gate.
- At 390×844 with touch emulation, nothing overflows, tap targets are at least 44 px, pinch-zoom works, tap-to-focus works, and decision cards take one tap.
- Three named motion moments (accepting a ghost, a diff resolving, a level zoom) each have a short recorded capture, and a `prefers-reduced-motion` test.
- Light and dark themes are both designed.
- There are no console errors.
- Every edit type has an undo test.

**S14. Release.**
- CI is green on the pushed branch.
- From a fresh clone of planroom, the release build (`cargo build --release` plus the frontend build) runs on the built fixture.
- **Plugin install, end to end:**
  1. In a fresh Claude Code session, run `/plugin marketplace add <local path to the repo>`, then `/plugin install planroom@<marketplace name>`.
  2. On first use, the engine binary resolves: a checksum-verified release asset, or a source build when no release exists yet.
  3. `/room` works, Planroom's skills and agent types are listed, and an agent can drive S1 and S2 using only the plugin's tools and skills.
- The docs, art, changelog and release notes are complete per `<constraints>`.

**S15. Tiny surface and principles.**
- `planroom --help` and the mod's command list are generated and snapshot-tested: one binary and one command family.
- The UI exposes no top-level concepts beyond the five in R14.
- An empty-state screenshot audit finds no blank states.
- The error-code schema has a drift test.
- The boundary checks fail on a deliberately planted bad import, in both Rust and TypeScript. The test plants the import in a temporary copy.
- Every edit type has an undo test.
</acceptance_scenarios>

<constraints>
- **Surfaces**: a Claude Code mod, a canvas web app also published as a Claude Artifact, and a CLI with a local server. The implementer is a separate agent.
- **Packaging**: everything ships as one Claude Code plugin, installable from this repo's `marketplace.json`.
  - The plugin carries the mod, skills, agent definitions and any tool or MCP wiring.
  - Plugins can't contain a `bin/` folder, so the engine binary is fetched from a GitHub release on first use (checksum-verified, in the style of Iris's `install.sh`), with a `cargo install` or source-build fallback.
  - The release workflow builds the engine for Linux and macOS, on x86_64 and arm64.
- **Languages**:
  - The **engine** (scanning, extraction, store, server, reconciler, diff engine, Jev and Claude adapters, CLI) is written in **Rust**.
  - The **mod** is TypeScript, because that's what the mod API loads.
  - The **canvas** is TypeScript, and the local server serves its built assets.
  - Contracts have **one source of truth**. Schemas for the store, the API, the CLI `--json` output, the tools and the domain packs are generated from that source for every language, and a test fails on drift. Never hand-copy types across the language boundary.
- **Build on good libraries.** Use mature, well-maintained, widely used libraries with permissive licenses, rather than hand-rolling parsing, graphs, layout, storage, HTTP, diffing, git access, CLI parsing, UI primitives or animation. Hand-roll only Planroom's own core: the kernel, the decision policy, reconciliation and the domain packs.
  - Pin versions, and audit dependencies in CI (`cargo-deny` as in Iris, plus an npm audit).
  - Record each significant library choice, and the alternatives you rejected, in the decision records.
- **Jev** (TypeSafe AI) is the cheap-decision model. Support both the Vercel AI Gateway and direct TypeSafe connections behind one adapter.
  - Pin `typesafe-ai/jev-1.14.0` on the gateway.
  - On the direct API, pin a versioned ID if the docs list one. Otherwise send `jev-latest`, record the version the response reports, and fail the evals if it changes. Ignore thejevai.com: it isn't affiliated with TypeSafe.
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
  - CI covering:
    - Rust: `fmt`, `clippy` with warnings as errors, tests, `cargo-deny`;
    - TypeScript: lint, format, typecheck, tests;
    - end-to-end tests;
    - plugin and mod validation and tests;
    - schema drift between the languages;
  - a release workflow;
  - a `marketplace.json`, so the whole plugin is installable.
- **How Claude is reached**: through the mod (the chat model, `$.model.complete`), the Artifact (`sample`), or Claude models on the Vercel AI Gateway for the engine. Pick a model from the gateway's `/v1/models` list and pin it in config.
  - `OPENAI_API_KEY` and `GEMINI_API_KEY` are for Iris only. Never use them for inference.
  - When no model is reachable, every surface falls back to the deterministic extractor tiers, so no repo gets nothing.
- **Deterministic CI**: CI needs no secrets and no live model. Every agent and model boundary has a deterministic stand-in: the fixture implementer, and recorded or replayed responses.
- **Spend**:
  - Live Jev, gateway (including Claude) and `sample` calls made by the product and its tests during the build stay under $3 in total. Gate every live test behind an env flag.
  - There's no hard cap on the build's own agent tokens, but keep it proportionate: at most 3 candidate architectures, at most 7 review lenses, at most 3 review rounds. Log a rough spend per phase in `progress.md`.
- **Code**: keep it minimal: no speculative abstractions, no defensive code for impossible cases, and no hard-coding to pass tests.
- **Don't, without me**: publish packages (crates.io or npm), push tags, create GitHub releases, or change repo settings. Prepare all of these, and list exactly what I need to click.
</constraints>

<verified_facts>
I checked these in this container. Rely on them.

- **Tools**:
  - Chromium and Playwright are pre-installed. `require('playwright')` works and `chromium.launch()` needs no options. Don't reinstall them.
  - Claude Code is version 2.1.310.
  - `*.vercel.sh` calls get credentials injected by the proxy.
  - `OPENAI_API_KEY` and `GEMINI_API_KEY` are set, for Iris only. `TYPESAFE_API_KEY` is not set.
  - The gateway also serves Claude models. A live chat-completions call from here worked.
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
- **Toolchains**: Rust 1.97 (`cargo`, `clippy`, `rustfmt`) and Node 22 are installed here.
- **Plugins**: a saved plugin's `name` is lowercase letters, digits and hyphens (64 at most), and **a plugin with a `bin/` folder is refused**. Agent types can be declared in a plugin's `agents/*.md` files or with `$.agent.register`. Check the current plugin docs, through the `claude-code-guide` agent, for how a plugin declares skills and MCP servers before you design around them.
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
- **Repo layout sketch** (not binding; redesign it freely):
  ```
  Cargo.toml / pnpm-workspace.yaml / justfile (or cargo xtask) / rust-toolchain.toml / .node-version
  crates/    planroom-schema (contract source) · planroom-kernel · planroom-store · planroom-extract-* ·
             planroom-packs · planroom-decide (Jev + Claude adapters) · planroom-server · planroom-cli
  packages/  schema (generated TS, never edited) · canvas · mod · ui (component kit)
  plugin/    .claude-plugin/ · skills/ · agents/ · hooks/ (the built mod) · engine bootstrap
  examples/  fixture sources · build-fixture · TRUTH.json
  docs/      guides · concepts · reference · contributing (architecture, decisions, repo-layout)
  ```
  For boundaries, a crate-graph test or `cargo-deny` bans on the Rust side, and `dependency-cruiser` or `eslint-plugin-boundaries` on the TypeScript side.
- **Libraries to evaluate first** (these are suggestions; pick on maintenance, fit and licence, and record the choice):
  - Rust:
    - `tree-sitter` with its language grammars;
    - `petgraph` for graph algorithms;
    - `rusqlite` or `sqlx` for SQLite;
    - `axum` and `tokio` for the server and live stream;
    - `serde` and `schemars` for schemas;
    - `clap` for the CLI;
    - `gix` (gitoxide) or `git2` for git;
    - `notify` for file watching;
    - `similar` for diffs;
    - `rmcp` (the official Rust MCP SDK) if you build an MCP server;
    - `tracing` for logs;
    - `reqwest` for the Jev and gateway calls.
  - TypeScript:
    - Vite;
    - React with `@xyflow/react` (React Flow) for the node canvas;
    - `elkjs` for stable hierarchical layout;
    - Motion for animation;
    - Radix UI primitives;
    - TanStack Query, or a small store, for state;
    - XState for the statechart simulator;
    - generated types from the shared schemas (for example `json-schema-to-typescript`, or a Rust-to-TS generator such as `ts-rs` or `specta`).
- **Extractors**, in tiers:
  1. universal: the file tree, git history (co-change, ownership, churn), manifests and lockfiles;
  2. syntactic: tree-sitter with per-language query files and import resolvers; TS/JS, Python, Go and Rust as scanned languages in v0.1;
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
- **Remote mode** (largely forced, because Artifact pages can't reach the local server or Jev): the workspace is the source of truth. The session pushes snapshots into the Artifact `db`. Artifact edits are written as events to a `db` collection, and the session imports them on `/room open`, at turn start and on edit notices. The Artifact shows cached decisions and queues new ones.
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

**If `ultracode` is on**, use workflows like this. If it's off, use subagents in the same shapes.
- Run one workflow per phase, and **one workflow per milestone and per review round**. Read each result before you start the next.
- `CLAUDE.md` is enforced by a hook:
  - write `model:` and `effort:` inline in every `agent({...})` call, and don't spread a shared options object (the hook can't check variables, and it blocks the call);
  - never use a checker weaker than what it checks.
- Keep Artifact publishing, consent-dependent steps and git pushes in the main session.

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
     - `docs/contributing/repo-layout.md`: the repo's design per R16, covering the directory map, boundaries and how they're enforced, the task commands, toolchain pinning, codegen flow and conventions. Lay out the skeleton directories and tooling before any feature work;
     - a traceability table (each requirement and scenario → the design element → the planned test);
     - `feature_list.json`, where every scenario is an entry with its tier (A or B) and its two paths, plus any tier C entries you add.
3. **Independent architecture review.**
   - Give the design documents, not your reasoning, to reviewers at your own model level or above. Give each a distinct lens: correctness and concurrency, extensibility and core simplicity, repo design (R16: boundaries, tooling, commands, and how easy it is for an agent to find its way around), the principles (attention and agent-native), user surface and UX, security and privacy (secrets, the repos being scanned, prompt injection from scanned content), operability and release, and cost.
   - Each reviewer reports findings within their own lens. A separate independent verifier then tries to refute each finding, and every finding the verifier fails to refute gets fixed. Don't vote across lenses: a security finding must not need the UX reviewer's agreement.
   - A finding is **material** when it would make a tier A or B scenario fail, violates a principle or constraint, or is a security or data-loss risk.
   - Repeat until a round finds nothing material, up to 3 rounds. Record anything left after round 3 in the decision records as accepted risk, with a one-line reason.
4. **Walking skeleton.** Build one thin slice through every surface before deepening any of them: scan the fixture, then a node in the store, then the mod pane showing it, then the canvas showing it, then the Artifact publishing it. Load all of it through the plugin install from day one, with the Rust engine built from source, so the packaging is real from the start. This raises the mod and Artifact consent prompts early, while I'm likely still around. Update the architecture with anything the skeleton teaches you.
5. **Milestones.** Order them so each one ends shippable and green:
   1. kernel, store, code and plan packs, mod and canvas basics;
   2. diffs, reconciliation, hand-off and Jev decisions;
   3. multi-repo, observability and release;
   4. artifacts, state machines and anatomy;
   5. context curation, polish and release.

   Build independent tracks in parallel, each in its own worktree and one feature at a time. Give every track `repo-layout.md` and `AGENTS.md` as its working rules. Before merging a track, check it against R16, and send structural deviations back to the track rather than accepting them. The orchestrator merges the tracks into the branch one at a time, runs the full suite after each merge, then pushes. Every milestone ends with:
   - a review of its diff by distinct lenses, with the same per-finding verification, definition of material and 3-round cap as the architecture review;
   - a check of the code against the architecture, where you fix drift or update the decision records;
   - a context review.
6. **Release.** Docs, Iris art, CI, the release workflow, the changelog and `v0.1.0` notes. Then a final independent review of the whole repo against this prompt, with a completeness critic asking what's missing, unverified or untested. It uses the same definition of material and the same 3-round cap. Fix what it finds.

**Context review and skill acquisition**, at the end of every phase and whenever you repeat a procedure:
- Correct or cut stale agent context (`AGENTS.md`, `CLAUDE.md`, contributing docs, `progress.md`), and add the invariants you've learned.
- Distill a procedure into a project skill in `.claude/skills/` once you've done it twice and expect it at least 3 more times. Write and test it with `skill-creator`, with at most 2 skill evals per phase.
- Log what each review changed and what it will save.

**Working rules:**
- Work one feature at a time: verify it, flip its flag, commit, push.
- Context compacts automatically, so don't wind down early to save tokens. Keep `progress.md` good enough that any later session can resume from it alone.
- Verify by using the product. Drive the canvas with Playwright as a person would, screenshot every main view at 1440×900 and 390×844 in both themes, and critique each screenshot against the principles and the look and feel ("is the next action obvious? is any decision costlier than it needs to be?"). Fix, then re-shoot.
</process>

<definition_of_done>
- Every tier A scenario passes on its CI path, with its evidence recorded.
- Every tier B scenario passes, or is documented as a gap with evidence. Any substitution has an approved decision record.
- List any tier C entries that don't pass as known gaps.
- The final review finds nothing material, or what's left after 3 rounds is recorded as accepted risk.
- Everything is committed and pushed, and CI is green.
- Screenshots are committed under `docs/screenshots/`. The Iris art and its prompts are committed.
- The final message leads with the Artifact link and what works. It shows the evidence: test and CI output, the S7 decision-eval table, screenshot paths, Iris and Jev spend, and the review outcomes. Then it lists known gaps honestly, and the exact clicks I owe (tag, release, any registry names or tokens, repo settings).
</definition_of_done>

<autonomy>
A standing instruction from the user, the person you are working for. It is about how your turns end. A message with no tool call in it ends your turn, and the work stops there until you are asked to continue. The user has seen you end turns in four ways while work they asked for was still owed, and does not want any of them. One: a long summary of what was done that closes by announcing the next step and has no tool call, so the next thing never starts. Two: an offer to carry on with something unless the user would prefer otherwise, which stops to wait for an answer the user was not going to give. Three: a list of decisions for the user when, by your own account, none of them blocks the rest of the work. Four: deciding that this is a good place to report, because the turn has been long or a milestone is done. Status notes are welcome, and so are your recommendations on open decisions, but put them in the same message as your next tool call and carry on with whatever does not depend on the user's answer. If you notice yourself inviting the user to redirect you or offering to wait, delete it and do the next thing. The stops the user does want are the ones where nothing can move without them, or where the thing blocking you is deliberately protected from you. This does not override the need for confirmation on risky or destructive actions.

Make reasonable assumptions and record them in `progress.md` instead of asking. The only prompts that need me are the Artifact consent dialogs and the "turn on mods" question. I'll answer them when they appear, so keep working on whatever doesn't depend on them.
</autonomy>
