# Research: the TypeScript stack (canvas, mod, shared packages)

Phase 1 research, topic `ts-stack`. Run 2026-10-25 in this container: Node 22.22.0, pnpm 10.28.0, Playwright 1.63.0 with the preinstalled Chromium 153.0.8010.12 (headless, SwiftShader software GL), 4 vCPU Intel Xeon @ 2.80 GHz, 15 GB RAM.

## Evidence key

- **[V]** I verified it here, and the command or measurement is cited. The spike code was throwaway and lives in the scratchpad (`/tmp/claude-0/-home-user-planroom/d26fcf29-d6d2-57f8-acca-1138f06b3754/scratchpad/ts-stack/`), not in the repo. Section 9 has what you need to reproduce it.
- **[D]** I read it in docs or package metadata (URL or file named) and did not exercise it.
- **[A]** This is my assumption or inference.

---

## 1. Summary

- **ELK is the bottleneck, not React Flow.** On this VM, `elk.layout()` with layered, `INCLUDE_CHILDREN` and 2 levels of compound nodes takes about **1.1–1.5 s for 200 nodes / 400 edges** and **3.4–4.5 s for 500 nodes / 1000 edges**, in both Node and Chromium [V]. A flat graph with no compounds is not much faster (0.88 s / 2.7 s) [V]. The "<1 s interactive" bar therefore **cannot include a live ELK run**. Run layout in a Web Worker, persist the result as positions, and only re-lay out what changed.
- **Rendering is fine.** With a cached layout, the production build is interactive at **0.31–0.45 s** (median of 5 cold loads, in two separate sessions) at 200 nodes, and at **1.4–1.6 s** with 4× CPU throttling [V]. Panning and zooming at 200 nodes gives a **p95 frame time of 16.8 ms** (vsync-capped 60 fps). With 4× throttling it is 33–50 ms [V].
- **Bundle.** The first-paint critical path is about **215 KB gzip**: JS 127.6 KB, CSS 5.5 KB, and 82 KB of latin woff2 for two variable fonts. The whole `dist/` is 4.1 MB raw, far under the Artifact's 16 MB cap. ELK's worker adds 1.6 MB raw (462 KB gzip), loaded only when a layout is computed [V].
- **Relative paths work.** A Vite 8 build with `base: './'`, served statically from a nested sub-path (`/a/b/canvas/`), loaded every asset, the ELK worker and a relative `fetch()` with no 404s [V].
- **Boundaries: use `dependency-cruiser`.** It caught 5 of 5 planted bad imports, and `eslint-plugin-boundaries` caught 4 of 5 [V].
- **TypeScript 6.0.3, not 7.** `typescript-eslint` 8.x requires `typescript <6.1.0`, and TS 7.0.2 is the native Go compiler [V].
- **ESLint 10, not 9.** `eslint@9.39.5` is marked deprecated on npm. Typed lint and boundaries both ran under 10.13.0 [V].
- **Prettier, not Biome (for now).** Biome added Markdown and YAML formatting only in 2.6.x, released 4 days ago. 2.5.15 skips `.md` and `.yaml` files [V].

---

## 2. Versions and licenses

Everything was looked up with `npm view <pkg> version license time` on 2026-10-25 [V]. "Latest" is the npm `latest` tag. "Pin" is what I recommend for the repo's first lockfile. It is the newest version that passes a **7-day `minimumReleaseAge`**, which pnpm 10.28 enforces: I set `minimumReleaseAge: 10080` in `pnpm-workspace.yaml`, and `pnpm add @biomejs/biome` picked 2.5.15 over 2.6.3 [V]. Pins were resolved with that setting in place, using `pnpm add` in `scratchpad/ts-stack/mra/` [V].

