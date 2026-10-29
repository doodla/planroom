/**
 * The pane's and the band's rows, as plain data: the PaneModel or a fallback laid out for a body of
 * `columns` by `rows` cells (architecture.md, "Pane"). register.tsx turns rows into elements.
 *
 * Every string a row draws comes from the repo, the engine or the resolver: the engine's own lines
 * from the PaneModel, and the mod's lines from `MOD_STRINGS`, which `cargo xtask gen` writes from
 * the engine's `en.toml` (DR-21). So `text` and each Button label pass the display-sanitizing pass
 * before they're cut to fit (DR-64, security-5). Every Button runs something the engine accepts
 * today (DR-85): blocks are plain rows until `map --focus` exists, a row key selects its row and
 * runs nothing (DR-54), and a next action the person ran shows its result in place of the press.
 */

import {
  CATALOGUE,
  MOD_STRINGS,
  type CatalogueLeaf,
  type DiffLine,
  type Handle,
  type PaneModel,
} from "@planroom/contract";
import {
  BAND_KEYS,
  CARD_KEYS,
  fill,
  keyRows,
  page,
  PAGE_ROWS,
  sanitize,
  truncate,
  width,
  type ActionKey,
  type RowKey,
} from "@planroom/ui";

import { bandMayRun } from "./catalogue";
import type { Fallback } from "./fallback";
import { shows, type Shown } from "./room";

/** The narrowest dock, measured at 120x40 [mod-spike]. */
export const DOCK = { columns: 49, rows: 34 } as const;

/** A fixed choice label holds at most this many columns (DR-54). */
export const CHOICE_COLUMNS = 20;

/** The rows a row key can select: the top three diff lines, the drift and who lines' top items. */
export type Selectable = `diff-${0 | 1 | 2}` | "drift-0" | "who-0";

/**
 * What pressing a Button does: a command, a fallback's step, putting the next action back,
 * selecting a row for an action key to act on, or turning the list's page.
 */
export type Action =
  | {
      kind: "run";
      command: string;
      args: string[];
      label: string;
      /** The press takes back the last one: its reply offers no undo of its own. */
      undo?: boolean;
    }
  | { kind: "fallback"; run: "retry" | "build" | "rekey" }
  | { kind: "clear" }
  | { kind: "select"; row: Selectable }
  | { kind: "page"; index: number };

export interface ButtonSpec {
  key: string;
  label: string;
  hotkey?: string;
  primary?: boolean;
  action: Action;
}

export type Row =
  | {
      kind: "text";
      key: string;
      text: string;
      tone?: "dim" | "accent" | "warn";
    }
  | { kind: "buttons"; key: string; buttons: ButtonSpec[] }
  | { kind: "diff"; key: string; source: string; lines: number };

/** What the pane keeps between draws: the list's page and the row a row key selected. */
export interface PaneView {
  page?: number;
  selected?: string | null;
}

/** Buttons sit apart by this many columns. */
const GAP = 2;

/** One of the mod's lines from `en.toml`, filled. */
function say(
  key: keyof typeof MOD_STRINGS,
  vars: Readonly<Record<string, string | number>> = {},
): string {
  return fill(MOD_STRINGS[key], vars);
}

/** How many columns a plain Button takes: `1: label`, or the label alone. */
export function buttonWidth(b: ButtonSpec): number {
  return width(b.label) + (b.hotkey === undefined ? 0 : b.hotkey.length + 2);
}

/** How many columns a row takes; a diff is drawn by the surface inside the body. */
export function rowWidth(row: Row): number {
  switch (row.kind) {
    case "text":
      return width(row.text);
    case "buttons":
      return (
        row.buttons.reduce((sum, b) => sum + buttonWidth(b), 0) +
        GAP * Math.max(0, row.buttons.length - 1)
      );
    case "diff":
      return 0;
  }
}

/** How many terminal rows a row takes. */
export function rowHeight(row: Row): number {
  return row.kind === "diff" ? row.lines : 1;
}

