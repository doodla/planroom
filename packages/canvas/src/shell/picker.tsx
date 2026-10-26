/**
 * A control that deals a sheet of entries, the current one marked: the lens switcher and the
 * project shelf. Escape or a press outside closes it.
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
