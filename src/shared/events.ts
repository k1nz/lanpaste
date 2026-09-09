import { isTauri } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { PairingInputPayload, PairingShowPayload, TransferProgressPayload } from "./types";

export async function onEvent<T>(
  name: string,
  handler: (payload: T) => void,
): Promise<UnlistenFn> {
  if (!isTauri()) return () => {};
  try {
    return await listen<T>(name, (event) => handler(event.payload));
  } catch (err) {
    console.warn(`[lanpaste] listen ${name} failed`, err);
    return () => {};
  }
}

export function onHistoryChanged(handler: () => void) {
  return onEvent<unknown>("history-changed", () => handler());
}

export function onDevicesChanged(handler: () => void) {
  return onEvent<unknown>("devices-changed", () => handler());
}

export function onPairingShow(handler: (payload: PairingShowPayload) => void) {
  return onEvent<PairingShowPayload>("pairing-show", handler);
}

export function onPairingHide(handler: () => void) {
  return onEvent<unknown>("pairing-hide", () => handler());
}

export function onPairingInput(handler: (payload: PairingInputPayload) => void) {
  return onEvent<PairingInputPayload>("pairing-input", handler);
}

export function onTransferProgress(handler: (payload: TransferProgressPayload) => void) {
  return onEvent<TransferProgressPayload>("transfer-progress", handler);
}

export function onOverlayShown(handler: () => void) {
  return onEvent<unknown>("overlay-shown", () => handler());
}

export function onLocaleChanged(handler: (resolved: string) => void) {
  return onEvent<string>("locale-changed", handler);
}