function text(
  key: string,
  value: string,
  columns: number,
  tone?: "dim" | "accent" | "warn",
): Row {
  const shown = truncate(sanitize(value), columns);
  return tone === undefined
    ? { kind: "text", key, text: shown }
    : { kind: "text", key, text: shown, tone };
}

/** Rows an empty state's line takes at most. */
const EMPTY_LINE_ROWS = 3;

/**
 * `value` as text rows of at most `columns`, sanitized, broken at spaces: at most `most` rows, the
 * last cut to fit when the text runs longer.
 */
function wrapped(
  key: string,
  value: string,
  columns: number,
  most: number,
): Row[] {
  const lines: string[] = [];
  let line = "";
  for (const word of sanitize(value).split(" ")) {
    const longer = line === "" ? word : `${line} ${word}`;
    if (line === "" || width(longer) <= columns) {
      line = longer;
    } else {
      lines.push(line);
      line = word;
    }
  }
  lines.push(line);
  const kept =
    lines.length > most
      ? [...lines.slice(0, most - 1), lines.slice(most - 1).join(" ")]
      : lines;
  return kept.map((l, i) => text(`${key}-${i}`, l, columns));
}

/** A Button's label: sanitized, then cut to `columns`. */
function label(value: string, columns: number): string {
  return truncate(sanitize(value), columns);
}

function run(handle: Handle): Extract<Action, { kind: "run" }> {
  return {
    kind: "run",
    command: handle.command,
    args: handle.args,
    label: handle.label,
  };
}

/**
 * One selectable row as a Button: its row key selects it and runs nothing (DR-54), and the row
 * the person selected is marked. Its label is cut to what's left of `columns` beside the key.
 */
function selectButton(
  row: Selectable,
  value: string,
  hotkey: RowKey | undefined,
  columns: number,
  view: PaneView,
): ButtonSpec {
  const marked =
    view.selected === row ? say("selected_row", { label: value }) : value;
  const room = columns - (hotkey === undefined ? 0 : hotkey.length + 2);
  const button: ButtonSpec = {
    key: row,
    label: label(marked, room),
    action: { kind: "select", row },
  };
  if (hotkey !== undefined) button.hotkey = hotkey;
  return button;
}

/**
 * A unified-diff hunk cut to its header and `body` lines, its header recounted so the cut hunk
 * still parses (`Code format="diff"` draws an invalid one as plain code).
 */
export function cutHunk(hunk: readonly string[], body: number): string | null {
  const [header, ...lines] = hunk;
  const m =
    header === undefined
      ? null
      : /^@@ -(\d+)(?:,\d+)? \+(\d+)(?:,\d+)? @@(.*)$/.exec(header);
  if (m === null) {
    return null;
  }
  const kept = lines.slice(0, body);
  const old = kept.filter((l) => !l.startsWith("+")).length;
  const added = kept.filter((l) => !l.startsWith("-")).length;
  return [`@@ -${m[1]},${old} +${m[2]},${added} @@${m[3]}`, ...kept].join("\n");
}

function focusRows(model: PaneModel, columns: number, compact: boolean): Row[] {
  const crumbs =
    model.focus.breadcrumb.map((c) => c.name).join(" › ") || say("workspace");
  const { level, lens, project, seq } = model.focus;
  if (compact) {
    return [
      text(
        "focus",
        say("compact_focus_line", { crumbs, level, lens }),
        columns,
        "accent",
      ),
    ];
  }
  return [
    text("focus", say("focus_line", { crumbs }), columns, "accent"),
    text("focus-lens", say("lens_line", { level, lens }), columns, "dim"),
    text(
      "focus-project",
      say("project_line", { project: project ?? say("no_project"), seq }),
      columns,
      "dim",
    ),
  ];
}

