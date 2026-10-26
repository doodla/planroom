# Planroom

Planroom is a shared, living map of your software system that you and Claude plan on together. It maps your repos, services, data stores and the relations between them, keeps your plans and intended rules beside the map, shows what drifted, and hands code changes to an implementer agent for you to review and merge.

## Install

At the prompt of a Claude Code session:

```text
/plugin install planroom --marketplace doodla/planroom
```

The first time Planroom runs, it fetches its engine for your platform, checked against the release's `SHA256SUMS`, or offers to build it from source. It runs on macOS, Linux and WSL 2.

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
