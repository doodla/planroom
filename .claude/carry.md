# Carry (current, load-bearing only)

Task: execute `KICKOFF.md` (ultracode on). Full record: `progress.md`. Branch `ccr-b124f0e3-xnzjca`; push with `git push -u origin ccr-b124f0e3-xnzjca`.

## Where I am
- Phases 1-3 DONE. Phase 4 skeleton: all tracks merged, DR-81..84, docs fold done, real mod ran LIVE (research/ws8-install.md), Artifact round trip on real platform (ws7-remote.md). CI green at 3e435fd+. Dev mods unloaded.
- Skill: .claude/skills/merge-track (use it for every track merge; eval 7/7 vs 6/7).
- Running: skeleton review wf_5ee442c6-106 (correctness, security, drift, tests) + wf_dc8e7240-2fc (repo, ux, operability, unasked), each lens with a refuter. Next: fix every unrefuted MATERIAL finding (round 2 if needed, max 3), log minor as known gaps (docs/contributing/research/skeleton-review/), then M1.
- M1 plan (115 h): tracks K (fold+IDs, kernel/store), X (extractors+linkers), P (packs+plan pack, after K/X), A (actor/security+backups/.git/planroom), M (mod), C (canvas), G (S1 path, gates leftovers, serve/resolver). Add default excludes for fixture/test-data trees. Shrink disk: [profile.dev]/[profile.test] debug="line-tables-only" before parallel worktrees (~9 GB free).
- Lesson: chain `check && test && commit && push`; CI covers deny/busybox/macOS.

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