/** The next action's result, while the pane still offers that action at the seq it ran at. */
function shownNext(model: PaneModel, shown: Shown | null): Shown | null {
  const next = model.empty?.next ?? model.next;
  return model.decision === null && shows(shown, next, model.focus.seq)
    ? shown
    : null;
}

function cardRows(
  model: PaneModel,
  columns: number,
  shown: Shown | null,
): Row[] {
  const card = model.decision;
  if (card === null) {
    const next = model.empty?.next ?? model.next;
    if (shown !== null) {
      // Pressed: its result in place of the press, and a way back to the pane's own level.
      return [
        text("card-title", next.label, columns, "dim"),
        text("card-result", shown.line, columns, "accent"),
        {
          kind: "buttons",
          key: "card-back",
          buttons: [
            {
              key: "back",
              label: label(
                say("back_label", { level: model.focus.level }),
                columns - 3,
              ),
              hotkey: CARD_KEYS[0],
              primary: true,
              action: { kind: "clear" },
            },
          ],
        },
      ];
    }
    // An empty state's action is short ("Check again"), so its line, which says why the pane is
    // empty and what to do, takes the title's place above it.
    const line = model.empty?.line;
    return [
      ...(line === undefined || line === null || line === ""
        ? [text("card-title", say("next_title"), columns, "dim")]
        : wrapped("card-empty", line, columns, EMPTY_LINE_ROWS)),
      {
        kind: "buttons",
        key: "card-next",
        buttons: [
          {
            key: "next",
            label: label(next.label, columns - 3),
            hotkey: CARD_KEYS[0],
            primary: true,
            action: run(next),
          },
        ],
      },
    ];
  }
  const rows: Row[] = [
    text("card-question", card.question, columns),
    text("card-why", card.why, columns, "dim"),
  ];
  const choices: ButtonSpec[] = card.choices
    .slice(0, CARD_KEYS.length)
    .map((c, i) => ({
      key: `choice-${c.id}`,
      label: label(c.label, CHOICE_COLUMNS),
      hotkey: CARD_KEYS[i],
      primary: c.preselected,
      action: {
        kind: "run",
        command: "decide",
        args: [card.id, c.id],
        label: c.label,
      },
    }));
  for (let i = 0; i < choices.length; i += 2) {
    rows.push({
      kind: "buttons",
      key: `card-choices-${i}`,
      buttons: choices.slice(i, i + 2),
    });
  }
  const more: string[] = [];
  if (card.batch !== null)
    more.push(say("batch_count", { count: card.batch.count }));
  if (model.more_decisions > 0)
    more.push(say("more_decisions", { count: model.more_decisions }));
  if (more.length > 0)
    rows.push(text("card-more", more.join(" · "), columns, "dim"));
  return rows;
}

function progressRow(model: PaneModel, columns: number): Row {
  const p = model.progress;
  const parts = [
    p.stage ?? say("progress_no_stage"),
    say("progress_done", { done: p.done, total: p.total }),
  ];
  if (p.handoffs_running > 0)
    parts.push(
      say(
        p.handoffs_running === 1
          ? "progress_handoffs_one"
          : "progress_handoffs",
        { count: p.handoffs_running },
      ),
    );
  if (p.scan !== "done")
    parts.push(
      say(p.scan === "running" ? "progress_scanning" : "progress_not_scanned"),
    );
  return text("progress", parts.join(" · "), columns, "dim");
}

/** One diff line: its row key selects it, and an action key then acts on it. */
function diffLine(
  d: DiffLine,
  i: 0 | 1 | 2,
  hotkey: RowKey | undefined,
  columns: number,
  view: PaneView,
): Row {
  return {
    kind: "buttons",
    key: `diff-row-${i}`,
    buttons: [selectButton(`diff-${i}`, d.summary, hotkey, columns, view)],
  };
}

