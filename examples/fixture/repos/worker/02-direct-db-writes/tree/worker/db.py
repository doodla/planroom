"""Order state, read and written straight in the orders database (orders-db).

The orders service owns this schema (orders/migrations); the worker writes to
it directly to record fulfilment progress.
"""

from __future__ import annotations

import psycopg


class StaleOrderError(Exception):
    """The order was not in the state we expected, so nothing was written."""


_conn: psycopg.Connection | None = None


def connect(database_url: str) -> psycopg.Connection:
    global _conn
    if _conn is None or _conn.closed:
        _conn = psycopg.connect(database_url)
    return _conn


def set_order_state(
    database_url: str,
    order_id: str,
    from_state: str,
    to_state: str,
    *,
    detail: str = "",
) -> None:
    """Move an order from from_state to to_state and log it in order_events."""
    conn = connect(database_url)
    with conn.transaction(), conn.cursor() as cur:
        cur.execute(
            "UPDATE orders SET state = %s, updated_at = now() WHERE id = %s AND state = %s",
            (to_state, order_id, from_state),
        )
        if cur.rowcount != 1:
            raise StaleOrderError(f"order {order_id} is not {from_state}")
        cur.execute(
            "INSERT INTO order_events (order_id, from_state, to_state, detail, at) "
            "VALUES (%s, %s, %s, %s, now())",
            (order_id, from_state, to_state, detail),
        )
