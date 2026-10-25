import { parseMoney } from '@acme/kit';
import { Router } from 'express';
import { listProducts } from './catalog.js';

export function catalogRouter(): Router {
  const router = Router();

  // GET /catalog?page=2&min=15.00
  router.get('/', (req, res) => {
    const page = Number(req.query.page ?? 1);
    const minCents = typeof req.query.min === 'string' ? parseMoney(req.query.min, 'USD') : undefined;
    res.json(listProducts({ page, minCents }));
  });

  return router;
}
