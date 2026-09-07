import type { PasteType } from "./types";
import { t, type MsgKey } from "./i18n";

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
  return new Intl.DateTimeFormat("zh-CN", {
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
    .replace(/\s*\+\s*/g, "");
}

export function typeLabel(type: PasteType): string {
  const key = `type.${type}` as MsgKey;
  return t(key);
}

export function prefersReducedMotion(): boolean {
  return window.matchMedia("(prefers-reduced-motion: reduce)").matches;
}

export function truncateFingerprint(fp: string, len = 8): string {
  if (fp.length <= len) return fp;
  return fp.slice(0, len);
}
