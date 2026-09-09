<script setup lang="ts">
import { PhCaretDown } from "@phosphor-icons/vue";
import { computed, onMounted, onUnmounted, ref } from "vue";
import { t } from "../../shared/i18n";
import { typeLabel } from "../../shared/format";
import type { PasteType } from "../../shared/types";
import { PASTE_TYPES } from "../typeMeta";

const props = defineProps<{
  modelValue: PasteType | "all";
  open: boolean;
}>();

const emit = defineEmits<{
  "update:modelValue": [value: PasteType | "all"];
  "update:open": [open: boolean];
}>();

const root = ref<HTMLElement | null>(null);

const label = computed(() =>
  props.modelValue === "all" ? t("overlay.allTypes") : typeLabel(props.modelValue),
);

const options = computed(() => [
  { value: "all" as const, label: t("overlay.allTypes") },
  ...PASTE_TYPES.map((value) => ({ value, label: typeLabel(value) })),
]);

function toggle() {
  emit("update:open", !props.open);
}

function pick(value: PasteType | "all") {
  emit("update:modelValue", value);
  emit("update:open", false);
}

function onDoc(ev: MouseEvent) {
  if (!props.open) return;
  const el = root.value;
  if (el && ev.target instanceof Node && !el.contains(ev.target)) {
    emit("update:open", false);
  }
}

onMounted(() => document.addEventListener("mousedown", onDoc));
onUnmounted(() => document.removeEventListener("mousedown", onDoc));
</script>

<template>
  <div ref="root" class="type-filter">
    <button
      class="type-filter-btn"
      type="button"
      :aria-expanded="open"
      aria-haspopup="listbox"
      @click="toggle"
    >
      <span>{{ label }}</span>
      <PhCaretDown :size="12" weight="regular" />
    </button>
    <ul v-if="open" class="type-filter-menu" role="listbox">
      <li
        v-for="opt in options"
        :key="opt.value"
        role="option"
        :aria-selected="opt.value === modelValue"
        :class="{ selected: opt.value === modelValue }"
        @click="pick(opt.value)"
      >
        {{ opt.label }}
      </li>
    </ul>
  </div>
</template>

<style scoped>
.type-filter {
  position: relative;
  flex-shrink: 0;
}

.type-filter-btn {
  display: flex;
  align-items: center;
  gap: var(--space-sm);
  height: 28px;
  padding: 0 8px;
  border-radius: var(--radius-row);
  color: var(--color-muted);
  font-size: 12px;
  cursor: pointer;
  transition: opacity 160ms ease;
}

.type-filter-btn:hover {
  background: var(--color-elevated);
  color: var(--color-foreground);
}

.type-filter-menu {
  position: absolute;
  top: calc(100% + 4px);
  right: 0;
  z-index: 20;
  min-width: 140px;
  margin: 0;
  padding: 4px;
  list-style: none;
  background: var(--color-surface-solid);
  border: 1px solid var(--color-border);
  border-radius: 8px;
  box-shadow: var(--shadow-window);
}

.type-filter-menu li {
  padding: 6px 10px;
  border-radius: 6px;
  font-size: 13px;
  cursor: pointer;
}

.type-filter-menu li:hover {
  background: var(--color-elevated);
}

.type-filter-menu li.selected {
  background: var(--color-accent);
  color: var(--color-on-accent);
}
</style>
