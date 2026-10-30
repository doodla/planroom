/**
 * The canvas entry: tokens and faces first, then the theme on the local transport only, then the
 * store on its transport, then the app. The page
 * `share` writes marks its root `data-transport="artifact"`; every other page, the one `serve`
 * serves, talks to the engine through the local transport.
 */

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
import { artifactTransport, browserArtifactEnv } from "./transport/artifact";
import { browserEnv, localTransport } from "./transport/local";
import { ThemeContext, browserThemeEnv, startTheme } from "./theme";
import { StoreContext } from "./use-canvas";

const root = document.getElementById("root");
const local = root?.dataset.transport !== "artifact";
const transport = local
  ? localTransport(browserEnv())
  : artifactTransport(browserArtifactEnv());
// claude.ai owns the Artifact page's theme; only the local canvas keeps a pin [DR-122].
const theme = local ? startTheme(browserThemeEnv()) : null;
const store = createCanvasStore(transport, layoutInWorker);
if (root) {
  createRoot(root).render(
    <StrictMode>
      <ThemeContext.Provider value={theme}>
        <StoreContext.Provider value={store}>
          <App />
        </StoreContext.Provider>
      </ThemeContext.Provider>
    </StrictMode>,
  );
}
void store.getState().start();
