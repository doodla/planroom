/** The few glyphs the shell draws, inline so they inherit the text color. */

const common = {
  width: 20,
  height: 20,
  viewBox: "0 0 20 20",
  fill: "none",
  stroke: "currentColor",
  strokeWidth: 2,
  strokeLinecap: "round" as const,
  strokeLinejoin: "round" as const,
  "aria-hidden": true,
};

export function SearchIcon() {
  return (
    <svg {...common}>
      <circle cx="8.5" cy="8.5" r="5.5" />
      <path d="M13 13l4.5 4.5" />
    </svg>
  );
}

export function CloseIcon() {
  return (
    <svg {...common}>
      <path d="M5 5l10 10M15 5L5 15" />
    </svg>
  );
}

/** A loop of yarn: the lens switcher's swatch. */
export function YarnIcon() {
  return (
    <svg {...common} className="yarn">
      <path d="M3 14c2-6 5-9 7-6s1 6-2 5 1-7 5-8 4 3 4 5" />
    </svg>
  );
}

export function ChevronIcon({ up }: { up: boolean }) {
  return (
    <svg {...common}>
      <path d={up ? "M5 12l5-5 5 5" : "M5 8l5 5 5-5"} />
    </svg>
  );
}
