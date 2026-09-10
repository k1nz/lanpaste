<script setup lang="ts">
import { computed, ref } from "vue";
import { useI18n } from "vue-i18n";
import {
  PhClipboard,
  PhFile,
  PhFileText,
  PhImage,
  PhLink,
  PhMagnifyingGlass,
  PhPalette,
} from "@phosphor-icons/vue";
import clipboardStill from "@/assets/clipboard-still.png";
import codeThumb from "@/assets/code-thumb.png";
import { OVERLAY_HEIGHT, OVERLAY_WIDTH } from "../constants";
import Keycap from "./Keycap.vue";

type ItemType = "url" | "color" | "image" | "text" | "file";

interface DemoItem {
  id: string;
  type: ItemType;
  titleKey: string;
  timeKey: string;
  sourceKey: string;
  color?: string;
  image?: string;
  sizeKey?: string;
  pending?: boolean;
}

const { t } = useI18n();
const selectedId = ref("image");

const items: DemoItem[] = [
  {
    id: "url",
    type: "url",
    titleKey: "hero.itemUrl",
    timeKey: "hero.time2m",
    sourceKey: "hero.sourceStudio",
  },
  {
    id: "color",
    type: "color",
    titleKey: "hero.itemColor",
    timeKey: "hero.time6m",
    sourceKey: "hero.sourceLocal",
    color: "#0A84FF",
  },
  {
    id: "image",
    type: "image",
    titleKey: "hero.itemImage",
    timeKey: "hero.time18m",
    sourceKey: "hero.sourceOffice",
    image: clipboardStill,
  },
  {
    id: "shot",
    type: "image",
    titleKey: "hero.itemShot",
    timeKey: "hero.time27m",
    sourceKey: "hero.sourceStudio",
    image: codeThumb,
  },
  {
    id: "text",
    type: "text",
    titleKey: "hero.itemText",
    timeKey: "hero.time41m",
    sourceKey: "hero.sourceStudio",
  },
  {
    id: "ssh",
    type: "text",
    titleKey: "hero.itemSsh",
    timeKey: "hero.time1h",
    sourceKey: "hero.sourceOffice",
  },
  {
    id: "env",
    type: "text",
    titleKey: "hero.itemEnv",
    timeKey: "hero.time2h",
    sourceKey: "hero.sourceLocal",
  },
  {
    id: "file",
    type: "file",
    titleKey: "hero.itemFile",
    timeKey: "hero.time3h",
    sourceKey: "hero.sourceOffice",
    sizeKey: "hero.fileSize",
    pending: true,
  },
];

const selected = computed(() => items.find((item) => item.id === selectedId.value) ?? items[0]);

const typeLabel: Record<ItemType, string> = {
  url: "hero.typeUrl",
  color: "hero.typeColor",
  image: "hero.typeImage",
  text: "hero.typeText",
  file: "hero.typeFile",
};

function iconFor(type: ItemType) {
  switch (type) {
    case "url":
      return PhLink;
    case "color":
      return PhPalette;
    case "image":
      return PhImage;
    case "file":
      return PhFile;
    default:
      return PhFileText;
  }
}
</script>

