/**
 * `budgets.json` at the repo root, which `vitest.config.js` defines for the vitest files only: the
 * mod's tests gate tool results, `/room` text and the prompt section against it. Product code
 * never reads it, so `room.ts` keeps `ROOM_TEXT_TOKENS` and a test checks the two agree.
 */
declare const __PLANROOM_BUDGETS__: {
  readonly surface: { readonly tools: number; readonly section: number };
  readonly room_text: number;
  readonly tool_results: Readonly<Record<string, number>>;
};
