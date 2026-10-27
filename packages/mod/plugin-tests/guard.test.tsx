/**
 * The guard under `claude plugin test`: `tool.check` as a session raises it for an agent's call,
 * the session's own rules beneath standing as an allow, so every deny here is the mod's.
 */

import { expect, test } from "claude-code/testing";
import type { On } from "claude-code";

import { host, isResolver, START } from "./host";

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

test("security-r3-1: a cd in one call and a relative path in the next are denied, since the guard reads the shell's directory per call", async ($, on) => {
  const h = host(on);
  allowBeneath(on);
  await $.session.start(START);
  await h.clock.settle();
  const bash = (command: string) =>
    $.tool.check({ tool: "Bash", input: { command } });

  // The reviewer's sequences: the host keeps each `cd`, so the next call runs where it went.
  h.shell.cwd = "/work/.planroom";
  for (const command of [
    "cat serve.json",
    "rm -rf ./*",
    "sqlite3 store.db .dump",
  ]) {
    expect((await bash(command)).decision, command).toBe("deny");
  }
  const read = await $.tool.check({
    tool: "Read",
    input: { file_path: "serve.json" },
  });
  expect(read.decision).toBe("deny");
  h.shell.cwd = "/home/u/.claude/plugins";
  expect((await bash("cat store/planroom_x.json")).decision).toBe("deny");
  h.shell.cwd = "/home/u/.local/share/planroom";
  for (const command of [
    "cp evil engine/0.1.0/planroom",
    "cat open/abc.html",
  ]) {
    expect((await bash(command)).decision, command).toBe("deny");
  }
  h.shell.cwd = "/home/u/.config/planroom";
  expect((await bash("cat config.toml")).decision).toBe("deny");

  // A cd into a guarded place is refused itself, in case a host doesn't report where it went.
  h.shell.cwd = "/work";
  for (const command of [
    "cd .planroom",
    "pushd ~/.claude/plugins/store",
    "cd ~/.local/share/planroom",
  ]) {
    expect((await bash(command)).decision, command).toBe("deny");
  }
  // From a folder inside the workspace, its state directory is still held, and work goes on.
  h.shell.cwd = "/work/src";
  expect((await bash("rm -rf ..")).decision).toBe("deny");
  for (const command of [
    "cd lib",
    "cat main.rs",
    "rm -rf ./build",
    "cd .. && ls",
  ]) {
    expect((await bash(command)).decision, command).toBe("allow");
  }
});

test("security-r3-2: Artifact, ArtifactData, SendUserFile, LS and unknown tools are held to the read rule; S11's own calls pass", async ($, on) => {
  const h = host(on);
  allowBeneath(on);
  await $.session.start(START);
  await h.clock.settle();
  const store =
    "/home/u/.claude/plugins/store/planroom_planroom-0123456789ab.json";

  for (const [tool, input] of [
    [
      "ArtifactData",
      { action: "update", collection: "c", doc_id: "d", file_path: store },
    ],
    [
      "ArtifactData",
      {
        action: "batch",
        writes: [{ op: "set", collection: "c", doc_id: "d", file_path: store }],
      },
    ],
    [
      "Artifact",
      { action: "publish", asset: true, url: "u", file_path: store },
    ],
    [
      "Artifact",
      {
        action: "publish",
        files: { "k.json": "/home/u/.config/planroom/config.toml" },
      },
    ],
    [
      "SendUserFile",
      { files: ["/work/.planroom/serve.json"], status: "normal" },
    ],
    ["LS", { path: "/home/u/.claude/plugins/store" }],
    ["NotebookRead", { notebook_path: store }],
    ["mcp__other__upload", { attachment: "~/.config/planroom/config.toml" }],
  ] as const) {
    const v = await $.tool.check({ tool, input });
    expect(v.decision, `${tool} ${JSON.stringify(input)}`).toBe("deny");
  }

  // S11: import's batch files and share's bundle, both under the state directory's share/.
  const batch = await $.tool.check({
    tool: "ArtifactData",
    input: {
      action: "batch",
      url: "https://claude.ai/artifact/x",
      writes: [
        {
          op: "set",
          collection: "answers",
          doc_id: "r1",
          file_path: "/work/.planroom/share/sync/412-1/answers/r1.json",
        },
        {
          op: "set",
          collection: "live",
          doc_id: "head",
          file_path: "/work/.planroom/share/sync/412-1/live/head.json",
          if_version: 3,
        },
      ],
    },
  });
  expect(batch.decision).toBe("allow");
  const publish = await $.tool.check({
    tool: "Artifact",
    input: {
      action: "publish",
      file_path: "/work/.planroom/share/page.html",
      root: "/work/.planroom/share",
      files: {
        "assets/index.js": "assets/index.js",
        "data/snapshot.json": "data/snapshot.json",
      },
      icon: "map",
    },
  });
  expect(publish.decision).toBe("allow");
});

