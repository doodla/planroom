# Research: packaging Planroom as a Claude Code plugin

Phase 1 research, topic `plugin-packaging`. Target build: **Claude Code 2.1.310** (`claude --version` -> `2.1.310 (Claude Code)`, binary `/opt/claude-code/bin/claude`).

## Evidence key

- **[verified: `cmd`]**: I ran it in this container and quote the result. Throwaway work lives under `/tmp/claude-0/-home-user-planroom/d26fcf29-d6d2-57f8-acca-1138f06b3754/scratchpad/plugin-packaging/` (written `$S` below).
- **[docs: url]**: read in the official docs. I fetched these pages on 2026-10-25 (HTTP 200) and grepped them: `plugins/components.md`, `plugins/host-marketplace.md`, `plugins/manifest-reference.md`, `plugins/loading.md`, `hooks.md`, `env-vars.md` under `https://code.claude.com/docs/en/`.
- **[guide]**: claimed by the `claude-code-guide` agent and not checked further.
- **[A]**: my assumption or recommendation.

Isolation: every install ran with `HOME=$S/home` and `CLAUDE_CONFIG_DIR=$S/home/.claude`. The headless sessions also used `env -i`, so this session's remote environment did not leak in. The user's real `~/.claude` was not touched. Three short headless sessions ran on `haiku`, and one of them spawned a `sonnet` subagent. Each produced a few hundred tokens.

## 0. Corrections to the brief and to the guide

| Claim | Source | What I found |
|---|---|---|
| "There's no `--marketplace` flag" for install | KICKOFF.md:303 | **Wrong for 2.1.310.** `claude plugin install --help` lists `--marketplace <source>`: "Install <plugin> (its bare name) from the marketplace at this source: owner/repo, a git or https URL, or a path. A marketplace not added yet is added first" [verified: `claude plugin install --help`]. The two-step `/plugin marketplace add` then `/plugin install` flow still works and is what S14 tests. |
| "A plugin with a `bin/` folder is refused" | KICKOFF.md:236, :304 | **Not by Claude Code itself.** With `bin/` present, `validate`, `validate --strict`, `validate --profile directory` and `install` all pass, and a session puts `bin/` on the Bash tool's PATH (section 3). The refusal is in **claude.ai and Cowork** (org sync): "claude.ai rejects a plugin that has one ... `Plugin contains a top-level bin/ directory`" [docs: plugins/host-marketplace.md line 97; plugins/components.md line 485]. The string is not in the CLI binary [verified: `grep -a 'top-level bin/' /opt/claude-code/bin/claude` -> no match], which fits a server-side check. **The constraint still holds for Planroom**: we want claude.ai and Cowork installs to work, so ship no `bin/`. |
| Plugin `name` is "at most 64" characters | KICKOFF.md:304 | Not enforced by validate: names of 64, 65 and 200 characters all pass with exit 0 [verified: name loop, section 4]. The only length cap I saw is "max 128 chars" for **marketplace** names on Claude Desktop (a validate warning). Keep `planroom` (8 characters) and it doesn't matter. |
| Names "can't contain `@`, `:`, path separators" | guide | validate gives a **warning, exit 0**, not an error. The warning text says Claude Code "refuses a plugin with this name when it is loaded by path (such as --plugin-dir), and cannot install it from a marketplace that lists it under this name" [verified]. So the name fails when used, not at validate. |
| `claudeai-` prefixed marketplace names are reserved | guide | validate accepted `claudeai-x` and `claude-plugins-official` as marketplace names [verified]. `npm`, `github` and `inline` were refused (section 4). The rest is docs-only. |
| "A local-path marketplace is referenced live, not copied" | guide | **Half right.** `install` *does* copy the tree, `bin/` included, into `~/.claude/plugins/cache/<mkt>/<plugin>/<version>/` [verified: `find`]. But `claude plugin list --json` reports `readFromFolder: <source dir>`, and in a session `CLAUDE_PLUGIN_ROOT` is the **source** directory. A hook script I edited after install ran in its edited form [verified: headless session, section 3]. Docs: "loads in place ... edits ... take effect at the next session start or `/reload-plugins`" [docs: plugins/loading.md line 211]. |

