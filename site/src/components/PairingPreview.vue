<script setup lang="ts">
import { useI18n } from "vue-i18n";

const { t } = useI18n();
</script>

<template>
  <div
    class="flex min-h-[380px] items-center justify-center rounded-[var(--radius-window)] bg-[var(--canvas-2)] md:min-h-[480px]"
  >
    <div class="pair" role="region" :aria-label="t('setup.previewAria')">
      <p class="label">{{ t("setup.previewTitle") }}</p>
      <p class="token">{{ t("setup.previewCode") }}</p>
      <p class="hint">{{ t("setup.previewHint") }}</p>
      <p class="count">{{ t("setup.previewCount") }}</p>
    </div>
  </div>
</template>

<style scoped>
.pair {
  width: 360px;
  max-width: calc(100% - 2rem);
  height: 220px;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 8px;
  padding: 24px 20px;
  color: #ededef;
  background: rgb(28 28 30 / 0.82);
  border: 1px solid rgb(255 255 255 / 0.08);
  border-radius: 12px;
  box-shadow:
    inset 0 1px 0 rgb(255 255 255 / 0.12),
    0 18px 48px rgb(0 0 0 / 0.4);
  backdrop-filter: blur(20px) saturate(140%);
  -webkit-backdrop-filter: blur(20px) saturate(140%);
  font-family:
    -apple-system, BlinkMacSystemFont, "SF Pro Text", var(--font-sans), sans-serif;
  text-align: center;
  user-select: none;
}

@media (prefers-reduced-transparency: reduce) {
  .pair {
    background: #1c1c1e;
    backdrop-filter: none;
    -webkit-backdrop-filter: none;
  }
}

.label,
.hint,
.count {
  margin: 0;
  font-size: 11px;
  color: #8a8f98;
}

.token {
  margin: 4px 0;
  font-family: var(--font-mono);
  font-size: 32px;
  font-variant-numeric: tabular-nums;
  letter-spacing: 0.2em;
  color: #ededef;
  padding-left: 0.2em;
}
</style>
