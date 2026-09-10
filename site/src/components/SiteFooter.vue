<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { PhClipboard } from "@phosphor-icons/vue";
import { GITHUB_RELEASES, GITHUB_REPO } from "../constants";

const { t } = useI18n();
</script>

<template>
  <footer class="border-t border-[var(--line)] px-4 py-12 md:px-8 md:py-16">
    <div
      class="mx-auto flex max-w-[1400px] flex-col gap-8 md:flex-row md:items-end md:justify-between"
    >
      <div>
        <p class="flex items-center gap-2 text-[15px] font-semibold tracking-tight">
          <PhClipboard :size="18" weight="regular" class="text-accent" />
          LanPaste
        </p>
        <p class="mt-3 max-w-[42ch] text-sm leading-relaxed text-[var(--muted)]">
          {{ t("footer.tagline") }}
        </p>
      </div>
      <nav class="flex flex-wrap gap-6 text-sm text-[var(--muted)]">
        <a :href="GITHUB_REPO" target="_blank" rel="noreferrer" class="hover:text-[var(--ink)]">
          {{ t("footer.source") }}
        </a>
        <a :href="GITHUB_RELEASES" target="_blank" rel="noreferrer" class="hover:text-[var(--ink)]">
          {{ t("footer.releases") }}
        </a>
        <span>{{ t("footer.license") }}</span>
      </nav>
    </div>
  </footer>
</template>
