<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from "vue";
import type { SessionShowPayload } from "../shared/types";
import { t } from "../shared/i18n";
import { toMs } from "../shared/format";
import { onSessionShow, onSessionShowHide } from "../shared/events";

const payload = ref<SessionShowPayload | null>(null);
const now = ref(Date.now());
let tick: number | null = null;
const unlisteners: Array<() => void> = [];

const items = computed(() => payload.value?.items ?? []);

function remaining(expiresAt: number) {
  return Math.max(0, Math.ceil((toMs(expiresAt) - now.value) / 1000));
}

function formatCode(code: string) {
  const digits = code.replace(/\D/g, "");
  if (digits.length !== 8) return code;
  return `${digits.slice(0, 4)} ${digits.slice(4)}`;
}

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
    await onSessionShow((next) => {
      payload.value = next;
      now.value = Date.now();
      startTick();
    }),
    await onSessionShowHide(() => {
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
  <div class="session-shell glass-surface">
    <p class="label">{{ t("session.show.title") }}</p>
    <p class="hint">{{ t("session.show.hint") }}</p>
    <p v-if="items.length === 0" class="waiting">{{ t("session.waiting") }}</p>
    <ul v-else class="codes">
      <li v-for="item in items" :key="item.instanceId" class="code-row">
        <p class="name">{{ item.deviceName }}</p>
        <p class="token" :class="{ expired: remaining(item.expiresAt) <= 0 }">
          {{ formatCode(item.code) }}
        </p>
        <p class="count">
          {{
            remaining(item.expiresAt) <= 0
              ? t("session.show.expired")
              : t("session.show.countdown", { seconds: remaining(item.expiresAt) })
          }}
        </p>
      </li>
    </ul>
  </div>
</template>

<style scoped>
.session-shell {
  width: 100%;
  height: 100%;
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: var(--space-md);
  padding: var(--space-xl);
  box-sizing: border-box;
}

.label {
  margin: 0;
  font-size: 15px;
  font-weight: 600;
}

.hint,
.waiting,
.count {
  margin: 0;
  font-size: 12px;
  color: var(--color-muted);
  text-align: center;
}

.codes {
  list-style: none;
  margin: 0;
  padding: 0;
  width: 100%;
  overflow: auto;
  display: flex;
  flex-direction: column;
  gap: var(--space-lg);
}

.code-row {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: var(--space-xs);
}

.name {
  margin: 0;
  font-size: 13px;
  font-weight: 600;
}

.token {
  margin: 0;
  font-family: var(--font-mono);
  font-size: 28px;
  font-variant-numeric: tabular-nums;
  letter-spacing: 0.14em;
  user-select: text;
}

.token.expired {
  opacity: 0.45;
}
</style>
