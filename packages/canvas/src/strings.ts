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
  // The top lid's name when the pane hasn't named the workspace.
  top: "system",
  backTo: (place: string) => `Back to ${place}`,
  // An empty level's line when the engine sends no empty state of its own (ux-3).
  emptyLevel: (level: string) => `Nothing at the ${level} level yet.`,
  lenses: "Lenses",
  projects: "Projects",
  noProject: "No project",
  decisions: "Decisions",
  diffs: "Diffs",
  sheetPeek: "Show less",
  sheetHalf: "Show more",
  outline: "Blocks on the map",
  map: "Map",
  // The Artifact page's request state and its note (DR-56, DR-77); the "Not in this snapshot"
  // line itself comes from the snapshot, which the engine authors.
  asked: "Asked · it arrives at the next sync",
  backToMap: "Back to the map",
  refused: "Your computer couldn't answer this view.",
  snapshotOnly:
    "Snapshot only: this view can't reach live updates or ask your computer.",
  snapshotMissing: "This page's snapshot didn't load.",
} as const;