<template>
  <div
    class="overlay-stage"
    :style="{
      '--overlay-w': `${OVERLAY_WIDTH}px`,
      '--overlay-h': `${OVERLAY_HEIGHT}px`,
    }"
  >
    <div class="overlay" role="region" :aria-label="t('hero.overlayAria')">
    <header class="search">
      <PhMagnifyingGlass :size="16" weight="regular" class="search-icon" />
      <span class="placeholder">{{ t("hero.overlaySearch") }}</span>
    </header>
    <div class="split">
      <div class="list">
        <p class="group">{{ t("hero.overlayToday") }}</p>
        <button
          v-for="item in items"
          :key="item.id"
          type="button"
          class="row"
          :class="{ selected: item.id === selectedId }"
          @click="selectedId = item.id"
        >
          <span class="glyph">
            <img
              v-if="item.image"
              :src="item.image"
              alt=""
              class="thumb"
              width="24"
              height="24"
            />
            <span
              v-else-if="item.color"
              class="swatch"
              :style="{ background: item.color }"
            />
            <component
              :is="iconFor(item.type)"
              v-else
              :size="16"
              :weight="item.id === selectedId ? 'fill' : 'regular'"
            />
          </span>
          <span class="title">{{ t(item.titleKey) }}</span>
          <span class="meta">{{ item.pending ? t(item.sizeKey!) : t(item.timeKey) }}</span>
        </button>
      </div>
      <aside class="preview">
        <div class="preview-body">
          <div v-if="selected.color" class="color-block">
            <div class="color-swatch" :style="{ background: selected.color }" />
            <code>{{ selected.color }}</code>
          </div>
          <div v-else-if="selected.image" class="image-block">
            <img
              :src="selected.image"
              :alt="t(selected.titleKey)"
              class="preview-image"
            />
          </div>
          <p v-else-if="selected.type === 'file'" class="file-block">
            <span class="file-name">{{ t(selected.titleKey) }}</span>
            <span class="muted">{{ t("hero.fileSize") }} · {{ t("hero.pending") }}</span>
          </p>
          <p v-else class="text-block">{{ t(selected.titleKey) }}</p>
        </div>
        <div class="info">
          <h3>{{ t("hero.previewInfo") }}</h3>
          <dl>
            <div>
              <dt>{{ t("hero.previewSource") }}</dt>
              <dd>{{ t(selected.sourceKey) }}</dd>
            </div>
            <div>
              <dt>{{ t("hero.previewType") }}</dt>
              <dd>{{ t(typeLabel[selected.type]) }}</dd>
            </div>
            <div v-if="selected.sizeKey">
              <dt>{{ t("hero.previewSize") }}</dt>
              <dd>{{ t(selected.sizeKey) }}</dd>
            </div>
          </dl>
        </div>
      </aside>
    </div>
    <footer class="bar">
      <div class="lead">
        <PhClipboard :size="20" weight="regular" />
        <span>{{ t("hero.overlayHistory") }}</span>
      </div>
      <span class="bar-action">
        {{ t("hero.overlayPaste") }}
        <Keycap k="enter" decorative />
      </span>
      <span class="bar-sep" aria-hidden="true" />
      <span class="bar-action">
        {{ t("hero.overlayActions") }}
        <span class="chord">
          <Keycap k="command" decorative />
          <Keycap k="K" decorative />
        </span>
      </span>
    </footer>
  </div>
  </div>
</template>

<style scoped>
.overlay-stage {
  container-type: inline-size;
  width: min(100%, var(--overlay-w, 780px));
  aspect-ratio: 780 / 520;
  overflow: hidden;
}

.overlay {
  --ov-bg: rgb(28 28 30 / 0.82);
  --ov-fg: #ededef;
  --ov-muted: #8a8f98;
  --ov-line: rgb(255 255 255 / 0.08);
  --ov-elev: rgb(255 255 255 / 0.06);
  --keycap-bg: rgb(255 255 255 / 0.1);
  --keycap-border: rgb(255 255 255 / 0.14);
  --keycap-fg: #c8ccd4;
  --keycap-shine: rgb(255 255 255 / 0.16);
  width: var(--overlay-w, 780px);
  height: var(--overlay-h, 520px);
  transform-origin: top left;
  transform: scale(calc(100cqw / var(--overlay-w, 780px)));
  display: flex;
  flex-direction: column;
  overflow: hidden;
  color: var(--ov-fg);
  background: var(--ov-bg);
  border: 1px solid var(--ov-line);
  border-radius: 12px;
  box-shadow:
    inset 0 1px 0 rgb(255 255 255 / 0.12),
    0 24px 72px rgb(0 0 0 / 0.52);
  backdrop-filter: blur(20px) saturate(140%);
  -webkit-backdrop-filter: blur(20px) saturate(140%);
  font-family:
    -apple-system, BlinkMacSystemFont, "SF Pro Text", var(--font-sans), sans-serif;
  font-size: 13px;
  line-height: 1.35;
  text-align: left;
}

