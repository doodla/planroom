/**
 * A fake Artifact `db` for the specs, installed in the page with `page.addInitScript` (S11's CI
 * path; DR-31). It keeps the platform's rules as `artifact-capabilities` 0.2.90's `db.d.ts` states
 * them for the manifest `share` declares:
 *
 * - documents at even-segment paths, JSON objects, each with a version;
 * - the page writes last-writer-wins: `set`, `update` (which needs the document), `delete`;
 * - only the owner writes shared state and reads `inbox`; a contributor (`interact`) reads and
 *   writes only `inbox/<its own id>/...`; a write it may not make rejects `invalid_argument`, and a
 *   read it may not make sees a document that doesn't exist;
 * - `onSnapshot` delivers the current state soon after it subscribes, then every change.
 *
 * The session's side, which `ArtifactData` stands for in the specs, writes as the owner and pins
 * every write to an existing document with `if_version`: a batch with a stale or missing pin
 * writes nothing.
 *
 * The function runs in the page, so it is self-contained: no imports, no closure over the spec.
 */

/** A document as the session reads it. */
export interface StoredDoc {
  readonly path: string;
  readonly data: Record<string, unknown>;
  readonly version: number;
}

/** One write of a session batch, as an `ArtifactData batch` entry with its data inline. */
export interface SessionWrite {
  readonly op: "set" | "update" | "delete";
  readonly collection: string;
  readonly doc_id: string;
  readonly data?: Record<string, unknown>;
  readonly if_version?: number;
}

export interface FakeDbOptions {
  /** The viewer's ID; null for a signed-out viewer. */
  readonly uid: string | null;
  /** The viewer's level: the owner, or a contributor. */
  readonly level: "owner" | "interact";
  /** Documents the session wrote before the page opened, each at version 1. */
  readonly docs: Record<string, Record<string, unknown>>;
}

/** What the shim puts on `window.__artifactDb`. */
export interface FakeDb {
  /** The `db` capability's namespace, as `claude.use("db")` resolves it. */
  readonly page: unknown;
  readonly session: {
    get(path: string): StoredDoc | null;
    list(collection: string): StoredDoc[];
    batch(writes: readonly SessionWrite[]): void;
  };
}

declare global {
  interface Window {
    __artifactDb?: FakeDb;
  }
}

