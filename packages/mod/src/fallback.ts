/**
 * What the pane and the band show before the engine answers `focus` (DR-57): one headline, one
 * detail line, and one action that runs the resolver or retries the mod's start-up.
 *
 * The strings come from the engine's `en.toml`, generated into `FALLBACK_PANES`; this module only
 * picks the pane and what its action runs.
 */

import { FALLBACK_PANES } from "@planroom/contract";

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

type PaneState = keyof typeof FALLBACK_PANES;

/** The pane for `state`: its strings, a detail from the resolver when it has one, and its action. */
function pane(
  state: PaneState,
  detail: string | null,
  run: FallbackAction["run"] = "retry",
): Fallback {
  const strings = FALLBACK_PANES[state];
  return {
    status: state,
    headline: strings.headline,
    detail: detail ?? strings.detail,
    action: strings.action === null ? null : { label: strings.action, run },
  };
}

/** The fallback for an engine that isn't ready, or a `focus` that failed (`error`). */
export function fallbackFor(
  engine: EngineState,
  focusError: string | null = null,
): Fallback {
  switch (engine.status) {
    case "resolving":
      return pane("resolving", null);
    case "fetching":
      return pane("fetching", engine.line);
    case "build_offered":
      return pane("build_offered", engine.line, "build");
    case "building":
      return pane("building", engine.line);
    case "version_mismatch":
      return pane("version_mismatch", engine.line);
    case "person_key_mismatch":
      // A pane Button only, never the band's, so a stray digit can't replace a key (DR-75).
      return pane("person_key_mismatch", engine.line, "rekey");
    case "unsupported_platform":
      return pane("unsupported_platform", engine.line);
    case "failed":
      return pane("failed", engine.line);
    case "ready":
      return pane("no_focus", focusError);
  }
}
