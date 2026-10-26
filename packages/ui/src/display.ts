/**
 * The display-sanitizing pass [DR-64], the port of the engine's `kernel::display::sanitize`: C0
 * and C1 controls, DEL and ESC show as visible escapes such as `\x1b`, and bidi controls and
 * zero-width characters as markers such as `⟨U+202E⟩`. So a repo-derived name can't drive a
 * terminal or hide text in a label. Every pane row and canvas label passes through it.
 */

/** Bidi controls and zero-width characters, shown as `⟨U+XXXX⟩`. */
const INVISIBLE = new Set([
  0x061c, 0x200b, 0x200c, 0x200d, 0x200e, 0x200f, 0x202a, 0x202b, 0x202c,
  0x202d, 0x202e, 0x2060, 0x2066, 0x2067, 0x2068, 0x2069, 0xfeff,
]);

function isControl(code: number): boolean {
  return code < 0x20 || (code >= 0x7f && code <= 0x9f);
}

/** `text` with every control shown as an escape and every invisible character as a marker. */
export function sanitize(text: string): string {
  let out = "";
  for (const ch of text) {
    const code = ch.codePointAt(0) ?? 0;
    if (isControl(code)) {
      out += `\\x${code.toString(16).padStart(2, "0")}`;
    } else if (INVISIBLE.has(code)) {
      out += `⟨U+${code.toString(16).toUpperCase().padStart(4, "0")}⟩`;
    } else {
      out += ch;
    }
  }
  return out;
}
