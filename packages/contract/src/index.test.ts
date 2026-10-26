import { expect, test } from "vitest";

import type { Envelope, Input, MapView } from "./index";

test("a reply and a request type-check against the generated contract", () => {
  const map: MapView = {
    level: "container",
    lens: "code",
    focus: null,
    nodes: [],
    edges: [],
    hidden: 0,
    empty: null,
  };
  const reply: Envelope = {
    schema_version: 1,
    ok: true,
    command: "map",
    result: map,
    meta: {
      seq: 412,
      took_ms: 38,
      size: { bytes: 2210, tokens_est: 553 },
      next: null,
      truncated: false,
      replayed: false,
      warnings: [],
      models: {
        jev: { calls: 0, cache_hits: 0, cost_usd: 0 },
        claude: { calls: 0, cache_hits: 0, cost_usd: 0 },
      },
      model_requests: [],
    },
  };
  // The input view lets defaulted and optional fields be left out.
  const request: Input.Command = { command: "map", args: {} };
  expect(reply.schema_version).toBe(1);
  expect(request.command).toBe("map");
});