export function installArtifactDb(options: FakeDbOptions): void {
  type Data = Record<string, unknown>;
  type Snap = {
    id: string;
    exists: boolean;
    data(): Data | undefined;
    metadata: { fromCache: boolean; hasPendingWrites: boolean };
  };
  const docs = new Map<string, { data: Data; version: number }>();
  const listeners = new Map<string, Set<() => void>>();
  for (const [path, data] of Object.entries(options.docs)) {
    docs.set(path, { data: structuredClone(data), version: 1 });
  }

  const segments = (path: string) => path.split("/");
  const isDoc = (path: string) => segments(path).length % 2 === 0;
  const lastOf = (path: string) => segments(path).at(-1) ?? "";
  const rejected = (message: string) =>
    Promise.reject(
      Object.assign(new Error(message), { code: "invalid_argument", message }),
    );

  /** `inbox` is the owner's; `inbox/{self}` each contributor's; the rest is read by all. */
  const readable = (path: string) => {
    const s = segments(path);
    if (s[0] !== "inbox") {
      return true;
    }
    return options.level === "owner" || s[1] === options.uid;
  };
  const writable = (path: string) => {
    if (options.level === "owner") {
      return true;
    }
    const s = segments(path);
    return s[0] === "inbox" && options.uid !== null && s[1] === options.uid;
  };

  const notify = (path: string) => {
    for (const listener of listeners.get(path) ?? []) {
      setTimeout(listener, 0);
    }
  };

  const snapOf = (path: string): Snap => {
    const held = readable(path) ? docs.get(path) : undefined;
    const data = held
      ? (Object.freeze(structuredClone(held.data)) as Data)
      : undefined;
    return {
      id: lastOf(path),
      exists: held !== undefined,
      data: () => data,
      metadata: { fromCache: false, hasPendingWrites: false },
    };
  };

  const write = (path: string, data: Data | null) => {
    const held = docs.get(path);
    if (data === null) {
      docs.delete(path);
    } else {
      docs.set(path, { data, version: (held?.version ?? 0) + 1 });
    }
    notify(path);
  };

  let minted = 0;
  const mint = () => {
    minted += 1;
    return `${Date.now().toString(36)}${minted.toString(36)}`.padEnd(20, "x");
  };

  const docRef = (path: string): unknown => {
    if (!isDoc(path)) {
      throw new TypeError(
        `${path}: a document path has an even number of segments`,
      );
    }
    return {
      id: lastOf(path),
      path,
      get: () => Promise.resolve(snapOf(path)),
      set(data: Data) {
        if (!writable(path)) {
          return rejected(`${path}: not writable at this level`);
        }
        write(path, structuredClone(data));
        return Promise.resolve();
      },
      update(data: Data) {
        const held = docs.get(path);
        if (!writable(path) || held === undefined) {
          return rejected(`${path}: no such document to update`);
        }
        write(path, { ...held.data, ...structuredClone(data) });
        return Promise.resolve();
      },
      delete() {
        if (!writable(path)) {
          return rejected(`${path}: not writable at this level`);
        }
        write(path, null);
        return Promise.resolve();
      },
      onSnapshot(next: (snap: Snap) => void) {
        const deliver = () => next(snapOf(path));
        const set = listeners.get(path) ?? new Set();
        set.add(deliver);
        listeners.set(path, set);
        setTimeout(deliver, 0);
        return () => {
          set.delete(deliver);
        };
      },
      collection: (sub: string) => collectionRef(`${path}/${sub}`),
    };
  };

  const collectionRef = (path: string): unknown => {
    if (isDoc(path)) {
      throw new TypeError(
        `${path}: a collection path has an odd number of segments`,
      );
    }
    return {
      path,
      doc: (id?: string) => docRef(`${path}/${id ?? mint()}`),
      add: async (data: Data) => {
        const ref = docRef(`${path}/${mint()}`) as {
          set(d: Data): Promise<void>;
        };
        await ref.set(data);
        return ref;
      },
    };
  };

  const inCollection = (collection: string) => {
    const depth = segments(collection).length + 1;
    return [...docs]
      .filter(
        ([path]) =>
          path.startsWith(`${collection}/`) && segments(path).length === depth,
      )
      .map(([path, d]) => ({
        path,
        data: structuredClone(d.data),
        version: d.version,
      }));
  };

  window.__artifactDb = {
    page: Object.freeze({ doc: docRef, collection: collectionRef }),
    session: {
      get(path) {
        const held = docs.get(path);
        return held
          ? { path, data: structuredClone(held.data), version: held.version }
          : null;
      },
      list: inCollection,
      batch(writes) {
        // All or nothing: every pin is checked before anything is written.
        for (const w of writes) {
          const held = docs.get(`${w.collection}/${w.doc_id}`);
          if (held !== undefined && w.if_version !== held.version) {
            throw new Error(
              `${w.collection}/${w.doc_id}: pinned to ${w.if_version}, at ${held.version}`,
            );
          }
          if (held === undefined && w.op !== "set" && w.op !== "delete") {
            throw new Error(`${w.collection}/${w.doc_id}: no such document`);
          }
        }
        for (const w of writes) {
          const path = `${w.collection}/${w.doc_id}`;
          if (w.op === "delete") {
            write(path, null);
          } else if (w.op === "update") {
            write(path, {
              ...docs.get(path)!.data,
              ...structuredClone(w.data ?? {}),
            });
          } else {
            write(path, structuredClone(w.data ?? {}));
          }
        }
      },
    },
  };
}
