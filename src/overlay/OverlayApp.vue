<script setup lang="ts">
import { isTauri } from "@tauri-apps/api/core";
import { computed, nextTick, onMounted, onUnmounted, ref, watch } from "vue";
import type { HistoryEntry, PairedDevice, PasteType, TransferProgressPayload } from "../shared/types";
import { t } from "../shared/i18n";
import { localizeError } from "../shared/errors";
import { prefersReducedMotion } from "../shared/format";
import {
  copyEntryToClipboard,
  copyText,
  deleteEntry,
  frontmostAppName,
  getEntry,
  hideOverlay,
  listHistory,
  listPaired,
  pasteEntry,
  revealInFinder,
  showSettings,
  syncTo,
} from "../shared/ipc";
import {
  onHistoryChanged,
  onOverlayShown,
  onTransferProgress,
  onDevicesChanged,
} from "../shared/events";
import TypeFilter from "./components/TypeFilter.vue";
import HistoryList from "./components/HistoryList.vue";
import PreviewPane from "./components/PreviewPane.vue";
import ActionBar from "./components/ActionBar.vue";
import ActionsMenu from "./components/ActionsMenu.vue";
import { buildActionItems, type ActionId } from "./actions";
import { PhMagnifyingGlass } from "@phosphor-icons/vue";

const shown = ref(!isTauri());
const query = ref("");
const typeFilter = ref<PasteType | "all">("all");
const filterOpen = ref(false);
const items = ref<HistoryEntry[]>([]);
const selectedId = ref<string | null>(null);
const detail = ref<HistoryEntry | null>(null);
const frontmost = ref("");
const error = ref("");
const progress = ref<TransferProgressPayload | null>(null);
const devices = ref<PairedDevice[]>([]);
const searchEl = ref<HTMLInputElement | null>(null);

const menuOpen = ref(false);
const menuMode = ref<"actions" | "context">("actions");
const menuX = ref(0);
const menuY = ref(0);
const activeIndex = ref(0);
const submenuOpen = ref(false);
const submenuIndex = ref(0);

let persistSelected: string | null = null;
let queryTimer: number | null = null;
const unlisteners: Array<() => void> = [];

const selected = computed(
  () => items.value.find((it) => it.id === selectedId.value) ?? null,
);
const previewEntry = computed(() => {
  if (detail.value && detail.value.id === selectedId.value) return detail.value;
  return selected.value;
});

watch(query, () => {
  if (queryTimer != null) window.clearTimeout(queryTimer);
  queryTimer = window.setTimeout(() => {
    void refreshList();
  }, 80);
});

watch(typeFilter, () => {
  void refreshList();
});

watch(selectedId, async (id) => {
  persistSelected = id;
  detail.value = null;
  if (!id) return;
  const full = await getEntry(id);
  if (full && full.id === persistSelected) detail.value = full;
});

function closeMenu() {
  menuOpen.value = false;
  submenuOpen.value = false;
  activeIndex.value = 0;
  submenuIndex.value = 0;
}

function openActions(mode: "actions" | "context", x = 0, y = 0) {
  if (!selected.value) return;
  menuMode.value = mode;
  menuX.value = x;
  menuY.value = y;
  menuOpen.value = true;
  submenuOpen.value = false;
  activeIndex.value = 0;
  submenuIndex.value = 0;
}

async function refreshList(restoreId?: string | null) {
  try {
    const list = await listHistory(query.value, typeFilter.value);
    items.value = list;
    const want = restoreId ?? persistSelected ?? selectedId.value;
    if (want && list.some((it) => it.id === want)) {
      selectedId.value = want;
    } else {
      selectedId.value = list[0]?.id ?? null;
    }
  } catch (err) {
    items.value = [];
    selectedId.value = null;
    error.value = localizeError(err);
  }
}

async function refreshChrome() {
  frontmost.value = await frontmostAppName();
  devices.value = await listPaired();
}

async function onShown() {
  shown.value = false;
  error.value = "";
  progress.value = null;
  closeMenu();
  await refreshChrome();
  await refreshList(persistSelected);
  await nextTick();
  requestAnimationFrame(() => {
    shown.value = true;
    searchEl.value?.focus();
    searchEl.value?.select();
  });
}

async function animateHide() {
  shown.value = false;
  const ms = prefersReducedMotion() ? 120 : 160;
  await new Promise((r) => setTimeout(r, ms));
  await hideOverlay();
}

async function doPaste() {
  const id = selectedId.value;
  if (!id) return;
  error.value = "";
  const result = await pasteEntry(id);
  if (result.ok) {
    await animateHide();
    return;
  }
  error.value = localizeError(result.error);
}

