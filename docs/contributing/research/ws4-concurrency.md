# Spike: multi-process SQLite and the live loop (WS4)

This page records the WS4 concurrency spike, run on 2026-10-26. It tests bet 1 in [Architecture](../architecture.md#biggest-bets): several engine processes write one SQLite store with no daemon, each commit one `BEGIN IMMEDIATE` transaction in WAL mode with `synchronous = FULL` and a 5 s busy timeout [DR-1].

## Method

- **Writers.** `crates/store/tests/concurrency.rs` starts three processes, each the test binary run again as a writer. A start file releases them together. Each makes 500 commits through `Store::commit`, one `version` event per commit, each under its own idempotency key. A writer retries only on `store_busy` and counts each retry.
- **Watch under load.** `crates/planroom/tests/live.rs` scans a fresh fixture, starts `planroom watch --json` on it, and waits for its first `ping`. It then releases the same three writers. 50 ms later it makes an empty commit in `worker` and runs `planroom scan`, so a scan commits a snapshot, its facts and a new link snapshot among the writers. Each writer prints the time each commit returned; the test stamps each frame as it reads it from `watch`'s stdout. Latency is frame read minus commit return.
- **Own commits.** The same file runs the live loop in-process on its own read-only connection, with the store's commit hook wired to it, and commits 100 events 7 ms apart. It then repeats the run with no hook, so frames come only from the 200 ms `data_version` poll.
- **Kill after commit.** `crates/store/tests/idem.rs` starts a writer that commits, prints its `seq`, and hangs. The test kills it with SIGKILL before it replies, then retries the same key and request hash from a new connection.
- **Machine.** A 4-CPU Linux VM (kernel 6.18), ext4 on a virtio disk. Debug build (`cargo xtask test`), so SQLite's C code is unoptimized. Each harness ran 3 times; the tables give the range.

## Results

| Measure | Result |
|---|---|
| Commits lost, repeated or out of a writer's order | none: `seq` 1 to 1,500 exactly once, each writer's commits in its own order |
| `SQLITE_BUSY` past the 5 s busy timeout | 0 in every run |
| Throughput, 3 writers × 500 commits | 1,630 to 1,690 commits/s (885 to 921 ms wall) |
| Commit time, p50 / p95 / p99 | 0.42–0.49 ms / 0.88–0.95 ms / 1.6–2.0 ms |
| Commit time, max | 631–634 ms in every run (see below) |
| `watch` commit-to-frame, p50 / p95 / p99 / max | 105–114 ms / 182–186 ms / 194–196 ms / 199–205 ms |
| Own commits through the commit hook, p50 / p95 / max | 0.35–0.43 ms / 0.56–0.77 ms / 1.0–3.6 ms |
| The same loop without the hook (poll only), p50 | 191 ms: each commit landed just after a poll, so this is the worst case of one interval |
| Rescan during the load: one repo moved, snapshot, facts, link and scan event | 135–546 ms for the whole command; the writers' max commit was 632 ms to 1.04 s in those runs |
| Retry after a kill after commit | the stored reply, `replayed: true`, the original `seq`, nothing written; another request hash under the key is `idempotency_mismatch` |

**The 632 ms tail.** One commit per run waits about 630 ms, in both harnesses. That is SQLite's busy handler: a waiting connection sleeps 1, 2, 5, 10, 15, 20, 25, 25, 25, 50, 50 ms, then 100 ms per try, which sums to 628 ms after three 100 ms tries. Under steady contention a waiting writer sleeps through free lock time while the others keep committing. A 1 ms fixed-step handler, measured the same way, cut the max to 64–188 ms but raised p95 to 3–7.5 ms and p99 to 29–36 ms, because the writer that just committed wins most races. The store keeps SQLite's handler, as DR-1 describes it.

## Verdict

Bet 1 holds. Three processes at about 1,600 commits/s, far above any agent or person's write rate, met no `SQLITE_BUSY`, lost no commit and kept every writer's order. `watch` framed every commit within 205 ms, a fifth of the 1 s live bar. A process's own commits reach its live loop through the commit hook in under 4 ms, with no poll. A killed writer's retry returns its stored reply. No fallback is needed for WS4.

## Limits of this run

- The disk is a VM's virtio disk, whose fsync may return before the host's disk does, and the build is a debug build. Real laptops will show slower fsync and a faster engine. S8 repeats the load on CI runners.
- On macOS, `synchronous = FULL` calls `fsync`, which doesn't flush the drive's cache. Durability past power loss there needs `PRAGMA fullfsync`; nothing measured it here.
- The scan's link step builds an empty link snapshot, because no linker exists yet, and the reconcile step doesn't exist yet. The rescan here times extraction, the snapshot, its facts and the link snapshot, not linking or reconciling.
- The 632 ms tail is under a load no workspace makes: three processes committing back to back. S8's harness should watch it.
