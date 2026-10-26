import { defineConfig } from "vitest/config";

export default defineConfig({
  test: {
    // Only the workspace's own tests: examples/fixture holds test files that are scan inputs.
    include: ["packages/*/src/**/*.test.ts", "tests/e2e/src/**/*.test.ts"],
    // One real `planroom watch --json` run, which packages/mod/src/watch-engine.test.ts reads.
    globalSetup: ["tests/e2e/src/engine-watch.ts"],
  },
});
