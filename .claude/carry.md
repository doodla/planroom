# Carry (current, load-bearing only)

Task: execute `KICKOFF.md` (ultracode on). Full record: `progress.md`. Branch `ccr-b124f0e3-xnzjca`; push with `git push -u origin ccr-b124f0e3-xnzjca`.

## Where I am
- Phases 1-3 DONE. Phase 4 skeleton DONE and live. Skeleton review round 1: 94 findings, 18 material (docs/contributing/research/skeleton-review/README.md).
- Round 1 fixes MERGED locally (not pushed yet): fix-engine, fix-surfaces, fix-scan (89c1bd4), fix-contract + DR-85 + layout.lock (90476d3). My own: resolver canvas build, README install lines, check's not-built lines (CI run 64 green).
- Running: full suite on 90476d3 (bg task bfk2y75lh; status in scratchpad/m-status.log), and docs fold of DR-85 (wf_09ee2ebd-c73, branch track/docs-dr85 + checker).
- Next: suite green -> push; merge docs fold; update review README rows to "fixed (commit)"; progress.md log; then round 2 review (review-round skill), max 3 rounds; then M1.
- M1 plan: tracks K (fold+IDs), X (extractors+linkers, dirty signal with untracked files per DR-85), P (packs+plan pack), A (actor/security, backups, .git/planroom, fs check), M (mod), C (canvas), G (S1 path, gates incl. not-built ones, serve/resolver). Default excludes for fixture/test-data trees.
- Skills: merge-track, review-round (eval: 5/5 both, non-discriminating; RESULTS.md).
- Lesson: chain `check && test && commit && push`; CI covers deny/busybox/macOS. Don't pkill broadly: kill own PIDs.

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
