import { Router, type Request } from 'express';
import { findProduct } from '../catalog/catalog.js';
import { Cart } from './cart.js';

const carts = new Map<string, Cart>();

/** The cart for this request's session (the x-session-id header). */
export function cartFor(req: Request): Cart {
  const id = req.header('x-session-id') ?? 'anonymous';
  let cart = carts.get(id);
  if (!cart) {
    cart = new Cart();
    carts.set(id, cart);
  }
  return cart;
}

export function cartRouter(): Router {
  const router = Router();

  router.get('/', (req, res) => {
    res.json(cartFor(req).summary());
  });

  router.post('/items', (req, res) => {
    const product = findProduct(String(req.body.sku));
    if (!product) {
      res.status(404).json({ error: 'unknown sku' });
      return;
    }
    const cart = cartFor(req);
    cart.add(product, Number(req.body.quantity ?? 1));
    res.status(201).json(cart.summary());
  });

  router.delete('/items/:sku', (req, res) => {
    const cart = cartFor(req);
    cart.remove(req.params.sku);
    res.json(cart.summary());
  });

  return router;
}
