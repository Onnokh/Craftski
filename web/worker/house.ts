// Which TSM file a request names and when to download it, mirroring the desktop app's
// src/public_data.rs. Pure: index.ts does the KV and fetch work.

const BASE = "https://public-data.tradeskillmaster.com/forever";
/** TSM refreshes a realm's file about hourly and the region's a few times a day. */
const REALM_MAX_AGE = 3600;
const REGION_MAX_AGE = 6 * 3600;
/** How long a failed download holds the next one back. */
const FLOOR = 5 * 60;
/** The hold after a 429 that names no Retry-After in seconds, the length the host has sent. */
const DEFAULT_RETRY_AFTER = 3600;
/** Browsers and edges recheck a stale or nearly due file no sooner than this. */
const MIN_CACHE = 60;

const REGIONS = ["eu", "us", "kr", "tw"] as const;
const REALMS = ["normal", "pvp", "roleplay", "roleplay-pvp"] as const;
const FACTIONS = ["horde", "alliance"] as const;

export type Region = (typeof REGIONS)[number];
export type Realm = (typeof REALMS)[number];
export type Faction = (typeof FACTIONS)[number];

/** One file TSM publishes: an auction house's, or its region's. */
export type File = { kind: "realm"; region: Region; realm: Realm; faction: Faction } | { kind: "region"; region: Region };

/** No download starts before `until`, unix seconds. */
export type Hold = { until: number; why: "rate-limited" | "unreachable" };

/** Why a download failed. */
export type Failure = { kind: "rate-limited"; retryAfter: number | null } | { kind: "not-found" } | { kind: "unreachable"; detail: string };

export type Plan = "cached" | "download" | "held";

const REALM_PATH = /^\/prices\/([a-z]+)\/realm\/([a-z-]+)-([a-z]+)\.csv$/;
const REGION_PATH = /^\/prices\/([a-z]+)\/region\.csv$/;

const member = <T extends string>(list: readonly T[], s: string): s is T => (list as readonly string[]).includes(s);

/** The file a request path names, or null for anything but the paths this site serves. */
export function parsePath(pathname: string): File | null {
  const realm = REALM_PATH.exec(pathname);
  if (realm) {
    const [, region, name, faction] = realm;
    return member(REGIONS, region) && member(REALMS, name) && member(FACTIONS, faction) ? { kind: "realm", region, realm: name, faction } : null;
  }
  const region = REGION_PATH.exec(pathname)?.[1];
  return region !== undefined && member(REGIONS, region) ? { kind: "region", region } : null;
}

export function key(file: File): string {
  return file.kind === "realm" ? `${file.region}/realm/${file.realm}-${file.faction}` : `${file.region}/region`;
}

export function tsmUrl(file: File): string {
  return `${BASE}/${key(file)}/items.csv`;
}

export function maxAge(file: File): number {
  return file.kind === "realm" ? REALM_MAX_AGE : REGION_MAX_AGE;
}

/** A response that wasn't a 200 as a Failure. Retry-After counts only in seconds, as TSM sends it. */
export function failureOf(status: number, retryAfter: string | null): Failure {
  if (status === 429) {
    const seconds = retryAfter !== null && /^\s*\d+\s*$/.test(retryAfter) ? Number(retryAfter) : null;
    return { kind: "rate-limited", retryAfter: seconds };
  }
  return status === 404 ? { kind: "not-found" } : { kind: "unreachable", detail: `HTTP ${status}` };
}

/** The hold after a download failed at `now`. A 404 says only that one house has no file, so none. */
export function holdAfter(failure: Failure, now: number): Hold | null {
  switch (failure.kind) {
    case "rate-limited":
      return { until: now + (failure.retryAfter ?? DEFAULT_RETRY_AFTER), why: "rate-limited" };
    case "not-found":
      return null;
    case "unreachable":
      return { until: now + FLOOR, why: "unreachable" };
  }
}

/** Whether to download a file cached `age` seconds ago, null when it isn't cached. */
export function plan(age: number | null, maxAge: number, hold: Hold | null, now: number): Plan {
  if (age !== null && age < maxAge) return "cached";
  return hold !== null && hold.until > now ? "held" : "download";
}

/** Cache-Control for a file due in `remaining` seconds. */
export function cacheControl(remaining: number, maxAge: number): string {
  return `public, max-age=${Math.min(maxAge, Math.max(MIN_CACHE, Math.floor(remaining)))}`;
}
