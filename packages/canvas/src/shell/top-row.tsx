/**
 * The top row (design-language.md, "Layout" and "Touch"): the breadcrumb of stacked lids with
 * search inside the current lid (Levels), the yarn-color lens switcher (Lenses) and the project
 * shelf (Intent). On the phone it holds at most four targets and never wraps.
 */

import { blocksOf, matchBlocks, sanitize } from "@planroom/ui";
import { useMemo, useRef, useState } from "react";

import { crumbsAt, type LevelCrumb } from "../levels";
import { STRINGS } from "../strings";
import { PHONE, useCanvas, useMedia } from "../use-canvas";
import { CloseIcon, SearchIcon, YarnIcon } from "./icons";
import { useTopName } from "./next-action";
import { Picker } from "./picker";

/** The last part of an ID, `plan:checkout:retries` reads `retries`. */
function idKey(id: string): string {
  return sanitize(id.slice(id.lastIndexOf(":") + 1));
}

/**
 * The lids above the current one. On the desktop each is a lid that goes back up; on the phone
 * one `⋯` lid lists them, so the row keeps four targets (design-language.md, "Touch").
 */
function Ancestors({ crumbs }: { crumbs: readonly LevelCrumb[] }) {
  const showLevel = useCanvas((s) => s.showLevel);
  const back = useCanvas((s) => s.back);
  const phone = useMedia(PHONE);
  if (crumbs.length === 0) {
    return null;
  }
  const go = (c: LevelCrumb) =>
    void (c.level === null ? back() : showLevel(c.level));
  if (phone) {
    return (
      <Picker
        concept="levels"
        label={STRINGS.ancestors}
        className="lid-more"
        buttonClass="lid lid-ancestor lid-more-button"
        entries={crumbs.map((c) => ({
          key: c.level ?? "",
          label: c.label,
          current: false,
        }))}
        onPick={(key) => {
          const c = crumbs.find((x) => (x.level ?? "") === key);
          if (c) go(c);
        }}
      >
        <span aria-hidden="true">⋯</span>
      </Picker>
    );
  }
  return (
    <>
      {crumbs.map((c) => (
        <button
          key={c.level ?? ""}
          type="button"
          className="lid lid-ancestor"
          onClick={() => go(c)}
        >
          {c.label}
        </button>
      ))}
    </>
  );
}

function Breadcrumb() {
  const map = useCanvas((s) => s.map);
  // The level asked for, even while its view waits on a request; the top when none was.
  const level = useCanvas((s) => s.level);
  const query = useCanvas((s) => s.query);
  const search = useCanvas((s) => s.search);
  const reveal = useCanvas((s) => s.reveal);
  const phone = useMedia(PHONE);
  const [searching, setSearching] = useState(false);
  const input = useRef<HTMLInputElement>(null);
  const rows = useMemo(() => (map ? blocksOf(map) : []), [map]);
  const top = useTopName();

  // From the map's level, not the pane's crumb, which names the workspace at every level (ux-4).
  const crumbs = crumbsAt(level, top);
  const current = crumbs.at(-1);
  const ancestors = crumbs.slice(0, -1);
  const showInput = !phone || searching;

  const submit = () => {
    const first = rows.find(
      (r) => matchBlocks([r], query).size > 0 && query.trim() !== "",
    );
    if (first) {
      reveal(first.id);
    }
  };

  return (
    <nav
      className="breadcrumb"
      data-concept="levels"
      aria-label={STRINGS.levels}
    >
      <Ancestors crumbs={ancestors} />
      <div
        className="lid lid-current"
        aria-current="location"
        data-searching={searching || undefined}
      >
        {!(phone && searching) && (
          <span className="lid-name">{current?.label ?? ""}</span>
        )}
        {showInput && (
          <input
            ref={input}
            className="lid-search"
            type="search"
            placeholder={STRINGS.openSearch}
            aria-label={STRINGS.search}
            value={query}
            onChange={(e) => search(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") {
                submit();
              } else if (e.key === "Escape") {
                search("");
                setSearching(false);
              }
            }}
          />
        )}
        {phone && (
          <button
            type="button"
            className="icon-button"
            aria-label={searching ? STRINGS.closeSearch : STRINGS.search}
            onClick={() => {
              if (searching) {
                search("");
              }
              setSearching(!searching);
              setTimeout(() => input.current?.focus(), 0);
            }}
          >
            {searching ? <CloseIcon /> : <SearchIcon />}
          </button>
        )}
      </div>
    </nav>
  );
}

export function TopRow() {
  const map = useCanvas((s) => s.map);
  const pane = useCanvas((s) => s.pane);
  const phone = useMedia(PHONE);
  const lens = map?.lens ?? pane?.focus.lens ?? "";
  const project = pane?.focus.project ?? null;
  return (
    <header className="top-row">
      <Breadcrumb />
      <Picker
        concept="lenses"
        label={STRINGS.lenses}
        className="lens-switcher"
        entries={[{ key: lens, label: sanitize(lens), current: true }]}
      >
        <YarnIcon />
        {!phone && <span className="picker-label">{sanitize(lens)}</span>}
      </Picker>
      <Picker
        concept="intent"
        label={STRINGS.projects}
        className="project-shelf"
        entries={
          project
            ? [{ key: project, label: idKey(project), current: true }]
            : []
        }
      >
        <span className="chip-label">
          {project ? idKey(project) : STRINGS.noProject}
        </span>
      </Picker>
    </header>
  );
}
