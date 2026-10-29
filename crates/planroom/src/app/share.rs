//! `planroom share --out DIR`: the bundle the session publishes as an Artifact (architecture.md,
//! "Remote mode through the Artifact (S11)", step 1 and 2).
//!
//! It writes the embedded canvas build with a page fragment that selects the artifact transport,
//! `DIR/data/snapshot.json` (`planroom.snapshot/v1`: the top-level map with its layout positions
//! and the pane, redacted, with no evidence text unless `--include-snippets`), and the first
//! `live/head` document. Its result is the publish's exact arguments, whose `files` are relative to
//! one `root` so the reply keeps to the leaf's budget, and the `db` batch that seeds `live/head`.
//! It records the snapshot's `seq` in `meta.share_snapshot_seq`, which `import`'s
//! `live/head` extends.
//!
//! This slice precomputes only the top-level map. What the full snapshot adds in M4 (every level
//! for every lens, cards, diff summaries, plans, machines and the precomputed answers S1 to S6
//! need) is a read request the next sync answers.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use planroom_contract::remote::{LIVE_HEAD_FORMAT, SNAPSHOT_FORMAT, SNAPSHOT_PATH};
use planroom_contract::{
    CommandName, DbBatch, DbOp, DbWrite, Detail, Empty, EmptyReason, ErrorCode, Handle, LiveHead, MapReq,
    PublishArgs, ShareReq, ShareResult, SharedFile, Snapshot,
};
use planroom_kernel::redact;
use planroom_store::Store;
use serde_json::json;

use crate::app::error::AppError;
use crate::app::{Reply, data, focus, map, workspace};
use crate::server::canvas;
use crate::{clock, text};

/// The page fragment, published as the Artifact's page.
pub const PAGE: &str = "page.html";
/// The first `live/head` document, which the `db` batch writes; not a published file.
pub const LIVE_HEAD_FILE: &str = "db/live-head.json";
/// Where the published snapshot's `seq` is kept.
pub const SNAPSHOT_SEQ_META: &str = "share_snapshot_seq";

