import { Router } from 'express';
import { listProducts } from './catalog.js';

export function catalogRouter(): Router {
  const router = Router();

  // GET /catalog?page=2
  router.get('/', (req, res) => {
    const page = Number(req.query.page ?? 1);
    res.json(listProducts({ page }));
  });

  return router;
}
