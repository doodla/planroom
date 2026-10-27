// The canvas's end-to-end specs, run by `cargo xtask e2e` after it builds the canvas. One worker,
// so the cold-load timings don't compete with another spec for the CPU. Two projects: `specs`,
// which every e2e run runs, and `screens`, the screenshots, which only `cargo xtask e2e --screens`
// runs, so no run skips a test inside a spec. The JSON report is how `cargo xtask e2e` sees a test
// that skipped itself, which fails under CI=true.
import { defineConfig } from "@playwright/test";

export default defineConfig({
  testDir: "./src",
  testMatch: /.*\.spec\.ts$/,
  fullyParallel: false,
  workers: 1,
  retries: 0,
  forbidOnly: true,
  reporter: [
    ["list"],
    ["json", { outputFile: "../../target/e2e/report.json" }],
  ],
  outputDir: "../../target/e2e/results",
  timeout: 60_000,
  use: { browserName: "chromium" },
  projects: [
    { name: "specs", testIgnore: /screens\.spec\.ts$/ },
    { name: "screens", testMatch: /screens\.spec\.ts$/ },
  ],
});
