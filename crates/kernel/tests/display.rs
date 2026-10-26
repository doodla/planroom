//! `kernel::display::sanitize`: controls and bidi characters show as markers (DR-64). The cases
//! after the first are `packages/ui/src/display.test.ts`'s, so the engine and its TypeScript port
//! agree on one table.

use planroom_kernel::display::{sanitize, sanitize_lines};

#[test]
fn controls_and_bidi_show_as_markers() {
    // An OSC 52 clipboard write and an RLO in a compose service's name, as a scanned repo can hold.
    let name = "evil\x1b]52;c;aGVsbG8=\x07name\u{202e}abc";
    let shown = sanitize(name);
    assert_eq!(shown, "evil\\x1b]52;c;aGVsbG8=\\x07name⟨U+202E⟩abc");
    assert!(!shown.chars().any(|c| c.is_control()), "{shown:?}");
    // A Trojan Source hunk: the isolate and override characters show where they hide code.
    let hunk = "if (isAdmin) { \u{202e} \u{2066}// check later\u{2069} \u{2066}";
    assert_eq!(sanitize(hunk), "if (isAdmin) { ⟨U+202E⟩ ⟨U+2066⟩// check later⟨U+2069⟩ ⟨U+2066⟩");
}

#[test]
fn plain_text_passes_through_emoji_and_accents_included() {
    assert_eq!(sanitize("orders-db · café 🧱"), "orders-db · café 🧱");
}

#[test]
fn c0_del_c1_and_esc_show_as_visible_escapes() {
    assert_eq!(sanitize("a\x1b[31mred"), "a\\x1b[31mred");
    assert_eq!(sanitize("tab\there\nnext"), "tab\\x09here\\x0anext");
    assert_eq!(sanitize("del\x7f c1\u{9b}"), "del\\x7f c1\\x9b");
}

#[test]
fn bidi_controls_and_zero_width_characters_show_as_markers() {
    assert_eq!(sanitize("evil\u{202E}txt.exe"), "evil⟨U+202E⟩txt.exe");
    assert_eq!(sanitize("zero\u{200B}width\u{FEFF}"), "zero⟨U+200B⟩width⟨U+FEFF⟩");
    assert_eq!(sanitize("\u{2066}isolate\u{2069}"), "⟨U+2066⟩isolate⟨U+2069⟩");
}

#[test]
fn the_pass_is_idempotent_and_keeps_line_breaks_between_lines() {
    let once = sanitize("a\x1b]52;c;x\x07\u{202e}");
    assert_eq!(sanitize(&once), once);
    assert_eq!(sanitize_lines("one\x1b\ntwo\u{202e}"), "one\\x1b\ntwo⟨U+202E⟩");
}
