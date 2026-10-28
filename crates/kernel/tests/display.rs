//! `kernel::display::sanitize`: controls show as escapes, and bidi, zero-width, separator and tag
//! characters as markers (DR-64). The cases after the first are `packages/ui/src/display.test.ts`'s,
//! and `display_parity` holds the same table as its `display_parity`, so the engine and its
//! TypeScript port agree on one table.

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

/// One table, the same rows as `packages/ui/src/display.test.ts`'s `display_parity`, so the engine
/// and its TypeScript port agree on every case (architecture.md, One display-sanitizing pass):
/// each input, what it shows as, and that showing it again changes nothing.
#[test]
fn display_parity() {
    let table: [(&str, &str); 75] = [
        // An OSC 52 clipboard write, ESC, DEL, a C1 control, a tab and a line feed.
        ("evil\x1b]52;c;aGVsbG8=\x07name", "evil\\x1b]52;c;aGVsbG8=\\x07name"),
        ("a\x1b[31mred", "a\\x1b[31mred"),
        ("del\x7f", "del\\x7f"),
        ("c1\u{9b}", "c1\\x9b"),
        ("tab\there\nnext", "tab\\x09here\\x0anext"),
        // Every bidi control and zero-width character.
        ("a\u{061C}b", "a⟨U+061C⟩b"),
        ("a\u{200B}b", "a⟨U+200B⟩b"),
        ("a\u{200C}b", "a⟨U+200C⟩b"),
        ("a\u{200D}b", "a⟨U+200D⟩b"),
        ("a\u{200E}b", "a⟨U+200E⟩b"),
        ("a\u{200F}b", "a⟨U+200F⟩b"),
        ("a\u{202A}b", "a⟨U+202A⟩b"),
        ("a\u{202B}b", "a⟨U+202B⟩b"),
        ("a\u{202C}b", "a⟨U+202C⟩b"),
        ("a\u{202D}b", "a⟨U+202D⟩b"),
        ("a\u{202E}b", "a⟨U+202E⟩b"),
        ("a\u{2060}b", "a⟨U+2060⟩b"),
        ("a\u{2066}b", "a⟨U+2066⟩b"),
        ("a\u{2067}b", "a⟨U+2067⟩b"),
        ("a\u{2068}b", "a⟨U+2068⟩b"),
        ("a\u{2069}b", "a⟨U+2069⟩b"),
        ("a\u{FEFF}b", "a⟨U+FEFF⟩b"),
        // The line and paragraph separators, and tag characters from both ends of their block.
        ("a\u{2028}b", "a⟨U+2028⟩b"),
        ("a\u{2029}b", "a⟨U+2029⟩b"),
        ("a\u{E0001}b", "a⟨U+E0001⟩b"),
        ("a\u{E0041}b", "a⟨U+E0041⟩b"),
        ("a\u{E007F}b", "a⟨U+E007F⟩b"),
        // A run of tag characters spelling hidden text after visible text.
        ("ok\u{E0069}\u{E0067}\u{E006E}", "ok⟨U+E0069⟩⟨U+E0067⟩⟨U+E006E⟩"),
        // Every other code point Unicode marks default-ignorable, variation selectors and the
        // unassigned ones included, and the interlinear annotation characters, from both ends of
        // each run: each draws nothing, so each can hide text.
        ("a\u{00AD}b", "a⟨U+00AD⟩b"),
        ("a\u{034F}b", "a⟨U+034F⟩b"),
        ("a\u{115F}b", "a⟨U+115F⟩b"),
        ("a\u{1160}b", "a⟨U+1160⟩b"),
        ("a\u{17B4}b", "a⟨U+17B4⟩b"),
        ("a\u{17B5}b", "a⟨U+17B5⟩b"),
        ("a\u{180B}b", "a⟨U+180B⟩b"),
        ("a\u{180E}b", "a⟨U+180E⟩b"),
        ("a\u{180F}b", "a⟨U+180F⟩b"),
        ("a\u{2061}b", "a⟨U+2061⟩b"),
        ("a\u{2064}b", "a⟨U+2064⟩b"),
        ("a\u{2065}b", "a⟨U+2065⟩b"),
        ("a\u{206A}b", "a⟨U+206A⟩b"),
        ("a\u{206F}b", "a⟨U+206F⟩b"),
        ("a\u{3164}b", "a⟨U+3164⟩b"),
        ("a\u{FE00}b", "a⟨U+FE00⟩b"),
        ("a\u{FE0F}b", "a⟨U+FE0F⟩b"),
        ("a\u{FFA0}b", "a⟨U+FFA0⟩b"),
        ("a\u{FFF0}b", "a⟨U+FFF0⟩b"),
        ("a\u{FFF8}b", "a⟨U+FFF8⟩b"),
        ("a\u{FFF9}b", "a⟨U+FFF9⟩b"),
        ("a\u{FFFB}b", "a⟨U+FFFB⟩b"),
        ("a\u{1BCA0}b", "a⟨U+1BCA0⟩b"),
        ("a\u{1BCA3}b", "a⟨U+1BCA3⟩b"),
        ("a\u{1D173}b", "a⟨U+1D173⟩b"),
        ("a\u{1D17A}b", "a⟨U+1D17A⟩b"),
        ("a\u{E0080}b", "a⟨U+E0080⟩b"),
        ("a\u{E0100}b", "a⟨U+E0100⟩b"),
        ("a\u{E01EF}b", "a⟨U+E01EF⟩b"),
        ("a\u{E0FFF}b", "a⟨U+E0FFF⟩b"),
        // The other format characters (Unicode category Cf), from both ends of each run.
        ("a\u{0600}b", "a⟨U+0600⟩b"),
        ("a\u{0605}b", "a⟨U+0605⟩b"),
        ("a\u{06DD}b", "a⟨U+06DD⟩b"),
        ("a\u{070F}b", "a⟨U+070F⟩b"),
        ("a\u{0890}b", "a⟨U+0890⟩b"),
        ("a\u{0891}b", "a⟨U+0891⟩b"),
        ("a\u{08E2}b", "a⟨U+08E2⟩b"),
        ("a\u{110BD}b", "a⟨U+110BD⟩b"),
        ("a\u{110CD}b", "a⟨U+110CD⟩b"),
        ("a\u{13430}b", "a⟨U+13430⟩b"),
        ("a\u{1343F}b", "a⟨U+1343F⟩b"),
        // An emoji's presentation selector shows as a marker too: a run of them can carry hidden bytes.
        ("\u{2764}\u{FE0F}", "\u{2764}⟨U+FE0F⟩"),
        // Their visible neighbours pass through unchanged.
        ("x\u{00AC}\u{2070}\u{FFFC}\u{1D172}", "x\u{00AC}\u{2070}\u{FFFC}\u{1D172}"),
        // Single-code-point emoji and accented text pass through unchanged.
        ("orders-db · café 🧱", "orders-db · café 🧱"),
        ("naïve résumé", "naïve résumé"),
        // A subdivision flag's tag characters show as markers: a tag can hide text.
        (
            "\u{1F3F4}\u{E0067}\u{E0062}\u{E0073}\u{E0063}\u{E0074}\u{E007F}",
            "\u{1F3F4}⟨U+E0067⟩⟨U+E0062⟩⟨U+E0073⟩⟨U+E0063⟩⟨U+E0074⟩⟨U+E007F⟩",
        ),
        // A ZWJ sequence shows its joiner.
        ("\u{1F469}\u{200D}\u{1F4BB}", "\u{1F469}⟨U+200D⟩\u{1F4BB}"),
    ];
    for (input, shown) in table {
        assert_eq!(sanitize(input), shown, "{input:?}");
        assert_eq!(sanitize(shown), shown, "the pass is idempotent on {shown:?}");
    }
}