## 1. CLI surface (2.1.310)

[verified: `claude plugin --help` and `--help` on each subcommand]

- `claude plugin` subcommands: `configure`, `details <name>`, `disable`, `enable`, `eval [target]`, `init|new <name>`, `install|i <plugin>`, `list`, `marketplace {add,list,remove|rm,update}`, `prune|autoremove`, `tag [path]`, `test [dir]`, `uninstall|remove`, `update`, `validate <path>`.
- `validate [--json] [--strict] [--profile directory] <path>`.
  - `--strict`: warnings become exit 1.
  - `--profile directory`: also exit 1 on anything the public plugin directory would refuse (README missing, licence missing, and so on).
- `test [dir]`: "Runs every *.test.ts and *.test.tsx under dir ... imports its kit from 'claude-code/testing'. Exits 1 when a test fails."
  - On a plugin with no mod it exits 1: `no hooks module to load; hooks/hooks.json names none in "modules"` [verified: `claude plugin test $S/mkt/plugins/planroom`; exit=1]. **Run it on the mod's plugin dir only.**
- `install`:
  - `-s/--scope user|project|local` (default `user`), `--json`, `--marketplace <source>`, `--config key=value` (userConfig), `-y`, `--accept-command <sha256>`, `--registry`.
- `uninstall`: `--keep-data`, `--prune`, `-s`, `--json`.
- `marketplace add <source>`: `--scope user|project|local`, `--sparse <paths...>` (git sparse checkout), `--json`, `--claudeai`.
  - `file://` git URLs are rejected: "Invalid marketplace source format. Try: owner/repo, https://..., or ./path" [verified].
- `tag [path]`: "Create a {name}--v{version} git tag ..., validating that plugin.json and any enclosing marketplace entry agree". Options: `--dry-run`, `--push`, `--remote`, `-m`, `-f`.
- `details <name>`: works only on an installed or `--plugin-dir` plugin, not on a bare path [verified]. Output for the throwaway plugin:
  ```
  Skills (1) scan · Agents (1) curator · Hooks (1) SessionStart (harness-only — no model context cost)
  MCP servers (1) engine · Always-on: ~67 tok added to every session
  ```
- `init|new <name>`: scaffolds into `~/.claude/skills/<name>/` (`<name>@skills-dir`). Options include `--with skills agents hooks mcp lsp output-style channel`.
- `claude --plugin-dir <path>` loads a plugin from a directory **or a `.zip`** for one session [verified: `claude --help`].

## 2. Throwaway plugin and what validate checks

Layout built at `$S/mkt` [verified: files written, then validated]:

```
mkt/.claude-plugin/marketplace.json   {"name":"planroom","owner":{...},"plugins":[{"name":"planroom","source":"./plugins/planroom",...}]}
mkt/plugins/planroom/
  .claude-plugin/plugin.json          name, version 0.0.1, description, author, license, homepage, repository, keywords
  skills/scan/SKILL.md                frontmatter name + description
  agents/curator.md                   name, description, model: sonnet, effort: medium, tools: Read, Grep, Glob
  hooks/hooks.json                    {"hooks":{"SessionStart":[{"hooks":[{"type":"command","command":"\"${CLAUDE_PLUGIN_ROOT}/scripts/bootstrap.sh\""}]}]}}
  .mcp.json                           {"mcpServers":{"engine":{"command":"${CLAUDE_PLUGIN_ROOT}/scripts/planroom","args":["mcp"],"env":{"PLANROOM_HOME":"${CLAUDE_PLUGIN_DATA}"}}}}
  scripts/bootstrap.sh, scripts/planroom   (POSIX sh, chmod +x)
```

