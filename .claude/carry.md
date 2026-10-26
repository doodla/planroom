# Carry (current, load-bearing only)

Task: execute `KICKOFF.md` (ultracode on). Full record: `progress.md`. Branch `ccr-b124f0e3-xnzjca`; push with `git push -u origin ccr-b124f0e3-xnzjca`.

## Where I am
- Phase 1 Understand: research workflow wf_310271b8-f10 -> `docs/contributing/research/*.md`.
- Fixture: built (wf_18151f54-96a); review r1 had 4 material (rules ambiguous) -> README amended, fix+review r2 running (wf_f298299d-a7d). Max 3 rounds, then freeze TRUTH.json (after freeze: change only via decision record).
- Phase 1 research done (findings table in progress.md). Phase 2 workflow wf_c692d467-171 running: candidates -> judges -> synthesis drafts docs/contributing/{architecture,decisions,contracts,repo-layout,traceability}.md + feature_list.json. Then I review/edit, then Phase 3 review workflow (9 lenses + refuters).
- Spikes done: mod (research/mod-spike.md: dock 49x34 at 120x40; bare digit presses band Button; /room+hotkey = 2 keys). Artifact probe https://claude.ai/artifact/3U8hgkkX2Rp1ECATZCptXD (research/artifact-spike.md); page-side results land in db inbox/<uid>/probe/latest when someone opens it -> read back later.
- Spike mod lives in ~/.claude/dev-mods/<session>/planroom-spike (consent question raised); delete it once the real mod loads.
- TRUTH.json FROZEN (commit 8053725) after 3 review rounds; changes only via decision record. `examples/check-fixture` verifies it (CI it).
- Art in docs/art/ (Iris $0.27); social preview needs wordmark overlay later.
- Phase 3: rounds 1+2 fixed (DR-44..72; records in docs/contributing/research/arch-review/round-{1,2}/). Round 3 (LAST) running: wf_ff925915-1f5 + wf_906aafcd-e69 -> save to round-3/, decide, fix, then record leftovers as accepted risk.
- Phase 4 tracks running in worktrees: ENGINE (WS3 scan/store/map + WS4 concurrency spike) and PLUGIN (packaging, resolver, mod shell + harness). Verify each (xtask check/test myself), merge one at a time, push. Then WS5 wiring, WS6 serve+canvas, WS7 artifact (main session), WS8 install.
- WS1+WS2 merged (3cd4e66): xtask check/test green (verified). Gaps: cargo-deny never run locally; e2e/installer/release jobs are if:false stubs; layout gate, release pins, secret scan, catalogue.ts not built yet.
- CI green on run #6 (all jobs incl. macOS).
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
