export type PasteType =
  | "text"
  | "url"
  | "color"
  | "html"
  | "rtf"
  | "image"
  | "file";

export interface HistoryEntry {
  id: string;
  copiedAt: number;
  sourceDeviceId: string | null;
  sourceDeviceName: string;
  primaryType: PasteType;
  title: string;
  preview: {
    text?: string;
    color?: string;
    url?: string;
    html?: string;
    imageThumb?: string;
    width?: number;
    height?: number;
    fileName?: string;
    fileSize?: number;
    path?: string;
  };
  needsFileDownload: boolean;
  fileDownloadState: "idle" | "downloading" | "failed";
}

export interface NearbyDevice {
  instanceId: string;
  name: string;
  fingerprint: string;
}

export interface PairedDevice {
  instanceId: string;
  name: string;
  note: string;
  online: boolean;
  allowSend: boolean;
  allowReceive: boolean;
  autoWriteClipboard: boolean;
  trustBroken: boolean;
}

export type LocalePref = "system" | "en-US" | "zh-CN";

export interface AppSettings {
  autoSyncMaxBytes: number;
  cleanupMaxItems: number;
  cleanupMaxBytes: number;
  cleanupMaxAgeDays: number | null;
  overlayShortcut: string;
  locale: LocalePref;
}

export interface PairingShowPayload {
  token: string;
  expiresAt: number;
}

export interface PairingInputPayload {
  instanceId: string;
  deviceName: string;
}

export interface TransferProgressPayload {
  id: string;
  received: number;
  total: number;
}