Results:

- **Plugin dir:** `claude plugin validate $P` -> `√ Validation passed`, exit 0. It also prints "plugin directory checks" that don't affect the result: `README_MISSING` (error class, blocking for the directory: "at least 40 words"; "A README at the repository root also counts for a plugin in a sub-folder") and `ICON_MISSING` (warning: square PNG 512–2048 px, under 2 MB, at `.claude-plugin/icon.png`; SVG and WebP not accepted) [verified]. `--strict` -> exit 0. `--profile directory` -> exit 1 [verified].
- **Marketplace dir:** `claude plugin validate $M` -> passed with one warning (no marketplace `description`). `--strict` -> exit 1 [verified].

**What validate does not check** [verified, `$S/badcomp`, `$S/mnames`, `$S/repo`]:

- **It doesn't descend from a marketplace into its plugins.**
  - On a repo-root marketplace with `source: "./plugin"`, `contents` is `[]`.
  - A `source` naming a directory that doesn't exist passes.
  - An entry name (`planroom-x`) that differs from the `plugin.json` name (`planroom`) passes.
  - The exception: when `marketplace.json` and `plugin.json` share one `.claude-plugin/` (source `./`), both are checked. The output then says "these checks cover only the plugin in this folder. To check another plugin that the marketplace lists, run validate on that plugin's folder."
  - **So CI must run validate on the marketplace root and on `plugin/`, and add its own name-equality check.**
- **It doesn't check agent `model` or `effort` values.**
  - `model: notamodel`, `effort: ludicrous`, `permissionMode: bypassPermissions`, `hooks: {}` and `mcpServers: {}` in an agent all passed with no warning.
  - Only a missing `description` warns.
  - The docs say `permissionMode`, `hooks` and `mcpServers` are ignored in plugin agents [guide].
- **It doesn't follow symlinks.** It warns: "1 entry here is a symlink and was not read ... A session loading this plugin does follow them, so validate the real paths separately."

**What validate does catch** [verified, `$S/badcomp`]:

- An unknown `plugin.json` key: warning "Unknown field 'bogusKey'. Claude Code ignores it at load time."
- A `CLAUDE.md` at the plugin root: warning "not loaded as project context ... use a skill".
- A SKILL.md with no frontmatter: warning.
- An unknown hook event: warning "entry ignored at runtime".
- An `.mcp.json` server with no `command`: **error**, "The plugin loader silently drops this server at load." The overall `success` was then false.

Also accepted with no warning [verified]:

- Exec-form hooks (`"command": ..., "args": [...], "timeout": 30`) in `hooks/hooks.json`.
- Inline `hooks` (with `"async": true`) and `mcpServers` inside `plugin.json`.

## 3. Install and runtime behaviour

Steps run with the isolated HOME [verified]:

1. `claude plugin marketplace add $S/mkt` -> "Successfully added marketplace: planroom (declared in user settings)". It wrote `extraKnownMarketplaces.planroom.source = {"source":"directory","path":...}` and `enabledPlugins["planroom@planroom"]=true` to `settings.json`, and an entry to `plugins/known_marketplaces.json`. `marketplace list` also shows a built-in `anthropic-plugin-directory`.
2. `claude plugin install planroom@planroom --json` -> `{"outcome":"ok","pluginId":"planroom@planroom","scope":"user"}`. This was **with `bin/` present**.
3. `claude plugin list --json` shows:
   - `installPath` in the cache;
   - `readFromFolder` and `folderVersion: "0.0.1"`;
   - the MCP servers *unsubstituted* (`${CLAUDE_PLUGIN_ROOT}/...`).

Headless session (`env -i HOME=... CLAUDE_CONFIG_DIR=... claude -p ... --output-format stream-json --verbose`), from the `init` event [verified]:

