<script setup lang="ts">
import { t } from "../../shared/i18n";
</script>

<template>
  <section class="pane">
    <h1>{{ t("settings.about.name") }}</h1>
    <p>{{ t("settings.about.tagline") }}</p>
    <p class="ver">{{ t("settings.about.version", { version: "0.1.0" }) }}</p>
  </section>
</template>

<style scoped>
.pane h1 {
  margin: 0 0 var(--space-md);
  font-size: 20px;
  font-weight: 600;
}

.pane p {
  margin: 0 0 var(--space-md);
  color: var(--color-muted);
}

.ver {
  font-size: 12px;
}
</style>
