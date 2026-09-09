import { ref } from "vue";
import enUS from "../locales/en-US.json";
import zhCN from "../locales/zh-CN.json";

export type LocaleId = "en-US" | "zh-CN";
export type LocalePref = "system" | LocaleId;
export type MsgKey = keyof typeof enUS;

const catalogs: Record<LocaleId, Record<MsgKey, string>> = {
  "en-US": enUS,
  "zh-CN": zhCN,
};

export const localePref = ref<LocalePref>("system");
export const resolvedLocale = ref<LocaleId>(detectSystemLocale());

export function detectSystemLocale(): LocaleId {
  const lang =
    typeof navigator !== "undefined" ? navigator.language || navigator.languages?.[0] || "" : "";
  return lang.toLowerCase().startsWith("zh") ? "zh-CN" : "en-US";
}

export function isLocalePref(value: string | undefined | null): value is LocalePref {
  return value === "system" || value === "en-US" || value === "zh-CN";
}

export function resolveLocale(pref: LocalePref, system = detectSystemLocale()): LocaleId {
  if (pref === "en-US" || pref === "zh-CN") return pref;
  return system;
}

function applyDocumentLang(locale: LocaleId) {
  if (typeof document === "undefined") return;
  document.documentElement.lang = locale;
}

export function applyLocalePref(pref: LocalePref) {
  localePref.value = pref;
  resolvedLocale.value = resolveLocale(pref);
  applyDocumentLang(resolvedLocale.value);
}

export function setResolvedLocale(locale: LocaleId) {
  resolvedLocale.value = locale;
  applyDocumentLang(locale);
}

export function t(key: MsgKey, vars?: Record<string, string | number>): string {
  const loc = resolvedLocale.value;
  const table = catalogs[loc];
  const fallback = catalogs["en-US"];
  let out = table[key] || fallback[key] || fallback["err.generic"];
  if (!vars) return out;
  for (const [k, v] of Object.entries(vars)) {
    out = out.replace(new RegExp(`\\{${k}\\}`, "g"), String(v));
  }
  return out;
}

const GENERIC_CLIPBOARD = new Set([
  enUS["overlay.genericClipboard"],
  zhCN["overlay.genericClipboard"],
]);
const GENERIC_IMAGE = new Set([enUS["type.image"], zhCN["type.image"]]);
const GENERIC_LOCAL = new Set([enUS["overlay.local"], zhCN["overlay.local"]]);
const OPAQUE_TITLES = new Set([
  enUS["type.rtf"],
  zhCN["type.rtf"],
  enUS["type.html"],
  zhCN["type.html"],
  "RTF",
  "HTML",
]);

function oneLine(s: string): string {
  return s.replace(/\s+/g, " ").trim();
}

function isOpaqueTitle(title: string): boolean {
  const t = title.trim();
  if (!t) return true;
  if (OPAQUE_TITLES.has(t)) return true;
  if (GENERIC_CLIPBOARD.has(t)) return true;
  return t.startsWith("<") || t.startsWith("{\\rtf");
}

function previewSnippet(preview?: { text?: string; html?: string }): string {
  for (const raw of [preview?.text, preview?.html]) {
    if (!raw) continue;
    const line = oneLine(raw);
    if (line && !isOpaqueTitle(line)) return line;
  }
  return "";
}

export function displayTitle(
  title: string,
  primaryType?: string,
  preview?: { text?: string; html?: string },
): string {
  if (GENERIC_IMAGE.has(title)) return t("type.image");
  const snippet = previewSnippet(preview);
  if (snippet && (isOpaqueTitle(title) || primaryType === "rtf" || primaryType === "html")) {
    return snippet;
  }
  if (GENERIC_CLIPBOARD.has(title)) {
    return primaryType === "image" ? t("type.image") : t("overlay.genericClipboard");
  }
  return title;
}

export function displaySource(name: string | null | undefined): string {
  if (!name || GENERIC_LOCAL.has(name)) return t("overlay.local");
  return name;
}
