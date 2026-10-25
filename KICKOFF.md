# Planroom kickoff prompt

Paste everything below the line into a fresh Claude Code session on this repo. Put the word `ultracode` in the same message to turn on multi-agent workflows (see `<process>`).

---

<context>
I'm the only user of what you're about to build, and I want it built right: a complete, releasable open-source project on GitHub, not a demo. You have full autonomy (see `<autonomy>`).

**How to read this prompt.** Everything is binding except `<design_starting_points>`. Those are my guesses at *how*; override any of them when you find something better, and record why in a decision record. `<verified_facts>` were checked live in this environment, so rely on them instead of rediscovering them.

Environment: a cloud Claude Code container with this repo (`planroom`, MIT, owner Sridhar Vadlamani) checked out. Read `CLAUDE.md` first. It covers the hook-enforced subagent rules and `.claude/carry.md`, which survives compactions. Use the carry file for where you are and what's in flight, and keep `progress.md` as the full record that any later session could resume from. Commit and push to the branch you were assigned.
</context>

<stance>
**You are Planroom's founding engineer and its owner.** You're accountable for what this repo is a year from now: whether people trust it, whether agents can work in it, whether it ships cleanly, whether it's still coherent after many hands. Nobody will review your work closely before it lands, so nobody will catch what you miss.

**Read my asks as examples of a standard, not the full list.** Everything specific here is an instance of a higher bar:
- "engine in Rust, TypeScript for the mod" implies a repo designed for two languages;
- "it will be compacted" implies state that survives;
- "respect attention" implies every default, message and screen.

Infer the bar behind each ask, and meet it where this prompt is silent. Gaps I didn't foresee are yours to handle, the way a seasoned team would: packaging, security, migrations, upgrades, performance, error messages, first run, how an agent finds its way around.

**Habits:**
- **Implications.** At each significant decision, ask what it implies for the layout, contracts, tests, release, agents and users. Write the non-obvious ones in the decision record, and act on them.
- **Pre-mortem.** At the start of each phase, write in `progress.md` the five most damaging findings a final review could make about that phase, and prevent them.
- **Second-order effects.** Before finishing anything, ask what it makes harder: for the next milestone, for an agent resuming after a compaction, for someone using it on a phone.

**Keep the bar on quality, not scope.** New capabilities still need a requirement. Unforeseen gaps in quality are yours to close, in proportion: fix what's material, and list the rest as known gaps.
</stance>

<principles>
Two ideas shape every decision. When a requirement is unclear, resolve it in their favour.

1. **Respect human attention.** It's the scarcest resource in the system. A person should be glad Planroom exists, because it gives them understanding and control at a fraction of the effort.
   - Interrupt a person only for decisions only they can make. Make those decisions cheap: pre-decided, batched, explained in one line, one tap to confirm.
   - Never make someone read what a summary, diagram or diff could show them instead.
   - Never show a blank state. Never ask what a good default could answer. Never lose or overwrite what someone did.
   - Everything is reversible, which is what makes bold defaults safe.
