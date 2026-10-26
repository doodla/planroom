# Carry (current, load-bearing only)

Task: execute `KICKOFF.md` (ultracode on). Full record: `progress.md`. Branch `ccr-b124f0e3-xnzjca`; push with `git push -u origin ccr-b124f0e3-xnzjca`.

## Where I am
- Phases 1-3 DONE (research, architecture, 3 review rounds; accepted risks in docs/contributing/research/arch-review/round-3/accepted-risk.md). Estimate 376 h.
- Phase 4 walking skeleton: WS1-WS4 + PLUGIN shell merged. Running: wf_fb2eb3a8-670 (track/engine-2: version/focus/person tokens/serve/map graph; track/canvas: ui tokens + canvas app + e2e screenshots) and wf_c5faf8db-e42 (track/mod-2: mod<->engine wiring; docs DR-81 fold of build feedback, edits docs in main tree).
- Merge order: engine-2 -> canvas -> mod-2 (each: verify xtask check/test myself, resolve, push). Then WS7 Artifact round trip in MAIN session (share/publish/import), WS8 install test (claude plugin marketplace add/install in temp HOME), then skeleton review + update architecture with findings, then M1.

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
