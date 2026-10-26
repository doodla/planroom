// Modules Vite resolves at build time.

declare module "*.webp" {
  const url: string;
  export default url;
}

declare module "*.css";

declare module "*?url" {
  const url: string;
  export default url;
}
