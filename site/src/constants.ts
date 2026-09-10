export const GITHUB_REPO = "https://github.com/k1nz/lanpaste";
export const GITHUB_RELEASES = "https://github.com/k1nz/lanpaste/releases";
export const GITHUB_LATEST_API =
  "https://api.github.com/repos/k1nz/lanpaste/releases/latest";
export const GITHUB_RELEASES_API =
  "https://api.github.com/repos/k1nz/lanpaste/releases?per_page=1";

export const FALLBACK_ASSETS = {
  "mac-arm":
    "https://github.com/k1nz/lanpaste/releases/download/v0.1.0/LanPaste_0.1.0_aarch64.dmg",
  "mac-intel":
    "https://github.com/k1nz/lanpaste/releases/download/v0.1.0/LanPaste_0.1.0_x64.dmg",
  windows:
    "https://github.com/k1nz/lanpaste/releases/download/v0.1.0/LanPaste_0.1.0_x64-setup.exe",
} as const;

export type PlatformId = keyof typeof FALLBACK_ASSETS;
