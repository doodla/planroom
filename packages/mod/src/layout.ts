/**
 * The pane's and the band's rows, as plain data: the PaneModel or a fallback laid out for a body of
 * `columns` by `rows` cells (architecture.md, "Pane"). register.tsx turns rows into elements.
 */

import type { DiffLine, Handle, PaneModel } from "@planroom/contract";
import { BAND_KEYS, rowKeys, truncate, width } from "@planroom/ui";

import { bandMayRun } from "./catalogue";
import type { Fallback } from "./fallback";

/** The narrowest dock, measured at 120x40 [mod-spike]. */
export const DOCK = { columns: 49, rows: 34 } as const;

/** A fixed choice label holds at most this many columns (DR-54). */
export const CHOICE_COLUMNS = 20;

/** What pressing a Button does. */
export type Action =
  | { kind: "run"; command: string; args: string[]; label: string }
  | { kind: "fallback"; run: "retry" | "build" };

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

/** Buttons sit apart by this many columns. */
const GAP = 2;

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
  return tone === undefined
    ? { kind: "text", key, text: truncate(value, columns) }
    : { kind: "text", key, text: truncate(value, columns), tone };
}

function run(handle: Handle): Action {
  return {
    kind: "run",
    command: handle.command,
    args: handle.args,
    label: handle.label,
  };
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
    model.focus.breadcrumb.map((c) => c.name).join(" › ") || "workspace";
  const { level, lens, project, seq } = model.focus;
  if (compact) {
    return [
      text("focus", `▣ ${crumbs} · ${level} · ${lens}`, columns, "accent"),
    ];
  }
  return [
    text("focus", `▣ ${crumbs}`, columns, "accent"),
    text("focus-lens", `${level} · lens ${lens}`, columns, "dim"),
    text(
      "focus-project",
      `${project ?? "no project"} · seq ${seq}`,
      columns,
      "dim",
    ),
  ];
}

