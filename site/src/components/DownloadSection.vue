<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { PhAppleLogo, PhWindowsLogo } from "@phosphor-icons/vue";
import { GITHUB_RELEASES } from "../constants";
import { useRelease, type PlatformId } from "../composables/useRelease";
import MagneticButton from "./MagneticButton.vue";
import Reveal from "./Reveal.vue";

const { t } = useI18n();
const { assets, platform, primaryUrl, loading, error } = useRelease();

const primaryLabel = computed(() => {
  if (platform.value === "windows") return t("download.windows");
  if (platform.value === "mac-intel") return t("download.macIntel");
  return t("download.macArm");
});

const others = computed(() => {
  const order: PlatformId[] = ["mac-arm", "mac-intel", "windows"];
  return order.filter((id) => id !== platform.value);
});

function labelFor(id: PlatformId) {
  if (id === "windows") return t("download.windows");
  if (id === "mac-intel") return t("download.macIntel");
  return t("download.macArm");
}
</script>

<template>
  <section id="download" class="px-4 py-24 md:px-8 md:py-32">
    <Reveal>
      <div
        class="mx-auto max-w-[1400px] rounded-[var(--radius-window)] border border-[var(--line)] bg-[var(--canvas-2)] px-6 py-16 md:px-16 md:py-20"
      >
        <h2 class="max-w-[12ch] text-3xl tracking-tighter md:text-5xl leading-[1.1]">
          {{ t("download.headline") }}
        </h2>
        <p class="mt-4 max-w-[52ch] text-base leading-relaxed text-[var(--muted)]">
          {{ t("download.body") }}
        </p>

        <div class="mt-10 flex flex-wrap items-center gap-3">
          <MagneticButton :href="primaryUrl" :label="t('hero.download')">
            <span class="inline-flex items-center gap-2">
              <PhWindowsLogo v-if="platform === 'windows'" :size="18" weight="fill" />
              <PhAppleLogo v-else :size="18" weight="fill" />
              {{ t("hero.download") }}
            </span>
          </MagneticButton>
          <span class="text-sm text-[var(--muted)]">{{ primaryLabel }}</span>
        </div>

        <p v-if="loading" class="mt-4 text-sm text-[var(--muted)]">
          {{ t("download.loading") }}
        </p>
        <p v-else-if="error" class="mt-4 text-sm text-[var(--muted)]">
          {{ t("download.error") }}
        </p>

        <div class="mt-8">
          <p class="text-sm text-[var(--muted)]">{{ t("download.otherPlatforms") }}</p>
          <div class="mt-3 flex flex-wrap gap-x-5 gap-y-2 text-sm">
            <a
              v-for="id in others"
              :key="id"
              :href="assets[id]"
              class="text-[var(--ink)] underline-offset-4 hover:underline"
            >
              {{ labelFor(id) }}
            </a>
            <a
              :href="GITHUB_RELEASES"
              target="_blank"
              rel="noreferrer"
              class="text-[var(--ink)] underline-offset-4 hover:underline"
            >
              {{ t("download.allReleases") }}
            </a>
          </div>
        </div>

        <div class="mt-10 max-w-[62ch] space-y-2 text-sm leading-relaxed text-[var(--muted)]">
          <p>{{ t("download.macGate") }}</p>
          <p>{{ t("download.winGate") }}</p>
          <p>{{ t("download.linux") }}</p>
        </div>
      </div>
    </Reveal>
  </section>
</template>
