<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { motion } from "motion-v";
import { PhGithubLogo } from "@phosphor-icons/vue";
import heroDesk from "@/assets/hero-desk.png";
import { GITHUB_REPO, OVERLAY_WIDTH } from "../constants";
import { useReducedMotion } from "../composables/useReducedMotion";
import { useRelease } from "../composables/useRelease";
import MagneticButton from "./MagneticButton.vue";
import OverlayPreview from "./OverlayPreview.vue";

const { t } = useI18n();
const reduce = useReducedMotion();
const { primaryUrl } = useRelease();

const enter = computed(() =>
  reduce.value ? false : { opacity: 0, y: 18 },
);
</script>

<template>
  <section class="relative">
    <div
      class="lg:grid lg:min-h-[calc(100dvh-4rem)] lg:grid-cols-[minmax(0,1fr)_minmax(0,1.25fr)]"
    >
      <motion.div
        class="flex flex-col justify-center px-4 pt-8 pb-6 md:px-8 lg:justify-start lg:px-0 lg:pt-16 lg:pb-8 lg:pl-[max(2rem,calc((100vw-1400px)/2+2rem))] lg:pr-12 xl:pt-20"
        :initial="enter"
        :animate="{ opacity: 1, y: 0 }"
        :transition="{ duration: 0.7, ease: [0.16, 1, 0.3, 1] }"
      >
        <div class="max-w-xl">
          <h1
            class="text-4xl tracking-tighter text-[var(--ink)] md:text-5xl lg:text-6xl leading-[1.08]"
          >
            {{ t("hero.headline") }}
          </h1>
          <p class="mt-5 max-w-[36ch] text-base leading-relaxed text-[var(--muted)] md:text-[17px]">
            {{ t("hero.sub") }}
          </p>
          <div class="mt-8 flex flex-wrap items-center gap-3">
            <MagneticButton :href="primaryUrl" :label="t('hero.download')">
              {{ t("hero.download") }}
            </MagneticButton>
            <a
              :href="GITHUB_REPO"
              target="_blank"
              rel="noreferrer"
              class="inline-flex h-11 items-center gap-2 rounded-[var(--radius-control)] border border-[var(--line)] px-4 text-[15px] text-[var(--ink)] hover:bg-[var(--elevated)] active:scale-[0.98]"
            >
              <PhGithubLogo :size="18" weight="regular" />
              {{ t("hero.github") }}
            </a>
          </div>
        </div>
      </motion.div>

      <motion.div
        class="relative isolate flex min-h-[340px] w-full items-center justify-center overflow-hidden md:min-h-[440px] lg:min-h-[calc(100dvh-4rem)]"
        :initial="reduce ? false : { opacity: 0, y: 28 }"
        :animate="{ opacity: 1, y: 0 }"
        :transition="{ duration: 0.8, delay: 0.08, ease: [0.16, 1, 0.3, 1] }"
      >
        <img
          :src="heroDesk"
          alt=""
          width="1600"
          height="900"
          class="absolute inset-0 h-full w-full scale-[1.04] object-cover"
        />
        <div class="absolute inset-0 bg-[var(--hero-veil)]" />
        <div
          class="relative z-10 w-full px-4 py-8 md:px-8 lg:px-10"
          :style="{ maxWidth: `${OVERLAY_WIDTH}px` }"
        >
          <OverlayPreview />
        </div>
      </motion.div>
    </div>
  </section>
</template>
