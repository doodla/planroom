"""Entry point: pop jobs from Redis and handle them until stopped."""

from __future__ import annotations

import logging
import signal
import sys

from worker import config, jobs, telemetry
from worker.consumer import Consumer

log = logging.getLogger("worker")


def main() -> int:
    logging.basicConfig(level=logging.INFO, format="%(asctime)s %(levelname)s %(name)s %(message)s")
    settings = config.load()
    tracer = telemetry.setup(settings.metrics_port)
    consumer = Consumer.from_url(settings.redis_url, settings.pop_timeout_seconds)

    stopping = False

    def stop(signum: int, _frame: object) -> None:
        nonlocal stopping
        log.info("received signal %s, finishing the current job", signum)
        stopping = True

    signal.signal(signal.SIGTERM, stop)
    signal.signal(signal.SIGINT, stop)

    log.info("worker started, metrics on :%d", settings.metrics_port)
    while not stopping:
        job = consumer.pop()
        if job is None:
            continue
        with tracer.start_as_current_span(f"job {job.kind}") as span:
            span.set_attribute("order.id", job.order_id)
            try:
                outcome = jobs.handle(job, settings)
            except Exception:
                log.exception("job failed: %s", job)
                outcome = "failed"
        telemetry.JOBS_PROCESSED.labels(kind=job.kind, outcome=outcome).inc()
    return 0


if __name__ == "__main__":
    sys.exit(main())