- `agents` has `planroom:curator`.
- `skills` and `slash_commands` have `planroom:scan`.
- `mcp_servers` has `plugin:planroom:engine`. Its status was `failed` because my stub exits 1, as intended.
- The plugin `path` is the source folder.

SessionStart hook environment, dumped by `bootstrap.sh` [verified: `$S/out/hook-env.txt`]:

```
ROOT=$S/mkt/plugins/planroom                      # source dir, not cache
DATA=$S/home/.claude/plugins/data/planroom-planroom   # id "planroom@planroom" -> "planroom-planroom"; dir existed
PATH=/opt/node22/bin:/usr/local/bin:/usr/bin:/bin     # bin/ NOT on hook PATH
```

MCP server environment, dumped by the stub `scripts/planroom` [verified: `$S/out/mcp-env.txt`]:

- `args=mcp`.
- `CLAUDE_PLUGIN_ROOT` and `CLAUDE_PLUGIN_DATA` are exported.
- `PLANROOM_HOME` was substituted to the data dir.

Bash tool environment [verified: tool result in `$S/out/stream.jsonl`]:

```
P=/opt/node22/bin:/usr/local/bin:/usr/bin:/bin:$S/mkt/plugins/planroom/bin   # bin/ appended AFTER user PATH
command -v planroom -> $S/mkt/plugins/planroom/bin/planroom
R=unset   D=unset                                     # CLAUDE_PLUGIN_ROOT/DATA not in Bash env
```

Other session results:

- **Skill bodies:** `${CLAUDE_PLUGIN_ROOT}` and `${CLAUDE_PLUGIN_DATA}` in the SKILL.md body are substituted. `/planroom:scan` made the model echo `$S/mkt/plugins/planroom/scripts/planroom` and `$S/home/.claude/plugins/data/planroom-planroom` [verified: `claude -p '/planroom:scan'`].
- **Agent `model:` is honoured.** The main session ran on `--model haiku`. Spawning `planroom:curator` (`model: sonnet`) produced assistant messages from `claude-sonnet-5-5` under the Agent tool's `parent_tool_use_id`. `modelUsage` listed both models [verified: `$S/out/agent.jsonl`]. `effort:` wasn't observable this way; the docs list it as supported [guide].
- **Plugin agents ran in the background.** The Agent call returned "It runs in the background", and the reply arrived on a later notification [verified]. I don't know whether that comes from the plugin agent or from the default `CLAUDE_AUTO_BACKGROUND_TASKS` behaviour [A].

Uninstall [verified]:

- `uninstall planroom@planroom` -> `keptData:false`. `plugins/data/` was then empty, and `installed_plugins.json` showed `"plugins": {}`. The cache dir `0.0.1` remained; the docs say orphaned versions go after 14 days [guide].
- `uninstall --keep-data` -> `keptData:true`, and the marker file survived.

From the docs:

- `${CLAUDE_PLUGIN_DATA}` "survives updates, for node_modules, virtual environments, and caches ... created when first referenced" [docs: plugins/components.md line 646].
- The recommended pattern is a `SessionStart` command hook that installs into `${CLAUDE_PLUGIN_DATA}` (an npm example with a `diff -q` guard) [docs: plugins/components.md lines 653–674].
- In interactive sessions, **SessionStart hooks run in the background**: "You can type right away ... Claude's first response still waits for the hooks to finish" [docs: hooks.md line 1248].
- The default command-hook `timeout` is 600 s [docs: hooks.md line 414].
- MCP server startup times out at `MCP_TIMEOUT` (default 30000 ms) [docs: env-vars.md].
- The `Setup` hook event fires only on `--init-only`, `-p --init` or `-p --maintenance`, never on normal startup [docs: hooks.md line 1392], so it can't do first-use fetching.
- Automatic Node dependency install happens only for cached (non-local) plugins with `package.json` plus a lockfile, has a 60 s timeout, and does not run for local in-place plugins [docs: plugins/loading.md lines 245–278].

## 4. Name rules

