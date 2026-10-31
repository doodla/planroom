/**
 * Send the workspace's pending edits to the engine, oldest first, each with the idempotency key
 * and the tab ID it was made with (DR-8, DR-73): an edit the engine already wrote replays instead
 * of applying twice, and an edit whose view moved on reports a conflict.
 */

import type { Input } from "@planroom/contract";

import {
  CommandError,
  TransportError,
  type Transport,
} from "../transport/types";
import type { PendingStore } from "./pending";

export type FlushOutcome =
  | {
      idem: string;
      state: "landed";
      result: Input.EditResult;
      summary: string | null;
      replayed: boolean;
    }
  | { idem: string; state: "conflict"; message: string }
  | { idem: string; state: "refused"; message: string; hint: string | null }
  /** Refused for now: the registry's retry class is `now` or `later`, so the edit stays pending. */
  | { idem: string; state: "held"; message: string; hint: string | null };

export interface Flushed {
  readonly outcomes: readonly FlushOutcome[];
  /** The error that stopped the flush; the edits it did not reach stay pending. */
  readonly stopped: TransportError | null;
}

/**
 * Write each pending edit in order. A landed, conflicting or refused edit leaves the store; an
 * unreachable engine, a signed-out tab, or a refusal whose retry class is `now` or `later` (such
 * as `store_busy` or `serve_limit`) stops the flush and keeps that edit and the rest, in order, for
 * the next flush, which sends it with the same idem, so one that did commit replays.
 */
export async function flush(
  transport: Transport,
  pending: PendingStore,
): Promise<Flushed> {
  const outcomes: FlushOutcome[] = [];
  for (const entry of pending.list()) {
    try {
      // The entry's client always wins: a flush never writes as the flushing tab.
      const reply = await transport.write<Input.EditResult>(
        "edit",
        { ...entry.request, client: entry.client },
        { idem: entry.idem },
      );
      pending.remove(entry.idem);
      outcomes.push({
        idem: entry.idem,
        state: "landed",
        result: reply.result,
        summary: reply.summary,
        replayed: reply.replayed,
      });
    } catch (e) {
      if (e instanceof TransportError) {
        return { outcomes, stopped: e };
      }
      if (e instanceof CommandError) {
        const error = e.envelope.error;
        if (error?.retry === "now" || error?.retry === "later") {
          outcomes.push({
            idem: entry.idem,
            state: "held",
            message: e.message,
            hint: error.hint ?? null,
          });
          return { outcomes, stopped: null };
        }
        pending.remove(entry.idem);
        outcomes.push(
          error?.code === "stale_write"
            ? { idem: entry.idem, state: "conflict", message: error.message }
            : {
                idem: entry.idem,
                state: "refused",
                message: e.message,
                hint: error?.hint ?? null,
              },
        );
        continue;
      }
      throw e;
    }
  }
  return { outcomes, stopped: null };
}
