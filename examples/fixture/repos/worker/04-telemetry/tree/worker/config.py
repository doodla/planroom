"""Settings for the worker, read from the environment."""

from __future__ import annotations

import os
from dataclasses import dataclass


@dataclass(frozen=True)
class Settings:
    redis_url: str
    database_url: str
    metrics_port: int = 9464
    pop_timeout_seconds: int = 5


def load(env: dict[str, str] | None = None) -> Settings:
    """Read settings. REDIS_URL and DATABASE_URL are required."""
    env = dict(os.environ if env is None else env)
    missing = [k for k in ("REDIS_URL", "DATABASE_URL") if not env.get(k)]
    if missing:
        raise RuntimeError(f"missing required environment variables: {', '.join(missing)}")
    return Settings(
        redis_url=env["REDIS_URL"],
        database_url=env["DATABASE_URL"],
        metrics_port=int(env.get("METRICS_PORT", "9464")),
        pop_timeout_seconds=int(env.get("POP_TIMEOUT_SECONDS", "5")),
    )
