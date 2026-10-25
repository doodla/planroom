# Changelog

Platform releases are named `YYYY.MM.N`.

## [2026.10.0] - 2026-10-02

### Added

- `docker-compose.yml` with storefront, orders, worker, Redis and the orders
  Postgres database.
- OpenTelemetry Collector, Prometheus (scraping `orders`, `worker` and the
  collector) and Grafana with the `orders-overview` dashboard.
- `OrdersHighErrorRate` alert on the orders 5xx ratio.
- `tools/healthcheck.rb`.

### Images

- `acme/storefront:2.3.0`
- `acme/orders:1.4.0`
- `acme/worker:0.9.0`
