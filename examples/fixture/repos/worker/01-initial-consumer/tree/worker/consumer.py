"""Pops fulfilment jobs off the order-jobs Redis list."""

from __future__ import annotations

import json
import logging
from collections.abc import Iterator
from dataclasses import dataclass

import redis

log = logging.getLogger(__name__)

# The orders service LPUSHes onto this list (orders/internal/queue/publisher.go).
JOBS_KEY = "order-jobs"


@dataclass(frozen=True)
class Job:
    kind: str
    order_id: str
    enqueued_at: str | None = None


def parse(raw: bytes | str) -> Job:
    data = json.loads(raw)
    return Job(kind=data["kind"], order_id=data["order_id"], enqueued_at=data.get("enqueued_at"))


class Consumer:
    def __init__(self, client: redis.Redis, timeout_seconds: int = 5) -> None:
        self._redis = client
        self._timeout = timeout_seconds

    @classmethod
    def from_url(cls, url: str, timeout_seconds: int = 5) -> Consumer:
        return cls(redis.Redis.from_url(url), timeout_seconds)

    def pop(self) -> Job | None:
        """Wait up to the timeout for one job. LPUSH plus BRPOP keeps jobs in order."""
        item = self._redis.brpop([JOBS_KEY], timeout=self._timeout)
        if item is None:
            return None
        _key, raw = item
        try:
            return parse(raw)
        except (ValueError, KeyError):
            log.warning("dropping malformed job: %r", raw)
            return None

    def jobs(self) -> Iterator[Job]:
        """Yield jobs forever."""
        while True:
            job = self.pop()
            if job is not None:
                yield job