| Package | Latest (published) | **Pin** | License | Status / notes |
|---|---|---|---|---|
| `vite` | 8.3.5 (10-21) | **8.3.4** (10-08) | MIT | [V] built with 8.3.5. Vite 8 bundles with Rolldown. |
| `@vitejs/plugin-react` | 6.1.2 (10-05) | **6.1.2** | MIT | [V] peer `vite ^8.0.0`. The babel and React Compiler peers are optional. |
| `react`, `react-dom` | 19.3.1 (10-14) | **19.3.1** | MIT | [V] |
| `@types/react`, `@types/react-dom` | 19.3.0 (09-09) | **19.3.0** | MIT | [V] |
| `typescript` | 7.0.2 (07-08) | **6.0.3** (04-16) | Apache-2.0 | [V] 7.x is native binaries only. `typescript-eslint` peer is `>=4.8.4 <6.1.0`. |
| `@xyflow/react` | 12.12.0 (09-24) | **12.12.0** | MIT | [V] peers react ≥17 |
| `elkjs` | 0.12.0 (07-17) | **0.12.0** | **EPL-2.0 OR GPL-3.0-or-later** | [V] See Risks: needs a license note. |
| `motion` | 14.6.0 (10-22) | **14.3.0** (10-15) | MIT | [V] size measured on 14.6.0 |
| `radix-ui` (umbrella, tree-shaken) | 1.8.1 (10-12) | **1.8.1** | MIT | [V] Per-primitive packages also exist, e.g. `@radix-ui/react-dialog` 1.3.1. |
| `xstate` | 5.33.3 (10-17) | **5.33.3** | MIT | [V] |
| `@xstate/react` | 6.1.0 (02-26) | **6.1.0** | MIT | [V] peer `xstate ^5.28.0` |
| `zustand` | 5.0.16 (10-22) | **5.0.15** (08-13) | MIT | [V] |
| `@tanstack/react-query` | 5.104.5 (10-19) | (not chosen; 5.104.4) | MIT | [V] |
| `json-schema-to-typescript` | 16.0.0 (08-28) | **16.0.0** | MIT | [V] handles 2020-12 `$defs` and `const` discriminators |
| `vitest` | 5.0.5 (10-15) | **5.0.5** | MIT | [V] engines node `^22.12.0 \|\| ^24 \|\| >=26` |
| `eslint` | 10.13.0 (10-16) | **10.13.0** | MIT | [V] 9.39.5 is flagged "no longer supported". |
| `@eslint/js` | 10.0.1 | **10.0.1** | MIT | [V] |
| `typescript-eslint` | 8.72.0 (10-19) | **8.71.2** (10-12) | MIT | [V] peer eslint `^8.57 \|\| ^9 \|\| ^10` |
| `eslint-plugin-react-hooks` | 7.1.1 | **7.1.1** | MIT | [V] peer includes eslint ^10 |
| `prettier` | 3.9.10 (10-10) | **3.9.10** | MIT | [V] chosen formatter |
| `@biomejs/biome` | 2.6.3 (10-24) | (not chosen; 2.5.15) | MIT OR Apache-2.0 | [V] |
| `dependency-cruiser` | 18.7.0 (10-19) | **18.6.0** (10-11) | MIT | [V] chosen boundary checker. Tested on 18.7.0. |
| `eslint-plugin-boundaries` | 7.3.0 (10-23) | (not chosen; 7.2.0) | MIT | [V] |
| `@playwright/test` | 1.64.1 | **1.63.0** (09-04) | Apache-2.0 | [V] matches the preinstalled `playwright` 1.63.0 and its `chromium-1243`. Don't bump one without the other. |
| `@fontsource-variable/*` | 5.3.0 (onest 5.3.1) | **5.3.0** | OFL-1.1 | [V] Each package ships its `LICENSE`. |

The spike ran on the "Latest" column. The pins differ from it by patch level, except `motion` (14.3 vs 14.6) and `eslint-plugin-boundaries` (7.2 vs 7.3). I don't expect behaviour differences [A].

---

## 3. Perf spike

### 3.1 Setup [V]

- **App:** Vite 8 + React 19 + `@xyflow/react` 12, built for production with `base: './'`. It renders `<ReactFlow fitView>` with `<Background>`, `<MiniMap>` and `<Controls>`, default nodes, parent-relative child positions, and group nodes for compounds.
- **Graph:** a deterministic generator (LCG seed 42) shaped like C4:
  - 200 nodes = 4 systems ⊃ 16 containers ⊃ 180 components, with 400 edges;
  - 500 nodes = 6 ⊃ 36 ⊃ 458, with 1000 edges.
  - Edge locality is 60% same container, 25% same system, 15% cross-system. This is denser and more random than a real code graph, so treat it as pessimistic [A].
- **ELK options:**
  ```
  elk.algorithm=layered, elk.direction=DOWN, elk.hierarchyHandling=INCLUDE_CHILDREN,
  elk.layered.nodePlacement.strategy=NETWORK_SIMPLEX, spacing nodeNode=24, betweenLayers=40,
  padding [top=36,left=12,bottom=12,right=12]
  ```
  Leaves are 150×44.
