<script setup lang="ts">
import { computed } from "vue";
import { useI18n } from "vue-i18n";
import { motion } from "motion-v";
import { PhGithubLogo } from "@phosphor-icons/vue";
import heroDesk from "@/assets/hero-desk.png";
import { GITHUB_REPO } from "../constants";
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
  <section class="relative min-h-[100dvh] pt-16 md:pt-20">
    <div
      class="mx-auto grid max-w-[1400px] items-center gap-10 px-4 pb-28 md:px-8 lg:grid-cols-[minmax(0,0.92fr)_minmax(0,1.08fr)] lg:gap-8 lg:pb-24"
    >
      <motion.div
        class="max-w-xl"
        :initial="enter"
        :animate="{ opacity: 1, y: 0 }"
        :transition="{ duration: 0.7, ease: [0.16, 1, 0.3, 1] }"
      >
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
      </motion.div>

      <motion.div
        class="relative min-h-[360px] overflow-hidden rounded-[var(--radius-window)] md:min-h-[480px]"
        :initial="reduce ? false : { opacity: 0, y: 28 }"
        :animate="{ opacity: 1, y: 0 }"
        :transition="{ duration: 0.8, delay: 0.08, ease: [0.16, 1, 0.3, 1] }"
      >
        <img
          :src="heroDesk"
          alt=""
          width="1600"
          height="900"
          class="h-[360px] w-full object-cover md:h-[460px] lg:h-[520px]"
        />
        <div
          class="pointer-events-auto absolute inset-x-4 top-8 flex justify-center md:inset-x-8 md:top-12 lg:top-16"
        >
          <div class="origin-top scale-[0.78] md:scale-[0.86] lg:scale-[0.9]">
            <OverlayPreview />
          </div>
        </div>
      </motion.div>
    </div>
  </section>
</template>
