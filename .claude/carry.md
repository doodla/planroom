# Carry (current, load-bearing only)

Task: execute `KICKOFF.md` (ultracode on). Full record: `progress.md`. Branch `ccr-b124f0e3-xnzjca`; push with `git push -u origin ccr-b124f0e3-xnzjca`.

## Owner rules (binding)
- Keep orchestration out of the repo and commits: no agent roles, track/wave/lens/workflow/worktree-as-process names, session IDs or bookkeeping commits. Commits say what changed and why, in Planroom's terms. Squash-merge parallel branches into one plain commit. Trailers stay. carry.md is untracked (local only).
- Owner said: rewrite all past commit messages plainly and force-push (with lease, backup ref, trees byte-identical), but only in a proper gap: no running workflows or worktrees, CI idle, merged local branches deleted, before M1 starts.
- Owner said: keep the project skills, in plain wording (merge-track -> merge-branch; no session IDs or code names).

## Where I am
- Skeleton review closed: DR-87, README round 3, accepted-risk.md, progress log; pushed through de7a4b0. CI run 90 RED (macOS: build-fixture git EINVAL under load) -> fix/fixture-once running (build fixture once per source hash, copy per test) in wf_ff18a522-ffa, with docs fixes (DR-87 check's 12 findings, 4 held-back docs, broken anchors) + independent checks.
- Uncommitted in main checkout: plain-language sweep (decisions.md, progress.md, review notes, AGENTS.md, repo-layout.md, xtask strings, layout.lock header), skills rename merge-track->merge-branch (staged), DR-87 docs patch, carry untracked (.gitignore). Commit as plain commits after the docs check; then merge fix/fixture-once with git merge --squash; push; CI green.
- Then history rewrite (owner OK, in a gap): messages drafted by wf_49bf3890-44c into scratchpad/history/msgs-*.json; scripts scratchpad/history/{lookup.py,verify.py}; dry run passed (206 commits, 0 structural diffs). Apply: backup ref, filter-branch --msg-filter on efa9446..HEAD, verify.py, force-with-lease.
- M1 plan r3: wf_8976aa34-440 (scratchpad/m1-plan-r3.md + check). Then DR-88 (plain terms; no waves/tracks in repo), launch W0 with squash merges.
- Task list: #10, #11 nearly done; #13 sweep in progress.
## Gotchas
- 4 CPUs -> workflow concurrency is 2. Prefer few, larger agents.
- Workflow worktree isolation starts from main, not HEAD: brief agents to `git checkout -B <branch> <sha>` first.
- Every workflow `agent()` needs inline literal `model:` and `effort:` (hook). Agent tool calls need `model` + `effort`.
- `just` not installed; pnpm 10.28, Node 22, Rust 1.97, Go 1.24, Python 3.13 available.
- Keep Artifact publish, consent steps, git push in main session. Never edit root `KICKOFF.md`; snapshot is `examples/kickoff-snapshot.md`.
- Don't publish packages, push tags, create releases, change repo settings.

## Key research facts (details in docs/contributing/research/)
- Mod: plugin `hooks/hooks.json {modules:[...]}` -> `register.tsx`; no import()/require/Node; bundle with esbuild. Pane/band/status/toast; Svg not on terminal (use Raster). http.fetch supports socketPath, no streaming/timeout. process.spawn streams but dies on reload -> engine must self-daemonize. $.agent.spawn has no isolation: register agent type with isolation:'worktree'. Tools appear as mcp__planroom__<name>.
- Artifact: db LWW page-side, ArtifactData needs if_version; db writes don't wake the session; edits via inbox/{self} (server-enforced author). Bulk snapshot via published data file + small db head doc.
- This session is a live session the user watches in the app; dev-mods dir exists at ~/.claude/dev-mods.
