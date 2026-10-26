/**
 * The five concepts around the map (design-language.md, "Layout" and "Touch"). At desktop widths
 * the top row runs across, the zoom rail sits left, the card stack right, and the diff tray in a
 * drawer along the bottom. On the phone the card stack and the tray share a bottom sheet with two
 * detents: peek, the face-up card; and half, the card and the tray.
 */

import { useState } from "react";

import { STRINGS } from "../strings";
import { PHONE, useCanvas, useMedia } from "../use-canvas";
import { MapCanvas } from "../views/map-view";
import { RequestState, TransportNote } from "../views/request-state";
import { CardStack } from "./card-stack";
import { TrayBadge, TrayList, TrayPeek } from "./diff-tray";
import { ChevronIcon } from "./icons";
import { TopRow } from "./top-row";
import { ZoomRail } from "./zoom-rail";

function BottomSheet() {
  const [half, setHalf] = useState(false);
  return (
    <section className="sheet panel" data-detent={half ? "half" : "peek"}>
      <div className="sheet-head">
        <TrayBadge open={half} onPress={() => setHalf(!half)} />
        <button
          type="button"
          className="icon-button sheet-toggle"
          aria-label={half ? STRINGS.sheetPeek : STRINGS.sheetHalf}
          aria-expanded={half}
          onClick={() => setHalf(!half)}
        >
          <ChevronIcon up={!half} />
        </button>
      </div>
      <div className="sheet-body">
        <CardStack />
        {half && <TrayList />}
      </div>
    </section>
  );
}

export function Shell() {
  const phone = useMedia(PHONE);
  const [trayOpen, setTrayOpen] = useState(false);
  const requesting = useCanvas((s) => s.request !== null);
  if (phone) {
    return (
      <div className="shell phone">
        <TopRow />
        <main className="stage">
          <TransportNote />
          {requesting ? <RequestState /> : <MapCanvas />}
        </main>
        <BottomSheet />
      </div>
    );
  }
  return (
    <div className="shell desk">
      <TopRow />
      <ZoomRail />
      <main className="stage">
        <TransportNote />
        {requesting ? <RequestState /> : <MapCanvas />}
      </main>
      <aside className="side">
        <CardStack />
      </aside>
      <footer className="drawer panel" data-open={trayOpen || undefined}>
        <div className="drawer-head">
          <TrayBadge open={trayOpen} onPress={() => setTrayOpen(!trayOpen)} />
          {!trayOpen && <TrayPeek />}
        </div>
        {trayOpen && <TrayList />}
      </footer>
    </div>
  );
}
