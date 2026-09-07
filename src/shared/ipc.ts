import { convertFileSrc, invoke, isTauri } from "@tauri-apps/api/core";
import type {
  AppSettings,
  HistoryEntry,
  NearbyDevice,
  PairedDevice,
  PasteType,
} from "./types";

export const DEFAULT_SETTINGS: AppSettings = {
  autoSyncMaxBytes: 20 * 1024 * 1024,
  cleanupMaxItems: 500,
  cleanupMaxBytes: 1024 * 1024 * 1024,
  cleanupMaxAgeDays: null,
  overlayShortcut: "CommandOrControl+Shift+V",
};

export async function invokeSafe<T>(
  cmd: string,
  args?: Record<string, unknown>,
): Promise<T | null> {
  if (!isTauri()) return null;
  try {
    return await invoke<T>(cmd, args);
  } catch (err) {
    console.warn(`[lanpaste] ${cmd} failed`, err);
    return null;
  }
}

export async function invokeResult(
  cmd: string,
  args?: Record<string, unknown>,
): Promise<{ ok: true } | { ok: false; error: string }> {
  if (!isTauri()) return { ok: false, error: "unavailable" };
  try {
    await invoke(cmd, args);
    return { ok: true };
  } catch (err) {
    return { ok: false, error: String(err) };
  }
}

export async function listHistory(
  query?: string,
  typeFilter: PasteType | "all" = "all",
): Promise<HistoryEntry[]> {
  const args: Record<string, unknown> = { typeFilter };
  if (query && query.trim()) args.query = query.trim();
  return (await invokeSafe<HistoryEntry[]>("list_history", args)) ?? [];
}

export async function getEntry(id: string): Promise<HistoryEntry | null> {
  return invokeSafe<HistoryEntry | null>("get_entry", { id });
}

export function pasteEntry(id: string) {
  return invokeResult("paste_entry", { id });
}

export function hideOverlay() {
  return invokeResult("hide_overlay");
}

export function showSettings() {
  return invokeResult("show_settings");
}

export function copyEntryToClipboard(id: string) {
  return invokeResult("copy_entry_to_clipboard", { id });
}

export function syncTo(id: string, deviceId: string) {
  return invokeResult("sync_to", { id, deviceId });
}

export function deleteEntry(id: string) {
  return invokeResult("delete_entry", { id });
}

export function revealInFinder(id: string) {
  return invokeResult("reveal_in_finder", { id });
}

export async function listNearby(): Promise<NearbyDevice[]> {
  return (await invokeSafe<NearbyDevice[]>("list_nearby")) ?? [];
}

export async function listPaired(): Promise<PairedDevice[]> {
  return (await invokeSafe<PairedDevice[]>("list_paired")) ?? [];
}

export function startPair(instanceId: string) {
  return invokeResult("start_pair", { instanceId });
}

export function submitPairToken(instanceId: string, token: string) {
  return invokeResult("submit_pair_token", { instanceId, token });
}

export function cancelPair() {
  return invokeResult("cancel_pair");
}

export function updateDeviceNote(instanceId: string, note: string) {
  return invokeResult("update_device_note", { instanceId, note });
}

export function updateDeviceFlags(
  instanceId: string,
  flags: Pick<PairedDevice, "allowSend" | "allowReceive" | "autoWriteClipboard">,
) {
  return invokeResult("update_device_flags", { instanceId, ...flags });
}

export function removeDevice(instanceId: string) {
  return invokeResult("remove_device", { instanceId });
}

export async function getSettings(): Promise<AppSettings> {
  return (await invokeSafe<AppSettings>("get_settings")) ?? { ...DEFAULT_SETTINGS };
}

export function updateSettings(partial: Partial<AppSettings>) {
  return invokeResult("update_settings", { ...partial });
}

export async function frontmostAppName(): Promise<string> {
  return (await invokeSafe<string>("frontmost_app_name")) ?? "";
}

export function mediaSrc(raw?: string): string | undefined {
  if (!raw) return undefined;
  if (
    raw.startsWith("data:") ||
    raw.startsWith("blob:") ||
    raw.startsWith("http:") ||
    raw.startsWith("https:") ||
    raw.startsWith("asset:") ||
    raw.startsWith("asset://")
  ) {
    return raw;
  }
  try {
    return convertFileSrc(raw);
  } catch {
    return raw;
  }
}

export async function copyText(text: string): Promise<boolean> {
  try {
    await navigator.clipboard.writeText(text);
    return true;
  } catch {
    return false;
  }
}
