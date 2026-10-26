/**
 * The engine's state from the resolver's replies, and the one tool-result shape (DR-58, DR-66).
 * Pure: the hooks in register.tsx run the processes and write the atoms.
 */

import type { EngineState } from "./state-types";

/** What `$.process.run` resolves with, as far as the mod reads it. */
export interface RunResult {
  exitCode: number;
  stdout: string;
  stderr: string;
}

/** A `--json` envelope, as far as the mod reads it. */
export interface Reply {
  ok: boolean;
  command?: string;
  result?: unknown;
  error?: ReplyError;
  meta?: Record<string, unknown>;
}

export interface ReplyError {
  code: string;
  category?: string;
  retry?: string;
  retry_after_ms?: number;
  message: string;
  hint?: string | null;
  details?: unknown;
}

export const INITIAL_ENGINE: EngineState = {
  status: "resolving",
  resolver: null,
  path: null,
  version: null,
  catalogue_hash: null,
  backend: null,
  line: null,
  fix: null,
};

/** The one JSON document a `--json` command printed, or null when it printed something else. */
export function parseReply(stdout: string): Reply | null {
  try {
    const value: unknown = JSON.parse(stdout.trim());
    if (
      typeof value === "object" &&
      value !== null &&
      typeof (value as Reply).ok === "boolean"
    ) {
      return value as Reply;
    }
  } catch {
    // Not JSON: the caller reports the process's own output.
  }
  return null;
}

function firstLine(text: string): string {
  return text.trim().split("\n")[0] ?? "";
}

/** A native Windows plugin root (`C:\...`): Planroom runs there only under WSL 2. */
export function isWindowsRoot(root: string): boolean {
  return /^[A-Za-z]:[\\/]/.test(root) || root.includes("\\");
}

export function unsupportedPlatform(): EngineState {
  return {
    ...INITIAL_ENGINE,
    status: "unsupported_platform",
    line: "Planroom v0.1 runs on macOS, Linux and WSL 2. Open this repo from WSL.",
  };
}

/** The engine's state after `scripts/planroom version --json`. */
export function engineFromVersion(
  resolver: string,
  run: RunResult,
): EngineState {
  const base = { ...INITIAL_ENGINE, resolver };
  const reply = parseReply(run.stdout);
  if (reply === null) {
    const said =
      firstLine(run.stderr) || firstLine(run.stdout) || `exit ${run.exitCode}`;
    return {
      ...base,
      status: "failed",
      line: `The Planroom engine didn't answer version --json: ${said}`,
    };
  }
  if (reply.ok) {
    const info = (reply.result ?? {}) as {
      version?: string;
      catalogue_hash?: string;
    };
    return {
      ...base,
      status: "ready",
      version: info.version ?? null,
      catalogue_hash: info.catalogue_hash ?? null,
    };
  }
  const error = reply.error ?? {
    code: "internal",
    message: "The engine failed without an error.",
  };
  const details = (error.details ?? {}) as { reason?: string; fix?: string };
  const fix = details.fix ?? error.hint ?? null;
  if (error.code === "engine_fetching") {
    return { ...base, status: "fetching", line: error.message };
  }
  if (error.code === "engine_unavailable") {
    switch (details.reason) {
      case "stale_build":
        return {
          ...base,
          status: "version_mismatch",
          line: `Rebuild the engine: ${fix ?? ""}`.trim(),
          fix,
        };
      case "no_release":
      case "no_build":
      case "download_failed":
        return { ...base, status: "build_offered", line: error.message, fix };
      default:
        return { ...base, status: "failed", line: error.message, fix };
    }
  }
  return { ...base, status: "failed", line: error.message, fix };
}

/** The binary path from `scripts/planroom status --json`, or null. */
export function pathFromStatus(run: RunResult): string | null {
  try {
    const value = JSON.parse(run.stdout.trim()) as {
      ok?: boolean;
      path?: unknown;
    };
    return value.ok === true && typeof value.path === "string"
      ? value.path
      : null;
  } catch {
    return null;
  }
}

/** What a mod tool returns, for success and failure alike (contracts.md, DR-66). */
export interface ToolResult {
  ok: boolean;
  result?: unknown;
  error?: ReplyError;
  meta: Record<string, unknown>;
}

const AGENT_META = [
  "seq",
  "next",
  "truncated",
  "warnings",
  "model_requests",
] as const;

function isEmpty(value: unknown): boolean {
  return (
    value === null ||
    value === undefined ||
    value === false ||
    (Array.isArray(value) && value.length === 0)
  );
}

/** The tool result for one command's run: the envelope less what an agent doesn't act on. */
export function toolResult(run: RunResult): ToolResult {
  const reply = parseReply(run.stdout);
  if (reply === null) {
    return {
      ok: false,
      error: {
        code: "internal",
        retry: "never",
        message:
          firstLine(run.stderr) ||
          `The engine exited ${run.exitCode} without a reply.`,
      },
      meta: {},
    };
  }
  const meta: Record<string, unknown> = {};
  for (const key of AGENT_META) {
    const value = reply.meta?.[key];
    if (key === "seq" ? value !== undefined : !isEmpty(value)) {
      meta[key] = value;
    }
  }
  return reply.ok
    ? { ok: true, result: reply.result, meta }
    : { ok: false, error: reply.error, meta };
}

/** The tool result when there is no engine to run: the engine's own state as the error. */
export function engineMissing(engine: EngineState): ToolResult {
  return {
    ok: false,
    error: {
      code:
        engine.status === "fetching" ? "engine_fetching" : "engine_unavailable",
      retry: engine.status === "fetching" ? "later" : "never",
      message: engine.line ?? "The Planroom engine isn't ready yet.",
      hint: engine.fix,
    },
    meta: {},
  };
}
