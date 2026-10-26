# Carry (current, load-bearing only)

Task: execute `KICKOFF.md` (ultracode on). Full record: `progress.md`. Branch `ccr-b124f0e3-xnzjca`; push with `git push -u origin ccr-b124f0e3-xnzjca`.

## Where I am
- Phases 1-3 DONE. Phase 4 skeleton DONE and live. Review round 1 DONE: 18 material fixed and merged (DR-85; README rows name fix commits), docs folded + checked + fixed (30898ce, pushed, check ok).
- Found after merge: canvas store load race (stale load overwrote a newer level's request; CI run 66 e2e) fixed with load generations (6e07f6a); scan --json map now capped to scan's budget.
- Running: review ROUND 2 on 30898ce: wf_dc32e19d-2a4 (A: correctness, security, drift, tests) and wf_be838c9f-a82 (B: repo, ux, operability, unasked). Triage with .claude/skills/review-round/scripts/triage.py --round 2. Max 3 rounds; after round 3 write accepted-risk.md.
- Then: context review + skills pass close-out (task #11), progress.md log, then M1 (task #12).
- M1 plan: tracks K (fold+IDs), X (extractors+linkers, dirty signal with untracked files per DR-85), P (packs+plan pack), A (actor/security, backups, .git/planroom, fs check), M (mod; map --focus), C (canvas), G (S1 path, gates incl. not-built ones, serve/resolver). Default excludes for fixture/test-data trees.
- Skills: merge-track, review-round. Lesson: chain check && test && push; kill own PIDs only.

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
