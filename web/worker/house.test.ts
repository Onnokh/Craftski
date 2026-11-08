import assert from "node:assert/strict";
import { test } from "node:test";
import { cacheControl, failureOf, type Hold, holdAfter, key, maxAge, parsePath, plan, tsmUrl } from "./house.ts";

test("every house's file and every region's file parse, with keys TSM's paths share", () => {
  const seen = new Map<string, string>();
  for (const region of ["eu", "us", "kr", "tw"]) {
    for (const realm of ["normal", "pvp", "roleplay", "roleplay-pvp"]) {
      for (const faction of ["horde", "alliance"]) {
        const file = parsePath(`/prices/${region}/realm/${realm}-${faction}.csv`);
        assert.deepEqual(file, { kind: "realm", region, realm, faction });
        seen.set(key(file!), tsmUrl(file!));
      }
    }
    const file = parsePath(`/prices/${region}/region.csv`);
    assert.deepEqual(file, { kind: "region", region });
    seen.set(key(file!), tsmUrl(file!));
  }
  assert.equal(seen.size, 36);
  assert.equal(seen.get("tw/realm/roleplay-pvp-alliance"), "https://public-data.tradeskillmaster.com/forever/tw/realm/roleplay-pvp-alliance/items.csv");
  assert.equal(seen.get("kr/region"), "https://public-data.tradeskillmaster.com/forever/kr/region/items.csv");
});

test("a roleplay-pvp house splits at its last hyphen", () => {
  assert.deepEqual(parsePath("/prices/us/realm/roleplay-pvp-horde.csv"), { kind: "realm", region: "us", realm: "roleplay-pvp", faction: "horde" });
});

test("lookalike paths name no file", () => {
  const lookalikes = [
    "/prices/xx/region.csv",
    "/prices/xx/realm/pvp-horde.csv",
    "/prices/eu/realm/pvp.csv",
    "/prices/eu/realm/pvp-.csv",
    "/prices/eu/realm/pvp-neutral.csv",
    "/prices/eu/realm/arena-horde.csv",
    "/prices/eu/realm/roleplay-pvp.csv",
    "/prices/eu/realm/pvp-horde-horde.csv",
    "/prices/EU/region.csv",
    "/prices/eu/realm/PvP-Horde.csv",
    "/prices/eu/region.CSV",
    "/prices/eu/region",
    "/prices/eu/region.csv/",
    "/prices/eu/region/items.csv",
    "/prices/eu/realm/pvp-horde/items.csv",
    "/prices/eu/realm/pvp-horde.csv/extra",
    "/prices/eu/extra/realm/pvp-horde.csv",
    "/prices/../eu/region.csv",
    "/prices/eu/realm/../region.csv",
    "/prices/eu/realm/..%2fpvp-horde.csv",
    "/x/prices/eu/region.csv",
    "prices/eu/region.csv",
    "/prices/",
    "/prices",
    "/",
  ];
  assert.deepEqual(lookalikes.filter((p) => parsePath(p) !== null), []);
});

test("a realm file is due hourly and a region file every six hours", () => {
  assert.deepEqual([maxAge(parsePath("/prices/eu/realm/pvp-horde.csv")!), maxAge(parsePath("/prices/eu/region.csv")!)], [3600, 21600]);
});

test("a file downloads when missing or due and never while held", () => {
  const now = 1_000_000;
  const held: Hold = { until: now + 600, why: "rate-limited" };
  const ended: Hold = { until: now, why: "rate-limited" };
  assert.equal(plan(null, 3600, null, now), "download");
  assert.equal(plan(3599, 3600, null, now), "cached");
  assert.equal(plan(3600, 3600, null, now), "download");
  assert.equal(plan(3600, 3600, held, now), "held");
  assert.equal(plan(3600, 3600, ended, now), "download");
  assert.equal(plan(null, 3600, held, now), "held");
  assert.equal(plan(10, 3600, held, now), "cached");
});

test("a 429 holds for its Retry-After, a 404 not at all, and another failure for five minutes", () => {
  const now = 1_000_000;
  assert.deepEqual(holdAfter({ kind: "rate-limited", retryAfter: 3540 }, now), { until: now + 3540, why: "rate-limited" });
  assert.deepEqual(holdAfter({ kind: "rate-limited", retryAfter: null }, now), { until: now + 3600, why: "rate-limited" });
  assert.deepEqual(holdAfter({ kind: "unreachable", detail: "offline" }, now), { until: now + 300, why: "unreachable" });
  assert.equal(holdAfter({ kind: "not-found" }, now), null);
});

test("a response's status and Retry-After read as a failure", () => {
  assert.deepEqual(failureOf(429, "3540"), { kind: "rate-limited", retryAfter: 3540 });
  assert.deepEqual(failureOf(429, "Sun, 08 Nov 2026 22:40:00 GMT"), { kind: "rate-limited", retryAfter: null });
  assert.deepEqual(failureOf(429, null), { kind: "rate-limited", retryAfter: null });
  assert.deepEqual(failureOf(404, null), { kind: "not-found" });
  assert.deepEqual(failureOf(500, null), { kind: "unreachable", detail: "HTTP 500" });
});

test("Cache-Control lasts until the file is due, at least a minute and at most its max age", () => {
  assert.equal(cacheControl(1234.6, 3600), "public, max-age=1234");
  assert.equal(cacheControl(5, 3600), "public, max-age=60");
  assert.equal(cacheControl(-100, 3600), "public, max-age=60");
  assert.equal(cacheControl(90_000, 3600), "public, max-age=3600");
});
