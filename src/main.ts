import { createApp, type Component } from "vue";
import OverlayApp from "./overlay/OverlayApp.vue";
import { applyLocalePref, isLocalePref, setResolvedLocale, type LocaleId } from "./shared/i18n";
import { onLocaleChanged } from "./shared/events";
import { getSettings } from "./shared/ipc";
import "./shared/styles.css";

const params = new URLSearchParams(window.location.search);
const windowName = params.get("window") ?? "overlay";

async function resolveRoot(): Promise<Component> {
  if (windowName === "settings") {
    return (await import("./settings/SettingsApp.vue")).default;
  }
  if (windowName === "pairing-show") {
    return (await import("./pairing/PairingShowApp.vue")).default;
  }
  if (windowName === "pairing-input") {
    return (await import("./pairing/PairingInputApp.vue")).default;
  }
  return OverlayApp;
}

async function boot() {
  const settings = await getSettings();
  applyLocalePref(isLocalePref(settings.locale) ? settings.locale : "system");
  await onLocaleChanged((resolved) => {
    if (resolved === "en-US" || resolved === "zh-CN") {
      setResolvedLocale(resolved as LocaleId);
    }
  });
  const Root = await resolveRoot();
  createApp(Root).mount("#app");
}

void boot();
