#!/usr/bin/env node
/**
 * Token discipline check (Development.md §7.3, docs/design/design-system.md).
 *
 * Every colour, spacing, radius, motion and layout value in the UI must come
 * from tokens.css. This script fails the build if a literal slips in.
 *
 * Scope: the UI layer (components/, pages/, hooks/, App.tsx, main.tsx).
 * Excluded: src/services/mock/ (fixture artwork generation, not UI styling)
 *           and src/styles/tokens.css (the definition site).
 */

import { readdirSync, readFileSync, statSync } from "node:fs";
import { join, relative, sep } from "node:path";

const ROOT = new URL("../src", import.meta.url).pathname.replace(/^\//, "");
const TOKENS = "styles/tokens.css";
const MOCK_DIR = "services/mock";

const colourRe = /#[0-9a-fA-F]{3,8}\b|rgba?\s*\(|hsla?\s*\(/g;
const pxRe = /\b\d+px\b/g;

const files = [];
function walk(dir) {
  for (const entry of readdirSync(dir)) {
    const full = join(dir, entry);
    const st = statSync(full);
    if (st.isDirectory()) walk(full);
    else if (/\.(ts|tsx|css)$/.test(entry)) files.push(full);
  }
}
walk(ROOT);

const violations = [];
for (const file of files) {
  const rel = relative(ROOT, file).split(sep).join("/");
  if (rel === TOKENS) continue;
  if (rel.startsWith(MOCK_DIR + "/")) continue;
  const content = readFileSync(file, "utf8");
  const lines = content.split(/\r?\n/);
  lines.forEach((line, i) => {
    const colourMatches = line.match(colourRe);
    if (colourMatches) {
      violations.push(`${rel}:${i + 1}: colour literal ${colourMatches.join(", ")}`);
    }
    const pxMatches = line.match(pxRe);
    if (pxMatches) {
      violations.push(`${rel}:${i + 1}: px literal ${pxMatches.join(", ")}`);
    }
  });
}

if (violations.length > 0) {
  console.error("Token discipline violations:");
  for (const v of violations) console.error("  " + v);
  console.error(`\n${violations.length} violation(s). Move these values into src/styles/tokens.css.`);
  process.exit(1);
}

console.log(`check-tokens: ${files.length - 1} UI files scanned, 0 violations.`);
