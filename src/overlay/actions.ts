import type { HistoryEntry, PairedDevice } from "../shared/types";
import { t } from "../shared/i18n";
import { isApplePlatform } from "../shared/ipc";

export type ActionId =
  | "paste"
  | "copy"
  | "sync"
  | "reveal"
  | "delete"
  | "copyColor"
  | "copyUrl";

export interface MenuItem {
  id: ActionId;
  label: string;
  disabled?: boolean;
  destructive?: boolean;
  submenu?: boolean;
}

export function buildActionItems(
  entry: HistoryEntry | null,
  devices: PairedDevice[],
): MenuItem[] {
  if (!entry) return [];
  const list: MenuItem[] = [
    { id: "paste", label: t("overlay.paste") },
    { id: "copy", label: t("overlay.copyToClipboard") },
    {
      id: "sync",
      label: t("overlay.syncTo"),
      submenu: true,
      disabled: devices.length === 0,
    },
  ];
  if (entry.primaryType === "file") {
    list.push({
      id: "reveal",
      label: t(isApplePlatform() ? "overlay.revealInFinder" : "overlay.revealInExplorer"),
    });
  }
  list.push({ id: "delete", label: t("overlay.delete"), destructive: true });
  if (entry.primaryType === "color" && entry.preview.color) {
    list.push({ id: "copyColor", label: t("overlay.copyColor") });
  }
  if (entry.primaryType === "url" && entry.preview.url) {
    list.push({ id: "copyUrl", label: t("overlay.copyUrl") });
  }
  return list;
}
