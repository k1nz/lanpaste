<script setup lang="ts">
import { useI18n } from "vue-i18n";
import PairingPreview from "./PairingPreview.vue";
import Reveal from "./Reveal.vue";

const { t } = useI18n();
</script>

<template>
  <section class="px-4 py-24 md:px-8 md:py-32">
    <div class="mx-auto grid max-w-[1400px] gap-10 lg:grid-cols-[1.15fr_0.85fr] lg:items-start">
      <Reveal>
        <PairingPreview />
      </Reveal>

      <Reveal :delay="0.08">
        <div>
          <h2
            class="max-w-[16ch] text-3xl tracking-tighter md:text-5xl leading-[1.1]"
          >
            {{ t("setup.headline") }}
          </h2>
          <ol class="mt-10">
            <li class="py-5">
              <h3 class="text-lg font-medium">{{ t("setup.installTitle") }}</h3>
              <p class="mt-2 max-w-[46ch] text-sm leading-relaxed text-[var(--muted)]">
                {{ t("setup.installBody") }}
              </p>
            </li>
            <li class="border-t border-[var(--line)] py-5">
              <h3 class="text-lg font-medium">{{ t("setup.pairTitle") }}</h3>
              <p class="mt-2 max-w-[46ch] text-sm leading-relaxed text-[var(--muted)]">
                {{ t("setup.pairBody") }}
              </p>
            </li>
            <li class="border-t border-[var(--line)] py-5">
              <h3 class="text-lg font-medium">{{ t("setup.pasteTitle") }}</h3>
              <p class="mt-2 max-w-[46ch] text-sm leading-relaxed text-[var(--muted)]">
                {{ t("setup.pasteBody") }}
              </p>
            </li>
          </ol>
        </div>
      </Reveal>
    </div>
  </section>
</template>
