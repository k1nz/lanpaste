<script setup lang="ts">
import { computed } from "vue";
import { PhCommand, PhOption } from "@phosphor-icons/vue";

const props = withDefaults(
  defineProps<{
    k: string;
    size?: "sm" | "md";
    decorative?: boolean;
  }>(),
  { size: "sm", decorative: false },
);

type Kind = "command" | "option" | "shift" | "control" | "enter" | "text";

const LABELS: Record<Exclude<Kind, "text">, string> = {
  command: "Command",
  option: "Option",
  shift: "Shift",
  control: "Ctrl",
  enter: "Enter",
};

const kind = computed<Kind>(() => {
  const k = props.k.trim();
  const lower = k.toLowerCase();
  if (["command", "cmd", "meta", "⌘"].includes(lower)) return "command";
  if (["option", "alt", "⌥"].includes(lower)) return "option";
  if (["shift", "⇧"].includes(lower)) return "shift";
  if (["control", "ctrl", "⌃"].includes(lower)) return "control";
  if (["enter", "return", "↵", "⏎"].includes(lower)) return "enter";
  return "text";
});

const label = computed(() => (kind.value === "text" ? props.k : LABELS[kind.value]));
const iconPx = computed(() => (props.size === "md" ? 15 : 11));
const iconWeight = computed(() => (props.size === "md" ? "fill" : "bold"));
</script>

<template>
  <kbd
    class="keycap"
    :class="size"
    :aria-hidden="decorative ? 'true' : undefined"
    :aria-label="decorative ? undefined : label"
  >
    <PhCommand v-if="kind === 'command'" :size="iconPx" :weight="iconWeight" />
    <PhOption v-else-if="kind === 'option'" :size="iconPx" :weight="iconWeight" />
    <svg
      v-else-if="kind === 'shift'"
      :width="iconPx"
      :height="iconPx"
      viewBox="0 0 16 16"
      fill="currentColor"
      aria-hidden="true"
    >
      <path
        d="M8.49 2.18a.7.7 0 0 0-.98 0L2.15 7.54A.7.7 0 0 0 2.64 8.8H4.7v4.4c0 .4.32.72.72.72h5.16c.4 0 .72-.32.72-.72V8.8h2.06a.7.7 0 0 0 .49-1.26L8.49 2.18Z"
      />
    </svg>
    <svg
      v-else-if="kind === 'enter'"
      :width="iconPx"
      :height="iconPx"
      viewBox="0 0 16 16"
      fill="none"
      aria-hidden="true"
    >
      <path
        d="M13 3.25v5.1A1.9 1.9 0 0 1 11.1 10.25H3.6"
        stroke="currentColor"
        stroke-width="1.6"
        stroke-linecap="round"
        stroke-linejoin="round"
      />
      <path
        d="M6.15 7.35 3.35 10.15 6.15 12.95"
        stroke="currentColor"
        stroke-width="1.6"
        stroke-linecap="round"
        stroke-linejoin="round"
      />
    </svg>
    <span v-else class="key-text">{{ label }}</span>
  </kbd>
</template>

<style scoped>
.keycap {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  flex-shrink: 0;
  box-sizing: border-box;
  margin: 0;
  border: 1px solid var(--keycap-border);
  background: var(--keycap-bg);
  box-shadow: inset 0 1px 0 var(--keycap-shine);
  color: var(--keycap-fg);
  font-family: inherit;
  font-weight: 500;
  line-height: 1;
  letter-spacing: 0;
  user-select: none;
}

.keycap.sm {
  min-width: 18px;
  height: 18px;
  padding: 0 5px;
  border-radius: 4px;
  font-size: 11px;
}

.keycap.md {
  min-width: 28px;
  height: 28px;
  padding: 0 8px;
  border-radius: 6px;
  font-size: 13px;
}

.keycap :deep(svg) {
  display: block;
  flex-shrink: 0;
}

.key-text {
  display: block;
  transform: translateY(0.5px);
}
</style>
