import { isOrderId } from '@acme/kit';
import type { Order, OrdersClient } from '../api/orders.js';
import type { Cart } from '../cart/cart.js';

export interface PlaceOrderInput {
  customerId: string;
  cart: Cart;
  /** Set when the card was charged before the order was created. */
  paymentRef?: string;
}

/** Turns a cart into an order through the orders API, then empties the cart. */
export async function placeOrder(orders: OrdersClient, input: PlaceOrderInput): Promise<Order> {
  const { cart } = input;
  if (cart.isEmpty()) {
    throw new Error('cart is empty');
  }
  const order = await orders.createOrder({
    customer_id: input.customerId,
    currency: cart.currency,
    payment_ref: input.paymentRef,
    items: cart.lines().map((l) => ({ sku: l.sku, quantity: l.quantity, price_cents: l.priceCents })),
  });
  if (!order.id || !isOrderId(order.id)) {
    throw new Error(`orders returned a malformed order id: ${String(order.id)}`);
  }
  cart.clear();
  return order;
}

/** Stand-in for the payment provider: charges the amount and returns a reference. */
export async function charge(amountCents: number, currency: string): Promise<string> {
  if (amountCents <= 0) {
    throw new Error(`nothing to charge in ${currency}`);
  }
  return `pay_${Date.now().toString(36)}`;
}
