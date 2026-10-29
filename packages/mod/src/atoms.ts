/**
 * The mod's `$.state` atoms (contracts.md, "Mod state"), declared in the generated contract
 * `plugin/types/index.d.ts`. They survive a reload; the module's own variables don't.
 */

import { atom } from "claude-code";

import { INITIAL_ENGINE } from "./engine";
import type { LastSeq, Outbox, PaneState, TurnSeq } from "./state-types";

export const engineAtom = atom(
  { plugin: "planroom", key: "engine" } as const,
  INITIAL_ENGINE,
  { shape: "engine-1" },
);

const INITIAL_PANE: PaneState = { model: null, error: null };

export const paneAtom = atom(
  { plugin: "planroom", key: "pane" } as const,
  INITIAL_PANE,
  { shape: "pane-1" },
);

const INITIAL_SEQ: LastSeq = { seq: null };

export const lastSeqAtom = atom(
  { plugin: "planroom", key: "lastSeq" } as const,
  INITIAL_SEQ,
  { shape: "seq-1" },
);

const INITIAL_TURN: TurnSeq = { turnId: null, seq: null };

export const turnSeqAtom = atom(
  { plugin: "planroom", key: "turnSeq" } as const,
  INITIAL_TURN,
  { shape: "turn-1" },
);

const INITIAL_OUTBOX: Outbox = [];

/** The writes sent and not yet answered, resent after a reload (DR-8). */
export const outboxAtom = atom(
  { plugin: "planroom", key: "outbox" } as const,
  INITIAL_OUTBOX,
  { shape: "outbox-1" },
);