/** tests-3-1: with no usable HOME, every call the guard reads is asked, never let through unread. */
const NO_HOME_CALLS = [
  [
    "Read",
    {
      file_path:
        "/home/u/.claude/plugins/store/planroom_planroom-0123456789ab.json",
    },
  ],
  ["Bash", { command: "cat ~/.config/planroom/config.toml" }],
  ["Bash", { command: "ls" }],
] as const;

test("tests-3-1: a session whose HOME is unset has every call asked", async ($, on) => {
  const h = host(on, undefined, { home: null });
  allowBeneath(on);
  await $.session.start(START);
  await h.clock.settle();
  for (const [tool, input] of NO_HOME_CALLS) {
    const v = await $.tool.check({ tool, input });
    expect(v.decision, tool).toBe("ask");
  }
});

test("tests-3-1: a session whose HOME is empty has every call asked", async ($, on) => {
  const h = host(on, undefined, { home: "" });
  allowBeneath(on);
  await $.session.start(START);
  await h.clock.settle();
  for (const [tool, input] of NO_HOME_CALLS) {
    const v = await $.tool.check({ tool, input });
    expect(v.decision, tool).toBe("ask");
  }
});

test("security-r2-4: a call that names more paths than the guard resolves is asked, not let through", async ($, on) => {
  const h = host(on);
  allowBeneath(on);
  await $.session.start(START);
  await h.clock.settle();
  const words = Array.from({ length: 1100 }, (_, i) => `./f${i}`).join(" ");
  const many = await $.tool.check({
    tool: "Bash",
    input: { command: `cat ${words}` },
  });
  expect(many.decision).toBe("ask");
  const few = await $.tool.check({
    tool: "Bash",
    input: { command: "cat ./a ./b" },
  });
  expect(few.decision).toBe("allow");
});

test("security-r3-3: the plugin's own files are never written, by the Write tool or by Bash", async ($, on) => {
  const h = host(on);
  allowBeneath(on);
  await $.session.start(START);
  await h.clock.settle();
  // Wherever the kit loaded the plugin from: the mod runs its resolver from `$.plugin.root`.
  const resolver = h.runs.find((r) => isResolver(r.argv[0]))?.argv[0] ?? "";
  const root = resolver.slice(0, -"/scripts/planroom".length);
  expect(root).not.toBe("");

  const write = await $.tool.check({
    tool: "Write",
    input: { file_path: `${root}/scripts/planroom`, content: "evil" },
  });
  expect(write.decision).toBe("deny");
  for (const command of [
    `echo x > ${root}/scripts/planroom`,
    `cp evil.js ${root}/hooks/planroom-mod.js`,
    `python3 -c 'open(1)' ${root}/hooks/planroom-mod.js`,
    `rm -rf ${root}`,
  ]) {
    const v = await $.tool.check({ tool: "Bash", input: { command } });
    expect(v.decision, command).toBe("deny");
  }
  // The resolver still runs, as the planroom skill has the agent run it, and its files still read.
  const run = await $.tool.check({
    tool: "Bash",
    input: { command: `"${root}/scripts/planroom" map --json` },
  });
  expect(run.decision).toBe("allow");
  const read = await $.tool.check({
    tool: "Read",
    input: { file_path: `${root}/skills/planroom/SKILL.md` },
  });
  expect(read.decision).toBe("allow");
});

