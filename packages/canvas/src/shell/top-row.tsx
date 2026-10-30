/**
 * The top row (design-language.md, "Layout" and "Touch"): the breadcrumb of stacked lids with
 * search inside the current lid (Levels) and, on the desktop, an open lid for the selected block
 * that has blocks to go into, the yarn-color lens switcher (Lenses), whose sheet lists
 * the lenses `ask` lists and also holds the theme choice on the local canvas, and the project shelf
 * (Intent). On the phone it holds at most four targets and never wraps.
 */

import { CANVAS_STRINGS } from "@planroom/contract";
import { blocksOf, fill, matchBlocks, sanitize, yarnVar } from "@planroom/ui";
import { useMemo, useRef, useState, type CSSProperties } from "react";

import { crumbsAt } from "../levels";
import { lensEntries, lensLabel } from "../lens-switcher";
import { useThemeChoice } from "../theme";
import { PHONE, useCanvas, useMedia } from "../use-canvas";
import { CloseIcon, SearchIcon, YarnIcon } from "./icons";
import { useTopName } from "./next-action";
import { Picker } from "./picker";
import { ThemeChoice } from "./theme-choice";

/** One lid of the breadcrumb: the top, then a block of the trail or a level of the chain. */
interface Lid {
  readonly key: string;
  readonly label: string;
  readonly go: () => void;
}

/**
 * The breadcrumb's lids as one list, the top first and the current lid last. Under a focus the
 * lids after the top are the blocks the person went into; otherwise they are the chain's levels
 * down to the shown one, from the map's own level, not the pane's crumb, which names the workspace
 * at every level (ux-4).
 */
function useLids(): Lid[] {
  const map = useCanvas((s) => s.map);
  const trail = useCanvas((s) => s.trail);
  const back = useCanvas((s) => s.back);
  const popTo = useCanvas((s) => s.popTo);
  const showLevel = useCanvas((s) => s.showLevel);
  const top = useTopName();
  const first: Lid = { key: "top", label: top, go: () => void back() };
  if (trail.length > 0) {
    return [
      first,
      ...trail.map((entry, i) => ({
        key: `trail:${entry.id}`,
        label: sanitize(entry.name),
        go: () => void popTo(i),
      })),
    ];
  }
  return crumbsAt(map?.levels ?? [], map?.level ?? null, top).map((c) =>
    c.level === null
      ? first
      : {
          key: `level:${c.level}`,
          label: c.label,
          go: () => void showLevel(c.level!),
        },
  );
}

/**
 * The lids above the current one. On the desktop each is a lid that goes back up; on the phone one
 * `⋯` lid, at every level, lists them and then every level of the shown map, so the row keeps four
 * targets and the phone reaches any level (design-language.md, "Touch").
 */
function Ancestors({ lids }: { lids: readonly Lid[] }) {
  const map = useCanvas((s) => s.map);
  const showLevel = useCanvas((s) => s.showLevel);
  const phone = useMedia(PHONE);
  const above = lids.slice(0, -1);
  if (phone) {
    const levels = map?.levels ?? [];
    if (levels.length === 0) {
      return null;
    }
    return (
      <Picker
        concept="levels"
        label={CANVAS_STRINGS.levels_sheet}
        className="lid-more"
        buttonClass="lid lid-ancestor lid-more-button"
        entries={[
          ...above.map((l) => ({
            key: `lid:${l.key}`,
            label: l.label,
            current: false,
          })),
          ...levels.map((l) => ({
            key: `level:${l.id}`,
            label: sanitize(l.label),
            current: l.id === map?.level,
            apart: l.apart,
          })),
        ]}
        onPick={(key) => {
          if (key.startsWith("level:")) {
            void showLevel(key.slice("level:".length));
          } else {
            above.find((l) => `lid:${l.key}` === key)?.go();
          }
        }}
      >
        <span aria-hidden="true">⋯</span>
      </Picker>
    );
  }
  return (
    <>
      {above.map((l) => (
        <button
          key={l.key}
          type="button"
          className="lid lid-ancestor"
          onClick={l.go}
        >
          {l.label}
        </button>
      ))}
    </>
  );
}

