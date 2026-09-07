<script setup lang="ts">
defineProps<{
  modelValue: boolean;
  label: string;
}>();

const emit = defineEmits<{
  "update:modelValue": [value: boolean];
}>();
</script>

<template>
  <label class="switch-row">
    <span>{{ label }}</span>
    <button
      type="button"
      class="switch"
      role="switch"
      :aria-checked="modelValue"
      :class="{ on: modelValue }"
      @click="emit('update:modelValue', !modelValue)"
    >
      <span class="knob" />
    </button>
  </label>
</template>

<style scoped>
.switch-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-lg);
  font-size: 13px;
  cursor: pointer;
}

.switch {
  position: relative;
  width: 40px;
  height: 24px;
  border-radius: 99px;
  background: var(--color-elevated);
  border: 1px solid var(--color-border);
  flex-shrink: 0;
  cursor: pointer;
  transition: background 160ms ease;
}

.switch.on {
  background: var(--color-accent);
  border-color: var(--color-accent);
}

.knob {
  position: absolute;
  top: 1.5px;
  left: 1.5px;
  width: 19px;
  height: 19px;
  border-radius: 99px;
  background: var(--color-on-accent);
  transition: transform 160ms ease;
}

.switch.on .knob {
  transform: translateX(16px);
}

@media (prefers-reduced-motion: reduce) {
  .switch,
  .knob {
    transition: none;
  }
}
</style>