**Plugin names** in `plugin.json` [verified: `claude plugin validate --json` on 23 names under `$S/names`]:

| Result | Names |
|---|---|
| ok, exit 0 | `planroom`, `1planroom`, 64/65/200 × `a` |
| **error**, exit 1 | `""` ("cannot be empty"), `plan room` ("cannot contain spaces"), and the reserved names `claude-planroom`, `claude`, `anthropic-x`, `cc-plugin-x`, `claude-code`, `official-claude-tools` |
| warning: "reads as one of Anthropic's own" | `planroom-claude`, `my-claude-tool`, `planroom-anthropic` |
| warning: "Claude Code accepts it, but the Claude.ai marketplace sync requires kebab-case" | `Planroom`, `plan_room`, `plan.room` |
| warning: Claude Code refuses or cannot install it | `plan@room`, `plan:room`, `plan/room` ("refuses ... when loaded by path ... cannot install it from a marketplace"); `-planroom` ("cannot install it from a marketplace that lists it under this name") |

The reserved-name error text, verbatim: "A third party's plugin name cannot start with "claude-", "anthropic-", "anthropics-", or "cc-plugin-", be "claude", "anthropic", "anthropics", "claude-code", or "claude-mods", or put "official" beside "claude" or "anthropic"."

**Marketplace names** [verified, `$S/mnames`]:

- ok: `planroom`, `Planroom`, `plan_room`, `plan.room`, `claudeai-x`, `claude-plugins-official`, `anthropic-tools`, `doodla-planroom`.
- error:
  - `npm`: "reserved for plugins installed from an npm registry";
  - `github`: "reserved ... `<owner>/<repo>@github` ... not yet available";
  - `inline`: "reserved for --plugin-dir session plugins";
  - `plan room`: spaces. The error also states the id rule: "Each part of a plugin id (plugin@marketplace) may use only the letters a-z and A-Z, digits, ".", "_" and "-", and must start with a letter or digit". A Desktop warning gives "max 128 chars".

**Source paths** [verified]:

- `./plugins/../plugins/planroom` -> error "Path contains ".."".
- `plugins/planroom` -> error "must start with "./"".
- `metadata.pluginRoot: "./plugins"` with bare `source: "planroom"` -> ok.
- An entry `version` that differs from `plugin.json` -> warning "At install time, plugin.json wins".

## 5. Things I did not verify

- **Real git and GitHub marketplace installs.** `file://` was refused and I didn't push a repo. From the docs: plugins from non-local marketplaces are copied into `cache/<mkt>/<plugin>/<version>/`, and `CLAUDE_PLUGIN_ROOT` then points at the cache. The version resolves from `plugin.json` `version`, then the entry `version`, then the commit SHA. Auto-update is off for third-party marketplaces [guide; docs: plugins/loading.md].
- **The claude.ai and Cowork `bin/` refusal itself** [docs only].
- **Whether one `hooks/hooks.json` can hold both `"modules"` (the mod) and classic `"hooks"`.** Also open in `mod-api.md:71`. I wrote no mod files, per the task.
- **Agent `effort:` taking effect.** Also whether plugin agents can set `isolation: "worktree"` [guide].
- **`.mcpb` bundles, `userConfig`, `dependencies`, `channels`** [guide].

## Design implications for Planroom

### Layout [A, grounded in sections 2–4]

```
.claude-plugin/marketplace.json     {"name":"planroom","description":"…","owner":{…},
                                     "plugins":[{"name":"planroom","source":"./plugin","description":"…"}]}
plugin/
  .claude-plugin/plugin.json        name "planroom", version = workspace version, description, author,
                                    license, homepage, repository, keywords, icon
  .claude-plugin/icon.png           square PNG 512–2048 px, < 2 MB (Iris art)
  README.md                         ≥ 40 words (or rely on repo-root README)
  skills/<verb>/SKILL.md            one skill per user intent (scan, explain, tweak, …)
  agents/implementer.md, agents/curator.md
  hooks/hooks.json                  {"modules": ["./planroom-mod.js"]}  (built mod; see mod-api.md)
  hooks/planroom-mod.js             built output, committed or produced by the release build
  .mcp.json                         only if the engine exposes MCP tools (see below)
  scripts/planroom                  POSIX sh engine resolver (NOT in bin/)
  scripts/engine.env                generated: PLANROOM_ENGINE_VERSION, repo slug
```

