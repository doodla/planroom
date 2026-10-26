/**
 * The artifact transport (architecture.md, "Remote mode through the Artifact (S11)"; DR-56, DR-77):
 * the canvas published as an Artifact, where no engine answers. It draws from the published
 * `./data/snapshot.json`, follows the `db` document `live/head` the session writes, and asks for a
 * view the snapshot doesn't hold by writing a read request row to `inbox/{self}/events/{docId}`
 * (and bumping `inbox/{self}`), then draws the answer when `answers/{docId}` arrives.
 *
 * The page renders from the snapshot alone: `claude.use("db")` resolves later, or never, or to
 * null (signed out, not granted, a saved copy), and then the page says so and offers no request.
 * The platform's page API is the one `artifact-capabilities` 0.2.90 defines (`claude.d.ts`,
 * `db.d.ts`, `user.d.ts`); `ArtifactHost` below is the slice of it this transport calls.
 */

import type {
  Answer,
  Handle,
  InboxRow,
  LiveHead,
  MapView,
  Snapshot,
  StreamFrame,
} from "@planroom/contract";

import { STRINGS } from "../strings";
import { requestOf } from "./request";
import {
  TransportError,
  type Capabilities,
  type PartialViews,
  type Requested,
  type Transport,
} from "./types";

/** One document as a read or a subscription delivers it (`db.d.ts` `DocumentSnapshot`). */
export interface HostDoc {
  readonly id: string;
  readonly exists: boolean;
  data(): Record<string, unknown> | undefined;
}

/** A document reference (`db.d.ts` `DocumentReference`), as far as this transport calls it. */
export interface HostDocRef {
  readonly id: string;
  set(data: Record<string, unknown>): Promise<void>;
  update(data: Record<string, unknown>): Promise<void>;
  onSnapshot(
    next: (snap: HostDoc) => void,
    error?: (e: { code: string; message: string }) => void,
  ): () => void;
}

/** The `db` capability's namespace (`db.d.ts` `DB`). */
export interface HostDb {
  doc(path: string): HostDocRef;
  collection(path: string): { doc(id?: string): HostDocRef };
}

/** The `user` capability's namespace (`user.d.ts`), as far as this transport calls it. */
export interface HostUser {
  id(): Promise<string | null>;
}

/** `window.claude` (`claude.d.ts`): resolves a capability's namespace, or null. */
export interface ArtifactHost {
  use(name: "db"): Promise<HostDb | null>;
  use(name: "user"): Promise<HostUser | null>;
}

/** The browser pieces the transport uses, passed in so tests can stand them in. */
export interface ArtifactEnv {
  readonly fetch: typeof fetch;
  /** `window.claude`; absent on a copy of the page opened outside the platform. */
  readonly host: ArtifactHost | null;
  /** A fresh ID for this page load: the row's `instance` (DR-61). */
  readonly instance: () => string;
  readonly now: () => Date;
}

/** The browser's own pieces. */
export function browserArtifactEnv(): ArtifactEnv {
  const w = window as unknown as { claude?: ArtifactHost };
  return {
    fetch: window.fetch.bind(window),
    host: w.claude ?? null,
    instance: () => crypto.randomUUID(),
    now: () => new Date(),
  };
}

/** The view key a request names, as `Snapshot.capabilities` lists views: `map:<level>:<lens>`. */
export function viewKey(command: string, request: object): string {
  if (command === "map") {
    const r = request as { level?: string | null; lens?: string | null };
    return `map:${r.level ?? "system"}:${r.lens ?? "all"}`;
  }
  return command === "focus" ? "pane" : command;
}

const INBOX_FORMAT = "planroom.inbox/v1";

