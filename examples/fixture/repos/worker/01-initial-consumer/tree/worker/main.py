"""Entry point: pop jobs from Redis and handle them until stopped."""

from __future__ import annotations

import logging
import signal
import sys

from worker import config, jobs
from worker.consumer import Consumer

log = logging.getLogger("worker")


def main() -> int:
    logging.basicConfig(level=logging.INFO, format="%(asctime)s %(levelname)s %(name)s %(message)s")
    settings = config.load()
    consumer = Consumer.from_url(settings.redis_url, settings.pop_timeout_seconds)

    stopping = False

    def stop(signum: int, _frame: object) -> None:
        nonlocal stopping
        log.info("received signal %s, finishing the current job", signum)
        stopping = True

    signal.signal(signal.SIGTERM, stop)
    signal.signal(signal.SIGINT, stop)

    log.info("worker started")
    while not stopping:
        job = consumer.pop()
        if job is None:
            continue
        try:
            outcome = jobs.handle(job, settings)
        except Exception:
            log.exception("job failed: %s", job)
            outcome = "failed"
        log.info("job %s for %s: %s", job.kind, job.order_id, outcome)
    return 0


if __name__ == "__main__":
    sys.exit(main())
