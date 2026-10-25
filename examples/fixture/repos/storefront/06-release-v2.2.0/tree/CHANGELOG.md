# Changelog

All notable changes to the storefront. The format follows
[Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [2.2.0] - 2026-08-21

### Added

- Checkout creates orders through the orders API (`POST /orders`).
- Money and order IDs are formatted with `@acme/kit` 1.0.0.
- OpenTelemetry traces exported over OTLP.
- Container image (`acme/storefront`).
