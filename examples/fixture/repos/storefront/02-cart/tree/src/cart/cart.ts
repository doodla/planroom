import type { Product } from '../catalog/catalog.js';

export interface CartLine {
  sku: string;
  name: string;
  quantity: number;
  priceCents: number;
}

/** A shopping cart in one currency. */
export class Cart {
  private readonly items = new Map<string, CartLine>();

  constructor(readonly currency = 'USD') {}

  add(product: Pick<Product, 'sku' | 'name' | 'priceCents'>, quantity = 1): void {
    if (!Number.isInteger(quantity) || quantity < 1) {
      throw new RangeError(`quantity must be a positive integer, got ${quantity}`);
    }
    const line = this.items.get(product.sku);
    if (line) {
      line.quantity += quantity;
    } else {
      this.items.set(product.sku, { ...product, quantity });
    }
  }

  remove(sku: string): void {
    this.items.delete(sku);
  }

  lines(): CartLine[] {
    return [...this.items.values()];
  }

  isEmpty(): boolean {
    return this.items.size === 0;
  }

  totalCents(): number {
    return this.lines().reduce((sum, l) => sum + l.priceCents * l.quantity, 0);
  }

  clear(): void {
    this.items.clear();
  }

  private format(cents: number): string {
    return new Intl.NumberFormat('en-US', { style: 'currency', currency: this.currency }).format(cents / 100);
  }

  summary() {
    return {
      lines: this.lines().map((l) => ({ ...l, price: this.format(l.priceCents) })),
      total: this.format(this.totalCents()),
    };
  }
}
