/**
 * A control that deals a sheet of entries, the current one marked: the `⋯` lid, the lens switcher
 * and the project shelf. Escape or a press outside closes it.
 *
 * A picker with nothing to pick, no entry but the current one or no entries at all, isn't a
 * button: it shows what it holds, so no press opens a blank sheet or one that does nothing
 * (design-language.md, "Empty states"; DR-85, "Surfaces offer only what works").
 */

import { useEffect, useId, useRef, useState, type ReactNode } from "react";

export interface PickerEntry {
  readonly key: string;
  readonly label: string;
  readonly current: boolean;
}

export function Picker({
  concept,
  label,
  className,
  children,
  entries,
  onPick,
  buttonClass = "button picker-button",
}: {
  concept: string;
  label: string;
  className: string;
  children: ReactNode;
  entries: readonly PickerEntry[];
  /** What a press on an entry does, by its key; the sheet closes either way. */
  onPick?: (key: string) => void;
  buttonClass?: string;
}) {
  const [open, setOpen] = useState(false);
  const root = useRef<HTMLDivElement>(null);
  const id = useId();
  useEffect(() => {
    if (!open) {
      return;
    }
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && setOpen(false);
    const onDown = (e: PointerEvent) => {
      if (!root.current?.contains(e.target as Node)) {
        setOpen(false);
      }
    };
    document.addEventListener("keydown", onKey);
    document.addEventListener("pointerdown", onDown);
    return () => {
      document.removeEventListener("keydown", onKey);
      document.removeEventListener("pointerdown", onDown);
    };
  }, [open]);
  if (onPick === undefined || !entries.some((e) => !e.current)) {
    return (
      <div
        className={`picker picker-static ${className}`}
        data-concept={concept}
      >
        <span className="picker-face">
          <span className="visually-hidden">{label}: </span>
          {children}
        </span>
      </div>
    );
  }
  return (
    <div className={`picker ${className}`} data-concept={concept} ref={root}>
      <button
        type="button"
        className={buttonClass}
        aria-label={label}
        aria-expanded={open}
        aria-controls={id}
        onClick={() => setOpen(!open)}
      >
        {children}
      </button>
      {open && (
        <div
          className="picker-sheet panel"
          id={id}
          role="dialog"
          aria-label={label}
        >
          <ul>
            {entries.map((e) => (
              <li key={e.key}>
                <button
                  type="button"
                  className="picker-entry"
                  aria-current={e.current || undefined}
                  onClick={() => {
                    setOpen(false);
                    onPick?.(e.key);
                  }}
                >
                  {e.label}
                </button>
              </li>
            ))}
          </ul>
        </div>
      )}
    </div>
  );
}