function Breadcrumb({
  searching,
  setSearching,
}: {
  searching: boolean;
  setSearching: (searching: boolean) => void;
}) {
  const map = useCanvas((s) => s.map);
  const query = useCanvas((s) => s.query);
  const search = useCanvas((s) => s.search);
  const reveal = useCanvas((s) => s.reveal);
  const dive = useCanvas((s) => s.dive);
  // The selected block, when a dive into it goes anywhere.
  const diver = useCanvas((s) =>
    s.selected !== null && s.canDive(s.selected)
      ? (s.map?.nodes.find((n) => n.id === s.selected) ?? null)
      : null,
  );
  const phone = useMedia(PHONE);
  const input = useRef<HTMLInputElement>(null);
  const rows = useMemo(() => (map ? blocksOf(map) : []), [map]);
  const lids = useLids();
  const current = lids.at(-1);
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
      aria-label={CANVAS_STRINGS.levels}
    >
      <Ancestors lids={lids} />
      <div
        className="lid lid-current"
        aria-current="location"
        title={current?.label}
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
            placeholder={CANVAS_STRINGS.open_search}
            aria-label={CANVAS_STRINGS.search}
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
            aria-label={
              searching ? CANVAS_STRINGS.close_search : CANVAS_STRINGS.search
            }
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
      {!phone && diver !== null && (
        <button
          type="button"
          className="lid lid-open"
          onClick={() => void dive(diver.id)}
        >
          {fill(CANVAS_STRINGS.open_block, { name: sanitize(diver.name) })}
        </button>
      )}
    </nav>
  );
}

/**
 * The lens switcher lists the lenses `ask` lists and its sheet holds the theme choice on the local
 * canvas, so it is a button there even with one lens, which the sheet shows flat above the choice.
 * The shelf lists `intent list`'s projects and stays flat with none to pick between. While search
 * is open on the phone, both yield, so the row holds `⋯`, the input and Close.
 */
export function TopRow() {
  const map = useCanvas((s) => s.map);
  const pane = useCanvas((s) => s.pane);
  const lenses = useCanvas((s) => s.lenses);
  const projects = useCanvas((s) => s.projects);
  const pickedProject = useCanvas((s) => s.pickedProject);
  const pickLens = useCanvas((s) => s.pickLens);
  const pickProject = useCanvas((s) => s.pickProject);
  const phone = useMedia(PHONE);
  const theme = useThemeChoice();
  const [searching, setSearching] = useState(false);
  const lens = map?.lens ?? pane?.focus.lens ?? null;
  // The last project picked, else the pane's, else the only one there is.
  const project =
    pickedProject ??
    pane?.focus.project ??
    (projects.length === 1 ? projects[0]!.id : null);
  const current = projects.find((p) => p.id === project);
  const currentName = current === undefined ? null : sanitize(current.name);
  return (
    <header className="top-row">
      <Breadcrumb searching={searching} setSearching={setSearching} />
      {!(phone && searching) && (
        <>
          <Picker
            concept="lenses"
            label={CANVAS_STRINGS.lenses}
            className="lens-switcher"
            style={{ "--yarn": yarnVar(map?.yarn ?? null) } as CSSProperties}
            entries={lensEntries(lenses, lens)}
            onPick={(key) => void pickLens(key)}
            extra={theme === null ? undefined : <ThemeChoice />}
          >
            <YarnIcon />
            <span className={phone ? "visually-hidden" : "picker-label"}>
              {lensLabel(lenses, lens)}
            </span>
          </Picker>
          <Picker
            concept="intent"
            label={CANVAS_STRINGS.projects}
            className="project-shelf"
            entries={projects.map((p) => ({
              key: p.id,
              label: sanitize(p.name),
              current: p.id === project,
            }))}
            onPick={
              projects.length > 1 ? (key) => void pickProject(key) : undefined
            }
          >
            <span className="chip-label" title={currentName ?? undefined}>
              {currentName ?? CANVAS_STRINGS.no_project}
            </span>
          </Picker>
        </>
      )}
    </header>
  );
}
