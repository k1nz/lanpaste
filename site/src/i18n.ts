import { createI18n } from "vue-i18n";
import en from "./locales/en-US.json";
import zh from "./locales/zh-CN.json";

export const LOCALES = ["en-US", "zh-CN"] as const;
export type LocaleCode = (typeof LOCALES)[number];

const STORAGE_KEY = "lanpaste-lang";

export function detectLocale(): LocaleCode {
  try {
    const stored = localStorage.getItem(STORAGE_KEY);
    if (stored === "en-US" || stored === "zh-CN") return stored;
  } catch {
    /* ignore */
  }
  if (typeof navigator !== "undefined" && navigator.language.toLowerCase().startsWith("zh")) {
    return "zh-CN";
  }
  return "en-US";
}

export function persistLocale(locale: LocaleCode) {
  try {
    localStorage.setItem(STORAGE_KEY, locale);
  } catch {
    /* ignore */
  }
  document.documentElement.lang = locale === "zh-CN" ? "zh-CN" : "en";
}

export const i18n = createI18n({
  legacy: false,
  locale: "en-US",
  fallbackLocale: "en-US",
  messages: {
    "en-US": en,
    "zh-CN": zh,
  },
});
