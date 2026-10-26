// The canvas's end-to-end specs, run by `cargo xtask e2e` after it builds the canvas. One worker,
// so the cold-load timings don't compete with another spec for the CPU.
import { defineConfig } from "@playwright/test";

export default defineConfig({
  testDir: "./src",
  testMatch: /.*\.spec\.ts$/,
  fullyParallel: false,
  workers: 1,
  retries: 0,
  forbidOnly: true,
  reporter: [["list"]],
  outputDir: "../../target/e2e/results",
  timeout: 60_000,
  use: { browserName: "chromium" },
});
