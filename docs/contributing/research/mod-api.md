# Research: the Claude Code mod API (for the Planroom mod)

Phase 1 research, topic `mod-api`. Read-only: no mod file was written anywhere. Target build: **Claude Code 2.1.310**.

## Sources and evidence key

- **[T:n]**: the build's own API declaration, `claude-code.d.ts` line *n*. The `plugin-authoring` skill wrote it to `/tmp/claude-0/bundled-skills/2.1.310/<hash>/plugin-authoring/types/claude-code.d.ts`, 22,649 lines, header `// Written by Claude Code 2.1.310.` That path is per process, so load the skill again to get the file back. This file is the authority. The skill's own note says "The API is early access and moves between releases: the declaration file is the authority".
- **[R:n]**: the skill's `reference.md` line *n* (268 lines, same folder).
- **[S]**: the skill's `SKILL.md` text. **[E:file]**: the skill's `examples/` (`pane.tsx`, `pane-state.d.ts`, `band.tsx`, `band-state.d.ts`, `tool-call.ts`, `pane.test.tsx`).
- **[V]**: I ran it in this container. Commands and their output are quoted.
- **[A]**: an assumption or inference, not verified. **[D]**: recalled from public docs, not checked here.

What I ran [V]:

```
$ claude --version            -> 2.1.310 (Claude Code)
$ claude plugin --help        -> commands: configure, details, disable, enable, eval, init|new,
                                 install, list, marketplace, prune, tag, test [dir], uninstall,
                                 update, validate [options] <path>
$ claude plugin validate --help -> options --json, --profile directory, --strict
$ claude plugin test --help   -> "Runs every *.test.ts and *.test.tsx under dir, each file in a
                                 child of this binary, in an environment like the one the mod's
                                 hooks run in. A test file imports its kit from
                                 'claude-code/testing'. Exits 1 when a test fails."
$ claude plugin validate --json /home/user/planroom
  -> {"success": false, ... "message": "No manifest found in directory. Expected
      .claude-plugin/marketplace.json or .claude-plugin/plugin.json", ... "gatingHooks": []}
```

I did not exercise any runtime behaviour. Writing a mod would raise the consent question, so nothing below was loaded or run in a session. Every runtime claim is [T]/[R]/[S] (documented by the build itself) or [A].

---

## 1. Layout, build, load, and shipping inside a plugin

**Files** [S][R:7-24]:

```
<plugin>/
  .claude-plugin/plugin.json   { "name", "version", "description", "types"?: "./types/index.d.ts",
                                 "userConfig"?: {...}, "dependencies"?: [...] }
  hooks/hooks.json             { "modules": ["./register.tsx"] }   // one path, relative to hooks.json
  hooks/register.tsx           export const register: Register = (on, options) => { ... }
  types/index.d.ts             // the $.state contract: declare module 'claude-code' { interface PluginState { ... } }
  *.test.ts(x)                 // run by `claude plugin test`
```

- `name` is lowercase letters, digits and hyphens, at most 64. **A plugin with a `bin/` folder is refused** [R:139].
- Module rules [T:20-27][R:14]. Files are `.ts .tsx .jsx .js .mjs .cjs .mts .cts`, and every one is an ES module, so there is **no `require`**. Plugin files are imported with static `import` declarations. **A module containing `import()` does not load.** There is no DOM and no Node. Globals are `h`/`Fragment` (JSX factory), plus `URL`, `TextEncoder`, `AbortController`, `crypto.subtle` and similar.
- JSX compiles against `h`. The tsconfig from the header [T:66-76] is `"jsx": "react", "jsxFactory": "h", "jsxFragmentFactory": "Fragment"`, `lib: ["es2023"]`, `types: []`, `moduleResolution: "bundler"`, `strict`, `noUncheckedIndexedAccess`.
- **There is no build step that the engine requires.** The engine loads `.ts/.tsx` source directly [S]. `import type ... from 'claude-code'` is empty at run time [T:15].
  - [A] Bare npm specifiers are not resolvable ("A file of the plugin is imported"). Third-party code therefore has to be bundled into plugin files.
- Types for editors. Once loaded from a watched folder, the engine lays `.claude-plugin/types/{claude-code,claude-code-tools,claude-code-mcp}/index.d.ts` and a `tsconfig.json` for the mod to extend, so `tsc -p <mod>` works [R:44-53].
  - An installed plugin's folder is never written to. CI must type-check with the header's tsconfig, kept outside the mod folder [R:50-53].
- `$.state` keys must be literals and must be declared in the `types` contract. `claude plugin validate` holds the module to it [S][T:3480-3489].
- `$.env.get/set` names must also be string literals that validate can read [T:3699-3721].

**Loading paths** [R:72-76]:
- During development: `claude --plugin-dir <folder>`, or `CLAUDE_CODE_PLUGIN_DIRS`. In an interactive session these are watched, and a save hot-reloads the module.
  - Saves made by the model during a turn reload once, when the turn ends.
  - A reload re-runs `register` and `session.start`. `$.state` and `$.store` survive it. Module variables, timers and spawned children do not.
- Installed: `/plugin marketplace add <source>`, then `/plugin install planroom@<marketplace>`.
  - For a **folder marketplace with a relative `source`**, the plugin is read from that folder itself. Editing it and running `/reload-plugins` picks up the change with no reinstall [R:76]. This is the right loop for working on the Planroom repo.
  - KICKOFF.md says there is no `--marketplace` flag, but reference.md shows `/plugin install <mod> --marketplace <owner>/<repo>` [R:89]. **Conflict: verify on 2.1.310.**
