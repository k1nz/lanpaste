import { t, type MsgKey } from "./i18n";

const RULES: Array<{ test: RegExp; key: MsgKey }> = [
  { test: /expired/i, key: "err.tokenExpired" },
  { test: /token/i, key: "err.tokenInvalid" },
  { test: /offline/i, key: "err.offline" },
  { test: /reject|refus/i, key: "err.rejected" },
  { test: /removed|revok|401/i, key: "err.removed" },
  { test: /disk|space|volume/i, key: "err.disk" },
  { test: /payload_too_large|length limit exceeded|413/i, key: "err.tooLarge" },
  { test: /not found|gone|missing|no longer/i, key: "err.sourceGone" },
  { test: /unavailable/i, key: "err.unavailable" },
  { test: /无效快捷键|unsupported key|invalid hotkey|invalid format/i, key: "err.invalidShortcut" },
];

export function localizeError(err: unknown): string {
  const raw = err == null ? "" : String(err);
  for (const rule of RULES) {
    if (rule.test.test(raw)) return t(rule.key);
  }
  return raw && raw !== "Error" ? raw : t("err.generic");
}
