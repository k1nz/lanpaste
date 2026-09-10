<script setup lang="ts">
import { useI18n } from "vue-i18n";
import lanOffice from "@/assets/lan-office.png";
import Reveal from "./Reveal.vue";

const { t } = useI18n();
</script>

<template>
  <section class="px-4 py-24 md:px-8 md:py-32">
    <div class="mx-auto max-w-[1400px]">
      <Reveal>
        <h2
          class="max-w-[18em] text-3xl tracking-tighter text-[var(--ink)] md:text-5xl leading-[1.1]"
        >
          {{ t("privacy.headline") }}
        </h2>
        <p class="mt-5 max-w-[62ch] text-base leading-relaxed text-[var(--muted)]">
          {{ t("privacy.body") }}
        </p>
      </Reveal>

      <Reveal :delay="0.08">
        <figure class="mt-12">
          <img
            :src="lanOffice"
            :alt="t('privacy.imageAlt')"
            width="1600"
            height="900"
            class="h-[320px] w-full rounded-[var(--radius-window)] object-cover md:h-[480px]"
          />
        </figure>
      </Reveal>
    </div>
  </section>
</template>
