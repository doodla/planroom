# WS8: install path and open assumptions

Walking-skeleton slice WS8, run on 2026-10-26 in the build container (Claude Code 2.1.310, Ubuntu 24.04, git 2.43.0). Each item says what was run and what it showed. `[V]` marks a result seen in a run here; `[D]` marks a result read from the 2.1.310 mod API types or the CLI's own schema.

## Install from the local marketplace [V]

In a throwaway `HOME`, `claude plugin marketplace add /home/user/planroom` then `claude plugin install planroom@planroom` both succeed. `claude plugin list` shows `planroom@planroom`, version `0.1.0`, enabled, "Read from: /home/user/planroom/plugin": a local directory marketplace is read in place.

The plugin's version is `0.1.0` on `main`, where E11 says `main` carries `X.Y.Z-dev` between releases. With `0.1.0` the resolver looks for a `v0.1.0` release asset, finds a 404 and offers the source build; with `0.1.0-dev` it would go straight to the source build. Fix: set the workspace version to `0.1.0-dev` until the release.

## The marketplace source that names a release [A-20] [V]

- A `github` source with `ref` validates and installs, but it ignores `path`: the install cached the whole repository root (`Cargo.toml`, `KICKOFF.md`, ...) and named the version by commit (`071909da5f77`).
- A `git-subdir` source installs exactly the subdirectory, sparse-checked out, at the version in its `plugin.json`:

  ```json
  { "name": "planroom", "source": { "source": "git-subdir", "url": "doodla/planroom", "path": "plugin", "ref": "v0.1.0" } }
  ```

  Tried with `ref` set to this build's branch: the cache held `.claude-plugin/`, `AGENTS.md`, `README.md`, `agents/`, `hooks/`, `scripts/`, `skills/` and `types/`, at version `0.1.0`.
- So F16's first option holds: the published marketplace entry names the release tag through `git-subdir` with `ref`, and the release workflow moves `ref` at each tag. A settings-sourced marketplace must use remote sources; a relative `./plugin` works only for a marketplace added from a repository or directory.

## The resolver from an installed copy [V]

From the `git-subdir` install, which has no checkout around it, `scripts/planroom version --json` fetched `https://github.com/doodla/planroom/releases/download/v0.1.0/planroom-v0.1.0-x86_64-unknown-linux-musl.tar.gz`, got a 404, and answered `engine_unavailable` with `reason: download_failed` and the fix `scripts/planroom build` (about 4 minutes). The build clones the repository at the tag, or at `main` for a `-dev` version or a tag not yet published, so an install holding only `plugin/` can still build. With `PLANROOM_ENGINE` naming a built engine, the same call answered `VersionInfo` with exit 0. The source build itself is S14's evidence, run at M5.

## A headless first turn with and without the plugin [V]

`claude -p "Reply with the single word ok." --model haiku --max-turns 1 --output-format stream-json --verbose` in the built fixture workspace, eight runs, recorded in `docs/evidence/s15/first-turn.json`:

| Configuration | Runs | Agents listed | Total input tokens |
|---|---|---|---|
| plugin enabled | 3 | 8 (adds `planroom:curator`, `planroom:implementer`) | 39,765 to 39,992 |
| plugin installed, disabled | 2 | 6 | 40,073 both |
| empty `HOME`, no plugin | 3 | 6 | 39,890 to 40,117 |

The plugin's three skills are `user-invocable: false`, so they are absent from the init listing [A-1]. The mod doesn't run under `-p`, since nobody can answer the mods question there: this is also the install with mods off. The run with the plugin enabled used about 200 fewer tokens than the same `HOME` with it disabled, and two identical runs differed by 106. The host's prompt assembly varies by more than the plugin's surface, so a first-turn delta can't gate that surface. That confirms DR-51's gate on `count_tokens` over the exact bytes; the deltas here are evidence only. Cost: about $0.018 for the eight runs, logged in `docs/evidence/spend.jsonl`.

## The implementer's Bash sandbox [A-18] [V]

Container prerequisites: `bubblewrap` and `socat`, installed here with apt (they weren't present).

With user settings `sandbox: { enabled: true, autoAllowBashIfSandboxed: true, allowUnsandboxedCommands: false, network: { allowedDomains: [] }, filesystem: { allowWrite: [CLONE] } }`, a headless session told to delegate to `planroom:implementer` spawned it, and the subagent's Bash calls ran sandboxed:

- `echo inside > CLONE/ok.txt` wrote the file.
- `echo outside > /root/a18-outside/bad.txt` failed: `Read-only file system`.
- `curl https://example.com` failed: `CONNECT tunnel failed, response 403`, with `<sandbox_violations> deny network-outbound example.com:443`.

A first run that wrote "outside" to a path under `/tmp/claude-0/` succeeded, since the sandbox lets every command write its `TMPDIR`; the implementer can always write there, which never reaches the person's repositories.

The sandbox is a session setting. An agent definition (`AgentSpec` in the 2.1.310 types) has no sandbox field [D], so the mod can't turn it on for the implementer alone. The mod reads it with `$.settings.read({ key: "sandbox" })` [D]. So the design's rule stands as written: with the sandbox on, the implementer's approved commands run without prompts inside its clones; with it off, they ask (`guard.test.ts::without_a_sandbox_approved_commands_ask`). The clones' directory must also be in `sandbox.filesystem.allowWrite`, so the hand-off guide (M3) needs to say how to add it.

## The mod reading its host's version [A-19] [D]

`$.session.version()` answers the engine's `version`, its `base` release and its `builtAt` build time, in every mode and build. A live call from the spike mod follows with the `$.store` spike below.

## Still open in WS8

- A-15: where the mod's `$.store` lands on disk. The spike mod now has a `store` probe, which runs once the mod reloads.
- A-14: the mod's plugin root and its children's environment, through the spike's `env` probe.
- A-16: `claudeMdExcludes` over the fixture's agent files.
- A-21: the git adapter's tests on git 2.34.1, in a CI job with that git.
- A-23: `/plugin update` at an unchanged `-dev` version, against a later push to the branch the `git-subdir` test installed from.
- The live-mod spikes: `tool.check` seeing a spawned agent's `agentId`, `$.tool.call` on `ArtifactData` from a press, and `comments.sendToClaude` waking the session's watch.

## Spend logs seeded [DR-65]

- `docs/evidence/spend.jsonl`: Phase 1's live spend as `scenario: "phase1"` rows (Jev through the gateway $0.00517, Claude through the gateway $0.0219), plus the headless Claude Code runs (the A-1 spike about $0.012, these WS8 runs about $0.018), which aren't Jev, gateway or `sample` calls but are counted to keep the $3 cap conservative.
- `docs/art/spend.jsonl`: one row per committed image from `docs/art/README.md`'s estimates, and one `drafts` row for the rest of the day's $0.27.
