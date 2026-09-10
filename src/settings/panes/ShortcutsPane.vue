<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from "vue";
import { t } from "../../shared/i18n";
import { localizeError } from "../../shared/errors";
import { eventToShortcut, formatShortcut, parseShortcut } from "../../shared/format";
import { DEFAULT_SETTINGS, getSettings, updateSettings } from "../../shared/ipc";
import Keycap from "../../shared/Keycap.vue";

const shortcut = ref(DEFAULT_SETTINGS.overlayShortcut);
const recording = ref(false);
const saving = ref(false);
const error = ref("");
const recorder = ref<HTMLButtonElement | null>(null);

const isDefault = computed(() => shortcut.value === DEFAULT_SETTINGS.overlayShortcut);
const shortcutParts = computed(() => parseShortcut(shortcut.value));

function stopRecording() {
  recording.value = false;
}

async function applyShortcut(next: string) {
  if (next === shortcut.value) {
    stopRecording();
    return;
  }
  stopRecording();
  saving.value = true;
  error.value = "";
  const r = await updateSettings({ overlayShortcut: next });
  saving.value = false;
  if (!r.ok) {
    error.value = localizeError(r.error);
    return;
  }
  shortcut.value = next;
}

function onRecordKey(ev: KeyboardEvent) {
  if (!recording.value || saving.value) return;
  ev.preventDefault();
  ev.stopPropagation();
  if (ev.key === "Escape") {
    stopRecording();
    return;
  }
  const next = eventToShortcut(ev);
  if (!next) return;
  void applyShortcut(next);
}

function onDocPointer(ev: PointerEvent) {
  if (!recording.value) return;
  const el = recorder.value;
  if (el && ev.target instanceof Node && el.contains(ev.target)) return;
  stopRecording();
}

function toggleRecording() {
  if (saving.value) return;
  error.value = "";
  recording.value = !recording.value;
}

async function reset() {
  if (saving.value) return;
  await applyShortcut(DEFAULT_SETTINGS.overlayShortcut);
}

onMounted(async () => {
  const s = await getSettings();
  shortcut.value = s.overlayShortcut || DEFAULT_SETTINGS.overlayShortcut;
  window.addEventListener("keydown", onRecordKey, true);
  window.addEventListener("pointerdown", onDocPointer, true);
  window.addEventListener("blur", stopRecording);
});

onUnmounted(() => {
  window.removeEventListener("keydown", onRecordKey, true);
  window.removeEventListener("pointerdown", onDocPointer, true);
  window.removeEventListener("blur", stopRecording);
});
</script>

<template>
  <section class="pane">
    <h1>{{ t("settings.shortcuts.title") }}</h1>
    <p v-if="error" class="live-error" role="alert">{{ error }}</p>
    <div class="row">
      <div class="copy">
        <span>{{ t("settings.shortcuts.overlay") }}</span>
        <small>{{ t("settings.shortcuts.hint") }}</small>
      </div>
      <div class="controls">
        <button
          v-if="!isDefault && !recording"
          type="button"
          class="reset"
          @click="reset"
        >
          {{ t("settings.shortcuts.reset") }}
        </button>
        <button
          ref="recorder"
          type="button"
          class="recorder"
          :class="{ recording }"
          :aria-label="t('settings.shortcuts.edit')"
          :aria-pressed="recording"
          :disabled="saving"
          @click="toggleRecording"
        >
          <span v-if="recording">{{ t("settings.shortcuts.recording") }}</span>
          <span v-else class="keys" :aria-label="formatShortcut(shortcut)">
            <Keycap v-for="part in shortcutParts" :key="part" :k="part" decorative />
          </span>
        </button>
      </div>
    </div>
  </section>
</template>

<style scoped>
.pane h1 {
  margin: 0 0 var(--space-xl);
  font-size: 15px;
  font-weight: 600;
}

.row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-xl);
  padding: var(--space-lg) 0;
  border-bottom: 1px solid var(--color-border);
  font-size: 13px;
}

.copy {
  display: flex;
  flex-direction: column;
  gap: var(--space-sm);
  min-width: 0;
}

.copy small {
  color: var(--color-muted);
  font-size: 12px;
}

.controls {
  display: flex;
  align-items: center;
  gap: var(--space-md);
  flex-shrink: 0;
}

.reset {
  color: var(--color-muted);
  font-size: 12px;
  padding: 4px 6px;
  border-radius: 6px;
}

.reset:hover {
  color: var(--color-foreground);
}

.recorder {
  font-family: var(--font-ui);
  font-size: 13px;
  min-width: 72px;
  min-height: 28px;
  padding: 4px 6px;
  border-radius: 6px;
  border: 1px solid var(--color-border);
  background: var(--color-elevated);
  color: var(--color-foreground);
  text-align: center;
}

.recorder.recording {
  border-color: var(--color-accent);
  color: var(--color-accent);
  box-shadow: 0 0 0 1px var(--color-accent);
}

.recorder:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.keys {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: 3px;
}
</style>
