/**
 * The engine's state from the resolver's replies, and the one tool-result shape (DR-58, DR-66).
 * Pure: the hooks in register.tsx run the processes and write the atoms.
 */

import type {
  Envelope,
  ErrorObject,
  ResolverStatus,
  ToolError,
  ToolMeta,
  ToolResult,
  VersionInfo,
} from "@planroom/contract";

import type { EngineState } from "./state-types";

/** What `$.process.run` resolves with, as far as the mod reads it. */
export interface RunResult {
  exitCode: number;
  stdout: string;
  stderr: string;
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

/**
 * The one envelope a `--json` command printed, or null when it printed something else. It's
 * checked only as far as `ok`, so the readers below still guard what they take from it.
 */
export function parseReply(stdout: string): Envelope | null {
  try {
    const value: unknown = JSON.parse(stdout.trim());
    if (
      typeof value === "object" &&
      value !== null &&
      typeof (value as Partial<Envelope>).ok === "boolean"
    ) {
      return value as Envelope;
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
    const info = reply.result as Partial<VersionInfo> | null | undefined;
    return {
      ...base,
      status: "ready",
      version: info?.version ?? null,
      catalogue_hash: info?.catalogue_hash ?? null,
    };
  }
  const error = reply.error ?? NO_ERROR;
  // engine_unavailable's details, as the error registry documents them (contracts.md).
  const details = error.details as
    { reason?: string; fix?: string } | null | undefined;
  const fix = details?.fix ?? error.hint ?? null;
  if (error.code === "engine_fetching") {
    return { ...base, status: "fetching", line: error.message };
  }
  if (error.code === "engine_unavailable") {
    switch (details?.reason) {
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
    const value = JSON.parse(
      run.stdout.trim(),
    ) as Partial<ResolverStatus> | null;
    return value?.ok === true && typeof value.path === "string"
      ? value.path
      : null;
  } catch {
    return null;
  }
}

/** What a mod tool returns, for success and failure alike: the contract's type (contracts.md, DR-66). */
export type { ToolResult };

/** The error of an envelope that carried none: the engine failed without saying why. */
const NO_ERROR: ErrorObject = {
  code: "internal",
  category: "internal",
  retry: "never",
  message: "The engine failed without an error.",
  hint: null,
  details: {},
};

/**
 * The envelope's error as a tool result carries it: whole (DR-66). A field an older engine left
 * out stays out, and a missing retry class reads `never`.
 */
function toolError(error: ErrorObject): ToolError {
  const sent = error as Partial<ErrorObject>;
  const out: ToolError = {
    code: error.code,
    retry: sent.retry ?? "never",
    message: error.message,
  };
  if (sent.category !== undefined) {
    out.category = sent.category;
  }
  if (error.retry_after_ms !== undefined && error.retry_after_ms !== null) {
    out.retry_after_ms = error.retry_after_ms;
  }
  if (error.hint !== undefined && error.hint !== null) {
    out.hint = error.hint;
  }
  if (error.details !== undefined) {
    out.details = error.details;
  }
  return out;
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
  const agentMeta = meta as ToolMeta;
  if (reply.ok) {
    return { ok: true, result: reply.result, meta: agentMeta };
  }
  return {
    ok: false,
    error: toolError(reply.error ?? NO_ERROR),
    meta: agentMeta,
  };
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
