# platform

How Acme Shop runs: one `docker-compose.yml` for every service, plus the
observability stack and a few operational tools.

| Path | What |
|---|---|
| `docker-compose.yml` | All services and the image version each one runs. |
| `otel/collector.yaml` | OpenTelemetry Collector: OTLP in, Prometheus exporter out. |
| `prometheus/prometheus.yml` | Scrape config: `orders`, `worker`, `otel-collector`. |
| `prometheus/alerts.yml` | Alert rules. |
| `grafana/` | Provisioned data source and dashboards. |
| `tools/healthcheck.rb` | Pings the services' `/healthz` endpoints. |

## Deploying a new version

Bump the service's `image:` tag in `docker-compose.yml`, commit, and run
`docker compose up -d <service>`. Platform releases are tagged by date
(`YYYY.MM.N`).
