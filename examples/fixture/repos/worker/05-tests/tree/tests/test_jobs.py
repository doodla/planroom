import pytest

from worker import jobs
from worker.config import Settings
from worker.consumer import Job

SETTINGS = Settings(redis_url="redis://unused", database_url="postgresql://unused")
JOB = Job(kind="fulfil", order_id="ord_1f2e3d4c5b6a7980")


def test_fulfil_moves_a_paid_order_to_shipped(monkeypatch):
    moves = []
    monkeypatch.setattr(jobs.db, "set_order_state", lambda url, oid, frm, to, **kw: moves.append((frm, to)))
    monkeypatch.setattr(jobs, "book_shipment", lambda order_id: "1ZTEST")
    assert jobs.handle(JOB, SETTINGS) == "shipped"
    assert moves == [("paid", "fulfilling"), ("fulfilling", "shipped")]


def test_unknown_kind_raises():
    with pytest.raises(jobs.UnknownJobKind):
        jobs.handle(Job(kind="refund", order_id=JOB.order_id), SETTINGS)
