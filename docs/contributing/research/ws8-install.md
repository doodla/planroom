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

## The fixture's agent files [A-16] [V]

Headless haiku sessions in this repository, with and without `--settings` carrying the two entries now in `.claude/settings.json`:

- Without `claudeMdExcludes`, reading `examples/fixture/repos/worker/05-tests/tree/pyproject.toml` brought that directory's `CLAUDE.md` into context: the model quoted its first bullet exactly. With `claudeMdExcludes: ["**/examples/fixture/**/CLAUDE.md"]`, nothing arrived.
- A session at the repository root isn't offered the fixture's `run-worker-tests` skill. A session whose working directory is `examples/fixture/repos/worker/07-run-tests-skill/tree` is, and without a rule the Skill tool loaded it. With `permissions.deny: ["Skill(run-worker-tests)"]` the call was refused: "Skill execution blocked by permission rules".

So both guards hold, and the `examples/` area contract's rule stays as the second line.

## `/plugin update` at an unchanged version [A-23] [V]

A-23 fails. After later pushes to the branch the `git-subdir` install named, `claude plugin marketplace update planroom` then `claude plugin update planroom@planroom` answered "planroom is already at the latest version (0.1.0)", and `installed_plugins.json` kept `gitCommitSha` `071909da…`: the update compares versions, not commits. So the fallback applies: plain `gen` stamps a `-dev` version with a hash, so a new `main` commit installs as a new version. The catalogue hash the fallback names misses a change to a skill, an agent, the resolver or the mod alone. The skeleton's record should stamp the hash of `plugin/`'s own files instead (every file but `plugin.json`), and the resolver should compare the engine against the version without that suffix.

## The mod's `$.store` file [A-15] [V]

The spike mod set a marker with `$.store.set`, and a search found it in `~/.claude/plugins/store/planroom-spike_inline-d70c18432e39.json` (mode 0600, the store as one JSON object): the host keeps every plugin's store at `<config>/plugins/store/<plugin>_<source>-<hash>.json`. MOD-2's interim guard exempted everything under `plugins/`, so it missed exactly this file; the guard now denies that folder and its `planroom_*` files (DR-82).

## The mod's plugin root and its children's environment [A-14] [V, D]

`$.process.run(["sh", "-c", "env ..."])` from the spike mod listed the session's own variables (`CLAUDE_CODE_*`, `CLAUDE_CODE_REMOTE=true`), so children inherit the session's environment. `CLAUDE_PLUGIN_ROOT` isn't among them: the 2.1.310 types give the mod `$.plugin.root` for its own files instead [D]. So the mod finds the resolver at `${$.plugin.root}/scripts/planroom`, and A-14's fallback isn't needed.

`CLAUDE_CODE_VERSION` in that environment said `2.1.42` while `$.session.version()` answered `{ version: "2.1.310", base: "2.1.310", builtAt: "2026-10-23T17:43:32Z" }` [A-19, V]: the variable names something else, so the host check reads the API, never the environment.

## The mod's view of spawned agents and of `ArtifactData` [V]

The spike mod hooked `tool.check` for `Bash` and recorded each call's `agentId`:

- A Bash call from a subagent the Agent tool spawned carried that subagent's `agentId` (`af013627…`). So did the calls of this session's workflow agents (`a13d44ba…`), which pass through the same mod.
- The main loop's own Bash calls carried `agentId: null`.

So the guard can tell the implementer's calls from the session's own by `agentId`, as the design assumes.

From a tool handler, `$.tool.call({ tool: "ArtifactData", action: "list", url, collection: "inbox" })` reached the Phase 1 probe Artifact and returned its one inbox document. The structured `result.db_read.docs[].data` came back as `{}`, while the result's `text` carried each document's data, one JSON object per line, between `=== BEGIN ARTIFACT DB … ===` and `=== END ARTIFACT DB … ===` markers. So `/room sync` reads the rows from that text block, treating it as data, and checks every line against `InboxRow` before passing it to `import -`. The call ran from a tool handler, not a press; a press uses the same `$.tool.call`.

## The real mod, live [V]

`plugin/` was linked into this session's dev-mods folder, and this repository was scanned as a workspace (one repo, so the engine took its own working tree as the workspace root). The first two live loads failed, each on a seam the mocked harness couldn't show:

1. Every tool answered `engine_unavailable`, "CLAUDE_PLUGIN_ROOT is unset": the mod read its root from that variable, which the host doesn't set for a mod (A-14). It now reads `$.plugin.root`, and a harness test starts the mod with no such variable.
2. `session.start` threw: the host refused `$.command.register("room")` because the plugin's own `room` skill claims `/planroom:room`, and the refusal skipped the whole start. The skill is now `planroom`, and the plugin layout check refuses a skill named like a mod command (DR-84).

The third load ran. Through the mod's own tools, `room_query focus` returned the workspace's `PaneModel` (breadcrumb `planroom`, `scan: done`, next action "Open the containers"). `room_query map --level container` returned the container level's 9 blocks. So the whole skeleton ran live: the scan wrote the store, the mod read it through the resolver and the CLI, the canvas drew it (e2e and screenshots), and the Artifact published it (`research/ws7-remote.md`).

The same answers carry a dogfooding finding: the fixture's compose services under `examples/fixture/` appear as this workspace's containers (`ws:container:grafana`, `orders`, `redis` and the others), beside the repo's own `planroom` container. M1 adds default excludes for fixture and test-data trees.

## Still open in WS8
- A-21: the git adapter's tests on git 2.34.1, in a CI job with that git.
- `$.tool.call` on `ArtifactData` from a press (run here from a tool handler, below) and `comments.sendToClaude` waking the session's watch: both need a person's tap on a page or the pane, so they run with the WS7 publish.

## Spend logs seeded [DR-65]

- `docs/evidence/spend.jsonl`: Phase 1's live spend as `scenario: "phase1"` rows (Jev through the gateway $0.00517, Claude through the gateway $0.0219), plus the headless Claude Code runs (the A-1 spike about $0.012, these WS8 runs about $0.018), which aren't Jev, gateway or `sample` calls but are counted to keep the $3 cap conservative.
- `docs/art/spend.jsonl`: one row per committed image from `docs/art/README.md`'s estimates, and one `drafts` row for the rest of the day's $0.27.