function diffRows(
  diffs: readonly DiffLine[],
  keys: readonly RowKey[],
  columns: number,
  hunkLines: number,
  view: PaneView,
): Row[] {
  if (diffs.length === 0) {
    return [text("diffs-none", say("no_diffs"), columns, "dim")];
  }
  const rows: Row[] = diffs
    .slice(0, 3)
    .map((d, i) => diffLine(d, i as 0 | 1 | 2, keys[i], columns, view));
  const source =
    diffs[0] === undefined ? null : cutHunk(diffs[0].hunk, hunkLines);
  if (source !== null) {
    rows.splice(1, 0, {
      kind: "diff",
      key: "diff-hunk",
      source,
      lines: source.split("\n").length,
    });
  }
  return rows;
}

/** The drift line: a Button its row key selects when the engine names a top item, else text. */
function driftRow(
  model: PaneModel,
  hotkey: RowKey | undefined,
  columns: number,
  view: PaneView,
): Row {
  const { drift } = model;
  if (drift.top !== undefined && drift.top !== null) {
    return {
      kind: "buttons",
      key: "drift",
      buttons: [selectButton("drift-0", drift.summary, hotkey, columns, view)],
    };
  }
  return text(
    "drift",
    drift.summary,
    columns,
    drift.rules_broken + drift.machine_drift > 0 ? "warn" : "dim",
  );
}

/** The who line: everyone present, its top row's key selecting it when anyone is. */
function whoRow(
  model: PaneModel,
  hotkey: RowKey | undefined,
  columns: number,
  view: PaneView,
): Row {
  const who = model.who.map((p) => {
    const name =
      p.actor.kind === "person"
        ? (p.actor.name ?? p.actor.id)
        : p.actor.kind === "agent"
          ? say("who_agent", { role: p.actor.role, session: p.actor.session })
          : p.actor.kind;
    return p.doing === null ? name : say("who_doing", { name, doing: p.doing });
  });
  if (who.length === 0) {
    return text("who", say("nobody_here"), columns, "dim");
  }
  return {
    kind: "buttons",
    key: "who",
    buttons: [selectButton("who-0", who.join(" · "), hotkey, columns, view)],
  };
}

/**
 * The list: the pane's blocks, or the blocks the next action showed, as plain rows, a page of at
 * most 7 with one paging line. A block row runs nothing until `map --focus` exists (DR-85), so it
 * takes no row key. With room for one row the list draws only its paging line; past one page a
 * Button turns it, wrapping from the last page to the first, on the next free row key (DR-77).
 */
function blockRows(
  model: PaneModel,
  columns: number,
  room: number,
  shown: Shown | null,
  view: PaneView,
  pageKey: RowKey | undefined,
): Row[] {
  if (room <= 0) return [];
  const map = shown?.level != null ? shown : null;
  const level = map?.level ?? model.minimap?.level ?? model.focus.level;
  const blocks =
    map !== null
      ? map.blocks.map((name, i) => ({
          key: `shown-${i}`,
          name,
          focused: false,
        }))
      : (model.minimap?.blocks ?? []).map((b) => ({
          key: b.id,
          name: b.name,
          focused: b.focused,
        }));
  if (blocks.length === 0) {
    // A pane in an empty state says why on its card; a shown map with no nodes says its own line.
    if (map === null && model.empty !== null && model.empty !== undefined) {
      return [];
    }
    const line = (map === null ? null : map.empty) ?? say("no_blocks");
    const none = text("blocks-none", line, columns, "dim");
    return room === 1
      ? [none]
      : [
          text("blocks-title", say("blocks_title", { level }), columns, "dim"),
          none,
        ];
  }
  const fits = (size: number) => page(blocks, view.page ?? 0, size);
  // The title and the paging line, plus the Button when there is more than one page.
  let size = Math.min(PAGE_ROWS, room - 2);
  if (size >= 1 && fits(size).pages > 1) size = Math.min(PAGE_ROWS, room - 3);
  // With no room for a row, the paging line counts the list's own 7-row page behind its key.
  const shownPage = fits(size < 1 ? PAGE_ROWS : size);
  const pageLine = text(
    "blocks-page",
    say("page_line", {
      first: shownPage.first,
      last: shownPage.last,
      total: shownPage.total,
    }),
    columns,
    "dim",
  );
  if (size < 1) return [pageLine];
  const rows: Row[] = [
    text("blocks-title", say("blocks_title", { level }), columns, "dim"),
    ...shownPage.rows.map((b) =>
      text(
        `block-${b.key}`,
        say(b.focused ? "focused_block_row" : "block_row", { name: b.name }),
        columns,
      ),
    ),
    pageLine,
  ];
  if (shownPage.pages > 1) {
    const last = shownPage.index === shownPage.pages - 1;
    const turn: ButtonSpec = {
      key: "page",
      label: label(
        say(last ? "first_page_label" : "next_page_label"),
        columns - (pageKey === undefined ? 0 : pageKey.length + 2),
      ),
      action: { kind: "page", index: last ? 0 : shownPage.index + 1 },
    };
    if (pageKey !== undefined) turn.hotkey = pageKey;
    rows.push({ kind: "buttons", key: "blocks-paging", buttons: [turn] });
  }
  return rows;
}

