# Architecture review round 3: accepted risks

Material findings the documents can't fully close, one line each: the finding, why it stays open, and what would close it. The main agent signs each off.

| ID | Title | Why it stays open | What would close it |
|---|---|---|---|
| unasked-4 | A single-repo store sits where build contexts and copies pick it up | F6 moves it to `<git dir>/planroom/`, out of reach of `git clean`, `git archive` and copies that leave out `.git`, but a Docker build context or `rsync` that copies `.git` still carries the plan history; `SECURITY.md` says so. | Keeping every store under the data root with only a pointer beside the repo, plus the temporary-host ref backup for hosts that lose the data root. |
