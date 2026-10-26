# Planroom

Planroom is a shared, living map of your software system that you and Claude plan on together. It maps your repos, services, data stores and the relations between them, keeps your plans and intended rules beside the map, shows what drifted, and hands code changes to an implementer agent for you to review and merge.

## Install

At the prompt of a Claude Code session, add Planroom's marketplace, then install the plugin from it:

```text
/plugin marketplace add doodla/planroom
/plugin install planroom@planroom
```

The first time Planroom runs, it needs its engine. Until the first release, it offers to build the engine from source, which takes about 4 minutes and needs Rust and a C compiler. With Node 22 and pnpm installed too, the build includes the canvas. From the first release on, it fetches the engine for your platform instead, checked against the release's `SHA256SUMS`. It runs on macOS, Linux and WSL 2.

## First steps

1. Open a session in a folder that holds one or more repos.
2. Type `/room` to open the pane. It names the next step, such as mapping the repos.
3. Ask Claude about your system, or to plan a change.

`/room help` lists every subcommand.

## Mods

The pane, the band above the prompt and `/room` come from Planroom's mod. Turn mods on when Claude Code asks; with mods off, Claude still reaches Planroom through its skills.

## Links

- Source and issues: <https://github.com/doodla/planroom>
- License: MIT