/**
 * The pane for a PaneModel: the full layout from 49x34, else the compact one (DR-54, DR-77).
 * `shown` is what the next action showed when the person ran it, drawn while the pane still
 * offers it; `view` is the list's page and the row a row key selected; `undo` is the press the
 * pane's `u` takes back, while there is one.
 */
export function paneRows(
  model: PaneModel,
  columns: number,
  rows: number,
  shown: Shown | null = null,
  view: PaneView = {},
  undo: Handle | null = null,
): Row[] {
  const compact = columns < DOCK.columns || rows < DOCK.rows;
  const result = shownNext(model, shown);
  const out: Row[] = [
    ...focusRows(model, columns, compact),
    ...cardRows(model, columns, result),
  ];
  const hasTop = model.drift.top !== undefined && model.drift.top !== null;
  // An action key acts on the selected row: the first keyed row until a row key selects another.
  const first: Selectable | null =
    model.diffs.length > 0
      ? "diff-0"
      : compact
        ? null
        : hasTop
          ? "drift-0"
          : model.who.length > 0
            ? "who-0"
            : null;
  const at: PaneView = { ...view, selected: view.selected ?? first };
  let pageKey: RowKey | undefined;
  if (compact) {
    // The top pending diff line keeps its row key; the rest move behind their keys.
    const top = model.diffs[0];
    const keys = keyRows(
      { diffs: top === undefined ? 0 : 1, drift: false, who: false, list: 0 },
      true,
    );
    pageKey = keys.page;
    if (top !== undefined)
      out.push(diffLine(top, 0, keys.diffs[0], columns, at));
  } else {
    const keys = keyRows(
      {
        diffs: model.diffs.length,
        drift: hasTop,
        who: model.who.length > 0,
        // Block rows take no key until `map --focus` exists (DR-85).
        list: 0,
      },
      true,
    );
    pageKey = keys.page;
    const hunkLines = rows >= 46 ? 5 : 2;
    out.push(progressRow(model, columns));
    out.push(...diffRows(model.diffs, keys.diffs, columns, hunkLines, at));
    out.push(driftRow(model, keys.drift, columns, at));
    out.push(whoRow(model, keys.who, columns, at));
  }
  const actions = actionRow(columns, undo);
  const used = out.reduce((sum, r) => sum + rowHeight(r), 0) + 1;
  out.push(...blockRows(model, columns, rows - used, result, at, pageKey));
  out.push(actions);
  return out;
}

/** The action key that opens the canvas, and the one that takes back the last press. */
const CANVAS_KEY: ActionKey = "e";
const UNDO_KEY: ActionKey = "u";

/**
 * The list's action row: `e` opens the canvas as the person's press, and `u` undoes the last press
 * while the pane offers that (DR-54, DR-85). Every other action key waits for its leaf or data.
 */
