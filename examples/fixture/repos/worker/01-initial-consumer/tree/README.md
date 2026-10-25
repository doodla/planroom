# worker

The Acme Shop fulfilment worker. It pops `fulfil` jobs that the orders
service pushes onto the `order-jobs` Redis list, and ships the order.

```sh
pip install -e .
REDIS_URL=redis://localhost:6379/0 acme-worker
```
