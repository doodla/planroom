---
name: run-worker-tests
description: Run the worker's test suite and linter, and read the results. Use after changing anything under worker/ or tests/, or before handing a worker change back.
---

# Run the worker tests

## When to use

- You changed code in `worker/` or `tests/` and need to know it still works.
- You are about to hand a worker change back for review.
- A reviewer asks whether the worker tests pass.

## Steps

1. Create and activate a virtualenv if there isn't one:
   `python -m venv .venv && . .venv/bin/activate`.
2. Install the worker with its test extras: `pip install -e '.[test]'`.
3. Run the suite: `pytest -q`.
4. Run the linter: `ruff check .`.
5. If a test fails, rerun just that test with `pytest -q tests/<file>.py::<test> -x`
   and read the first failure before changing anything.

## Checks

- `pytest -q` exits 0 and reports no failures or errors.
- `ruff check .` prints no findings.
- No test was skipped or deleted to make the suite pass.