- The session's own mods folder (`~/.claude/dev-mods/<session>/`) is only for mods written in-session. The first write there raises the consent question [S]. Planroom should not use it. Its walking skeleton should go through `--plugin-dir` or the folder marketplace instead.

**Shipping inside the Planroom plugin** [A, design]:
- `packages/mod` (TypeScript source, tests run with a normal toolchain) gets bundled with esbuild into `plugin/hooks/register.js`. The esbuild settings are `--format=esm --bundle --splitting=false`, with no dynamic imports and an external of `claude-code`.
- Any `Client` surface module must be emitted as its **own file**, because `Client.module` is a string-literal path relative to the hooks file, read off the source at load [T:1535-1550].
- `plugin/types/index.d.ts` is the `$.state` contract.
- Unresolved: whether one `hooks/hooks.json` can carry both `"modules"` and classic command `"hooks"`. The docs show only `modules`. Spike it or ask `claude-code-guide`.

## 2. UI

### Sites

| Site | How | Where it draws | Notes |
|---|---|---|---|
| Pane | `$.ui.open(pane: PaneOpenArgs) => Promise<UiOpenResult>` [T:2536], drawn by `on('ui.render', { component: 'Pane', requestId: id }, hook)` | every surface [T:10877] | `id` is 1-64 of `[A-Za-z0-9_-]`, one instance per id. Options are `title`, `focus`, `closeOnEscape`, `holdToasts`, `rows`, `columns` [T:7589-7660]. It resolves `{ isPlaced: true }` or `{ isPlaced: false, reason }` [T:15082] |
| AbovePrompt (band) | `on('ui.render', { component: 'AbovePrompt' }, hook)` returning a tree, or `next(e)` to show nothing [S][E:band.tsx] | **terminal and desktop only** [T:10820] | props are `hasSurvey`, `isWorking`, `maxRows`, `bodyColumns`, `scroll`, `view` |
| status | `$.ui.status(text: string \| undefined): void` [T:2513] | under the prompt | one per plugin; `undefined` clears it; 2,000 chars drawn on the terminal, 10,000 remotely |
| toast | `$.ui.toast(text: string, options?: ToastOptions): void` [T:2483] | stack at the top right | `timeoutMs` defaults to 4000 [T:13496]; held while a `holdToasts` pane shows |
| log line | `$.ui.log(text, { to?: 'transcript' \| 'debug' })` [T:2450] | transcript, dim, never sent to the model | |
| native notification | `$.ui.notify(text, { title })` [T:2501] | OS | |
| dialog question | `$.ui.ask(question, options: 2-4 labels \| AskOptions): Promise<string>` [T:2466] | AskUserQuestion dialog | rejects in `-p` |
| prompt suggestion | `$.prompt.suggest({ text })` [T:3072] | dim suggestion in the prompt box, taken with Tab | `{ isShown: false }` while the box holds text or a turn runs |
| prompt fill | `$.prompt.fill({ text, mode, cursor? })` [T:3060] | prompt box draft | |
| inline command output | `on('ui.render', { component: 'CommandOutput', props: { command: 'room' } })` [R:169] | transcript | draws a `/room` answer as a rich tree |

Other ways in: `$.ui.close({ id })`, `$.ui.panes()`, `$.ui.focus({ requestId, key })`, `$.ui.scroll({ in, to, block })`, `$.ui.copy`, `$.ui.view({ agentId })` (switches to an agent's transcript, only on the person's own input) [T:2550-2639].

### Pane geometry and placement [T:2520-2536, 10877-10930, 7640-7660]

