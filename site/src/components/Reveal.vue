<script setup lang="ts">
import { motion } from "motion-v";
import { useReducedMotion } from "../composables/useReducedMotion";

const reduce = useReducedMotion();
</script>

<template>
  <motion.div
    :initial="reduce ? false : { opacity: 0, y: 24 }"
    :while-in-view="{ opacity: 1, y: 0 }"
    :in-view-options="{ once: true, amount: 0.28 }"
    :transition="{ duration: 0.6, ease: [0.16, 1, 0.3, 1] }"
  >
    <slot />
  </motion.div>
</template>
