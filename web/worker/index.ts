// Serves TSM's public price files at /prices/* from KV, so visitors never reach TSM and TSM sees
// at most one download per file each time it is due. Every other path is a static file in site/.
//
// KV is written only after a download or a 304, never per request, so writes stay within what
// house.ts's MIN_MAX_AGE allows. Two simultaneous misses may both download; that costs one extra
// request, not a burst.

import {
  cacheControl,
  defaultMaxAge,
  failureOf,
  type Failure,
  type File,
  type Hold,
  holdAfter,
  key,
  type Meta,
  metaFrom,
  parsePath,
  plan,
  tsmUrl,
  validators,
} from "./house.ts";

interface Env {
  ASSETS: Fetcher;
  PRICES: KVNamespace;
}

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
  const [stored, hold] = await Promise.all([env.PRICES.getWithMetadata<Meta>(key(file), "text"), env.PRICES.get<Hold>(HOLD, "json")]);
  const copy: Copy | null = stored.value !== null && stored.metadata !== null ? { body: stored.value, meta: stored.metadata } : null;
  const age = copy === null ? null : now - copy.meta.fetchedAt;
  const step = plan(age, copy?.meta.maxAge ?? defaultMaxAge(file), hold, now);

  if (step === "cached" && copy !== null && age !== null) return serve(request, copy, cacheControl(copy.meta.maxAge - age, copy.meta.maxAge));
  if (step === "held" && hold !== null) return stale(request, copy, hold) ?? unavailable(request, 503, hold, now);

  const result = await download(file, copy, now);
  const failed = result.ok ? null : holdAfter(result.failure, now);
  if (failed === null) {
    const fresh: Copy = result.ok ? result.copy : { body: NO_FILE, meta: { fetchedAt: now, maxAge: defaultMaxAge(file), etag: null, lastModified: null, missing: true } };
    await env.PRICES.put(key(file), fresh.body, { metadata: fresh.meta });
    return serve(request, fresh, cacheControl(fresh.meta.maxAge, fresh.meta.maxAge));
  }
  await env.PRICES.put(HOLD, JSON.stringify(failed), { expirationTtl: Math.max(60, failed.until - now) });
  return stale(request, copy, failed) ?? unavailable(request, failed.why === "rate-limited" ? 503 : 502, failed, now);
}

/** `file` from TSM, or `copy` again when TSM answers 304 that it is unchanged. */
async function download(file: File, copy: Copy | null, now: number): Promise<{ ok: true; copy: Copy } | { ok: false; failure: Failure }> {
  const url = tsmUrl(file);
  try {
    const response = await fetch(url, { headers: copy === null ? {} : validators(copy.meta), signal: AbortSignal.timeout(60_000) });
    console.log(`TSM download ${url}: HTTP ${response.status}`);
    const meta = metaFrom(response.headers, copy?.meta ?? null, defaultMaxAge(file), now);
    if (response.status === 200) return { ok: true, copy: { body: await response.text(), meta } };
    if (response.status === 304 && copy !== null && !copy.meta.missing) return { ok: true, copy: { body: copy.body, meta } };
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