test("a call on another attached machine or outside the sandbox is asked, and a start-up file never sets the engine", async ($, on) => {
  const h = host(on);
  allowBeneath(on);
  await $.session.start(START);
  await h.clock.settle();

  for (const [tool, input] of [
    ["Bash", { command: "ls", _host: "laptop" }],
    ["Read", { file_path: "README.md", _host: "laptop" }],
    ["Bash", { command: "ls", dangerouslyDisableSandbox: true }],
  ] as const) {
    const v = await $.tool.check({ tool, input });
    expect(v.decision, `${tool} ${JSON.stringify(input)}`).toBe("ask");
  }
  for (const [tool, input] of [
    [
      "Write",
      {
        file_path: "/home/u/.bashrc",
        content: "export PLANROOM_ENGINE=/tmp/x\n",
      },
    ],
    [
      "Edit",
      {
        file_path: ".claude/settings.json",
        old_string: "{",
        new_string:
          '{"env":{"PLANROOM_RELEASE_BASE_URL":"http://127.0.0.1:1"},',
      },
    ],
  ] as const) {
    const v = await $.tool.check({ tool, input });
    expect(v.decision, tool).toBe("deny");
  }
  // The everyday forms the usability review found refused now pass.
  for (const command of [
    "git clean -ndx",
    "rm -rf **/__pycache__",
    "sed -i.bak 's/^version = .*/version = \"0.2.0\"/' Cargo.toml",
    "find . -empty -type d -delete",
    "sqlite3 store.db '.schema'",
  ]) {
    const v = await $.tool.check({ tool: "Bash", input: { command } });
    expect(v.decision, command).toBe("allow");
  }
  const glob = await $.tool.check({
    tool: "Glob",
    input: { pattern: "**/*.json" },
  });
  expect(glob.decision).toBe("allow");
});

test("a linked worktree's state is held where its .git file points, read as git reads it", async ($, on) => {
  // The workspace is a linked worktree of /src/app: its `.git` is a file naming its git directory
  // relative to the worktree, and its state directory is `planroom/` in there (DR-74).
  const h = host(on, undefined, {
    files: { "/work/.git": "gitdir: ../src/app/.git/worktrees/work\n" },
  });
  allowBeneath(on);
  await $.session.start(START);
  await h.clock.settle();
  const bash = (command: string) =>
    $.tool.check({ tool: "Bash", input: { command } });

  for (const command of [
    "rm -rf /src/app/.git",
    "rm -rf /src/app/.git/worktrees",
    "find /src/app/.git -name '*.db' -delete",
    "find /src/app -name '*.db' -not -path '*/.planroom/*' -delete",
    "rm -rf /src/app/.git/worktrees/work/planroom",
  ]) {
    expect((await bash(command)).decision, command).toBe("deny");
  }
  const read = await $.tool.check({
    tool: "Read",
    input: { file_path: "/src/app/.git/worktrees/work/planroom/store.db" },
  });
  expect(read.decision).toBe("deny");
  // The main repo's other git work goes on.
  for (const command of [
    "cat /src/app/.git/worktrees/work/HEAD",
    "git worktree list",
    "rm -rf /src/app/build",
  ]) {
    expect((await bash(command)).decision, command).toBe("allow");
  }
});

test("without a readable .git file, a worktree's state is still held by its path", async ($, on) => {
  const h = host(on);
  allowBeneath(on);
  await $.session.start(START);
  await h.clock.settle();
  const bash = (command: string) =>
    $.tool.check({ tool: "Bash", input: { command } });

  for (const command of [
    "rm -rf /src/app/.git/worktrees/work/planroom",
    "cat /src/app/.git/worktrees/work/planroom/store.db",
  ]) {
    expect((await bash(command)).decision, command).toBe("deny");
  }
  // Another repo's git directory above it is that repo's own, as another repo's `.git` is.
  expect((await bash("rm -rf /src/app/.git/worktrees")).decision).toBe("allow");
});
