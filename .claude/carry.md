# Carry (current, load-bearing only)

Task: execute `KICKOFF.md` (ultracode on). Full record: `progress.md`. Branch `ccr-b124f0e3-xnzjca`; push with `git push -u origin ccr-b124f0e3-xnzjca`.

## Where I am
- Phases 1-3 DONE. Estimate 376 h.
- Phase 4 skeleton: ALL tracks merged (ENGINE..REMOTE, GATES). DR-81/82/83 written. layout.lock exists (first lock DR-79); any gated-surface change now needs a new DR + `cargo xtask gen --layout-lock DR-n` in the same commit.
- WS7 done on real platform (artifact 69VbWfb55J2p6kduVUJcG8, research/ws7-remote.md). WS8 done except A-21 (git 2.34.1 CI job) and press-only spikes. A-13 fails -> every unsigned row held (DR-83).
- Live check: real plugin symlinked at ~/.claude/dev-mods/<session>/planroom -> /home/user/planroom/plugin; repo scanned (store at ./.planroom, gitignored). Next turn: call mcp__planroom__room_query (e.g. focus/map) to prove mod+engine live; guard now denies my reads of .planroom paths -> remove symlink after the check (and delete planroom-spike).
- Running: wf_d998336e-ae8 docs fold (main tree, uncommitted edits to docs). Then: skeleton review workflow (lenses + refuters, protocol), fix material, context review + skills, then M1 planning (tracks).
- Dogfood findings for M1: fixture compose pollutes repo map (default excludes); store should be .git/planroom (DR-74).
- Lesson: run full `cargo xtask test` before every push; CI covers deny/busybox/macOS.

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
