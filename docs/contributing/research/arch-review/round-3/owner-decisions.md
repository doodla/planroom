# Architecture review round 3: owner decisions

Round 3 is the last round the review protocol allows. It ran the same nine focuses with fresh reviewers and verifiers: 29 material findings upheld. Apply each upheld material finding as decided below, else by its own proposed fix adapted to the design. Apply minor and downgraded findings when the fix is small and clearly right; list the rest in `known-gaps.md` here. Anything material that can't be closed in the documents goes to `accepted-risk.md` here, one line each with its reason, and is signed off there.

## Writes and data

- **F1. A person's field wins over any automatic write** (correctness-1). A prefill, an auto-enabled effect or an infer output never applies to a field with a person-authored revision that no undo reversed, whatever its base. The decision is recorded as `Superseded` and listed in Diffs, with no card. The runner skips subjects whose effect fields a person authored.
- **F2. Wrapper calls bind their arguments** (correctness-2). When a `route_call` hint's method or path is a parameter of its enclosing function, the linker resolves it from each call site in the same container with literal arguments, one route call per call site, with both lines as evidence. An interpolated path segment normalizes to `{}` and matches any `{param}` segment of a served route. This is what S4's post-hand-off `worker -> orders` relation needs.
- **F3. A flushed edit keeps its own writer instance** (correctness-3). A pending canvas edit stores its tab's client ID, and whichever tab flushes it sends that ID, so its conflict check is the original tab's.
- **F4. A restore's replay set is rebased** (correctness-4). Replayed edits take the restored store's head as their base and go through the normal compare-and-set; a conflict becomes a card.
- **F5. Repo identity survives duplicates and renames** (unasked-1, unasked-2). Two discovered repos with one name get distinct namespaces, the second suffixed from a hash of its relative path, and `doctor` says so. Git's rename detection carries a file's ID to its new path as an alias; anything else that vanishes goes through the `rescan.retarget` role as before.
- **F6. A single repo's store lives in its git directory** (unasked-4). For a workspace that is one repo, Planroom's state lives in `<repo>/.git/planroom/`, which builds, copies and `git clean -x` never touch. A directory of repos keeps `.planroom/` beside them.
- **F7. The temporary-host backup stays local and private** (unasked-3, security-9). The backup ref holds the redacted event export, is never pushed by Planroom, and no model-visible output suggests pushing it. The pane says once where it is and how to copy it out.
- **F8. A stable canvas origin** (unasked-7). `serve` uses the workspace's recorded port; if another process holds it, `serve` exits with `port_in_use` and the fix line rather than moving, so pending edits never strand in another origin.

## Surfaces

- **F9. Every pane action is at most two keys from pane focus** (principles-2, ux-2, ux-3). Redesign the key map so each main flow in S9 needs at most two keys after focus: lens and project pickers are a key plus a digit; each visible pane section shows its top items with direct row keys, and a second key acts on the selected row; a stage run's cost card is the second key. A test counts real keys on scripted flows (S1 to S4, hand-off, sync), not key-map entries.
- **F10. The Artifact serves what a cloud session needs** (ux-1). The snapshot precomputes the views S1 to S6 need for their subjects (paths for the S2 questions, anatomy outlines, machine views, top `show` pages), and anything else becomes an inbox request the next sync answers. In a cloud session with remote mode on, the session syncs at each turn start.
- **F11. Unpaired edits are kept, not discarded** (principles-3). The confirming card for unpaired owner rows pre-selects Apply for reversible edits and lists them; person-only commands from any Artifact row, paired or not, always need a desk confirmation (see F15).
- **F12. Budgets on every agent output** (principles-4). Every agent-facing output has a size budget, including type and schema discovery, which pages.

## Packs and contracts

- **F13. Lenses name their kinds, and prechecks read structure** (extensibility-1, extensibility-2). `LensDecl` gains `kinds`; a lens's map lists its kinds' nodes first. `when` is the `filter` shape's predicate (fields plus `with_edge` and `without_edge`), evaluated by the kernel. `flag_stale` becomes "gift_wrap is off and unused on main. Remove it or keep it?"
- **F14. The resolver's outputs are part of the one contract** (principles-1): its JSON and error documents are contract types with a generated schema and a drift test.

## Security

- **F15. Authority stays with a person's own action** (security-1 to security-4):
  - The implementer runs as a Claude Code agent type with Bash sandboxing on (writes confined to its clone, network off) where the host supports it; WS8 verifies it as A-18. SECURITY.md states the residual risk that approved test commands run code the implementer wrote.
  - The Artifact pairing key lives in page storage that a republished page can read, so pairing authenticates rows against on-disk tampering only, not against a compromised session. Paired rows therefore apply only reversible edits; every person-only command from any Artifact row needs a desk confirmation.
  - Remove every stale description of the HMAC person key kept in `store.db` (DR-72 replaced it).
  - `person init --rekey` needs the old key or a mod press, and `open` runs only from a mod press; neither accepts the terminal confirmation.

## Operations and repo gates

- **F16. Releases are what users install** (operability-1). The published marketplace entry points users at the latest release, not at `-dev` main: by a release-tag source if the marketplace schema allows one, else a `release` branch the owner fast-forwards to each tag. WS8 checks which. The README's install command names it.
- **F17. Detached work gets its own session on every OS** (operability-2): `setsid` on Linux, `perl -MPOSIX -e 'POSIX::setsid(); exec @ARGV'` on macOS, with stdio closed, tested on both in the installer job.
- **F18. Gates fire only where they mean something** (operability-4, repo-1, repo-2). The layout walk runs on push and pull-request ranges; a `workflow_dispatch` run checks state only. Network access is an allow-list per crate in `boundaries.toml` (only `planroom-models` and the server may open sockets), and the contract crate gets its own dependency allow-list. In CI a `skipped` check fails unless it is on a declared list with a reason.
- **F19. The mod checks its host** (unasked-5): at `session.start` it checks the Claude Code version and the mod API it needs; on a mismatch it shows one status line and falls back to the skills and CLI.
- **F20. Hand-off results can land by pull request** (unasked-6): an item is done when its rules hold on the tracked branch, however the change got there (local merge, pull request or squash), with the patch identity recorded for the reconciler. `handoff merge` stays the local path.
