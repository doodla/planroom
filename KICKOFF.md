# Planroom kickoff prompt

Paste everything below the line into a fresh Claude Code session on this repo.

---

<context>
I'm the only user of what you're about to build. I start a lot of fuzzy tasks ("launch a side project", "move apartments", "refactor the billing service") and I want a tool that turns a vague intent into a plan I believe in. It should go from the high level down to concrete next actions, and it should do this *with* me rather than for me. You have full autonomy: make every product, design and technical call yourself, build it end to end, and ship it. Don't stop to ask me questions.

Environment: a cloud Claude Code container with this repo (`planroom`) checked out. Chromium and Playwright are pre-installed: `require('playwright')` works and `chromium.launch()` needs no options. Don't reinstall them. Read `CLAUDE.md` first. It has a rule about setting `model` and `effort` on subagents, and a hook enforces it. Commit and push to the branch you were assigned.
</context>

<product_vision>
Planroom is two surfaces over one shared plan.

1. **The canvas (web app).** This is the main event: a big, beautiful, zoomable canvas where my plan grows as a tree or map, from intent down to next actions. The AI interviews me in a side or bottom panel. As I answer, nodes appear on the canvas. AI proposals show up as ghost (dashed) nodes that I accept, edit, drag or delete. Nothing the AI invents counts as real until I accept it. I can also add, rename, re-parent and reorder nodes by hand, at any time.
2. **The Claude Code mod.** This is a companion that lives beside the chat in Claude Code, so I can plan inside a coding session:
   - a **pane** showing the current plan's outline, its progress, the current stage, and the open question;
   - a `/plan <task>` **slash command** that starts or resumes a plan;
   - **tools the model can call** (for example `plan_get`, `plan_propose`, `plan_update`), so the Claude in the chat runs the interview and edits the plan;
   - an **SVG mini-map** of the tree on surfaces that support `Svg` (desktop, mobile). On the terminal, a clean text outline.

   Load the `plugin-authoring` skill before you write any of it. It tells you the folder layout, where the types are, how to validate and test a mod, and that I will be asked once to turn mods on.

Both surfaces read and write the **same plan JSON schema**, defined once in the repo, and can move a plan between them. At minimum that means export and import of a `.planroom.json` file. If a cleaner bridge fits the runtimes, use it. For example: the mod writes `.planroom/plan.json` in the working directory, and the Claude session syncs it into the published canvas's `db` with the ArtifactData tool.

Go well beyond the basics. I want delightful motion (springy drag and drop, nodes that bloom into place, a satisfying moment when a branch is completed). I want smart suggestions that anticipate the next thing I'd add. And it should feel native and beautiful on my phone, not just survive at 375px.
</product_vision>

<planning_method>
The AI should use established methods to pull out my real intent and break the work down. It must never just dump a to-do list. Walk me through these stages, and let me jump back to any earlier one at any time (it works like the Double Diamond: not strictly linear).

1. **Intent** (Jobs-to-be-Done, Socratic questioning, laddering). Ask what's happening that makes me want this, and what being done will let me do. Ladder up with "why does that matter?" until my answer stabilizes, usually after 2–3 steps. Don't force five Whys. Output: a root card that reads "When … I want … so I can …", which I edit and confirm.
2. **Outcome** (SMART / OKR, the Define step of the Double Diamond). Ask: how will I know it worked, by when, and what is out of scope? Output: success-measure chips (metric, target, deadline) and an explicit "Not doing" list.
3. **Breakdown** (issue tree / MECE, WBS 100% rule; or an Impact Map or a User Story Map, depending on the kind of task). Propose 3–7 children per node, all as ghosts. Pick the splitting lens that fits the task (phases or deliverables for building something, activities for a user flow, hypotheses for a decision) and say which lens you used.
4. **Next actions** (GTD, HTN). Keep decomposing until each leaf is a physical, visible action that starts with a verb, has a "done when …" line, and an estimate under about a day. Flag vague verbs such as "think about", "figure out" and "handle".
5. **Prioritize and slice** (MoSCoW, optional RICE, story-map release line). Let me tag nodes Must/Should/Could/Won't and drag an "MVP line" across the plan. Propose the thin walking-skeleton slice.
6. **Pre-mortem** (Klein). Ask: "It's three months from now and this failed. Why?" Each answer becomes a risk node on the branch it affects, and each high-severity risk gets a mitigation action placed back in the tree.
7. **Review (plan lint).** Show these as gentle suggestions on the canvas, not as pass/fail:
   - the children of each parent cover 100% of its scope, with no gaps or overlaps;
   - siblings are MECE and share one splitting criterion, with no catch-all "Other";
   - each parent has 2–7 children;
   - every leaf is actionable, has a done-test, and is neither over-split (under about 15 minutes) nor under-split;
   - every leaf traces up to the outcome, so there are no orphans or gold-plating;
   - the goal is SMART and has at least one non-goal;
   - there are no dependency cycles;
   - every Must branch has at least one risk;
   - siblings are at a similar depth and granularity;
   - inferred nodes I haven't confirmed are flagged.

Each node carries the fields its stage produces. Examples: a job statement; goal measures and non-goals; `type` (goal, phase, deliverable, action, risk); `doneWhen`; `estimate`; `moscow`; `status`; `source` (ai-proposed or user); `accepted`; dependencies. Design the schema so that a plan made on either surface opens on the other.
</planning_method>

