import { describe, expect, it } from 'vitest';
import { formatOrderId, isOrderId } from '../src/orderId.js';

describe('order IDs', () => {
  it('accepts IDs from the orders service', () => {
    expect(isOrderId('ord_1f2e3d4c5b6a7980')).toBe(true);
  });

  it('rejects anything else', () => {
    expect(isOrderId('ord_123')).toBe(false);
    expect(isOrderId('ORD-1F2E-3D4C-5B6A-7980')).toBe(false);
  });

  it('formats for customers', () => {
    expect(formatOrderId('ord_1f2e3d4c5b6a7980')).toBe('ORD-1F2E-3D4C-5B6A-7980');
  });
});
