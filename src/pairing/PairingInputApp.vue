<script setup lang="ts">
import { onMounted, onUnmounted, ref } from "vue";
import type { PairingInputPayload } from "../shared/types";
import { t } from "../shared/i18n";
import { localizeError } from "../shared/errors";
import { cancelPair, submitPairToken } from "../shared/ipc";
import { onPairingHide, onPairingInput } from "../shared/events";

const payload = ref<PairingInputPayload | null>(null);
const token = ref("");
const error = ref("");
const busy = ref(false);
const inputEl = ref<HTMLInputElement | null>(null);
const unlisteners: Array<() => void> = [];

function onTokenInput(ev: Event) {
  const el = ev.target as HTMLInputElement;
  token.value = el.value.replace(/\D/g, "").slice(0, 6);
}

async function submit() {
  if (!payload.value || token.value.length !== 6 || busy.value) return;
  busy.value = true;
  error.value = "";
  const r = await submitPairToken(payload.value.instanceId, token.value);
  busy.value = false;
  if (!r.ok) error.value = localizeError(r.error);
}

async function cancel() {
  await cancelPair();
}

onMounted(async () => {
  unlisteners.push(
    await onPairingInput((p) => {
      payload.value = p;
      token.value = "";
      error.value = "";
      requestAnimationFrame(() => inputEl.value?.focus());
    }),
    await onPairingHide(() => {
      payload.value = null;
      token.value = "";
    }),
  );
  requestAnimationFrame(() => inputEl.value?.focus());
});

onUnmounted(() => {
  for (const u of unlisteners) u();
});
</script>

<template>
  <div class="pair-shell">
    <p class="label">{{ t("pairing.input.title") }}</p>
    <p class="hint">
      {{ payload ? t("pairing.input.hint", { name: payload.deviceName }) : t("pairing.waiting") }}
    </p>
    <input
      ref="inputEl"
      class="token-input"
      :value="token"
      maxlength="6"
      inputmode="numeric"
      autocomplete="one-time-code"
      :placeholder="t('pairing.input.placeholder')"
      @input="onTokenInput"
      @keydown.enter.prevent="submit"
      @keydown.escape.prevent="cancel"
    />
    <p v-if="error" class="live-error" role="alert">{{ error }}</p>
    <div class="actions">
      <button type="button" class="btn-ghost" @click="cancel">
        {{ t("pairing.input.cancel") }}
      </button>
      <button
        type="button"
        class="btn-primary"
        :disabled="token.length !== 6 || busy || !payload"
        @click="submit"
      >
        {{ t("pairing.input.submit") }}
      </button>
    </div>
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
  padding: var(--space-xl);
  background: var(--color-surface);
  backdrop-filter: blur(var(--blur)) saturate(140%);
  -webkit-backdrop-filter: blur(var(--blur)) saturate(140%);
  border: 1px solid var(--color-border);
  border-radius: var(--radius-window);
  box-shadow: var(--shadow-window);
}

.label {
  margin: 0;
  font-size: 15px;
  font-weight: 600;
}

.hint {
  margin: 0;
  font-size: 12px;
  color: var(--color-muted);
}

.token-input {
  width: 220px;
  height: 40px;
  text-align: center;
  font-family: var(--font-mono);
  font-size: 22px;
  letter-spacing: 0.2em;
  font-variant-numeric: tabular-nums;
  border: 1px solid var(--color-border);
  border-radius: 8px;
  background: var(--color-elevated);
  caret-color: var(--color-accent);
}

.token-input:focus-visible {
  outline: 2px solid var(--color-ring);
  outline-offset: 1px;
}

.actions {
  display: flex;
  gap: var(--space-md);
  margin-top: var(--space-sm);
}
</style>
