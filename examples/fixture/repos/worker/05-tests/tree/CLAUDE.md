# worker

Python 3.12 fulfilment worker for Acme Shop. It pops `fulfil` jobs from the
`order-jobs` list and ships the order.

- Code is in `worker/`, tests in `tests/`.
- Set up with `python -m venv .venv && . .venv/bin/activate && pip install -e '.[test]'`.
- Run the tests with `pytest -q` and lint with `ruff check .` before you finish.
- One handler per job kind in `worker/jobs.py`; register it in `HANDLERS`.
- Keep `worker/consumer.py` compatible with the job format in
  `orders/internal/queue/publisher.go`.