- **Marketplace and plugin names.** Both are `planroom`, so the install id is `planroom@planroom` and the data dir is `~/.claude/plugins/data/planroom-planroom/` [verified pattern]. That name is reserved by nothing, gets no warning, and is kebab-case.
- **No top-level `bin/`.** Keep executables in `scripts/` [docs: host-marketplace.md line 97].
- **Putting the plugin in `plugin/`, not the repo root, keeps Cargo and pnpm files out of the plugin.** A local-marketplace install copies the whole plugin dir into the cache [verified]. A git install clones the repo; `--sparse .claude-plugin plugin` limits that [verified: help].
- **CI job "plugin"**:
  1. `claude plugin validate --strict .` and `claude plugin validate --strict plugin` (each catches different things; section 2).
  2. A script asserting that the entry name equals the `plugin.json` name and that the `plugin.json` version equals the `Cargo.toml` and `package.json` versions. `claude plugin tag --dry-run plugin` also checks plugin and entry agreement.
  3. `claude plugin test packages/mod` (or the built mod dir).
  4. A check that `plugin/bin` does not exist and that `plugin/` holds no symlinks (validate skips them).
  5. Optionally `--profile directory` once the README and icon exist.
  6. Pin the CI Claude Code to `2.1.310`, the version these facts were checked against [A].

### Engine binary on first use [A]

The resolver is `plugin/scripts/planroom`, a POSIX sh script in the style of Iris `install.sh` (see `iris-release-bar.md` §3). Every entry point calls it:

- the MCP `command` (`${CLAUDE_PLUGIN_ROOT}/scripts/planroom`, `args: ["mcp"]`);
- skills, written as `"${CLAUDE_PLUGIN_ROOT}/scripts/planroom" …` in the body (substitution verified);
- the mod, through `$.process.spawn`.

It resolves in this order:

1. **Override:** `$PLANROOM_ENGINE` if it is set and executable.
2. **Dev checkout:** when the plugin is loaded in place from a local marketplace, `CLAUDE_PLUGIN_ROOT` is the repo's `plugin/` [verified]. So use `"$CLAUDE_PLUGIN_ROOT/../target/release/planroom"` (or `debug`) if it exists and `--version` matches. This is the walking-skeleton "built from source" path, and edits apply with no version bump [verified].
3. **Cached release:** `$DATA/engine/$VERSION/planroom` if a `.ok` stamp with its sha256 is present. Take `DATA` from `$CLAUDE_PLUGIN_DATA`, which is set in hooks and MCP processes [verified]. If it is unset (the Bash tool [verified]), fall back to `$PLANROOM_DATA`, which skills pass as `PLANROOM_DATA="${CLAUDE_PLUGIN_DATA}"`, or to `${XDG_DATA_HOME:-$HOME/.local/share}/planroom`.
4. **Fetch:**
   - Map `uname -s`/`uname -m` to one of four targets: `x86_64`/`aarch64` × `unknown-linux-musl`/`apple-darwin`. musl gives a static Linux binary [A].
   - Download `planroom-$VERSION-$TARGET.tar.gz` and `SHA256SUMS` from `https://github.com/<owner>/planroom/releases/download/v$VERSION/` with `curl -fsSL` or `wget`.
   - Verify with `sha256sum` or `shasum -a 256`, extract into a temp dir inside `$DATA`, check the layout, then `mv` atomically.
   - Hold a `mkdir` lock so a SessionStart prefetch and the MCP start don't race.
   - Write all progress to stderr. On an MCP start, stdout belongs to the protocol.
