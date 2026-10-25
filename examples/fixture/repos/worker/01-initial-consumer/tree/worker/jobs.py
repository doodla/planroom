"""Fulfilment job handlers, one function per job kind."""

from __future__ import annotations

import logging
import secrets

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
    tracking = book_shipment(job.order_id)
    # TODO: record fulfilling/shipped on the order.
    log.info("shipped %s with tracking number %s", job.order_id, tracking)
    return "shipped"


HANDLERS = {"fulfil": fulfil}


def handle(job: Job, settings: Settings) -> str:
    """Run the handler for job.kind and return its outcome."""
    try:
        handler = HANDLERS[job.kind]
    except KeyError:
        raise UnknownJobKind(job.kind) from None
    return handler(job, settings)
