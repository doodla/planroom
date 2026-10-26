/**
 * ELK in a Web Worker, so a layout never blocks the page (ts-stack §3.2: about 0.3 s to 1.5 s at
 * 200 blocks). The page holds only ELK's small API; the layout runs in ELK's own worker script,
 * which Vite copies as a stable-named asset and loads by a URL relative to the chunk, so it works
 * from 127.0.0.1 and from an Artifact's sub-path alike [ts-stack §5, §8]. The worker starts on the
 * first layout, which only a structure with no stored positions needs.
 */

import ELK from "elkjs/lib/elk-api.js";
import workerUrl from "elkjs/lib/elk-worker.min.js?url";

import type { ElkGraphIn, ElkGraphOut } from "./graph";

let elk: InstanceType<typeof ELK> | null = null;

/** Lay out `graph` in the worker. */
export function layoutInWorker(graph: ElkGraphIn): Promise<ElkGraphOut> {
  elk ??= new ELK({
    workerFactory: () => new Worker(workerUrl, { name: "elk" }),
  });
  return elk.layout(graph);
}
