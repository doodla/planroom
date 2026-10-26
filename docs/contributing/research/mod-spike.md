# Spike: mod pane density and keyboard reach

This page records a live spike of a Claude Code 2.1.310 mod, run on 2026-10-25. It settles the open questions in [Mod API research](mod-api.md) about pane density and how many keystrokes an action takes.

## Method

- A spike plugin (`planroom-spike`) drew a dense pane: a focus header, a decision card with three hotkeyed Buttons, a mini-map (Svg off the terminal, text on it), a progress line, a diff, a presence line and 30 list rows. A band above the prompt showed a next action with two hotkeyed Buttons. A `$.clock.every` timer wrote render and press diagnostics to a file.
- `claude plugin validate` and `claude plugin test` ran on it. The test mounted the pane on all four surfaces.
- A nested interactive session ran `claude --plugin-dir <spike>` in tmux at 200x50 and at 120x40. Keys were sent with `tmux send-keys`, and the screen was read with `tmux capture-pane`.

## Results

| Question | Result | Evidence |
|---|---|---|
| Dock size at 200x50 | 87 body columns, about 46 rows | capture at 200x50 |
| Dock size at 120x40 | 49 body columns, 34 body rows, placement `dock` | `pane.render` diagnostic: `bodyColumns: 49, bodyRows: 34, viewport {columns: 70, rows: 40, isFullscreen: true}` |
| Unasked open at session start | Placed at both sizes (`ui.open` resolved `isPlaced: true`) | diagnostic |
| A bare digit at the empty prompt | Presses the mod's band Button (`1` pressed `band-review`) | diagnostic `press: band-review` |
| `ctrl+x tab` | Focuses the band; a hotkey then presses a band Button (`2` pressed `band-later`) | diagnostic |
| `/room-spike` then Enter | The command opens the pane with `focus: true`; the pane reports `isFocused: true`; `2` presses the pane's `Keep`, and the redraw shows it at once | diagnostic `press: keep`, capture shows `Picked: keep` |
| Four surfaces in `claude plugin test` | One tree draws on all four; terminal drops the Svg branch as designed; 33 Buttons, about 10,000 characters | test output |
| Diff rendering | `Code format="diff"` needs a valid unified-diff hunk header; an invalid one falls back to plain code with a transcript notice | capture |
| Tool result shape (live, in this session) | `result` must be a string or an array of content blocks. A plain object fails with "does not match its output shape". A string reaches the model as one text output; `[{type:'text',text},…]` reaches it as one output per block. | `mcp__planroom-spike__spike_echo` calls |
| A cloud session with no app attached | `session.start` reports `surface: null, isInteractive: false`, and `ui.open` resolves `isPlaced: false, reason: "no-surface: no surface is attached"` | live diagnostic |
| Validate's plugin-directory checks | A plugin needs a `README.md`, a `LICENSE` (or `license` in `plugin.json`) and a 512 to 2048 px square PNG icon at `.claude-plugin/icon.png` | `claude plugin validate` output |

## Implications

- **Keystrokes (S9).** The next best action is one keystroke from the prompt: a bare digit presses the band's primary Button. Every pane action is two: `/room` (or `ctrl+x tab`) for focus, then one hotkey.
- **Hotkey namespaces.** Digits at the empty prompt go to the band. The band owns `1` to `3`. The pane's decision choices also use digits, and those only reach the pane while it holds focus. List rows and secondary actions use letters.
- **Density budget.** Design the pane for 49 columns by 34 rows (120x40, the narrowest dock). That fits a 3-row focus header, a 6-row decision card, a 1-row progress line, a 5-row diff preview, a 1-row presence line and about 15 list rows. Wider terminals get more list rows, not more sections.
- **Tool results are compact JSON text.** Every Planroom tool returns `JSON.stringify` of its envelope as one string, never a bare object.
- **Open the pane when a surface arrives.** A cloud session can start with no surface. The mod opens its pane on `session.start` when a surface is present and again on `session.attach`, and the pane's state never depends on being drawn.
- **The plugin needs** a README, a LICENSE reference and a square PNG icon, which the Iris art set makes.