export function artifactTransport(env: ArtifactEnv): Transport {
  let snapshot: Snapshot | null = null;
  let head: LiveHead | null = null;
  let db: HostDb | null = null;
  let uid: string | null = null;
  /** Whether the platform has answered whether this view has a store. */
  let settled = false;
  let counter = 0;
  const instance = env.instance();
  /** Answers by view key: precomputed in the snapshot, or arrived from a sync. */
  const answers = new Map<string, Answer>();
  /** Read requests written and not yet answered, by view key. */
  const asked = new Map<string, string>();
  const listeners = new Set<(frame: StreamFrame) => void>();

  /** Tell the store to refetch: an answer arrived, the head moved, or the store came up. */
  const changed = () => {
    const seq = head?.seq ?? snapshot?.seq ?? 0;
    for (const listener of listeners) {
      listener({ type: "reset", seq });
    }
  };

  const mapOf = (key: string): MapView | null => {
    const held = snapshot?.maps.find((m) => `map:${m.level}:${m.lens}` === key);
    if (held) {
      return held;
    }
    const answer = answers.get(key);
    return answer && !answer.error
      ? (answer.result as unknown as MapView)
      : null;
  };

  /** Resolve the store and the viewer, then follow `live/head`; never blocks the first render. */
  async function connect(): Promise<void> {
    try {
      db = (await env.host?.use("db")) ?? null;
      const user = (await env.host?.use("user")) ?? null;
      uid = user ? await user.id() : null;
    } catch {
      db = null;
    }
    settled = true;
    if (db !== null) {
      // Subscribe once, for the page's life (db.d.ts: never from render code).
      db.doc("live/head").onSnapshot(
        (snap) => {
          if (snap.exists) {
            head = snap.data() as unknown as LiveHead;
            changed();
          }
        },
        () => undefined,
      );
    }
    changed();
  }

  /** Follow `answers/{docId}` until it arrives, then stop. */
  function follow(key: string, docId: string): void {
    if (db === null) {
      return;
    }
    let done = false;
    let stop: (() => void) | null = null;
    stop = db.doc(`answers/${docId}`).onSnapshot(
      (snap) => {
        if (done || !snap.exists) {
          return;
        }
        done = true;
        answers.set(key, snap.data() as unknown as Answer);
        asked.delete(key);
        stop?.();
        changed();
      },
      () => undefined,
    );
    if (done) {
      stop();
    }
  }

  const partial: PartialViews = {
    serves(command, request) {
      const key = viewKey(command, request);
      if (command === "map") {
        return mapOf(key) !== null;
      }
      return key === "pane";
    },
    requested(command, request): Requested {
      const key = viewKey(command, request);
      const answer = answers.get(key);
      if (answer?.error) {
        return {
          state: "refused",
          line: STRINGS.refused,
          problem: answer.error.message,
        };
      }
      if (asked.has(key)) {
        return { state: "asked", line: STRINGS.asked, problem: null };
      }
      return {
        state: "unavailable",
        line: snapshot?.unavailable.next.label ?? STRINGS.snapshotOnly,
        problem: null,
      };
    },
    canAsk: () => db !== null && uid !== null,
    async ask(command, request) {
      if (db === null || uid === null) {
        throw new TransportError("unreachable", "no shared store");
      }
      const key = viewKey(command, request);
      // A failed answer is asked for again.
      answers.delete(key);
      counter += 1;
      const ref = db.collection(`inbox/${uid}/events`).doc();
      const at = env.now().toISOString();
      const row: InboxRow = {
        format: INBOX_FORMAT,
        instance,
        counter,
        command,
        request: request as InboxRow["request"],
        base: head?.seq ?? snapshot?.seq ?? 0,
        at,
      };
      await ref.set(row as unknown as Record<string, unknown>);
      asked.set(key, ref.id);
      // Bump the viewer's own head, so the session can list who wrote.
      const own = db.doc(`inbox/${uid}`);
      try {
        await own.update({ seq: counter, updated_at: at });
      } catch {
        await own.set({ seq: counter, updated_at: at, sample: [] });
      }
      follow(key, ref.id);
    },
    note() {
      if (!settled) {
        return null;
      }
      return db === null || uid === null ? STRINGS.snapshotOnly : null;
    },
  };

  const capabilities: Capabilities = {
    get views() {
      return snapshot?.capabilities ?? [];
    },
    layoutWriteBack: false,
  };

  return {
    capabilities,
    partial,

    async login() {
      let response: Response;
      try {
        response = await env.fetch("./data/snapshot.json");
      } catch (e) {
        throw new TransportError("unreachable", String(e));
      }
      if (!response.ok) {
        throw new TransportError("unreachable", STRINGS.snapshotMissing);
      }
      snapshot = (await response.json()) as Snapshot;
      for (const answer of snapshot.answers) {
        const handle: Handle = answer.handle;
        answers.set(viewKey(handle.command, requestOf(handle)), answer);
      }
      void connect();
      return true;
    },

    query<T>(command: string, request: object): Promise<T> {
      if (snapshot === null) {
        return Promise.reject(
          new TransportError("unreachable", STRINGS.snapshotMissing),
        );
      }
      if (command === "focus") {
        return Promise.resolve(snapshot.pane as T);
      }
      const map = command === "map" ? mapOf(viewKey(command, request)) : null;
      if (map !== null) {
        return Promise.resolve(map as T);
      }
      return Promise.reject(
        new TransportError("unreachable", `${command}: not in this snapshot`),
      );
    },

    write() {
      return Promise.reject(
        new TransportError("unreachable", "the Artifact page writes only rows"),
      );
    },

    run(handle: Handle) {
      return Promise.reject(
        new TransportError("unreachable", `${handle.command}: not here`),
      );
    },

    saveLayout() {
      // Layouts are a cache the engine keeps; the page lays out again on each load.
      return Promise.resolve();
    },

    subscribe(onFrame) {
      listeners.add(onFrame);
      return () => listeners.delete(onFrame);
    },
  };
}