pub fn run(req: &ShareReq, cwd: &Path) -> Result<Reply<ShareResult>, AppError> {
    let root = workspace::find_root(cwd).ok_or_else(|| AppError::workspace_not_found(cwd))?;
    let mut store = Store::open_existing(&workspace::store_path(&root))?
        .ok_or_else(|| AppError::workspace_not_found(&root))?;
    crate::app::hook(&mut store);
    let index = canvas::file("index.html").ok_or_else(no_canvas)?;
    let index = std::str::from_utf8(index.0).map_err(AppError::internal)?;
    let page = fragment(index)?;

    let snapshot = snapshot(&store, &root, req)?;
    let bytes = serde_json::to_vec(&snapshot).map_err(AppError::internal)?;
    let head = LiveHead {
        format: LIVE_HEAD_FORMAT.into(),
        seq: snapshot.seq,
        snapshot_seq: snapshot.seq,
        cards: snapshot.cards.clone(),
        changes: vec![],
        conflicts: vec![],
        extra: Default::default(),
    };

    let out = out_dir(cwd, &req.out)?;
    let mut files = BTreeMap::new();
    let mut written = Vec::new();
    // The result lists, with their sizes, only the files made from this workspace: the canvas
    // build's assets are the same for every share, and listing each twice would pass the budget.
    let mut put = |rel: &str, body: &[u8], publish: bool, made: bool| -> Result<(), AppError> {
        let path = out.join(rel);
        if let Some(parent) = path.parent() {
            data::ensure_dir(parent)?;
        }
        data::write_private(&path, body)?;
        if made {
            written.push(SharedFile { path: rel.to_owned(), bytes: body.len() as u64 });
        }
        if publish {
            files.insert(rel.to_owned(), rel.to_owned());
        }
        Ok(())
    };
    put(PAGE, page.as_bytes(), false, true)?;
    for (rel, body) in canvas::files().filter(|(rel, _)| *rel != "index.html") {
        put(rel, body, true, false)?;
    }
    put(SNAPSHOT_PATH, &bytes, true, true)?;
    let head_bytes = serde_json::to_vec(&head).map_err(AppError::internal)?;
    put(LIVE_HEAD_FILE, &head_bytes, false, true)?;

    let held = store.meta(SNAPSHOT_SEQ_META)?;
    store.meta_swap(SNAPSHOT_SEQ_META, held.as_deref(), &snapshot.seq.to_string(), &[])?;

    let workspace_name = root.file_name().and_then(|n| n.to_str()).unwrap_or("workspace");
    let repos = store.active_set()?.repos.len();
    let blocks = snapshot.maps.first().map_or(0, |m| m.nodes.len());
    let summary = text::get(
        "share.summary",
        &[
            ("repos", &repos),
            ("blocks", &blocks),
            ("code", &text::get(if req.include_snippets { "share.code" } else { "share.no_code" }, &[])),
            (
                "presence",
                &text::get(if req.include_presence { "share.presence" } else { "share.no_presence" }, &[]),
            ),
        ],
    );
    let publish = PublishArgs {
        file_path: out.join(PAGE).display().to_string(),
        root: out.display().to_string(),
        files,
        capabilities: capabilities(),
        title: text::get("share.title", &[]),
        description: text::get("share.description", &[("workspace", &redact::text(workspace_name))]),
        icon: "map".into(),
    };
    let db = vec![DbBatch {
        writes: vec![DbWrite {
            op: DbOp::Set,
            collection: "live".into(),
            doc_id: "head".into(),
            file_path: Some(out.join(LIVE_HEAD_FILE).display().to_string()),
            if_version: None,
        }],
    }];
    let result =
        ShareResult { summary: summary.clone(), publish, db, files: written, holds: snapshot.capabilities };
    Ok(Reply { result, seq: snapshot.seq, warnings: vec![], text: summary })
}

/// The snapshot: the top-level map with its cached layout and the pane, redacted.
fn snapshot(store: &Store, root: &Path, req: &ShareReq) -> Result<Snapshot, AppError> {
    // The map, the pane, and the `seq` the snapshot records all come from one read transaction,
    // so the snapshot never holds an event past its `seq`, nor a pane from another version of the
    // store than its map (DR-73).
    let read = store.snapshot()?;
    let map = map::for_canvas_in(&read, root, &MapReq { detail: Detail::Graph, ..MapReq::default() })?;
    let seq = map.reply.seq;
    let mut top = map.reply.result;
    let mut pane = focus::reply_in(store, &read, root)?.result;
    let workspace_id = read.meta("workspace_id")?.unwrap_or_default();
    drop(read);
    redact::map_view(&mut top, req.include_snippets);
    let who = req.include_presence.then(|| pane.who.clone());
    if !req.include_presence {
        pane.who.clear();
    }
    redact::pane(&mut pane);
    let who = who.map(|mut rows| {
        redact::presence(&mut rows);
        rows
    });
    let capabilities = vec![format!("map:{}:{}", top.level.as_str(), top.lens), "pane".into()];
    Ok(Snapshot {
        format: SNAPSHOT_FORMAT.into(),
        workspace_id,
        seq,
        published_at: clock::now(),
        snippets: req.include_snippets,
        maps: vec![top],
        cards: pane.decision.iter().cloned().collect(),
        who,
        pane,
        answers: vec![],
        capabilities,
        unavailable: Empty {
            reason: EmptyReason("not_in_snapshot".into()),
            line: None,
            next: Handle {
                command: CommandName("map".into()),
                args: vec![],
                label: text::label("share.unavailable", &[]),
            },
        },
        extra: Default::default(),
    })
}

