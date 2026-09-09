<script setup lang="ts">
import { computed } from "vue";
import type { HistoryEntry, TransferProgressPayload } from "../../shared/types";
import { formatBytes, hasImagePreview, typeLabel } from "../../shared/format";
import { displaySource, displayTitle, t } from "../../shared/i18n";
import { mediaSrc } from "../../shared/ipc";

const props = defineProps<{
  entry: HistoryEntry | null;
  progress: TransferProgressPayload | null;
}>();

const imageSrc = computed(() => {
  const e = props.entry;
  if (!e || !hasImagePreview(e)) return undefined;
  return mediaSrc(e.preview.path) || mediaSrc(e.preview.imageThumb);
});
const ratio = computed(() => {
  const p = props.progress;
  const e = props.entry;
  if (!p || !e || p.id !== e.id || p.total <= 0) return 0;
  return Math.min(1, p.received / p.total);
});
const transferring = computed(() => {
  const e = props.entry;
  return Boolean(
    e &&
      (e.fileDownloadState === "downloading" ||
        (props.progress && props.progress.id === e.id && ratio.value < 1)),
  );
});
</script>

<template>
  <aside class="preview" v-if="entry">
    <div class="preview-body lp-scroll">
      <div v-if="entry.primaryType === 'color' && entry.preview.color" class="color-block">
        <div class="color-swatch" :style="{ background: entry.preview.color }" />
        <code>{{ entry.preview.color }}</code>
      </div>
      <div v-else-if="imageSrc" class="image-block">
        <img :src="imageSrc" :alt="displayTitle(entry.title, entry.primaryType)" />
        <p v-if="entry.primaryType === 'file'" class="file-name">
          {{ entry.preview.fileName || displayTitle(entry.title, entry.primaryType) }}
        </p>
      </div>
      <div v-else-if="entry.primaryType === 'url' && entry.preview.url" class="text-block url">
        {{ entry.preview.url }}
      </div>
      <div v-else-if="entry.primaryType === 'file'" class="file-block">
        <p class="file-name">{{ entry.preview.fileName || displayTitle(entry.title, entry.primaryType) }}</p>
        <p v-if="entry.preview.fileSize != null" class="muted">
          {{ formatBytes(entry.preview.fileSize) }}
        </p>
        <p v-if="entry.needsFileDownload" class="muted">{{ t("overlay.pendingDownload") }}</p>
      </div>
      <pre v-else-if="entry.preview.text || entry.preview.html" class="text-block">{{
        entry.preview.text || entry.preview.html
      }}</pre>
      <p v-else class="muted">{{ t("overlay.noPreview") }}</p>
    </div>

    <div v-if="transferring" class="transfer">
      <div class="progress-track">
        <div class="progress-fill" :style="{ width: `${ratio * 100}%` }" />
      </div>
    </div>
    <p v-if="entry.fileDownloadState === 'failed'" class="live-error" role="alert">
      {{ t("overlay.transferFailed") }}
    </p>

    <div class="info">
      <h3>{{ t("overlay.information") }}</h3>
      <dl>
        <div>
          <dt>{{ t("overlay.source") }}</dt>
          <dd>{{ displaySource(entry.sourceDeviceName) }}</dd>
        </div>
        <div>
          <dt>{{ t("overlay.type") }}</dt>
          <dd>{{ typeLabel(entry.primaryType) }}</dd>
        </div>
        <div v-if="entry.primaryType === 'file' && entry.preview.path">
          <dt>{{ t("overlay.path") }}</dt>
          <dd class="mono">{{ entry.preview.path }}</dd>
        </div>
        <div v-if="entry.preview.width && entry.preview.height">
          <dt>{{ t("overlay.dimensions") }}</dt>
          <dd>{{ entry.preview.width }} × {{ entry.preview.height }}</dd>
        </div>
        <div v-if="entry.preview.fileSize != null">
          <dt>{{ t("overlay.size") }}</dt>
          <dd>{{ formatBytes(entry.preview.fileSize) }}</dd>
        </div>
      </dl>
    </div>
  </aside>
  <aside v-else class="preview empty">
    <p class="muted">{{ t("overlay.noPreview") }}</p>
  </aside>
</template>

<style scoped>
.preview {
  display: flex;
  flex-direction: column;
  min-width: 0;
  min-height: 0;
  height: 100%;
  overflow: hidden;
  border-left: 1px solid var(--color-border);
  padding: var(--space-lg);
  gap: var(--space-lg);
}

.preview.empty {
  align-items: center;
  justify-content: center;
}

.preview-body {
  flex: 1;
  min-height: 0;
}

.muted {
  color: var(--color-muted);
  margin: 0;
}

.color-block {
  display: flex;
  flex-direction: column;
  gap: var(--space-md);
}

.color-swatch {
  height: 120px;
  border-radius: 8px;
  border: 1px solid var(--color-border);
}

.color-block code,
.mono,
.url {
  font-family: var(--font-mono);
  font-size: 12px;
  word-break: break-all;
}

.image-block {
  display: flex;
  flex-direction: column;
  gap: var(--space-md);
}

.image-block img {
  max-width: 100%;
  max-height: 220px;
  object-fit: contain;
  border-radius: 6px;
}

.text-block {
  margin: 0;
  font-size: 13px;
  white-space: pre-wrap;
  word-break: break-word;
  user-select: text;
  font-family: var(--font-ui);
}

.file-name {
  margin: 0 0 var(--space-sm);
  font-size: 15px;
}

.info {
  flex-shrink: 0;
}

.info h3 {
  margin: 0 0 var(--space-md);
  font-size: 11px;
  font-weight: 600;
  color: var(--color-muted);
  text-transform: none;
}

.info dl {
  margin: 0;
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.info dl > div {
  display: grid;
  grid-template-columns: 64px 1fr;
  gap: var(--space-md);
  font-size: 12px;
}

.info dt {
  color: var(--color-muted);
}

.info dd {
  margin: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}
</style>
