<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from "vue";
import type { PairingShowPayload } from "../shared/types";
import { t } from "../shared/i18n";
import { toMs } from "../shared/format";
import { onPairingHide, onPairingShow } from "../shared/events";

const payload = ref<PairingShowPayload | null>(null);
const now = ref(Date.now());
let tick: number | null = null;
const unlisteners: Array<() => void> = [];

const remaining = computed(() => {
  if (!payload.value) return 0;
  return Math.max(0, Math.ceil((toMs(payload.value.expiresAt) - now.value) / 1000));
});
const expired = computed(() => Boolean(payload.value) && remaining.value <= 0);

function startTick() {
  if (tick != null) return;
  tick = window.setInterval(() => {
    now.value = Date.now();
  }, 250);
}

function stopTick() {
  if (tick != null) {
    window.clearInterval(tick);
    tick = null;
  }
}

onMounted(async () => {
  unlisteners.push(
    await onPairingShow((p) => {
      payload.value = p;
      now.value = Date.now();
      startTick();
    }),
    await onPairingHide(() => {
      payload.value = null;
      stopTick();
    }),
  );
});

onUnmounted(() => {
  stopTick();
  for (const u of unlisteners) u();
});
</script>

<template>
  <div class="pair-shell glass-surface">
    <p class="label">{{ t("pairing.show.title") }}</p>
    <p v-if="payload" class="token" :class="{ expired }">{{ payload.token }}</p>
    <p v-else class="waiting">{{ t("pairing.waiting") }}</p>
    <p class="hint">{{ t("pairing.show.hint") }}</p>
    <p v-if="payload" class="count">
      {{ expired ? t("pairing.show.expired") : t("pairing.show.countdown", { seconds: remaining }) }}
    </p>
  </div>
</template>

<style scoped>
.pair-shell {
  width: 100%;
  height: 100%;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: var(--space-md);
  user-select: none;
}

.label,
.hint,
.count,
.waiting {
  margin: 0;
  font-size: 11px;
  color: var(--color-muted);
}

.token {
  margin: var(--space-sm) 0;
  font-family: var(--font-mono);
  font-size: 32px;
  font-variant-numeric: tabular-nums;
  letter-spacing: 0.2em;
  color: var(--color-foreground);
  padding-left: 0.2em;
}

.token.expired {
  opacity: 0.45;
}
</style>