- **ELK timing** wraps only `await elk.layout(graph)`, with a fresh input object each run. Node used `elkjs/lib/elk.bundled.js`. Chromium used `elk.bundled.js` on the main thread, or `elk-api.js` with a `workerFactory` loading `elk-worker.min.js?url`.

### 3.2 ELK layout time

Node 22.22, 7 runs at the default options (`bench-node.mjs`):

| Nodes / edges | First run | Median | Min |
|---|---|---|---|
| 200 / 400 | 2583 ms | **1264 ms** | 1125 ms |
| 500 / 1000 | 3813 ms | **3839 ms** | 3738 ms |

Chromium 153, 5 runs (`browser.cjs elk`):

| Nodes | Main thread: first / median | Web Worker: first / median |
|---|---|---|
| 200 | 2374 / **1405 ms** | 1647 / **869 ms** |
| 500 | 4313 / **3513 ms** | 5199 / **3382 ms** |

Option variants, Node, median of 5 (`bench-variants.mjs`):

| Variant | 200 | 500 |
|---|---|---|
| A: default (INCLUDE_CHILDREN + NETWORK_SIMPLEX) | 1510 | 4506 |
| B: INCLUDE_CHILDREN + BRANDES_KOEPF | 1103 | 3657 |
| C: B + `elk.layered.thoroughness=1` | **592** | 1823 |
| D: B + SPLINES routing | 1276 | 3753 |
| E: SEPARATE_CHILDREN + BRANDES_KOEPF (cross-compound edges not routed) | **285** | **531** |
| Flat graph, leaves only, no compounds, defaults (`bench-flat.mjs`) | 880 | 2733 |
| Folded view: 1 system expanded, the rest collapsed, edges aggregated (124 / 219 visible edges) | 532 | 853 |
| Folded + C + `considerModelOrder=NODES_AND_EDGES` | 333 | 480 |

As a fallback check, `@dagrejs/dagre` 3.1.3 (MIT) in compound mode took **3460 ms** at 200 nodes. At 500 it **threw** `Not possible to find intersection inside of the rectangle` [V]. It is not an alternative.

What this means for the design [A, from the numbers above]:
- Positions are data. Compute them once in a Worker, write them back to the store through the engine API, and serve them with the graph. The Artifact and other tabs then reuse them, and no viewer pays the layout cost. This also gives R6 "pin a layout" and a stable layout for free.
- Lay out per compound with `SEPARATE_CHILDREN`, bottom up, so a rescan re-lays out only the containers that changed. Draw cross-compound edges with React Flow's own routing, or a second cheap pass.
- Semantic folding by default: show one level expanded, with aggregated edges.

### 3.3 Frame time while panning and zooming (200 nodes, cached layout)

**Method.** `browser.cjs frames`, viewport 1440×900.
- An rAF loop records `t - last` deltas.
- Playwright then scripts the interaction:
  - 12 wheel-zoom ticks (scale 0.074 → 0.54, 27 nodes on screen), or none (fit view, all 200 nodes and 400 edges on screen);
  - a 240-step mouse drag pan;
  - 40 wheel ticks out and back in;
  - a 120-step pan.
- `nodesDraggable=false`, so the drag pans the pane. I verified the viewport transform changes, e.g. `translate(-398px,-2615px)` → `translate(-44px,-2497px)` (`verify-pan.cjs`). A first attempt with draggable nodes dragged a group instead of panning; I discarded those numbers.
- Throttling is CDP `Emulation.setCPUThrottlingRate`.

| Scenario | Frames | p50 | **p95** | p99 | max | >33 ms |
|---|---|---|---|---|---|---|
| 1×, zoomed to 0.54 | 796 | 16.7 | **16.8** | 33.3 | 50 | 5 |
| 1×, fit view (all on screen) | 703 | 16.7 | **16.8** | 16.8 | 33 | 0 |
| 1×, `onlyRenderVisibleElements` | 712 | 16.7 | **16.8** | 33.4 | 50 | 11 |
| 4×, zoomed | 1267 | 16.7 | **33.4** | 50.1 | 100 | 106 |
| 4×, fit view | 1207 | 16.7 | **50.0** | 50.1 | 83 | 141 |
| 4×, `onlyRenderVisibleElements` | 1181 | 16.7 | **33.3** | 66.7 | 150 | 37 |
| 6×, zoomed | 1285 | 16.7 | **66.6** | 83.4 | 150 | 303 |

