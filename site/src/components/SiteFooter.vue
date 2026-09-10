<script setup lang="ts">
import { useI18n } from "vue-i18n";
import { PhClipboard } from "@phosphor-icons/vue";
import { GITHUB_RELEASES, GITHUB_REPO } from "../constants";

const { t } = useI18n();
</script>

<template>
  <footer class="border-t border-[var(--line)] px-4 py-10 md:px-8">
    <div
      class="mx-auto flex max-w-[1400px] flex-col gap-6 md:flex-row md:items-end md:justify-between"
    >
      <div>
        <p class="flex items-center gap-2 font-semibold tracking-tight">
          <PhClipboard :size="18" weight="regular" class="text-accent" />
          LanPaste
        </p>
        <p class="mt-2 max-w-[42ch] text-sm text-[var(--muted)]">
          {{ t("footer.tagline") }}
        </p>
      </div>
      <nav class="flex flex-wrap gap-5 text-sm text-[var(--muted)]">
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