/// The page's capabilities (contracts.md, "Artifact db layout and event format"): only the
/// session writes shared state, only the owner reads `inbox`, and each viewer writes only under
/// `inbox/{self}`. `user` with `profile` gives the page the viewer's ID for that path. `sample` and
/// `comments` join when the page offers Ask Claude and Send to Claude.
pub fn capabilities() -> serde_json::Value {
    json!({
        "db": {"rules": [
            {"path": "", "read": "view", "write": "owner"},
            {"path": "inbox", "read": "owner", "write": "owner"},
            {"path": "inbox/{self}", "read": "interact", "write": "interact"},
        ]},
        "user": {"scopes": ["profile"]},
    })
}

/// The Vite page as an Artifact page fragment: the platform wraps the page in its own document,
/// so the fragment keeps only the title, the stylesheets, the root marked for the artifact
/// transport and the module script, all by relative path [artifact §5].
fn fragment(index: &str) -> Result<String, AppError> {
    let tags = |open: &str, close: &str| -> Vec<String> {
        let mut found = Vec::new();
        let mut rest = index;
        while let Some(start) = rest.find(open) {
            let tail = &rest[start..];
            let Some(end) = tail.find(close) else { break };
            found.push(tail[..end + close.len()].to_owned());
            rest = &tail[end + close.len()..];
        }
        found
    };
    let mut links = tags("<link ", ">");
    // The design tokens first, so the felt paints before the app's own stylesheet loads.
    links.sort_by_key(|l| !l.contains("tokens.css"));
    let scripts = tags("<script ", "</script>");
    if scripts.is_empty() {
        return Err(AppError::internal("the canvas build's index.html has no module script"));
    }
    let title = text::get("share.title", &[]);
    let mut page = format!("<title>{title}</title>\n");
    for link in links {
        page.push_str(&link);
        page.push('\n');
    }
    page.push_str("<div id=\"root\" data-transport=\"artifact\"></div>\n");
    for script in scripts {
        page.push_str(&script);
        page.push('\n');
    }
    Ok(page)
}

/// `--out`, made absolute against the working directory.
fn out_dir(cwd: &Path, out: &str) -> Result<PathBuf, AppError> {
    if out.trim().is_empty() {
        return Err(bad_out(&"empty"));
    }
    let path = cwd.join(out);
    data::ensure_dir(&path).map_err(|e| bad_out(&e.message.clone()))?;
    path.canonicalize().map_err(|e| bad_out(&e))
}

fn bad_out(why: &dyn std::fmt::Display) -> AppError {
    AppError::new(ErrorCode::InvalidArgument, &[("what", &text::get("argument.share_out", &[("why", why)]))])
        .details(json!({"field": "out"}))
}

/// `not_found` naming the canvas build's fix, as `serve`'s fix page does (DR-69).
fn no_canvas() -> AppError {
    let what = text::get("share.no_canvas", &[]);
    AppError::new(ErrorCode::NotFound, &[("what", &what)])
        .details(json!({"closest": null}))
        .with_hint(canvas::fix_line())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_fragment_keeps_the_title_styles_root_and_module_script() {
        let index = r#"<!doctype html>
<html lang="en"><head><meta charset="utf-8" /><title>Planroom</title>
<script type="module" crossorigin src="./assets/index.js"></script>
<link rel="stylesheet" crossorigin href="./assets/index.css">
<link rel="stylesheet" href="./assets/tokens.css">
</head><body><div id="root"></div></body></html>"#;
        let page = fragment(index).unwrap();
        assert_eq!(
            page,
            "<title>Planroom Canvas</title>\n\
             <link rel=\"stylesheet\" href=\"./assets/tokens.css\">\n\
             <link rel=\"stylesheet\" crossorigin href=\"./assets/index.css\">\n\
             <div id=\"root\" data-transport=\"artifact\"></div>\n\
             <script type=\"module\" crossorigin src=\"./assets/index.js\"></script>\n"
        );
        assert!(!page.contains("<!doctype") && !page.contains("<html") && !page.contains("<body"));
    }
}
