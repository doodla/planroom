/** Text fitting for fixed-width surfaces: the pane, the band and the status line. */

const ELLIPSIS = "…";

/** `text` cut to at most `columns` code points, with an ellipsis where it was cut. */
export function truncate(text: string, columns: number): string {
  const points = Array.from(text);
  if (points.length <= columns) {
    return text;
  }
  if (columns <= 0) {
    return "";
  }
  return points.slice(0, columns - 1).join("") + ELLIPSIS;
}

/**
 * `template` with each `{name}` replaced by its value in `vars`, as the engine's `text::get` fills a
 * line: the template is read once, left to right, and a value is written as it is, never read
 * again, so a value that holds `{name}` stays as written. A `{name}` with no value stays too. It
 * doesn't sanitize: a surface passes the filled line through `sanitize`, as every line it draws.
 */
export function fill(
  template: string,
  vars: Readonly<Record<string, string | number>>,
): string {
  let out = "";
  let rest = template;
  for (;;) {
    const open = rest.indexOf("{");
    if (open < 0) break;
    out += rest.slice(0, open);
    const after = rest.slice(open + 1);
    const close = after.indexOf("}");
    const name = close < 0 ? null : after.slice(0, close);
    const value =
      name !== null && Object.prototype.hasOwnProperty.call(vars, name)
        ? vars[name]
        : undefined;
    if (value === undefined) {
      out += "{";
      rest = after;
    } else {
      out += String(value);
      rest = after.slice(close + 1);
    }
  }
  return out + rest;
}

/** How many columns `text` takes, counted in code points. */
export function width(text: string): number {
  return Array.from(text).length;
}
