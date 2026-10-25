/** Formats an amount in cents: formatMoney(1250, 'USD') === '$12.50'. */
export function formatMoney(amountCents: number, currency: string, locale = 'en-US'): string {
  if (!Number.isInteger(amountCents)) {
    throw new TypeError(`amount must be a whole number of cents, got ${amountCents}`);
  }
  return new Intl.NumberFormat(locale, { style: 'currency', currency }).format(amountCents / 100);
}
