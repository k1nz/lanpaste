export const GITHUB_REPO = "https://github.com/k1nz/lanpaste";
export const GITHUB_RELEASES = "https://github.com/k1nz/lanpaste/releases";
export const GITHUB_LATEST_API =
  "https://api.github.com/repos/k1nz/lanpaste/releases/latest";
export const GITHUB_RELEASES_API =
  "https://api.github.com/repos/k1nz/lanpaste/releases?per_page=1";

/**
 * Version manifest served from GitHub's raw CDN. Unlike api.github.com this host
 * is not rate-limited and sends `Access-Control-Allow-Origin: *`, so the site can
 * discover the current release version without the releases API.
 */
export const VERSION_MANIFEST_URL =
  "https://raw.githubusercontent.com/k1nz/lanpaste/main/package.json";

export type PlatformId = "mac-arm" | "mac-intel" | "windows";

/**
 * GitHub permalink that always serves the newest published release. The asset
 * filename still embeds the version, so callers pair this with a version found
 * at runtime (`latestAssetUrl`) instead of a version frozen in this file.
 */
export const LATEST_DOWNLOAD_BASE = `${GITHUB_REPO}/releases/latest/download`;

const ASSET_FILENAMES: Record<PlatformId, (version: string) => string> = {
  "mac-arm": (version) => `LanPaste_${version}_aarch64.dmg`,
  "mac-intel": (version) => `LanPaste_${version}_x64.dmg`,
  windows: (version) => `LanPaste_${version}_x64-setup.exe`,
};

/** Build a download URL that resolves to the newest release for `platform`. */
export function latestAssetUrl(platform: PlatformId, version: string): string {
  return `${LATEST_DOWNLOAD_BASE}/${ASSET_FILENAMES[platform](version)}`;
}

/**
 * Last-resort assets, used only when neither the releases API nor the version
 * manifest can be reached — i.e. the visitor cannot talk to GitHub's API hosts
 * at all. These are pinned to a release *tag* rather than to `latest`, because
 * `<tag>/<filename>` is permanent while `<latest>/<filename>` 404s as soon as a
 * newer release ships under a different filename. Degrading to a known older
 * build beats handing the visitor a dead link.
 */
const FALLBACK_VERSION = "0.1.0";

export const FALLBACK_ASSETS: Record<PlatformId, string> = {
  "mac-arm": `${GITHUB_REPO}/releases/download/v${FALLBACK_VERSION}/${ASSET_FILENAMES["mac-arm"](FALLBACK_VERSION)}`,
  "mac-intel": `${GITHUB_REPO}/releases/download/v${FALLBACK_VERSION}/${ASSET_FILENAMES["mac-intel"](FALLBACK_VERSION)}`,
  windows: `${GITHUB_REPO}/releases/download/v${FALLBACK_VERSION}/${ASSET_FILENAMES.windows(FALLBACK_VERSION)}`,
};

/** Matches `src-tauri/tauri.conf.json` overlay window. */
export const OVERLAY_WIDTH = 780;
export const OVERLAY_HEIGHT = 520;
