// Checks the UI copy before every build (npm run i18n:check):
//   1. every language file has exactly the keys of the reference (zh-TW) and no empty strings;
//   2. each translation uses the same {{placeholders}} as the reference;
//   3. every key the code asks for exists, and every key in the files is used somewhere;
//   4. no user-visible words are written straight into components (JSX text or text attributes).
// Exit code 1 on any error; unused keys are reported as warnings.

import { readFileSync, readdirSync, statSync } from "node:fs";
import { join, relative } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("..", import.meta.url));
const src = join(root, "src");
const localeDir = join(src, "i18n", "locales");
const REFERENCE = "zh-TW";

const errors = [];
const warnings = [];

function flatten(obj, prefix = "", out = new Map()) {
  for (const [k, v] of Object.entries(obj)) {
    const key = prefix ? `${prefix}.${k}` : k;
    if (v && typeof v === "object") flatten(v, key, out);
    else out.set(key, String(v));
  }
  return out;
}

const placeholders = (s) => [...s.matchAll(/\{\{\s*(\w+)[^}]*\}\}/g)].map((m) => m[1]).sort().join(",");

// ---- 1 & 2: language files ----
const locales = Object.fromEntries(
  readdirSync(localeDir)
    .filter((f) => f.endsWith(".json"))
    .map((f) => [f.replace(/\.json$/, ""), flatten(JSON.parse(readFileSync(join(localeDir, f), "utf8")))]),
);
const reference = locales[REFERENCE];
if (!reference) {
  console.error(`missing reference language file ${REFERENCE}.json`);
  process.exit(1);
}
for (const [lng, keys] of Object.entries(locales)) {
  for (const [key, value] of keys) {
    if (!value.trim()) errors.push(`${lng}.json: "${key}" is empty`);
    if (lng === REFERENCE) continue;
    if (!reference.has(key)) errors.push(`${lng}.json: "${key}" is not in ${REFERENCE}.json`);
    else if (placeholders(value) !== placeholders(reference.get(key)))
      errors.push(`${lng}.json: "${key}" uses {{${placeholders(value)}}} but ${REFERENCE} uses {{${placeholders(reference.get(key))}}}`);
  }
  if (lng !== REFERENCE) for (const key of reference.keys()) if (!keys.has(key)) errors.push(`${lng}.json: missing "${key}"`);
}

// ---- 3 & 4: source files ----
function walk(dir) {
  return readdirSync(dir).flatMap((name) => {
    const path = join(dir, name);
    if (statSync(path).isDirectory()) return walk(path);
    return /\.(tsx?|jsx?)$/.test(name) ? [path] : [];
  });
}

const isKey = (k) => reference.has(k);
const isPrefix = (p) => [...reference.keys()].some((k) => k.startsWith(p));
const used = new Set();
const dynamicPrefixes = new Set();
// Words people read: Latin letters, kana or CJK ideographs.
const WORDS = /[A-Za-z぀-ヿ㐀-鿿]/;

for (const file of walk(src)) {
  const rel = relative(root, file).replaceAll("\\", "/");
  const code = readFileSync(file, "utf8");

  // t("a.b"), t('a.b'), t(`a.${x}`), i18n.t(...)
  for (const m of code.matchAll(/\bt\(\s*(["'`])([^"'`]+)\1/g)) {
    const key = m[2];
    const dyn = key.indexOf("${");
    if (dyn === -1) {
      if (isKey(key)) used.add(key);
      else errors.push(`${rel}: t("${key}") has no entry in ${REFERENCE}.json`);
    } else {
      const prefix = key.slice(0, dyn);
      if (!isPrefix(prefix)) errors.push(`${rel}: t(\`${key}\`) matches no key in ${REFERENCE}.json`);
      dynamicPrefixes.add(prefix);
    }
  }
  // Keys passed around as plain strings, e.g. navigation tables.
  for (const m of code.matchAll(/["'`]([a-zA-Z]+(?:\.[\w-]+)+)["'`]/g)) if (isKey(m[1])) used.add(m[1]);

  if (!file.endsWith("x")) continue;
  // JSX text between tags, e.g. <th>capacity</th>; "=>" is an arrow function, not a tag.
  for (const m of code.matchAll(/(?<!=)>([^<>{}()=;&|]*)</g)) {
    const text = m[1].trim();
    if (text && WORDS.test(text)) errors.push(`${rel}: hard-coded text "${text}" (move it to the language files)`);
  }
  // Text-bearing attributes with a literal value
  for (const m of code.matchAll(/\b(placeholder|title|alt|aria-label|label)="([^"]*)"/g))
    if (WORDS.test(m[2])) errors.push(`${rel}: hard-coded ${m[1]}="${m[2]}"`);
}

for (const key of reference.keys()) {
  // Keys built from API codes (e.g. notice.<code>) count as used through their prefix.
  if (used.has(key) || [...dynamicPrefixes].some((p) => key.startsWith(p))) continue;
  warnings.push(`unused key "${key}"`);
}

for (const w of warnings) console.warn(`warning: ${w}`);
for (const e of errors) console.error(`error: ${e}`);
const langs = Object.keys(locales).join(", ");
if (errors.length) {
  console.error(`i18n check failed: ${errors.length} error(s) in ${langs}`);
  process.exit(1);
}
console.log(`i18n check passed: ${reference.size} keys × ${Object.keys(locales).length} languages (${langs})${warnings.length ? `, ${warnings.length} warning(s)` : ""}`);
