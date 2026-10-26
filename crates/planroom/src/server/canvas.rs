//! The canvas, embedded in the binary by `build.rs` from `target/canvas/` (DR-58). A binary built
//! without one serves one engine-rendered page naming the fix.

use std::path::Path;

use crate::text;

mod embedded {
    include!(concat!(env!("OUT_DIR"), "/canvas.rs"));
}

/// The embedded file at `path` (`index.html`, `assets/app.js`), with its content type.
pub fn file(path: &str) -> Option<(&'static [u8], &'static str)> {
    embedded::FILES.iter().find(|(p, _)| *p == path).map(|(p, bytes)| (*bytes, content_type(p)))
}

/// Whether this binary embeds a canvas build.
pub fn embedded() -> bool {
    file("index.html").is_some()
}

fn content_type(path: &str) -> &'static str {
    match path.rsplit('.').next().unwrap_or_default() {
        "html" => "text/html; charset=utf-8",
        "js" | "mjs" => "text/javascript; charset=utf-8",
        "css" => "text/css; charset=utf-8",
        "json" | "map" => "application/json",
        "svg" => "image/svg+xml",
        "png" => "image/png",
        "woff2" => "font/woff2",
        "woff" => "font/woff",
        "ico" => "image/x-icon",
        _ => "application/octet-stream",
    }
}

/// The fix line for a binary without a canvas: in the checkout it was built from, the canvas
/// build; anywhere else, the toolchain and the resolver's build.
pub fn fix_line() -> String {
    let checkout = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../Cargo.toml");
    let built_here = std::env::current_exe()
        .ok()
        .and_then(|exe| exe.canonicalize().ok())
        .zip(Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().ok())
        .is_some_and(|(exe, root)| exe.starts_with(root));
    if built_here && checkout.is_file() {
        text::get("canvas.missing_checkout", &[])
    } else {
        text::get("canvas.missing_install", &[])
    }
}

/// The engine-rendered page a binary without a canvas serves at `/`.
pub fn fix_page() -> String {
    let title = text::get("canvas.missing_title", &[]);
    let fix = html_escape(&fix_line());
    format!(
        "<!doctype html>\n<html lang=\"en\"><head><meta charset=\"utf-8\"><title>{title}</title>\
         <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\"></head>\
         <body><main><h1>{title}</h1><p>{fix}</p></main></body></html>\n"
    )
}

fn html_escape(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}
