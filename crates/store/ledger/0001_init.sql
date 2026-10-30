-- Ledger version 1: the spend ledger of docs/contributing/architecture.md#store-schema, one per
-- user at DATA_ROOT/ledger.db, and `meta`, which records the oldest engine version that reads the
-- file (contracts.md, Store schema and migrations; DR-78). A later migration only adds: a table, or
-- a column that is nullable or has a default, so an older engine's INSERT still fits.

CREATE TABLE meta (key TEXT PRIMARY KEY, value TEXT NOT NULL);
INSERT INTO meta (key, value) VALUES ('min_reader_version', '0.1.0-dev');

CREATE TABLE ledger (id INTEGER PRIMARY KEY, at TEXT NOT NULL, workspace_id TEXT, provider TEXT NOT NULL,
                     model TEXT NOT NULL, purpose TEXT NOT NULL, scope TEXT NOT NULL, uncached_in INTEGER, cache_read INTEGER,
                     cache_write INTEGER, output INTEGER, cost_usd REAL, prefix_hash TEXT, generation_id TEXT, replayed INTEGER NOT NULL);
