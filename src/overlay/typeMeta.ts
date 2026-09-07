import {
  PhCode,
  PhFile,
  PhFileText,
  PhImage,
  PhLink,
  PhPalette,
  PhTextT,
} from "@phosphor-icons/vue";
import type { PasteType } from "../shared/types";
import type { Component } from "vue";

export const PASTE_TYPES: PasteType[] = [
  "text",
  "url",
  "color",
  "html",
  "rtf",
  "image",
  "file",
];

export const TYPE_ICONS: Record<PasteType, Component> = {
  text: PhTextT,
  url: PhLink,
  color: PhPalette,
  html: PhCode,
  rtf: PhFileText,
  image: PhImage,
  file: PhFile,
};
