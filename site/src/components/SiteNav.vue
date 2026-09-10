<script setup lang="ts">
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import {
  PhClipboard,
  PhList,
  PhMoon,
  PhSun,
  PhX,
} from "@phosphor-icons/vue";
import { GITHUB_REPO } from "../constants";
import { persistLocale, type LocaleCode } from "../i18n";
import { useTheme } from "../composables/useTheme";

const { t, locale } = useI18n();
const { theme, toggle } = useTheme();
const open = ref(false);

const localeCode = computed(() => locale.value as LocaleCode);

function setLocale(next: LocaleCode) {
  locale.value = next;
  persistLocale(next);
  document.title = t("meta.title");
  const meta = document.querySelector('meta[name="description"]');
  if (meta) meta.setAttribute("content", t("meta.description"));
}

function close() {
  open.value = false;
}
</script>

<template>
  <header
    class="glass-nav sticky top-0 z-[var(--z-nav)] h-16 border-b border-[var(--line)] bg-[var(--nav)] backdrop-blur-xl"
  >
    <div class="mx-auto flex h-full max-w-[1400px] items-center gap-6 px-4 md:px-8">
      <a href="#top" class="flex items-center gap-2 text-[15px] font-semibold tracking-tight">
        <PhClipboard :size="22" weight="regular" class="text-accent" />
        LanPaste
      </a>

      <nav class="ml-auto hidden items-center gap-7 text-[14px] text-[var(--muted)] lg:flex">
        <a href="#features" class="hover:text-[var(--ink)]">{{ t("nav.features") }}</a>
        <a href="#download" class="hover:text-[var(--ink)]">{{ t("nav.download") }}</a>
        <a :href="GITHUB_REPO" target="_blank" rel="noreferrer" class="hover:text-[var(--ink)]">
          {{ t("nav.github") }}
        </a>
      </nav>

      <div class="ml-auto flex items-center gap-2 lg:ml-0">
        <div class="flex rounded-[var(--radius-control)] border border-[var(--line)] p-0.5 text-[12px]">
          <button
            type="button"
            class="cursor-pointer rounded-[4px] px-2 py-1"
            :class="localeCode === 'en-US' ? 'bg-[var(--elevated)] text-[var(--ink)]' : 'text-[var(--muted)]'"
            @click="setLocale('en-US')"
          >
            {{ t("nav.langEn") }}
          </button>
          <button
            type="button"
            class="cursor-pointer rounded-[4px] px-2 py-1"
            :class="localeCode === 'zh-CN' ? 'bg-[var(--elevated)] text-[var(--ink)]' : 'text-[var(--muted)]'"
            @click="setLocale('zh-CN')"
          >
            {{ t("nav.langZh") }}
          </button>
        </div>

        <button
          type="button"
          class="hidden h-9 w-9 cursor-pointer items-center justify-center rounded-[var(--radius-control)] border border-[var(--line)] text-[var(--ink)] lg:inline-flex"
          :aria-label="theme === 'dark' ? t('nav.themeLight') : t('nav.themeDark')"
          @click="toggle"
        >
          <PhSun v-if="theme === 'dark'" :size="16" weight="regular" />
          <PhMoon v-else :size="16" weight="regular" />
        </button>

        <a
          href="#download"
          class="hidden h-9 items-center rounded-[var(--radius-control)] bg-accent px-3 text-[13px] font-medium text-on-accent whitespace-nowrap hover:opacity-90 lg:inline-flex"
        >
          {{ t("nav.download") }}
        </a>

        <button
          type="button"
          class="inline-flex h-9 w-9 cursor-pointer items-center justify-center rounded-[var(--radius-control)] border border-[var(--line)] lg:hidden"
          :aria-label="open ? t('nav.close') : t('nav.menu')"
          :aria-expanded="open"
          @click="open = !open"
        >
          <PhX v-if="open" :size="18" weight="regular" />
          <PhList v-else :size="18" weight="regular" />
        </button>
      </div>
    </div>

    <div
      v-if="open"
      class="absolute inset-x-0 top-16 z-[var(--z-menu)] border-b border-[var(--line)] bg-[var(--canvas)] px-4 py-4 lg:hidden"
    >
      <nav class="flex flex-col gap-3 text-[15px]">
        <a href="#features" @click="close">{{ t("nav.features") }}</a>
        <a :href="GITHUB_REPO" target="_blank" rel="noreferrer" @click="close">
          {{ t("nav.github") }}
        </a>
        <button
          type="button"
          class="flex cursor-pointer items-center gap-2 text-left"
          @click="toggle"
        >
          <PhSun v-if="theme === 'dark'" :size="16" />
          <PhMoon v-else :size="16" />
          {{ theme === "dark" ? t("nav.themeLight") : t("nav.themeDark") }}
        </button>
        <a
          href="#download"
          class="inline-flex h-10 items-center justify-center rounded-[var(--radius-control)] bg-accent font-medium text-on-accent"
          @click="close"
        >
          {{ t("nav.download") }}
        </a>
      </nav>
    </div>
  </header>
</template>
