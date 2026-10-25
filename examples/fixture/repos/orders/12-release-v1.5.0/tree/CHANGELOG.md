# Changelog

All notable changes to the orders service. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and versions follow
[Semantic Versioning](https://semver.org/).

## [1.5.0] - 2026-09-22

### Added

- `ship` requires a tracking number (the `hasTrackingNumber` guard), and
  orders carry `tracking_number`. Run `migrations/002_add_tracking_number.sql`.

### Changed

- `POST /orders` rejects a `currency` that is not an ISO 4217 code (422).
- Errors are RFC 9457 problem details (`application/problem+json`).

## [1.4.0] - 2026-08-25

### Added

- `POST /orders/{id}/events` applies a lifecycle event to an order.
- Paid orders are queued for fulfilment as `fulfil` jobs on the
  `order-jobs` Redis list.
- OpenTelemetry traces over OTLP, and request metrics on a Prometheus
  endpoint (`:9464`).
- CI workflow with lint, test and build jobs.
