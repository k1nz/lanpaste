import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const zh = JSON.parse(readFileSync(join(root, "src/locales/zh-CN.json"), "utf8"));
const en = JSON.parse(readFileSync(join(root, "src/locales/en-US.json"), "utf8"));
const zhKeys = Object.keys(zh).sort();
const enKeys = Object.keys(en).sort();
const missingEn = zhKeys.filter((k) => !(k in en));
const missingZh = enKeys.filter((k) => !(k in zh));
if (missingEn.length || missingZh.length) {
  console.error("i18n key mismatch");
  if (missingEn.length) console.error("missing en-US:", missingEn.join(", "));
  if (missingZh.length) console.error("missing zh-CN:", missingZh.join(", "));
  process.exit(1);
}
console.log(`i18n ok: ${zhKeys.length} keys`);
