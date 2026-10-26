---
name: room-handoff
description: Hand a planned code change from Planroom to the implementer agent, then review, merge or revert it. Load it when a plan's next actions are ready to build, or when a hand-off reports back.
user-invocable: false
---

# Hand-offs

Load the `planroom` skill first for how to call Planroom. Code changes go through a hand-off, never through edits in chat.

## The loop

1. **Spec.** A hand-off's spec names the items, the repos it touches, the checks to run and the rules that must hold. Read it with `diff --handoff KEY`.
2. **Start.** Only the person starts it, with a press in the pane or on the canvas. Planroom makes a separate clone per repo and the mod runs `planroom:implementer` in them.
3. **Report and collect.** When the implementer finishes, the mod runs `handoff report` and `handoff collect`, which bring the branch back as a diff on the map and deal one review card.
4. **Review.** Show the person the diff: whether the spec's rules hold, the hunks, any command the guard denied. Ask for changes through the card, never by editing the clone yourself.
5. **Merge or revert.** Only the person merges (`handoff merge`) or reverts (`handoff revert`); each takes a deliberate second key. A cancel keeps the work on an archived branch.

## Your part

- Use `room_handoff` for `handoff report`, `handoff collect` and `share`; you can't start, merge, revert or cancel.
- While a hand-off runs, don't edit the files it touches. The guard asks the person if you try.
- If a hand-off fails, report its error's `hint` to the person and stop.
