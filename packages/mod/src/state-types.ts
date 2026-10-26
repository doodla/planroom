/**
 * The values of the mod's `$.state` atoms, read off the generated contract,
 * `plugin/types/index.d.ts`, which `cargo xtask gen` writes.
 */

import type { PluginState, ShapedValue } from "claude-code";

export type EngineState = ShapedValue<PluginState["planroom"]["engine"]>;
export type EngineStatus = EngineState["status"];
export type PaneState = ShapedValue<PluginState["planroom"]["pane"]>;
