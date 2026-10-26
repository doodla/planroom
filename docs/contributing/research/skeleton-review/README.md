# The walking skeleton's review

The skeleton's review ran on 2026-10-26, against commit `6cc222d`, under KICKOFF's review protocol:

- Eight reviewers, each with one focus: correctness, security, drift from the docs, tests and evidence, repo design, the user's surfaces, operability and release, and what was expected but not asked.
- Each reviewer was given the code and the docs, not the build's reasoning.
- A separate verifier then tried to refute each focus's findings. It reran the repro where it could and set each finding's severity again.

You fix every material finding that survives refutation. You log each minor one here as a known gap, to be closed in the milestone its area belongs to.

## Files

| File | What it holds |
| --- | --- |
| `round<N>-<focus>.json` | One focus's findings in round N (1 to 3): `id`, `title`, `severity`, `where`, `evidence`, `why` and `fix`, and its verifier's verdicts: `refuted`, `severity_after` and `reason` |
| `README.md` | This index: each finding's outcome, round by round |
| `accepted-risk.md` | What stays open after round 3, why it's accepted, what checks round 3's fixes, and who signed it off |

## Round 1

There were 94 findings. One was refuted. 18 stayed material. They cover 17 distinct defects, because unasked-3 repeats correctness-1. The other 75 are known gaps.

