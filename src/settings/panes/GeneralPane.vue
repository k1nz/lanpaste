<script setup lang="ts">
import { t } from "../../shared/i18n";

const emit = defineEmits<{
  goDevices: [];
}>();
</script>

<template>
  <section class="pane">
    <h1>{{ t("settings.general.title") }}</h1>
    <p>{{ t("settings.general.body") }}</p>
    <button type="button" class="btn-ghost" @click="emit('goDevices')">
      {{ t("settings.general.goDevices") }}
    </button>
  </section>
</template>

<style scoped>
.pane h1 {
  margin: 0 0 var(--space-lg);
  font-size: 15px;
  font-weight: 600;
}

.pane p {
  margin: 0 0 var(--space-xl);
  color: var(--color-muted);
  max-width: 42em;
}
</style>