<runtime_and_constraints>
- **Publish the canvas as a Claude Artifact** (a single self-contained HTML page) and also commit its source to this repo. A build step that inlines everything into one HTML file is fine, and so are CDN scripts from the hosts the Artifact tool allows.
  - AI inside the page comes from the Artifact `sample` capability, which asks Claude. Persistence comes from the `db` capability, with a localStorage cache.
  - Load the `artifact-design` and `artifact-capabilities` skills before you write the page. Their contracts (allowed hosts, theming tokens, dark mode, how capabilities degrade when unavailable) are binding.
  - The page must work without AI (fully manual planning) and light up when `sample` is available.
- Keep AI prompts in one module, with structured JSON outputs per stage. That way the interview logic is testable with a mock, and the mod can reuse the same stage definitions and prompts where its runtime allows.
- Commit the mod's source to `mod/` in the repo. Also place it in this session's mods folder (the skill gives the exact path) so it hot-loads here. Add a `.claude-plugin/marketplace.json` and README install line so I can install it in other sessions.
- Make the product ambitious and the code minimal: no speculative abstractions, no defensive code for impossible cases, and no hard-coding to pass tests.
</runtime_and_constraints>

<design_direction>
Bold and playful, with personality. Think of a toy-like, tactile planning table, not a corporate PM tool. Commit to one strong concept and carry it through color, type, motion and sound-free micro-feedback.
- Choose distinctive typefaces (from Google Fonts). Don't use Inter, Roboto, Arial, system-ui or Space Grotesk.
- Build a cohesive palette as CSS variables: dominant colors with sharp accents, and a dark mode that is designed, not inverted.
- Put the motion budget into a few high-impact moments: nodes blooming in, a ghost being accepted, a branch completing, stage transitions. Respect `prefers-reduced-motion`.
- Give the canvas atmosphere: a subtle texture, grid or layered gradient, not a flat white void.
- Don't use: a cream or off-white background, purple-on-white gradients, italic accent words in headlines, numbered "01/02/03" section labels, monospace labels as decoration, pill-shaped buttons everywhere, or a generic three-card hero layout.
- Design for mobile first:
  - thumb-reachable controls;
  - a bottom-sheet interview panel;
  - pinch-zoom and pan on the canvas;
  - large touch targets;
  - no horizontal page scroll.
</design_direction>

<working_method>
- First write `feature_list.json`, one entry per user-visible feature, each with a `passes: false` flag. Also write `progress.md` (decisions, assumptions, what's next) and a script that builds and runs the tests. Keep all three current as you go. Never delete or weaken a test to make it pass.
- Build one feature at a time, verify it, flip its flag, and commit with a descriptive message. Push regularly.
- Suggested order:
  1. plan schema and store;
  2. canvas rendering and manual editing;
  3. interview stages with a mock AI;
  4. real `sample` integration;
  5. lint pass;
  6. polish and motion;
  7. mobile;
  8. Artifact publish;
  9. mod pane, command and tools;
  10. bridge between the surfaces;
  11. final polish.
- Use parallel tool calls for independent work. Use subagents only for large independent chunks, following `CLAUDE.md`.
- Context compacts automatically, so don't wind down early to save tokens. Save state to `progress.md` before big steps.
</working_method>

<verification>
- Unit-test the schema, the lint checks and the interview state machine, using the mock AI.
- Drive the real app with Playwright, the way a person would. Run one full plan from "plan my move to Lisbon" through all seven stages. Accept and reject ghosts, drag nodes, set the MVP line, run the pre-mortem.
- Screenshot every main view at 1440×900 and 390×844, in light and dark mode. Look at the screenshots, critique them against the design direction (coherence, originality, craft, function), fix what's weak, and re-shoot. There should be no console errors.
- For the mod: run `claude plugin validate`, type-check it, and run `claude plugin test` with a test that exercises the slash command, the tools and the pane render.
</verification>

<definition_of_done>
- Every entry in `feature_list.json` passes. Tests are green, and the latest screenshots are reviewed and committed under `docs/screenshots/`.
- The canvas is published as an Artifact and I have the link. The mod validates, passes its tests, and is installed in this session.
- The README explains what Planroom is, how to run the web app locally, how to install the mod, and how to move a plan between the two.
- Everything is committed and pushed.
- The final message leads with the Artifact link and what works, shows the evidence (test output, screenshot paths), then lists known gaps honestly.
</definition_of_done>

<autonomy>
A standing instruction from the user, the person you are working for. It is about how your turns end. A message with no tool call in it ends your turn, and the work stops there until you are asked to continue. The user has seen you end turns in four ways while work they asked for was still owed, and does not want any of them. One: a long summary of what was done that closes by announcing the next step and has no tool call, so the next thing never starts. Two: an offer to carry on with something unless the user would prefer otherwise, which stops to wait for an answer the user was not going to give. Three: a list of decisions for the user when, by your own account, none of them blocks the rest of the work. Four: deciding that this is a good place to report, because the turn has been long or a milestone is done. Status notes are welcome, and so are your recommendations on open decisions, but put them in the same message as your next tool call and carry on with whatever does not depend on the user's answer. If you notice yourself inviting the user to redirect you or offering to wait, delete it and do the next thing. The stops the user does want are the ones where nothing can move without them, or where the thing blocking you is deliberately protected from you. This does not override the need for confirmation on risky or destructive actions.

Make reasonable assumptions and record them in `progress.md` instead of asking. The two prompts only I can answer are the Artifact consent dialogs and the "turn on mods" question. I'll handle those when they appear, so keep working on whatever doesn't depend on them.
</autonomy>
