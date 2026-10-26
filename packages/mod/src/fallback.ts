/**
 * What the pane and the band show before the engine answers `focus` (DR-57): one headline, one
 * detail line, and one action that runs the resolver or retries the mod's start-up.
 *
 * These strings belong in the engine's `en.toml`, generated into `FALLBACK_PANES`; until the engine
 * has a string catalog, they live here.
 */

import type { EngineState } from "./state-types";

/** What a fallback's one action does. */
export type FallbackAction = {
  label: string;
  run: "retry" | "build" | "rekey";
};

export interface Fallback {
  status: EngineState["status"] | "no_focus";
  headline: string;
  detail: string | null;
  action: FallbackAction | null;
}

const RETRY: FallbackAction = { label: "Check again", run: "retry" };

/** The fallback for an engine that isn't ready, or a `focus` that failed (`error`). */
export function fallbackFor(
  engine: EngineState,
  focusError: string | null = null,
): Fallback {
  switch (engine.status) {
    case "resolving":
      return {
        status: "resolving",
        headline: "Looking for the Planroom engine…",
        detail: null,
        action: RETRY,
      };
    case "fetching":
      return {
        status: "fetching",
        headline: "Getting the Planroom engine…",
        detail: engine.line,
        action: RETRY,
      };
    case "build_offered":
      return {
        status: "build_offered",
        headline: "Build the engine from source, about 4 minutes?",
        detail: engine.line,
        action: { label: "Build it", run: "build" },
      };
    case "building":
      return {
        status: "building",
        headline: "Building the engine from source…",
        detail: engine.line ?? "This takes about 4 minutes.",
        action: RETRY,
      };
    case "version_mismatch":
      return {
        status: "version_mismatch",
        headline: "This engine doesn't match the plugin.",
        detail: engine.line,
        action: { label: "Try again", run: "retry" },
      };
    case "person_key_mismatch":
      // A pane Button only, never the band's, so a stray digit can't replace a key (DR-75).
      return {
        status: "person_key_mismatch",
        headline: "This workspace knows another person key.",
        detail:
          engine.line ??
          "Presses from this session can't act as you until you use its key.",
        action: { label: "Use this session's key", run: "rekey" },
      };
    case "unsupported_platform":
      return {
        status: "unsupported_platform",
        headline: "Planroom can't run here.",
        detail: engine.line,
        action: null,
      };
    case "failed":
      return {
        status: "failed",
        headline: "The Planroom engine isn't available.",
        detail: engine.line,
        action: { label: "Try again", run: "retry" },
      };
    case "ready":
      return {
        status: "no_focus",
        headline: "Planroom couldn't show this folder.",
        detail: focusError,
        action: { label: "Try again", run: "retry" },
      };
  }
}
