# Carry (current, load-bearing only)

Task: execute `KICKOFF.md` (ultracode on). Full record: `progress.md`. Branch `ccr-b124f0e3-xnzjca`; push with `git push -u origin ccr-b124f0e3-xnzjca`.

## Where I am
- Phase 1 Understand: research workflow wf_310271b8-f10 -> `docs/contributing/research/*.md`.
- Fixture: built (wf_18151f54-96a); review r1 had 4 material (rules ambiguous) -> README amended, fix+review r2 running (wf_f298299d-a7d). Max 3 rounds, then freeze TRUTH.json (after freeze: change only via decision record).
- Phase 1 research done (findings table in progress.md). Phase 2 workflow wf_c692d467-171 running: candidates -> judges -> synthesis drafts docs/contributing/{architecture,decisions,contracts,repo-layout,traceability}.md + feature_list.json. Then I review/edit, then Phase 3 review workflow (9 lenses + refuters).
- Spikes done: mod (research/mod-spike.md: dock 49x34 at 120x40; bare digit presses band Button; /room+hotkey = 2 keys). Artifact probe https://claude.ai/artifact/3U8hgkkX2Rp1ECATZCptXD (research/artifact-spike.md); page-side results land in db inbox/<uid>/probe/latest when someone opens it -> read back later.
- Spike mod lives in ~/.claude/dev-mods/<session>/planroom-spike (consent question raised); delete it once the real mod loads.
- TRUTH.json FROZEN (commit 8053725) after 3 review rounds; changes only via decision record. `examples/check-fixture` verifies it (CI it).
- Art done in docs/art/ (Iris, $0.27): logo, hero, social-base (wordmark overlay later), 3 empty states. Plugin icon = logo (512-2048 px PNG at plugin/.claude-plugin/icon.png).
- Design language: docs/contributing/design-language.md (planning table).
- Phase 3 round 1 done: 102 findings, 49 material upheld (records + my owner decisions D1-D21 in docs/contributing/research/arch-review/round-1/). Fix workflow wf_170682f9-3d0 running (kernel -> security -> surfaces -> consistency). Then round 2 review (all 9 lenses, fresh), max 3 rounds.
- WS1+WS2 merged (3cd4e66): xtask check/test green (verified). Gaps: cargo-deny never run locally; e2e/installer/release jobs are if:false stubs; layout gate, release pins, secret scan, catalogue.ts not built yet.
- After fix workflow: apply skeleton doc notes: repo-layout self-test step 4 path + not-to-unresolvable; contracts EventBody tag -> DecisionRecord.decision_kind; repo-layout Conventions -> link AGENTS.md; AGENTS.md attribution wording; record actual pins in DR-39/40.
- All WS assumptions A-1..A-4, A-8..A-11 settled (progress.md).
- Docs style rules (bind README/docs/CONTRIBUTING/SECURITY): `docs/contributing/research/iris-release-bar.md` sec 4.

## Gotchas
- 4 CPUs -> workflow concurrency is 2. Prefer few, larger agents.
- Every workflow `agent()` needs inline literal `model:` and `effort:` (hook). Agent tool calls need `model` + `effort`.
- `just` not installed; pnpm 10.28, Node 22, Rust 1.97, Go 1.24, Python 3.13 available.
- Keep Artifact publish, consent steps, git push in main session. Never edit root `KICKOFF.md`; snapshot is `examples/kickoff-snapshot.md`.
- Don't publish packages, push tags, create releases, change repo settings.

## Key research facts (details in docs/contributing/research/)
- Mod: plugin `hooks/hooks.json {modules:[...]}` -> `register.tsx`; no import()/require/Node; bundle with esbuild. Pane/band/status/toast; Svg not on terminal (use Raster). http.fetch supports socketPath, no streaming/timeout. process.spawn streams but dies on reload -> engine must self-daemonize. $.agent.spawn has no isolation: register agent type with isolation:'worktree'. Tools appear as mcp__planroom__<name>.
- Artifact: db LWW page-side, ArtifactData needs if_version; db writes don't wake the session; edits via inbox/{self} (server-enforced author). Bulk snapshot via published data file + small db head doc.
- This session is a live session the user watches in the app; dev-mods dir exists at ~/.claude/dev-mods.
