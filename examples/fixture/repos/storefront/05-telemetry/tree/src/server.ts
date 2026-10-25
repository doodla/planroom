// Telemetry must load first so auto-instrumentation can patch http and express.
import './app/telemetry.js';

import express from 'express';
import { OrdersClient } from './api/orders.js';
import { loadConfig } from './app/config.js';
import { cartRouter } from './cart/routes.js';
import { catalogRouter } from './catalog/routes.js';
import { checkoutRouter } from './checkout/routes.js';

const config = loadConfig();
const orders = new OrdersClient(config.ordersUrl);

const app = express();
app.use(express.json());

app.get('/healthz', (_req, res) => {
  res.json({ status: 'ok' });
});
app.use('/catalog', catalogRouter());
app.use('/cart', cartRouter());
app.use('/checkout', checkoutRouter({ orders }));

app.listen(config.port, () => {
  console.log(`storefront listening on :${config.port}, orders at ${config.ordersUrl}`);
});
