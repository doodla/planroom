"""Settings for the worker, read from the environment."""

from __future__ import annotations

import os
from dataclasses import dataclass


@dataclass(frozen=True)
class Settings:
    redis_url: str
    pop_timeout_seconds: int = 5


def load(env: dict[str, str] | None = None) -> Settings:
    """Read settings. REDIS_URL is required."""
    env = dict(os.environ if env is None else env)
    missing = [k for k in ("REDIS_URL",) if not env.get(k)]
    if missing:
        raise RuntimeError(f"missing required environment variables: {', '.join(missing)}")
    return Settings(
        redis_url=env["REDIS_URL"],
        pop_timeout_seconds=int(env.get("POP_TIMEOUT_SECONDS", "5")),
    )
