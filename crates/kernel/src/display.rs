//! The display-sanitizing pass [DR-64]: every human-facing string the engine prints passes
//! through `sanitize`, and `@planroom/ui`'s `sanitize` (packages/ui/src/display.ts) is its port,
//! with the same markers, so both surfaces agree.
//!
//! C0 and C1 controls, DEL and ESC show as visible escapes such as `\x1b`. The characters that
//! draw nothing, or that change how others draw, show as markers such as `⟨U+202E⟩` or
//! `⟨U+E0041⟩`: bidi controls, zero-width characters and every other default-ignorable code point
//! (Unicode's `Default_Ignorable_Code_Point`, variation selectors and the Unicode tag characters,
//! U+E0000 to U+E007F, among them), every other format character (category Cf), and the line and
//! paragraph separators (U+2028, U+2029). So a repo-derived name can't drive a terminal (an OSC 52
//! clipboard write, say), break a line where none shows, or hide text in a line. The pass is
//! idempotent: its output holds no character it would change.

/// The characters shown as `⟨U+XXXX⟩`, as inclusive ranges: every code point Unicode 16 marks
/// `Default_Ignorable_Code_Point`, every other format character (category Cf), and the line and
/// paragraph separators. `@planroom/ui`'s list is the same.
const INVISIBLE: [(u32, u32); 30] = [
    // Unicode 16's `Default_Ignorable_Code_Point`: bidi controls, zero-width characters, fillers,
    // invisible operators and variation selectors.
    (0x00ad, 0x00ad),
    (0x034f, 0x034f),
    (0x061c, 0x061c),
    (0x115f, 0x1160),
    (0x17b4, 0x17b5),
    (0x180b, 0x180f),
    (0x200b, 0x200f),
    (0x202a, 0x202e),
    (0x2060, 0x206f),
    (0x3164, 0x3164),
    (0xfe00, 0xfe0f),
    (0xfeff, 0xfeff),
    (0xffa0, 0xffa0),
    (0xfff0, 0xfff8),
    (0x1bca0, 0x1bca3),
    (0x1d173, 0x1d17a),
    // The tag characters, which can spell hidden text as a subdivision flag's do, then the
    // variation selectors supplement and the rest of the block Unicode reserves as
    // default-ignorable.
    (0xe0000, 0xe007f),
    (0xe0080, 0xe00ff),
    (0xe0100, 0xe01ef),
    (0xe01f0, 0xe0fff),
    // The other format characters (Unicode category Cf): prepended concatenation marks, the
    // interlinear annotation characters and the Egyptian hieroglyph format controls.
    (0x0600, 0x0605),
    (0x06dd, 0x06dd),
    (0x070f, 0x070f),
    (0x0890, 0x0891),
    (0x08e2, 0x08e2),
    (0xfff9, 0xfffb),
    (0x110bd, 0x110bd),
    (0x110cd, 0x110cd),
    (0x13430, 0x1343f),
    // The line and paragraph separators.
    (0x2028, 0x2029),
];

fn is_control(code: u32) -> bool {
    code < 0x20 || (0x7f..=0x9f).contains(&code)
}

fn is_invisible(code: u32) -> bool {
    INVISIBLE.iter().any(|&(first, last)| (first..=last).contains(&code))
}

/// `text` with every control shown as an escape and every invisible character as a marker.
pub fn sanitize(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        let code = u32::from(ch);
        if is_control(code) {
            out.push_str(&format!("\\x{code:02x}"));
        } else if is_invisible(code) {
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