- The deltas are vsync-quantised (16.7 ms steps), and GL is SwiftShader. Real GPUs should do better on paint. Phones are CPU-bound like the throttled rows [A].
- `onlyRenderVisibleElements` did **not** help at 200 nodes: it raised p99 and max because of mount and unmount churn during the pan. Leave it off until node counts are much higher [V].

### 3.4 Time to interactive (production build, served statically)

**Method.** `browser.cjs tti`.
- Each run is a fresh browser context (cold cache) against `python3 -m http.server`, with 5 runs and the median reported.
- The layout comes from a relative `fetch('./layout-200.json')`.
- **TTI = max(FCP, graph ready, end of the last long task)**. "Graph ready" is the rAF after React Flow's `onInit`, with all 200 `.react-flow__node` elements in the DOM. Long tasks come from a buffered `PerformanceObserver('longtask')`, and a 2.5 s quiet window follows.

| Mode | FCP | Graph ready | Longest long task | **TTI** (median) | Click → next frame |
|---|---|---|---|---|---|
| Cached layout, 1× (session 1) | 124 ms | 447 ms | 147 ms | **447 ms** | 4 ms |
| Cached layout, 1× (session 2) | 100 ms | 314 ms | 89 ms | **314 ms** (range 297–363) | 8.5 ms |
| Cached layout, 4× | 384–432 ms | 1411–1595 ms | 488–525 ms | **1411–1595 ms** | 22–31 ms |
| Live ELK in a Worker, 1× (3 runs) | 108 ms | 2138 ms | 133 ms | **2138 ms** | 8.6 ms |

Breakdown, median of 5 (`breakdown.cjs`):

| Stage | 1× | 4× |
|---|---|---|
| App mounted | 119 ms | 365 ms |
| Layout JSON parsed | 161 ms | 478 ms |
| React Flow ready | 414 ms | 1619 ms |
| Render cost (ready minus parsed) | **253 ms** | **1142 ms** |

The 4× budget goes to React rendering 200 nodes, 400 edges and the minimap, not to loading. Folding is the lever: fewer DOM nodes at first paint [A].

---

## 4. Bundle size (production, `vite build`) [V]

`gzip -9` of each file in `dist/`. Woff2 is already compressed.

| File | Raw | Gzip | Loaded |
|---|---|---|---|
| `index.html` | 0.36 KB | 0.27 KB | first paint |
| `assets/index-*.js` (React 19 + React Flow + app) | 407.7 KB | **127.6 KB** | first paint |
| `assets/index-*.css` (incl. `@xyflow/react/dist/style.css`, 3.0 KB gzip) | 21.4 KB | 5.5 KB | first paint |
| Bricolage Grotesque latin wght woff2 | 41.3 KB | n/a | first paint (by `unicode-range`) |
| JetBrains Mono latin wght woff2 | 40.4 KB | n/a | first paint |
| 6 non-latin subsets (latin-ext, cyrillic, greek, vietnamese) | 71 KB | n/a | only if those glyphs are used. The network log shows only the 2 latin files fetched. |
| `assets/elk-worker.min-*.js` | 1595 KB | 462 KB | only when computing a layout |
| `assets/elk.bundled-*.js` | 1431 KB | 434 KB | only in main-thread mode. **Drop that mode in production.** |
| `layout-200.json` / `layout-500.json` | 140 / 358 KB | 21.6 / 55.3 KB | data |
| **Total `dist/`** | **4.1 MB** | | |

**Library costs.** I measured each library with its own probe build, React externalised, so these are added costs (`probe/*.ts`):

| Library (imports) | Min | Gzip |
|---|---|---|
| react + react-dom/client | 218 KB | 67.2 KB |
| `@xyflow/react` (ReactFlow, MiniMap, Controls, Background) | 176 KB | 56.7 KB |
| `motion/react` (`motion`, `AnimatePresence`, `MotionConfig`) | 137 KB | 44.8 KB |
| `motion` with `LazyMotion` + `domAnimation` + `m` | n/a | 30.9 KB |
| `radix-ui` (Dialog, Popover, Tooltip, DropdownMenu, Tabs, ToggleGroup) | 117 KB | 36.4 KB |
| `xstate` + `@xstate/react` | 38 KB | 12.3 KB |
| `@tanstack/react-query` | 33 KB | 9.6 KB |
| `zustand` | 0.6 KB | 0.4 KB |

