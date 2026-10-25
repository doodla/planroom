# Carry (current, load-bearing only)

Task: execute `KICKOFF.md` (ultracode on). Full record: `progress.md`. Branch `ccr-b124f0e3-xnzjca`; push with `git push -u origin ccr-b124f0e3-xnzjca`.

## Where I am
- Phase 1 Understand: research workflow wf_310271b8-f10 -> `docs/contributing/research/*.md`.
- Fixture: built (wf_18151f54-96a); review r1 had 4 material (rules ambiguous) -> README amended, fix+review r2 running (wf_f298299d-a7d). Max 3 rounds, then freeze TRUTH.json (after freeze: change only via decision record).
- Next: Phase 2 architecture workflow (3 candidates + judges) once research lands.
- Docs style rules (bind README/docs/CONTRIBUTING/SECURITY): `docs/contributing/research/iris-release-bar.md` sec 4.

## Gotchas
- 4 CPUs -> workflow concurrency is 2. Prefer few, larger agents.
- Every workflow `agent()` needs inline literal `model:` and `effort:` (hook). Agent tool calls need `model` + `effort`.
- `just` not installed; pnpm 10.28, Node 22, Rust 1.97, Go 1.24, Python 3.13 available.
- Keep Artifact publish, consent steps, git push in main session. Never edit root `KICKOFF.md`; snapshot is `examples/kickoff-snapshot.md`.
- Don't publish packages, push tags, create releases, change repo settings.
