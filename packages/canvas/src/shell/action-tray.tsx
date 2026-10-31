/**
 * The action tray (DR-68): the six hand edits on one selected block, each one `edit` leaf. It sits
 * under the block on the desktop (as the map's node toolbar) and at the top of the phone's half
 * sheet. Rename takes a name; Group with, Move into and Relate to pick other blocks of the shown
 * map from a list, so the keyboard and a thumb reach everything a drag does; Hide and Pin send at
 * once. Every finished gesture, and Escape, return it to its buttons. It draws nothing when the
 * view can't be edited (the Artifact, a map with no layout, a workspace with no store), and a
 * button that couldn't complete is not drawn: Pin needs a stored layout and the block's place,
 * Move into a block to move into, Relate to a relation the view already draws.
 */

import { CANVAS_STRINGS, type NodeId } from "@planroom/contract";
import { sanitize } from "@planroom/ui";
import {
  useEffect,
  useMemo,
  useRef,
  useState,
  type KeyboardEvent,
} from "react";

import { moveTargets, relationKinds } from "../edits/ops";
import { useCanvas } from "../use-canvas";

type Mode =
  | "buttons"
  | "rename"
  | "group"
  | "group-name"
  | "move"
  | "relate"
  | "relate-kind";

export function ActionTray({
  id,
  startRelateTo,
}: {
  id: NodeId;
  /** Open at the relation step for a relation to this block, as a chalk line drawn to it does. */
  startRelateTo?: NodeId;
}) {
  const can = useCanvas((s) => s.canEdit());
  const map = useCanvas((s) => s.map);
  const positions = useCanvas((s) => s.positions);
  const edit = useCanvas((s) => s.edit);
  const [mode, setMode] = useState<Mode>(
    startRelateTo === undefined ? "buttons" : "relate-kind",
  );
  const [name, setName] = useState("");
  const [ticked, setTicked] = useState<readonly NodeId[]>([]);
  const [to, setTo] = useState<NodeId | null>(startRelateTo ?? null);
  const root = useRef<HTMLDivElement>(null);
  const field = useRef<HTMLInputElement>(null);
  const back = useRef(false);

  const nameOf = (node: { id: string; name: string }) =>
    sanitize(node.name || node.id.split(":").at(-1) || node.id);
  const self = map?.nodes.find((n) => n.id === id);
  const others = useMemo(
    () => (map?.nodes ?? []).filter((n) => n.id !== id),
    [map, id],
  );
  const targets = useMemo(
    () => (map === null ? [] : moveTargets(map, id)),
    [map, id],
  );
  const kinds = useMemo(() => (map === null ? [] : relationKinds(map)), [map]);
  const place = positions[id];

  // A tray for another block starts over.
  useEffect(() => {
    setMode(startRelateTo === undefined ? "buttons" : "relate-kind");
    setName("");
    setTicked([]);
    setTo(startRelateTo ?? null);
  }, [id, startRelateTo]);

  // The field takes the focus when a step opens it; the buttons take it back when a step closes.
  useEffect(() => {
    if (mode === "rename" || mode === "group-name") {
      field.current?.focus();
      field.current?.select();
    } else if (mode === "buttons" && back.current) {
      root.current?.querySelector<HTMLElement>("button")?.focus();
    }
    back.current = mode !== "buttons";
  }, [mode]);

  if (!can || map === null || self === undefined) {
    return null;
  }

  const finish = () => {
    setMode("buttons");
    setName("");
    setTicked([]);
    setTo(null);
  };
  const onKeyDown = (e: KeyboardEvent) => {
    if (e.key === "Escape" && mode !== "buttons") {
      e.stopPropagation();
      finish();
    }
  };
  const nameStep = (done: () => void) => (
    <div className="tray-step">
      <label className="tray-field">
        <span>{CANVAS_STRINGS.name_field}</span>
        <input
          ref={field}
          type="text"
          value={name}
          onChange={(e) => setName(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter") {
              e.preventDefault();
              e.stopPropagation();
              done();
            }
          }}
        />
      </label>
      <div className="tray-row-buttons">
        <button type="button" className="button primary" onClick={done}>
          {CANVAS_STRINGS.done}
        </button>
        <button type="button" className="button" onClick={finish}>
          {CANVAS_STRINGS.cancel}
        </button>
      </div>
    </div>
  );
  const cancel = (
    <button type="button" className="button" onClick={finish}>
      {CANVAS_STRINGS.cancel}
    </button>
  );
  const renamed = () => {
    if (name.trim() !== "") {
      void edit({ op: "rename", id, name: name.trim() });
    }
    finish();
  };
  const grouped = () => {
    if (name.trim() !== "") {
      void edit({ op: "group", name: name.trim(), ids: [id, ...ticked] });
    }
    finish();
  };
  const related = (kind: string) => {
    if (to !== null) {
      void edit({ op: "relate", from: id, to, kind });
    }
    finish();
  };

  let body;
  switch (mode) {
    case "rename":
      body = nameStep(renamed);
      break;
    case "group":
      body = (
        <div className="tray-step">
          <ul
            className="tray-list-pick"
            aria-label={CANVAS_STRINGS.pick_blocks}
          >
            {others.map((n) => (
              <li key={n.id}>
                <label className="tray-pick">
                  <input
                    type="checkbox"
                    checked={ticked.includes(n.id)}
                    onChange={(e) =>
                      setTicked(
                        e.target.checked
                          ? [...ticked, n.id]
                          : ticked.filter((t) => t !== n.id),
                      )
                    }
                  />
                  <span>{nameOf(n)}</span>
                </label>
              </li>
            ))}
          </ul>
          <div className="tray-row-buttons">
            <button
              type="button"
              className="button primary"
              disabled={ticked.length === 0}
              onClick={() => setMode("group-name")}
            >
              {CANVAS_STRINGS.done}
            </button>
            {cancel}
          </div>
        </div>
      );
      break;
    case "group-name":
      body = nameStep(grouped);
      break;
    case "move":
      body = (
        <div className="tray-step">
          <ul className="tray-list-pick" aria-label={CANVAS_STRINGS.pick_block}>
            {targets.map((n) => (
              <li key={n.id}>
                <button
                  type="button"
                  className="button"
                  onClick={() => {
                    void edit({ op: "move", id, parent: n.id });
                    finish();
                  }}
                >
                  {nameOf(n)}
                </button>
              </li>
            ))}
          </ul>
          {cancel}
        </div>
      );
      break;
    case "relate":
      body = (
        <div className="tray-step">
          <ul className="tray-list-pick" aria-label={CANVAS_STRINGS.pick_block}>
            {others.map((n) => (
              <li key={n.id}>
                <button
                  type="button"
                  className="button"
                  onClick={() => {
                    if (kinds.length === 1) {
                      void edit({
                        op: "relate",
                        from: id,
                        to: n.id,
                        kind: kinds[0]!,
                      });
                      finish();
                    } else {
                      setTo(n.id);
                      setMode("relate-kind");
                    }
                  }}
                >
                  {nameOf(n)}
                </button>
              </li>
            ))}
          </ul>
          {cancel}
        </div>
      );
      break;
    case "relate-kind":
      body = (
        <div className="tray-step">
          <ul
            className="tray-list-pick"
            aria-label={CANVAS_STRINGS.pick_relation}
          >
            {kinds.map((kind) => (
              <li key={kind}>
                <button
                  type="button"
                  className="button"
                  onClick={() => related(kind)}
                >
                  {sanitize(kind)}
                </button>
              </li>
            ))}
          </ul>
          {cancel}
        </div>
      );
      break;
    default:
      body = (
        <>
          <button
            type="button"
            className="button"
            onClick={() => {
              setName(self.name);
              setMode("rename");
            }}
          >
            {CANVAS_STRINGS.rename}
          </button>
          <button
            type="button"
            className="button"
            onClick={() => setMode("group")}
          >
            {CANVAS_STRINGS.group_with}
          </button>
          <button
            type="button"
            className="button"
            onClick={() => void edit({ op: "hide", id })}
          >
            {CANVAS_STRINGS.hide}
          </button>
          {map.layout != null && place !== undefined && (
            <button
              type="button"
              className="button"
              onClick={() => void edit({ op: "pin", id, ...place })}
            >
              {CANVAS_STRINGS.pin}
            </button>
          )}
          {targets.length > 0 && (
            <button
              type="button"
              className="button"
              onClick={() => setMode("move")}
            >
              {CANVAS_STRINGS.move_into}
            </button>
          )}
          {kinds.length > 0 && (
            <button
              type="button"
              className="button"
              onClick={() => setMode("relate")}
            >
              {CANVAS_STRINGS.relate_to}
            </button>
          )}
        </>
      );
  }
  return (
    <div
      ref={root}
      className="action-tray"
      role="toolbar"
      aria-label={CANVAS_STRINGS.tray}
      data-concept="levels"
      data-mode={mode}
      onKeyDown={onKeyDown}
    >
      {body}
    </div>
  );
}
