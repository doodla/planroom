import { formatMoney } from '@acme/kit';

export interface Product {
  sku: string;
  name: string;
  priceCents: number;
  currency: string;
}

export interface CatalogQuery {
  page?: number;
  /** Only products at or above this price, in minor units. */
  minCents?: number;
}

/** Products shown per catalog page. */
export const PAGE_SIZE = 20;

const PRODUCTS: Product[] = [
  { sku: 'mug-classic', name: 'Classic mug', priceCents: 1200, currency: 'USD' },
  { sku: 'mug-travel', name: 'Travel mug', priceCents: 2400, currency: 'USD' },
  { sku: 'tee-logo', name: 'Logo tee', priceCents: 2200, currency: 'USD' },
  { sku: 'hoodie-grey', name: 'Grey hoodie', priceCents: 5400, currency: 'USD' },
  { sku: 'cap-navy', name: 'Navy cap', priceCents: 1800, currency: 'USD' },
  { sku: 'tote-canvas', name: 'Canvas tote', priceCents: 1500, currency: 'USD' },
  { sku: 'sticker-pack', name: 'Sticker pack', priceCents: 600, currency: 'USD' },
  { sku: 'notebook-a5', name: 'A5 notebook', priceCents: 900, currency: 'USD' },
];

export function findProduct(sku: string): Product | undefined {
  return PRODUCTS.find((p) => p.sku === sku);
}

export function listProducts(q: CatalogQuery = {}) {
  const matching = PRODUCTS.filter((p) => q.minCents === undefined || p.priceCents >= q.minCents);
  const pageCount = Math.max(1, Math.ceil(matching.length / PAGE_SIZE));
  const page = Math.min(Math.max(1, q.page ?? 1), pageCount);
  const start = (page - 1) * PAGE_SIZE;
  return {
    page,
    pageCount,
    products: matching
      .slice(start, start + PAGE_SIZE)
      .map((p) => ({ ...p, price: formatMoney(p.priceCents, p.currency) })),
  };
}
