import assert from "node:assert/strict";
import { test } from "node:test";
import { dataHeaders, parseDataPath, upstreamUrl } from "./data.ts";

test("each file the app reads maps to its wago.tools table or foreverdb file", () => {
  const paths = [
    "/data/wago/1.60.1.70862/SkillLineAbility.csv",
    "/data/wago/1.60.1.70862/SpellEffect.csv",
    "/data/wago/1.60.1.70862/SpellReagents.csv",
    "/data/wago/1.60.1.70862/ItemSparse.csv",
    "/data/wago/1.60.1.70862/Item.csv",
    "/data/wago/1.60.1.70862/ItemModifiedAppearance.csv",
    "/data/wago/1.60.1.70862/ItemAppearance.csv",
    "/data/foreverdb/vendors.json",
    "/data/foreverdb/items.json",
    "/data/foreverdb/stats.json",
    "/data/foreverdb/itemmeta.json",
    "/data/foreverdb/ratings.json",
  ];
  assert.deepEqual(
    paths.map((p) => upstreamUrl(parseDataPath(p)!)),
    [
      "https://wago.tools/db2/SkillLineAbility/csv?build=1.60.1.70862",
      "https://wago.tools/db2/SpellEffect/csv?build=1.60.1.70862",
      "https://wago.tools/db2/SpellReagents/csv?build=1.60.1.70862",
      "https://wago.tools/db2/ItemSparse/csv?build=1.60.1.70862",
      "https://wago.tools/db2/Item/csv?build=1.60.1.70862",
      "https://wago.tools/db2/ItemModifiedAppearance/csv?build=1.60.1.70862",
      "https://wago.tools/db2/ItemAppearance/csv?build=1.60.1.70862",
      "https://foreverdb.net/data/vendors.json",
      "https://foreverdb.net/data/items.json",
      "https://foreverdb.net/data/stats.json",
      "https://foreverdb.net/data/itemmeta.json",
      "https://foreverdb.net/data/ratings.json",
    ],
  );
});

test("nothing outside the list maps to an upstream file", () => {
  const lookalikes = [
    "/data/wago/1.60.1.70861/ItemSparse.csv",
    "/data/wago/ItemSparse.csv",
    "/data/wago/1.60.1.70862/Spell.csv",
    "/data/wago/1.60.1.70862/itemsparse.csv",
    "/data/wago/1.60.1.70862/ItemSparse.CSV",
    "/data/wago/1.60.1.70862/ItemSparse",
    "/data/wago/1.60.1.70862/ItemSparse.csv/",
    "/data/wago/1.60.1.70862/../ItemSparse.csv",
    "/data/wago/1.60.1.70862/ItemSparse%2ecsv",
    "/data/foreverdb/quests.json",
    "/data/foreverdb/Vendors.json",
    "/data/foreverdb/../vendors.json",
    "/data/foreverdb/vendors.json/",
    "/data/foreverdb/sub/vendors.json",
    "/data/foreverdb/vendors.json.gz",
    "/data/casc/8216846",
    "/data/icons/inv_misc_pelt_wolf_01.jpg",
    "/x/data/foreverdb/vendors.json",
    "data/foreverdb/vendors.json",
    "/data/",
    "/data",
  ];
  assert.deepEqual(lookalikes.filter((p) => parseDataPath(p) !== null), []);
});

test("a table pinned to its build is kept a year and foreverdb's files a day", () => {
  assert.deepEqual(dataHeaders(parseDataPath("/data/wago/1.60.1.70862/Item.csv")!), {
    "Content-Type": "text/csv; charset=utf-8",
    "Cache-Control": "public, max-age=31536000, immutable",
  });
  assert.deepEqual(dataHeaders(parseDataPath("/data/foreverdb/stats.json")!), {
    "Content-Type": "application/json; charset=utf-8",
    "Cache-Control": "public, max-age=86400",
  });
});
