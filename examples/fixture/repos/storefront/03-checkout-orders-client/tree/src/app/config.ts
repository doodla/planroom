export interface Config {
  port: number;
  /** Base URL of the orders API, e.g. http://orders:8080. */
  ordersUrl: string;
}

export function loadConfig(env: NodeJS.ProcessEnv = process.env): Config {
  return {
    port: Number(env.PORT ?? 3000),
    ordersUrl: env.ORDERS_URL ?? 'http://localhost:8080',
  };
}