5. **Fallback:** print the exact `cargo install --locked --git https://github.com/<owner>/planroom --tag v$VERSION planroom-cli` (or local `cargo build --release`) and exit non-zero with a clear message.

`$VERSION` is fixed at release time from `scripts/engine.env`, never "latest", so the plugin version and the engine version move together. Plugin data survives updates [docs], so old engine versions in `$DATA/engine/` need pruning by the resolver.

**Timing.** MCP startup has a 30 s timeout [docs], and an interactive SessionStart hook runs in the background but holds Claude's first reply [docs].

- Add a SessionStart command hook, `"${CLAUDE_PLUGIN_ROOT}/scripts/planroom" prefetch` with `async: true`, that only downloads. It must never print to stdout and must exit 0 when offline.
- The MCP server path resolves lazily. If the binary is still missing after the lock wait, fail fast with an actionable error rather than hang past `MCP_TIMEOUT`.
- If the mod already carries all the tools (`$.tool.register` calling the local HTTP server), skip `.mcp.json` entirely. That leaves a single tool-wiring path; choose this in the architecture.

**Hooks next to the mod.** Mixing `modules` and classic `hooks` in one `hooks/hooks.json` is unverified. Declare the prefetch hook inline in `plugin.json` `"hooks"` instead: it validates [verified] and merges with the default file [guide]. Alternatively, have the mod trigger the prefetch itself.

### Skills, agents and the mod

- **Skills:** `plugin/skills/<name>/SKILL.md` with `name` and `description` in frontmatter. A pushy description goes in `description`, and `description` plus `when_to_use` is cut at 1,536 characters [guide]. They appear as `/planroom:<name>` [verified]. Each costs always-on tokens; `claude plugin details` reports about 40 tokens for a short skill [verified]. Snapshot that number in CI to keep the surface small (S15).
- **Agents:** `plugin/agents/implementer.md` and `plugin/agents/curator.md`. They are listed as `planroom:implementer` and `planroom:curator` [verified pattern].
  - Set `model:` (honoured, verified) and `effort:`.
  - Set `tools:` to the minimal list.
  - Don't rely on `permissionMode`, `hooks` or `mcpServers` in plugin agents: they're ignored [guide], and validate is silent about them [verified].
  - The repo's own CLAUDE.md requires model and effort on every spawn. Plugin agent frontmatter satisfies that for spawns that name the agent type [A].
- **Mod:** `plugin/hooks/hooks.json` = `{"modules": ["./<built>.js"]}`, built from `packages/mod` (see `mod-api.md` §1 for layout and build and §4–5 for tools and agents). `claude plugin test` needs that `modules` key, or it exits 1 [verified]. Agents the mod registers with `$.agent.register` are an alternative to `agents/*.md`. Prefer the files: validate sees them, and `details` counts them.

## Top risks

1. **bin/ and claude.ai or Cowork distribution.** Claude Code accepts `bin/`, so nothing local catches a regression. Only the server-side sync refuses it. Mitigation: the CI check `test ! -e plugin/bin`.
2. **First-use fetch inside MCP or SessionStart timing.** A slow download hits the 30 s `MCP_TIMEOUT`, or races the async prefetch. On an offline first run, the fallback is a cargo build that takes minutes. Mitigation: lock, prefetch, fail fast with clear stderr, and keep the dev-checkout path.
3. **Validate's blind spots give false confidence.** It doesn't descend into plugins, check entry and plugin name equality, check agent model or effort values, follow symlinks, or settle whether `modules` and classic hooks can mix. Mitigation: run validate twice, add custom checks, and run a headless smoke session with an isolated `HOME` and `CLAUDE_CONFIG_DIR` that asserts the `init` event lists `planroom:*` skills, agents and MCP (the method used here).
