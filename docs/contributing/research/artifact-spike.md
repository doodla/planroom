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

The page saves its checks to `inbox/<viewer id>/probe/latest` the first time someone opens it: module script, relative fetch of the data file, `db` and `user` availability, a write to the viewer's own inbox, the write-to-server-confirmation time, and live `live/head` updates with their lag. The walking skeleton reads that document back and records the numbers here.

## Implications

- The access rules give the shape remote mode needs: only the session writes shared state, each viewer writes only under `inbox/<their id>`, and the path proves who wrote an edit.
- Session-side writes need `if_version` after the first write. The engine should own the version bookkeeping and hand the session batch files, so the session never retypes data.
