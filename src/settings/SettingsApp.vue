<script setup lang="ts">
import { PhDesktop, PhGear, PhHardDrives, PhInfo, PhKeyboard } from "@phosphor-icons/vue";
import { computed, ref, type Component } from "vue";
import { t, type MsgKey } from "../shared/i18n";
import GeneralPane from "./panes/GeneralPane.vue";
import DevicesPane from "./panes/DevicesPane.vue";
import StoragePane from "./panes/StoragePane.vue";
import ShortcutsPane from "./panes/ShortcutsPane.vue";
import AboutPane from "./panes/AboutPane.vue";

type NavId = "general" | "devices" | "storage" | "shortcuts" | "about";

const nav: Array<{ id: NavId; label: MsgKey; icon: Component }> = [
  { id: "general", label: "settings.nav.general", icon: PhGear },
  { id: "devices", label: "settings.nav.devices", icon: PhDesktop },
  { id: "storage", label: "settings.nav.storage", icon: PhHardDrives },
  { id: "shortcuts", label: "settings.nav.shortcuts", icon: PhKeyboard },
  { id: "about", label: "settings.nav.about", icon: PhInfo },
];

const active = ref<NavId>("general");

const pane = computed(() => {
  switch (active.value) {
    case "devices":
      return DevicesPane;
    case "storage":
      return StoragePane;
    case "shortcuts":
      return ShortcutsPane;
    case "about":
      return AboutPane;
    default:
      return GeneralPane;
  }
});
</script>

<template>
  <div class="settings-root">
    <nav class="nav" :aria-label="t('settings.title')">
      <button
        v-for="item in nav"
        :key="item.id"
        type="button"
        class="nav-item"
        :class="{ active: active === item.id }"
        @click="active = item.id"
      >
        <component :is="item.icon" :size="20" weight="regular" />
        <span>{{ t(item.label) }}</span>
      </button>
    </nav>
    <main class="detail">
      <component :is="pane" @go-devices="active = 'devices'" />
    </main>
  </div>
</template>

<style scoped>
.settings-root {
  display: flex;
  width: 100%;
  height: 100%;
  background: var(--color-background);
  color: var(--color-foreground);
}

.nav {
  width: 180px;
  flex-shrink: 0;
  padding: var(--space-xl) var(--space-md);
  border-right: 1px solid var(--color-border);
  display: flex;
  flex-direction: column;
  gap: 2px;
  background: var(--color-surface-solid);
}

.nav-item {
  display: flex;
  align-items: center;
  gap: var(--space-md);
  height: 32px;
  padding: 0 10px;
  border-radius: var(--radius-row);
  font-size: 13px;
  text-align: left;
  color: var(--color-foreground);
  cursor: pointer;
  transition: opacity 160ms ease;
}

.nav-item:hover {
  background: var(--color-elevated);
}

.nav-item.active {
  background: var(--color-accent);
  color: var(--color-on-accent);
}

.detail {
  flex: 1;
  min-width: 0;
  overflow: auto;
  padding: var(--space-3xl);
}
</style>
