import { createApp, type Component } from "vue";
import OverlayApp from "./overlay/OverlayApp.vue";
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

void resolveRoot().then((Root) => {
  createApp(Root).mount("#app");
});
