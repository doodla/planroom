# Carry (current, load-bearing only)

Task: execute `KICKOFF.md` (ultracode on). Full record: `progress.md`. Branch `ccr-b124f0e3-xnzjca`; push with `git push -u origin ccr-b124f0e3-xnzjca`.

## Where I am
- Phases 1-3 DONE. Phase 4 skeleton DONE and live. Review round 1 DONE (DR-85, README). CI run 70 green.
- Review ROUND 2 triaged: 16 material = 12 defects (round2-*.json committed). tests-r2-1 fixed in plan (map paging into M1 S1 slice, M1 121 h, 71bd6ff). Track map for README rows: scratchpad/tracks2.json.
- Fixing round 2: wf_86ae4359-6d1 (SCAN2 track/r2-scan: key incl. repo set, no branch in facts, stale scan can't move back, no lazy fetch, per-repo outcomes; BUDGET2 track/r2-budget: warnings capped, whole-reply budget check, schema --command and share within budget) and wf_8b2a4ca7-639 (GUARD2 track/r2-guard: Bash writeDenied, recursive reads of guarded parents, generated Envelope/VersionInfo/ResolverStatus in mod, resolver sha check; CANVAS2 track/r2-canvas: 44px lid, no blank sheet).
- Then: merge (merge-track skill), DR-86 (round 2 decisions; amends DR-85's "no meta.next until M2"), README round 2 section with fix commits, progress log, then ROUND 3 (last; then accepted-risk.md), then M1.
- M1 plan: tracks K, X (dirty signal untracked; cross-repo join into linkers), P, A (fs check), M (map --focus), C, G (S1 path + map paging, gates incl. not-built ones). Default excludes for fixture/test-data trees.
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
