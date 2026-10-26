-- Store version 1: the ten tables of docs/contributing/architecture.md#store-schema.
-- `events.reply` holds the whole reply of the command that wrote the event, on its first event
-- only, so a retry with the same idem and request hash returns it (DR-8).

CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);

CREATE TABLE events (
    seq INTEGER PRIMARY KEY AUTOINCREMENT,
    id TEXT NOT NULL UNIQUE,
    idem TEXT NOT NULL UNIQUE,
    request_hash TEXT NOT NULL,
    at TEXT NOT NULL,
    actor TEXT NOT NULL,
    client TEXT NOT NULL,
    project TEXT,
    cause TEXT,
    kind TEXT NOT NULL,
    target TEXT,
    status TEXT NOT NULL,
    body TEXT NOT NULL,
    reply TEXT
);
CREATE INDEX events_target ON events(target, seq);
CREATE INDEX events_project ON events(project, seq);

CREATE TABLE snapshots (
    id INTEGER PRIMARY KEY,
    repo TEXT NOT NULL,
    commit_id TEXT NOT NULL,
    branch TEXT NOT NULL,
    purpose TEXT NOT NULL,
    packs_hash TEXT NOT NULL,
    UNIQUE (repo, commit_id, purpose, packs_hash)
);

CREATE TABLE blob_facts (
    blob TEXT NOT NULL,
    extractor TEXT NOT NULL,
    packs_hash TEXT NOT NULL,
    body TEXT NOT NULL,
    PRIMARY KEY (blob, extractor, packs_hash)
);

CREATE TABLE facts (
    snapshot INTEGER NOT NULL REFERENCES snapshots(id) ON DELETE CASCADE,
    id TEXT NOT NULL,
    shape TEXT NOT NULL,
    kind TEXT NOT NULL,
    hash TEXT NOT NULL,
    body TEXT NOT NULL,
    PRIMARY KEY (snapshot, id)
);

CREATE TABLE fact_rev (id TEXT PRIMARY KEY, hash TEXT NOT NULL, changed_seq INTEGER NOT NULL);

CREATE TABLE model_cache (
    key TEXT PRIMARY KEY,
    provider TEXT NOT NULL,
    model TEXT NOT NULL,
    response TEXT NOT NULL,
    usage TEXT NOT NULL,
    at TEXT NOT NULL
);

CREATE TABLE leases (name TEXT PRIMARY KEY, holder TEXT NOT NULL, epoch INTEGER NOT NULL, expires_at TEXT NOT NULL);

CREATE TABLE presence (
    actor TEXT NOT NULL,
    surface TEXT NOT NULL,
    client_id TEXT NOT NULL,
    doing TEXT,
    focus TEXT,
    seen_at TEXT NOT NULL,
    PRIMARY KEY (actor, surface, client_id)
);

CREATE TABLE layouts (
    view_key TEXT NOT NULL,
    structure_hash TEXT NOT NULL,
    positions TEXT NOT NULL,
    PRIMARY KEY (view_key, structure_hash)
);