Projection [A]:
- With all of it loaded eagerly, first-paint JS is about 225 KB gzip.
- If XState (statechart view) and Motion are lazy, it stays about 165 KB gzip.
- The 16 MB Artifact cap is not a concern. Gzip on the wire is: the engine must serve with compression, e.g. `tower-http` `CompressionLayer`. Whether the Artifact host compresses is unverified [A].

---

## 5. Static multi-file bundle with relative paths [V]

- **Config:** `vite.config.ts` = `{ base: './', plugins: [react()] }`.
- **Built `index.html`:** references `./assets/index-*.js` (`type="module" crossorigin`) and `./assets/index-*.css`. No absolute `/assets` strings appear in the JS (grep).
- **Worker URL:** the `?url` import of the ELK worker compiles to ``new URL(`elk-worker.min-*.js`, import.meta.url)``, so it resolves relative to the chunk, not the page.
- **Fonts:** emitted as separate `assets/*.woff2` and referenced relatively from the CSS. The default `assetsInlineLimit` of 4 KB doesn't inline them.
- **Test:** I copied `dist/` to `serve/a/b/canvas/` and loaded `http://127.0.0.1:4321/a/b/canvas/`.
  - Cached mode: 6 requests, all 200 and all under `/a/b/canvas/` (html, css, js, `layout-200.json`, 2 woff2).
  - Worker mode: the same, with `elk-worker.min-*.js` in place of the JSON.
  - 200 nodes rendered, and there were no `pageerror`s.
- **Not exercised here:** the Artifact-specific post-processing (stripping the doctype into a fragment, `modulePreload.polyfill: false`) and its CSP. See `artifact.md` §"Publishing a Vite build". That file records Workers from the page's own files and relative `fetch()` as working, and same-origin woff2 as an assumption.

---

## 6. Choices and rejected alternatives

| Area | Choice | Rejected | Why |
|---|---|---|---|
| Bundler / dev server | **Vite 8.3.4** + `@vitejs/plugin-react` 6.1.2 | (brief-mandated) | [V] 2.5 s build. Relative `base` works. Rolldown's chunk-size warning fires on the ELK worker asset. Set `build.chunkSizeWarningLimit` or ignore it. |
| Graph view | **`@xyflow/react` 12.12.0** | (brief-mandated) | [V] 60 fps at 200 nodes. Compound nodes map to `parentId` with relative positions. |
| Layout | **elkjs 0.12.0 in a Web Worker**, persisted positions, per-compound `SEPARATE_CHILDREN` incremental layout | `@dagrejs/dagre` 3.1.3: 3× slower and crashes at 500 compound nodes [V]. Main-thread ELK blocks 1.4 s [V]. | Section 3.2 |
| Typecheck | **TypeScript 6.0.3** | TS 7.0.2 (native) | [V] `typescript-eslint` 8.72 peer `<6.1.0`. A single compiler avoids drift between lint and typecheck. Revisit TS 7 when `typescript-eslint` supports it. |
| Lint | **ESLint 10.13.0** + `typescript-eslint` 8.71.2 (`recommendedTypeChecked`, `projectService`) + `eslint-plugin-react-hooks` 7.1.1 | ESLint 9.39.5 | [V] npm marks 9.39.5 deprecated. Typed lint ran under both 9.39.5 (2.9 s on the canvas spike) and 10.13.0. |
| Format | **Prettier 3.9.10** for TS, JSON, CSS, Markdown and YAML | Biome 2.6.3 | [V] Biome formats `.md` and `.yaml` only from 2.6.x (2.6.0 released 2026-10-21). 2.5.15, the newest that passes a 7-day age gate, skipped both. Biome's speed doesn't matter at this repo's TS size, and its linter would duplicate ESLint, which typed rules require anyway. **Tradeoff:** Prettier is slower and JS-based, but stable on Markdown, which is most of the docs. Revisit Biome once 2.6 has a few patch releases. |
| Boundaries | **dependency-cruiser 18.6.0** (allow-list config) | `eslint-plugin-boundaries` 7.x | See section 6.1 |
| State | **zustand 5.0.15** view store, plus a thin transport adapter (engine HTTP + SSE/WS locally, Artifact `db` remotely) that applies snapshots and events | TanStack Query 5 | [A] Canvas data is a live, pushed, event-sourced model from two transports, not request/response caching. Query's cache would duplicate the store. zustand is 0.4 KB gzip [V]. |
| Animation | **motion 14.3.0** via `LazyMotion` + `m`, lazy-loaded | eager `motion.*` (44.8 KB) | [V] 30.9 KB vs 44.8 KB gzip. The brief wants a few high-impact moments, honouring `prefers-reduced-motion` (`MotionConfig reducedMotion="user"`). |
| Primitives | **`radix-ui` 1.8.1** umbrella package | per-primitive `@radix-ui/react-*` packages | [V] Tree-shakes to 36 KB for 6 primitives. One version to pin instead of a dozen. |
| Statecharts | **xstate 5.33.3 + `@xstate/react` 6.1.0**, lazy-loaded with the statechart view | (brief-mandated) | [V] 12.3 KB gzip |
| Schema → TS | **json-schema-to-typescript 16.0.0** from the committed JSON Schemas (`schemars` output) | `ts-rs`, `specta` | [V] It handled 2020-12 `$defs`, `const` discriminated unions, enums and nullable objects (section 9). JSON Schema stays the one contract, usable for runtime validation and by the mod. `ts-rs`/`specta` would make Rust types the source and skip the schema the brief wants committed [A]. |
| Tests | **vitest 5.0.5**; **@playwright/test 1.63.0** for e2e | n/a | [V] Vitest ran a TS test in 275 ms. Playwright must match the preinstalled browsers. |

