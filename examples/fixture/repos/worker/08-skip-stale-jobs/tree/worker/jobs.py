"""Fulfilment job handlers, one function per job kind."""

from __future__ import annotations

import logging
import secrets

from worker import db
from worker.config import Settings
from worker.consumer import Job

log = logging.getLogger(__name__)


class UnknownJobKind(Exception):
    pass


def book_shipment(order_id: str) -> str:
    """Stand-in for the carrier API: books a parcel and returns its tracking number."""
    log.info("booking shipment for %s", order_id)
    return "1Z" + secrets.token_hex(8).upper()


def fulfil(job: Job, settings: Settings) -> str:
    """Pick, pack and ship a paid order: paid -> fulfilling -> shipped."""
    state = db.get_order_state(settings.database_url, job.order_id)
    if state != "paid":
        log.info("skipping fulfil for %s: order is %s", job.order_id, state)
        return "skipped"
    db.set_order_state(settings.database_url, job.order_id, "paid", "fulfilling")
    tracking = book_shipment(job.order_id)
    db.set_order_state(
        settings.database_url,
        job.order_id,
        "fulfilling",
        "shipped",
        tracking_number=tracking,
        detail=tracking,
    )
    return "shipped"


HANDLERS = {"fulfil": fulfil}


def handle(job: Job, settings: Settings) -> str:
    """Run the handler for job.kind and return its outcome."""
    try:
        handler = HANDLERS[job.kind]
    except KeyError:
        raise UnknownJobKind(job.kind) from None
    return handler(job, settings)