@media (prefers-reduced-transparency: reduce) {
  .overlay {
    background: #1c1c1e;
    backdrop-filter: none;
    -webkit-backdrop-filter: none;
  }
}

.search {
  display: flex;
  align-items: center;
  gap: 8px;
  height: 44px;
  padding: 0 12px 0 14px;
  border-bottom: 1px solid var(--ov-line);
  flex-shrink: 0;
}

.search-icon,
.placeholder {
  color: var(--ov-muted);
}

.placeholder {
  font-size: 15px;
}

.split {
  flex: 1;
  min-height: 0;
  display: grid;
  grid-template-columns: 1.15fr 0.85fr;
}

.list {
  min-width: 0;
  overflow: auto;
  padding: 8px;
}

.group {
  margin: 4px 8px 6px;
  font-size: 11px;
  color: var(--ov-muted);
}

.row {
  display: flex;
  align-items: center;
  gap: 8px;
  width: 100%;
  height: 38px;
  padding: 0 8px;
  border: 0;
  border-radius: 6px;
  background: transparent;
  color: inherit;
  font: inherit;
  cursor: pointer;
  text-align: left;
}

.row:hover:not(.selected) {
  background: var(--ov-elev);
}

.row.selected {
  background: #0a84ff;
  color: #fff;
}

.glyph {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 24px;
  height: 24px;
  flex-shrink: 0;
}

.thumb {
  width: 24px;
  height: 24px;
  object-fit: cover;
  border-radius: 4px;
}

.swatch {
  width: 12px;
  height: 12px;
  border-radius: 3px;
  border: 1px solid rgb(255 255 255 / 0.2);
}

.title {
  flex: 1;
  min-width: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.meta {
  flex-shrink: 0;
  font-size: 11px;
  color: var(--ov-muted);
}

.row.selected .meta {
  color: rgb(255 255 255 / 0.8);
}

.preview {
  min-width: 0;
  display: flex;
  flex-direction: column;
  border-left: 1px solid var(--ov-line);
  padding: 12px;
  gap: 12px;
}

.preview-body {
  flex: 1;
  min-height: 0;
}

.color-block {
  display: flex;
  flex-direction: column;
  gap: 8px;
}

.color-swatch {
  height: 120px;
  border-radius: 8px;
  border: 1px solid var(--ov-line);
}

.color-block code,
.file-name,
.text-block {
  font-family: var(--font-mono);
  font-size: 12px;
}

.image-block {
  display: flex;
  flex-direction: column;
  gap: 8px;
  min-height: 0;
}

.preview-image {
  max-width: 100%;
  max-height: 220px;
  object-fit: contain;
  border-radius: 6px;
}

.file-block,
.text-block {
  margin: 0;
  display: flex;
  flex-direction: column;
  gap: 6px;
}

.muted {
  color: var(--ov-muted);
  font-size: 12px;
}

.info {
  flex-shrink: 0;
}

.info h3 {
  margin: 0 0 8px;
  font-size: 11px;
  font-weight: 600;
  color: var(--ov-muted);
}

.info dl {
  margin: 0;
  display: grid;
  gap: 6px;
}

.info div {
  display: grid;
  grid-template-columns: 64px 1fr;
  gap: 8px;
  font-size: 12px;
}

.info dt {
  color: var(--ov-muted);
}

.info dd {
  margin: 0;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.bar {
  display: flex;
  align-items: center;
  gap: 8px;
  height: 36px;
  padding: 0 12px;
  border-top: 1px solid var(--ov-line);
  flex-shrink: 0;
}

.lead {
  display: flex;
  align-items: center;
  gap: 6px;
  margin-right: auto;
  color: var(--ov-muted);
  font-size: 12px;
}

.bar-action {
  display: inline-flex;
  align-items: center;
  gap: 6px;
  font-size: 12px;
}

.bar-sep {
  width: 1px;
  height: 12px;
  background: var(--ov-line);
}

.chord {
  display: inline-flex;
  align-items: center;
  gap: 3px;
}
</style>
