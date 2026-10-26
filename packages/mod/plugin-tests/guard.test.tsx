/**
 * The guard under `claude plugin test`: `tool.check` as a session raises it for an agent's call,
 * the session's own rules beneath standing as an allow, so every deny here is the mod's.
 */

import { expect, test } from "claude-code/testing";
import type { On } from "claude-code";

import { host, START } from "./host";

/** The session's own permission rules, beneath the mod: they allow what the mod lets through. */
function allowBeneath(on: On): void {
  on("tool.check", () => ({ decision: "allow" as const }));
}

test("the_mod_store_file_is_guarded: no agent reads the person key, the stores or the user config", async ($, on) => {
  const h = host(on, undefined, {
    links: { "/work/notes.db": "/work/.planroom/store.db" },
  });
  allowBeneath(on);
  await $.session.start(START);
  await h.clock.settle();

  for (const file_path of [
    "/home/u/.claude/plugins/store/planroom_planroom-0123456789ab.json",
    ".planroom/store.db",
    "notes.db",
    "/home/u/.local/share/planroom/backups/ws/events.ndjson",
    "/home/u/.config/planroom/config.toml",
  ]) {
    const v = await $.tool.check({ tool: "Read", input: { file_path } });
    expect(v.decision, file_path).toBe("deny");
  }
  const sql = await $.tool.check({
    tool: "Bash",
    input: { command: "sqlite3 .planroom/store.db .dump" },
  });
  expect(sql.decision).toBe("deny");
  const readme = await $.tool.check({
    tool: "Read",
    input: { file_path: "README.md" },
  });
  expect(readme.decision).toBe("allow");
});

test("person-only commands through Bash are denied, with a reason that names the pane", async ($, on) => {
  const h = host(on);
  allowBeneath(on);
  await $.session.start(START);
  await h.clock.settle();

  for (const command of [
    "/plugins/planroom/scripts/planroom decide card_1 accept",
    "script -q -c 'planroom handoff merge K1' /dev/null",
    "planroom person init < key.pub",
    "PLANROOM_PERSON_TOKEN=1.x planroom map",
    "git push origin refs/planroom/archive/K1",
  ]) {
    const v = await $.tool.check({ tool: "Bash", input: { command } });
    expect(v.decision, command).toBe("deny");
  }
  const decide = await $.tool.check({
    tool: "Bash",
    input: { command: "planroom decide card_1 accept" },
  });
  expect(decide.reason).toContain("Planroom pane");
  const map = await $.tool.check({
    tool: "Bash",
    input: { command: "planroom map --level container --json" },
  });
  expect(map.decision).toBe("allow");
});

test("no agent writes under the workspace's Planroom state or the data root", async ($, on) => {
  const h = host(on, undefined, {
    links: { "/work/cfg.toml": "/work/.planroom/config.toml" },
  });
  allowBeneath(on);
  await $.session.start(START);
  await h.clock.settle();

  for (const [tool, input] of [
    ["Write", { file_path: ".planroom/config.toml", content: "" }],
    ["Edit", { file_path: "/work/cfg.toml", old_string: "a", new_string: "b" }],
    [
      "Write",
      {
        file_path: "/home/u/.local/share/planroom/handoffs/w/K/result.json",
        content: "{}",
      },
    ],
  ] as const) {
    const v = await $.tool.check({ tool, input });
    expect(v.decision, `${tool} ${input.file_path}`).toBe("deny");
  }
  const src = await $.tool.check({
    tool: "Write",
    input: { file_path: "src/app.ts", content: "" },
  });
  expect(src.decision).toBe("allow");
  // security-r2-1: Bash writes none of it either, by a command, a redirection or a link.
  for (const command of [
    "cp target/release/planroom ~/.local/share/planroom/engine/0.1.0/planroom",
    "echo '[models]' > .planroom/config.toml",
    "rm -rf .planroom",
    "sed -i s/a/b/ cfg.toml",
    "git clean -fdx",
  ]) {
    const v = await $.tool.check({ tool: "Bash", input: { command } });
    expect(v.decision, command).toBe("deny");
  }
  const build = await $.tool.check({
    tool: "Bash",
    input: { command: "cargo build > /tmp/build.log 2>&1" },
  });
  expect(build.decision).toBe("allow");
});

test("security-r2-2: recursive reads of a guarded place's parent are denied; plain rg passes", async ($, on) => {
  const h = host(on);
  allowBeneath(on);
  await $.session.start(START);
  await h.clock.settle();

  for (const command of [
    "grep -rn api_key ~/.config",
    "rg token ~/.claude",
    "find ~ -path '*plugins/store*' | xargs cat",
    "tar czf /tmp/c.tgz ~/.claude",
    "cp -r .planroom /tmp/p",
  ]) {
    const v = await $.tool.check({ tool: "Bash", input: { command } });
    expect(v.decision, command).toBe("deny");
  }
  const grep = await $.tool.check({
    tool: "Grep",
    input: { pattern: "api_key", path: "/home/u/.config" },
  });
  expect(grep.decision).toBe("deny");
  for (const command of [
    "rg foo",
    "grep -rn foo src/",
    "find . -name '*.ts'",
  ]) {
    const v = await $.tool.check({ tool: "Bash", input: { command } });
    expect(v.decision, command).toBe("allow");
  }
});

test("Planroom's own reads and ghost-making writes need no prompt; scan goes to the session's rules", async ($, on) => {
  const h = host(on);
  on("tool.check", () => ({ decision: "ask" as const }));
  await $.session.start(START);
  await h.clock.settle();

  const query = await $.tool.check({
    tool: "mcp__planroom__room_query",
    input: { command: "map" },
  });
  expect(query.decision).toBe("allow");
  const edit = await $.tool.check({
    tool: "mcp__planroom__room_change",
    input: { command: "edit group" },
  });
  expect(edit.decision).toBe("allow");
  const scan = await $.tool.check({
    tool: "mcp__planroom__room_change",
    input: { command: "scan" },
  });
  expect(scan.decision).toBe("ask");
});

test("security-3: globs, cd, split quoting and an expanded command word are denied through tool.check", async ($, on) => {
  const h = host(on);
  allowBeneath(on);
  await $.session.start(START);
  await h.clock.settle();

  for (const command of [
    "cat ~/.claude/plugins/st*/planroom_*",
    "cd ~/.claude/plugins && cat store/planroom_x.json",
    "cat ~/.config/planroom/config.tom?",
    "cat /home/u/.local/share/planroom/op*/*.html",
    "sqlite3 .planroom/stor?.db .dump",
    'env PLANROOM_PERSON"_TOKEN"=1 planroom map',
    "p=planroom; $p person init",
    "cat /proc/*/environ",
  ]) {
    const v = await $.tool.check({ tool: "Bash", input: { command } });
    expect(v.decision, command).toBe("deny");
  }
  for (const command of ["ls src/*.ts", "cd packages && pnpm test"]) {
    const v = await $.tool.check({ tool: "Bash", input: { command } });
    expect(v.decision, command).toBe("allow");
  }
});