### 6.1 dependency-cruiser vs eslint-plugin-boundaries [V]

**Test repo.** `scratchpad/ts-stack/bnd/` has `packages/{schema,ui,canvas,mod}`. Each has `"exports": {".": "./src/index.ts"}` and is symlinked into `node_modules/@planroom/*` as pnpm would link it. Policy:
- the surfaces (`canvas`, `mod`) may import only `schema/src/index.ts` and `ui/src/index.ts`;
- `ui` may import only `schema/src/index.ts`;
- nothing may import sideways or upward.

There were five planted violations plus one legal file:

| Planted import | depcruise 18.7.0 | boundaries 7.3.0 (ESLint 9.39.5 and 10.13.0) |
|---|---|---|
| canvas → `@planroom/mod` (sideways, by package name) | ✔ `not-in-allowed` | ✔ |
| canvas → `../../mod/src/index` (sideways, relative) | ✔ | ✔ |
| canvas → `../../schema/src/internal` (deep, relative) | ✔ | ✔ |
| canvas → `@planroom/schema/src/internal` (deep, by package name; blocked by `exports`; `@ts-ignore`d) | ✔ `not-to-unresolvable` + `not-in-allowed` | ✘ silent: an unresolved import is skipped |
| ui → `../../canvas/src/index` (upward) | ✔ | ✔ |
| canvas/index.ts → `@planroom/schema`, `@planroom/ui`, `./helper` (legal) | no error | no error |
| Exit code / time | 6 errors, exit 6, 0.93 s | 4 errors, 1.0 s |

**Pick dependency-cruiser** because:
- It is an **allow-list with a default deny** (`allowed` + `allowedSeverity: 'error'`), which is the shape of "surfaces import only published contracts".
- It catches unresolvable deep imports.
- It runs without ESLint, as one CI command, and has a `no-circular` rule.
- It emits JSON and DOT graphs of the TS side. Planroom can ingest those when dogfooding [A].

`eslint-plugin-boundaries` 7.x is mid-migration. `boundaries/no-unknown`, `element-types`, `entry-point` and `external` all print deprecation warnings pointing to the v6→v7 guide [V]. That churn is a cost for an agent-maintained config.

The config that worked (18.7.0):

```js
module.exports = {
  forbidden: [
    { name: 'not-to-unresolvable', severity: 'error', from: {}, to: { couldNotResolve: true } },
    { name: 'no-circular', severity: 'error', from: {}, to: { circular: true } },
  ],
  allowed: [
    { comment: 'same package', from: { path: '^packages/([^/]+)/' }, to: { path: '^packages/$1/' } },
    { comment: 'external deps', from: {}, to: { dependencyTypes: ['npm', 'npm-dev', 'npm-peer', 'core'] } },
    { comment: 'surfaces -> published contract + ui', from: { path: '^packages/(canvas|mod)/' },
      to: { path: ['^packages/schema/src/index\\.ts$', '^packages/ui/src/index\\.ts$'] } },
    { comment: 'ui -> published contract', from: { path: '^packages/ui/' }, to: { path: '^packages/schema/src/index\\.ts$' } },
  ],
  allowedSeverity: 'error',
  options: {
    doNotFollow: { path: 'node_modules' },
    tsConfig: { fileName: 'tsconfig.json' },
    tsPreCompilationDeps: true,
    enhancedResolveOptions: { exportsFields: ['exports'], conditionNames: ['import', 'types', 'default'], extensions: ['.ts', '.tsx', '.js'] },
  },
};
```

