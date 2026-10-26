import { expect, test } from "vitest";

import {
  engineFromVersion,
  engineMissing,
  isWindowsRoot,
  pathFromStatus,
  toolResult,
} from "./engine";

const R = "/p/scripts/planroom";

function unavailable(
  reason: string,
  message = "No engine.",
  fix = `${R} build`,
) {
  return {
    exitCode: 7,
    stderr: "",
    stdout: JSON.stringify({
      schema_version: 1,
      ok: false,
      command: "version",
      error: {
        code: "engine_unavailable",
        category: "unavailable",
        retry: "never",
        message,
        hint: fix,
        details: { reason, fix },
      },
      meta: { seq: 0, took_ms: 0, size: { bytes: 1, tokens_est: 1 } },
    }),
  };
}

test("a version reply makes the engine ready", () => {
  const run = {
    exitCode: 0,
    stderr: "",
    stdout:
      '{"ok":true,"command":"version","result":{"version":"0.1.0","catalogue_hash":"ab12"},"meta":{"seq":0}}\n',
  };
  expect(engineFromVersion(R, run)).toMatchObject({
    status: "ready",
    resolver: R,
    version: "0.1.0",
    catalogue_hash: "ab12",
  });
});

test("the resolver's reasons pick the fallback pane", () => {
  expect(engineFromVersion(R, unavailable("no_release"))).toMatchObject({
    status: "build_offered",
    fix: `${R} build`,
  });
  expect(engineFromVersion(R, unavailable("download_failed"))).toMatchObject({
    status: "build_offered",
  });
  expect(
    engineFromVersion(
      R,
      unavailable("stale_build", "Stale.", "cd /r && cargo build -p planroom"),
    ),
  ).toMatchObject({
    status: "version_mismatch",
    line: "Rebuild the engine: cd /r && cargo build -p planroom",
  });
  expect(
    engineFromVersion(R, unavailable("no_toolchain", "Needs cargo.")),
  ).toMatchObject({ status: "failed", line: "Needs cargo." });
  expect(
    engineFromVersion(R, unavailable("override_missing", "No engine there.")),
  ).toMatchObject({ status: "failed", line: "No engine there." });
  // A newer engine's reason, outside the generated union, still reads as a failure.
  expect(
    engineFromVersion(R, unavailable("from_the_future", "Newer.")),
  ).toMatchObject({ status: "failed", line: "Newer." });
  const fetching = unavailable("x", "Another session is fetching.");
  fetching.stdout = fetching.stdout.replace(
    "engine_unavailable",
    "engine_fetching",
  );
  expect(engineFromVersion(R, fetching)).toMatchObject({
    status: "fetching",
    line: "Another session is fetching.",
  });
});

test("an engine that doesn't speak version --json fails with what it said", () => {
  const run = {
    exitCode: 2,
    stdout: "",
    stderr: "error: unrecognized subcommand 'version'\n\nUsage: planroom",
  };
  expect(engineFromVersion(R, run)).toMatchObject({
    status: "failed",
    line: "The Planroom engine didn't answer version --json: error: unrecognized subcommand 'version'",
  });
});

test("status --json gives the binary path", () => {
  expect(
    pathFromStatus({
      exitCode: 0,
      stderr: "",
      stdout:
        '{"ok":true,"step":"checkout","path":"/r/target/debug/planroom","version":"0.1.0"}',
    }),
  ).toBe("/r/target/debug/planroom");
  expect(
    pathFromStatus({
      exitCode: 1,
      stderr: "",
      stdout: '{"ok":false,"step":"download"}',
    }),
  ).toBeNull();
  expect(
    pathFromStatus({ exitCode: 1, stderr: "", stdout: "nope" }),
  ).toBeNull();
});

test("windows_shows_one_line: a native Windows root is unsupported", () => {
  expect(isWindowsRoot("C:\\Users\\me\\plugin")).toBe(true);
  expect(isWindowsRoot("/home/me/plugin")).toBe(false);
});

test("error_envelopes_reach_the_model: a tool result keeps the whole error and the meta an agent acts on", () => {
  const ok = toolResult({
    exitCode: 0,
    stderr: "",
    stdout: JSON.stringify({
      ok: true,
      command: "map",
      result: { nodes: [] },
      meta: {
        seq: 4,
        took_ms: 3,
        size: {},
        next: null,
        truncated: false,
        warnings: [],
      },
    }),
  });
  expect(ok).toEqual({ ok: true, result: { nodes: [] }, meta: { seq: 4 } });
  const failed = toolResult(unavailable("no_release"));
  expect(failed.ok).toBe(false);
  expect(failed.error).toMatchObject({
    code: "engine_unavailable",
    retry: "never",
    details: { reason: "no_release" },
  });
  expect(failed.meta).toEqual({ seq: 0 });
  expect(
    toolResult({
      exitCode: 101,
      stdout: "",
      stderr: "thread 'main' panicked\nmore",
    }),
  ).toEqual({
    ok: false,
    error: {
      code: "internal",
      retry: "never",
      message: "thread 'main' panicked",
    },
    meta: {},
  });
});

test("with no engine a tool says why, as an engine error", () => {
  const result = engineMissing({
    status: "build_offered",
    resolver: R,
    path: null,
    version: null,
    catalogue_hash: null,
    backend: null,
    line: "No release.",
    fix: `${R} build`,
  });
  expect(result).toEqual({
    ok: false,
    error: {
      code: "engine_unavailable",
      retry: "never",
      message: "No release.",
      hint: `${R} build`,
    },
    meta: {},
  });
});
