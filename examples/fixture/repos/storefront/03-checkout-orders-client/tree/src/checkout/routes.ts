import { formatMoney, formatOrderId, isOrderId } from '@acme/kit';
import { Router } from 'express';
import type { OrdersClient } from '../api/orders.js';
import { cartFor } from '../cart/routes.js';
import { placeOrder } from './checkout.js';

export function checkoutRouter(deps: { orders: OrdersClient }): Router {
  const router = Router();

  router.post('/', async (req, res, next) => {
    try {
      const cart = cartFor(req);
      const customerId = String(req.body.customerId ?? 'guest');
      // The customer pays on the next page; the order starts as created.
      const order = await placeOrder(deps.orders, { customerId, cart });
      res.status(201).json({
        orderId: order.id,
        reference: formatOrderId(order.id!),
        total: formatMoney(order.total_cents ?? 0, order.currency ?? cart.currency),
        state: order.state,
        next: 'payment',
      });
    } catch (err) {
      next(err);
    }
  });

  router.get('/orders/:id', async (req, res, next) => {
    if (!isOrderId(req.params.id)) {
      res.status(404).json({ error: 'unknown order' });
      return;
    }
    try {
      const order = await deps.orders.getOrder(req.params.id);
      res.json({ reference: formatOrderId(req.params.id), state: order.state });
    } catch (err) {
      next(err);
    }
  });

  return router;
}
