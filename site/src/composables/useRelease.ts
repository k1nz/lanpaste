import { computed, onMounted, ref } from "vue";
import {
  FALLBACK_ASSETS,
  GITHUB_LATEST_API,
  GITHUB_RELEASES,
  GITHUB_RELEASES_API,
  VERSION_MANIFEST_URL,
  latestAssetUrl,
  type PlatformId,
} from "../constants";

export type { PlatformId };

type Assets = Record<PlatformId, string>;

interface GhAsset {
  name: string;
  browser_download_url: string;
}

interface GhRelease {
  assets?: GhAsset[];
}

export interface PlatformDetection {
  /** Concrete build to feature, or null when none can be inferred. */
  platform: PlatformId | null;
  /** macOS visitor whose CPU architecture could not be determined. */
  macArchUnknown: boolean;
}

function parseAssets(assets: GhAsset[] | undefined): Partial<Assets> {
  const out: Partial<Assets> = {};
  for (const asset of assets ?? []) {
    const name = asset.name.toLowerCase();
    if (name.endsWith(".dmg") && name.includes("aarch64")) {
      out["mac-arm"] = asset.browser_download_url;
    } else if (name.endsWith(".dmg") && name.includes("x64")) {
      out["mac-intel"] = asset.browser_download_url;
    } else if (name.includes("x64-setup.exe")) {
      out.windows = asset.browser_download_url;
    }
  }
  return out;
}

function assetsForVersion(version: string): Assets {
  return {
    "mac-arm": latestAssetUrl("mac-arm", version),
    "mac-intel": latestAssetUrl("mac-intel", version),
    windows: latestAssetUrl("windows", version),
  };
}

export function detectPlatform(): PlatformDetection {
  if (typeof navigator === "undefined") {
    return { platform: null, macArchUnknown: false };
  }
  const ua = navigator.userAgent;
  const platform = navigator.platform || "";
  const isWin = /Win/.test(platform) || /Windows/.test(ua);
  if (isWin) return { platform: "windows", macArchUnknown: false };
  const isMac = /Mac/.test(platform) || /Mac OS/.test(ua);
  // Linux and everything else have no dedicated build: fall through to the
  // generic "other builds" path instead of guessing a macOS download.
  if (!isMac) return { platform: null, macArchUnknown: false };
  const arch = (
    navigator as Navigator & { userAgentData?: { architecture?: string } }
  ).userAgentData?.architecture;
  if (arch === "x86") return { platform: "mac-intel", macArchUnknown: false };
  if (arch === "arm") return { platform: "mac-arm", macArchUnknown: false };
  // Safari and Firefox do not expose an architecture, and Intel/Apple Silicon
  // Macs share a user agent. Report unknown rather than defaulting to arm64.
  return { platform: null, macArchUnknown: true };
}

const assets = ref<Assets>({ ...FALLBACK_ASSETS });
const loading = ref(true);
const error = ref(false);
const platform = ref<PlatformId | null>(null);
const macArchUnknown = ref(false);
let started = false;

export function useRelease() {
  const primaryUrl = computed(() =>
    platform.value ? assets.value[platform.value] : GITHUB_RELEASES,
  );

  onMounted(() => {
    const detected = detectPlatform();
    platform.value = detected.platform;
    macArchUnknown.value = detected.macArchUnknown;
    if (started) return;
    started = true;
    void load();
  });

  return { assets, loading, error, platform, macArchUnknown, primaryUrl };
}

async function fetchReleaseAssets(): Promise<Partial<Assets>> {
  let res = await fetch(GITHUB_LATEST_API, {
    headers: { Accept: "application/vnd.github+json" },
  });
  if (res.status === 404) {
    res = await fetch(GITHUB_RELEASES_API, {
      headers: { Accept: "application/vnd.github+json" },
    });
  }
  if (!res.ok) throw new Error("release fetch failed");
  const data = (await res.json()) as GhRelease | GhRelease[];
  const release = Array.isArray(data) ? data[0] : data;
  return parseAssets(release?.assets);
}

async function fetchCurrentVersion(): Promise<string> {
  const res = await fetch(VERSION_MANIFEST_URL, {
    headers: { Accept: "application/json" },
  });
  if (!res.ok) throw new Error("version manifest fetch failed");
  const data = (await res.json()) as { version?: unknown };
  if (typeof data.version !== "string" || !/^\d+\.\d+\.\d+/.test(data.version)) {
    throw new Error("invalid version manifest");
  }
  return data.version;
}

async function load() {
  try {
    const parsed = await fetchReleaseAssets();
    assets.value = {
      "mac-arm": parsed["mac-arm"] ?? FALLBACK_ASSETS["mac-arm"],
      "mac-intel": parsed["mac-intel"] ?? FALLBACK_ASSETS["mac-intel"],
      windows: parsed.windows ?? FALLBACK_ASSETS.windows,
    };
    error.value = false;
  } catch {
    // The releases API can be rate-limited or blocked. The version manifest
    // lives on a different, unthrottled host, so use it to point at the newest
    // release instead of falling back to a frozen filename.
    try {
      assets.value = assetsForVersion(await fetchCurrentVersion());
      error.value = false;
    } catch {
      error.value = true;
      assets.value = { ...FALLBACK_ASSETS };
    }
  } finally {
    loading.value = false;
  }
}
