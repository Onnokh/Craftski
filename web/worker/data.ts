// Which game data file a /data/* request names: the wago.tools tables and foreverdb files the
// browser build reads, and nothing else. Neither site allows cross-origin reads, so this one
// serves them. Pure: index.ts does the fetch and cache work. Never art or icons: the page loads
// those from wago.tools and Wowhead itself.

/** The Forever build the app's recipes are joined from, src/recipes.rs's BUILD. A table at this
 * build never changes, so a copy is good forever and a new build is a new path. */
export const BUILD = "1.60.1.70862";
const TABLES = ["SkillLineAbility", "SpellEffect", "SpellReagents", "ItemSparse", "Item", "ItemModifiedAppearance", "ItemAppearance"] as const;
const FOREVERDB = ["vendors.json", "items.json", "stats.json", "itemmeta.json", "ratings.json"] as const;

/** foreverdb updates its files now and then; the desktop app rereads them weekly. */
const FOREVERDB_MAX_AGE = 86_400;
const YEAR = 365 * 86_400;

export type Table = (typeof TABLES)[number];
export type ForeverdbFile = (typeof FOREVERDB)[number];
export type DataFile = { source: "wago"; table: Table } | { source: "foreverdb"; file: ForeverdbFile };

const WAGO_PATH = /^\/data\/wago\/([0-9.]+)\/([A-Za-z]+)\.csv$/;
const FOREVERDB_PATH = /^\/data\/foreverdb\/([a-z]+\.json)$/;

const member = <T extends string>(list: readonly T[], s: string): s is T => (list as readonly string[]).includes(s);

/** The file a request path names, or null for anything but the files the app reads. */
export function parseDataPath(pathname: string): DataFile | null {
  const wago = WAGO_PATH.exec(pathname);
  if (wago) {
    const [, build, table] = wago;
    return build === BUILD && member(TABLES, table) ? { source: "wago", table } : null;
  }
  const file = FOREVERDB_PATH.exec(pathname)?.[1];
  return file !== undefined && member(FOREVERDB, file) ? { source: "foreverdb", file } : null;
}

export function upstreamUrl(file: DataFile): string {
  return file.source === "wago" ? `https://wago.tools/db2/${file.table}/csv?build=${BUILD}` : `https://foreverdb.net/data/${file.file}`;
}

/** The headers a copy is served and cached with; the Cache API keeps it as long as Cache-Control
 * says. */
export function dataHeaders(file: DataFile): Record<string, string> {
  return {
    "Content-Type": file.source === "wago" ? "text/csv; charset=utf-8" : "application/json; charset=utf-8",
    "Cache-Control": file.source === "wago" ? `public, max-age=${YEAR}, immutable` : `public, max-age=${FOREVERDB_MAX_AGE}`,
  };
}
