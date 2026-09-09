<script setup lang="ts">
import { PhDotsThree, PhPlus, PhWarning } from "@phosphor-icons/vue";
import { computed, onMounted, onUnmounted, ref } from "vue";
import type { NearbyDevice, PairedDevice } from "../../shared/types";
import { t } from "../../shared/i18n";
import { localizeError } from "../../shared/errors";
import { truncateFingerprint } from "../../shared/format";
import {
  listNearby,
  listPaired,
  removeDevice,
  startPair,
  updateDeviceFlags,
  updateDeviceNote,
} from "../../shared/ipc";
import { onDevicesChanged } from "../../shared/events";
import ToggleSwitch from "../components/ToggleSwitch.vue";

const nearby = ref<NearbyDevice[]>([]);
const paired = ref<PairedDevice[]>([]);
const error = ref("");
const menuFor = ref<string | null>(null);
const editingId = ref<string | null>(null);
const noteDraft = ref("");
const unlisteners: Array<() => void> = [];

const unpairedNearby = computed(() => {
  const ids = new Set(paired.value.map((d) => d.instanceId));
  return nearby.value.filter((d) => !ids.has(d.instanceId));
});

async function refresh() {
  const [n, p] = await Promise.all([listNearby(), listPaired()]);
  nearby.value = n;
  paired.value = p;
}

async function addDevice(instanceId: string) {
  error.value = "";
  const r = await startPair(instanceId);
  if (!r.ok) error.value = localizeError(r.error);
}

async function setFlags(dev: PairedDevice, patch: Partial<PairedDevice>) {
  const next = { ...dev, ...patch };
  const r = await updateDeviceFlags(dev.instanceId, {
    allowSend: next.allowSend,
    allowReceive: next.allowReceive,
    autoWriteClipboard: next.autoWriteClipboard,
  });
  if (!r.ok) error.value = localizeError(r.error);
  else await refresh();
}

function startEdit(dev: PairedDevice) {
  editingId.value = dev.instanceId;
  noteDraft.value = dev.note;
  menuFor.value = null;
}

async function saveNote(instanceId: string) {
  const r = await updateDeviceNote(instanceId, noteDraft.value.trim());
  editingId.value = null;
  if (!r.ok) error.value = localizeError(r.error);
  else await refresh();
}

async function remove(instanceId: string) {
  menuFor.value = null;
  const r = await removeDevice(instanceId);
  if (!r.ok) error.value = localizeError(r.error);
  else await refresh();
}

function onDoc(ev: MouseEvent) {
  if (!(ev.target instanceof Element) || !ev.target.closest(".overflow")) {
    menuFor.value = null;
  }
}

onMounted(async () => {
  document.addEventListener("mousedown", onDoc);
  unlisteners.push(await onDevicesChanged(() => void refresh()));
  await refresh();
});

onUnmounted(() => {
  document.removeEventListener("mousedown", onDoc);
  for (const u of unlisteners) u();
});
</script>

