// The canvas build: `cargo xtask gen --canvas` runs `vite build` into target/canvas/, which
// crates/planroom/build.rs embeds and `share` publishes as an Artifact. Paths are relative and file
// names stable, so the same files serve from 127.0.0.1 and from an Artifact's sub-path.
import { URL, fileURLToPath } from "node:url";

import { tokensCss } from "@planroom/ui";
import react from "@vitejs/plugin-react";
import { defineConfig } from "vite";

const TOKENS = "assets/tokens.css";

/**
 * The design tokens from @planroom/ui as their own stylesheet, linked first in the page's head, so
 * the felt paints before any script runs and the CSP needs no inline style for it.
 */
function tokens() {
  return {
    name: "planroom-tokens",
    transformIndexHtml() {
      return [
        {
          tag: "link",
          attrs: { rel: "stylesheet", href: `./${TOKENS}` },
          injectTo: "head",
        },
      ];
    },
    configureServer(server) {
      server.middlewares.use(`/${TOKENS}`, (_req, res) => {
        res.setHeader("Content-Type", "text/css");
        res.end(tokensCss());
      });
    },
    generateBundle() {
      this.emitFile({ type: "asset", fileName: TOKENS, source: tokensCss() });
    },
  };
}

const names = {
  entryFileNames: "assets/[name].js",
  chunkFileNames: "assets/[name].js",
  assetFileNames: "assets/[name][extname]",
};

export default defineConfig({
  root: fileURLToPath(new URL(".", import.meta.url)),
  base: "./",
  plugins: [react(), tokens()],
  build: {
    outDir: fileURLToPath(new URL("../../target/canvas", import.meta.url)),
    emptyOutDir: true,
    // The Artifact page loads the module itself; no polyfill script.
    modulePreload: { polyfill: false },
    // ELK's worker is large by nature and loads only to lay out a new structure [ts-stack §8].
    chunkSizeWarningLimit: 2048,
    rolldownOptions: { output: names },
  },
  worker: {
    format: "es",
    rolldownOptions: { output: names },
  },
});
