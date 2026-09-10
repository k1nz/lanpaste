<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { PhMagnifyingGlass } from "@phosphor-icons/vue";
import Keycap from "./Keycap.vue";
import Reveal from "./Reveal.vue";

const { t } = useI18n();
</script>

<template>
  <section class="px-4 pb-24 md:px-8 md:pb-32">
    <div class="mx-auto max-w-[1400px]">
      <Reveal>
        <h2
          class="max-w-[16ch] text-3xl tracking-tighter text-[var(--ink)] md:text-5xl leading-[1.1]"
        >
          {{ t("use.headline") }}
        </h2>
        <p class="mt-4 max-w-[58ch] text-base leading-relaxed text-[var(--muted)]">
          {{ t("use.body") }}
        </p>
      </Reveal>

      <Reveal :delay="0.08">
        <div
          class="mt-12 grid grid-cols-1 gap-10 border-t border-[var(--line)] pt-10 sm:grid-cols-2 lg:grid-cols-[1.35fr_1.1fr_0.75fr_1.15fr] lg:gap-0"
        >
          <article class="lg:pr-8">
            <div
              class="flex h-11 items-center gap-2 rounded-[var(--radius-control)] border border-[var(--line)] bg-[var(--elevated)] px-3 text-sm text-[var(--muted)]"
            >
              <PhMagnifyingGlass :size="16" weight="regular" />
              <span>{{ t("hero.overlaySearch") }}</span>
            </div>
            <h3 class="mt-5 text-base font-medium">{{ t("use.filterTitle") }}</h3>
            <p class="mt-2 max-w-[36ch] text-sm leading-relaxed text-[var(--muted)]">
              {{ t("use.filterBody") }}
            </p>
          </article>

          <article class="lg:border-l lg:border-[var(--line)] lg:px-8">
            <p class="flex h-11 items-center gap-1" :aria-label="t('use.pasteTitle')">
              <Keycap k="↑" size="md" />
              <Keycap k="↓" size="md" />
              <Keycap k="enter" size="md" />
            </p>
            <h3 class="mt-5 text-base font-medium">{{ t("use.pasteTitle") }}</h3>
            <p class="mt-2 max-w-[36ch] text-sm leading-relaxed text-[var(--muted)]">
              {{ t("use.pasteBody") }}
            </p>
          </article>

          <article class="lg:border-l lg:border-[var(--line)] lg:px-8">
            <p class="flex h-11 items-center" :aria-label="t('use.escTitle')">
              <Keycap k="Esc" size="md" />
            </p>
            <h3 class="mt-5 text-base font-medium">{{ t("use.escTitle") }}</h3>
            <p class="mt-2 max-w-[32ch] text-sm leading-relaxed text-[var(--muted)]">
              {{ t("use.escBody") }}
            </p>
          </article>

          <article class="lg:border-l lg:border-[var(--line)] lg:pl-8">
            <div class="flex min-h-11 flex-wrap items-center gap-3">
              <p class="flex items-center gap-2" :aria-label="`${t('use.actionsMac')} Command K`">
                <span class="text-[11px] text-[var(--muted)]">{{ t("use.actionsMac") }}</span>
                <span class="inline-flex items-center gap-1">
                  <Keycap k="command" size="md" />
                  <Keycap k="K" size="md" />
                </span>
              </p>
              <p class="flex items-center gap-2" :aria-label="`${t('use.actionsWin')} Control K`">
                <span class="text-[11px] text-[var(--muted)]">{{ t("use.actionsWin") }}</span>
                <span class="inline-flex items-center gap-1">
                  <Keycap k="control" size="md" />
                  <Keycap k="K" size="md" />
                </span>
              </p>
            </div>
            <h3 class="mt-5 text-base font-medium">{{ t("use.actionsTitle") }}</h3>
            <p class="mt-2 max-w-[36ch] text-sm leading-relaxed text-[var(--muted)]">
              {{ t("use.actionsBody") }}
            </p>
          </article>
        </div>
      </Reveal>
    </div>
  </section>
</template>
