// Drives the built site (run build.mjs first) in headless Chromium against a local server that
// stands in for the Worker. It switches professions, opens the flips tab, hovers a recipe for its
// tooltip, changes a setting and reloads to see it kept, then fails on any console error.
//
// Prices are the local TSM fixtures only; nothing here asks TSM. foreverdb files come from the
// desktop app's cache. wago.tools tables and atlases are downloaded once into FIXTURES and
// reused. Icons come from the desktop cache under their Wowhead names.
//
//   node smoke.mjs [screenshot dir]
import { existsSync, mkdirSync, readFileSync, statSync, writeFileSync } from "node:fs";
import { createServer } from "node:http";
import { tmpdir } from "node:os";
import { dirname, extname, join, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium } from "playwright";
import { dataHeaders, parseDataPath, upstreamUrl } from "./worker/data.ts";

const here = dirname(fileURLToPath(import.meta.url));
const site = join(here, "site");
const shots = resolve(process.argv[2] ?? join(tmpdir(), "craftski-shots"));
const fixtures = process.env.FIXTURES ?? join(tmpdir(), "craftski-fixtures");
const desktop = join(process.env.LOCALAPPDATA ?? "", "Craftski");
const tsm = process.env.TSM_SAMPLES ?? join(tmpdir(), "tsmpub-926758634");
mkdirSync(shots, { recursive: true });
mkdirSync(fixtures, { recursive: true });

const fetchedUpstream = [];
let priceReads = 0;
/** `name` in FIXTURES, downloaded from `url` the first time. */
async function fixture(name, url) {
  const path = join(fixtures, name);
  if (!existsSync(path)) {
    const response = await fetch(url);
    if (!response.ok) throw new Error(`${url}: HTTP ${response.status}`);
    writeFileSync(path, Buffer.from(await response.arrayBuffer()));
    fetchedUpstream.push(url);
  }
  return readFileSync(path);
}

const housePath = /^\/prices\/(eu|us)\/(?:realm\/([a-z-]+)|region)\.csv$/;
const types = { ".html": "text/html", ".js": "text/javascript", ".wasm": "application/wasm", ".json": "application/json" };
const server = createServer(async (request, response) => {
  const { pathname } = new URL(request.url, "http://localhost");
  try {
    const house = housePath.exec(pathname);
    if (house) {
      priceReads += 1;
      const file = join(tsm, `forever_${house[1]}_${house[2] ? `realm_${house[2]}` : "region"}_items.csv`);
      if (!existsSync(file)) return response.writeHead(404, { "Content-Type": "text/plain" }).end("TSM publishes no data for that house.\n");
      return response.writeHead(200, { "Content-Type": "text/csv", "X-Craftski-Fetched-At": statSync(file).mtime.toISOString() }).end(readFileSync(file));
    }
    if (pathname.startsWith("/data/")) {
      const file = parseDataPath(pathname);
      if (file === null) return response.writeHead(404).end("No such data file.\n");
      const body = file.source === "foreverdb" ? readFileSync(join(desktop, `foreverdb-${file.file}`)) : await fixture(`wago-${file.table}.csv`, upstreamUrl(file));
      return response.writeHead(200, dataHeaders(file)).end(body);
    }
    const path = resolve(site, "." + decodeURIComponent(pathname).replace(/\/$/, "/index.html"));
    if (!path.startsWith(site + sep) || !existsSync(path)) return response.writeHead(404).end();
    response.writeHead(200, { "Content-Type": types[extname(path)] ?? "application/octet-stream" }).end(readFileSync(path));
  } catch (e) {
    response.writeHead(500).end(String(e));
  }
});
await new Promise((done) => server.listen(0, "127.0.0.1", done));
const url = `http://127.0.0.1:${server.address().port}/`;

// Icon names back to the desktop cache's numbered files; named icons are cached by name.
const byName = new Map(Object.entries(JSON.parse(readFileSync(join(desktop, "icons", "names.json"), "utf8"))).map(([id, name]) => [name, id]));
const iconFile = (name) => [join(desktop, "icons", `${byName.get(name)}.jpg`), join(desktop, "icons", `${name}.jpg`)].find(existsSync);

const installed = chromium.executablePath();
const executablePath = process.env.CHROMIUM_PATH ?? (existsSync(installed) ? installed : join(process.env.LOCALAPPDATA ?? "", "ms-playwright", "chromium-1248", "chrome-win64", "chrome.exe"));
const browser = await chromium.launch({ headless: true, executablePath, args: ["--enable-unsafe-webgpu"] });
const context = await browser.newContext({ viewport: { width: 1640, height: 1000 }, deviceScaleFactor: 1, colorScheme: "dark" });
const outside = [];
await context.route(/^https?:\/\/(?!127\.0\.0\.1)/, async (route) => {
  const target = new URL(route.request().url());
  const cors = { "Access-Control-Allow-Origin": "*" };
  const icon = target.host === "wow.zamimg.com" && /\/icons\/large\/([^/]+)\.jpg$/.exec(target.pathname)?.[1];
  const atlas = target.host === "wago.tools" && /^\/api\/casc\/(\d+)$/.exec(target.pathname)?.[1];
  if (icon) {
    const file = iconFile(icon);
    return file ? route.fulfill({ status: 200, headers: { ...cors, "Content-Type": "image/jpeg" }, body: readFileSync(file) }) : route.fulfill({ status: 404, headers: cors });
  }
  if (atlas) return route.fulfill({ status: 200, headers: { ...cors, "Content-Type": "application/octet-stream" }, body: await fixture(`casc-${atlas}.blp`, target.href) });
  outside.push(target.href);
  return route.abort();
});

