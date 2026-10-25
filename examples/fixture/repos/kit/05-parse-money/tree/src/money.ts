/** Formats an amount in cents: formatMoney(1250, 'USD') === '$12.50'. */
export function formatMoney(amountCents: number, currency: string, locale = 'en-US'): string {
  if (!Number.isInteger(amountCents)) {
    throw new TypeError(`amount must be a whole number of cents, got ${amountCents}`);
  }
  return new Intl.NumberFormat(locale, { style: 'currency', currency }).format(amountCents / 100);
}

/** Parses '12.50' or '$12.50' into cents: parseMoney('12.50', 'USD') === 1250. */
export function parseMoney(text: string, currency: string): number {
  const cleaned = text.replace(/[^0-9.-]/g, '');
  if (!/^-?\d+(\.\d{1,2})?$/.test(cleaned)) {
    throw new SyntaxError(`not a ${currency} amount: ${text}`);
  }
  return Math.round(Number(cleaned) * 100);
}
