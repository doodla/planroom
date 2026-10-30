/**
 * The pane's and the band's rows, as plain data: the PaneModel or a fallback laid out for a body of
 * `columns` by `rows` cells (architecture.md, "Pane"). register.tsx turns rows into elements.
 *
 * Every string a row draws comes from the repo, the engine or the resolver: the engine's own lines
 * from the PaneModel, and the mod's lines from `MOD_STRINGS`, which `cargo xtask gen` writes from
 * the engine's `en.toml` (DR-21). So `text` and each Button label pass the display-sanitizing pass
 * before they're cut to fit (DR-64, security-5). Every Button runs something the engine accepts
 * today (DR-85): a block row's key selects it and runs nothing (DR-54), `x` then goes down into
 * it through `map --focus` and `z` comes back, and a next action the person ran shows its result
 * in place of the press. A picker `l`, `p` or `i` dealt takes the card's place and owns every
 * digit and row key until it closes (DR-77).
 */

import {
  CATALOGUE,
  MOD_STRINGS,
  type CatalogueLeaf,
  type DiffLine,
  type Handle,
  type PaneModel,
  type Picker,
  type PickerName,
} from "@planroom/contract";
import {
  BAND_KEYS,
  CARD_KEYS,
  fill,
  keyRows,
  page,
  PAGE_ROWS,
  ROW_KEYS,
  sanitize,
  truncate,
  width,
  type ActionKey,
  type RowKey,
} from "@planroom/ui";

import { bandMayRun } from "./catalogue";
import type { Fallback } from "./fallback";
import { shows, type Shown } from "./room";
import type { View } from "./views";

/** The narrowest dock, measured at 120x40 [mod-spike]. */
export const DOCK = { columns: 49, rows: 34 } as const;

/** A fixed choice label holds at most this many columns (DR-54). */
export const CHOICE_COLUMNS = 20;

/**
 * The rows a row key can select: the top three diff lines, the drift and who lines' top items, and
 * a block row, named by its node's ID.
 */
export type Selectable =
  `diff-${0 | 1 | 2}` | "drift-0" | "who-0" | `block-${string}`;

/**
 * What pressing a Button does: a command, a fallback's step, putting the next action back,
 * selecting a row for an action key to act on, turning the list's page, coming back up from a
 * view, or dealing a picker.
 */
export type Action =
  | {
      kind: "run";
      command: string;
      args: string[];
      label: string;
      /** The press takes back the last one: its reply offers no undo of its own. */
      undo?: boolean;
      /** The press goes down into a block: its result is a view on the pane's stack. */
      down?: boolean;
      /** The press is a pick from this picker: its result is a view, and the picker closes. */
      pick?: PickerName;
    }
  | { kind: "fallback"; run: "retry" | "build" | "rekey" }
  | { kind: "clear" }
  | { kind: "select"; row: Selectable }
  | { kind: "page"; index: number }
  | { kind: "up" }
  | { kind: "open"; picker: PickerName };

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
  // One name for an actor however many surfaces it is on; its doings are said once each.
  const actors = new Map<string, { name: string; doing: string[] }>();
  for (const p of model.who) {
    const id =
      p.actor.kind === "person"
        ? `person:${p.actor.id}`
        : p.actor.kind === "agent"
          ? `agent:${p.actor.session}`
          : p.actor.kind;
    const name =
      p.actor.kind === "person"
        ? (p.actor.name ?? p.actor.id)
        : p.actor.kind === "agent"
          ? say("who_agent", { role: p.actor.role, session: p.actor.session })
          : p.actor.kind;
    const held = actors.get(id) ?? { name, doing: [] };
    if (p.doing !== null && !held.doing.includes(p.doing)) {
      held.doing.push(p.doing);
    }
    actors.set(id, held);
  }
  const who = [...actors.values()].map((a) =>
    a.doing.length === 0
      ? a.name
      : say("who_doing", { name: a.name, doing: a.doing.join(", ") }),
  );
  if (who.length === 0) {
    return text("who", say("nobody_here"), columns, "dim");
  }
  return {
    kind: "buttons",
    key: "who",
    buttons: [selectButton("who-0", who.join(" · "), hotkey, columns, view)],
  };
}

