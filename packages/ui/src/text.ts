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

/** How many columns `text` takes, counted in code points. */
export function width(text: string): number {
  return Array.from(text).length;
}
