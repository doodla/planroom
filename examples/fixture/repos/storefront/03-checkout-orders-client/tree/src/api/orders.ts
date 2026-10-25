import type { components } from './orders.schema.js';

export type Order = components['schemas']['Order'];
export type CreateOrderRequest = components['schemas']['CreateOrderRequest'];

export class OrdersApiError extends Error {
  constructor(
    readonly status: number,
    message: string,
  ) {
    super(message);
    this.name = 'OrdersApiError';
  }
}

/**
 * Client for the orders API described in orders/api/openapi.yaml. The types
 * come from orders.schema.ts; run `npm run gen:orders-client` when the spec
 * changes.
 */
export class OrdersClient {
  constructor(private readonly baseUrl: string) {}

  /** POST /orders */
  async createOrder(body: CreateOrderRequest): Promise<Order> {
    const res = await fetch(`${this.baseUrl}/orders`, {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify(body),
    });
    return this.parse<Order>(res);
  }

  /** GET /orders/{id} */
  async getOrder(id: string): Promise<Order> {
    const res = await fetch(`${this.baseUrl}/orders/${encodeURIComponent(id)}`);
    return this.parse<Order>(res);
  }

  private async parse<T>(res: Response): Promise<T> {
    if (!res.ok) {
      throw new OrdersApiError(res.status, await res.text());
    }
    return (await res.json()) as T;
  }
}