Run it as `depcruise packages --config .dependency-cruiser.cjs --output-type err`. Add a "generated code is never hand-edited" check separately, for example a header check on `packages/schema/src/generated/**` plus the drift test [A].

---

## 7. Fonts (self-hosted via Fontsource, no CDN)

All are `@fontsource-variable/*` 5.3.0, OFL-1.1, with `LICENSE` in the package and `font-display: swap` in the CSS [V]. Sizes are the latin `wght` variable woff2 from `npm pack --dry-run --json` [V]. Only the latin subset downloads for English UI, thanks to `unicode-range` [V, network log §5]. None is Inter, Roboto, Arial, system-ui or Space Grotesk.

| # | Display | Text | Character [A] | Latin woff2 (display + text) |
|---|---|---|---|---|
| **1 (recommended)** | **Bricolage Grotesque** (wght; `opsz`/`wdth` axes in separate files, 75–76 KB) | **Atkinson Hyperlegible Next** | Quirky ink-trap grotesque, toy-like at display sizes. Atkinson is built for legibility on small phone labels. | 40.4 + 33.2 KB |
| 2 | **Unbounded** | **Onest** | Wide, rounded and chunky, very "playful planning table". Onest is a calm humanist text face. | 49.7 + 31.5 KB |
| 3 | **Recursive** (Casual axis) | Recursive (Linear) | One superfamily, with `CASL` for playful headings and `MONO` for code. Cohesive, but `casl`/`mono` files are 71–106 KB each and the full axis file is 298 KB. | 54.6 KB + axis files |
| 4 | **Fraunces** (SOFT / WONK axes) | **Commissioner** | Soft "wonky" serif display with a flared grotesque text. Warmer and craftier, less techy. | 35.8 + 35.8 KB |
| 5 | **Gabarito** | **Schibsted Grotesk** | Geometric, heavy and friendly display, with a newsy grotesque text. | 33.4 + 45.7 KB |
| 6 | **Syne** (extra-bold widths) | **Host Grotesk** | Art-school display with a compact text face. Host is the smallest file here. | 33.8 + 19.8 KB |

| Code font | Latin woff2 | Note |
|---|---|---|
| **JetBrains Mono** (recommended) | 39.5 KB | [V] used in the spike. Ligatures can be turned off. Familiar to engineers. |
| Martian Mono | 23.0 KB (`wdth` axis 37.6 KB) | Wide, pairs with Unbounded (pair 2) |
| Geist Mono | 22.6 KB | Smallest |
| Commit Mono (static only) | 47.0 KB per weight | Neutral, no variable build |
| Recursive Mono | (axis file 70.9 KB) | Only if pair 3 is chosen |

Recommendation [A]: pair 1 with JetBrains Mono. It costs 3 woff2 files and about 113 KB, cached forever thanks to hashed names. Import `wght.css` (or `index.css`) per family from the canvas entry, and `preload` only the latin display face. The mod renders in Claude Code's own UI, so check `mod-api.md` before assuming it can load web fonts [A].

---

## 8. Gotchas

