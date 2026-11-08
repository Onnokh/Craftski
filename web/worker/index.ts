// Serves TSM's public price files at /prices/* from KV, so visitors never reach TSM and TSM sees
// at most one download per file each time it is due. Every other path is a static file in site/.
//
// KV is written only after a download, never per request: 32 realm files hourly and 4 region
// files six-hourly come to at most 784 writes a day, under the free tier's 1000. Two
// simultaneous misses may both download; that costs one extra request, not a burst.

import { cacheControl, failureOf, type Failure, type File, type Hold, holdAfter, key, maxAge, parsePath, plan, tsmUrl } from "./house.ts";

interface Env {
  ASSETS: Fetcher;
  PRICES: KVNamespace;
}

/** missing: TSM answered 404, kept so a house it doesn't publish costs one download per max age. */
type Meta = { fetchedAt: number; missing?: true };
type Copy = { body: string; meta: Meta };

const HOLD = "hold";
const NO_FILE = "TSM publishes no data for that house.\n";
const STALE_CACHE = "public, max-age=60";

export default {
  async fetch(request, env): Promise<Response> {
    const url = new URL(request.url);
    if (url.pathname !== "/prices" && !url.pathname.startsWith("/prices/")) return env.ASSETS.fetch(request);
    if (request.method !== "GET" && request.method !== "HEAD") return text(request, 405, "Only GET and HEAD.\n", { Allow: "GET, HEAD" });
    const file = parsePath(url.pathname);
    if (file === null) return text(request, 404, "No such price file.\n");
    return prices(request, env, file);
  },
} satisfies ExportedHandler<Env>;

async function prices(request: Request, env: Env, file: File): Promise<Response> {
  const now = Math.floor(Date.now() / 1000);
  const max = maxAge(file);
  const [stored, hold] = await Promise.all([env.PRICES.getWithMetadata<Meta>(key(file), "text"), env.PRICES.get<Hold>(HOLD, "json")]);
  const copy: Copy | null = stored.value !== null && stored.metadata !== null ? { body: stored.value, meta: stored.metadata } : null;
  const age = copy === null ? null : now - copy.meta.fetchedAt;
  const step = plan(age, max, hold, now);

  if (step === "cached" && copy !== null && age !== null) return serve(request, copy, cacheControl(max - age, max));
  if (step === "held" && hold !== null) return stale(request, copy, hold) ?? unavailable(request, 503, hold, now);

  const result = await download(tsmUrl(file));
  const failed = result.ok ? null : holdAfter(result.failure, now);
  if (failed === null) {
    const fresh: Copy = result.ok ? { body: result.body, meta: { fetchedAt: now } } : { body: NO_FILE, meta: { fetchedAt: now, missing: true } };
    await env.PRICES.put(key(file), fresh.body, { metadata: fresh.meta });
    return serve(request, fresh, cacheControl(max, max));
  }
  await env.PRICES.put(HOLD, JSON.stringify(failed), { expirationTtl: Math.max(60, failed.until - now) });
  return stale(request, copy, failed) ?? unavailable(request, failed.why === "rate-limited" ? 503 : 502, failed, now);
}

async function download(url: string): Promise<{ ok: true; body: string } | { ok: false; failure: Failure }> {
  try {
    const response = await fetch(url, { signal: AbortSignal.timeout(60_000) });
    console.log(`TSM download ${url}: HTTP ${response.status}`);
    if (response.status === 200) return { ok: true, body: await response.text() };
    return { ok: false, failure: failureOf(response.status, response.headers.get("Retry-After")) };
  } catch (e) {
    console.log(`TSM download ${url}: ${e}`);
    return { ok: false, failure: { kind: "unreachable", detail: String(e) } };
  }
}

function serve(request: Request, copy: Copy, cache: string, staleWhy?: Hold["why"]): Response {
  const headers = new Headers({
    "Content-Type": copy.meta.missing ? "text/plain; charset=utf-8" : "text/csv; charset=utf-8",
    "Cache-Control": cache,
    "X-Craftski-Fetched-At": new Date(copy.meta.fetchedAt * 1000).toISOString(),
  });
  if (staleWhy !== undefined) headers.set("X-Craftski-Stale", staleWhy);
  return new Response(request.method === "HEAD" ? null : copy.body, { status: copy.meta.missing ? 404 : 200, headers });
}

/** The due copy while downloads are held, or null when there is no CSV to fall back to. */
function stale(request: Request, copy: Copy | null, hold: Hold): Response | null {
  if (copy === null || copy.meta.missing) return null;
  return serve(request, copy, STALE_CACHE, hold.why);
}

function unavailable(request: Request, status: 502 | 503, hold: Hold, now: number): Response {
  const words = hold.why === "rate-limited" ? "TSM is rate-limiting downloads" : "TSM could not be reached";
  const extra: Record<string, string> = status === 503 ? { "Retry-After": String(Math.max(1, hold.until - now)) } : {};
  return text(request, status, `${words}; no copy of this file yet.\n`, extra);
}

function text(request: Request, status: number, body: string, extra: Record<string, string> = {}): Response {
  return new Response(request.method === "HEAD" ? null : body, { status, headers: { "Content-Type": "text/plain; charset=utf-8", ...extra } });
}
