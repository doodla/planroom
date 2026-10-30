/**
 * The theme choice on the lens switcher's sheet: System, Light or Dark in a radio group. It is on
 * the local canvas only; on the Artifact page there is no host, so it draws nothing and claude.ai's
 * theme stands [DR-122].
 */

import { THEME_PREFERENCES, type ThemePreference } from "@planroom/ui";
import { useId, useRef, type KeyboardEvent } from "react";

import { CANVAS_STRINGS } from "@planroom/contract";
import { useThemeChoice } from "../theme";

export function ThemeChoice() {
  const choice = useThemeChoice();
  const id = useId();
  const group = useRef<HTMLDivElement>(null);
  if (choice === null) {
    return null;
  }
  const labels: Record<ThemePreference, string> = {
    system: CANVAS_STRINGS.theme_system,
    light: CANVAS_STRINGS.theme_light,
    dark: CANVAS_STRINGS.theme_dark,
  };
  // Arrows move focus only: a choice closes the sheet, so choosing as the focus moved would shut
  // it on the first press. Enter, Space or a press chooses.
  const onKey = (e: KeyboardEvent<HTMLDivElement>) => {
    const radios = [
      ...(group.current?.querySelectorAll<HTMLElement>('[role="radio"]') ?? []),
    ];
    const at = radios.indexOf(document.activeElement as HTMLElement);
    if (radios.length === 0 || at < 0) {
      return;
    }
    let next: number;
    switch (e.key) {
      case "ArrowRight":
      case "ArrowDown":
        next = (at + 1) % radios.length;
        break;
      case "ArrowLeft":
      case "ArrowUp":
        next = (at - 1 + radios.length) % radios.length;
        break;
      case "Home":
        next = 0;
        break;
      case "End":
        next = radios.length - 1;
        break;
      default:
        return;
    }
    e.preventDefault();
    radios[next]?.focus();
  };
  return (
    <div className="theme-choice">
      <p className="theme-choice-label" id={id}>
        {CANVAS_STRINGS.theme}
      </p>
      <div
        role="radiogroup"
        aria-labelledby={id}
        className="theme-choice-radios"
        ref={group}
        onKeyDown={onKey}
      >
        {THEME_PREFERENCES.map((p) => (
          <button
            key={p}
            type="button"
            role="radio"
            className="theme-choice-radio"
            aria-checked={p === choice.preference}
            tabIndex={p === choice.preference ? 0 : -1}
            onClick={() => choice.choose(p)}
          >
            {labels[p]}
          </button>
        ))}
      </div>
    </div>
  );
}
