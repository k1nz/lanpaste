<script setup lang="ts">
import { nextTick, onMounted, reactive, ref, watch } from "vue";
import type { AppSettings } from "../../shared/types";
import { t } from "../../shared/i18n";
import { localizeError } from "../../shared/errors";
import { DEFAULT_SETTINGS, getSettings, updateSettings } from "../../shared/ipc";

const MB = 1024 * 1024;
const GB = 1024 * 1024 * 1024;

const form = reactive({
  autoSyncMb: DEFAULT_SETTINGS.autoSyncMaxBytes / MB,
  maxItems: DEFAULT_SETTINGS.cleanupMaxItems,
  maxGb: DEFAULT_SETTINGS.cleanupMaxBytes / GB,
  maxAge: "" as string,
});
const error = ref("");
let skip = true;
let timer: number | null = null;

function toPartial(): Partial<AppSettings> {
  const age = form.maxAge.trim();
  return {
    autoSyncMaxBytes: Math.max(1, Number(form.autoSyncMb) || 0) * MB,
    cleanupMaxItems: Math.max(1, Math.round(Number(form.maxItems) || 0)),
    cleanupMaxBytes: Math.max(1, Number(form.maxGb) || 0) * GB,
    cleanupMaxAgeDays: age === "" ? null : Math.max(1, Math.round(Number(age) || 0)),
  };
}

function scheduleSave() {
  if (skip) return;
  if (timer != null) window.clearTimeout(timer);
  timer = window.setTimeout(async () => {
    const r = await updateSettings(toPartial());
    if (!r.ok) error.value = localizeError(r.error);
    else error.value = "";
  }, 400);
}

watch(form, scheduleSave, { deep: true });

onMounted(async () => {
  const s = await getSettings();
  form.autoSyncMb = s.autoSyncMaxBytes / MB;
  form.maxItems = s.cleanupMaxItems;
  form.maxGb = s.cleanupMaxBytes / GB;
  form.maxAge = s.cleanupMaxAgeDays == null ? "" : String(s.cleanupMaxAgeDays);
  await nextTick();
  skip = false;
});
</script>

<template>
  <section class="pane">
    <h1>{{ t("settings.storage.title") }}</h1>
    <p v-if="error" class="live-error" role="alert">{{ error }}</p>

    <label class="field">
      <span>{{ t("settings.storage.autoSync") }}</span>
      <span class="control">
        <input v-model.number="form.autoSyncMb" type="number" min="1" max="1024" step="1" />
        <span class="unit">{{ t("settings.storage.mb") }}</span>
      </span>
      <small>{{ t("settings.storage.autoSyncHint") }}</small>
    </label>

    <label class="field">
      <span>{{ t("settings.storage.maxItems") }}</span>
      <span class="control">
        <input v-model.number="form.maxItems" type="number" min="50" max="10000" step="1" />
      </span>
    </label>

    <label class="field">
      <span>{{ t("settings.storage.maxBytes") }}</span>
      <span class="control">
        <input v-model.number="form.maxGb" type="number" min="0.1" max="10" step="0.1" />
        <span class="unit">{{ t("settings.storage.gb") }}</span>
      </span>
    </label>

    <label class="field">
      <span>{{ t("settings.storage.maxAge") }}</span>
      <span class="control">
        <input v-model="form.maxAge" type="number" min="1" max="3650" step="1" />
        <span class="unit">{{ t("settings.storage.days") }}</span>
      </span>
      <small>{{ t("settings.storage.maxAgeHint") }}</small>
    </label>
  </section>
</template>

<style scoped>
.pane h1 {
  margin: 0 0 var(--space-xl);
  font-size: 15px;
  font-weight: 600;
}

.field {
  display: flex;
  flex-direction: column;
  gap: var(--space-sm);
  margin-bottom: var(--space-xl);
  font-size: 13px;
}

.control {
  display: flex;
  align-items: center;
  gap: var(--space-md);
}

.control input {
  width: 120px;
  height: 28px;
  padding: 0 8px;
  border-radius: 6px;
  border: 1px solid var(--color-border);
  background: var(--color-elevated);
}

.unit,
small {
  color: var(--color-muted);
  font-size: 12px;
}
</style>