function moveSelection(delta: number) {
  if (items.value.length === 0) return;
  const idx = items.value.findIndex((it) => it.id === selectedId.value);
  const next = Math.min(items.value.length - 1, Math.max(0, (idx < 0 ? 0 : idx) + delta));
  selectedId.value = items.value[next].id;
}

async function runAction(id: ActionId) {
  const entry = selected.value;
  if (!entry) return;
  closeMenu();
  error.value = "";
  if (id === "paste") {
    await doPaste();
    return;
  }
  if (id === "copy") {
    const r = await copyEntryToClipboard(entry.id);
    if (!r.ok) error.value = localizeError(r.error);
    return;
  }
  if (id === "reveal") {
    const r = await revealInFinder(entry.id);
    if (!r.ok) error.value = localizeError(r.error);
    return;
  }
  if (id === "delete") {
    const r = await deleteEntry(entry.id);
    if (!r.ok) error.value = localizeError(r.error);
    else await refreshList();
    return;
  }
  if (id === "copyColor" && entry.preview.color) {
    const ok = await copyText(entry.preview.color);
    if (!ok) {
      const r = await copyEntryToClipboard(entry.id);
      if (!r.ok) error.value = localizeError(r.error);
    }
    return;
  }
  if (id === "copyUrl" && entry.preview.url) {
    const ok = await copyText(entry.preview.url);
    if (!ok) {
      const r = await copyEntryToClipboard(entry.id);
      if (!r.ok) error.value = localizeError(r.error);
    }
  }
}

async function syncDevice(deviceId: string) {
  const entry = selected.value;
  closeMenu();
  if (!entry) return;
  const r = await syncTo(entry.id, deviceId);
  if (!r.ok) error.value = localizeError(r.error);
}

function clearFilters() {
  query.value = "";
  typeFilter.value = "all";
}

function onContext(payload: { id: string; x: number; y: number }) {
  selectedId.value = payload.id;
  openActions("context", payload.x, payload.y);
}

function onKey(ev: KeyboardEvent) {
  const meta = ev.metaKey || ev.ctrlKey;
  if (meta && ev.key.toLowerCase() === "k") {
    ev.preventDefault();
    if (menuOpen.value && menuMode.value === "actions") closeMenu();
    else openActions("actions");
    return;
  }
  if (meta && ev.key === ",") {
    ev.preventDefault();
    void showSettings();
    return;
  }
  if (ev.key === "Escape") {
    ev.preventDefault();
    if (filterOpen.value) {
      filterOpen.value = false;
      return;
    }
    if (submenuOpen.value) {
      submenuOpen.value = false;
      return;
    }
    if (menuOpen.value) {
      closeMenu();
      return;
    }
    void animateHide();
    return;
  }
  if (menuOpen.value) {
    if (ev.key === "ArrowDown") {
      ev.preventDefault();
      if (submenuOpen.value) {
        submenuIndex.value = Math.min(devices.value.length - 1, submenuIndex.value + 1);
      } else {
        activeIndex.value += 1;
      }
      return;
    }
    if (ev.key === "ArrowUp") {
      ev.preventDefault();
      if (submenuOpen.value) {
        submenuIndex.value = Math.max(0, submenuIndex.value - 1);
      } else {
        activeIndex.value = Math.max(0, activeIndex.value - 1);
      }
      return;
    }
    if (ev.key === "ArrowRight") {
      ev.preventDefault();
      const action = buildActionItems(selected.value, devices.value)[activeIndex.value];
      if (action?.submenu && !action.disabled) {
        submenuOpen.value = true;
        submenuIndex.value = 0;
      }
      return;
    }
    if (ev.key === "ArrowLeft") {
      ev.preventDefault();
      submenuOpen.value = false;
      return;
    }
    if (ev.key === "Enter") {
      ev.preventDefault();
      if (submenuOpen.value) {
        const dev = devices.value[submenuIndex.value];
        if (dev && dev.online && !dev.trustBroken) void syncDevice(dev.instanceId);
        return;
      }
      const action = buildActionItems(selected.value, devices.value)[activeIndex.value];
      if (!action || action.disabled) return;
      if (action.submenu) {
        submenuOpen.value = true;
        submenuIndex.value = 0;
        return;
      }
      void runAction(action.id);
      return;
    }
    return;
  }
  if (ev.key === "ArrowDown") {
    ev.preventDefault();
    moveSelection(1);
    return;
  }
  if (ev.key === "ArrowUp") {
    ev.preventDefault();
    moveSelection(-1);
    return;
  }
  if (ev.key === "Enter") {
    ev.preventDefault();
    void doPaste();
  }
}