const page = await context.newPage();
const errors = [];
page.on("pageerror", (e) => errors.push(`pageerror: ${e.message}`));
page.on("console", (m) => m.type() === "error" && errors.push(`console: ${m.text()}`));
page.on("response", (r) => r.status() >= 400 && errors.push(`${r.status()} ${r.url()}`));
await context.addInitScript(() => document.addEventListener("runepresent", () => (window.__firstFrame = performance.now()), { once: true }));

const steps = [];
const text = () => page.evaluate(() => document.body.innerText);
async function until(what, test, timeout = 60_000) {
  const started = Date.now();
  for (;;) {
    const words = await text();
    if (test(words)) return words;
    if (Date.now() - started > timeout) throw new Error(`timed out waiting for ${what}; page reads:\n${words.slice(0, 2000)}`);
    await page.waitForTimeout(200);
  }
}
/** Clicks the mirror element's place on the canvas, as a pointer would. */
async function press(locator) {
  const box = await locator.first().boundingBox();
  if (!box) throw new Error(`nothing to press for ${locator}`);
  await page.mouse.click(box.x + box.width / 2, box.y + box.height / 2);
}
async function shot(name) {
  await page.waitForTimeout(600);
  const path = join(shots, `web-${name}.png`);
  await page.screenshot({ path });
  return path;
}
const shown = (profession) => (words) => new RegExp(`${profession}\\n[\\s\\S]*?([1-9]\\d*) of \\d+ recipes shown`).test(words);

try {
  const started = Date.now();
  await page.goto(url);
  await page.waitForFunction(() => window.__firstFrame, null, { timeout: 60_000 });
  const firstFrameMs = Math.round(await page.evaluate(() => window.__firstFrame));
  await until("Leatherworking priced", shown("Leatherworking"));
  steps.push({ step: "first frame", ms: firstFrameMs }, { step: "Leatherworking priced", ms: Date.now() - started, shot: await shot("leatherworking") });

  const links = {};
  for (const [name, href] of [["Buy me a coffee", "https://buymeacoffee.com/onnokh1i"], ["TSM public data", "https://tradeskillmaster.com"]]) {
    const link = page.getByRole("link", { name, exact: true });
    const found = await link.getAttribute("href");
    if (found !== href) throw new Error(`${name} links to ${found}, not ${href}`);
    const box = await link.boundingBox();
    const [tab] = await Promise.all([context.waitForEvent("page", { timeout: 10_000 }), press(link)]);
    const opened = (await tab.waitForEvent("request", { timeout: 10_000 }).catch(() => null))?.url() ?? tab.url();
    await tab.close();
    const at = outside.indexOf(opened);
    if (at >= 0) outside.splice(at, 1);
    if (!opened.startsWith(href)) throw new Error(`pressing ${name} opened ${opened}, not ${href}`);
    links[name] = { href: found, opened, box: box && Object.fromEntries(Object.entries(box).map(([k, v]) => [k, Math.round(v)])) };
  }
  steps.push({ step: "links", links });

  for (const profession of ["Alchemy", "Tailoring"]) {
    await press(page.getByRole("button", { name: profession, exact: true }));
    const words = await until(`${profession} priced`, shown(profession));
    steps.push({ step: `${profession} priced`, rows: words.match(/\d+ of \d+ recipes shown/)[0], shot: await shot(profession.toLowerCase()) });
  }

  // The DOM mirror carries no tooltip role, so the tooltip shows as the lines it adds to the page.
  const before = new Set((await text()).split("\n"));
  const header = await page.getByText("Recipe", { exact: true }).first().boundingBox();
  await page.mouse.move(header.x + 60, header.y + header.height / 2 + 64);
  const added = (words) => words.split("\n").filter((line) => line.trim() && !before.has(line));
  const tip = await until("tooltip", (w) => added(w).some((line) => /Item Level|Requires|Binds/.test(line)), 10_000);
  steps.push({ step: "tooltip", lines: added(tip).slice(0, 6), shot: await shot("tooltip") });
  await page.mouse.move(5, 995);

  await press(page.getByRole("button", { name: "Vendor flips", exact: true }));
  const flips = await until("flips listed", (w) => /Thunder Bluff/.test(w) && /Profit/.test(w));
  steps.push({ step: "flips", subtitle: flips.match(/Thunder Bluff, .*/)[0], shot: await shot("flips") });

  await press(page.getByRole("button", { name: "Settings", exact: true }));
  await until("settings", (w) => /Not affiliated with Blizzard/.test(w));
  await press(page.getByRole("button", { name: "Alliance", exact: true }));
  await until("Alliance prices", (w) => /PvP-Alliance/.test(w));
  steps.push({ step: "faction set", stored: await page.evaluate(() => localStorage.getItem("craftski:faction.txt")), shot: await shot("settings") });

  await page.reload();
  await page.waitForFunction(() => window.__firstFrame, null, { timeout: 60_000 });
  const kept = await until("Alliance kept after reload", (w) => /PvP-Alliance/.test(w) && /recipes shown/.test(w));
  steps.push({ step: "page reloaded", heading: kept.match(/(\w[\w ]*)\s*\n\s*\d+ of \d+ recipes shown/)?.[1], shot: await shot("reloaded") });

  const reads = priceReads;
  await press(page.getByRole("button", { name: "Reload TSM prices", exact: true }));
  await until("prices read again", (w) => priceReads > reads && /recipes shown/.test(w), 20_000);
  steps.push({ step: "Reload button", priceReads: priceReads - reads });
} catch (e) {
  errors.push(`smoke: ${e.message}`);
  await shot("failure").catch(() => {});
} finally {
  await browser.close();
  server.close();
}
console.log(JSON.stringify({ executablePath, steps, fetchedUpstream, outside, errors }, null, 2));
process.exit(errors.length === 0 ? 0 : 1);
