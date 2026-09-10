<script setup lang="ts">
import { useI18n } from "vue-i18n";
import clipboardStill from "@/assets/clipboard-still.png";
import Keycap from "./Keycap.vue";
import Reveal from "./Reveal.vue";

const { t } = useI18n();
</script>

<template>
  <section id="features" class="px-4 pt-8 pb-24 md:px-8 md:pt-12 md:pb-32">
    <div class="mx-auto max-w-[1400px]">
      <Reveal>
        <h2
          class="max-w-[16ch] text-3xl tracking-tighter text-[var(--ink)] md:text-5xl leading-[1.1]"
        >
          {{ t("types.headline") }}
        </h2>
        <p class="mt-4 max-w-[58ch] text-base leading-relaxed text-[var(--muted)]">
          {{ t("types.body") }}
        </p>
      </Reveal>

      <div class="mt-12 grid grid-cols-1 gap-3 md:grid-cols-6">
        <Reveal class="h-full md:col-span-4 md:row-span-2" :delay="0.02">
          <article
            class="relative h-full overflow-hidden rounded-[var(--radius-window)] border border-[var(--line)] md:min-h-[340px]"
          >
            <img
              :src="clipboardStill"
              :alt="t('types.imageTitle')"
              class="h-56 w-full object-cover md:absolute md:inset-0 md:h-full"
            />
            <div
              class="absolute inset-x-0 bottom-0 bg-gradient-to-t from-[var(--scrim)] to-transparent p-5 md:p-6"
            >
              <h3 class="text-lg font-medium tracking-tight text-[var(--on-photo)]">
                {{ t("types.imageTitle") }}
              </h3>
              <p class="mt-1 max-w-[42ch] text-sm leading-relaxed text-[var(--on-photo-muted)]">
                {{ t("types.imageBody") }}
              </p>
            </div>
          </article>
        </Reveal>

        <Reveal class="h-full md:col-span-2" :delay="0.08">
          <article
            class="flex h-full min-h-[180px] flex-col justify-between overflow-hidden rounded-[var(--radius-window)] bg-accent text-on-accent"
          >
            <div class="flex flex-1 items-end p-5">
              <span class="font-mono text-3xl tracking-tight md:text-4xl">#0A84FF</span>
            </div>
            <h3 class="border-t border-white/20 px-5 py-4 text-base font-medium">
              {{ t("types.colorTitle") }}
            </h3>
          </article>
        </Reveal>

        <Reveal class="h-full md:col-span-2" :delay="0.12">
          <article
            class="flex h-full min-h-[180px] flex-col justify-between rounded-[var(--radius-window)] border border-[var(--line)] bg-[var(--canvas-2)] p-5"
          >
            <div>
              <p class="font-mono text-xl tracking-tight text-[var(--ink)] md:text-2xl">
                {{ t("types.fileName") }}
              </p>
              <p class="mt-2 text-sm text-[var(--muted)]">
                {{ t("types.fileSize") }} · {{ t("hero.pending") }}
              </p>
            </div>
            <div>
              <h3 class="text-base font-medium">{{ t("types.fileTitle") }}</h3>
              <p class="mt-1 text-sm leading-relaxed text-[var(--muted)]">
                {{ t("types.fileBody") }}
              </p>
            </div>
          </article>
        </Reveal>

        <Reveal class="h-full md:col-span-3" :delay="0.16">
          <article
            class="h-full rounded-[var(--radius-window)] border border-[var(--line)] p-5"
          >
            <h3 class="text-base font-medium">{{ t("types.shortcutTitle") }}</h3>
            <div class="mt-6 flex flex-wrap gap-3">
              <div class="rounded-[var(--radius-control)] bg-[var(--elevated)] px-3 py-2">
                <p class="text-[11px] text-[var(--muted)]">{{ t("types.shortcutMac") }}</p>
                <p class="mt-2 flex items-center gap-1" :aria-label="t('types.shortcutMacKeys')">
                  <Keycap k="option" size="md" />
                  <Keycap k="shift" size="md" />
                  <Keycap k="V" size="md" />
                </p>
              </div>
              <div class="rounded-[var(--radius-control)] bg-[var(--elevated)] px-3 py-2">
                <p class="text-[11px] text-[var(--muted)]">{{ t("types.shortcutWin") }}</p>
                <p class="mt-2 flex items-center gap-1" :aria-label="t('types.shortcutWinKeys')">
                  <Keycap k="control" size="md" />
                  <Keycap k="shift" size="md" />
                  <Keycap k="V" size="md" />
                </p>
              </div>
            </div>
          </article>
        </Reveal>

        <Reveal class="h-full md:col-span-3" :delay="0.2">
          <article
            class="flex h-full flex-col overflow-hidden rounded-[var(--radius-window)] border border-[var(--line)]"
          >
            <pre
              class="flex min-h-[7.5rem] flex-1 items-center overflow-x-auto bg-[var(--canvas-2)] px-5 py-4 font-mono text-[13px] leading-relaxed text-[var(--ink)] md:text-sm"
            ><code>{{ t("types.codeSnippet") }}</code></pre>
            <div class="p-5">
              <h3 class="text-base font-medium">{{ t("types.codeTitle") }}</h3>
              <p class="mt-1 text-sm leading-relaxed text-[var(--muted)]">
                {{ t("types.codeBody") }}
              </p>
            </div>
          </article>
        </Reveal>
      </div>
    </div>
  </section>
</template>
