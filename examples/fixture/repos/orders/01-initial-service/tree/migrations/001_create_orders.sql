-- Orders and their event log. The orders service owns this schema.
CREATE TABLE orders (
    id           text PRIMARY KEY,
    customer_id  text NOT NULL,
    items        jsonb NOT NULL,
    total_cents  bigint NOT NULL,
    currency     text NOT NULL DEFAULT 'USD',
    state        text NOT NULL,
    created_at   timestamptz NOT NULL,
    updated_at   timestamptz NOT NULL
);

CREATE TABLE order_events (
    id          bigserial PRIMARY KEY,
    order_id    text NOT NULL REFERENCES orders (id),
    from_state  text NOT NULL,
    to_state    text NOT NULL,
    detail      text NOT NULL DEFAULT '',
    at          timestamptz NOT NULL
);

CREATE INDEX order_events_order_id_idx ON order_events (order_id);
