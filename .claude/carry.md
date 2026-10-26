# Carry (current, load-bearing only)

Task: execute `KICKOFF.md` (ultracode on). Full record: `progress.md`. Branch `ccr-b124f0e3-xnzjca`; push with `git push -u origin ccr-b124f0e3-xnzjca`.

## Where I am
- Phases 1-3 DONE. Phase 4 skeleton DONE and live. Review rounds 1-2 DONE and merged (DR-85, DR-86 + folds checked and fixed; README both rounds). Suite green on 492bb69; pushed through 9cabf5f.
- Running: ROUND 3 (last) on b007ff3: wf_7568c126-595 (A) and wf_e7a0d842-821 (B). Triage: python3 -I .claude/skills/review-round/scripts/triage.py --round 3 --out docs/contributing/research/skeleton-review <journals>.
- After round 3: fix material (tracks), merge, suite; write accepted-risk.md (round-3 fixes not re-reviewed; checks instead: regression tests, repros, suite; list open items + sign-off); README round 3; DR-87; progress log; close tasks #10, #11.
- Context review done this phase: xtask test -p CRATE FILTER and --ts PATH built (e447235), review-round skill updated (9cabf5f).
- M1 next (#12): break 121 h into ~25 tracks of 4-5 h, waves of 4 (disk ~19 GB): wave 1 K-fold, K-ids, X-extract, X-link(+cross-repo join), A-actor, A-backups, G-S1(+paging), G-gates, G-serve; then P-packs, P-plan, M, C. Default excludes for fixture/test-data trees.
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
