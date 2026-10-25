/** Order IDs as the orders service issues them: "ord_" plus 16 hex digits. */
const ORDER_ID = /^ord_[0-9a-f]{16}$/;

/** Reports whether s looks like an order ID from the orders service. */
export function isOrderId(s: string): boolean {
  return ORDER_ID.test(s);
}

/**
 * The short, customer-facing form of an order ID:
 * formatOrderId('ord_1f2e3d4c5b6a7980') === 'ORD-1F2E-3D4C-5B6A-7980'.
 */
export function formatOrderId(id: string): string {
  if (!isOrderId(id)) {
    throw new TypeError(`not an order id: ${id}`);
  }
  const groups = id.slice('ord_'.length).toUpperCase().match(/.{4}/g) ?? [];
  return `ORD-${groups.join('-')}`;
}
