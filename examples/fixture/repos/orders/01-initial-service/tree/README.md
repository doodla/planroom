# orders

The Acme Shop orders service. It owns orders and their lifecycle and stores
them in Postgres (`orders-db`).

The HTTP API is described in [`api/openapi.yaml`](api/openapi.yaml), and the
lifecycle is the transitions table in `internal/order/lifecycle.go`.

## Develop

```sh
export DATABASE_URL=postgres://orders:orders@localhost:5432/orders?sslmode=disable
for f in migrations/*.sql; do psql "$DATABASE_URL" -f "$f"; done
make test
make run
```
