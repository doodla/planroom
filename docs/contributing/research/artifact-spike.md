# Spike: Artifact sync probe

This page records the remote-mode spike run on 2026-10-25. It checks the parts of [Artifact research](artifact.md) that need a published page.

The probe is published at <https://claude.ai/artifact/3U8hgkkX2Rp1ECATZCptXD> (private to the owner). Its source is in [`artifact-probe/`](artifact-probe/): a page fragment, a same-origin module script and a published data file.

## Manifest

```js
capabilities: {
  db: { rules: [
    { path: "", read: "view", write: "owner" },
    { path: "inbox", read: "interact", write: "owner" },
    { path: "inbox/{self}", read: "interact", write: "interact" },
  ] },
  user: {},
}
```

## Results from the session side

| Check | Result | Evidence |
|---|---|---|
| Multi-file publish (`files` with `assets/probe.js` and `data/snapshot.json`) | Accepted; stored contract 0.2.90 with the three rules | publish result |
| Session write to `live/head` | Committed at version 1, with no approval prompt in this session's mode | `ArtifactData set` result |
| Contributor reads `live/head` | Allowed | `ArtifactData list` with `as_level: interact` |
| Contributor writes outside its inbox | Refused, and reads as not found | `ArtifactData set` with `as_level: interact`: `invalid_argument` |

## Results from the page side

The probe was opened once, on 2026-10-26 at 00:11 UTC, in the Claude app on Android (`Claude/1.261022.20`, Chrome 157 WebView). It saved its results to `inbox/<viewer id>/probe/latest`, read back with `ArtifactData get`:

| Check | Result |
|---|---|
| Same-origin module script (`assets/probe.js`) | Runs: multi-file publishing with a module script passes the page's CSP |
| Relative fetch of the published data file | Works: `./data/snapshot.json` loaded, 4 nodes and 3 edges |
| `db` | Available |
| `user` | Signed in; `can("data.write")` is `true` |
| Write to the viewer's own inbox | Lands at `inbox/<uid>/events/<id>`, with the page-minted ID as its key |
| Write to server confirmation | 787 ms on the phone |
| `live/head` subscription | Delivered the head written earlier by the session, 3 snapshot callbacks; the lag shown (937 s) is the time since that write, not a live latency, because the session wrote the head before the page opened |

So remote mode's mechanics work end to end on a real phone. The live latency of a session write reaching an open page is still unmeasured; the walking skeleton's WS7 measures it with the page open.

## Implications

- The access rules give the shape remote mode needs: only the session writes shared state, each viewer writes only under `inbox/<their id>`, and the path proves who wrote an edit.
- Session-side writes need `if_version` after the first write. The engine should own the version bookkeeping and hand the session batch files, so the session never retypes data.
