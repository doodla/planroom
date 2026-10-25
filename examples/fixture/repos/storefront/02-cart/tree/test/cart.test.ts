import { describe, expect, it } from 'vitest';
import { Cart } from '../src/cart/cart.js';

describe('Cart', () => {
  it('adds up line totals', () => {
    const cart = new Cart();
    cart.add({ sku: 'mug-classic', name: 'Classic mug', priceCents: 1200 }, 2);
    cart.add({ sku: 'cap-navy', name: 'Navy cap', priceCents: 1800 });
    expect(cart.totalCents()).toBe(4200);
  });

  it('merges repeated adds of the same sku', () => {
    const cart = new Cart();
    cart.add({ sku: 'mug-classic', name: 'Classic mug', priceCents: 1200 });
    cart.add({ sku: 'mug-classic', name: 'Classic mug', priceCents: 1200 });
    expect(cart.lines()).toHaveLength(1);
    expect(cart.lines()[0].quantity).toBe(2);
  });

  it('rejects non-positive quantities', () => {
    expect(() => new Cart().add({ sku: 'x', name: 'X', priceCents: 1 }, 0)).toThrow(RangeError);
  });
});
