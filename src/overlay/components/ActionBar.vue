<script setup lang="ts">
import { PhClipboard } from "@phosphor-icons/vue";
import { computed } from "vue";
import Keycap from "../../shared/Keycap.vue";
import { t } from "../../shared/i18n";
import { isApplePlatform } from "../../shared/ipc";

const props = defineProps<{
  frontmost: string;
  canPaste: boolean;
}>();

const emit = defineEmits<{
  paste: [];
  actions: [];
}>();

const pasteLabel = computed(() =>
  props.frontmost ? t("overlay.pasteTo", { app: props.frontmost }) : t("overlay.paste"),
);

const actionMod = isApplePlatform() ? "command" : "control";
</script>

<template>
  <footer class="bar">
    <div class="lead">
      <PhClipboard :size="20" weight="regular" />
      <span>{{ t("overlay.clipboardHistory") }}</span>
    </div>
    <button
      type="button"
      class="bar-action"
      :disabled="!canPaste"
      @click="emit('paste')"
    >
      <span>{{ pasteLabel }}</span>
      <Keycap k="enter" decorative />
    </button>
    <span class="bar-sep" aria-hidden="true" />
    <button type="button" class="bar-action" @click="emit('actions')">
      <span>{{ t("overlay.actions") }}</span>
      <span class="chord">
        <Keycap :k="actionMod" decorative />
        <Keycap k="K" decorative />
      </span>
    </button>
  </footer>
</template>

<style scoped>
.bar {
  position: relative;
  z-index: 2;
  display: flex;
  align-items: center;
  gap: var(--space-md);
  height: 36px;
  padding: 0 12px;
  border-top: 1px solid var(--color-border);
  flex-shrink: 0;
  user-select: none;
}

.lead {
  display: flex;
  align-items: center;
  gap: 6px;
  color: var(--color-muted);
  font-size: 12px;
  margin-right: auto;
}

.bar-action {
  display: flex;
  align-items: center;
  gap: 6px;
  height: 24px;
  padding: 0 6px;
  border-radius: 4px;
  font-size: 12px;
  color: var(--color-foreground);
  cursor: pointer;
  transition: opacity 160ms ease;
}

.bar-action:hover:not(:disabled) {
  background: var(--color-elevated);
}

.bar-action:disabled {
  opacity: 0.4;
  cursor: not-allowed;
}

.bar-sep {
  width: 1px;
  height: 12px;
  background: var(--color-border);
}

.chord {
  display: inline-flex;
  align-items: center;
  gap: 3px;
}
</style>