<template>
  <section class="pane">
    <p v-if="error" class="live-error" role="alert">{{ error }}</p>

    <h2>{{ t("settings.devices.nearby") }}</h2>
    <p v-if="unpairedNearby.length === 0" class="empty">{{ t("settings.devices.emptyNearby") }}</p>
    <ul v-else class="device-list">
      <li v-for="dev in unpairedNearby" :key="dev.instanceId" class="device-row">
        <div class="who">
          <strong>{{ dev.name }}</strong>
          <code>{{ truncateFingerprint(dev.fingerprint) }}</code>
        </div>
        <button type="button" class="btn-primary add" @click="addDevice(dev.instanceId)">
          <PhPlus :size="14" weight="regular" />
          {{ t("settings.devices.add") }}
        </button>
      </li>
    </ul>

    <h2>{{ t("settings.devices.mine") }}</h2>
    <p v-if="paired.length === 0" class="empty">{{ t("settings.devices.emptyPaired") }}</p>
    <ul v-else class="device-list paired">
      <li v-for="dev in paired" :key="dev.instanceId" class="device-card">
        <header class="card-head">
          <span
            class="dot"
            :class="{ on: dev.online }"
            :title="dev.online ? t('settings.devices.online') : t('settings.devices.offline')"
          />
          <div class="who">
            <strong>{{ dev.note || dev.name }}</strong>
            <span class="sub">
              {{ dev.note ? dev.name : "" }}
              {{ dev.online ? t("settings.devices.online") : t("settings.devices.offline") }}
            </span>
          </div>
          <div class="overflow">
            <button
              type="button"
              class="icon-btn"
              :aria-label="t('settings.devices.editNote')"
              @click="menuFor = menuFor === dev.instanceId ? null : dev.instanceId"
            >
              <PhDotsThree :size="20" weight="regular" />
            </button>
            <div v-if="menuFor === dev.instanceId" class="overflow-menu" role="menu">
              <button type="button" role="menuitem" @click="startEdit(dev)">
                {{ t("settings.devices.editNote") }}
              </button>
              <button
                type="button"
                role="menuitem"
                class="btn-destructive"
                @click="remove(dev.instanceId)"
              >
                {{ t("settings.devices.remove") }}
              </button>
            </div>
          </div>
        </header>

        <p v-if="dev.trustBroken" class="trust" role="status">
          <PhWarning :size="16" weight="regular" />
          {{ t("settings.devices.trustBroken") }}
        </p>

        <form
          v-if="editingId === dev.instanceId"
          class="note-form"
          @submit.prevent="saveNote(dev.instanceId)"
        >
          <input
            v-model="noteDraft"
            :placeholder="t('settings.devices.note')"
            @keydown.escape.prevent="editingId = null"
          />
          <button type="submit" class="btn-primary">{{ t("settings.devices.saveNote") }}</button>
          <button type="button" class="btn-ghost" @click="editingId = null">
            {{ t("settings.devices.cancel") }}
          </button>
        </form>

        <div class="flags">
          <ToggleSwitch
            :model-value="dev.allowSend"
            :label="t('settings.devices.allowSend')"
            @update:model-value="setFlags(dev, { allowSend: $event })"
          />
          <ToggleSwitch
            :model-value="dev.allowReceive"
            :label="t('settings.devices.allowReceive')"
            @update:model-value="setFlags(dev, { allowReceive: $event })"
          />
          <ToggleSwitch
            :model-value="dev.autoWriteClipboard"
            :label="t('settings.devices.autoWrite')"
            @update:model-value="setFlags(dev, { autoWriteClipboard: $event })"
          />
        </div>
        <p class="flag-hint">{{ t("settings.devices.autoWriteHint") }}</p>
      </li>
    </ul>
  </section>
</template>

<style scoped>
.pane h2 {
  margin: 0 0 var(--space-md);
  font-size: 15px;
  font-weight: 600;
}

.pane h2:not(:first-child) {
  margin-top: var(--space-2xl);
}

.empty {
  margin: 0;
  color: var(--color-muted);
  font-size: 13px;
}

.device-list {
  list-style: none;
  margin: 0;
  padding: 0;
  display: flex;
  flex-direction: column;
  gap: var(--space-md);
}

.device-row,
.device-card {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-lg);
  padding: var(--space-lg);
  border: 1px solid var(--color-border);
  border-radius: 8px;
  background: var(--color-elevated);
}

.device-card {
  flex-direction: column;
  align-items: stretch;
}

.card-head {
  display: flex;
  align-items: center;
  gap: var(--space-md);
}

.who {
  display: flex;
  flex-direction: column;
  gap: 2px;
  min-width: 0;
  flex: 1;
}

.who strong {
  font-weight: 600;
  font-size: 13px;
}

.who code,
.sub {
  font-size: 11px;
  color: var(--color-muted);
  font-family: var(--font-mono);
}

.sub {
  font-family: var(--font-ui);
}

.add {
  display: inline-flex;
  align-items: center;
  gap: 4px;
}

.dot {
  width: 8px;
  height: 8px;
  border-radius: 99px;
  background: var(--color-muted);
  flex-shrink: 0;
}

.dot.on {
  background: var(--color-success);
}

.icon-btn {
  width: 28px;
  height: 28px;
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: 6px;
  cursor: pointer;
}

.icon-btn:hover {
  background: var(--color-elevated);
}

.overflow {
  position: relative;
}

.overflow-menu {
  position: absolute;
  right: 0;
  top: 100%;
  z-index: 5;
  min-width: 140px;
  padding: 4px;
  background: var(--color-surface-solid);
  border: 1px solid var(--color-border);
  border-radius: 8px;
  box-shadow: var(--shadow-window);
}

.overflow-menu button {
  display: block;
  width: 100%;
  text-align: left;
  padding: 6px 10px;
  border-radius: 6px;
  cursor: pointer;
}

.overflow-menu button:hover {
  background: var(--color-elevated);
}

.trust {
  display: flex;
  align-items: center;
  gap: 6px;
  margin: 0;
  font-size: 12px;
  color: var(--color-destructive);
}

.note-form {
  display: flex;
  gap: var(--space-md);
}

.note-form input {
  flex: 1;
  height: 28px;
  padding: 0 8px;
  border-radius: 6px;
  border: 1px solid var(--color-border);
  background: var(--color-background);
}

.flags {
  display: flex;
  flex-direction: column;
  gap: var(--space-md);
}

.flag-hint {
  margin: var(--space-sm) 0 0;
  font-size: 12px;
  color: var(--color-muted);
}
</style>
