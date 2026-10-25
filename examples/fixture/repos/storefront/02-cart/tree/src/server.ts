import express from 'express';
import { loadConfig } from './app/config.js';
import { cartRouter } from './cart/routes.js';
import { catalogRouter } from './catalog/routes.js';

const config = loadConfig();

const app = express();
app.use(express.json());

app.get('/healthz', (_req, res) => {
  res.json({ status: 'ok' });
});
app.use('/catalog', catalogRouter());
app.use('/cart', cartRouter());

app.listen(config.port, () => {
  console.log(`storefront listening on :${config.port}`);
});