function onDocMouse(ev: MouseEvent) {
  if (!menuOpen.value) return;
  const target = ev.target;
  if (target instanceof Element && target.closest(".menu")) return;
  closeMenu();
}

onMounted(async () => {
  window.addEventListener("keydown", onKey, true);
  window.addEventListener("mousedown", onDocMouse);
  unlisteners.push(
    await onOverlayShown(() => {
      void onShown();
    }),
    await onHistoryChanged(() => {
      void refreshList(selectedId.value);
    }),
    await onDevicesChanged(() => {
      void refreshChrome();
    }),
    await onTransferProgress((p) => {
      progress.value = p;
      items.value = items.value.map((it) =>
        it.id === p.id
          ? {
              ...it,
              fileDownloadState: p.received >= p.total ? "idle" : "downloading",
              needsFileDownload: p.received < p.total,
            }
          : it,
      );
    }),
  );
  await refreshChrome();
  await refreshList();
  shown.value = true;
  await nextTick();
  searchEl.value?.focus();
});

onUnmounted(() => {
  window.removeEventListener("keydown", onKey, true);
  window.removeEventListener("mousedown", onDocMouse);
  if (queryTimer != null) window.clearTimeout(queryTimer);
  for (const u of unlisteners) u();
});
</script>

<template>
  <div class="overlay-shell" :class="{ 'is-shown': shown }">
    <header class="search-bar">
      <PhMagnifyingGlass class="search-icon" :size="16" weight="regular" />
      <input
        ref="searchEl"
        v-model="query"
        class="search-input"
        type="search"
        :placeholder="t('overlay.searchPlaceholder')"
        autocomplete="off"
        spellcheck="false"
      />
      <TypeFilter v-model="typeFilter" v-model:open="filterOpen" />
    </header>

    <div class="split">
      <HistoryList
        :items="items"
        :selected-id="selectedId"
        :progress="progress"
        @select="selectedId = $event"
        @context="onContext"
        @activate="doPaste"
        @clear-filters="clearFilters"
      />
      <PreviewPane :entry="previewEntry" :progress="progress" />
    </div>

    <p v-if="error" class="banner" role="alert">{{ error }}</p>

    <ActionBar
      :frontmost="frontmost"
      :can-paste="Boolean(selectedId)"
      @paste="doPaste"
      @actions="openActions('actions')"
    />

    <ActionsMenu
      :open="menuOpen"
      :mode="menuMode"
      :x="menuX"
      :y="menuY"
      :entry="selected"
      :devices="devices"
      :active-index="activeIndex"
      :submenu-open="submenuOpen"
      :submenu-index="submenuIndex"
      @close="closeMenu"
      @run="runAction"
      @sync-device="syncDevice"
      @update:active-index="activeIndex = $event"
      @update:submenu-open="submenuOpen = $event"
      @update:submenu-index="submenuIndex = $event"
    />
  </div>
</template>

<style scoped>
.overlay-shell {
  position: relative;
  width: 100%;
  height: 100%;
  display: flex;
  flex-direction: column;
  overflow: hidden;
  background: var(--color-surface);
  backdrop-filter: blur(var(--blur)) saturate(140%);
  -webkit-backdrop-filter: blur(var(--blur)) saturate(140%);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-window);
  box-shadow: var(--shadow-window);
  opacity: 0;
  transform: translateY(6px);
  transition:
    opacity 150ms var(--ease-overlay),
    transform 150ms var(--ease-overlay);
}

.overlay-shell.is-shown {
  opacity: 1;
  transform: none;
}

.search-bar {
  display: flex;
  align-items: center;
  gap: var(--space-md);
  height: 44px;
  padding: 0 12px 0 14px;
  border-bottom: 1px solid var(--color-border);
  flex-shrink: 0;
}

.search-icon {
  color: var(--color-muted);
  flex-shrink: 0;
}

.search-input {
  flex: 1;
  min-width: 0;
  height: 32px;
  border: none;
  background: transparent;
  font-size: 15px;
  caret-color: var(--color-accent);
}

.search-input::placeholder {
  color: var(--color-muted);
}

.search-input:focus-visible {
  outline: none;
  box-shadow: inset 0 -2px 0 var(--color-ring);
}

.split {
  flex: 1;
  min-height: 0;
  display: grid;
  grid-template-columns: 1.15fr 0.85fr;
}

.banner {
  margin: 0;
  padding: 4px 12px;
  font-size: 11px;
  color: var(--color-destructive);
  border-top: 1px solid var(--color-border);
}

@media (prefers-reduced-motion: reduce) {
  .overlay-shell {
    transform: none;
    transition: opacity 150ms ease;
  }
}
</style>
