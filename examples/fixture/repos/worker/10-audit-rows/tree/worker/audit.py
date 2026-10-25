"""Audit trail: one row per job the worker handled, in orders-db."""

from __future__ import annotations

import logging
import os

import psycopg

from worker.consumer import Job

log = logging.getLogger(__name__)

_INSERT = "INSERT INTO worker_audit (order_id, job_kind, outcome, at) VALUES (%s, %s, %s, now())"


def record(job: Job, outcome: str) -> None:
    """Write an audit row. Best effort: an audit failure never fails the job."""
    try:
        with psycopg.connect(os.environ["DATABASE_URL"], autocommit=True) as conn:
            conn.execute(_INSERT, (job.order_id, job.kind, outcome))
    except (KeyError, psycopg.Error):
        log.warning("could not write an audit row for %s", job.order_id, exc_info=True)