- `placement` is `'dock'` (beside the transcript, in the terminal's fullscreen layout from **110 columns**) or `'inline'` (above the prompt).
- An unasked open is placed only from **144 columns**, or 110 for an id the person opened before. An open the person asked for is placed at any width. Below the floor it waits undrawn with `isPlaced: false`.
- `columns` (dock width wanted) is capped at **45% of terminal columns, minimum 24**. `rows` (inline height wanted) defaults to a third. The person's own resize wins over both.
- `e.props.bodyColumns` gives the drawable width. `e.props.scroll.bodyRows` gives the rows of the window. The engine owns scrolling.
  - Size trees to `bodyColumns`, not `viewport.columns` [R:153].
- Several panes become tabs. The title is drawn only when more than one pane is open.
- Keys: **ctrl+x tab** focuses the band or pane, **Esc** returns to the prompt, **ctrl+x x** closes the pane, **ctrl+x ctrl+a** collapses the band [T:10820-10877, R:169].

### Element set (`const { Box, Text, ... } = $.ui.resolve(e)` [T:2433])

The elements are not globals. They come from the surface's table `Elements[e.surface]` [T:3919-4000]:

| | Box | Text | Button | Link | Code | Markdown | Input | Select | Client | Svg | Raster | Image |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| terminal | y | y | y | y | y | y | y | y | y | **no** | y | y |
| desktop | y | y | y | y | y | y | y | y | y | y | - | - |
| vscode | y | y | y | y | y | y | y | y | - | y | - | - |
| mobile | y | y | y | y | y | y | **no** | **no** | - | y | - | - |

Narrowing `e.surface` narrows the table. An element that the surface lacks fails the hook, and the hook is skipped [R:163-166].

- **Box** (`BoxProps` [T:928]). Ink flex layout:
  - layout: `flexDirection`, `flexGrow`, `flexShrink`, `flexWrap`, `alignItems`, `alignSelf`, `justifyContent`, `gap`/`columnGap`/`rowGap`
  - size: `width`, `height`, `minWidth`, `minHeight` (numbers or `%` strings)
  - spacing: `margin*`, `padding*` (`Space = number | 'xs'|'sm'|'md'|'lg'|'xl'`)
  - decoration: `borderStyle` (`single double round bold singleDouble doubleSingle classic arrow dashed quote`), `borderColor`, `backgroundColor`
  - other: `overflow`, `display: 'flex'|'none'`, `position: 'relative'|'absolute'` with `top/left/right/bottom`
  - `key` makes the Box a hover scope, and `hover: BoxHoverProps` sets the hover style. Hover-reveal cards are drawn with `display: 'none'` plus `hover: { display: 'flex' }`.
- **Text** (`TextProps` [T:13380]): `color` (a `ThemeKey` or a raw color), `backgroundColor`, `dimColor`, `size` (`caption…title`, ignored on the terminal), `weight`, `mono`, `tabular`, `italic`, `underline`, `strikethrough`, `inverse`, `dir`, `wrap` (`wrap|end|middle|truncate|truncate-start|truncate-middle|truncate-end`), `hover`.
- **Button** (`ButtonProps` [T:1093]): `key`, `label` (or children made of strings and `Text` only), `hotkey` (**one digit or one lowercase letter**), `action` (an engine keybinding action), `plain`, `dimColor`, `variant: 'primary'|'secondary'`, `role: 'dismiss'`, `autoFocus`, `hover`, `onPress(e: UiPressArgument)`.
  - **Give every Button in a list its own `key`** [R:167].
  - A list row is one plain Button: `<Button key={id} plain onPress={open}>{name} <Text dimColor>2m</Text></Button>`.
- **Markdown** (`MarkdownProps` [T:5879]): `text`, `dimColor`, `key` plus `onLinkPress(link, e)`, `pressableLinks`. It is drawn like an assistant reply (headings, lists, tables, fences).
- **Code** (`CodeProps` [T:1662]): `source`, `language`/`path`, `startLine`, `format: 'source'|'diff'` (unified-diff hunks with gutters and colours, which suits pending diffs), `wrap: 'wrap'|'truncate-end'`.
- **Input** (`InputProps` [T:5627]): `key` (required), `label`, `placeholder`, `value`, `submitLabel`, `rows` (multi-line), `autoFocus`, `onInput`, `onSubmit` (required).
- **Select** (`SelectProps` [T:11059]): `key`, `label`, `options: {value,label?}[]`, `value`, `autoFocus`, `onSelect`.
- **Link** (`LinkProps` [T:5823]): `href` (at most 2,048 chars), `label`. A remote surface draws only `https:` links as links.
- **Svg** (`SvgProps` [T:13089]): `source` (at most 131,072 chars), `alt` (**required**), `width`/`height` in CSS pixels, `isInteractive`.
  - `isInteractive` puts the SVG in a script-less sandboxed frame, so CSS `:hover`, SMIL and `<title>` tooltips work. Scripts and event handlers are never enabled.
- **Raster** (`RasterProps` [T:9722]), terminal only: `key`, `columns` (1-512), `rows` (1-256), `cells` (base64 of little-endian u32 triplets `[codePoint, fg, bg]`, each code point a printable width-1 BMP character, so braille and block characters work).
  - `$.ui.blit({ requestId, key, cells })` repaints it without a render pass, up to 120 taken and about 60 shown per second [T:2397-2417].
- **Image** (`ImageProps` [T:5471]), terminal only: PNG/RGBA bytes (at most 2 MiB) or a file or shared-memory name, with `columns`/`rows` up to 255 and a required `alt`.
  - It draws through the kitty graphics protocol (kitty, Ghostty) or iTerm2 3.5+ inline images. Elsewhere it draws its `alt` [R:161].
- **Client** (`ClientProps` [T:1535]; `ClientSurface` [T:1586]): a region drawn by a separate *surface module* on the drawing thread. It has local `state`/`setState`, `every(ms)`, `onPointer` (cell positions, sub-cell where the terminal reports pixels), `onKey`, and `post(data)` back to the hooks (as a `ui.message`).
  - Props must be plain JSON (at most 100,000 chars serialized).
  - Its table is `ClientElements = Omit<Elements['terminal'], 'Client'|'Raster'|'Image'>` [T:1445], so it has **no Svg or Raster**. It exists on terminal and desktop only.

**What "Svg off the terminal" means.** `Svg` is in the desktop, vscode and mobile tables and **not in the terminal's** [T:3919-3925]. The terminal draws pictures only as `Raster` (character cells) or `Image` (pixel graphics on supporting terminals). A mini-map therefore needs one hook that branches on `e.surface`:
- `Svg` on desktop, vscode and mobile;
- `Raster` (braille or half-blocks) on the terminal;
- a text fallback where neither fits.

[T:3919] notes that "terminal no `Svg` but alone `Raster` and `Image`". [R:159] says the same.

### Keyboard and focus [R:167-169][T:1093-1200]

- Keys reach a Button, Input or Select only while its site holds focus.
- A site holds focus after ctrl+x tab, a click, or `$.ui.open({ focus: true })`. The `focus` request is honoured only over an empty composer [T:7605].
- Inside a focused site, Tab and the arrows walk the focus ring and Enter presses. `hotkey` presses directly. Esc returns focus to the prompt.
- `autoFocus` sets where the ring starts. `$.ui.focus({ requestId, key })` moves it.
- `ui.focus` is an event fired on every ring move, and a hook can `{ deny }` the move.
- A pane opened with `focus + closeOnEscape + holdToasts` behaves as a dialog. Leave `holdToasts` off a pane that stays open.
- Button `hotkey` docs: "never the composer, save that a bare digit in an empty one answers a band Button (a survey)" [T:1117-1123]. [A] Whether a bare digit from an empty prompt presses a **mod's** band Button, or only a survey's, is ambiguous. This decides whether "next best action" can be a single keystroke from the prompt. **Spike it.**
- Button `action: "<engine action>"` lets the person's own chord for an engine keybinding press the button from the prompt while it is mounted [T:1124-1130].
- Hotkeys per site: 10 digits plus 26 letters gives **at most 36 one-key actions per pane** [A, derived from the hotkey rule].

### Re-render and update model [R:150-154][T:2383-2396, 3473-3505, 4100-4110]

- A `ui.render` hook is a function of `(e.props, e.viewport, $.state reads)`. It runs again:
  - when the props or viewport width change (a height-only change does **not** redraw);
  - on plugin load;
  - on `$.ui.invalidate('ui.render')`;
  - automatically when a `$.state` value it **read while drawing** is written.
- **A render hook must not write.** `$.state.set` during draw is refused. Writes come from `onPress`/`onSubmit` closures or from other events. Use `update($, atom, fn)` (read-modify-write with `ifVersion` and retry) rather than captured values.
- Helpers from `'claude-code'`: `atom(ref, initial, { shape? })`, `derive(sources, fn)`, `memberOf(family, e)`, `read($, source)`, `update($, atom, fn)` [T:15675-15709].
- Redraw rate: at most **10/s**, or **30/s** on the terminal for a shown pane, the expanded band and the prompt hint. Calls made sooner fold together [T:2387].
- Values outlive a reload. Declare a key as `Shaped<T>` and give the atom `{ shape: 'items-2' }`, and a value stored under an older shape reads as `initial` [R:154][E:pane.tsx].
- **The hook budget is 10 s** of the hook's own code per dispatch (`HookBudget.ms: 10_000`). Calls on `$` and `next` do not count against it, but `$.clock.sleep` does. A `.catch` handler gets a 1 s grace [T:5268-5300]. Draw from state only; never fetch inside a render hook.

### Size constraints [R:166]

- Per tree, the first **100,000 characters** of text are drawn and the rest is cut with a notice. `Input` values get a separate allowance of the same size.
- Svg `source` is at most 131,072 chars. Link addresses are at most 2,048. Keys and values are at most 10,000.
- A pane `title` or command `description` is kept up to 4,096. Toast and status text: 2,000 drawn on the terminal, 10,000 remotely.
- Inside the AskUserQuestion dialog only: 12 rows on the terminal, 48 elsewhere.
- Band: `maxRows` is capped at **half the terminal's rows, prompt included**. A taller tree scrolls in `scroll.bodyRows` [T:10835-10850].

### Density estimate [A, derived from the figures above; spike it, see S9 in KICKOFF]

| Terminal | Placement | Body | Practical content |
|---|---|---|---|
| 200x50 fullscreen | dock | up to about 88 cols (45% minus the frame) x about 40-44 rows | about 40 one-line items visible and scrollable, 36 hotkeyed. A two-column layout (list plus detail) of about 40+44 cols, or a braille mini-map Raster of about 80x20 cells (160x80 dots) above a 20-row list |
| 120x40 fullscreen | dock (from 110 cols) | about 52 cols x about 30-34 rows | one column: focus header (3 rows), decision with 3-4 choice Buttons (5 rows), about 15-20 item rows |
| < 110 cols or main screen | inline above the prompt | full width x a third of the height (about 13 rows at 40), or `rows` requested | about 8-10 items; use the band plus `/room` for the rest |
| desktop / vscode | pane | the surface measures it in code-font cells | the same tree; Svg for the map |
| mobile | pane (`isFullscreen: false`) | narrow | no Input or Select, no band: Buttons and Markdown only |

The text cap (100k chars) is not the binding limit. The binding limits are rows (about 30-44), dock width (at most 45% of columns) and 36 hotkeys.

## 3. `$.command.register` and the `/room` family

Signature: `register: (command: CommandSpec) => Promise<{ command: string }>` [T:3177, 7436]. Call it in `session.start`. `CommandSpec` [T:1922]:

```ts
{ name: string            // letters, digits, _, -; up to 64; a built-in's name is refused
  description: string     // typeahead and /help line (kept up to 4,096)
  argumentHint?: string   // dim hint after the name, e.g. "[map|plan|diff|decide|focus …]"
  immediate?: true }      // runs mid-turn instead of waiting for the turn to end
```

- Serve it with `on('command.run', { command: 'room' }, ($, e) => ({ text, context? }))`.
  - `CommandRunInput` [T:1828] is `{ command, args: string, origin: PromptOrigin, presentation: { isFullscreen, columns } }`.
  - `CommandRunResult` [T:1873] is `{ text?, context?: readonly string[], exitCode? }`. `text` is the transcript row, which the model also reads. `context` is hidden notes only the model reads.
  - A command hook that answers only its own registered command, with a literal-name matcher and no `next`, is reported by validate as "answers its own command", not as a gate [R:66].
- **There is no subcommand construct.** `args` is the raw string after the name, so `/room plan open` means `args = "plan open"`. Planroom parses it itself.
- For subcommand discovery, a `prompt.autocomplete` hook adds typeahead rows (`{ text, label?, description? }`) for the token at the cursor [T:8434-8500].
  - [A] It is unverified whether the engine's slash typeahead and the plugin's rows coexist while typing `/room p…`. Spike it.
- `$.command.run({ command, args })` can run a command programmatically [T:3162].
- `presentation.isFullscreen`/`columns` decides whether to open a pane or print inline [T:1797].

## 4. `$.tool.register`

Signature: `register: (tool: ToolSpec) => Promise<{ tool: string }>` [T:3137, 7432]. `ToolSpec` [T:14033]:

```ts
{ name: string                          // letters, digits, _, -; up to 64
  description: string                   // what the model reads
  inputSchema?: Record<string, unknown> // JSON Schema object; default { type: "object" }
  isDeferred?: boolean }                // false = schema in the prompt from turn 1; default = behind ToolSearch
```

- The model sees the tool as **`mcp__planroom__<name>`**. By default it is deferred behind ToolSearch, and the model loads the schema on demand. Set `isDeferred: false` for the tools the model should reach for unprompted [R:258-264].
- Register in `session.start` and await it, so the tool is listed by turn one [R:196]. It rejects before the session binds [T:3130].
- Serve it with `on('tool.call', { tool: 'mcp__planroom__<name>' }, async ($, e) => ({ result }))`. The arguments are spread on `e`. A call that no hook answers fails. Registering the same name again replaces the tool.
- `ToolCallResult` [T:13566] is `{ result, context? }` or `{ deny: string }`. The result type for an MCP-named tool is `unknown` [T:13989].
  - [A] The doc does not say how a plugin's `result` is serialized for the model (string, JSON, or MCP content blocks). **Return a plain string or a small JSON object and spike what the model sees.**
- Size limits [R:166]:
  - `description` reaches the model up to **4,096** chars, or **16,384** once loaded through ToolSearch;
  - `inputSchema` is sent whole, with a warning past 65,536 chars that it rides on every request;
  - `context` reminders are kept whole up to 100,000 each, 200,000 together, past which head plus path [T:13600].
- [D] The general tool-result cap (MCP output defaults to about 25k tokens, set by `MAX_MCP_OUTPUT_TOKENS`) is not in the types. **Keep results small and paged**, and return ids and summaries rather than whole graphs.
- Permissions: a model call goes through `tool.check` like any tool. The mod can pre-allow its own read-only tools with `on('tool.check', { tool: 'mcp__planroom__read_map' }, () => ({ decision: 'allow' }))` [T:4097, 13687].

## 5. `$.agent.register`, `$.agent.spawn`, isolation `'worktree'`

- `register: (spec: AgentSpec) => Promise<{ agent: string }>` [T:3302]. `AgentSpec` [T:467] fields:
  - core: `name` (becomes `planroom:<name>`), `description`, `prompt` (the whole system prompt)
  - tools and limits: `tools`, `disallowedTools`, `model` (`haiku|sonnet|opus|<id>|inherit`), `effort`, `permissionMode`, `mcpServers`, `hooks`, `maxTurns`, `autoCompactWindow`
  - context: `skills`, `initialPrompt` (`{{intent}}`), `memory`, `background`, `omitClaudeMd`
  - **`isolation?: 'worktree' | 'remote'`**
  - `agents/*.md` in the plugin declares the same thing statically.
- `spawn: (input: AgentSpawnArgs) => Promise<AgentSpawnResult>` [T:3267, 4906].
  - `AgentSpawnArgs = { prompt; description?; subagentType?; model?; name?; cwd? }` [T:251].
  - **Gotcha: `$.agent.spawn` takes no `isolation` argument.** Worktree isolation is a property of the *agent type*. To get it, register `planroom:implementer` with `isolation: 'worktree'` and spawn that `subagentType`.
  - The alternative is `$.tool.call({ tool: 'Agent', description, prompt, subagent_type, isolation: 'worktree' })`, since the Agent tool input has `isolation` [T:17006]. A plugin-raised Agent call always runs in the background and resolves with `AgentCallRecord { agentId, resolvedModel? }` [T:110].
- The spawn resolves **once the subagent has started** (`{ model, agentId }` or `{ deny }`). It always runs in the background.
- **The result comes back as that agent's `turn.complete`**: `on('turn.complete', ($, e, next) => { if (e.agentId === id) resolve(e.answer); return next(e) })`. `TurnCompleteInput` [T:14192] carries `answer`, `reason`, `durationMs`, `usage`, `turnId` and `agentId`.
  - Keep a map from agentId to resolver in `$.state`. Module variables do not survive a reload, and a re-spawn of an interrupted agent restarts [R:198].
  - The Agent tool's completed record carries `worktreePath?` and `worktreeBranch?` [T:17926-17927]. [A] Whether the plugin sees that path on its own spawn is unverified. It may need `$.session.messages({ agentId })` or a convention: the implementer ends its answer with branch and commit.
- `agent.offer` returning `{ isOffered: false }` hides a type from the model while the plugin can still spawn it [T:214-245, 3285-3300]. Use it for a curator the model should not call directly.
- `$.agent.list()` gives `AgentInfo { id, type, status, parentId, spawnedBy, name, description }`. Use it for "who is working on what".

## 6. `$.model.complete`

`complete: (request: ModelCompleteRequest, options?: { signal?: AbortSignal }) => Promise<ModelCompleteResult>` [T:2671].

```ts
ModelCompleteRequest = {           // [T:6352]
  model: string                    // REQUIRED: alias ('haiku') or full id, allowlist-checked like --model
  prompt: string | readonly ModelTextBlock[]
  system?: string | readonly ModelTextBlock[]  // after the CLI's identity block
  maxTokens?: number               // default 1024; max min(model limit, 64000)
  effort?: 'low'|'medium'|'high'|'xhigh'|'max'
  timeoutMs?: number }             // default none
ModelTextBlock = { text: string; cache?: true }   // [T:6571]
ModelCompleteResult =              // [T:6426]  never rejects for provider failures
  | { isAnswered: true;  text; usage }
  | { isAnswered: false; reason: 'api-error'; status: number|null; error: ModelApiError; usage }
  | { isAnswered: false; reason: 'empty-reply' | 'aborted'; usage }
ModelUsage = { input_tokens; output_tokens; cache_read_input_tokens; cache_creation_input_tokens }  // [T:6606]
```

- **Which model and whose account:** the session's own API client and credentials, with the model the caller names. There is no default model, because `model` is required.
  - `$.model.classify(text, labels, { model? })` defaults to "the engine's small fast model" [T:2709].
  - `$.model.fork({ prompt })` reuses the main thread's model, system prompt and cached transcript prefix [T:2690].
- The completion has no tools and no history. The reply is not streamed. It rejects only when the engine refuses to send it (a blocked model or a bad cap).
- **Caching:** `cache: true` on a block caches the request up to and including that block, for 5 minutes [T:6571-6600][R:199].
  - Put the fixed text first and mark its last block.
  - The call that wrote the cache reports `cache_creation_input_tokens`. Calls that read it report `cache_read_input_tokens`.
  - Both read zero when the text is under the model's minimum or caching is off.
  - **On Bedrock, Vertex, Foundry or behind a gateway, mark the cached text in `prompt`, not `system`.**
  - Marks are not counted by the engine. One more than the API accepts comes back as `api-error` with status 400.
- **Cost accounting:** a completion is *not* in the session's cost totals, so the mod must sum `usage` itself. A fork is counted [R:199].
- `turn.complete.usage` is `TurnUsage`, which is `ModelUsage` plus `model`.

## 7. `$.http.fetch`, `$.process`, `$.fs`, `$.store`, `$.state`

- **`fetch(url, init?: HttpInit) => Promise<HttpResponse>`** [T:3579, 5363]. `init` takes `{ method, headers, body: string, auth, socketPath }`. The response is `{ status, ok, headers (lowercased), text }`, resolved **once the body is fully read**.
  - It goes through the host's network to http or https.
  - **`socketPath` is supported** (a Unix domain socket of about 100 bytes at most). It is a clean way to reach the local engine without a TCP port.
  - **There is no timeout or `signal` in `HttpInit`, and no streaming or SSE.** Live updates need polling, or `process.spawn` (below).
  - It can be refused by organization web-fetch policy. `CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC` refuses requests that carry `auth`.
- **`process.run(argv, { cwd, env, stdin, timeoutMs })`** [T:3604, 8203] has no shell, a 30 s default timeout and a 10 min maximum. Output is capped at 4 MiB per stream, with `isStdoutTruncated`.
  - "A background process left writing holds the call until the timeout." A daemon launched with `run` must detach and close its stdio.
  - "CLI only" [T:3590]. Git runs with repo hooks off.
- **`process.spawn({ argv, cwd, env, input }) => HookStream<{ stream, text }, { code, signal }>`** [T:3642, 8269-8330]. It **streams stdout and stderr** piece by piece, as written and not line-split. Backpressure kicks in at 1,048,576 unread chars.
  - **"The loop is the child's life":** leaving the `for await`, `next.signal` aborting, or **the module unloading kills the child**.
  - A session-long child is started from `session.start` in a detached async loop, as in the doc example. A plain `$.process.spawn` server therefore **dies on every hot reload and at session end**. It restarts because `session.start` re-runs on reload.
  - [A] Recommended: `planroom serve --detach`, which is idempotent, self-daemonizing and uses a lockfile, launched with `$.process.run`. Then `planroom watch --json` through `$.process.spawn` as the live event stream into `$.state`. Watch is cheap to lose on a reload, and the server outlives sessions.
  - [A] It is unverified whether the engine kills the child's process group or only its pid. A daemon that `setsid`s should survive either way.
- **`fs`** [T:3312-3431]: `read(path, { as?: 'text'|'bytes' })`, `write(path, text)`, `list`, `exists`, `stat(path, { resolve? })` returning `realPath`, and `ancestors({ names, of?, below? })`.
  - Relative paths resolve against the session cwd. **Reads and writes are capped at 4 MiB.** There is no watch API.
- **`store`** [T:3440-3463]: `get/set/delete/keys` on JSON, per plugin, across sessions, a file under the user's Claude config dir. **The whole store is capped at 4 MiB of JSON.**
- **`state`** [T:3473-3505]: `get(ref) => { value, version }` and `set(ref, value, { ifVersion }) => { isSet, version }`. It is session-scoped and survives a reload.
  - `plugin` and `key` must be literals, declared in `interface PluginState`. Families use `StateFamily<T>` plus `id`.
  - A `get` during draw subscribes the drawing. Only the owner writes; others read.
- Also present: `$.clock.now/sleep/after/every` (timers are cancelled on reload), `$.env.get/set` (literal names), `$.settings.read`, `$.session.*` (`cwd`, `root`, `id`, `repo`, `model`, `messages`, `usage`, `surfaces`, `send`, `append`, `compact`), `$.mcp.call/connect`, and `$.prompt.submit` (queues a new turn when idle).

## 8. Events and lifecycle

Engine events [T:4070-4632] (hook with `on(name, matcher?, ($, e, next) => …)`):

- **Session:**
  - `session.start` `{ cwd, surface, isInteractive }` fires once per process per plugin, **is awaited before the first prompt**, and fires again on every reload [T:4470].
  - `session.end` `{ reason }` shares one short wall-clock bound across the chain.
  - Also: `session.attach`/`session.detach` (remote surfaces joining or leaving), `session.measure` (after each main-thread turn, and when rate limits move), `session.compact`, `session.append` (every stored row), `session.receive`/`session.send` (peer messages).
- **Turn:**
  - `turn.start` `{ text, turnId, agentId? }` (observe only);
  - `turn.step` (a streaming async generator, one per model request);
  - `turn.complete` `{ answer, reason, durationMs, usage, turnId, agentId? }`;
  - `prompt.submit` (rewrite, add `context`, or drop).
- **Tools:**
  - `tool.call` (wrap, deny, or answer; `await next(e)` gives the result);
  - `tool.check` (the permission verdict);
  - `tool.describe` (rewrite descriptions or deferral).
- **Agents:** `agent.spawn` (rewrite the prompt or model, or deny) and `agent.offer`.
- **UI:** `ui.render`, `ui.press`, `ui.input`, `ui.select`, `ui.focus`, `ui.scroll`, `ui.message` (Client posts), `ui.fault`.
- **Prompt:** `prompt.compose` (add a `planroom:*` system-prompt section, `scope: 'session'`), `prompt.context` (blocks on the first message), `prompt.autocomplete`, `prompt.fill`, `prompt.suggest`, `prompt.edit`, `prompt.change`, `prompt.mention`, `prompt.attachment`, `prompt.section`.
- **Other:** `command.run`, `command.describe`, `config.set`, `config.describe`, `skill.prompt`, `attribution.text`, `plugin.register`, `engine.create`, `telemetry.*`.
- Every `$` call is also an event (`model.complete`, `http.fetch`, `process.spawn`, `state.set`, …) [T:7019-7384]. That is how tests stub them.
- Classic settings-hook events are available as `classic.<Event>` (`classic.Stop`, `classic.SubagentStop`, `classic.Notification`, `classic.TeammateIdle`, `classic.FileChanged`, `classic.WorktreeCreate`, …) [T:1274-1300, 5328].
- **Idle:** there is no dedicated `idle` event.
  - "Idle" means `turn.complete` with `agentId === undefined` (or `classic.Stop`).
  - `classic.Notification` carries `notification_type: string`. [D] The documented values include `idle_prompt`.
  - `$.prompt.submit` itself waits for idle [R:198].
  - `session.measure` fires after each main-thread turn.
- **Guards** (`tool.call`, `tool.check`, `command.run`, `prompt.submit`, …) should carry `.catch(($, e, next) => next.called ? next(e) : { deny })`, because a failed hook is skipped [R:79][E:tool-call.ts].

## 9. Testing

- **`claude plugin validate <dir> [--json] [--strict] [--profile directory]`** [V, help text][R:3-5, 66]. It reads `plugin.json`, `marketplace.json`, `hooks.json` and the hooks module's source the way the engine will, and reports:
  - what the module hooks and calls;
  - `gatingHooks` (a fact, not a warning);
  - `$.state` keys checked against the `types` contract;
  - `$.env` names;
  - telemetry streams;
  - and everything the engine would refuse.
  
  It also validates `skills/`, `agents/` and `commands/` in a folder. In CI, `--strict` turns warnings into errors. [V] On a folder without a manifest it exits with `success: false` and "No manifest found…".
- **`claude plugin test [dir]`** [V, help text][R:81][T:15851-16980] runs every `*.test.ts(x)`, each in a child process, in the hooks environment with **no fs, network or process**. It imports `test`, `describe`, `expect`, `mock` and `tier` from `'claude-code/testing'`.
  - `test(name, [{ options }], async ($, on) => …)`. `$` is the engine's own. Hooks registered with the test's `on` sit beneath the plugin and stand in for the engine. A `$` call is answered with `{ value }` [E:pane.test.tsx].
  - So `on('http.fetch', () => ({ value: { status: 200, ok: true, headers: {}, text: '…' } }))` stubs the engine, and `on('model.complete', …)` stubs the model. That gives deterministic CI with no secrets.
  - Mocks: `mock.clock(on)` (`advance`, `set`, `settle`), `mock.store(on, entries)`, `mock.env(on, vars)`, `mock.session(on)` (`appended()`).
  - UI: `$.ui.mount({ plugin, surface, component, props })` returns a `Mounted` drawing with `find({ type, text, key })`, `press({ key })`, `input({ key, text })`, `select`, `key`, `drawn()` and `unmount()`. **Loop each UI test over `['terminal','desktop','vscode','mobile']`.**
  - `$.session.start(...)` and `$.tool.call(...)` raise events. `test(name, { options }, body)` injects `userConfig`.
  - Every act resolves only after the chain and any unawaited handler work have settled, so tests need no sleeps.
- **Unit tests for mod logic** [A]: keep reducers, the arg parser, layout math and protocol decoding in pure TS modules with no `$` dependency. Test them with the repo's normal TS runner (vitest) under Node. Use `claude plugin test` for hook wiring and UI trees.
  - Type-check with `tsc -p` against the header tsconfig, kept outside the plugin folder for CI [R:50-53].
- `claude plugin details <name>` shows a "component inventory and projected token cost" [V, help]. That is useful to track the cost of the mod's tool descriptions.

## 10. Limits and gotchas (consolidated)

1. **Early access.** The API "may change between releases without notice" [T:4]. Pin Claude Code 2.1.310 in CI and regenerate the types on upgrade [R:63].
2. **Writing a mod into the session's mods folder raises a one-time consent question** [S]. Use `--plugin-dir` or the folder marketplace for Planroom's own development.
3. **No `import()`, no `require`, no Node, no DOM.** Bundle third-party code as ESM with no code-splitting. `Client.module` paths must be string literals that point at real plugin files.
4. **A render hook cannot write state, and has a 10 s budget.** Put I/O in timers, events or the `process.spawn` loop.
5. **`process.spawn` children die on reload and session end.** `process.run` blocks on a child that keeps stdio open.
6. **`http.fetch` has no timeout, no abort signal and no streaming.** Bound calls yourself and never `await` one in a gating hook.
7. **`$.agent.spawn` takes no `isolation`.** Put `isolation: 'worktree'` on the registered agent type, or call the Agent tool. Spawns are always background, and results arrive via `turn.complete`.
8. **`$.model.complete` needs an explicit `model`.** It is not in session cost totals. Behind a gateway, cache marks go in `prompt`, not `system`.
9. **The band is terminal and desktop only. Svg is not on the terminal. Raster and Image are terminal only. Mobile has no Input or Select.** Branch on `e.surface`.
10. **Pane placement thresholds:** unasked only from 144 cols, dock from 110 cols in fullscreen, dock width at most 45%. Opening a pane unasked on a narrow terminal leaves it waiting (`isPlaced: false`).
11. **Hotkeys are one character:** 36 per site, and Shift+w equals `w` [T:1117-1123].
12. **`$.store` and `fs` are capped at 4 MiB.** Do not mirror the graph there; the engine's SQLite is the store.
13. **Plugin tool names are `mcp__planroom__*` and deferred by default.** How a tool's `result` is serialized for the model is undocumented [A].
14. **Module variables reset on reload. Timers are cancelled.** `session.start` runs again. Make startup idempotent.
15. **One `title` per pane, tabs when there are several.** Toasts are capped at 50 per plugin. Status is one line per plugin.
16. **Install-command conflict.** reference.md shows `--marketplace` [R:89], but KICKOFF says no such flag. Verify.
17. **A guard that throws is skipped** unless it has a `.catch`.

---

## Design implications for Planroom

1. **One hooks module, `plugin/hooks/register.js`, bundled from `packages/mod`.**
   - `session.start` does the following, idempotently: ensure the engine binary; run `planroom serve --detach` over a per-workspace Unix socket (`$.process.run`); register `/room`, the tools and the agent types; start a `planroom watch --json` stream with `$.process.spawn` that writes `$.state` atoms; open the pane only when `viewport.isFullscreen` is true.
   - All engine reads go through `$.http.fetch(url, { socketPath })` with our own timeout wrapper.
2. **State contract** (`plugin/types/index.d.ts`), small `Shaped<T>` atoms under `planroom`:
   - `focus`, `decision`, `progress`, `pendingDiffs`, `drift`, `presence`, `minimap`, `agents`, `engine`.
   - These hold summaries only. The graph stays in SQLite.
3. **Surfaces:**
   - **Pane** `planroom` (dock): a focus header, the one open decision as hotkeyed Buttons (`1`-`4`), progress, a diff list (`Code format="diff"` on expand), drift and presence. The mini-map is `Svg` off the terminal and a braille `Raster` on it (repainted with `$.ui.blit`).
   - **Band:** a single "next best action" row with a primary Button, yielding to surveys.
   - **`$.prompt.suggest`** offers the next `/room …` command.
   - **Status:** engine health and the scan state.
   - **Toast:** only for completions.
4. **`/room` is one registered command.** Subcommands are parsed from `args` and discovered through `prompt.autocomplete`. `immediate: true` lets `/room focus` and `/room decide` run mid-turn. Rich output goes through the `CommandOutput` site. The help is generated from one subcommand table, which is also the snapshot under test.
5. **Tools are thin proxies over the engine API.**
   - `isDeferred: false` only for the 2-3 core tools (`read_focus`, `propose_edit`, `decide`).
   - Paged and summarized results with stable error codes.
   - `tool.check` auto-allows the read-only tools.
6. **Agents:**
   - `planroom:implementer` is registered with `isolation: 'worktree'` and must report branch and commit in its final answer.
   - `planroom:curator` is hidden through `agent.offer`.
   - Spawn results are joined via `turn.complete` on `agentId`, tracked in `$.state`.
7. **Model use in the mod:** `$.model.complete` only for small, mod-local summaries (an explicit `model`, the fixed rubric first with `cache: true`, usage summed into our own ledger). Cheap decisions go to the engine (Jev) over HTTP.
8. **Testing:**
   - vitest for the pure logic;
   - `claude plugin test` with `http.fetch`/`model.complete` stubs, every UI test looped over all four surfaces;
   - `claude plugin validate --strict` and a `tsc -p` type-check in CI, pinned to Claude Code 2.1.310.

### Top risks

1. **Pane density and keyboard reach.**
   - Fact: at 120 columns the dock is about 52 columns, and 36 hotkeys per site.
   - Unverified: whether a bare digit can press a band Button from the prompt.
   - Why it matters: this decides S9's "two keystrokes". Spike it first, in a real session.
2. **Engine process lifetime under hot reload.** Spawned children die on reload, and `process.run` blocks on attached stdio. A self-daemonizing engine with a lockfile and socket is needed, and live updates depend on a reconnecting `watch` stream.
3. **API churn and undocumented edges.**
   - The API is early access.
   - Undocumented: how plugin tool results are serialized, whether `hooks.json` can mix `modules` with classic hooks, and the worktree path on a plugin spawn.
   - Unconfirmed: the install-flag conflict.
   - Pin the version and add a contract-test spike for each.
