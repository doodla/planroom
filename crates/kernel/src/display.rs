//! The display-sanitizing pass [DR-64]: every human-facing string the engine prints passes
//! through `sanitize`, and `@planroom/ui`'s `sanitize` (packages/ui/src/display.ts) is its port,
//! with the same markers, so both surfaces agree.
//!
//! C0 and C1 controls, DEL and ESC show as visible escapes such as `\x1b`, and bidi controls and
//! zero-width characters as markers such as `⟨U+202E⟩`. So a repo-derived name can't drive a
//! terminal (an OSC 52 clipboard write, say) or hide text in a line. The pass is idempotent: its
//! output holds no character it would change.

/// Bidi controls and zero-width characters, shown as `⟨U+XXXX⟩`.
const INVISIBLE: [u32; 17] = [
    0x061c, 0x200b, 0x200c, 0x200d, 0x200e, 0x200f, 0x202a, 0x202b, 0x202c, 0x202d, 0x202e, 0x2060, 0x2066,
    0x2067, 0x2068, 0x2069, 0xfeff,
];

fn is_control(code: u32) -> bool {
    code < 0x20 || (0x7f..=0x9f).contains(&code)
}

/// `text` with every control shown as an escape and every invisible character as a marker.
pub fn sanitize(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        let code = u32::from(ch);
        if is_control(code) {
            out.push_str(&format!("\\x{code:02x}"));
        } else if INVISIBLE.contains(&code) {
            out.push_str(&format!("⟨U+{code:04X}⟩"));
        } else {
            out.push(ch);
        }
    }
    out
}

/// Each line of `text` sanitized, the line breaks between them kept: for a multi-line text reply.
pub fn sanitize_lines(text: &str) -> String {
    text.split('\n').map(sanitize).collect::<Vec<_>>().join("\n")
}
