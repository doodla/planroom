# `@planroom/canvas`

This area's contract, from [Repo layout](../../docs/contributing/repo-layout.md#area-contracts).

- **Owns:** the canvas app, both transports, views, the ELK worker, motion.
- **May depend on:** `@planroom/contract`, `@planroom/ui`, React, `@xyflow/react`, `elkjs`, `zustand`, `motion`, `radix-ui`.
- **Must not:** `@planroom/mod`; engine internals; fetching any host but its own origin.
- **How to test it:** vitest, Playwright e2e.

## Layout

- `index.html`, `vite.config.js`: the Vite app; `cargo xtask gen --canvas` builds it into `target/canvas/` with relative paths and stable file names, and the config links the tokens from `@planroom/ui` as `assets/tokens.css`.
- `src/transport/`: the `Transport` interface, the local transport (login code, bearer token, the stream read through `fetch`) and its SSE parser.
- `src/layout/`: the `MapView` to ELK mapping and ELK in its Web Worker.
- `src/views/`: `MapView` and its blocks, the screen-reader outline, empty states.
- `src/shell/`: the five concepts around the map: the top row (breadcrumb and search, lenses, projects), the zoom rail, the card stack, the diff tray, and the phone's bottom sheet.
- `src/assets/`: the Iris empty-state art from `docs/art/`, cropped and converted to WebP.

`src/strings.ts` holds the lines the canvas shows before an engine answers; the needs-login line belongs in `en.toml`, generated into the build, once the engine generates it.
