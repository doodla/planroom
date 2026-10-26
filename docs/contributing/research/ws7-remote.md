# WS7: the Artifact round trip

Walking-skeleton slice WS7, run on 2026-10-26 from the build session against the real Artifact platform. `[V]` marks a result seen here.

## What ran [V]

1. `cargo xtask fixture`, then `planroom scan --json` and `planroom share --out .planroom/share --json` in the built `shop` workspace, with the engine built with the canvas embedded.
   - The share bundle: `page.html` (260 bytes), `data/snapshot.json` (3,660 bytes), and 15 assets: the canvas's JS and CSS, ELK's worker, two WebP empty states and the three faces' `woff`/`woff2` files.
   - The snapshot holds the top level's 10 blocks and the pane (`capabilities: ["map:system:all", "pane"]`), with no code text, no presence and no local path.
2. The session published it with the exact arguments `share --json` returned (`file_path`, the `files` map, `capabilities` with the three `db` rules and `user`, `title`, `description`, `icon: "map"`): https://claude.ai/artifact/69VbWfb55J2p6kduVUJcG8, version 1, readable by its owner only.
3. The session applied share's one `db` write, `live/head`, with `ArtifactData batch`, at version 1.
4. A read request row, the shape the page writes (`planroom.inbox/v1`, `command: "map"`, `request: {"level": "container"}`), went to `inbox/<owner uid>/events/ws7-roundtrip-1`, with the owner's `inbox/<uid>` head bumped.
5. `ArtifactData list` on that collection returned the row in its text block. The `{path, version, row}` line, plus `{"path": "live/head", "version": 1}`, went to the real `planroom import - --json` with no person token.
   - Import answered it: `Read requests answered: 1`.
   - It returned one batch: `answers/ws7-roundtrip-1` (the 9 container-level blocks, 1,937 bytes), `acks/<uid>` (`imported`), the row's delete pinned at version 1, and `live/head` last, pinned at version 1.
6. The session applied the batch atomically: answer and ack at version 1, the row deleted, `live/head` at version 2.

The page's own half (drawing the snapshot, writing the row when a viewer dives, drawing the answer) runs in `tests/e2e/src/artifact.spec.ts` against share's real bundle, with a `window.claude` shim that enforces the same `db` rules. On the platform it runs on the owner's first visit; the next sync reads what that visit wrote.

## A-13: the owner's session can write any viewer's inbox [V]

- As the owner, `ArtifactData set` on `inbox/u_A13probeAAAAAAAAAAAAAA/events/a13-probe`, a made-up viewer's inbox, committed at version 1. The probe was deleted in the next batch.
- The same write with `as_level: "interact"` was refused, reading as not found: the `inbox/{self}` rule holds for every other viewer.

So A-13 fails, and its fallback applies. Only the owner can write under another viewer's `inbox/{self}`, and the owner's session is a model that can do so with no prompt. So an inbox row is no proof that its viewer wrote it. Every imported row that no paired device signed, whoever's inbox it sits in, waits for the desk as the owner's unsigned rows do: an edit as a ghost on the one Apply card, a pick pre-selected on its card. Read requests need no person, so they are answered either way.

## Platform notes [V]

- `ArtifactData list` returns each document's `data` in its text block, one JSON object per line between the `BEGIN/END ARTIFACT DB` markers. The mod's structured `result.db_read.docs[].data` came back `{}` (WS8), so `/room sync` reads the text block.
- Every write to a document that exists needs `if_version`. Import's batches carry the pins from the `version` the session passes on each stdin line, so a sync never overwrites a viewer's newer write.
- The publish result arms a durable wake subscription for republishes and comments sent to Claude. `comments.sendToClaude` waking the session needs a viewer's tap on a page that declares `comments`; this snapshot's page doesn't yet (deferred with Send to Claude).
