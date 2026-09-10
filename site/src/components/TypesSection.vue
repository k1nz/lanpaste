<script setup lang="ts">
import { useI18n } from "vue-i18n";
import clipboardStill from "@/assets/clipboard-still.png";
import codeThumb from "@/assets/code-thumb.png";
import Reveal from "./Reveal.vue";

const { t } = useI18n();
</script>

<template>
  <section id="features" class="px-4 py-24 md:px-8 md:py-32">
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

      <Reveal>
        <div class="mt-12 grid grid-cols-1 gap-3 md:grid-cols-6">
          <article
            class="relative overflow-hidden rounded-[var(--radius-window)] border border-[var(--line)] md:col-span-4 md:row-span-2 md:min-h-[340px]"
          >
            <img
              :src="clipboardStill"
              :alt="t('types.imageTitle')"
              class="h-56 w-full object-cover md:absolute md:inset-0 md:h-full"
            />
            <div
              class="absolute inset-x-0 bottom-0 bg-gradient-to-t from-[#0a0a0c]/90 to-transparent p-5 md:p-6"
            >
              <h3 class="text-lg font-medium tracking-tight text-white">
                {{ t("types.imageTitle") }}
              </h3>
              <p class="mt-1 max-w-[42ch] text-sm leading-relaxed text-white/75">
                {{ t("types.imageBody") }}
              </p>
            </div>
          </article>

          <article
            class="flex min-h-[160px] flex-col justify-between rounded-[var(--radius-window)] bg-accent p-5 text-on-accent md:col-span-2"
          >
            <span class="font-mono text-3xl tracking-tight md:text-4xl">#0A84FF</span>
            <h3 class="text-base font-medium">{{ t("types.colorTitle") }}</h3>
          </article>

          <article
            class="flex min-h-[160px] flex-col justify-between rounded-[var(--radius-window)] border border-[var(--line)] bg-[var(--canvas-2)] p-5 md:col-span-2"
          >
            <div>
              <p class="font-mono text-sm text-[var(--ink)]">demo-reel.mov</p>
              <p class="mt-1 text-sm text-[var(--muted)]">1.8 GB · {{ t("hero.pending") }}</p>
            </div>
            <div>
              <h3 class="text-base font-medium">{{ t("types.fileTitle") }}</h3>
              <p class="mt-1 text-sm leading-relaxed text-[var(--muted)]">
                {{ t("types.fileBody") }}
              </p>
            </div>
          </article>

          <article
            class="rounded-[var(--radius-window)] border border-[var(--line)] p-5 md:col-span-3"
          >
            <h3 class="text-base font-medium">{{ t("types.shortcutTitle") }}</h3>
            <div class="mt-6 flex flex-wrap gap-3">
              <div class="rounded-[var(--radius-control)] bg-[var(--elevated)] px-3 py-2">
                <p class="text-[11px] text-[var(--muted)]">{{ t("types.shortcutMac") }}</p>
                <p class="mt-1 font-mono text-lg tracking-tight">⌥⇧V</p>
              </div>
              <div class="rounded-[var(--radius-control)] bg-[var(--elevated)] px-3 py-2">
                <p class="text-[11px] text-[var(--muted)]">{{ t("types.shortcutWin") }}</p>
                <p class="mt-1 font-mono text-lg tracking-tight">Ctrl+Shift+V</p>
              </div>
            </div>
          </article>

          <article
            class="overflow-hidden rounded-[var(--radius-window)] border border-[var(--line)] md:col-span-3"
          >
            <img :src="codeThumb" alt="" class="h-28 w-full object-cover" />
            <div class="p-5">
              <h3 class="text-base font-medium">{{ t("types.codeTitle") }}</h3>
              <p class="mt-1 text-sm leading-relaxed text-[var(--muted)]">
                {{ t("types.codeBody") }}
              </p>
            </div>
          </article>
        </div>
      </Reveal>
    </div>
  </section>
</template>
