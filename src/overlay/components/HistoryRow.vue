<script setup lang="ts">
import { computed, ref, watch } from "vue";
import type { HistoryEntry, TransferProgressPayload } from "../../shared/types";
import { formatBytes, hasImagePreview, relativeTime } from "../../shared/format";
import { t } from "../../shared/i18n";
import { mediaSrc } from "../../shared/ipc";
import { TYPE_ICONS } from "../typeMeta";

const props = defineProps<{
  item: HistoryEntry;
  selected: boolean;
  progress: TransferProgressPayload | null;
}>();

const emit = defineEmits<{
  select: [id: string];
  context: [payload: { id: string; x: number; y: number }];
  activate: [id: string];
}>();

const icon = computed(() =>
  hasImagePreview(props.item) ? TYPE_ICONS.image : TYPE_ICONS[props.item.primaryType],
);
const time = computed(() => relativeTime(props.item.copiedAt));
const thumbFailed = ref(false);
const thumb = computed(() => {
  if (props.item.preview.imageThumb) return mediaSrc(props.item.preview.imageThumb);
  if (hasImagePreview(props.item)) return mediaSrc(props.item.preview.path);
  return undefined;
});
const showThumb = computed(() => Boolean(thumb.value) && !thumbFailed.value);
const ratio = computed(() => {
  if (!props.progress || props.progress.id !== props.item.id || props.progress.total <= 0) {
    return 0;
  }
  return Math.min(1, props.progress.received / props.progress.total);
});
const showBar = computed(
  () =>
    props.item.fileDownloadState === "downloading" ||
    (props.progress && props.progress.id === props.item.id && ratio.value > 0 && ratio.value < 1),
);

watch(
  () => [props.item.id, thumb.value],
  () => {
    thumbFailed.value = false;
  },
);

function onContext(ev: MouseEvent) {
  ev.preventDefault();
  emit("context", { id: props.item.id, x: ev.clientX, y: ev.clientY });
}
</script>

<template>
  <div
    class="row"
    role="option"
    :aria-selected="selected"
    :class="{ selected, failed: item.fileDownloadState === 'failed' }"
    :id="`row-${item.id}`"
    @click="emit('select', item.id)"
    @dblclick="emit('activate', item.id)"
    @contextmenu="onContext"
  >
    <span class="glyph">
      <img
        v-if="showThumb"
        class="thumb"
        :src="thumb"
        alt=""
        draggable="false"
        decoding="async"
        @error="thumbFailed = true"
      />
      <span
        v-else-if="item.primaryType === 'color' && item.preview.color"
        class="swatch"
        :style="{ background: item.preview.color }"
      />
      <component
        :is="icon"
        v-else
        :size="16"
        :weight="selected ? 'fill' : 'regular'"
      />
    </span>
    <span class="title">{{ item.title }}</span>
    <span v-if="item.needsFileDownload && !showBar" class="meta pending">
      {{ item.preview.fileSize != null ? formatBytes(item.preview.fileSize) : t("overlay.pendingDownload") }}
    </span>
    <span v-else-if="time" class="meta">{{ time }}</span>
    <div v-if="showBar" class="row-progress">
      <div class="progress-track">
        <div class="progress-fill" :style="{ width: `${ratio * 100}%` }" />
      </div>
    </div>
  </div>
</template>

<style scoped>
.row {
  position: relative;
  display: flex;
  align-items: center;
  gap: var(--space-md);
  height: 38px;
  padding: 0 8px;
  border-radius: var(--radius-row);
  cursor: pointer;
  user-select: none;
}

.row:hover:not(.selected) {
  background: var(--color-elevated);
}

.row.selected {
  background: var(--color-accent);
  color: var(--color-on-accent);
}

.glyph {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 24px;
  height: 24px;
  flex-shrink: 0;
  color: inherit;
}

.thumb {
  width: 24px;
  height: 24px;
  object-fit: cover;
  border-radius: 4px;
  background: rgba(255, 255, 255, 0.06);
  border: 1px solid rgba(255, 255, 255, 0.12);
}

.row.selected .thumb {
  border-color: rgba(255, 255, 255, 0.4);
}

.swatch {
  width: 12px;
  height: 12px;
  border-radius: 3px;
  border: 1px solid rgba(255, 255, 255, 0.2);
}

.title {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  font-size: 13px;
}

.meta {
  flex-shrink: 0;
  font-size: 11px;
  color: var(--color-muted);
}

.row.selected .meta {
  color: rgba(255, 255, 255, 0.8);
}

.pending {
  font-variant-numeric: tabular-nums;
}

.row-progress {
  position: absolute;
  left: 8px;
  right: 8px;
  bottom: 2px;
}

.row.failed:not(.selected) {
  color: var(--color-destructive);
}
</style>
