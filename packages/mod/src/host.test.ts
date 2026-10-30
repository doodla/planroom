import type { EngineInterface } from "claude-code";
import { MOD_STRINGS } from "@planroom/contract";
import { fill } from "@planroom/ui";
import { expect, test } from "vitest";

import {
  MEMBERS,
  MIN_HOST,
  UNPROBED,
  changedLine,
  checkHost,
  hostLine,
  isMissing,
  missingMembers,
  refusedLine,
  showHostLine,
} from "./host";

/** A host with every member the mod calls, less the ones named in `without`. */
function stub(without: readonly string[] = []): Record<string, unknown> {
  const api: Record<string, Record<string, unknown>> = {};
  for (const member of MEMBERS) {
    if (without.includes(member)) continue;
    const [noun = "", name = ""] = member.split(".");
    const group = (api[noun] ??= {});
    group[name] = member === "plugin.root" ? "/p" : () => undefined;
  }
  return api;
}

test("a_missing_member_registers_nothing_and_shows_one_line: the check names the member and the line names the host", () => {
  expect(missingMembers(stub())).toEqual([]);
  const missing = missingMembers(stub(["ui.invalidate"]));
  expect(missing).toEqual(["ui.invalidate"]);
  expect(hostLine({ version: "2.1.320", base: "2.1.320" }, missing)).toBe(
    fill(MOD_STRINGS.host_changed, { version: "2.1.320" }),
  );

  // A whole noun gone, a throwing getter and a root that is not a string all count as missing.
  expect(missingMembers({})).toEqual([...MEMBERS]);
  const api = stub();
  Object.defineProperty(api, "store", {
    get() {
      throw new Error("gone");
    },
  });
  expect(missingMembers(api)).toEqual(["store.get", "store.set"]);
  const root = stub();
  (root.plugin as Record<string, unknown>).root = () => "/p";
  expect(missingMembers(root)).toEqual(["plugin.root"]);
});

test("an_older_host_falls_back_to_the_skills: the line names both versions", () => {
  const line = hostLine({ version: "2.1.300", base: "2.1.300" }, []);
  expect(line).toBe(
    fill(MOD_STRINGS.host_too_old, { version: "2.1.300", minimum: "2.1.310" }),
  );
  expect(line).toContain("2.1.300");
  expect(line).toContain("2.1.310");
  expect(MIN_HOST).toBe("2.1.310");
  // An older host is reported as old even when it also lacks a member.
  expect(hostLine({ version: "2.0.9", base: "2.0.9" }, ["ui.open"])).toBe(
    fill(MOD_STRINGS.host_too_old, { version: "2.0.9", minimum: "2.1.310" }),
  );
});

test("a_newer_host_with_every_member_runs: only a missing member or an old version gives a line", () => {
  expect(hostLine({ version: "2.1.330", base: "2.1.330" }, [])).toBeNull();
  expect(
    hostLine(
      {
        version: "2.1.310-dev.20261023.t174332.sha1a2b3c4",
        base: "2.1.310-dev",
      },
      [],
    ),
  ).toBeNull();
  expect(hostLine(null, [])).toBeNull();
  expect(hostLine({ version: "nightly" }, [])).toBeNull();
  expect(hostLine(null, ["ui.open"])).toBe(
    fill(MOD_STRINGS.host_changed, {
      version: MOD_STRINGS.host_unknown_version,
    }),
  );
});

/**
 * A `$` with every member the mod calls, less the ones in `without`, each answering as the host's
 * does; `calls` records each member called. A call to a member it lacks throws the host's TypeError.
 */
function host$(
  without: readonly string[] = [],
  calls: string[] = [],
): EngineInterface {
  const answers: Record<string, () => unknown> = {
    "session.version": () =>
      Promise.resolve({ version: "2.1.320", base: "2.1.320" }),
    "clock.after": () => ({ cancel: () => undefined }),
    "ui.invalidate": () => undefined,
    "ui.status": () => undefined,
  };
  const api: Record<string, Record<string, unknown>> = {};
  for (const member of MEMBERS) {
    if (without.includes(member)) continue;
    const [noun = "", name = ""] = member.split(".");
    const group = (api[noun] ??= {});
    group[name] =
      member === "plugin.root"
        ? "/p"
        : () => {
            calls.push(member);
            return (answers[member] ?? (() => Promise.resolve(undefined)))();
          };
  }
  return api as unknown as EngineInterface;
}

