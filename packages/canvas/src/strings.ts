/**
 * Strings the canvas shows before or without an engine answer.
 *
 * The needs-login line is engine-authored (DR-29) and belongs in `crates/planroom/text/en.toml`,
 * generated into the canvas build like the mod's fallback panes; until the engine generates them,
 * they live here, as the mod's do in `fallback.ts`. The rest name controls for screen readers.
 */
export const STRINGS = {
  needsLogin: "This tab needs a fresh link: press e in the pane",
  unreachable: "Planroom isn't answering at this address.",
  retry: "Try again",
  starting: "Setting out the blocks…",
  levels: "Levels",
  zoomRail: "Level",
  search: "Search blocks",
  openSearch: "Search",
  closeSearch: "Close search",
  ancestors: "Levels above",
  lenses: "Lenses",
  projects: "Projects",
  noProject: "No project",
  decisions: "Decisions",
  diffs: "Diffs",
  sheetPeek: "Show less",
  sheetHalf: "Show more",
  outline: "Blocks on the map",
  map: "Map",
} as const;