/** The row keys each section takes; a list row has none while a picker is open. */
interface ListKeys {
  diffs: readonly RowKey[];
  drift: RowKey | undefined;
  who: RowKey | undefined;
  list: readonly (RowKey | undefined)[];
  page?: RowKey;
}

/** One line of the list: a block, which takes a row key when it has a node ID, or a plain row. */
interface BlockItem {
  key: string;
  name: string;
  focused: boolean;
  /** The node `x` goes down into; null for a row that is only text. */
  id: string | null;
}

/** What the list draws: its title, its rows, and the line for no rows. */
interface BlockList {
  title: string;
  items: BlockItem[];
  none: string;
  /** A pane in an empty state says why on its card, so its empty list draws nothing. */
  silent: boolean;
}

/**
 * The list's source, in the order the pane prefers: the top view of the stack, else the map the
 * next action showed (plain rows: it keeps names only), else the pane's own mini-map.
 */
function blockList(
  model: PaneModel,
  shown: Shown | null,
  views: readonly View[],
): BlockList {
  const top = views.at(-1);
  if (top !== undefined) {
    return {
      title: top.title,
      items: top.rows.map((r, i) => ({
        key: r.id ?? `view-${i}`,
        name: r.name,
        focused: false,
        id: r.id,
      })),
      none: say("no_blocks"),
      silent: false,
    };
  }
  const map = shown?.level != null ? shown : null;
  const level = map?.level ?? model.minimap?.level ?? model.focus.level;
  const items: BlockItem[] =
    map !== null
      ? map.blocks.map((name, i) => ({
          key: `shown-${i}`,
          name,
          focused: false,
          id: null,
        }))
      : (model.minimap?.blocks ?? []).map((b) => ({
          key: b.id,
          name: b.name,
          focused: b.focused,
          id: b.id,
        }));
  return {
    title: say("blocks_title", { level }),
    items,
    none: (map === null ? null : map.empty) ?? say("no_blocks"),
    silent: map === null && model.empty !== null && model.empty !== undefined,
  };
}

/** The blocks that take a row key: those with a node ID. */
function keyedCount(list: BlockList): number {
  return list.items.filter((i) => i.id !== null).length;
}

/**
 * The list: the pane's blocks, a shown map's or the top view's rows, a page of as many as it has
 * row keys for, with one paging line. A block row is a Button on a row key that selects it; `x`
 * then goes down into it (DR-54, DR-85). A plain row has no key. With room for one row the list
 * draws only its paging line; past one page a Button turns it, wrapping from the last page to the
 * first, on the next free row key (DR-77). The page holds as many rows as `keys` has, never a
 * fixed count, so the paging Button keeps the key after the last of them.
 */
