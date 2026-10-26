# Architecture review round 3: accepted risks

Material findings the documents can't fully close, one line each: the finding, why it stays open, and what would close it. The main agent signs each off.

| ID | Title | Why it stays open | What would close it |
|---|---|---|---|
| unasked-4 | A single-repo store sits where build contexts and copies pick it up | F6 moves it to `<git dir>/planroom/`, out of reach of `git clean`, `git archive` and copies that leave out `.git`, but a Docker build context or `rsync` that copies `.git` still carries the plan history; `SECURITY.md` says so. | Keeping every store under the data root with only a pointer beside the repo, plus the temporary-host ref backup for hosts that lose the data root. |
| security-1 | An approved check runs code the implementer wrote | F15's sandbox confines that code to its clones with no network, and a host without the sandbox makes every approved command ask; the code still runs as the person's OS user inside the clone, `SECURITY.md` states it, and A-18 is unverified until WS8. | Running the checks as a separate OS user or in a VM, or an A-18 spike that also denies the sandbox reads of the mod's `$.store` and the data root. |
| security-2 | A page the model republishes can read the pairing key | Signed rows now apply only reversible edits, and every pick waits for a desk key (F15), so a forged signed row is an attributed, undoable edit at worst; it still lands without a card. | A publish guard that ties each Artifact publish to the last `share` manifest, or a pairing key the page can't export, once the Artifact runtime offers one. |
| security-4 | Re-keying by a press is a guardrail between processes of one OS user | The rekey and `open` no longer take a terminal confirmation (F15), but the engine can't tell a mod press under a new key from an agent that got past the guard's text match with a key of its own; the person's own mod then shows `person_key_mismatch` at once. | Registering the key through a channel no Bash process can reach, such as an OS keychain prompt or a hardware key. |

## Downgraded findings left open

The refuters downgraded these two from material to minor. The fix log doesn't mark them fully applied, so they're listed here too.

| ID | Title | Why it stays open | What would close it |
|---|---|---|---|
| ux-4 | Bare-digit band hotkeys take over a message that starts with 1, 2 or 3 | The band names only undoable, unpaid, non-hand-off leaves, with a toast and a 10 s undo, so a stray digit costs a retyped character; `known-gaps.md` has the trade-off. | The S9 evidence run in M5 with a real session, then dropping the bare digits if the harm shows. |
| cost-1 | The live spend plan undercounts S03 re-records | Applied in part: the plan now counts re-records, aborted recordings and Phase 1's spend, about $2.10 with about $0.65 left before the guard refuses at $2.75. Recording on the cheap tier was left out, because S3's replays should show the tier it runs. | The spend total each milestone's end records, checked against the plan before M2's recordings. |
