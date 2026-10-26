/** The canvas entry: tokens and faces first, then the store on the local transport, then the app. */

import "@fontsource/bricolage-grotesque/latin-600.css";
import "@fontsource/bricolage-grotesque/latin-800.css";
import "@fontsource/atkinson-hyperlegible-next/latin-400.css";
import "@fontsource/atkinson-hyperlegible-next/latin-600.css";
import "@fontsource/jetbrains-mono/latin-400.css";
import "@xyflow/react/dist/base.css";
import "./styles.css";

import { StrictMode } from "react";
import { createRoot } from "react-dom/client";

import { App } from "./app";
import { layoutInWorker } from "./layout/elk";
import { createCanvasStore } from "./store";
import { browserEnv, localTransport } from "./transport/local";
import { StoreContext } from "./use-canvas";

const store = createCanvasStore(localTransport(browserEnv()), layoutInWorker);
const root = document.getElementById("root");
if (root) {
  createRoot(root).render(
    <StrictMode>
      <StoreContext.Provider value={store}>
        <App />
      </StoreContext.Provider>
    </StrictMode>,
  );
}
void store.getState().start();