test("a_host_without_a_member_fails_the_wired_check: each member a call can see is called and named", async () => {
  expect(await checkHost(host$())).toEqual({
    line: null,
    missing: [],
    version: { version: "2.1.320", base: "2.1.320" },
  });
  const seen = MEMBERS.filter((m) => m !== "plugin.root" && !UNPROBED.has(m));
  for (const member of seen) {
    const host = await checkHost(host$([member]));
    expect(host.missing, member).toEqual([member]);
    const version =
      member === "session.version"
        ? MOD_STRINGS.host_unknown_version
        : "2.1.320";
    expect(host.line, member).toBe(fill(MOD_STRINGS.host_changed, { version }));
  }
  expect(await checkHost(host$(["plugin.root"]))).toEqual({
    line: fill(MOD_STRINGS.host_changed, { version: "2.1.320" }),
    missing: ["plugin.root"],
    version: { version: "2.1.320", base: "2.1.320" },
  });
});

test("the_wired_check_calls_no_member_with_an_effect: what it can't call it takes as present", async () => {
  expect([...UNPROBED].every((m) => MEMBERS.includes(m))).toBe(true);
  const calls: string[] = [];
  await checkHost(host$([], calls));
  expect(calls.filter((m) => UNPROBED.has(m))).toEqual([]);
  // A host without one of them passes the check, and fails at its first use instead, with the
  // line a missing member gets.
  for (const member of UNPROBED) {
    expect(await checkHost(host$([member]))).toEqual({
      line: null,
      missing: [],
      version: { version: "2.1.320", base: "2.1.320" },
    });
    expect(changedLine({ version: "2.1.320", base: "2.1.320" }, member)).toBe(
      fill(MOD_STRINGS.host_changed, { version: "2.1.320" }),
    );
  }
  expect(changedLine(null, "ui.open")).toBe(
    fill(MOD_STRINGS.host_changed, {
      version: MOD_STRINGS.host_unknown_version,
    }),
  );
});

test("a_host_without_a_status_line_fails_the_check_quietly", () => {
  expect(() => showHostLine(host$(["ui.status"]), "line")).not.toThrow();
});

test("a_refusal_is_not_a_missing_member: only a TypeError, thrown or rejected, is a member that isn't there", async () => {
  // A host without the member: the call throws at once, or a promise rejects with one.
  const host = host$(["ui.open"]);
  let thrown: unknown;
  try {
    await host.ui.open({ id: "planroom", title: "Planroom" });
  } catch (err) {
    thrown = err;
  }
  expect(thrown).toBeInstanceOf(TypeError);
  expect(isMissing(thrown)).toBe(true);
  expect(isMissing(new TypeError("$.ui.open is not a function"))).toBe(true);
  // One made in another realm: not this realm's TypeError, but named one.
  const foreign = Object.assign(new Error("x is not a function"), {
    name: "TypeError",
  });
  expect(isMissing(foreign)).toBe(true);
  // A hook's deny and the host's own refusals, as the host rejects with them, are refusals.
  const deny = Object.assign(
    new Error("planroom: $.ui.open: a pane hook said no"),
    { name: "HooksError" },
  );
  expect(isMissing(deny)).toBe(false);
  expect(isMissing(new Error("a built-in is named room"))).toBe(false);
  expect(isMissing("refused")).toBe(false);
  expect(isMissing(undefined)).toBe(false);
});

test("the_refused_line_names_what_and_a_sanitized_reason: once, without the host's prefix", () => {
  const deny = Object.assign(
    new Error("planroom: $.command.register: a built-in is named room"),
    { name: "HooksError" },
  );
  expect(refusedLine("/room", deny)).toBe(
    fill(MOD_STRINGS.host_refused, {
      what: "/room",
      reason: "a built-in is named room",
    }),
  );
  expect(refusedLine("/room", deny)).not.toBe(
    fill(MOD_STRINGS.host_changed, { version: "2.1.310" }),
  );
  // A control or bidi character in the reason shows as a marker; only its first line shows, cut.
  const hostile = refusedLine(
    "room_query",
    new Error(
      "planroom: $.tool.register: no\u001b]52;c;eA==\u0007\u202e\nsecond line",
    ),
  );
  for (const raw of ["\u001b", "\u0007", "\u202e"]) {
    expect(hostile).not.toContain(raw);
  }
  expect(hostile).not.toContain("second line");
  expect(hostile).toContain("room_query");
  const long = refusedLine("pane", new Error("x".repeat(500)));
  expect(long.length).toBeLessThan(MOD_STRINGS.host_refused.length + 220);
});
