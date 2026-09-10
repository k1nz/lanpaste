<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { PhLockKey, PhShieldCheck, PhHardDrives } from "@phosphor-icons/vue";
import lanOffice from "@/assets/lan-office.png";
import Reveal from "./Reveal.vue";

const { t } = useI18n();
</script>

<template>
  <section class="px-4 py-8 md:px-8 md:py-16">
    <Reveal>
      <figure
        class="relative mx-auto max-w-[1400px] overflow-hidden rounded-[var(--radius-window)]"
      >
        <img
          :src="lanOffice"
          :alt="t('privacy.imageAlt')"
          width="1600"
          height="900"
          class="h-[420px] w-full object-cover md:h-[560px]"
        />
        <figcaption
          class="absolute inset-0 flex items-end bg-gradient-to-t from-[#0a0a0c]/85 via-[#0a0a0c]/25 to-transparent p-6 md:items-center md:bg-gradient-to-r md:from-[#0a0a0c]/88 md:via-[#0a0a0c]/40 md:to-transparent md:p-12"
        >
          <div class="max-w-lg text-white">
            <h2
              class="text-3xl tracking-tighter md:text-5xl leading-[1.1]"
            >
              {{ t("privacy.headline") }}
            </h2>
            <p class="mt-4 max-w-[58ch] text-sm leading-relaxed text-white/75 md:text-base">
              {{ t("privacy.body") }}
            </p>
            <ul class="mt-6 grid gap-3 text-sm text-white/90">
              <li class="flex items-center gap-2">
                <PhLockKey :size="18" weight="regular" />
                {{ t("privacy.point1") }}
              </li>
              <li class="flex items-center gap-2">
                <PhShieldCheck :size="18" weight="regular" />
                {{ t("privacy.point2") }}
              </li>
              <li class="flex items-center gap-2">
                <PhHardDrives :size="18" weight="regular" />
                {{ t("privacy.point3") }}
              </li>
            </ul>
          </div>
        </figcaption>
      </figure>
    </Reveal>
  </section>
</template>
