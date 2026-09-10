<script setup lang="ts">
import { onMounted, watch } from "vue";
import { useI18n } from "vue-i18n";
import SiteNav from "./components/SiteNav.vue";
import HeroSection from "./components/HeroSection.vue";
import TypesSection from "./components/TypesSection.vue";
import OverlayUseSection from "./components/OverlayUseSection.vue";
import PrivacySection from "./components/PrivacySection.vue";
import SetupSection from "./components/SetupSection.vue";
import DownloadSection from "./components/DownloadSection.vue";
import SiteFooter from "./components/SiteFooter.vue";

const { t } = useI18n();

function syncMeta() {
  document.title = t("meta.title");
  const meta = document.querySelector('meta[name="description"]');
  if (meta) meta.setAttribute("content", t("meta.description"));
}

onMounted(syncMeta);
watch(() => t("meta.title"), syncMeta);
</script>

<template>
  <div id="top" class="min-h-[100dvh] bg-[var(--canvas)] text-[var(--ink)]">
    <div class="site-grain" aria-hidden="true" />
    <SiteNav />
    <main>
      <HeroSection />
      <TypesSection />
      <OverlayUseSection />
      <PrivacySection />
      <SetupSection />
      <DownloadSection />
    </main>
    <SiteFooter />
  </div>
</template>