function cardRows(model: PaneModel, columns: number): Row[] {
  const card = model.decision;
  if (card === null) {
    const next = model.empty?.next ?? model.next;
    return [
      text("card-title", "Next", columns, "dim"),
      {
        kind: "buttons",
        key: "card-next",
        buttons: [
          {
            key: "next",
            label: truncate(next.label, columns - 3),
            hotkey: "1",
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
  const choices: ButtonSpec[] = card.choices.slice(0, 4).map((c, i) => ({
    key: `choice-${c.id}`,
    label: truncate(c.label, CHOICE_COLUMNS),
    hotkey: String(i + 1),
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
  if (card.batch !== null) more.push(`applies ${card.batch.count} items`);
  if (model.more_decisions > 0) more.push(`+${model.more_decisions} more`);
  if (more.length > 0)
    rows.push(text("card-more", more.join(" · "), columns, "dim"));
  return rows;
}

function progressRow(model: PaneModel, columns: number): Row {
  const p = model.progress;
  const parts = [p.stage ?? "no plan stage", `${p.done}/${p.total} done`];
  if (p.handoffs_running > 0)
    parts.push(
      `${p.handoffs_running} hand-off${p.handoffs_running === 1 ? "" : "s"} running`,
    );
  if (p.scan !== "done")
    parts.push(p.scan === "running" ? "scanning" : "not scanned");
  return text("progress", parts.join(" · "), columns, "dim");
}

function diffRows(
  diffs: readonly DiffLine[],
  keys: (string | undefined)[],
  columns: number,
  hunkLines: number,
): Row[] {
  if (diffs.length === 0) {
    return [text("diffs-none", "No pending diffs", columns, "dim")];
  }
  const rows: Row[] = diffs.slice(0, 3).map((d, i) => {
    const hotkey = keys[i];
    const room = columns - (hotkey === undefined ? 0 : hotkey.length + 2);
    const button: ButtonSpec = {
      key: `diff-${i}`,
      label: truncate(d.summary, room),
      action: run(d.open),
    };
    if (hotkey !== undefined) button.hotkey = hotkey;
    return { kind: "buttons", key: `diff-row-${i}`, buttons: [button] };
  });
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

function whoRow(model: PaneModel, columns: number): Row {
  const who = model.who.map((p) => {
    const name =
      p.actor.kind === "person"
        ? (p.actor.name ?? p.actor.id)
        : p.actor.kind === "agent"
          ? `${p.actor.role} ${p.actor.session}`
          : p.actor.kind;
    return p.doing === null ? name : `${name}: ${p.doing}`;
  });
  return text(
    "who",
    who.length === 0 ? "Nobody else is here" : who.join(" · "),
    columns,
    "dim",
  );
}

function blockRows(
  model: PaneModel,
  keys: (string | undefined)[],
  columns: number,
  room: number,
): Row[] {
  const blocks = model.minimap.blocks;
  const rows: Row[] = [
    text("blocks-title", `Blocks · ${model.minimap.level}`, columns, "dim"),
  ];
  const shown = Math.max(0, Math.min(blocks.length, 9, room - 2));
  blocks.slice(0, shown).forEach((b, i) => {
    const hotkey = keys[i];
    const label = truncate(
      `${b.focused ? "▣" : "□"} ${b.name}`,
      columns - (hotkey === undefined ? 0 : hotkey.length + 2),
    );
    const button: ButtonSpec = {
      key: `block-${b.id}`,
      label,
      action: {
        kind: "run",
        command: "map",
        args: ["--focus", b.id],
        label: b.name,
      },
    };
    if (hotkey !== undefined) button.hotkey = hotkey;
    rows.push({ kind: "buttons", key: `block-row-${i}`, buttons: [button] });
  });
  if (blocks.length > shown)
    rows.push(
      text("blocks-more", `+${blocks.length - shown} more`, columns, "dim"),
    );
  if (blocks.length === 0)
    rows.push(
      text("blocks-none", "No blocks at this level yet", columns, "dim"),
    );
  return rows;
}

/** The pane for a PaneModel: the full layout from 49x34, else the compact one (DR-54). */
export function paneRows(
  model: PaneModel,
  columns: number,
  rows: number,
): Row[] {
  const compact = columns < DOCK.columns || rows < DOCK.rows;
  const out: Row[] = [
    ...focusRows(model, columns, compact),
    ...cardRows(model, columns),
  ];
  const keys = rowKeys(12);
  if (!compact) {
    const hunkLines = rows >= 46 ? 5 : 2;
    out.push(progressRow(model, columns));
    out.push(...diffRows(model.diffs, keys.slice(0, 3), columns, hunkLines));
    out.push(
      text(
        "drift",
        model.drift.summary,
        columns,
        model.drift.rules_broken + model.drift.machine_drift > 0
          ? "warn"
          : "dim",
      ),
    );
    out.push(whoRow(model, columns));
  }
  const used = out.reduce((sum, r) => sum + rowHeight(r), 0);
  const listKeys = compact ? keys : keys.slice(Math.min(3, model.diffs.length));
  out.push(...blockRows(model, listKeys, columns, rows - used));
  return out;
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
          label: f.action.label,
          hotkey: "1",
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
): Row {
  if ("fallback" in source) {
    const f = source.fallback;
    if (f.action === null) {
      return text("band", f.headline, columns, "dim");
    }
    const label = truncate(`${f.headline} ${f.action.label}`, columns - 3);
    return {
      kind: "buttons",
      key: "band",
      buttons: [
        {
          key: "band-primary",
          label,
          hotkey: BAND_KEYS.primary,
          primary: true,
          action: { kind: "fallback", run: f.action.run },
        },
      ],
    };
  }
  const next = source.model.empty?.next ?? source.model.next;
  if (!bandMayRun(next.command)) {
    return text(
      "band",
      truncate(`Next: ${next.label} · /room to open it`, columns),
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
        label: truncate(next.label, columns - 3),
        hotkey: BAND_KEYS.primary,
        primary: true,
        action: run(next),
      },
    ],
  };
}
