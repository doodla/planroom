/**
 * The mod's `$.state` atoms (contracts.md, "Mod state"), declared in the generated contract
 * `plugin/types/index.d.ts`. They survive a reload; the module's own variables don't.
 */

import { atom } from "claude-code";

import { INITIAL_ENGINE } from "./engine";
import type { LastSeq, PaneState } from "./state-types";

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