1. **TS 7 vs `typescript-eslint`.** `pnpm add typescript` now gives 7.0.2 (native), and `typescript-eslint` 8.x needs `<6.1.0` [V]. Pin `typescript@6.0.3` exactly.
2. **ESLint 9 is deprecated on npm** [V]. Use 10.x. The `tseslint.config()` helper still worked under 10.13.0 [V].
3. **Biome version cliff.** Markdown and YAML formatting exist only from 2.6.0 [V]. A 7-day age gate silently resolves to 2.5.15 and skips those files without error [V].
4. **`minimumReleaseAge` works in pnpm 10.28** [V]. Several "latest" versions are under 7 days old today (vite 8.3.5, motion 14.6.0, zustand 5.0.16, biome 2.6.3, boundaries 7.3.0, typescript-eslint 8.72.0), so the pins in §2 are deliberately one step back.
5. **ELK's first call is about 2× slower** (JIT warm-up of the GWT output): 2.4–2.6 s vs 1.3 s at 200 nodes [V]. Warm the worker with a tiny graph at idle [A].
6. **`elk.bundled.js` vs a worker.** Importing `elk.bundled.js` pulls 1.4 MB onto the main thread. Use `elk-api.js` with `workerFactory: () => new Worker(workerUrl)`, where `workerUrl` comes from `import workerUrl from 'elkjs/lib/elk-worker.min.js?url'` [V]. Vite copies the `?url` asset as-is and does not re-minify it.
7. **React Flow compound nodes.** Parents must precede children in the `nodes` array, and child positions are relative to the parent. ELK's output coordinates already are, so map them 1:1 [V].
8. **Group nodes are draggable by default.** A drag on a big group moves the group, not the viewport [V]. Decide drag affordances: e.g. pan on empty space and on locked groups, and drag only with a handle.
9. **`onlyRenderVisibleElements` hurt at 200 nodes** (worse p99 and max) [V]. Leave it off at this scale.
10. **fitView on a tall DOWN layout zooms to 7%** (0.074) at 200 nodes [V]: unreadable. Start folded, or start at a readable zoom on the focus.
11. **The MiniMap rendered blank** in the 1× screenshot taken right after `onInit` [V, `shot-200.png`]. I didn't chase it. Check minimap behaviour with group nodes in the walking skeleton [A].
12. **Rolldown's chunk-size warning** for the ELK worker is expected [V]. Don't "fix" it by inlining the worker.
13. **elkjs license is `EPL-2.0 OR GPL-3.0-or-later`** [V], the only non-permissive licence in the stack. EPL-2.0 is weak copyleft, so shipping it unmodified in the plugin and the Artifact needs the notice and a source pointer [A]. Add it to the npm license allow-list deliberately.

---

## 9. Reproduce

The scratchpad root is `/tmp/claude-0/-home-user-planroom/d26fcf29-d6d2-57f8-acca-1138f06b3754/scratchpad/ts-stack/`.

- `packages/canvas/`
  - `src/graph.ts`: the generator and the ELK ↔ React Flow mapping
  - `src/App.tsx`: query params `n`, `layout=cached|worker|main`, `runs`, `virt`, `drag`
  - `src/elk.ts`
  - `vite.config.ts`
  - `bench-node.mjs`, `bench-variants.mjs`, `bench-flat.mjs`, `bench-fold.mjs`, `bench-dagre.mjs`
  - `probe/`: the per-library size builds
- `bench/browser.cjs paths|elk|frames|tti [worker]`, `bench/breakdown.cjs`, `bench/verify-pan.cjs`. These use the global `require('playwright')`, `chromium.launch()`, and a static server: `cd serve && python3 -m http.server 4321`.
- `bnd/`: the boundary test (depcruise and ESLint configs)
- `fmt/`: the Biome / Prettier coverage test
- `codegen/`: the json-schema-to-typescript test
- `mra/`: the 7-day release-age resolution

json-schema-to-typescript output for a 2020-12 schema with a `oneOf` of `const`-tagged objects and `$defs` [V]:

```ts
export type GraphEvent =
  | { kind: "node_added"; node: Node }
  | { kind: "edge_added"; from: string; to: string };
export type Level = "intent" | "context" | "container" | "component" | "code";
export interface Node { id: string; level: Level; pinned?: { x: number; y: number } | null; }
```

(The real output is the same shape, multi-line, with the banner comment.)

---

## 10. Top risks

1. **Layout latency** (high). ELK at 200 to 500 nodes takes 1–4.5 s, and a live layout puts the cold TTI at 2.1 s [V]. Mitigation: worker, persisted positions, per-compound incremental layout, folding. This needs an engine-side "positions" field in the schema from day one.
2. **Phone-class CPU** (medium). At 4× throttle the first render of 200 nodes costs 1.1 s, and pan p95 is 33–50 ms [V]. Mitigation: fold to about 50 visible nodes, `React.memo` custom nodes, lazy minimap. Re-measure at 4× in the e2e suite.
3. **Toolchain churn** (medium):
   - TS 7 vs `typescript-eslint`;
   - ESLint 9 deprecated;
   - Biome's Markdown support 4 days old;
   - the boundaries plugin's v7 rule renames;
   - the elkjs EPL/GPL licence needing an explicit allow-list entry.

   Mitigation: exact pins, the 7-day `minimumReleaseAge`, Dependabot grouped by ecosystem, and the drift and licence checks in CI.