2. **Agent-native.** Planroom's code is written by agents, for agents, and run mostly by agents. People steer through the mod and the canvas, while agents read, extract, reconcile and implement.
   - Every capability is reachable through stable, typed, documented interfaces, with stable error codes and one versioned schema behind them all: CLI commands with `--json`, the local API, tools agents can call (the mod's tools, and an MCP server if the architecture calls for one), and skills that teach agents when to use each.
   - Output is designed for agents first: structured, compact, paginated, with stable IDs and pointers to evidence rather than prose. The human surfaces render the same output.
   - The repo is easy for agents to work in: `AGENTS.md` invariants, small modules with clear contracts, deterministic commands, fast tests, project skills, and context that is kept current. Planroom does the same for the repos it's pointed at.
</principles>

<requirements>
These are *what* and *why*. Each one must be traceable to a design element and a test.

**Vocabulary.** The *kernel* is the domain-free core. A *domain pack* is the unit that plugs a domain into it. An *adapter* connects the engine to something outside it (a model, git, a file format). What shape each of these takes is design.

**R1. Point it at work, get understanding.** Point Planroom at a repo, or a workspace of several, and it builds a map someone can understand at a glance. They move up and down several levels of abstraction, from intent at the top to code at the bottom, and every higher level is a faithful roll-up of the one below. It works on any language. Precision may vary, but no repo gets nothing, and adding a language never touches the core.

**R2. Understand relationships.** People can see and question how things relate: dependencies, dependents, data flow, deployment, co-change, ownership, and the plan's own links. They can ask "how does A reach B?", "what does this touch?" and "why do you think A depends on B?". Every inferred fact shows its evidence and confidence, and a person can confirm or reject it.

**R3. Break down any artifact.** Prompts, docs, decision records, specs, skills, agent configs, issues, PRs, commits, code reviews, review threads, CI workflows and modules break into parts. Each part gets its purpose, why it exists, and how it relates to the others (constrains, verifies, refines, contradicts, addresses), plus what's missing. The model is a Claude session answering "what are the components of that prompt?" with a clear table. For PRs and reviews, the breakdown shows which threads are addressed or open, and what parts of the system the change touches.

**R4. Extract state machines.** Planroom finds the state machines implicit in code, config and prose, and makes them explicit and explorable: states, transitions, events and guards, with evidence. It checks for unreachable states, dead ends, missing transitions and nondeterminism, and lets someone step through a machine. When prose and code describe the same machine differently, that drift is flagged.

**R5. Clarify intent, top down.** With Claude as partner, a person turns a fuzzy intent into a plan they believe in.
- Planning runs top down, through these stages: intent (Jobs-to-be-Done, laddering) → outcome (SMART, non-goals) → breakdown (MECE, the WBS 100% rule, or an impact or story map) → next actions (GTD, HTN) → priorities (MoSCoW, an MVP line) → pre-mortem → lint. The person can jump between stages.
- Claude checks, curates and extends the plan. The AI proposes and the person disposes: nothing the AI invents counts until a person accepts it.
- Plans attach to the real system, and next actions name what they touch.

**R6. Tweak everything.** The map, the plan and any breakdown can be shaped by hand on the canvas or by asking Claude: regroup, rename, re-scope, re-wire, hide noise, pin a layout, edit a breakdown. Corrections survive every rescan. A scan never silently overrides a person's edit; a conflict becomes a diff for them to resolve.

**R7. Diffs and reconciliation.** Three kinds of diff, with one engine and one visual language:
- plan and model versions, like git for the plan;
- intended vs actual, including declared boundary rules (what may depend on what);
- proposed changes, code or text, mapped onto the map.

A reconciler compares intent with reality on demand and on change, and classifies each mismatch as expected, in progress, done, unplanned or regression.

**R8. Hand-off.** An approved change that needs code or text edits becomes a self-contained change spec, handed to a **separate implementer agent** working in isolation. Its result comes back as a diff for review, and reconciliation closes the loop after merge. The planning partner in the chat never silently implements.

**R9. Cheap decisions.** Keeping the map and plan current must be nearly free, or it rots. Small, frequent decisions go to Jev, a cheap decision model:
- above a measured confidence bar, Planroom acts or pre-fills;
- in the uncertain band, a one-tap card with Jev's pick pre-selected;
- beyond that, it escalates to Claude.

Every decision kind has a declared policy and an eval behind its threshold. Every decision is recorded, and can be inspected and overridden.

**R10. Many agents, sessions and projects at once.** One workspace serves several Claude sessions, the mod in each, implementer and curator agents, automation, and canvas tabs on several devices, all at once. It holds several projects (plans) that share one model of the system. No write is lost, retries are safe, every surface updates live, and everyone sees who (person or agent) is working on what. User-authored state is never last-writer-wins.

**R11. Extensible domains.** The core data model is simple, domain-free, and extends into a graph. New domains plug in as packs without core changes. v0.1 covers:
- code;
- plans;
- observability ("how does observability work here? what's unmonitored? what fires if this breaks?");
- release management across repos and dependencies ("what must release, in what order? who's on a stale version? what breaks if I bump X?");
- artifacts (R3);
- workflows and state machines (R4).

**R12. Context curation and skills.** Planroom keeps the agent context of the repos it's pointed at accurate and lean, so repeat agent work gets simpler and cheaper.
- It proposes updates to `CLAUDE.md` and `AGENTS.md`, and new or updated project skills for procedures agents keep repeating, which it spots in the history of implementer runs.
- It runs on a trigger (for example, every N implementer runs or after a merge) and on demand.
- Proposals arrive as diffs for approval.

**R13. Surfaces.** Three surfaces share one model:
- **A Claude Code mod that guides.** It lives beside the chat. Max out its UX: the current focus, the one open decision with pre-answered choices, progress, pending diffs, drift, and who's working on what. It has a band above the prompt with the next best action, slash commands, tools the model calls, and a mini-map where the surface can draw one. It's keyboard-first.
- **A canvas web app** for understanding and tweaking, beautiful and fast on desktop and phone. It runs locally, and is also published as a Claude Artifact for when localhost isn't reachable.
- **A CLI and local server, the engine**, which agents use directly.

All three ship as one Claude Code plugin (see `<constraints>`).

**R14. A tiny surface to learn.** The whole mental model a person needs is five ideas: **Levels** (where I am), **Lenses** (which relationships I see), **Intent** (what I want, grouped into projects), **Diffs** (what's different) and **Decisions** (what needs me). There's one CLI entry point and one slash-command family, `/room …` (`plan` is already a Claude Code permission-mode name). Everything else stays internal. Before adding a concept, flag or setting, check whether a better default would remove the need for it.

**R15. Releasable.** The repo is ready for a public v0.1 release, at the standard of `doodla/iris` (see `<constraints>`).

**R16. A deliberately designed repo.** This is one long build in a box, so the main agent owns the repo's design and keeps it coherent; subagents and tracks follow it.
- Every directory has a stated purpose. Each top-level area has a short `AGENTS.md` or README with its contract: what it owns, what it may depend on, how to test it.
- Boundaries are enforced by tooling, not convention. The kernel depends on no domain. Packs and adapters depend inward, never sideways. Surfaces reach the engine only through published contracts. Generated code lives in marked directories and is never hand-edited. A check in CI fails on any crossing, in both Rust and TypeScript.
- Conventions are written down once, in `AGENTS.md`: naming, error codes, logging, configuration, and where new code goes.
- Changing the layout, a boundary or a convention needs a decision record from the main agent before it lands.
</requirements>

<acceptance_scenarios>
These define done.

| Tier | Scenarios | Rule |
|---|---|---|
| **A**, the core loop | S1, S2, S3, S4, S7, S8, S14, S15 | Must pass. |
| **B** | S5, S6, S9, S10, S11, S12, S13 | Must pass, or ship as a documented gap with evidence of how far it got. |
| **C** | Anything you add | Nice to have. |

- **Feature list.** Each scenario is an entry in `feature_list.json` with its tier and two paths:
  - a **deterministic CI path**: named CLI `--json` queries compared against the truth file, the fixture implementer, and recorded or replayed responses for every Claude and Jev call;
  - an **evidence path**, run live once and recorded under `docs/evidence/`.
- **Infeasible scenarios.** If one turns out to be infeasible as written, write a decision record with the evidence, propose the closest testable variant, and have an independent reviewer approve it. Never weaken or delete a scenario, test or entry any other way.
- **Effort.** Record an effort estimate per milestone in `progress.md`. At twice the estimate, finish the milestone's tier A items, record the rest, and move on.

**S1. First look.** A cold scan of the fixture finishes in under 5 s. Someone sees a map at the top level, with a sensible default lens and one suggested next action. Zooming to code and back shows faithful roll-ups. Search jumps to any entity.

**S2. Questions.** Answer these correctly against the fixture's truth:
- "How does observability work here?"
- "What's unmonitored?"
- "What must release, in what order, to ship a change to the shared library?"
- "Who's on a stale version?"
- "How does the web app reach the worker?" (each hop with its evidence)

**S3. Top-down plan.** "Add retries to the worker" goes from intent through every stage to next actions attached to real components. AI proposals stay ghosts until accepted. Lint flags a vague action and a missing done-test. The accepted plan becomes version 2, and its diff from version 1 renders in the same view as S4.

**S4. Tweak and hand off.**
1. Regroup two components, and change the intended architecture.
2. The intended-vs-actual diff appears with its impact radius.
3. Approve it. The implementer's code diff comes back for review, and you merge it.
4. Reconciliation shows the item resolved, and classifies the planted unplanned change and regression correctly.
5. A rescan keeps the regrouping.

**S5. Artifact anatomy.**
- Break down the snapshot of this prompt. Its parts match its sections.
- Break down the fixture PR: addressed and open threads are correct, and the touched components are shown.
- Break down one skill and one CI workflow.
- Edit a breakdown, and see a text diff to its source.

**S6. State machines.** Extract the fixture lifecycle from code and from the doc, and flag their drift. Extract the process-phase machine from the prompt snapshot, against a truth frozen beforehand. Planroom's own escalation and hand-off loops are modelled, and match its code. The checks find the planted unreachable state and dead end by name, and the step-through works.

**S7. Cheap decisions.**
- An independent labeller, who hasn't seen Jev's outputs, labels each decision kind's eval cases with rationales. The cases are held out from threshold tuning.
- Auto-action is enabled only for kinds with observed precision ≥ 0.95 on at least 60 cases. The rest use one-tap cards.
- Evals and thresholds run on the pinned gateway model. Both adapters pass the same contract tests against fixtures built from the docs.
- A commit-triggered rescan of the fixture makes 0 Claude calls on the routine path, spends under $0.001 on Jev, and raises at most one card.
- Report the eval table at the end.

**S8. Concurrency.** Two agents and a canvas tab write at once to two projects in one workspace. Check that no writes are lost, retries are idempotent, live updates arrive within 1 s, claims and presence are visible, and a stale write becomes a diff rather than a merge.

**S9. Mod.** In a live session, every main flow can be driven from the mod. Every action is at most two keystrokes from pane focus (a slash command counts as one entry point). The implementer hand-off runs once for real, with its output recorded.

**S10. Extensibility.** A toy domain pack, loaded from outside the source tree, adds kinds, a lens and a decision kind with no core change. A new language works through the extractor contract alone.

**S11. Remote.** The canvas is published as an Artifact and synced with the workspace. An edit made there comes back as an attributed user edit.

**S12. Context curation.** On the fixture, Planroom proposes at least one `CLAUDE.md` diff and one skill, from the seeded history of implementer runs.

**S13. Quality bars.**
- The canvas is interactive in under 1 s on the fixture.
- At 200 nodes, the p95 frame time is under 20 ms. Measure it with a local Playwright trace; this is evidence, not a CI gate.
- At 390×844 with touch emulation: no overflow, tap targets of at least 44 px, pinch-zoom and tap-to-focus work, and decision cards take one tap.
- Three named motion moments (accepting a ghost, a diff resolving, a level zoom) each have a short recorded capture, plus a `prefers-reduced-motion` test.
- No console errors.

**S14. Release.**
- CI is green on the pushed branch.
- From a fresh clone, the release build (`cargo build --release` plus the frontend build) runs on the built fixture.
- **Plugin install:**
  1. In a fresh session, run `/plugin marketplace add <local repo path>`, then `/plugin install planroom@<marketplace name>`.
  2. The engine resolves on first use. Test the source-build path live, and the release-asset path against a locally built, checksummed asset.
  3. `/room` works, the skills and agent types are listed, and an agent can drive S1 and S2 using only the plugin's tools and skills.
- The docs, art, changelog and release notes are complete.

**S15. Tiny surface and principles.**
- `planroom --help` and the mod's command list are generated and snapshot-tested: one binary, one command family.
- The UI exposes no top-level concepts beyond R14's five.
- An empty-state screenshot audit finds no blank states.
- The error-code schema has a drift test.
- Every edit type has an undo test.
- The boundary checks fail on a planted bad import in a temporary copy, in both Rust and TypeScript.
</acceptance_scenarios>

<fixture>
Nested git repos don't survive a commit (only a gitlink is stored), so commit these instead:
- plain source trees;
- a review-threads JSON file in a format you document;
- a seeded history of implementer runs;
- a deterministic `examples/build-fixture` script that makes fresh git repos (commits, tags, branches) in a temp directory for each test run.

All merges happen in that copy. The repos form a small polyglot workspace:
- a TS web app, a Python worker and a Go service, wired by docker-compose and an OpenAPI spec;
- a shared library that one repo publishes and another consumes at a stale version;
- OpenTelemetry instrumentation, a Prometheus alert rule and a dashboard JSON;
- an order lifecycle in code, plus a doc that describes it slightly differently;
- tags, changelogs, and a PR with review threads, one of which a later commit addresses;
- planted known positives, each with its exact expected name: an unreachable state, a dead end, the doc-vs-code drift, one unplanned change and one regression.

Also copy this prompt to `examples/kickoff-snapshot.md` at kickoff. S5 and S6 run on that snapshot. Never apply diffs to the root `KICKOFF.md`.

**Freeze the truth first.** Write `examples/TRUTH.json` before any extractor exists, and have an independent agent review it against the fixture. After that, change it only through a decision record.
</fixture>

<definition_of_done>
- Every tier A scenario passes on its CI path, with its evidence recorded. Every tier B scenario passes, or is a documented gap with evidence. Any substitution has an approved decision record. Failing tier C entries are listed as known gaps.
- The final review found nothing material, or what was left after 3 rounds is recorded as accepted risk.
- Everything is committed and pushed, and CI is green. Screenshots are under `docs/screenshots/`. The Iris art and its prompts are committed.
- The final message leads with the Artifact link and what works. Then the evidence: test and CI output, the S7 eval table, screenshot paths, Iris and Jev spend, and review outcomes. Then the known gaps, honestly, and the exact clicks I owe (tag, release, any registry names or tokens, repo settings).
</definition_of_done>

<constraints>
- **Packaging.** Everything ships as one Claude Code plugin, installable from this repo's `marketplace.json`.
  - The plugin carries the mod, skills, agent definitions (the implementer and the curator) and any tool or MCP wiring.
  - Plugins can't contain a `bin/` folder, so the engine binary is fetched on first use from a GitHub release, checksum-verified in the style of Iris's `install.sh`. The fallback is `cargo install --git` or a local build.
  - The release workflow builds the engine for Linux and macOS, on x86_64 and arm64.
- **Languages.** The engine is **Rust**: scanning, extraction, the store, the server, the reconciler, diffs, the Jev and Claude adapters, and the CLI. The mod and the canvas are **TypeScript**; the local server serves the canvas's built assets.
- **One contract source.** Schemas for the store, API, CLI `--json` output, tools and packs are generated for every language from one source, with a drift test. Never hand-copy types across the language boundary.
- **Repo tooling.**
  - One task entry point runs the same check, test, end-to-end, generate and release-dry-run commands locally and in CI, and agents use only those commands.
  - Toolchains are pinned, lockfiles are committed, and there's one Cargo workspace plus one JS workspace.
- **Build on good libraries.** Use mature, maintained, widely used, permissively licensed libraries rather than hand-rolling parsing, graphs, layout, storage, HTTP, diffing, git, CLI parsing, UI primitives or animation. Hand-roll only Planroom's core: the kernel, decision policy, reconciliation and packs. Pin versions, audit dependencies in CI (`cargo-deny`, as in Iris, plus an npm audit), and record significant choices and the alternatives you rejected.
- **Jev.** Jev (TypeSafe AI) handles cheap decisions, behind one adapter that supports both the Vercel AI Gateway and direct TypeSafe.
  - Pin `typesafe-ai/jev-1.14.0` on the gateway.
  - On the direct API, pin a versioned ID if the docs list one. Otherwise send `jev-latest` and record the version the response reports.
  - Ignore thejevai.com, which isn't affiliated with TypeSafe.
- **Claude.** The mod uses the chat model and `$.model.complete`, the Artifact uses `sample`, and the engine uses Claude models on the gateway, with the model pinned in config.
  - The engine reads its gateway key from environment or config. The credential proxy exists only in this build container.
  - `OPENAI_API_KEY` and `GEMINI_API_KEY` are for Iris only, never for inference.
  - When no model is reachable, every surface falls back to the deterministic extractor tiers.
- **Deterministic CI.** CI needs no secrets and no live model. Every agent and model boundary has a stand-in: the fixture implementer, and recorded or replayed responses.
- **Iris** (my CLI, `doodla/iris`) makes all images: logo, README hero, a 1280×640 social preview, and empty-state art. Spend under $5, with the prompts committed next to the images.
- **Look and feel.**
  - Bold and playful, with craft: a tactile, toy-like planning table for engineers, not a corporate PM tool. Commit to one concept across the canvas, the mod and the art.
  - Distinctive typefaces (not Inter, Roboto, Arial, system-ui or Space Grotesk). A cohesive palette as CSS variables. A dark mode that's designed, not inverted. Motion kept to a few high-impact moments, respecting `prefers-reduced-motion`. Mobile-first.
  - Don't use: a cream or off-white background, purple-on-white gradients, italic accent words in headlines, "01/02/03" section labels, monospace labels as decoration, pill buttons everywhere, or a generic three-card hero.
- **Release bar.** Match `doodla/iris`. Clone it with `GIT_LFS_SKIP_SMUDGE=1 git clone --depth 1 https://github.com/doodla/iris` and study its README, `AGENTS.md`, `CONTRIBUTING.md`, `SECURITY.md`, `CHANGELOG.md`, `docs/` (including `docs/contributing/style-guide.md`) and `.github/`. That means:
  - versioned public contracts with committed schemas and drift tests;
  - docs split into guides, concepts, reference and contributing, with each fact in one place;
  - `AGENTS.md`, imported at the top of `CLAUDE.md`, keeping `CLAUDE.md`'s existing sections and the `.claude/guardrails/` hooks and tests (run them in CI too);
  - a code of conduct, issue and PR templates, and Dependabot;
  - CI: Rust `fmt`, `clippy` with warnings as errors, tests and `cargo-deny`; TypeScript lint, format, typecheck and tests; end-to-end tests; plugin and mod validation and tests; schema drift;
  - a release workflow.
- **Spend.**
  - Live Jev, gateway and `sample` calls by the product and its tests stay under $3 in total during the build. Gate every live test behind an env flag.
  - The build's own agent tokens have no hard cap, but stay proportionate: see the review protocol in `<process>`. Log rough spend per phase.
- **Code.** Minimal: no speculative abstractions, no defensive code for impossible cases, no hard-coding to pass tests.
- **Don't, without me:** publish packages (crates.io or npm), push tags, create GitHub releases, or change repo settings. Prepare them, and list exactly what I need to click.
</constraints>

<verified_facts>
- **Environment:**
  - Rust 1.97 (`cargo`, `clippy`, `rustfmt`), Node 22 and Claude Code 2.1.310 are installed.
  - Chromium and Playwright are pre-installed: `require('playwright')` works and `chromium.launch()` needs no options. Don't reinstall them.
  - `*.vercel.sh` calls get credentials injected by the proxy, and the gateway serves Claude models (a live chat-completions call worked).
  - `OPENAI_API_KEY` and `GEMINI_API_KEY` are set. `TYPESAFE_API_KEY` is not.
- **Jev through the gateway:**
  - `POST https://ai-gateway.vercel.sh/v1/evaluate` with `{ model: "typesafe-ai/jev", state, questions }`. `state` is a string, object or array.
  - Question types: `boolean` → `{ probability }`; `choice` (`criteria`: a record of option names to descriptions) → `{ choice, probabilities, confidence }`; `score` (`criteria`: an array ordered low to high) → `{ score, probabilities }`.
  - Several questions go in one call. Limits: 32k tokens for the state plus the longest question, and 64k per request.
  - AI SDK: `experimental_decide`. A TypeSafe-compatible API is at `https://ai-gateway.vercel.sh/typesafe`. Docs: https://vercel.com/docs/ai-gateway/modalities/decision
  - A live call took about 50 ms and cost about $0.000016. `typesafe-ai/jev-1.13.0` and `typesafe-ai/jev-1.14.0` exist.
- **Jev direct:**
  - `POST https://api.typesafe.ai/v1/systemone` with `Authorization: Bearer $TYPESAFE_API_KEY` and `model: "jev-latest"`. The boolean type is `noul`, answering `{ noul: <p> }`, and usage is snake_case.
  - Read https://docs.typesafe.ai/llms.txt first. Test with fixtures built from the docs, plus a key-gated smoke test that records the reported version.
- **Iris:**
  - `curl -fsSL https://raw.githubusercontent.com/doodla/iris/main/install.sh | sh` installs it into `~/.local/bin`, and `iris doctor` passes here.
  - It never picks a model: choose one from `iris models list` and pass `-m`.
  - `--dry-run --json` shows the cost first, and `--max-cost` caps it. Generate at a supported size, then crop.
- **Mods and plugins:**
  - Load the `plugin-authoring` skill before writing mod code. Its types are the reference.
  - The API includes `$.http.fetch`, `$.process.spawn`, `$.fs`, `$.store`/`$.state`, `$.tool.register`, `$.agent.register`/`$.agent.spawn` (with `isolation: 'worktree'`), `$.model.complete`, `$.command.register` and `$.ui.*` (Pane, AbovePrompt, status, toast).
  - Panes draw from a fixed element set (Box, Text, Button, Markdown, and Svg off the terminal), not a DOM.
  - The first mod file you write triggers a one-time "turn on mods" question, which I'll answer.
  - `claude plugin validate` and `claude plugin test` exist.
  - Install is `/plugin marketplace add <source>`, then `/plugin install planroom@<marketplace name>`. There's no `--marketplace` flag.
  - A plugin's `name` is lowercase letters, digits and hyphens (at most 64). **A plugin with a `bin/` folder is refused.** Agent types come from `agents/*.md` or `$.agent.register`.
  - Ask the `claude-code-guide` agent how plugins declare skills and MCP servers before designing around them.
- **Artifacts:**
  - Load `artifact-design` and `artifact-capabilities` before writing the page.
  - `sample` asks Claude. `db` is shared storage that the session reads and writes with the ArtifactData tool.
  - Pages can't call arbitrary hosts, so they can't reach the local server or Jev.
- **CI:** Iris's CI triggers only on pushes to `main` and on PRs. Planroom's must also trigger on pushes to any branch.
</verified_facts>

<process>
Plan from the top down, review independently, and build in thin vertical slices.

**With `ultracode` on,** run one workflow per phase, per milestone and per review round, and read each result before starting the next. Without it, use subagents in the same shapes.
- Workflow `agent()` calls need literal `model:` and `effort:` options, or a `.claude/agents/` type. The hook can't read a spread options object.
- Keep Artifact publishing, consent-dependent steps and git pushes in the main session.

**Review protocol** (every review below uses it):
- Reviewers get the work, not your reasoning, and are at your model level or above. Each has a distinct lens. A separate verifier tries to refute each finding.
- Fix every unrefuted finding that's **material**: it would make a tier A or B scenario fail, violates a principle or constraint, or is a security or data-loss risk. Log the other unrefuted findings as known gaps.
- Don't vote across lenses: a security finding must not need the UX reviewer's agreement.
- Repeat until a round has nothing material, at most 3 rounds per review. Record anything left as accepted risk with a one-line reason.

1. **Understand.** Read the verified docs, the mod and Artifact skills, and Iris's repo. Spike the riskiest unknowns before designing:
   - how dense a mod pane can be;
   - Artifact `db` sync;
   - Jev latency and calibration on real plan-node slices;
   - tree-sitter performance;
   - edge roll-up performance at 200+ nodes.
2. **Architecture, top down.** Work from requirements to capabilities, then architecture, then modules and contracts, then milestones. Write up to 3 candidate architectures (simplicity-first, extensibility-first, concurrency- and agent-first), score them against the requirements and principles with parallel judges, and synthesize the best, grafting in the runners-up's good ideas. Deliver:
   - `docs/contributing/architecture.md`, with C4 diagrams;
   - `docs/contributing/decisions.md`, where each decision record names the starting point it kept or replaced;
   - the schemas and contracts;
   - `docs/contributing/repo-layout.md` (R16): the directory map, boundaries and their enforcement, task commands, toolchain pins, codegen flow and conventions;
   - a traceability table (requirement or scenario → design element → planned test);
   - `feature_list.json`.

   Lay out the skeleton directories and tooling before any feature work.
3. **Architecture review.** Use up to 9 lenses:
   - expected-but-unasked (reads only the requirements and asks what a seasoned team would have built that isn't here);
   - correctness and concurrency;
   - extensibility and core simplicity;
   - repo design (R16, including how easily an agent finds its way around);
   - the principles;
   - user surface and UX;
   - security and privacy (secrets, scanned repos, prompt injection from scanned content);
   - operability and release;
   - cost.
4. **Walking skeleton.** One thin slice through every surface before deepening any of them: scan the fixture, a node in the store, the mod pane showing it, the canvas showing it, the Artifact publishing it. Load it all through the plugin install from day one, with the engine built from source. This raises the mod and Artifact consent prompts while I'm likely still around. Update the architecture with what you learn.
5. **Milestones,** each ending shippable and green:
   1. kernel, store, code and plan packs, mod and canvas basics;
   2. diffs, reconciliation, hand-off, Jev decisions;
   3. multi-repo, observability, release;
   4. artifacts, state machines, anatomy;
   5. context curation, polish, release.

   Build independent tracks in parallel worktrees. Within a track, work one feature at a time: verify it, set its `feature_list.json` entry to passing, commit. Every track works from `repo-layout.md` and `AGENTS.md`. The main agent merges tracks one at a time, sends structural deviations from R16 back to the track, runs the full suite after each merge, and is the only one who pushes. Each milestone ends with a review of its diff, a check of the code against the architecture (fix drift or update the decision records), and a context review.
6. **Release.** Docs, Iris art, CI, the release workflow, the changelog and `v0.1.0` notes. Then a final review of the whole repo against this prompt, including a completeness critic who asks what's missing, unverified or untested.

**Context review and skills,** at the end of every phase:
- Correct or cut stale agent context, and add the invariants you've learned.
- Turn a procedure into a project skill in `.claude/skills/` once you've done it twice and expect to do it at least 3 more times. Use `skill-creator`, with at most 2 skill evals per phase.

**Verify by using the product.**
- Drive the canvas with Playwright as a person would, and screenshot every main view at 1440×900 and 390×844, in both themes.
- Critique each screenshot against the principles and the look and feel: is the next action obvious? Is any decision costlier than it needs to be?
- Fix, then re-shoot.

Context compacts automatically, so don't wind down early to save tokens.
</process>

<design_starting_points>
- **Kernel.**
  - A typed property graph of four primitives:
    - Entity `{id, kind, level, parent?, props, layer, provenance}`;
    - Relation `{id, kind, from, to, props, evidence[], confidence, layer, provenance}`;
    - Event (append-only; everything else is derived from events);
    - View (a saved query plus layout and overrides).
  - `layer` is actual or intended, so drift is a diff between layers.
  - Stable, namespaced IDs such as `<repo>:<kind>:<key>`.
- **Packs.** Declarative files that add kinds, levels, roll-up rules, extractors, lenses, decision kinds and a default view.
- **Levels.** C4 (System, Container, Component, Code) with Intent on top. One parent per node, and edges roll up with counts.
- **Extractors,** in tiers:
  1. universal: file tree, git history (co-change, ownership, churn), manifests;
  2. syntactic: tree-sitter queries and import resolvers for TS/JS, Python, Go and Rust;
  3. precise: SCIP/LSIF or a language server, when present;
  4. config: Docker, compose, k8s, Terraform, CI, OpenAPI, protobuf, OTel, Prometheus, dashboards;
  5. inference: Claude for groupings, Jev for cheap calls, both marked as inferred.

  For artifacts: deterministic parsing first, then Claude to segment, then Jev to classify.
- **Store.**
  - A single-writer local server per workspace: HTTP commands and queries, plus a scoped live stream.
  - Optimistic concurrency on entity versions.
  - SQLite in WAL mode for events, derived state and a decision cache.
  - A deterministic `.planroom/` export per repo (intended layer, overrides, plans, views), reviewable in PRs.
  - A session and agent registry with heartbeats and claims (leases that warn, not block).
  - Events carry the workspace, project, actor, cause and an idempotency key.
  - A typed query builder plus named queries (neighbourhood, paths, roll-ups, diffs).
- **Remote mode,** largely forced by the Artifact's limits:
  - The workspace is the source of truth, and the session pushes snapshots to the Artifact `db`.
  - Artifact edits land as events in a `db` collection. The session imports them on `/room open`, at turn start, and on edit notices.
  - The Artifact shows cached decisions and queues new ones.
- **Jev policy.**
  - Each decision kind declares its question, thresholds and escalation.
  - Answers are cached by a hash of the state and question.
  - State is a scoped slice from the query API.
  - Structural lint runs in code; semantic lint goes to Jev.
- **Canvas kit:**
  - a zoom rail and breadcrumb, with semantic folding and edge bundling;
  - lenses;
  - a focus (ego) view;
  - a path finder;
  - an impact-radius heat ramp;
  - a dependency matrix synced with the graph;
  - an anatomy outline;
  - a statechart view with a simulator;
  - evidence popovers;
  - a minimap and search;
  - a stable hierarchical layout with pinnable positions.
- **Repo layout sketch:**
  ```
  Cargo.toml · pnpm-workspace.yaml · justfile (or cargo xtask) · rust-toolchain.toml · .node-version
  crates/    planroom-schema (contract source) · -kernel · -store · -extract-* · -packs · -decide (Jev + Claude) · -server · -cli
  packages/  schema (generated TS) · canvas · mod · ui
  plugin/    .claude-plugin/ · skills/ · agents/ · hooks/ (built mod) · engine bootstrap
  examples/  fixture sources · build-fixture · TRUTH.json · kickoff-snapshot.md
  docs/      guides · concepts · reference · contributing
  ```
  For boundary checks: a crate-graph test or `cargo-deny` bans in Rust, and `dependency-cruiser` or `eslint-plugin-boundaries` in TypeScript.
- **Libraries to evaluate first:**
  - **Rust:**
    - `tree-sitter` and its grammars;
    - `petgraph`;
    - `rusqlite` or `sqlx`;
    - `axum` and `tokio`;
    - `serde` and `schemars`;
    - `clap`;
    - `gix` or `git2`;
    - `notify`;
    - `similar`;
    - `rmcp` (if there's an MCP server);
    - `tracing`;
    - `reqwest`.
  - **TypeScript:**
    - Vite;
    - React with `@xyflow/react`;
    - `elkjs`;
    - Motion;
    - Radix UI;
    - TanStack Query or a small store;
    - XState for the simulator;
    - a schema-to-TS generator (`json-schema-to-typescript`, `ts-rs` or `specta`).
</design_starting_points>

<autonomy>
A standing instruction from the user, the person you are working for. It is about how your turns end. A message with no tool call in it ends your turn, and the work stops there until you are asked to continue. The user has seen you end turns in four ways while work they asked for was still owed, and does not want any of them. One: a long summary of what was done that closes by announcing the next step and has no tool call, so the next thing never starts. Two: an offer to carry on with something unless the user would prefer otherwise, which stops to wait for an answer the user was not going to give. Three: a list of decisions for the user when, by your own account, none of them blocks the rest of the work. Four: deciding that this is a good place to report, because the turn has been long or a milestone is done. Status notes are welcome, and so are your recommendations on open decisions, but put them in the same message as your next tool call and carry on with whatever does not depend on the user's answer. If you notice yourself inviting the user to redirect you or offering to wait, delete it and do the next thing. The stops the user does want are the ones where nothing can move without them, or where the thing blocking you is deliberately protected from you. This does not override the need for confirmation on risky or destructive actions.

Make reasonable assumptions and record them in `progress.md` instead of asking. The only prompts that need me are the Artifact consent dialogs and the "turn on mods" question.
</autonomy>
