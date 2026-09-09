import type { HistoryEntry, PasteType } from "./types";
import { resolvedLocale, t, type MsgKey } from "./i18n";

export function toMs(ts: number): number {
  return ts < 1e12 ? ts * 1000 : ts;
}

export function startOfDay(d: Date): Date {
  const x = new Date(d);
  x.setHours(0, 0, 0, 0);
  return x;
}

export function groupDateLabel(copiedAt: number): string {
  const date = new Date(toMs(copiedAt));
  const today = startOfDay(new Date());
  const yesterday = new Date(today);
  yesterday.setDate(yesterday.getDate() - 1);
  const day = startOfDay(date);
  if (day.getTime() === today.getTime()) return t("overlay.today");
  if (day.getTime() === yesterday.getTime()) return t("overlay.yesterday");
  const sameYear = date.getFullYear() === today.getFullYear();
  return new Intl.DateTimeFormat(resolvedLocale.value, {
    month: "short",
    day: "numeric",
    year: sameYear ? undefined : "numeric",
  }).format(date);
}

export function relativeTime(copiedAt: number): string {
  const diff = Date.now() - toMs(copiedAt);
  const sec = Math.max(0, Math.floor(diff / 1000));
  if (sec < 45) return "";
  const min = Math.floor(sec / 60);
  if (min < 60) return `${min}m`;
  const hr = Math.floor(min / 60);
  if (hr < 24) return `${hr}h`;
  return "";
}

export function formatBytes(n: number): string {
  if (!Number.isFinite(n) || n < 0) return "—";
  if (n < 1024) return `${Math.round(n)} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(n < 10 * 1024 ? 1 : 0)} KB`;
  if (n < 1024 * 1024 * 1024) {
    const mb = n / (1024 * 1024);
    return `${mb.toFixed(mb < 10 ? 1 : 0)} MB`;
  }
  const gb = n / (1024 * 1024 * 1024);
  return `${gb.toFixed(gb < 10 ? 1 : 0)} GB`;
}

export function formatShortcut(raw: string): string {
  return raw
    .replace(/CommandOrControl/gi, "⌘")
    .replace(/Command|Cmd|Meta/gi, "⌘")
    .replace(/Shift/gi, "⇧")
    .replace(/Alt|Option/gi, "⌥")
    .replace(/Control|Ctrl/gi, "⌃")
    .replace(/Key([A-Z])/gi, "$1")
    .replace(/Digit([0-9])/g, "$1")
    .replace(/Arrow(Up|Down|Left|Right)/gi, "$1")
    .replace(/\s*\+\s*/g, "");
}

const MODIFIER_KEYS = new Set([
  "Alt",
  "AltGraph",
  "CapsLock",
  "Control",
  "Fn",
  "Meta",
  "OS",
  "Shift",
  "Super",
  "Hyper",
]);

const CODE_TO_KEY: Record<string, string> = {
  Space: "Space",
  Tab: "Tab",
  Enter: "Enter",
  Backspace: "Backspace",
  Delete: "Delete",
  Home: "Home",
  End: "End",
  PageUp: "PageUp",
  PageDown: "PageDown",
  Insert: "Insert",
  ArrowUp: "Up",
  ArrowDown: "Down",
  ArrowLeft: "Left",
  ArrowRight: "Right",
  Minus: "-",
  Equal: "=",
  BracketLeft: "[",
  BracketRight: "]",
  Backslash: "\\",
  Semicolon: ";",
  Quote: "'",
  Comma: ",",
  Period: ".",
  Slash: "/",
  Backquote: "`",
};

function codeToShortcutKey(code: string): string | null {
  if (/^Key[A-Z]$/.test(code)) return code.slice(3);
  if (/^Digit[0-9]$/.test(code)) return code.slice(5);
  if (/^F([1-9]|1[0-9]|2[0-4])$/.test(code)) return code;
  if (CODE_TO_KEY[code]) return CODE_TO_KEY[code];
  if (/^Numpad/.test(code)) return code;
  return null;
}

/** Convert a keydown event into a Tauri global-shortcut string, or null if incomplete. */
export function eventToShortcut(ev: KeyboardEvent): string | null {
  if (MODIFIER_KEYS.has(ev.key)) return null;
  const key = codeToShortcutKey(ev.code);
  if (!key) return null;
  const functionKey = /^F([1-9]|1[0-9]|2[0-4])$/.test(key);
  const hasPrimaryMod = ev.metaKey || ev.ctrlKey || ev.altKey;
  if (!hasPrimaryMod && !functionKey) return null;
  const parts: string[] = [];
  if (ev.metaKey) parts.push("Command");
  if (ev.ctrlKey) parts.push("Control");
  if (ev.altKey) parts.push("Option");
  if (ev.shiftKey) parts.push("Shift");
  parts.push(key);
  return parts.join("+");
}

export function typeLabel(type: PasteType): string {
  const key = `type.${type}` as MsgKey;
  return t(key);
}

const IMAGE_FILE_RE = /\.(png|jpe?g|gif|tiff?|bmp|webp|ico|heic|heif|svg)$/i;

export function isImageFileName(name?: string | null): boolean {
  if (!name) return false;
  const base = name.split(/[/\\]/).pop() ?? name;
  return IMAGE_FILE_RE.test(base);
}

export function hasImagePreview(entry: HistoryEntry): boolean {
  if (entry.primaryType === "image") return true;
  if (entry.preview.imageThumb) return true;
  return (
    isImageFileName(entry.preview.fileName) ||
    isImageFileName(entry.title) ||
    isImageFileName(entry.preview.path)
  );
}

export function prefersReducedMotion(): boolean {
  return window.matchMedia("(prefers-reduced-motion: reduce)").matches;
}

export function truncateFingerprint(fp: string, len = 8): string {
  if (fp.length <= len) return fp;
  return fp.slice(0, len);
}
