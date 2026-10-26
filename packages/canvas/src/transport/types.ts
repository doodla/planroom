/**
 * The one `Transport` interface the canvas's store reads through (architecture.md, "The canvas";
 * DR-56): `local` talks to `planroom serve`, and `artifact` (later) to the published snapshot and
 * the Artifact `db`. Each declares what it can serve.
 */

import type { Envelope, Handle, Input, StreamFrame } from "@planroom/contract";

/** What a transport can serve. `local` serves every view and query. */
export interface Capabilities {
  readonly views: "all" | readonly string[];
  /** Whether layouts the canvas computes can be stored for every tab and the Artifact. */
  readonly layoutWriteBack: boolean;
}

/**
 * `PUT /api/v1/layout`: an auto-layout, a cache keyed by the view and structure the map's
 * `layout` names.
 */
export type LayoutPut = Input.LayoutPut;

/** Why a transport can't answer: the tab needs a fresh link, or the engine can't be reached. */
export class TransportError extends Error {
  constructor(
    readonly kind: "needs_login" | "unreachable",
    message: string,
  ) {
    super(message);
    this.name = "TransportError";
  }
}

/** A command's failure, as its envelope reports it. */
export class CommandError extends Error {
  constructor(readonly envelope: Envelope) {
    super(envelope.error?.message ?? `${envelope.command} failed`);
    this.name = "CommandError";
  }
}

/** Where a view the transport doesn't hold stands (architecture.md, "The canvas"; DR-77). */
export interface Requested {
  /** Not held: its button asks for it. Asked: waiting for the next sync. Refused: the sync couldn't answer. */
  readonly state: "unavailable" | "asked" | "refused";
  /** The engine-authored line the view shows. */
  readonly line: string;
  /** Why the sync couldn't answer, for a refused request. */
  readonly problem: string | null;
}

/**
 * What a transport that serves only part of the views adds (DR-56, DR-77): the artifact transport
 * holds the snapshot's views and the answers that arrived, and asks for the rest as read requests
 * the next sync answers. The local transport serves every view and has none.
 */
export interface PartialViews {
  /** Whether `query(command, request)` can answer now. */
  serves(command: string, request: object): boolean;
  /** Where a view `serves` refuses stands. */
  requested(command: string, request: object): Requested;
  /** Whether this viewer can ask: the page has the shared store and a viewer ID. */
  canAsk(): boolean;
  /** Ask for a view as a read request; the subscription hears when its answer arrives. */
  ask(command: string, request: object): Promise<void>;
  /** A line about what this view can't do, such as having no live store; null when it can. */
  note(): string | null;
}

export interface Transport {
  readonly capabilities: Capabilities;
  /** Views the transport holds only some of; null when it serves every view. */
  readonly partial: PartialViews | null;
  /**
   * Sign the tab in: exchange the one-time code in the URL fragment for a bearer token, or reuse
   * the tab's token. False when the tab has neither and needs a fresh link.
   */
  login(): Promise<boolean>;
  /** A read command's result. */
  query<T>(command: string, request: object): Promise<T>;
  /** A write command's result. */
  write<T>(command: string, request: object): Promise<T>;
  /** Run an engine-authored handle, such as an empty state's next action. */
  run(handle: Handle): Promise<unknown>;
  /** Store a computed layout. */
  saveLayout(layout: LayoutPut): Promise<void>;
  /** Live frames until the returned function stops them; `onEnd` hears why the stream closed. */
  subscribe(
    onFrame: (frame: StreamFrame) => void,
    onEnd: (error: TransportError) => void,
  ): () => void;
}
