<script setup lang="ts">
import { computed, nextTick, ref, watch } from "vue";
import type { HistoryEntry, TransferProgressPayload } from "../../shared/types";
import { t } from "../../shared/i18n";
import HistoryRow from "./HistoryRow.vue";
import {
  flattenGroups,
  groupHistory,
  rowHeight,
  type FlatRow,
} from "../groupHistory";

const props = defineProps<{
  items: HistoryEntry[];
  selectedId: string | null;
  progress: TransferProgressPayload | null;
}>();

const emit = defineEmits<{
  select: [id: string];
  context: [payload: { id: string; x: number; y: number }];
  activate: [id: string];
  clearFilters: [];
}>();

const scrollEl = ref<HTMLElement | null>(null);
const scrollTop = ref(0);
const viewportH = ref(400);

const groups = computed(() => groupHistory(props.items));
const flat = computed(() => flattenGroups(groups.value));
const virtualize = computed(() => props.items.length > 200);

interface Laid {
  row: FlatRow;
  top: number;
  height: number;
}

const laid = computed(() => {
  let top = 0;
  return flat.value.map((row) => {
    const height = rowHeight(row);
    const item: Laid = { row, top, height };
    top += height;
    return item;
  });
});

const totalH = computed(() => {
  const last = laid.value[laid.value.length - 1];
  return last ? last.top + last.height : 0;
});

const OVERSCAN = 38 * 8;

const visibleLaid = computed(() => {
  if (!virtualize.value) return laid.value;
  const start = scrollTop.value - OVERSCAN;
  const end = scrollTop.value + viewportH.value + OVERSCAN;
  return laid.value.filter((l) => l.top + l.height >= start && l.top <= end);
});

const offsetY = computed(() => visibleLaid.value[0]?.top ?? 0);

function onScroll() {
  const el = scrollEl.value;
  if (!el) return;
  scrollTop.value = el.scrollTop;
  viewportH.value = el.clientHeight;
}

watch(
  () => props.selectedId,
  async (id) => {
    if (!id) return;
    await nextTick();
    const found = laid.value.find((l) => l.row.kind === "item" && l.row.item.id === id);
    const el = scrollEl.value;
    if (!found || !el) return;
    const top = found.top;
    const bottom = found.top + found.height;
    if (top < el.scrollTop) el.scrollTop = top;
    else if (bottom > el.scrollTop + el.clientHeight) {
      el.scrollTop = bottom - el.clientHeight;
    }
  },
);

function onListScrollSetup(el: Element | { $el?: Element } | null) {
  const node = el && "$el" in el ? el.$el : el;
  scrollEl.value = node instanceof HTMLElement ? node : null;
  if (scrollEl.value) viewportH.value = scrollEl.value.clientHeight;
}
</script>

<template>
  <div
    class="list-scroll"
    :ref="onListScrollSetup"
    role="listbox"
    :aria-activedescendant="selectedId ? `row-${selectedId}` : undefined"
    @scroll="onScroll"
  >
    <div v-if="items.length === 0" class="empty">
      <p class="empty-title">{{ t("overlay.emptyTitle") }}</p>
      <button type="button" class="empty-hint" @click="emit('clearFilters')">
        {{ t("overlay.emptyHint") }}
      </button>
    </div>

    <template v-else-if="!virtualize">
      <section v-for="group in groups" :key="group.id" class="group">
        <div class="group-label">{{ group.label }}</div>
        <HistoryRow
          v-for="item in group.items"
          :key="item.id"
          v-memo="[item.id, item.id === selectedId, item.title, item.fileDownloadState, progress?.id === item.id ? progress.received : 0]"
          :item="item"
          :selected="item.id === selectedId"
          :progress="progress"
          @select="emit('select', $event)"
          @context="emit('context', $event)"
          @activate="emit('activate', $event)"
        />
      </section>
    </template>

    <div v-else class="virtual-body" :style="{ height: `${totalH}px` }">
      <div class="virtual-window" :style="{ transform: `translateY(${offsetY}px)` }">
        <template v-for="laidRow in visibleLaid" :key="laidRow.row.key">
          <div v-if="laidRow.row.kind === 'header'" class="group-label">
            {{ laidRow.row.label }}
          </div>
          <HistoryRow
            v-else-if="laidRow.row.kind === 'item'"
            v-memo="[laidRow.row.item.id, laidRow.row.item.id === selectedId, laidRow.row.item.title, laidRow.row.item.fileDownloadState, progress?.id === laidRow.row.item.id ? progress.received : 0]"
            :item="laidRow.row.item"
            :selected="laidRow.row.item.id === selectedId"
            :progress="progress"
            @select="emit('select', $event)"
            @context="emit('context', $event)"
            @activate="emit('activate', $event)"
          />
        </template>
      </div>
    </div>
  </div>
</template>

<style scoped>
.list-scroll {
  height: 100%;
  overflow: auto;
  padding: 0 8px 8px;
}

.list-scroll::-webkit-scrollbar {
  width: 8px;
}

.list-scroll::-webkit-scrollbar-thumb {
  background: var(--color-border);
  border-radius: 99px;
}

.group-label {
  z-index: 1;
  height: 22px;
  display: flex;
  align-items: center;
  margin: 0 -8px;
  padding: 0 16px;
  font-size: 11px;
  color: var(--color-muted);
  background-color: transparent;
}

.virtual-body {
  position: relative;
}

.virtual-window {
  will-change: transform;
}

.empty {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  height: 100%;
  gap: var(--space-md);
  text-align: center;
}

.empty-title {
  margin: 0;
  font-size: 15px;
  color: var(--color-foreground);
}

.empty-hint {
  font-size: 12px;
  color: var(--color-muted);
  cursor: pointer;
  transition: opacity 160ms ease;
}

.empty-hint:hover {
  color: var(--color-foreground);
}
</style>