function blockRows(
  list: BlockList,
  columns: number,
  room: number,
  view: PaneView,
  keys: readonly (RowKey | undefined)[],
  pageKey: RowKey | undefined,
): Row[] {
  if (room <= 0) return [];
  const blocks = list.items;
  if (blocks.length === 0) {
    if (list.silent) return [];
    const none = text("blocks-none", list.none, columns, "dim");
    return room === 1
      ? [none]
      : [text("blocks-title", list.title, columns, "dim"), none];
  }
  const fits = (size: number) => page(blocks, view.page ?? 0, size);
  // A keyed list pages by the keys it has; a plain one by the pool's page.
  const most = keyedCount(list) === 0 ? PAGE_ROWS : Math.max(1, keys.length);
  // The title and the paging line, plus the Button when there is more than one page.
  let size = Math.min(most, room - 2);
  if (size >= 1 && fits(size).pages > 1) size = Math.min(most, room - 3);
  // With no room for a row, the paging line counts the list's own page behind its key.
  const shownPage = fits(size < 1 ? most : size);
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
    text("blocks-title", list.title, columns, "dim"),
    ...shownPage.rows.map((b, i): Row => {
      const line = say(b.focused ? "focused_block_row" : "block_row", {
        name: b.name,
      });
      return b.id === null
        ? text(`block-${b.key}`, line, columns)
        : {
            kind: "buttons",
            key: `block-row-${b.key}`,
            buttons: [
              selectButton(`block-${b.id}`, line, keys[i], columns, view),
            ],
          };
    }),
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

/** The action keys that deal each picker. */
export const PICKER_OPEN_KEYS: Readonly<Record<PickerName, ActionKey>> = {
  lens: "l",
  project: "p",
  stage: "i",
};

/**
 * The key each entry of `picker` takes, entry by entry: the first four the card's `1` to `4`, the
 * rest the pool's row keys in order, a fixed key per entry whatever else is on screen. An entry
 * whose command the band may not run (one that spends: a stage's draft) is a label and takes no
 * key, but keeps its place, so the entries after it keep theirs. A picker with nothing to pick
 * takes none: no entries, or every entry already in force (DR-86).
 */
export function pickerKeys(
  picker: Picker,
  leaves: readonly CatalogueLeaf[] = CATALOGUE,
): (string | null)[] {
  const pickable = picker.entries.some((e) => !e.current);
  return picker.entries.map((e, i) =>
    !pickable || !bandMayRun(e.handle.command, leaves)
      ? null
      : ((i < CARD_KEYS.length
          ? CARD_KEYS[i]
          : ROW_KEYS[i - CARD_KEYS.length]) ?? null),
  );
}

/** One entry as a Button on its key: its press runs the entry's handle as a pick. */
function pickButton(
  picker: Picker,
  n: number,
  hotkey: string,
  value: string,
  current: boolean,
): ButtonSpec {
  const entry = picker.entries[n];
  const handle = entry?.handle;
  const button: ButtonSpec = {
    key: `picker-entry-${String(n + 1)}`,
    label: value,
    hotkey,
    action: {
      kind: "run",
      command: handle?.command ?? "",
      args: handle?.args ?? [],
      label: handle?.label ?? "",
      pick: picker.name,
    },
  };
  if (current) button.primary = true;
  return button;
}

/** An entry's line: its label and what it holds. */
function entryLine(picker: Picker, n: number): string {
  const e = picker.entries[n];
  return say("picker_entry", {
    label: e?.label ?? "",
    detail: e?.detail ?? "",
  });
}

/**
 * The picker in the card's place: its question, its why, the first four entries two to a row
 * (those with a key as Buttons, the rest flat lines) and what the engine left out (DR-77).
 */
function pickerCardRows(
  picker: Picker,
  columns: number,
  leaves: readonly CatalogueLeaf[],
): Row[] {
  const keys = pickerKeys(picker, leaves);
  const rows: Row[] = [
    text("picker-question", picker.question, columns),
    text("picker-why", picker.why, columns, "dim"),
  ];
  if (picker.entries.length === 0) {
    rows.push(text("picker-empty", say("picker_empty"), columns, "dim"));
  }
  const first = Math.min(picker.entries.length, CARD_KEYS.length);
  for (let i = 0; i < first; i += 2) {
    const keyed: ButtonSpec[] = [];
    const flat: Row[] = [];
    for (const n of [i, i + 1].filter((n) => n < first)) {
      const hotkey = keys[n];
      if (hotkey === null || hotkey === undefined) {
        flat.push(
          text(`picker-entry-${String(n + 1)}`, entryLine(picker, n), columns),
        );
      } else {
        keyed.push(
          pickButton(
            picker,
            n,
            hotkey,
            label(picker.entries[n]?.label ?? "", CHOICE_COLUMNS),
            picker.entries[n]?.current === true,
          ),
        );
      }
    }
    if (keyed.length > 0) {
      rows.push({
        kind: "buttons",
        key: `picker-card-${String(i)}`,
        buttons: keyed,
      });
    }
    rows.push(...flat);
  }
  if (picker.more > 0) {
    rows.push(
      text(
        "picker-more",
        say("picker_more", { count: picker.more }),
        columns,
        "dim",
      ),
    );
  }
  return rows;
}

/**
 * The picker's list: entries 5 to 16, each on its fixed pool key, as one page; nothing else is
 * keyed while a picker is open, so `PAGE_ROWS` doesn't apply and no Button turns a page. With less
 * room than entries, the first `room - 1` and one line for those cut.
 */
function pickerListRows(
  picker: Picker,
  columns: number,
  room: number,
  leaves: readonly CatalogueLeaf[],
): Row[] {
  const keys = pickerKeys(picker, leaves);
  const rest = picker.entries.length - CARD_KEYS.length;
  if (rest <= 0 || room <= 0) return [];
  const shown = rest <= room ? rest : room - 1;
  const rows: Row[] = [];
  for (let i = 0; i < shown; i++) {
    const n = CARD_KEYS.length + i;
    const hotkey = keys[n];
    const line = entryLine(picker, n);
    rows.push(
      hotkey === null || hotkey === undefined
        ? text(`picker-entry-${String(n + 1)}`, line, columns)
        : {
            kind: "buttons",
            key: `picker-row-${String(n + 1)}`,
            buttons: [
              pickButton(
                picker,
                n,
                hotkey,
                label(line, columns - hotkey.length - 2),
                false,
              ),
            ],
          },
    );
  }
  if (shown < rest) {
    rows.push(
      text(
        "picker-list-more",
        say("picker_more", { count: rest - shown }),
        columns,
        "dim",
      ),
    );
  }
  return rows;
}

/** The Buttons that deal the pickers: `l`, `p` and `i`, each cut to a third of `columns` at most. */
function pickerOpenButtons(columns: number): ButtonSpec[] {
  const names: [PickerName, string][] = [
    ["lens", say("lens_picker")],
    ["project", say("project_picker")],
    ["stage", say("stage_picker")],
  ];
  const fitted = Math.max(1, Math.floor((columns - GAP * 2) / 3) - 3);
  return names.map(([picker, name]) => ({
    key: `picker-${picker}`,
    label: label(name, fitted),
    hotkey: PICKER_OPEN_KEYS[picker],
    action: { kind: "open", picker },
  }));
}

/**
 * The pane for a PaneModel: the full layout from 49x34, else the compact one (DR-54, DR-77).
 * `shown` is what the next action showed when the person ran it, drawn while the pane still
 * offers it; `view` is the list's page and the row a row key selected; `undo` is the press the
 * pane's `u` takes back, while there is one; `views` is the stack `x` went down into, whose top
 * the list draws in place of the blocks; `picker` is the one a key dealt, which takes the card's
 * place and the digits and row keys, its fifth entry on drawing in the list's place.
 */
export function paneRows(
  model: PaneModel,
  columns: number,
  rows: number,
  shown: Shown | null = null,
  view: PaneView = {},
  undo: Handle | null = null,
  views: readonly View[] = [],
  picker: Picker | null = null,
  leaves: readonly CatalogueLeaf[] = CATALOGUE,
): Row[] {
  const compact = columns < DOCK.columns || rows < DOCK.rows;
  const result = shownNext(model, shown);
  const out: Row[] = [
    ...focusRows(model, columns, compact),
    ...(picker === null
      ? cardRows(model, columns, result)
      : pickerCardRows(picker, columns, leaves)),
  ];
  const list = blockList(model, result, views);
  const hasTop = model.drift.top !== undefined && model.drift.top !== null;
  const firstBlock = list.items.find((i) => i.id !== null);
  const block: Selectable | null =
    firstBlock?.id == null ? null : `block-${firstBlock.id}`;
  // An action key acts on the selected row: the first keyed row until a row key selects another.
  const first: Selectable | null =
    model.diffs.length > 0
      ? "diff-0"
      : compact
        ? block
        : hasTop
          ? "drift-0"
          : model.who.length > 0
            ? "who-0"
            : block;
  const at: PaneView = { ...view, selected: view.selected ?? first };
  // The block rows take their keys after the diff, drift and who keys. While a picker is open
  // every digit and row key is its own, so these rows draw without one.
  const blocks = Math.min(keyedCount(list), PAGE_ROWS);
  const keyRowsFor = (
    sections: Parameters<typeof keyRows>[0],
    page: boolean,
  ): ListKeys =>
    picker === null
      ? keyRows(sections, page)
      : {
          diffs: [],
          drift: undefined,
          who: undefined,
          list: Array.from({ length: sections.list }, () => undefined),
        };
  let pageKey: RowKey | undefined;
  let listKeys: readonly (RowKey | undefined)[];
  if (compact) {
    // The top pending diff line keeps its row key; the rest move behind their keys.
    const top = model.diffs[0];
    const keys = keyRowsFor(
      {
        diffs: top === undefined ? 0 : 1,
        drift: false,
        who: false,
        list: blocks,
      },
      true,
    );
    pageKey = keys.page;
    listKeys = keys.list;
    if (top !== undefined)
      out.push(diffLine(top, 0, keys.diffs[0], columns, at));
  } else {
    const keys = keyRowsFor(
      {
        diffs: model.diffs.length,
        drift: hasTop,
        who: model.who.length > 0,
        list: blocks,
      },
      true,
    );
    pageKey = keys.page;
    listKeys = keys.list;
    const hunkLines = rows >= 46 ? 5 : 2;
    out.push(progressRow(model, columns));
    out.push(...diffRows(model.diffs, keys.diffs, columns, hunkLines, at));
    out.push(driftRow(model, keys.drift, columns, at));
    out.push(whoRow(model, keys.who, columns, at));
  }
  // `x` goes down into the selected block, and only a block; `z` only while a view is on the stack.
  const selected = at.selected ?? null;
  const down =
    selected !== null &&
    list.items.some((i) => i.id !== null && `block-${i.id}` === selected)
      ? selected.slice("block-".length)
      : null;
  // The picker keys share the action row in a short pane, where a row more would cost the list.
  const together = rows < DOCK.rows;
  const actions = actionRow(
    columns,
    undo,
    down,
    views.length > 0,
    together ? pickerOpenButtons(columns) : [],
  );
  const used =
    out.reduce((sum, r) => sum + rowHeight(r), 0) + (together ? 1 : 2);
  const room = rows - used;
  // A picker with entries past the card's four lists them where the blocks go.
  out.push(
    ...(picker !== null && picker.entries.length > CARD_KEYS.length
      ? pickerListRows(picker, columns, room, leaves)
      : blockRows(list, columns, room, at, listKeys, pageKey)),
  );
  if (!together) {
    out.push({
      kind: "buttons",
      key: "pickers",
      buttons: pickerOpenButtons(columns),
    });
  }
  out.push(actions);
  return out;
}

/** The action keys that go down into the selected block and come back up, and the ones below. */
const DOWN_KEY: ActionKey = "x";
const UP_KEY: ActionKey = "z";
const CANVAS_KEY: ActionKey = "e";
const UNDO_KEY: ActionKey = "u";

/**
 * The list's action row: `x` goes down into the selected block and `z` comes back up from a view,
 * each only when it has something to do, then the picker keys when they share the row; `e` opens the canvas as the person's press, and `u`
 * undoes the last press while the pane offers that (DR-54, DR-85). Every other action key waits
 * for its leaf or data.
 */
function actionRow(
  columns: number,
  undo: Handle | null,
  down: string | null,
  up: boolean,
  pickers: ButtonSpec[],
): Row {
  const buttons: ButtonSpec[] = [];
  if (down !== null) {
    const name = say("down_label");
    buttons.push({
      key: "down",
      label: name,
      hotkey: DOWN_KEY,
      action: {
        kind: "run",
        command: "map",
        args: ["--focus", down],
        label: name,
        down: true,
      },
    });
  }
  if (up) {
    buttons.push({
      key: "up",
      label: say("up_label"),
      hotkey: UP_KEY,
      action: { kind: "up" },
    });
  }
  buttons.push(...pickers);
  const held = buttons.reduce((sum, b) => sum + buttonWidth(b) + GAP, 0);
  if (undo !== null) {
    buttons.push({
      key: "undo",
      label: label(
        undo.label,
        Math.max(1, Math.floor((columns - held) / 2) - 3),
      ),
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
