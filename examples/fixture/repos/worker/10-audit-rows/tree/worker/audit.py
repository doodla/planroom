"""Audit trail: one row per job the worker handled, in orders-db."""

from __future__ import annotations

import logging
import os

import psycopg

from worker.consumer import Job

log = logging.getLogger(__name__)

# The audit table lives next to the orders it describes, in the orders database.
DEFAULT_AUDIT_DSN = "postgresql://orders:orders@orders-db:5432/orders"

_INSERT = "INSERT INTO worker_audit (order_id, job_kind, outcome, at) VALUES (%s, %s, %s, now())"


def record(job: Job, outcome: str) -> None:
    """Write an audit row. Best effort: an audit failure never fails the job."""
    dsn = os.environ.get("AUDIT_DSN", DEFAULT_AUDIT_DSN)
    try:
        with psycopg.connect(dsn, autocommit=True) as conn:
            conn.execute(_INSERT, (job.order_id, job.kind, outcome))
    except psycopg.Error:
        log.warning("could not write an audit row for %s", job.order_id, exc_info=True)
