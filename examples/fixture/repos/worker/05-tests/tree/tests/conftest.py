import os

import pytest
import redis

from worker.consumer import JOBS_KEY


@pytest.fixture
def redis_client():
    """A Redis client on a scratch database, with the jobs list emptied."""
    client = redis.Redis.from_url(os.environ.get("REDIS_URL", "redis://localhost:6379/15"))
    client.ping()
    client.delete(JOBS_KEY)
    yield client
    client.delete(JOBS_KEY)
    client.close()
