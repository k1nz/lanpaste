<script setup lang="ts">
import { onMounted, ref } from "vue";
import { t } from "../../shared/i18n";
import { formatShortcut } from "../../shared/format";
import { DEFAULT_SETTINGS, getSettings } from "../../shared/ipc";

const shortcut = ref(DEFAULT_SETTINGS.overlayShortcut);

onMounted(async () => {
  const s = await getSettings();
  shortcut.value = s.overlayShortcut || DEFAULT_SETTINGS.overlayShortcut;
});
</script>

<template>
  <section class="pane">
    <h1>{{ t("settings.shortcuts.title") }}</h1>
    <div class="row">
      <span>{{ t("settings.shortcuts.overlay") }}</span>
      <kbd>{{ formatShortcut(shortcut) }}</kbd>
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
  padding: var(--space-lg) 0;
  border-bottom: 1px solid var(--color-border);
  font-size: 13px;
}

kbd {
  font-family: var(--font-ui);
  font-size: 13px;
  padding: 4px 8px;
  border-radius: 6px;
  border: 1px solid var(--color-border);
  background: var(--color-elevated);
  color: var(--color-foreground);
}
</style>
