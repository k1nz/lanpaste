import { computed, onMounted, ref } from "vue";
import {
  FALLBACK_ASSETS,
  GITHUB_LATEST_API,
  GITHUB_RELEASES_API,
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

export function detectPlatform(): PlatformId {
  if (typeof navigator === "undefined") return "mac-arm";
  const ua = navigator.userAgent;
  const platform = navigator.platform || "";
  const isWin = /Win/.test(platform) || /Windows/.test(ua);
  if (isWin) return "windows";
  const isMac = /Mac/.test(platform) || /Mac OS/.test(ua);
  if (isMac) {
    const arch = (
      navigator as Navigator & { userAgentData?: { architecture?: string } }
    ).userAgentData?.architecture;
    if (arch === "x86") return "mac-intel";
    return "mac-arm";
  }
  return "mac-arm";
}

const assets = ref<Assets>({ ...FALLBACK_ASSETS });
const loading = ref(true);
const error = ref(false);
const platform = ref<PlatformId>("mac-arm");
let started = false;

export function useRelease() {
  const primaryUrl = computed(() => assets.value[platform.value]);

  onMounted(() => {
    platform.value = detectPlatform();
    if (started) return;
    started = true;
    void load();
  });

  return { assets, loading, error, platform, primaryUrl };
}

async function load() {
  try {
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
    const parsed = parseAssets(release?.assets);
    assets.value = {
      "mac-arm": parsed["mac-arm"] ?? FALLBACK_ASSETS["mac-arm"],
      "mac-intel": parsed["mac-intel"] ?? FALLBACK_ASSETS["mac-intel"],
      windows: parsed.windows ?? FALLBACK_ASSETS.windows,
    };
  } catch {
    error.value = true;
    assets.value = { ...FALLBACK_ASSETS };
  } finally {
    loading.value = false;
  }
}
