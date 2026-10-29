import { readFileSync } from "node:fs";
import { URL } from "node:url";

import { defineConfig } from "vitest/config";

/**
 * The repo's hand-ratcheted budgets, which the mod's vitest files gate tool results, `/room` text
 * and the prompt section against. They reach the tests as a define, as the plugin's skill names
 * reach the `claude plugin test` bundles, so no package imports a file outside its own tree.
 */
const budgets = readFileSync(
  new URL("./budgets.json", import.meta.url),
  "utf8",
);

export default defineConfig({
  define: {
    __PLANROOM_BUDGETS__: budgets,
  },
  test: {
    // Only the workspace's own tests: examples/fixture holds test files that are scan inputs.
    include: ["packages/*/src/**/*.test.ts", "tests/e2e/src/**/*.test.ts"],
    // One real `planroom watch --json` run, which packages/mod/src/watch-engine.test.ts reads.
    globalSetup: ["tests/e2e/src/engine-watch.ts"],
  },
});
