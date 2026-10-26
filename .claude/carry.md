# Carry (current, load-bearing only)

Task: execute `KICKOFF.md` (ultracode on). Full record: `progress.md`. Branch `ccr-b124f0e3-xnzjca`; push with `git push -u origin ccr-b124f0e3-xnzjca`.

## Where I am
- Phases 1-3 DONE. Phase 4 skeleton DONE and live. Review rounds 1-2 DONE (DR-85, DR-86). Round 3 (last) triaged: 9 material; ops-r3-1 fixed (CI); GUARD3 merged 5210c3b, ENGINE3 merged c9ae578 (not pushed yet). Worktrees removed.
- NOW (#10): full suite on c9ae578 running (logs scratchpad/suite-*.log); then DR-87 (ImportResult.batches removed, schema_version stays 1 pre-release; repos_unborn; scan_superseded reason head_moved|repos_changed; per-protocol -c flags; guard: shell cwd per call, every tool's paths, plugin root write-protected, uploads only from share/; doc conflicts from both track reports in wf_ace9eceb-7d9 journal); fold + checker; README round 3 (scratchpad/rows3.md); accepted-risk.md (round-3 fixes not re-reviewed; checks instead; open: intermittent Ubuntu CI, residual guard risks -> SECURITY.md M5; sign-off); progress log; push after prior CI run done; CI green.
- #11: plugin/AGENTS.md line: unload this repo's live mod before editing plugin/ (root write-protected); carry.
- #12 M1: plan done in wf_90589d7c-748 (41 tracks, 11 waves W0-W10; read task output file / journal). Write DR-88 (S1 passes in M3; M1 compares its packs' items; plan rules) + DR-89 (split lint test); save plan to progress.md; launch W0 via merge-track.
- Skills: merge-track, review-round. Lesson: chain check && test && push; kill own PIDs only; wait for a merge's CI run before pushing again.

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
