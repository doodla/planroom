---
tags: [skill-creator]
max_turns: 30
timeout_seconds: 900
allowed_tools: [Read, Glob, Grep, Skill, Write, Bash]
---

The Workflow tool run that built feat/pack-loader just finished. Its report says: "Built the pack loader and the code pack on feat/pack-loader (worktree .claude/worktrees/pack-loader). cargo xtask check and test pass in the worktree. Gated surfaces changed: boundaries.toml (packs.requires plants), and crates/planroom/Cargo.toml gains globset. Doc conflicts: contracts.md says PackDecl.tone is required, I made it optional. I didn't touch anything else." A test merge shows conflicts in plugin/hooks/planroom-mod.js, packages/contract/src/generated/catalogue.ts and crates/xtask/src/main.rs (both sides added a usage line). Don't run any git, cargo or pnpm command. Write the exact sequence of steps and commands you'd take to merge this branch into the working branch, verify it and get it pushed and green, with each decision you'd make, to merge-plan.md in the current directory.
