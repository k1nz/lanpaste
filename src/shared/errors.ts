import { t, type MsgKey } from "./i18n";

const RULES: Array<{ test: RegExp; key: MsgKey }> = [
  { test: /expired|已过期/i, key: "err.tokenExpired" },
  { test: /token|配对码不正确/i, key: "err.tokenInvalid" },
  { test: /offline|设备离线/i, key: "err.offline" },
  { test: /reject|refus|对端拒绝/i, key: "err.rejected" },
  { test: /removed|revok|401|设备已移除/i, key: "err.removed" },
  { test: /disk|space|volume|磁盘空间/i, key: "err.disk" },
  { test: /payload_too_large|length limit exceeded|413|体积过大/i, key: "err.tooLarge" },
  { test: /source gone|no longer|源设备已无/i, key: "err.sourceGone" },
  { test: /unavailable|尚未就绪/i, key: "err.unavailable" },
  { test: /无效快捷键|unsupported key|invalid hotkey|invalid format/i, key: "err.invalidShortcut" },
  { test: /条目不存在|item not found/i, key: "err.notFound" },
  { test: /需要先下载|download the file first/i, key: "err.needDownload" },
  { test: /没有可显示的文件|no file to (show|reveal)/i, key: "err.noFile" },
];

export function localizeError(err: unknown): string {
  const raw = err == null ? "" : String(err);
  for (const rule of RULES) {
    if (rule.test.test(raw)) return t(rule.key);
  }
  return t("err.generic");
}
