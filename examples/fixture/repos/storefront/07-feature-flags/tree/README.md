# storefront

The Acme Shop web app: catalog, cart and checkout. It is a small Express
server that talks to the orders API and uses `@acme/kit` for money and order
IDs.

## Run it

```sh
npm install
ORDERS_URL=http://localhost:8080 npm run dev
```

| Variable | Default | Meaning |
|---|---|---|
| `PORT` | `3000` | Listen port. |
| `ORDERS_URL` | `http://localhost:8080` | Base URL of the orders API. |
| `FLAGS_PATH` | `flags.yaml` | Feature flag file. |
| `OTEL_EXPORTER_OTLP_ENDPOINT` | | Where traces go. |

## Layout

- `src/catalog`: product listing.
- `src/cart`: per-session carts.
- `src/checkout`: turns a cart into an order.
- `src/api`: the orders API client. `orders.schema.ts` is generated from
  `../orders/api/openapi.yaml` by `npm run gen:orders-client`.
- `src/app`: config, feature flags, telemetry.

## Test

```sh
npm test
```
