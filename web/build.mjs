// Builds the browser app into site/app/, which `npm run deploy` ships with the rest of site/.
// cargo compiles the library to wasm and the wasm-bindgen CLI writes its JavaScript glue. Prints
// each step's download size. CARGO_PROFILE_RELEASE_* variables override the size settings.
import { execFileSync } from "node:child_process";
import { readFileSync, rmSync } from "node:fs";
import { homedir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { brotliCompressSync, constants, gzipSync } from "node:zlib";

const here = dirname(fileURLToPath(import.meta.url));
const root = resolve(here, "..");
const out = join(here, "site", "app");
const cargoHome = process.env.CARGO_HOME ?? join(homedir(), ".cargo");
const exe = (name) => name + (process.platform === "win32" ? ".exe" : "");
const cargoBin = (name) => join(cargoHome, "bin", exe(name));

const lock = readFileSync(join(root, "Cargo.lock"), "utf8");
const bindgen = lock.match(/name = "wasm-bindgen"\nversion = "([^"]+)"/)[1];
const installed = execFileSync(cargoBin("wasm-bindgen"), ["--version"], { encoding: "utf8" }).trim();
if (installed !== `wasm-bindgen ${bindgen}`) {
  console.error(`${installed} does not match Cargo.lock; run: cargo install wasm-bindgen-cli --version ${bindgen} --locked`);
  process.exit(2);
}

const sizes = [];
const measure = (step, file) => {
  const bytes = readFileSync(file);
  const brotli = brotliCompressSync(bytes, { params: { [constants.BROTLI_PARAM_QUALITY]: 11, [constants.BROTLI_PARAM_SIZE_HINT]: bytes.length } });
  sizes.push({ step, raw: bytes.length, gzip: gzipSync(bytes, { level: 9 }).length, brotli: brotli.length });
};

const started = Date.now();
const env = {
  ...process.env,
  CARGO_PROFILE_RELEASE_OPT_LEVEL: process.env.CARGO_PROFILE_RELEASE_OPT_LEVEL ?? "z",
  CARGO_PROFILE_RELEASE_LTO: process.env.CARGO_PROFILE_RELEASE_LTO ?? "fat",
  CARGO_PROFILE_RELEASE_CODEGEN_UNITS: "1",
  CARGO_PROFILE_RELEASE_DEBUG: "0",
  CARGO_PROFILE_RELEASE_STRIP: "debuginfo",
  RUSTFLAGS: `${process.env.RUSTFLAGS ?? ""} --remap-path-prefix=${homedir()}=/home --remap-path-prefix=${cargoHome}=/cargo`.trim(),
};
// A cdylib only for this build, so the desktop build never links one.
execFileSync(cargoBin("cargo"), ["rustc", "--release", "--locked", "--target", "wasm32-unknown-unknown", "--lib", "--crate-type", "cdylib"], { cwd: root, env, stdio: "inherit" });
const compiled = join(root, "target", "wasm32-unknown-unknown", "release", "craftski.wasm");
measure("cargo", compiled);

rmSync(out, { recursive: true, force: true });
execFileSync(cargoBin("wasm-bindgen"), ["--target", "web", "--no-typescript", "--remove-name-section", "--remove-producers-section", "--out-dir", out, "--out-name", "craftski", compiled], { stdio: "inherit" });
measure("wasm-bindgen", join(out, "craftski_bg.wasm"));
measure("glue (craftski.js)", join(out, "craftski.js"));

console.table(sizes);
console.log(`Built ${out} in ${Math.round((Date.now() - started) / 1000)} s.`);
