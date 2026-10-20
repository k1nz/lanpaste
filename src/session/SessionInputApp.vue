<script setup lang="ts">
import { onMounted, onUnmounted, ref } from "vue";
import type { SessionInputPayload } from "../shared/types";
import { t } from "../shared/i18n";
import { localizeError } from "../shared/errors";
import { cancelSession, submitSessionCode } from "../shared/ipc";
import { onSessionInput, onSessionInputHide } from "../shared/events";

const payload = ref<SessionInputPayload | null>(null);
const code = ref("");
const error = ref("");
const busy = ref(false);
const inputEl = ref<HTMLInputElement | null>(null);
const unlisteners: Array<() => void> = [];

function onCodeInput(ev: Event) {
  const el = ev.target as HTMLInputElement;
  code.value = el.value.replace(/\D/g, "").slice(0, 8);
}

async function submit() {
  if (!payload.value || code.value.length !== 8 || busy.value) return;
  busy.value = true;
  error.value = "";
  const r = await submitSessionCode(payload.value.instanceId, code.value);
  busy.value = false;
  if (!r.ok) error.value = localizeError(r.error);
}

async function cancel() {
  await cancelSession();
}

onMounted(async () => {
  unlisteners.push(
    await onSessionInput((next) => {
      const same = payload.value?.instanceId === next.instanceId;
      payload.value = next;
      if (!same) {
        code.value = "";
        error.value = "";
      }
      requestAnimationFrame(() => inputEl.value?.focus());
    }),
    await onSessionInputHide(() => {
      payload.value = null;
      code.value = "";
    }),
  );
  requestAnimationFrame(() => inputEl.value?.focus());
});

onUnmounted(() => {
  for (const u of unlisteners) u();
});
</script>

<template>
  <div class="session-shell glass-surface">
    <p class="label">{{ t("session.input.title") }}</p>
    <p class="hint">
      {{ payload ? t("session.input.hint", { name: payload.deviceName }) : t("session.waiting") }}
    </p>
    <input
      ref="inputEl"
      class="token-input"
      :value="code"
      maxlength="8"
      inputmode="numeric"
      autocomplete="one-time-code"
      :placeholder="t('session.input.placeholder')"
      @input="onCodeInput"
      @keydown.enter.prevent="submit"
      @keydown.escape.prevent="cancel"
    />
    <p v-if="error" class="live-error" role="alert">{{ error }}</p>
    <div class="actions">
      <button type="button" class="btn-ghost" @click="cancel">
        {{ t("session.input.cancel") }}
      </button>
      <button
        type="button"
        class="btn-primary"
        :disabled="code.length !== 8 || busy || !payload"
        @click="submit"
      >
        {{ t("session.input.submit") }}
      </button>
    </div>
  </div>
</template>

<style scoped>
.session-shell {
  width: 100%;
  height: 100%;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: var(--space-md);
  padding: var(--space-xl);
  box-sizing: border-box;
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
  text-align: center;
}

.token-input {
  width: 240px;
  height: 40px;
  text-align: center;
  font-family: var(--font-mono);
  font-size: 22px;
  letter-spacing: 0.18em;
  font-variant-numeric: tabular-nums;
  border: 1px solid var(--color-border);
  border-radius: 8px;
  background: var(--color-elevated);
  caret-color: var(--color-accent);
  color: var(--color-foreground);
}

.token-input:focus-visible {
  outline: 2px solid var(--color-ring);
  outline-offset: 1px;
}

.actions {
  display: flex;
  gap: var(--space-md);
}
</style>
