/**
 * The one `Transport` interface the canvas's store reads through (architecture.md, "The canvas";
 * DR-56): `local` talks to `planroom serve`, and `artifact` (later) to the published snapshot and
 * the Artifact `db`. Each declares what it can serve.
 */

import type {
  Envelope,
  Handle,
  Position,
  StreamFrame,
} from "@planroom/contract";

/** What a transport can serve. `local` serves every view and query. */
export interface Capabilities {
  readonly views: "all" | readonly string[];
  /** Whether layouts the canvas computes can be stored for every tab and the Artifact. */
  readonly layoutWriteBack: boolean;
}

/** `PUT /api/v1/layout`: an auto-layout, a cache keyed by view and structure. */
export interface LayoutPut {
  readonly view_key: string;
  readonly structure_hash: string;
  readonly positions: Readonly<Record<string, Position>>;
}

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

export interface Transport {
  readonly capabilities: Capabilities;
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
