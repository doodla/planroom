import { expect, test } from "vitest";

import {
  bashDenied,
  guardCall,
  guardPlaces,
  namedPaths,
  normalize,
  planroomLines,
  shellWords,
} from "./guard";

const places = guardPlaces("/work", { HOME: "/home/u" });

function verdict(
  tool: string,
  input: unknown,
  links: Record<string, string> = {},
) {
  return guardCall(
    tool,
    input,
    (p) => (links[p] === undefined ? [] : [links[p]]),
    places,
  );
}

test("the places follow the environment's overrides", () => {
  expect(places).toEqual({
    cwd: "/work",
    home: "/home/u",
    claudeConfig: "/home/u/.claude",
    dataRoot: "/home/u/.local/share/planroom",
    userConfig: "/home/u/.config/planroom/config.toml",
  });
  expect(
    guardPlaces("/w", {
      HOME: "/h",
      CLAUDE_CONFIG_DIR: "/c",
      PLANROOM_DATA: "/d",
      XDG_CONFIG_HOME: "/x",
    }),
  ).toMatchObject({
    claudeConfig: "/c",
    dataRoot: "/d",
    userConfig: "/x/planroom/config.toml",
  });
});

test("paths are made absolute and folded before they're matched", () => {
  expect(normalize("a/../.planroom/./store.db", "/work", "/h")).toBe(
    "/work/.planroom/store.db",
  );
  expect(normalize("~/.claude/x", "/work", "/h")).toBe("/h/.claude/x");
  expect(normalize("/a/b/../../../c", "/work", "/h")).toBe("/c");
});

test("the_mod_store_file_is_guarded: agents can't read the person key or the stores", () => {
  for (const file_path of [
    "/home/u/.claude/plugin-data/planroom/store.json",
    "~/.claude/planroom.json",
    ".planroom/store.db",
    "repo/.git/planroom/store.db-wal",
    ".planroom/serve.json",
    "/home/u/.local/share/planroom/ledger.db",
    "/home/u/.local/share/planroom/backups/ws1/events.ndjson",
    "/home/u/.local/share/planroom/open/abc.html",
    "/home/u/.config/planroom/config.toml",
  ]) {
    expect(verdict("Read", { file_path })?.decision, file_path).toBe("deny");
  }
  // A link elsewhere that lands on the store is the store.
  expect(
    verdict(
      "Read",
      { file_path: "notes.db" },
      { "notes.db": "/work/.planroom/store.db" },
    )?.decision,
  ).toBe("deny");
  // The plugin's own files and the engine cache stay readable.
  for (const file_path of [
    "/work/README.md",
    "/home/u/.claude/plugins/cache/planroom/scripts/planroom",
    "/home/u/.local/share/planroom/engine/0.1.0+ab12cd34/planroom",
  ]) {
    expect(verdict("Read", { file_path }), file_path).toBeNull();
  }
  expect(verdict("Grep", { pattern: "x", path: ".planroom" })?.decision).toBe(
    "deny",
  );
  expect(verdict("Glob", { pattern: "**/store.db" })?.decision).toBe("deny");
  expect(verdict("Grep", { pattern: "x", path: "src" })).toBeNull();
});

test("agents write nothing under a state directory or the data root", () => {
  expect(
    verdict("Write", { file_path: ".planroom/config.toml" })?.decision,
  ).toBe("deny");
  expect(verdict("Edit", { file_path: "/r/.git/planroom/x" })?.decision).toBe(
    "deny",
  );
  expect(
    verdict("Write", {
      file_path: "/home/u/.local/share/planroom/handoffs/w/K/spec.json",
    })?.decision,
  ).toBe("deny");
  expect(
    verdict("NotebookEdit", { notebook_path: ".planroom/a.ipynb" })?.decision,
  ).toBe("deny");
  expect(verdict("Write", { file_path: "src/a.ts" })).toBeNull();
  // A link inside the tree that lands in the state directory is refused too.
  expect(
    verdict(
      "Write",
      { file_path: "cfg" },
      { cfg: "/work/.planroom/config.toml" },
    )?.decision,
  ).toBe("deny");
});

test("person-only commands are refused through Bash, even wrapped", () => {
  for (const command of [
    "planroom decide card_1 accept",
    "/plugins/planroom/scripts/planroom --json handoff merge K1",
    "${CLAUDE_PLUGIN_ROOT}/scripts/planroom intent version v1",
    "script -q -c 'planroom open' /dev/null",
    'bash -c "planroom --workspace /w person init < key"',
    "planroom pack add ./p && echo ok",
    "planroom doctor --restore b.db",
  ]) {
    expect(bashDenied(command), command).not.toBeNull();
  }
  for (const command of [
    "planroom map --level container --json",
    "planroom decide --json",
    "planroom decide --run --base 4 --json",
    "planroom focus --json",
    "ls -la",
  ]) {
    expect(bashDenied(command), command).toBeNull();
  }
});

test("agents never set the token, raw output or the engine's source, nor push Planroom's refs", () => {
  for (const command of [
    "PLANROOM_PERSON_TOKEN=x planroom map",
    "export PLANROOM_RAW=1",
    "env PLANROOM_ENGINE=/tmp/p planroom map",
    "PLANROOM_RELEASE_BASE_URL=http://127.0.0.1:1 planroom version",
    "git push origin refs/planroom/archive/K1",
  ]) {
    expect(bashDenied(command), command).not.toBeNull();
  }
  expect(bashDenied("git push origin main")).toBeNull();
  expect(bashDenied("echo PLANROOM_ACTOR is set")).toBeNull();
});

test("Bash that reads a guarded file by any spelling is refused", () => {
  expect(
    verdict("Bash", { command: "sqlite3 .planroom/store.db .dump" })?.decision,
  ).toBe("deny");
  expect(
    verdict("Bash", { command: "cat ~/.config/planroom/config.toml" })
      ?.decision,
  ).toBe("deny");
  expect(verdict("Bash", { command: "cat data" }, {})).toBeNull();
  expect(
    verdict(
      "Bash",
      { command: "cat ./x/y" },
      { "./x/y": "/home/u/.claude/planroom-store.json" },
    )?.decision,
  ).toBe("deny");
  expect(namedPaths("Bash", { command: "cp a/b ~/c store.db" })).toEqual([
    "a/b",
    "~/c",
    "store.db",
  ]);
});

test("shell words drop quotes and operators; planroom lines skip its own options", () => {
  expect(shellWords(`a 'b c'; d|e && "f"`)).toEqual([
    "a",
    "b",
    "c",
    "d",
    "e",
    "f",
  ]);
  expect(planroomLines(["x", "planroom", "--workspace", "/w", "open"])).toEqual(
    [["--workspace", "open"]],
  );
});
