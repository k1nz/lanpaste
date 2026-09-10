<script setup lang="ts">
import { computed } from "vue";
import { motion, useMotionValue, useSpring } from "motion-v";
import { useReducedMotion } from "../composables/useReducedMotion";

const props = defineProps<{
  href: string;
  label: string;
}>();

const reduce = useReducedMotion();
const mx = useMotionValue(0);
const my = useMotionValue(0);
const x = useSpring(mx, { stiffness: 180, damping: 20 });
const y = useSpring(my, { stiffness: 180, damping: 20 });

const style = computed(() => (reduce.value ? undefined : { x, y }));

function onMove(event: PointerEvent) {
  if (reduce.value) return;
  const el = event.currentTarget as HTMLElement;
  const box = el.getBoundingClientRect();
  mx.set((event.clientX - box.left - box.width / 2) * 0.28);
  my.set((event.clientY - box.top - box.height / 2) * 0.28);
}

function onLeave() {
  mx.set(0);
  my.set(0);
}
</script>

<template>
  <motion.a
    :href="props.href"
    :style="style"
    :aria-label="props.label"
    class="inline-flex h-11 items-center justify-center rounded-[var(--radius-control)] bg-accent px-5 text-[15px] font-medium text-on-accent whitespace-nowrap transition-opacity duration-200 hover:opacity-90 active:scale-[0.98] cursor-pointer"
    :while-press="reduce ? undefined : { scale: 0.98 }"
    @pointermove="onMove"
    @pointerleave="onLeave"
    @pointerdown="onLeave"
  >
    <slot />
  </motion.a>
</template>
