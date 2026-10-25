import { describe, expect, it } from 'vitest';
import { formatMoney } from '../src/money.js';

describe('formatMoney', () => {
  it('formats cents', () => {
    expect(formatMoney(1250, 'USD')).toBe('$12.50');
  });

  it('rejects fractional cents', () => {
    expect(() => formatMoney(12.5, 'USD')).toThrow(TypeError);
  });
});
