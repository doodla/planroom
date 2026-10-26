# Carry (current, load-bearing only)

Task: execute `KICKOFF.md` (ultracode on). Full record: `progress.md`. Branch `ccr-b124f0e3-xnzjca`; push with `git push -u origin ccr-b124f0e3-xnzjca`.

## Where I am
- Phases 1-3 DONE. Estimate 376 h.
- Phase 4 skeleton: WS1-WS4, PLUGIN, ENGINE-2, CANVAS, MOD-2 merged + pushed; DR-82 written (token wire format etc.). Token seam bug fixed in mod (secs, args keep --idem/--base).
- Running: wf_e5bfa8c0-f14 (track/watch: watch --json + real watch-frames golden; track/real-serve: canvas e2e vs real serve, Origin/CSP/routes). On finish: verify each myself (check/test/e2e), merge, delete worktree target/ dirs (disk!), append server seams to DR-82, push.
- Next round (2 agents): GATES (boundaries.toml to repo-layout spec: third_party, external_allow, network allow-list; check layout + gen --layout-lock + first lock naming DR-79; CI --commits) and REMOTE (engine share + import -, canvas transport/artifact.ts + Artifact build). Then WS7 publish/round trip in MAIN session (A-13), WS8 install in temp HOME + spikes (spike mod now has store/env/version probes: call spike_probe what=store|env|version for A-15/A-14/A-19), skeleton review, architecture update, M1.
- CI e2e job is red until REAL-SERVE merges.

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
