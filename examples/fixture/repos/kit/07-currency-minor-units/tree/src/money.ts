/** Digits after the decimal point for each currency we sell in (ISO 4217). */
const MINOR_UNITS: Record<string, number> = {
  USD: 2,
  EUR: 2,
  GBP: 2,
  JPY: 0,
  KWD: 3,
};

/** How many minor units make one major unit's decimal places, e.g. 2 for USD. */
export function minorUnits(currency: string): number {
  const digits = MINOR_UNITS[currency.toUpperCase()];
  if (digits === undefined) {
    throw new RangeError(`unsupported currency: ${currency}`);
  }
  return digits;
}

/**
 * Formats an amount given in minor units.
 * formatMoney(1250, 'USD') === '$12.50'; formatMoney(1250, 'JPY') === '¥1,250'.
 */
export function formatMoney(amountMinor: number, currency: string, locale = 'en-US'): string {
  if (!Number.isInteger(amountMinor)) {
    throw new TypeError(`amount must be a whole number of minor units, got ${amountMinor}`);
  }
  const digits = minorUnits(currency);
  return new Intl.NumberFormat(locale, {
    style: 'currency',
    currency,
    minimumFractionDigits: digits,
    maximumFractionDigits: digits,
  }).format(amountMinor / 10 ** digits);
}

/** Parses '12.50' or '$12.50' into minor units: parseMoney('12.50', 'USD') === 1250. */
export function parseMoney(text: string, currency: string): number {
  const cleaned = text.replace(/[^0-9.-]/g, '');
  if (!/^-?\d+(\.\d+)?$/.test(cleaned)) {
    throw new SyntaxError(`not an amount: ${text}`);
  }
  const digits = minorUnits(currency);
  const [whole, frac = ''] = cleaned.split('.');
  if (frac.length > digits) {
    throw new RangeError(`${text} has more than ${digits} decimal places for ${currency}`);
  }
  const sign = whole.startsWith('-') ? -1 : 1;
  const major = Math.abs(Number(whole));
  return sign * (major * 10 ** digits + Number(frac.padEnd(digits, '0') || '0'));
}
