# Changelog

All notable changes to the worker. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [0.9.0] - 2026-09-16

### Added

- Shipped orders get their `tracking_number` (needs the orders migration
  `002_add_tracking_number.sql`).

### Fixed

- `fulfil` jobs for orders that are no longer `paid` are skipped instead of
  failing.

## [0.8.0] - 2026-08-22

### Added

- `fulfil` jobs move orders `paid -> fulfilling -> shipped` in the orders
  database and record each step in `order_events`.
- OpenTelemetry traces over OTLP and a `worker_jobs_processed_total`
  counter on `:9464`.
- Container image (`acme/worker`).
