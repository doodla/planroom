# Carry (current, load-bearing only)

Task: execute `KICKOFF.md` (ultracode on). Full record: `progress.md`. Branch `ccr-b124f0e3-xnzjca`; push with `git push -u origin ccr-b124f0e3-xnzjca`.

## Where I am
- Phases 1-3 DONE. Estimate 376 h.
- Phase 4 skeleton: WS1-WS6 tracks merged + pushed (ENGINE, PLUGIN, ENGINE-2, CANVAS, MOD-2, WATCH, REAL-SERVE). DR-81, DR-82 written. xtask check/test/e2e green at merge of real-serve.
- WS8 mostly done (docs/contributing/research/ws8-install.md): A-14/15/16/18/19/20 settled, A-23 fails (fallback = dev version stamped with plugin/ content hash, GATES builds it). A-15: store at <config>/plugins/store/planroom_*.json; guard fixed. Open: A-21 (git 2.34.1 CI job), spikes tool.check agentId + $.tool.call ArtifactData (spike mod has probes `checks`, `artifactdata`: spawn an Agent running Bash, then call spike_probe), comments.sendToClaude wake (needs a press on a page).
- Running: wf_942a6fbe-d44 (track/gates: boundaries/allow-lists/clippy.toml/check layout/ci_skips/area sync/0.1.0-dev version stamp/art spend check; track/remote: remote.rs, share --out, import -, canvas transport/artifact.ts, shim e2e). On finish: verify each (check/test/e2e), merge, rm worktree target/, WRITE FIRST layout.lock (gen --layout-lock DR-79 per DR-81) + DR-83 for gates/remote seams, push.
- Then: WS7 publish from MAIN session using share --json args (Artifact tool, icon map), write an inbox row as owner? (rows come from page; A-13 check: owner can't write another viewer's inbox), import round trip; skeleton review workflow; update architecture.md with findings; then M1.
- Lesson: run full `cargo xtask test` (incl. claude plugin test) before every push. CI found what local couldn't: deny (tokio 1.53.3 taken 1 day early for RUSTSEC-2026-0386 -> DR-83; BSD-3-Clause; versioned path deps), busybox-static applets (installer sandbox), macOS /private tmp, slow python3 server. Fixed in c61e9d9, 4c6d9b4, 6827792: verify CI green on 6827792. Local now has shellcheck, busybox-static, bwrap, socat (apt). cargo-deny install is blocked by the classifier: read CI logs.

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
