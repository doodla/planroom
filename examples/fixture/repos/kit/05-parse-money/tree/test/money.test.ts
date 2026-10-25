import { describe, expect, it } from 'vitest';
import { formatMoney, parseMoney } from '../src/money.js';

describe('formatMoney', () => {
  it('formats cents', () => {
    expect(formatMoney(1250, 'USD')).toBe('$12.50');
  });

  it('rejects fractional cents', () => {
    expect(() => formatMoney(12.5, 'USD')).toThrow(TypeError);
  });
});

describe('parseMoney', () => {
  it('round-trips with formatMoney', () => {
    expect(parseMoney(formatMoney(1999, 'USD'), 'USD')).toBe(1999);
  });

  it('rejects garbage', () => {
    expect(() => parseMoney('twelve', 'USD')).toThrow(SyntaxError);
  });
});
