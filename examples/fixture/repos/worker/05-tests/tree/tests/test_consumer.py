import json

from worker.consumer import JOBS_KEY, Consumer, Job, parse


def test_parse_reads_the_orders_job_format():
    raw = json.dumps({"kind": "fulfil", "order_id": "ord_1f2e3d4c5b6a7980", "enqueued_at": "2026-08-19T10:00:00Z"})
    assert parse(raw) == Job("fulfil", "ord_1f2e3d4c5b6a7980", "2026-08-19T10:00:00Z")


def test_pop_is_fifo(redis_client):
    for order_id in ("ord_0000000000000001", "ord_0000000000000002"):
        redis_client.lpush(JOBS_KEY, json.dumps({"kind": "fulfil", "order_id": order_id}))
    consumer = Consumer(redis_client, timeout_seconds=1)
    assert consumer.pop().order_id == "ord_0000000000000001"
    assert consumer.pop().order_id == "ord_0000000000000002"


def test_pop_drops_malformed_jobs(redis_client):
    redis_client.lpush(JOBS_KEY, b"not json")
    assert Consumer(redis_client, timeout_seconds=1).pop() is None


def test_pop_times_out_when_empty(redis_client):
    assert Consumer(redis_client, timeout_seconds=1).pop() is None
