// The web service: the cache and the database come from compose.
export const cacheUrl = process.env.CACHE_URL;
// A call only this comment names makes nothing: fetch('http://commented:1/x');
export async function health(): Promise<Response> {
  return fetch(`${cacheUrl}/healthz`);
}
