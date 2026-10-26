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
    "/home/u/.claude/plugins/store/planroom_planroom-0123456789ab.json",
    "~/.claude/plugins/store/planroom_inline-d70c18432e39.json",
    "/home/u/.claude/plugins/store",
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
  // A search over a folder that holds the mod's store reaches it.
  for (const path of ["/home/u/.claude", "/home/u/.claude/plugins"]) {
    expect(verdict("Grep", { pattern: "secret", path })?.decision, path).toBe(
      "deny",
    );
  }
  // The plugin's own files, other plugins' stores, transcripts and the engine cache stay readable.
  for (const file_path of [
    "/work/README.md",
    "/home/u/.claude/plugins/cache/planroom/scripts/planroom",
    "/home/u/.claude/plugins/store/other_market-0123456789ab.json",
    "/home/u/.claude/projects/-work-planroom/session.jsonl",
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
      {
        "./x/y":
          "/home/u/.claude/plugins/store/planroom_planroom-0123456789ab.json",
      },
    )?.decision,
  ).toBe("deny");
  expect(
    namedPaths("Bash", { command: "cp a/b ~/c store.db" }, places),
  ).toEqual(["a/b", "~/c", "store.db"]);
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

/** The bypasses review round 1 found (security-3): everyday shell forms, each now denied. */
const BYPASSES = [
  // A glob into the mod's store, the user-level config, the login pages and the store.
  "cat ~/.claude/plugins/st*/planroom_*",
  "cat ~/.config/planroom/config.tom?",
  "cat /home/u/.local/share/planroom/op*/*.html",
  "sqlite3 .planroom/stor?.db .dump",
  // `cd`, then a path relative to where it went.
  "cd ~/.claude/plugins && cat store/planroom_x.json",
  "cd ~/.claude/plugins/store; cat planroom_x.json",
  "cd /home/u/.local/share/planroom && cat open/abc.html",
  // A guarded variable's name split by quotes.
  'env PLANROOM_PERSON"_TOKEN"=1 planroom map',
  'env PLANROOM_RELEASE"_BASE_URL"=http://127.0.0.1:1 planroom map',
  "PLANROOM_'RAW'=1 planroom map",
  // A variable in command position before person-only words.
  "p=planroom; $p person init",
  'p=planroom; "$p" decide card_1 accept',
  "${PR} handoff merge K1",
  // Another process's environment or memory, which may hold a person token.
  "cat /proc/*/environ",
  "cat /proc/1234/mem",
  "cat /proc/$PID/environ",
  // $HOME spelled out.
  "cat $HOME/.claude/plugins/store/planroom_x.json",
  "cat ${HOME}/.config/planroom/config.toml",
];

test("security-3: the everyday shell forms that slipped past the guard are denied", () => {
  for (const command of BYPASSES) {
    expect(verdict("Bash", { command })?.decision, command).toBe("deny");
  }
});

test("security-3: a Glob tool's pattern is held to the same rule as a Bash glob", () => {
  expect(
    verdict("Glob", { pattern: "/home/u/.claude/plugins/st*/planroom_*" })
      ?.decision,
  ).toBe("deny");
  expect(
    verdict("Glob", { pattern: "~/.config/planroom/*.toml" })?.decision,
  ).toBe("deny");
  expect(verdict("Glob", { pattern: "packages/*/src/*.ts" })).toBeNull();
});

test("security-3: everyday commands still pass", () => {
  for (const command of [
    "ls src/*.ts",
    "cd packages && pnpm test",
    "cd /work/packages/mod && pnpm exec vitest run src/*.test.ts",
    "git status && git diff -- '*.rs'",
    "rg -n 'fn main' crates/*/src",
    "cat ~/.claude/settings.json",
    "ls ~/.claude/plugins/cache",
    "cat /proc/cpuinfo",
    "echo $PATH && ls *.md",
    "for f in packages/*/package.json; do cat $f; done",
    "planroom map --level container --json",
    "find . -name '*.ts' -not -path './node_modules/*'",
    "cd ~ && ls",
    "x=1; echo $x",
    "cat ~/.claude/plugins/store/other_market-0123456789ab.json",
    "ls ~/.local/share/planroom/engine/*/planroom",
    "cat .planroom/*.toml",
    "cd crates/kernel && cargo test -p planroom-kernel",
  ]) {
    expect(verdict("Bash", { command }), command).toBeNull();
  }
});
