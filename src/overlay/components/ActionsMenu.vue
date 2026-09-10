<script setup lang="ts">
import { PhCaretRight, PhCheck } from "@phosphor-icons/vue";
import { computed, nextTick, onMounted, ref, watch } from "vue";
import type { HistoryEntry, PairedDevice } from "../../shared/types";
import Keycap from "../../shared/Keycap.vue";
import { t } from "../../shared/i18n";
import { buildActionItems, type ActionId, type MenuItem } from "../actions";

const props = defineProps<{
  open: boolean;
  mode: "actions" | "context";
  x: number;
  y: number;
  entry: HistoryEntry | null;
  devices: PairedDevice[];
  activeIndex: number;
  submenuOpen: boolean;
  submenuIndex: number;
  confirmDelete: boolean;
}>();

const emit = defineEmits<{
  close: [];
  run: [id: ActionId];
  "update:activeIndex": [n: number];
  "update:submenuOpen": [open: boolean];
  "update:submenuIndex": [n: number];
  syncDevice: [deviceId: string];
}>();

const root = ref<HTMLElement | null>(null);

const items = computed<MenuItem[]>(() => buildActionItems(props.entry, props.devices));

const style = computed(() => {
  if (props.mode === "actions") {
    return { right: "12px", bottom: "44px" };
  }
  const w = 240;
  const h = 280;
  let left = props.x;
  let top = props.y;
  if (left + w > window.innerWidth) left = window.innerWidth - w - 8;
  if (top + h > window.innerHeight) top = window.innerHeight - h - 8;
  return { left: `${Math.max(8, left)}px`, top: `${Math.max(8, top)}px` };
});

watch(
  () => items.value.length,
  (len) => {
    if (props.activeIndex >= len) emit("update:activeIndex", Math.max(0, len - 1));
  },
);

watch(
  () => props.devices.length,
  (len) => {
    if (!props.submenuOpen) return;
    if (len === 0) {
      emit("update:submenuOpen", false);
    } else if (props.submenuIndex >= len) {
      emit("update:submenuIndex", len - 1);
    }
  },
);

onMounted(async () => {
  await nextTick();
  root.value?.focus();
});

function onItemClick(item: MenuItem, index: number) {
  if (item.disabled) return;
  emit("update:activeIndex", index);
  if (item.submenu) {
    emit("update:submenuOpen", true);
    emit("update:submenuIndex", 0);
    return;
  }
  emit("run", item.id);
}
</script>

<template>
  <div
    v-if="open && entry"
    ref="root"
    class="menu"
    :class="mode"
    :style="style"
    role="menu"
    tabindex="-1"
  >
    <template v-if="confirmDelete">
      <p class="confirm" role="none">{{ t("overlay.deleteConfirm") }}</p>
      <button
        type="button"
        class="menu-item destructive"
        role="menuitem"
        @click="emit('run', 'delete')"
      >
        <span>{{ t("overlay.delete") }}</span>
        <Keycap k="enter" decorative />
      </button>
      <button type="button" class="menu-item" role="menuitem" @click="emit('close')">
        <span>{{ t("overlay.cancel") }}</span>
      </button>
    </template>
    <template v-else-if="!submenuOpen">
      <button
        v-for="(item, i) in items"
        :key="item.id"
        type="button"
        class="menu-item"
        role="menuitem"
        :class="{ active: i === activeIndex, destructive: item.destructive }"
        :disabled="item.disabled"
        @mouseenter="emit('update:activeIndex', i)"
        @click="onItemClick(item, i)"
      >
        <span>{{ item.label }}</span>
        <PhCaretRight v-if="item.submenu" :size="12" weight="regular" />
        <Keycap v-else-if="item.id === 'paste'" k="enter" decorative />
      </button>
    </template>
    <template v-else>
      <p v-if="devices.length === 0" class="empty">{{ t("overlay.noDevices") }}</p>
      <button
        v-for="(dev, i) in devices"
        :key="dev.instanceId"
        type="button"
        class="menu-item"
        role="menuitem"
        :class="{ active: i === submenuIndex }"
        @mouseenter="emit('update:submenuIndex', i)"
        @click="emit('syncDevice', dev.instanceId)"
      >
        <span class="dev-name">
          <span class="dot" :class="{ on: dev.online }" />
          {{ dev.note || dev.name }}
        </span>
        <span v-if="!dev.online" class="hint">{{ t("overlay.offline") }}</span>
        <PhCheck v-else-if="dev.online" :size="12" weight="regular" style="opacity: 0" />
      </button>
    </template>
  </div>
</template>

<style scoped>
.menu {
  position: absolute;
  z-index: 30;
  min-width: 220px;
  max-height: 280px;
  overflow: auto;
  padding: 4px;
  background: var(--color-surface-solid);
  border: 1px solid var(--color-border);
  border-radius: 8px;
  box-shadow: var(--shadow-window);
}

.menu.actions {
  right: 12px;
  bottom: 44px;
}

.menu-item {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: 12px;
  width: 100%;
  height: 32px;
  padding: 0 10px;
  border-radius: 6px;
  font-size: 13px;
  text-align: left;
  cursor: pointer;
}

.menu-item:hover:not(:disabled),
.menu-item.active:not(:disabled) {
  background: var(--color-accent);
  color: var(--color-on-accent);
}

.menu-item:disabled {
  opacity: 0.4;
  cursor: not-allowed;
}

.menu-item.destructive:not(.active):not(:hover) {
  color: var(--color-destructive);
}

.hint {
  font-size: 11px;
  color: var(--color-muted);
}

.menu-item.active .hint,
.menu-item:hover:not(:disabled) .hint {
  color: rgba(255, 255, 255, 0.8);
}

.menu-item.active :deep(.keycap),
.menu-item:hover:not(:disabled) :deep(.keycap) {
  background: rgb(255 255 255 / 0.22);
  border-color: rgb(255 255 255 / 0.28);
  color: #fff;
  box-shadow: none;
}

.empty {
  margin: 0;
  padding: 10px;
  font-size: 12px;
  color: var(--color-muted);
}

.confirm {
  margin: 0;
  padding: 8px 10px 6px;
  font-size: 12px;
  color: var(--color-foreground);
}

.dev-name {
  display: flex;
  align-items: center;
  gap: 8px;
}

.dot {
  width: 6px;
  height: 6px;
  border-radius: 99px;
  background: var(--color-muted);
}

.dot.on {
  background: var(--color-success);
}
</style>
