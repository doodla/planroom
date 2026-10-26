# Carry (current, load-bearing only)

Task: execute `KICKOFF.md` (ultracode on). Full record: `progress.md`. Branch `ccr-b124f0e3-xnzjca`; push with `git push -u origin ccr-b124f0e3-xnzjca`.

## Where I am
- Phases 1-3 DONE. Phase 4 skeleton DONE and live. Review rounds 1 and 2 DONE and merged (DR-85, DR-86; README has both rounds with fix commits). Full suite green on 492bb69; unborn repo maps nothing (11fe179). Pushed.
- Running: DR-86 docs fold + checker (wf_a71b0977-635, branch track/docs-dr86). Then merge it, fix checker findings.
- Next: ROUND 3 (the last): regenerate r2a/r2b scripts (scratchpad) as r3 with the new commit, "Round 3" paragraph naming both rounds; triage --round 3; fix material; then accepted-risk.md (round 3's fixes un-reviewed; what stays open), progress log, close task #10 and #11; then M1 (#12).
- M1 plan: tracks K, X (dirty signal untracked; cross-repo join into linkers, then per-repo keys), P, A (fs check), M (map --focus), C, G (S1 path + map paging, gates incl. not-built ones). Default excludes for fixture/test-data trees.
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
