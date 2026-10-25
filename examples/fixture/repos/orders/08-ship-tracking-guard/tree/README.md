# orders

The Acme Shop orders service. It owns orders and their lifecycle, stores
them in Postgres (`orders-db`), and hands paid orders to the fulfilment
worker over Redis.

## API

The HTTP API is described in [`api/openapi.yaml`](api/openapi.yaml):

| Method | Path | What it does |
|---|---|---|
| `POST` | `/orders` | Create an order. With `payment_ref` it starts `paid`. |
| `GET` | `/orders/{id}` | Read an order. |
| `POST` | `/orders/{id}/events` | Apply a lifecycle event (`pay`, `fulfil`, `ship`, ...). |
| `GET` | `/healthz` | Liveness plus a database ping. |

The lifecycle itself is in `internal/order/lifecycle.go` and described in
[`docs/order-lifecycle.md`](docs/order-lifecycle.md).

## Configuration

| Variable | Default | Meaning |
|---|---|---|
| `ORDERS_ADDR` | `:8080` | API listen address. |
| `DATABASE_URL` | (required) | Postgres connection string. |
| `REDIS_URL` | (required) | Redis for the `order-jobs` queue. |
| `METRICS_ADDR` | `:9464` | Prometheus metrics endpoint. |
| `OTEL_EXPORTER_OTLP_ENDPOINT` | | Where traces go (the collector). |

## Develop

```sh
for f in migrations/*.sql; do psql "$DATABASE_URL" -f "$f"; done
make test
make run
```
