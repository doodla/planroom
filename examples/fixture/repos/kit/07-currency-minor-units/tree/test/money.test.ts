import { describe, expect, it } from 'vitest';
import { formatMoney, minorUnits, parseMoney } from '../src/money.js';

describe('formatMoney', () => {
  it('formats cents', () => {
    expect(formatMoney(1250, 'USD')).toBe('$12.50');
  });

  it('respects zero-decimal currencies', () => {
    expect(formatMoney(1250, 'JPY')).toBe('¥1,250');
  });

  it('rejects fractional minor units', () => {
    expect(() => formatMoney(12.5, 'USD')).toThrow(TypeError);
  });
});

describe('parseMoney', () => {
  it('round-trips with formatMoney', () => {
    expect(parseMoney(formatMoney(1999, 'USD'), 'USD')).toBe(1999);
  });

  it('pads missing decimals', () => {
    expect(parseMoney('15', 'USD')).toBe(1500);
    expect(parseMoney('1.5', 'KWD')).toBe(1500);
  });

  it('rejects too many decimals', () => {
    expect(() => parseMoney('1.234', 'USD')).toThrow(RangeError);
  });
});

describe('minorUnits', () => {
  it('knows the currencies we sell in', () => {
    expect(minorUnits('usd')).toBe(2);
    expect(() => minorUnits('XYZ')).toThrow(RangeError);
  });
});
