import { defineConfig } from "vitest/config";

export default defineConfig({
  test: {
    // Only the workspace's own tests: examples/fixture holds test files that are scan inputs.
    include: ["packages/*/src/**/*.test.ts", "tests/e2e/src/**/*.test.ts"],
  },
});
