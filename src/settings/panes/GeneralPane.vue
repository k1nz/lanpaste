<script setup lang="ts">
import { onMounted, ref } from "vue";
import { applyLocalePref, t, type LocalePref } from "../../shared/i18n";
import { localizeError } from "../../shared/errors";
import { getSettings, updateSettings } from "../../shared/ipc";

const emit = defineEmits<{
  goDevices: [];
}>();

const locale = ref<LocalePref>("system");
const error = ref("");

const choices: Array<{ id: LocalePref; label: string; translate?: boolean }> = [
  { id: "system", label: "settings.general.languageSystem", translate: true },
  { id: "zh-CN", label: "简体中文" },
  { id: "en-US", label: "English" },
];

async function setLocale(next: LocalePref) {
  if (next === locale.value) return;
  locale.value = next;
  applyLocalePref(next);
  const r = await updateSettings({ locale: next });
  if (!r.ok) error.value = localizeError(r.error);
  else error.value = "";
}

onMounted(async () => {
  const s = await getSettings();
  locale.value = s.locale;
  applyLocalePref(s.locale);
});
</script>

<template>
  <section class="pane">
    <h1>{{ t("settings.general.title") }}</h1>
    <p>{{ t("settings.general.body") }}</p>
    <p v-if="error" class="live-error" role="alert">{{ error }}</p>
    <div class="field">
      <span class="label">{{ t("settings.general.language") }}</span>
      <div class="seg" role="radiogroup" :aria-label="t('settings.general.language')">
        <button
          v-for="opt in choices"
          :key="opt.id"
          type="button"
          role="radio"
          class="seg-btn"
          :class="{ active: locale === opt.id }"
          :aria-checked="locale === opt.id"
          @click="setLocale(opt.id)"
        >
          {{ opt.translate ? t("settings.general.languageSystem") : opt.label }}
        </button>
      </div>
    </div>
    <button type="button" class="btn-ghost" @click="emit('goDevices')">
      {{ t("settings.general.goDevices") }}
    </button>
  </section>
</template>

<style scoped>
.pane h1 {
  margin: 0 0 var(--space-lg);
  font-size: 15px;
  font-weight: 600;
}

.pane p {
  margin: 0 0 var(--space-xl);
  color: var(--color-muted);
  max-width: 42em;
}

.field {
  display: flex;
  flex-direction: column;
  gap: var(--space-sm);
  margin-bottom: var(--space-xl);
}

.label {
  font-size: 13px;
}

.seg {
  display: flex;
  width: fit-content;
  padding: 2px;
  border-radius: 8px;
  border: 1px solid var(--color-border);
  background: var(--color-elevated);
}

.seg-btn {
  height: 28px;
  padding: 0 10px;
  border-radius: 6px;
  font-size: 12px;
  color: var(--color-muted);
  cursor: pointer;
}

.seg-btn:hover {
  color: var(--color-foreground);
}

.seg-btn.active {
  background: var(--color-accent);
  color: var(--color-on-accent);
}
</style>
