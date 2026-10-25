# Examples and the fixture

This directory holds the test fixture that every acceptance scenario runs on, the truth it's checked against, and the prompt snapshot.

| Path | Purpose |
|---|---|
| `fixture/repos/<repo>/<NN-slug>/` | Each repo's history as ordered commit overlays (see [History format](#history-format)). |
| `fixture/reviews/orders-pr-42.json` | Review threads for the fixture PR, in `planroom.review-threads/v1` (see [Review threads](#review-threads)). |
| `fixture/implementer-runs.jsonl` | Seeded history of implementer runs, in `planroom.implementer-run/v1` (see [Implementer runs](#implementer-runs)). |
| `fixture/implementer/<spec-key>.patch` | Canned output of the fixture implementer, the deterministic stand-in for the implementer agent. |
| `build-fixture` | Builds fresh git repos from `fixture/` into a directory (Python 3.11+, standard library only). Deterministic: the same input gives the same commit IDs. |
| `TRUTH.json` | The frozen expected answers. Changed only through a decision record. |
| `kickoff-snapshot.md` | A copy of `KICKOFF.md` taken at kickoff. S5 and S6 run on it. Never apply diffs to the root `KICKOFF.md`. |

## The workspace: Acme Shop

Five small repos, one polyglot system:

| Repo | Language | What it is | Tags |
|---|---|---|---|
| `storefront` | TypeScript (Node) | Web app: catalog, cart, checkout. Calls the orders API. Uses `@acme/kit`. | `v2.2.0`, `v2.3.0` |
| `orders` | Go | Orders service: HTTP API (OpenAPI spec), order lifecycle, Postgres store, publishes jobs to Redis. | `v1.4.0`, `v1.5.0` |
| `worker` | Python | Fulfilment worker: consumes jobs from Redis, writes order status **straight into the orders database** (the S4 smell). | `v0.8.0`, `v0.9.0` |
| `kit` | TypeScript | Shared library `@acme/kit` (money formatting, order IDs), published to npm on tag. | `v1.0.0`, `v1.1.0`, `v1.2.0` |
| `platform` | YAML, JSON, Ruby | docker-compose, OpenTelemetry collector, Prometheus scrape config and alert rule, Grafana dashboard, a Ruby health-check script. | `2026.10.0` |

### Runtime wiring (`platform/docker-compose.yml`)

| Service | Image | Talks to | Via |
|---|---|---|---|
| `storefront` | `acme/storefront:2.3.0` | `orders` | `ORDERS_URL=http://orders:8080` |
| `orders` | `acme/orders:1.4.0` (stale: repo is at `v1.5.0`) | `orders-db`, `redis` | `DATABASE_URL`, `REDIS_URL` |
| `worker` | `acme/worker:0.9.0` | `redis`, `orders-db` | `REDIS_URL`, `DATABASE_URL` (points at `orders-db`) |
| `redis` | `redis:7.4` | | |
| `orders-db` | `postgres:16.4` | | |
| `otel-collector` | `otel/opentelemetry-collector-contrib:0.111.0` | `prometheus` | OTLP in, Prometheus exporter out |
| `prometheus` | `prom/prometheus:v2.54.1` | scrapes `orders`, `worker`, `otel-collector` | `prometheus/prometheus.yml` |
| `grafana` | `grafana/grafana:11.2.2` | `prometheus` | provisioned dashboard |

**How the web app reaches the worker:** `storefront` POSTs `/orders` to `orders` (client in `storefront/src/api/orders.ts`, path in `orders/api/openapi.yaml`, URL from compose). `orders` pushes an `order-jobs` message onto `redis` (`orders/internal/queue/publisher.go`). `worker` pops `order-jobs` (`worker/worker/consumer.py`).

### Observability

- **Telemetry.** `storefront`: OTel traces only (`src/app/telemetry.ts`). `orders`: OTel traces and metrics (`internal/telemetry/otel.go`, Prometheus endpoint on `:9464`). `worker`: OTel traces and a `worker_jobs_processed_total` counter (`worker/telemetry.py`, `:9464`). `redis` and `orders-db` have no exporter.
- **Pipeline.** SDKs export OTLP to `otel-collector` (`platform/otel/collector.yaml`), which exposes a Prometheus exporter. Prometheus scrapes `orders:9464`, `worker:9464` and `otel-collector:8889`.
- **Alerts.** One rule, `OrdersHighErrorRate`, on the orders 5xx ratio (`platform/prometheus/alerts.yml`).
- **Dashboards.** `orders-overview` (`platform/grafana/dashboards/orders-overview.json`): orders request rate, error ratio, p95 latency, and worker jobs processed.
- **So, unmonitored** (nothing fires if it breaks): `storefront`, `worker`, `redis`, `orders-db`. Observability infrastructure (`otel-collector`, `prometheus`, `grafana`) is out of scope for coverage.

### Releases and versions

- `storefront` pins `@acme/kit` at `1.1.0`; `kit` is at `v1.2.0`. **Stale.**
- `platform` deploys `acme/orders:1.4.0`; `orders` is at `v1.5.0`. **Stale.** `storefront` and `worker` images match their latest tags.
- To ship a change to `kit`: release `kit`, then `storefront` (bump `@acme/kit`, release), then `platform` (bump the `storefront` image).

### The order lifecycle (`orders/internal/order/lifecycle.go`)

States: `created`, `paid`, `payment_failed`, `fulfilling`, `shipped`, `delivered`, `cancelled`, `on_hold`. Terminal: `delivered`, `cancelled`. Initial: `created`.

| From | Event | To | Guard |
|---|---|---|---|
| `created` | `pay` | `paid` | |
| `created` | `payment_fail` | `payment_failed` | |
| `created` | `cancel` | `cancelled` | |
| `paid` | `fulfil` | `fulfilling` | |
| `paid` | `cancel` | `cancelled` | |
| `fulfilling` | `ship` | `shipped` | `hasTrackingNumber` |
| `shipped` | `deliver` | `delivered` | |
| `on_hold` | `release` | `paid` | |

`orders/docs/order-lifecycle.md` describes the same machine, plus one transition the code lacks: `fulfilling --cancel--> cancelled`.

## Planted known positives

Each has an exact name that tests and `TRUTH.json` use.

| Name | Kind | Where |
|---|---|---|
| `on_hold` | Unreachable state | `order-lifecycle` machine in `orders/internal/order/lifecycle.go`: no transition enters it. |
| `payment_failed` | Dead end | Same machine: not terminal, no way out. |
| `fulfilling-cancel` | Doc-vs-code drift | The doc allows `fulfilling --cancel--> cancelled`; the code doesn't. |
| `storefront-gift-wrap` | Unplanned change | Branch `teammates` in `storefront`: "Add gift wrap option to checkout". No plan covers it. |
| `worker-audit-db-write` | Regression | Branch `teammates` in `worker`: "Write audit rows straight to orders-db". After S4 makes `worker -/-> orders-db` intended and the hand-off satisfies it, this commit breaks it again. |

## Scenario hooks

- **S3** plans "Add retries to the worker". Next actions attach to `worker/worker/consumer.py`, `platform/docker-compose.yml` and `platform/prometheus/alerts.yml`.
- **S4** regroups `storefront/src/cart` and `storefront/src/checkout` into a group named `purchase`, then changes the intended architecture: `worker` reports status through the orders API (`POST /orders/{id}/events`, which `orders` already serves) and must not depend on `orders-db`. The fixture implementer answers that change spec with `fixture/implementer/worker-status-via-api.patch`, which touches only `worker`. After the merge, the test merges each repo's `teammates` branch, which lands both planted changes.
- **S5** breaks down `orders` PR 42 ("Add cancellation reasons", branch `feature/cancel-reasons`): thread `T1` is addressed by a later commit, `T2` and `T3` stay open. It touches `orders/internal/http`, `orders/internal/order` and `orders/api/openapi.yaml`. S5 also breaks down the skill `worker/.claude/skills/run-worker-tests/SKILL.md` and the workflow `orders/.github/workflows/ci.yml`.
- **S10** uses `platform/tools/healthcheck.rb` (Ruby has no built-in extractor) and `storefront/flags.yaml` (read by the toy pack).
- **S7** uses branch `routine` in `storefront`: one small commit, "Show 24 products per catalog page", that a commit-triggered rescan should absorb with no Claude call and at most one card.
- **S12** seeds 8 implementer runs against `worker/CLAUDE.md` and `storefront`, which don't yet mention either lesson. 4 hit the same gotcha (worker tests need `REDIS_URL`, and fail with `redis.exceptions.ConnectionError` without it). 3 repeat the same procedure (regenerate the storefront orders client after `orders/api/openapi.yaml` changes: `npm run gen:orders-client`, then `npm test`).

## History format

`fixture/repos/<repo>/` holds one directory per commit, applied in name order: `NN-slug/`. Each has:

- `COMMIT.toml`: `message`, `author` (one of the people below), `date` (RFC 3339, UTC), optional `tag`, optional `branch` (default `main`; a branch starts from the `main` commit before its first step and stays on it), optional `delete = [paths]`;
- `tree/`: files to add or overwrite, at their repo-relative paths.

People: `Alice Ng <alice@acme.test>` (storefront, kit), `Bob Okafor <bob@acme.test>` (orders), `Carol Diaz <carol@acme.test>` (worker), `Dev Patel <dev@acme.test>` (platform).

`build-fixture <out-dir>` makes `<out-dir>/<repo>/` for each repo with `git init -b main`, applies the steps, and prints a JSON map of repo to branch heads. It pins everything that affects commit IDs: author and committer names, emails and dates from `COMMIT.toml`, `TZ=UTC`, no signing, no hooks, a fixed file mode, and `core.autocrlf=false`. All merges happen in that copy, never in `fixture/`.

## Review threads

`planroom.review-threads/v1`, one file per PR:

```json
{
  "format": "planroom.review-threads/v1",
  "repo": "orders",
  "pr": { "number": 42, "title": "…", "author": "bob", "base": "main", "head": "feature/cancel-reasons", "state": "open", "body": "…" },
  "threads": [
    {
      "id": "T1",
      "path": "internal/http/handlers.go",
      "line": 57,
      "commit_subject": "Accept a cancellation reason",
      "resolved": false,
      "comments": [{ "author": "carol", "at": "2026-09-14T10:02:00Z", "body": "…" }]
    }
  ]
}
```

Threads anchor to a commit by subject, not SHA, so the file doesn't depend on commit IDs. `line` is a line in that commit's version of `path`.

## Implementer runs

`planroom.implementer-run/v1`, one JSON object per line:

```json
{
  "format": "planroom.implementer-run/v1",
  "run_id": "run-0001",
  "change_spec": "spec-…",
  "repo": "worker",
  "started_at": "2026-09-01T09:00:00Z",
  "finished_at": "2026-09-01T09:12:00Z",
  "outcome": "merged",
  "steps": [
    { "kind": "command", "command": "pytest -q", "exit_code": 1, "output_excerpt": "redis.exceptions.ConnectionError: …" },
    { "kind": "edit", "path": "worker/consumer.py", "summary": "…" },
    { "kind": "note", "text": "…" }
  ]
}
```

`outcome` is `merged`, `rejected` or `abandoned`. `steps[].kind` is `command`, `edit` or `note`.
