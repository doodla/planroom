# Carry (current, load-bearing only)

Task: execute `KICKOFF.md` (ultracode on). Full record: `progress.md`. Branch `ccr-b124f0e3-xnzjca`; push with `git push -u origin ccr-b124f0e3-xnzjca`.

## Where I am
- Phases 1-3 DONE. Phase 4 skeleton DONE and live (ws8-install.md, ws7-remote.md). CI green at 3e435fd+.
- Skeleton review round 1 DONE: 94 findings, 18 material (index: docs/contributing/research/skeleton-review/README.md). Commit 20179de (local) also scheduled repo-1/unasked-4 in M1 (+4 h, M1 119 h) and set debug=line-tables-only.
- Fixing: wf_8e14921b-8a3 (SCAN track/fix-scan, CONTRACT track/fix-contract) and wf_4d5fbffc-d36 (ENGINE track/fix-engine, SURFACES track/fix-surfaces). Me, main checkout: PLUGIN = ops-2 (source build runs the canvas build, prunes src), ops-3 (README install lines), repo-1 (`not built (M1)` lines in check).
- Then: merge the 4 tracks with merge-track skill; write DR-85 (envelope `summary`, review fixes, doc conflicts); progress.md M1 119 h / total 380 h; round 2 review of the fixes (max 3 rounds); then M1.
- M1 plan: tracks K (fold+IDs), X (extractors+linkers), P (packs+plan pack), A (actor/security, backups, .git/planroom, fs check), M (mod), C (canvas), G (S1 path, gates leftovers, serve/resolver). Add default excludes for fixture/test-data trees.
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
