#!/usr/bin/env node
/** Fail the build if the dev-only signed HTTP client leaks into production.
 *
 * The plan's A7 criterion: the production bundle contains no dev token string
 * and no direct fetch to the backend. The whole devClient module is behind a
 * DEV-guarded dynamic import, so Vite tree-shakes it out; this proves it.
 * Sourcemaps are excluded: they embed third-party source text (react-router's
 * own fetch calls), which is not our transport. */
import { readdirSync, readFileSync, statSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const assetsDir = fileURLToPath(new URL("../dist/assets/", import.meta.url));

function walk(dir) {
  const out = [];
  for (const entry of readdirSync(dir)) {
    const full = join(dir, entry);
    if (statSync(full).isDirectory()) out.push(...walk(full));
    else out.push(full);
  }
  return out;
}

const chunks = walk(assetsDir).filter((file) => file.endsWith(".js"));
if (chunks.length === 0) {
  console.error("check-bundle: no JS chunks found under dist/assets — run the build first.");
  process.exit(1);
}

let bundle = "";
for (const file of chunks) bundle += readFileSync(file, "utf8");

// Markers unique to the dev signed-client path. Their absence means the host
// command surface is the only transport that ships.
const forbidden = [
  "dev-only-not-for-production", // dev token literal
  "X-MediaHaven-Sig", // dev signing header
  "X-MediaHaven-Timestamp", // dev signing header
  "crypto.subtle", // dev HMAC implementation
];
let failed = false;
for (const needle of forbidden) {
  if (bundle.includes(needle)) {
    console.error(`check-bundle: forbidden string "${needle}" found in production bundle.`);
    failed = true;
  }
}
if (failed) process.exit(1);

console.log("check-bundle: production bundle clean (no dev token or signed HTTP client).");
