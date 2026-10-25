# worker

The Acme Shop fulfilment worker. It pops `fulfil` jobs that the orders
service pushes onto the `order-jobs` Redis list, books a shipment, and moves
the order `paid -> fulfilling -> shipped`. It writes those state changes
straight into the orders database.

## Configuration

| Variable | Default | Meaning |
|---|---|---|
| `REDIS_URL` | (required) | Redis holding the `order-jobs` list. |
| `DATABASE_URL` | (required) | The orders Postgres database. |
| `METRICS_PORT` | `9464` | Prometheus `/metrics` port (`worker_jobs_processed_total`). |
| `POP_TIMEOUT_SECONDS` | `5` | How long one `BRPOP` waits. |
| `OTEL_EXPORTER_OTLP_ENDPOINT` | | Where traces go. |

## Develop

```sh
python -m venv .venv && . .venv/bin/activate
pip install -e '.[test]'
pytest -q
ruff check .
```