The fixes, merged on 2026-10-26 and recorded in [DR-85](../../decisions.md#dr-85-what-the-walking-skeletons-review-settled):

| Fixes | Findings they close |
| --- | --- |
| Scan (`e50d3e3`) | correctness-1 and unasked-3: extract from the commit, not the working tree |
| Contract (`e700ebc`) | unasked-1, correctness-4 and drift-2: open persisted formats, the `workspaces.json` lock, and the types the contract crate owns |
| Engine (`4cd9e32`) | drift-1, unasked-8, ux-6, the engine's half of ux-3 and security-5, and the summary line ux-2 needs |
| Surfaces (`687c713`) | ux-1, ux-4, security-3, and the mod's and canvas's halves of ux-2, ux-3 and security-5 |
| Plugin and build (`15cbb21`) | ops-2, ops-3, and repo-1's `not built` lines |

unasked-4 and the rest of repo-1 are schedule gaps. Their fix is a row in the milestone plan ([Architecture](../../architecture.md#milestones)).

| ID | Finding | Outcome |
| --- | --- | --- |
| correctness-1 | A snapshot is keyed by the HEAD commit but built from the working tree, so the first scan of a commit freezes a dirty tree's facts and later clean rescans never correct them | Material: fixed in `e50d3e3` (scan) |
| correctness-2 | Ref evidence verifies paths that aren't in the commit: untracked directories and empty untracked files reach the map | Minor: known gap |
| correctness-3 | A repo that leaves the workspace (deleted, moved or renamed) stays on the map forever | Minor: known gap |
| correctness-4 | workspaces.json is rewritten without a lock and drops every field this build doesn't know, min_reader_version included | Material: fixed in `e700ebc` (contract) |
| correctness-5 | Two concurrent first `serve --detach` runs in one workspace both start, on different ports; one is orphaned | Minor: known gap |
| correctness-6 | Reads aren't in one read transaction, so a reply's meta.seq can be newer than its content and the active set can be torn | Minor: known gap |
| correctness-7 | The canvas subscribes to the stream after loading, with no `since`, so a commit in between never triggers a refetch | Minor: known gap |
| correctness-8 | A foreground `serve --json` prints a second JSON document when serving fails after its success envelope | Minor: known gap |
| correctness-9 | meta.packs_hash is written only once, so after an engine upgrade it disagrees with the hash new snapshots carry | Minor: known gap |
| correctness-10 | live/head's seq bookkeeping can skip changes the published page never saw, and can list changes past head.seq | Minor: known gap |
| correctness-11 | serve's live loop thread dies silently on any error other than schema_mismatch, and the server keeps running with no live frames | Minor: known gap |
| correctness-12 | A store file left unmigrated makes every read command fail with internal until the next scan | Minor: known gap |
| security-1 | person init and --rekey accept a token signed by the key on stdin, so any process can make itself the person | Minor: known gap |
| security-2 | One-time person tokens can be replayed: the spent-token digest is over the raw token string, which has many valid spellings | Minor: known gap |
| security-3 | The mod guard is bypassed by everyday shell forms (globs, cd, split quoting, a script file), exposing the person key file and person-only commands | Material: fixed in `687c713` (surfaces) |
| security-4 | Redaction runs only in share and import: map/focus CLI output, mod tool results, /room, HTTP and watch emit planted secrets raw | Minor: known gap |
| security-5 | No display sanitizing: repo-derived names carry OSC 52 and bidi controls raw to the terminal and the mod pane | Material: fixed in `4cd9e32` (engine) and `687c713` (surfaces) |
| security-6 | A PLANROOM_RELEASE_BASE_URL download is installed into the shared engine cache that every later session runs | Minor: known gap |
| security-7 | Resolver drift: a release version with no tag builds and caches main; the checkout test lacks the plugins-dir exclusion; the cache key lacks the hash | Minor: known gap |
| security-8 | Node IDs embed repo-derived text and are exempt from redaction, so a secret-shaped name reaches the shared Artifact snapshot | Minor: known gap |
| security-9 | Person tokens bind neither the workspace nor stdin, and the token stays readable in /proc/<pid>/environ | Minor: known gap |
| security-10 | Import writes answers to answers/<docId> with no viewer in the key, so one viewer's row can overwrite another's answer | Minor: known gap |
| security-11 | SECURITY.md and the Dependabot config don't exist, yet the docs rest residual risks on them | Minor: known gap |
| drift-1 | With --json, a bad flag or value prints no JSON at all; the mod reports it as `internal` | Material: fixed in `4cd9e32` (engine) |
| drift-2 | Types the docs place in the contract crate are still hand-written outside it: EngineState, ToolResult and the resolver's envelopes (contract.sh doesn't exist) | Material: fixed in `e700ebc` (contract) |
| drift-3 | CommandSpec doesn't match contracts.md, and DR-81 records `spends` as done when it isn't | Minor: known gap |
| drift-4 | The error and warning registries hold about half the documented codes, and the documented table-versus-registry tests don't exist | Minor: known gap |
| drift-5 | The committed v1 schema publishes Stakes and ModelRequestRef in shapes contracts.md contradicts | Minor: known gap |
| drift-6 | `map` and `map --level system` both say level system but return different blocks; the explicit level drops parentless containers without counting them | Minor: known gap |
| drift-7 | Error envelopes break the envelope rules: meta.seq is always 0, invalid_argument lacks its synopsis and schema handle, and hints aren't exact commands | Minor: known gap |
| drift-8 | Neither `scan` nor `cargo xtask fixture` records the workspace in workspaces.json, so M1's trust check will treat skeleton workspaces and the fixture as untrusted | Minor: known gap |
| drift-9 | The catalogue hash covers only the leaf listing, so it can't detect an engine whose flags or result types changed | Minor: known gap |
| drift-10 | The CLI surface differs from the docs: `map --focus` is offered then refused, short flags exist, and bare `planroom` prints help | Minor: known gap |
| drift-11 | Logging follows neither convention: no `tracing`, no PLANROOM_LOG, and foreground serve and watch write no log file | Minor: known gap |
| drift-12 | Docs describe the built skeleton wrongly: the `room` skill and undocumented v1 result types | Minor: known gap |
| tests-1 | No test passes a token the mod signed to the real engine; the 'shared example' pins only the message bytes | Minor: known gap |
| tests-2 | WS5's resolver background detach and its installer-job check are claimed but not built; the resolver's JSON isn't validated against the schema | Minor: known gap |
| tests-3 | Tests that were built don't carry the names feature_list.json lists, and no gate checks the references | Minor: known gap |
| tests-4 | ws8-install.md backs the implementer's sandbox rule with a test that doesn't exist | Minor: known gap |
| tests-5 | The mod's harness answers `version` with a mismatched catalogue hash and calls it ready; the mod never makes the hash check architecture.md describes | Minor: known gap |
| tests-6 | The M1 overlay invents relation kinds and evidence paths that contradict TRUTH.json and the fixture | Minor: known gap |
| tests-7 | The testkit fixture's Ruby pack config names a directory that doesn't exist, and the engine never reads the config | Minor: known gap |
| tests-8 | The CI skip rule only sees whole-step skips; in-test skips pass as ok, including the only real-engine mod test | Minor: known gap |
| tests-9 | The 1 s commit-to-frame tests start their clock after the command returns, not at the commit | Minor: known gap |
| tests-10 | The golden-reader boundary is bypassed by a runtime fs read the gate can't see | Minor: known gap |
| tests-11 | WS4 and canvas timing claims have no committed run data | Minor: known gap |
| tests-12 | The pane's 49-column fit check allows 3 extra columns and measures elements, not rows | Refuted |
| tests-13 | The skill/command collision gate is a hand-kept list, and the harness accepts every command registration | Minor: known gap |
| repo-1 | S15 gates that docs place inside `cargo xtask check` are neither built nor scheduled, and check prints no line for them | Material: fixed in `15cbb21` (plugin and build) and the milestone plan `541ca9e` |
| repo-2 | Root AGENTS.md and repo-layout name xtask commands and flags that don't exist | Minor: known gap |
| repo-3 | Directory map and area tables describe about 20 paths that don't exist, unmarked | Minor: known gap |
| repo-4 | tests/e2e reads tests/golden via fs, contradicting the gated rule that only the mod's tests read the goldens | Minor: known gap |
| repo-5 | Generated-file marking described as done is mostly hand-written or missing | Minor: known gap |
| repo-6 | engine.env is generated whole, but the docs say PLANROOM_REPO is hand-written outside BEGIN/END markers | Minor: known gap |
| repo-7 | AGENTS.md conventions point to files that don't exist, including the wrong home for pane code | Minor: known gap |
| repo-8 | Most areas' 'How to test it' names tests that don't exist and omits the ones that do | Minor: known gap |
| repo-9 | Layout lists missing in 15 of 20 areas; several tracked directories have no stated purpose anywhere | Minor: known gap |
| repo-10 | repo-layout restates boundaries.toml, and the copy has drifted | Minor: known gap |
| repo-11 | xtask doesn't set PLANROOM_REPLAY, PLANROOM_SPEND_LOG or a temp PLANROOM_DATA as the docs say | Minor: known gap |
| ux-1 | In the mod pane, every actionable button leads nowhere: the next action does nothing, and every block row errors | Material: fixed in `687c713` (surfaces) |
| ux-2 | A typed /room command shows the person raw JSON cut off at 1,200 characters, not the summary line and handles | Material: fixed in `4cd9e32` (engine) and `687c713` (surfaces) |
| ux-3 | The Intent level is a blank map, with no empty state and a broken summary line | Material: fixed in `4cd9e32` (engine) and `687c713` (surfaces) |
| ux-4 | On the phone, "Open the containers" is one-way: no breadcrumb, rail or gesture leads back up, and the lid never names the level | Material: fixed in `687c713` (surfaces) |
| ux-5 | A second `serve` fails with port_in_use and tells the person to stop their own Planroom server | Minor: known gap |
| ux-6 | In a folder with no git repos, the one next action is a scan that fails, and map and focus disagree | Material: fixed in `4cd9e32` (engine) |
| ux-7 | No built path lets a person open the canvas, and the recovery strings point at an `e` key that doesn't exist | Minor: known gap |
| ux-8 | On the Artifact, the face-up next action points outside the snapshot, and without the db the page asks you to "Ask your computer" while saying it can't | Minor: known gap |
| ux-9 | A failed command takes over the whole canvas, and both surfaces drop the error's hint | Minor: known gap |
| ux-10 | The mod's user-facing strings bypass en.toml, and the version-mismatch pane's button doesn't do what its detail says | Minor: known gap |
| ux-11 | Blocks are mislabelled and never take a tone, and their subtitles truncate the counts | Minor: known gap |
| ux-12 | Engine text lines are wrong at other levels and use internal jargon | Minor: known gap |
| ux-13 | Design-language drift: the Artifact bundles its fonts instead of loading them from Google Fonts | Minor: known gap |
| ops-1 | The resolver's engine cache is keyed only by version, so a -dev install keeps running an older engine after the plugin updates. Nothing compares catalogue hashes. | Minor: known gap |
| ops-2 | The source build never builds the canvas, so the engine's fix page sends the user into a loop | Material: fixed in `15cbb21` (plugin and build) |
| ops-3 | The plugin README's install line uses a `--marketplace` flag, which KICKOFF says doesn't exist | Material: fixed in `15cbb21` (plugin and build) |
| ops-4 | PLANROOM_RELEASE_BASE_URL downloads go into the shared cache and become the engine later sessions run | Minor: known gap |
| ops-5 | The source build runs attached with a 10-minute cap and restarts from scratch each time | Minor: known gap |
| ops-6 | A second `serve --detach` in the same workspace fails with port_in_use and suggests moving the port. Once the 60 s login code expires, there's no way back into the canvas. | Minor: known gap |
| ops-7 | A release can't be cut from what exists, and the manifests' versions already disagree | Minor: known gap |
| ops-8 | The resolver's error codes aren't registered, and contract.sh and ResolverStatus don't exist | Minor: known gap |
| ops-9 | How the resolver decides it's in a checkout, and how it handles a missing release tag, both differ from the design | Minor: known gap |
| ops-10 | A failed download loses its reason and URLs when the toolchain is also missing | Minor: known gap |
| ops-11 | The resolver creates the data root as 0755, before the engine's 0700 rule can apply | Minor: known gap |
| ops-12 | The logging convention isn't built, and serve's failures are mostly silent | Minor: known gap |
| ops-13 | workspaces.json is rewritten without a lock and drops fields it doesn't know | Minor: known gap |
| unasked-1 | The versioning rules for persisted documents are implemented in no reader; EventKind ships as a closed set | Material: fixed in `e700ebc` (contract) |
| unasked-2 | workspaces.json is rewritten with no lock, dropping unknown fields and skipping the format and min_reader_version checks | Minor: known gap |
| unasked-3 | scan maps the working tree but files it under HEAD's commit, so untracked directories stay on the map after they're deleted | Material: fixed in `e50d3e3` (scan) |
| unasked-4 | The DR-1 filesystem check is neither built nor scheduled, so the skeleton opens its WAL store on WSL /mnt, NFS or a sync folder | Material: fixed by milestone plan (M1's event-backup and store slice) `541ca9e` |
| unasked-5 | The engine has no logging: no tracing, no PLANROOM_LOG, no log file for watch, and serve's log never rotates while it runs | Minor: known gap |
| unasked-6 | The canvas ships elkjs (EPL-2.0) and OFL fonts with no notice in served or Artifact builds, and no npm license gate | Minor: known gap |
| unasked-7 | Community-health files, Dependabot and the weekly audit are missing and unscheduled, while AGENTS.md describes Dependabot as running | Minor: known gap |
| unasked-8 | map has no output ceiling, and the live mod tool exposes it: 3.5 MB at code level for a 20,000-file repo | Material: fixed in `4cd9e32` (engine) |
| unasked-9 | The string catalogue has no plural forms, and the canvas builds count text by hand ("4 component · 11 code") | Minor: known gap |

## Round 2

Round 2 ran on commit `969a8e5` with the same eight focuses, each checked by a separate verifier. Each reviewer first checked that round 1's material fixes in its area held; they did. There were 65 findings: 16 material, covering 12 distinct defects, 48 minor known gaps, and 1 refuted. The fixes, merged on 2026-10-26 and recorded in [DR-86](../../decisions.md#dr-86-what-the-walking-skeletons-review-settled-in-round-2):

| Fixes | Findings they close |
| --- | --- |
| Scan (`5833a04`) | correctness-13 to -16, unasked-r2-1 and ux-a: snapshots keyed by every input, no move back, no fetch, per-repo outcomes |
| Reply budgets (`d757ac5`) | correctness-17, unasked-r2-2 and drift-r2-1: every CLI reply within its budget, warnings included |
| Guard (`bd47217`) | security-r2-1, security-r2-2, tests-r2-2 and drift-r2-2: the guard's write rule and whole-tree reads for Bash, the cache check, the contract's types in the mod |
| Canvas (`f984aa8`) | ux-b and ux-c: the phone's 44 px lid, no control that leads nowhere |

tests-r2-1 was fixed in the plan: `map` paging moves into M1's S1 slice.

| ID | Finding | Outcome |
| --- | --- | --- |
| correctness-13 | A snapshot's facts still depend on things outside its key: the compose join reads the workspace's repo set, and evidence reads the branch, so the map depends on scan history | Material: fixed in `5833a04` (scan) |
| correctness-14 | A slower scan that started before a commit lands after a newer scan and moves the repo back to the old commit, recorded as a real move | Material: fixed in `5833a04` (scan) |
| correctness-15 | The DR-85 commit walk triggers lazy fetches in partial clones, which run a command named in the scanned repo's local config | Material: fixed in `5833a04` (scan) |
| correctness-16 | One repo with no commits (fresh `git init`) fails the whole workspace's scan, and the empty state's next action is that failing scan | Material: fixed in `5833a04` (scan) |
| correctness-17 | scan's reply is still uncapped: the partial_scan warning carries every uncovered path (160 KB, about 40k tokens, for a 60k-file repo) | Material: fixed in `d757ac5` (reply budgets) |
| correctness-18 | A tag with the same name as the checked-out branch drops the repo node and all its directories; the files float to the top level, frozen for that commit | Minor: known gap |
| correctness-19 | The DR-85 extraction change didn't change packs_hash, so stores scanned before the fix keep their working-tree facts for every unchanged commit | Minor: known gap |
| correctness-20 | Template substitution re-expands placeholders inside values: a repo named `{commit}` prints 'Scanned 0b305cb at 0b305cb.' | Minor: known gap |
| correctness-21 | Merging one node ID across repos picks its winner by snapshot insertion order, so a shared container's name and props depend on scan history | Minor: known gap |
| drift-r2-1 | Agent-facing replies exceed their documented budgets: `schema --command` (the hint of every usage error) and `share`; DR-85's 'everything an agent can reach is capped' holds only for map | Material: fixed in `d757ac5` (reply budgets) |
| drift-r2-2 | drift-2's fix left the mod hand-copying contract types: the envelope as `Reply`/`ReplyError`, and the version and resolver-status replies as ad-hoc shapes | Material: fixed in `bd47217` (guard) |
| drift-r2-3 | import answers the Artifact canvas's `--detail graph` read requests through the agent-capped map, and drops the truncation, so the page silently draws a partial map | Minor: known gap |
| drift-r2-4 | Tool descriptions list leaf summaries, not the generated synopses contracts.md and architecture.md promise; catalogue.ts carries no tool specs or ROOM_COMMANDS | Minor: known gap |
| drift-r2-5 | DR-85's map drill-down never reaches /room text, and the shipped skill tells agents to page with `meta.next`, which map doesn't set until M2 | Minor: known gap |
| drift-r2-6 | Usage-error messages lose their content or nest a bug message: clap's first line only, and share's bad --out says 'Planroom hit a bug' | Minor: known gap |
| drift-r2-7 | The documented uint64 schema lint isn't built and isn't among the steps `check` prints as 'not built'; the schema has unannotated uint64 in 21 $defs | Minor: known gap |
| drift-r2-8 | architecture.md restates VersionInfo without `target`, contradicting contracts.md and DR-82 | Minor: known gap |
| drift-r2-9 | The resolver's `--json` output isn't valid JSON when an interpolated value holds a control character | Minor: known gap |
| drift-r2-10 | The resolver reports a data-root mkdir failure as `download_failed`, so the mod offers a source build for a permissions problem | Minor: known gap |
| drift-r2-11 | import's summary counts a refused row as not refused | Minor: known gap |
| security-r2-1 | The guard never applies its write rule to Bash, so an agent can overwrite the cached engine that every later session and every person press runs, and can rewrite or delete `.planroom/` | Material: fixed in `bd47217` (guard) |
| security-r2-2 | Recursive reads of a guarded place's parent pass the guard: `grep -r`, `rg`, `find -exec cat`, `tar` and `cp -r`/`ln -s` on ~/.claude, ~/.config or ~/.local/share, and the Grep tool on ~/.config or ~/.local/share | Material: fixed in `bd47217` (guard) |
| security-r2-3 | DR-85's `/proc/*/environ` denial misses the usual spelling, `/proc/$(pgrep planroom)/environ` | Minor: fixed after round 3, with a regression test ([accepted risk](accepted-risk.md#the-guards-residual-risks-decided)) |
| security-r2-4 | Other guard misses: `xargs planroom` hides a person-only command, `VAR+=` hides a guarded variable, `~user/` paths aren't expanded, and only 32 paths are resolved | Minor: fixed after round 3, with a regression test ([accepted risk](accepted-risk.md#the-guards-residual-risks-decided)) |
| security-r2-5 | The guard now denies `Glob **/*` in the project itself | Minor: known gap |
| security-r2-6 | The sanitizing pass leaves U+2028/U+2029 and Unicode tag characters raw, in both ports | Minor: known gap |
| tests-r2-1 | DR-85's map cap makes S1's tier-A CI path unpassable in M1: the CLI map at --detail graph is truncated on the fixture, and paging waits for M2 | Material: fixed by milestone plan (M1's S1 slice) `aa3f613` |
| tests-r2-2 | security-3's fix misses directory-recursive Bash reads: `grep -r`, `rg`, `cp -r`, `tar` and `zip` reach the mod's `$.store` (the person key) and `.planroom` | Material: fixed in `bd47217` (guard) |
| tests-r2-3 | scan.rs's 'the dirty edit never reaches extraction' assertion can't fail: it reads a top-level `warnings` that no envelope has | Minor: known gap |
| tests-r2-4 | The mod's ux-1 and ux-2 plugin tests run on a hand-written map reply with no `summary`, so they prove only the fallback line; the real engine sends '9 at the top: …' for the container level | Minor: known gap |
| tests-r2-5 | empty_states.rs guards ux-3 with a substring check: it would fail spuriously at 10, 20 or 30 blocks, and it misses the real wording defect | Minor: known gap |
| tests-r2-6 | A usage error for a command group carries `command: "person"`, not the `planroom` DR-85 specifies, and `--since -5` reports field `_5`; usage.rs tests neither | Minor: known gap |
| tests-r2-7 | The stand-in's 'unscanned workspace' spec covers a state the real serve can't reach, and serve's refusal there contradicts map | Minor: known gap |
| ops-r2-1 | The README's install line reads main's marketplace.json, so after a release it still installs main's -dev plugin; the design has no file for the 'published' git-subdir entry | Minor: known gap |
| ops-r2-2 | architecture.md says a clone build whose catalogue hash isn't the plugin's keeps no binary; the resolver compares only the version | Minor: known gap |
| ops-r2-3 | The source build hard-codes target/release, so a CARGO_TARGET_DIR or build.target-dir setting makes a successful build look like a failure or a version mismatch | Minor: known gap |
| ops-r2-4 | ops-2's residual: an engine built without a canvas leaves no reachable way to rebuild, and a failed canvas build is never shown | Minor: known gap |
| ops-r2-5 | The canvas step makes the attached source build longer, still under the mod's 10-minute cap, and the '4 minutes' estimate is unmeasured | Minor: known gap |
| ops-r2-6 | The resolver's envelope meta.size.bytes counts characters under bash, so it's wrong for a non-ASCII path, as on macOS, whose /bin/sh is bash | Minor: known gap |
| ops-r2-7 | The preflight can pass on a clean macOS, and it never checks git or the cargo that rustup just installed | Minor: known gap |
| ops-r2-8 | The -dev version stamp was never checked against how /plugin update orders versions | Minor: known gap |
| ops-r2-9 | plugin/README.md doesn't name the tested Claude Code version that DR-76 and R-13 rely on | Minor: known gap |
| ops-r2-10 | schema_mismatch's hint 'update Planroom, then retry' dead-ends for a -dev install, and serve's exit on it leaves serve.json naming a dead pid | Minor: known gap |
| repo2-1 | repo-1's fix covers `check` only: `check audit`, `test`, `e2e` and several R16 gates are still documented as running, print no `not built` line, and the M1 row still says the skeleton's gates work built the rest | Minor: known gap |
| repo2-2 | `Empty` carries no line, so the ux-3 fix hand-wrote three different empty-level texts in the canvas and the mod, outside en.toml | Minor: known gap |
| repo2-3 | A CI job turned off with `if: false` is a skipped check that neither the skip rule nor ci_runs_only_xtask can see | Refuted |
| unasked-r2-1 | One repo git can't read (no commits yet, or owned by another user) fails the whole workspace scan, and focus's one next action is that failing scan | Material: fixed in `5833a04` (scan) |
| unasked-r2-2 | unasked-8's fix doesn't hold for scan: partial_scan lists every uncovered path, so one scan reply ran to about 42,000 tokens | Material: fixed in `d757ac5` (reply budgets) |
| unasked-r2-3 | A cut scan isn't recorded anywhere, so map and focus show a partly mapped repo as whole, and the store schema has nowhere to keep coverage | Minor: known gap |
| unasked-r2-4 | Scan cost isn't bounded by DR-47's bounds: every blob is read past the cut, and an unchanged repo is fully re-extracted before the existing snapshot is found | Minor: known gap |
| unasked-r2-5 | Each snapshot stores a full copy of every fact, so store size grows with retained snapshots times repo size | Minor: known gap |
| unasked-r2-6 | Files whose paths aren't UTF-8 (or contain a backslash or newline) are dropped silently | Minor: known gap |
| unasked-r2-7 | `planroom doctor`, the diagnostics, bug-report and downgrade surface, still has no milestone slice beyond `doctor --pack` | Minor: known gap |
| ux-a | One repo with no commits blocks the whole workspace: scan fails, and every surface keeps offering that same scan as the next action | Material: fixed in `5833a04` (scan) |
| ux-b | On the phone, the ⋯ lid (the way back up) has a 34 px tap area, and the 44 px e2e check never visits a level where that lid exists | Material: fixed in `f984aa8` (canvas) |
| ux-c | The project shelf's "No project" chip opens a blank sheet; the lens switcher opens a one-entry sheet that does nothing | Material: fixed in `f984aa8` (canvas) |
| ux-d | An Empty's one label is used as both the line and the button: the intent level says the same thing twice, no_repos becomes a 104-character button the pane cuts mid-sentence, and pressing "check again" leaves a meaningless Back press | Minor: known gap |
| ux-e | docs/screenshots predate the engine fixes' merge, so they show labels and states the merged tree doesn't produce | Minor: known gap |
| ux-f | The pane's next action is always "Open the containers", whatever the level or the map | Minor: known gap |
| ux-g | The rail's System step and the top lid name the same place but show different maps, and the rail draws Intent nested above System | Minor: known gap |
| ux-h | A usage error's message drops the missing argument ("…were not provided:."), and `/room share`, which /room help offers, hits it | Minor: known gap |
| ux-i | /room hides a truncated map: the summary counts blocks the result doesn't hold, and the drill-down handle never reaches the person or the model | Minor: known gap |
| ux-j | `/room share` returns no publish arguments to the model, so the documented publish can't follow it | Minor: known gap |
| ux-k | During a source build, the fallback's only button ("Check again", also band key 1) flips the pane back to "Build it", and a second press starts a build that deletes the first one's clone | Minor: known gap |
| ux-l | Some fix lines and hints can't be run from where the person is, and the no-canvas fix page is a bare white page | Minor: known gap |

## Round 3

Round 3, the last, ran on commit `8e76c63` with the same eight focuses, each checked by a separate verifier. Each reviewer first checked that round 2's material fixes in its area held; they did, at the cases they were aimed at. Round 3 found their edges. There were 51 findings: 9 material, covering 8 distinct defects, 40 minor known gaps, and 2 refuted. Two fix branches and a CI fix closed the material ones. They were merged on 2026-10-26 and recorded in [DR-87](../../decisions.md#dr-87-what-the-walking-skeletons-review-settled-in-round-3):

| Fixes | Findings they close |
| --- | --- |
| Engine (merge `055f21e`) | correctness-r3-1, unasked-r3-2, ux-r3-1 and drift-3-1, with correctness-r3-2 and -3 and unasked-r3-8 on the way: a late scan over a changed repo set, each transport refused by name, a workspace with no commits, import within its budget |
| Guard (merge `d03c088`) | security-r3-1 to -3 and drift-3-2: the shell's directory per call, every tool's paths and uploads, the plugin's own files, the resolver's reason from the contract |
| CI (`6882469`) | ops-r3-1 and ops-r3-2: the fixture's physical root on macOS, and a failed step's last 400 lines in the log |

No fourth round runs. Round 3's own fixes had no reviewer; [the accepted risk](accepted-risk.md) says what checks them instead, and lists what stays open.

| ID | Finding | Outcome |
| --- | --- | --- |
| correctness-r3-1 | A late scan still moves repos back when its repo set (extraction inputs) changed meanwhile: DR-86's 'a late scan never moves a repo back' holds only for HEAD moves | Material: fixed in `f014f68` (engine) |
| correctness-r3-2 | A workspace whose only repos have no commits loops: 'Map these N repos?', then a scan that maps nothing, then 'Scan again' forever, while focus offers an empty containers level | Minor: fixed with ux-r3-1 in `0b43e3a` (engine) |
| correctness-r3-3 | The no-fetch defence is weaker than documented: a repo-local `protocol.<name>.allow=always` overrides `-c protocol.allow=never`, and partial-clone detection misses a valueless or integer `promisor` key | Minor: fixed with unasked-r3-2 in `6a3b4a2` (engine) |
| correctness-r3-4 | An unborn repo writes a scan event with an empty commit, so stream frames read 'Scanned newrepo at .' and the event log carries a commit that isn't a SHA | Minor: the frame line fixed in `0b43e3a` (engine); the event's empty commit is a known gap |
| correctness-r3-5 | The branch DR-86 moved onto the scan event goes stale: a branch switch or delete at the same commit writes no event, so the store keeps naming a branch that no longer exists | Refuted |
| correctness-r3-6 | A superseded repo still stores a full snapshot of facts, and its warning tells the person to rescan a HEAD that is already mapped | Minor: known gap |
| drift-3-1 | import's reply passes its 800-token budget from 5 read rows (5x at 40), the same defect drift-r2-1 fixed for share but not for import | Material: fixed in `1812b42` (engine) |
| drift-3-2 | drift-r2-2's fix holds only for the four shapes DR-86 named: room.ts still hand-copies MapView as MapLike, and engine.ts types engine_unavailable's details by hand | Material: fixed in `3b02dff` (guard) |
| drift-3-3 | focus's two budgets disagree: the PaneModel is fit to 4,096 bytes, but the envelope's catalogue budget is 1,000 tokens, so a large workspace's focus tells the agent its reply is a bug | Minor: known gap |
| drift-3-4 | An unborn repo writes a scan event and snapshot with an empty commit, counts as mapped and moved, and the empty map's next action loops on the same scan | Minor: the count and the loop fixed in `0b43e3a` (engine); the empty-commit event is a known gap |
| drift-3-5 | share's 'what will be shared' line is a fixed template, not generated from the snapshot's sections: it claims paths and line numbers, and with --include-snippets 'code lines', which the snapshot never holds | Minor: known gap |
| drift-3-6 | Public contract types trail contracts.md with no record accepting the lag, including persisted event fields | Minor: known gap |
| drift-3-7 | The docs cite WS7's live publish as proof of share's exact arguments, but DR-86 changed those arguments (root plus relative files) and the new form was never published live | Minor: known gap |
| drift-3-8 | scan's text mode prints only the summary line and drops every warning, so a person at a terminal never sees repo_skipped, repo_unborn or partial_scan | Minor: known gap |
| drift-3-9 | The docs disagree on where live/head goes in a multi-batch sync; the code follows one of them | Minor: known gap |
| security-r3-1 | The guard resolves Bash paths against the session's start directory, so `cd` in one call and a relative path in the next pass every r2 rule | Material: fixed in `d247aab` (guard) |
| security-r3-2 | The guard sees file paths only in eight tools; Artifact, ArtifactData and SendUserFile read the person key, the gateway key and serve.json unchecked | Material: fixed in `d247aab` (guard) |
| security-r3-3 | The installed plugin's files, the mod bundle with the guard and person-key code and the resolver, are writable by Write and Bash: r2-1's threat with a different target | Material: fixed in `d247aab` (guard) |
| security-r3-4 | The current workspace's state directory can still be deleted through everyday forms the remove rule doesn't read | Minor: fixed after round 3, with a regression test ([accepted risk](accepted-risk.md#the-guards-residual-risks-decided)) |
| security-r3-5 | The mod never re-registers its key once a workspace is in its `registered` list, even when the store lost the key | Minor: fixed after round 3, with a regression test ([accepted risk](accepted-risk.md#the-guards-residual-risks-decided)) |
| security-r3-6 | Tool outputs aren't marked `untrusted: true`, as architecture.md's injection containment says they are | Minor: fixed after round 3, with a regression test ([accepted risk](accepted-risk.md#the-guards-residual-risks-decided)) |
| tests-3-1 | The guard fails open when the host's $.env.get("HOME") gives nothing, and only mocked runs back it: the harness mocks HOME and fs.stat, and the last live run predates both guard rewrites | Minor: fixed after round 3, with a regression test ([accepted risk](accepted-risk.md#the-guards-residual-risks-decided)) |
| tests-3-2 | tests-r2-1's fix never reached feature_list.json: S1's tier-A ci_path still runs the capped map with no paging step | Minor: known gap |
| tests-3-3 | The recursive-read rule and its bypass list cover a closed set of programs: git grep --no-index, ugrep -r, bsdtar, pax and gzip -r still read the person key's parent | Minor: fixed after round 3, with a regression test ([accepted risk](accepted-risk.md#the-guards-residual-risks-decided)) |
| tests-3-4 | The M1 stand-in overlay's evidence names no commit, contradicting DR-86 and what the engine stores | Minor: known gap |
| tests-3-5 | budgets.rs claims to check the whole graph over HTTP but calls the app function in-process, so the route is never exercised | Minor: known gap |
| ops-r3-1 | CI is red on every completed run since round 2's merges: macOS fails round 2's new scan test every time, and Ubuntu fails a different test each run | Material: macOS fixed in `6882469` (CI); the intermittent Ubuntu failures are open in [the accepted risk](accepted-risk.md) |
| ops-r3-2 | xtask keeps only the last 80 lines of a failed step, so CI logs never show why a Rust test failed | Minor: fixed in `6882469` (CI: the last 400 lines) |
| ops-r3-3 | The source-build preflight checks the Rust version only in a checkout, and Node and pnpm only for presence, so the first user's card promises a 4-minute build that fails | Minor: known gap |
| ops-r3-4 | A failed source build leaves its clone and partial target directory under the data root until the next build | Minor: known gap |
| ops-r3-5 | `version --json` never reports git_commit, so nothing identifies which main commit a -dev engine was built from | Minor: known gap |
| ops-r3-6 | A rebuild swaps the cached binary before rewriting its .ok stamp, so a resolve in that window treats the cache as missing | Minor: known gap |
| repo3-1 | The fixture copy that `cargo xtask e2e` builds inside the repo escapes `claudeMdExcludes`, so its planted CLAUDE.md is no longer excluded | Minor: known gap |
| repo3-2 | repo2-1's test-filter fix (608b7f7, after 8e76c63) doesn't match repo-layout's `test` row, and a mistyped FILTER reports `ok` | Minor: known gap |
| repo3-3 | `.cargo/`, which makes `cargo xtask` exist at all, has no stated purpose, and its alias builds xtask without `--locked` | Minor: known gap |
| repo3-4 | The review index's Files table still describes only round 1's files | Minor: fixed in this index |
| unasked-r3-1 | Repo namespaces are the raw directory path: a directory with a space or colon yields node IDs the published NodeId contract rejects or splits wrongly, and the code differs from DR-74 | Minor: known gap |
| unasked-r3-2 | correctness-15's no-fetch fix rests on GIT_NO_LAZY_FETCH alone: a repo-local protocol.<name>.allow=always overrides -c protocol.allow=never, so on a git that ignores the variable a scan runs the repo's configured command | Material: fixed in `6a3b4a2` (engine) |
| unasked-r3-3 | With HOME unset, the data root is relative to the current directory: serve writes workspaces.json, logs and a login page holding a live code into the workspace tree | Minor: fixed after round 3, with a regression test ([accepted risk](accepted-risk.md#the-guards-residual-risks-decided)) |
| unasked-r3-4 | workspaces.json is keyed by root path: a moved or renamed workspace gets a new canvas port, and stale entries keep their ports forever | Minor: known gap |
| unasked-r3-5 | The DR-1 filesystem check covers only the state directory; its fallback, the data root, is never checked, though it will hold the spend ledger and event backups | Minor: known gap |
| unasked-r3-6 | repo_skipped's fix tells a root agent to chown -R another user's tree, the usual devcontainer and bind-mount case | Minor: known gap |
| unasked-r3-7 | The 50,000-file cut drops whole top-level directories in sorted path order, and focus reports the scan as 'done' | Refuted |
| unasked-r3-8 | The scan summary counts unborn and skipped repos as mapped and moved | Minor: fixed in `0b43e3a` (engine) |
| ux-r3-1 | A workspace whose only repos have no commits gets a blank map and a loop: "0 at the top: .", then "Open the containers", an empty level, "Scan again", and back; the line saying it maps after its first commit reaches no person | Material: fixed in `0b43e3a` (engine) |
| ux-r3-2 | DR-86's per-repo warnings never reach a person: CLI text, the pane's toast, /room rows and text, and the canvas drop repo_skipped and repo_unborn, so a partly mapped workspace reads as whole | Minor: known gap |
| ux-r3-3 | Two of DR-86's new fix lines can't be run as written: `chown` to yourself needs root, and paths go into commands unquoted | Minor: known gap |
| ux-r3-4 | person_required tells you to press a pane button or type the command after /room, but neither command that raises it is a pane button or a /room leaf | Minor: known gap |
| ux-r3-5 | On the phone, no control reaches the component, code or intent levels: there's no zoom rail, the ⋯ lid only goes up, and the next action only opens containers | Minor: known gap |
| ux-r3-6 | A picker with nothing to pick still looks like a button: a bordered 44 px box with a button radius, not "flat on the felt" | Minor: known gap |
| ux-r3-7 | Code-level text lists blocks by bare file name, so a person can't tell them apart: ".gitignore; .gitignore; .gitignore; .gitignore" | Minor: known gap |