function actionRow(columns: number, undo: Handle | null): Row {
  const buttons: ButtonSpec[] = [];
  if (undo !== null) {
    buttons.push({
      key: "undo",
      label: label(undo.label, Math.max(1, Math.floor(columns / 2) - 3)),
      hotkey: UNDO_KEY,
      action: { ...run(undo), undo: true },
    });
  }
  const taken = buttons.reduce((sum, b) => sum + buttonWidth(b) + GAP, 0);
  const canvas = say("canvas_label");
  buttons.push({
    key: "canvas",
    label: label(canvas, Math.max(1, columns - taken - 3)),
    hotkey: CANVAS_KEY,
    action: { kind: "run", command: "open", args: [], label: canvas },
  });
  return { kind: "buttons", key: "actions", buttons };
}

/** The pane for a fallback state: its headline, its detail and its one action. */
export function fallbackRows(f: Fallback, columns: number): Row[] {
  const rows: Row[] = [
    text("fallback-headline", f.headline, columns, "accent"),
  ];
  if (f.detail !== null && f.detail !== "") {
    rows.push(text("fallback-detail", f.detail, columns, "dim"));
  }
  if (f.action !== null) {
    rows.push({
      kind: "buttons",
      key: "fallback-actions",
      buttons: [
        {
          key: "fallback-action",
          label: label(f.action.label, columns - 3),
          hotkey: CARD_KEYS[0],
          primary: true,
          action: { kind: "fallback", run: f.action.run },
        },
      ],
    });
  }
  return rows;
}

/**
 * The band's one row. A digit at the empty prompt presses its Button, so the Button runs only what
 * `bandMayRun` allows; anything else is a label pointing at the pane (DR-54).
 */
export function bandRow(
  source: { model: PaneModel } | { fallback: Fallback },
  columns: number,
  shown: Shown | null = null,
  undo: Handle | null = null,
  leaves: readonly CatalogueLeaf[] = CATALOGUE,
): Row {
  if ("fallback" in source) {
    const f = source.fallback;
    if (f.action === null) {
      return text("band", f.headline, columns, "dim");
    }
    if (f.action.run === "rekey") {
      // Replacing the registered key is a pane press, never a stray digit at the prompt (DR-75).
      return text(
        "band",
        say("band_rekey", { headline: f.headline }),
        columns,
        "dim",
      );
    }
    const primary = label(
      say("band_fallback", { headline: f.headline, action: f.action.label }),
      columns - 3,
    );
    return {
      kind: "buttons",
      key: "band",
      buttons: [
        {
          key: "band-primary",
          label: primary,
          hotkey: BAND_KEYS.primary,
          primary: true,
          action: { kind: "fallback", run: f.action.run },
        },
      ],
    };
  }
  if (undo !== null && bandMayRun(undo.command, leaves)) {
    // For 10 s after a press that can be undone, the band's primary takes it back (DR-54).
    return {
      kind: "buttons",
      key: "band",
      buttons: [
        {
          key: "band-primary",
          label: label(undo.label, columns - 3),
          hotkey: BAND_KEYS.primary,
          primary: true,
          action: { ...run(undo), undo: true },
        },
      ],
    };
  }
  const next = source.model.empty?.next ?? source.model.next;
  if (shownNext(source.model, shown) !== null) {
    // Already run: the band says what it showed instead of offering the same press again.
    return text(
      "band",
      say("band_shown", { label: next.label, line: shown?.line ?? "" }),
      columns,
      "dim",
    );
  }
  if (!bandMayRun(next.command, leaves)) {
    return text(
      "band",
      say("band_points_to_pane", { label: next.label }),
      columns,
      "dim",
    );
  }
  return {
    kind: "buttons",
    key: "band",
    buttons: [
      {
        key: "band-primary",
        label: label(next.label, columns - 3),
        hotkey: BAND_KEYS.primary,
        primary: true,
        action: run(next),
      },
    ],
  };
}
