/**
 * A control that deals a sheet of entries, the current one marked: the `⋯` lid, the lens switcher
 * and the project shelf. Escape or a press outside closes it.
 *
 * A picker with nothing to pick, no entry but the current one or no entries at all, isn't a
 * button unless it carries `extra`, a control drawn in the sheet below the entries. Entries draw as
 * buttons only when one can be picked; otherwise they are flat, the current one marked, so no press
 * in the sheet does nothing (design-language.md, "Empty states"; DR-85, "Surfaces offer only what
 * works"; DR-86 as DR-122 amends it). The sheet takes focus on open, on its own current entry
 * first and on `extra` only when no entry can be pressed, and gives it back to the picker on Escape
 * or a pick.
 */

import { useEffect, useId, useRef, useState, type ReactNode } from "react";

export interface PickerEntry {
  readonly key: string;
  readonly label: string;
  readonly current: boolean;
}

/** The one part of an element the first focus reads, so a test checks the order without a DOM. */
export type FocusScope = Pick<ParentNode, "querySelector">;

/**
 * Where the focus lands when the sheet opens: the picker's own current entry, else its first
 * entry button; only when no entry can be pressed, the checked control in `extra`, else its first
 * button. So a lens list keeps the focus ahead of the theme choice below it.
 */
export function firstFocus(
  list: FocusScope | null,
  extra: FocusScope | null,
): HTMLElement | null {
  return (
    list?.querySelector<HTMLElement>("button[aria-current]") ??
    list?.querySelector<HTMLElement>("button") ??
    extra?.querySelector<HTMLElement>('[aria-checked="true"]') ??
    extra?.querySelector<HTMLElement>("button") ??
    null
  );
}

export function Picker({
  concept,
  label,
  className,
  children,
  entries,
  onPick,
  extra,
  buttonClass = "button picker-button",
}: {
  concept: string;
  label: string;
  className: string;
  children: ReactNode;
  entries: readonly PickerEntry[];
  /** What a press on an entry does, by its key; the sheet closes either way. */
  onPick?: (key: string) => void;
  /** Drawn in the sheet below the entries; a picker with `extra` is always a button. */
  extra?: ReactNode;
  buttonClass?: string;
}) {
  const [open, setOpen] = useState(false);
  const root = useRef<HTMLDivElement>(null);
  const trigger = useRef<HTMLButtonElement>(null);
  const list = useRef<HTMLUListElement>(null);
  const extraBox = useRef<HTMLDivElement>(null);
  const id = useId();
  const pickable = onPick !== undefined && entries.some((e) => !e.current);
  const close = (refocus: boolean) => {
    setOpen(false);
    if (refocus) {
      trigger.current?.focus();
    }
  };
  useEffect(() => {
    if (!open) {
      return;
    }
    firstFocus(list.current, extraBox.current)?.focus();
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && close(true);
    const onDown = (e: PointerEvent) => {
      if (!root.current?.contains(e.target as Node)) {
        close(false);
      }
    };
    document.addEventListener("keydown", onKey);
    document.addEventListener("pointerdown", onDown);
    return () => {
      document.removeEventListener("keydown", onKey);
      document.removeEventListener("pointerdown", onDown);
    };
  }, [open]);
  if (!pickable && extra === undefined) {
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
        ref={trigger}
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
          {entries.length > 0 && (
            <ul ref={list}>
              {entries.map((e) => (
                <li key={e.key}>
                  {pickable ? (
                    <button
                      type="button"
                      className="picker-entry"
                      aria-current={e.current || undefined}
                      onClick={() => {
                        close(true);
                        onPick?.(e.key);
                      }}
                    >
                      {e.label}
                    </button>
                  ) : (
                    <span
                      className="picker-entry picker-entry-flat"
                      aria-current={e.current || undefined}
                    >
                      {e.label}
                    </span>
                  )}
                </li>
              ))}
            </ul>
          )}
          {extra !== undefined && (
            <div
              className="picker-extra"
              ref={extraBox}
              onClick={(e) => {
                // A control inside `extra` has run its own handler by now; the sheet then closes.
                if ((e.target as Element).closest("button")) {
                  close(true);
                }
              }}
            >
              {extra}
            </div>
          )}
        </div>
      )}
    </div>
  );
}
