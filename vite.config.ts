import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";
// @ts-expect-error type error without @types/node package
import process from "node:process";
// @ts-expect-error type error without @types/node package
import { readFileSync } from "node:fs";
const host = process.env.TAURI_DEV_HOST;

// package.json is the version the release workflow gates the tag against, so
// the UI reads it from here instead of repeating the number in a component.
// @ts-expect-error type error without @types/node package
const { version } = JSON.parse(readFileSync(new URL("./package.json", import.meta.url), "utf8"));

// https://vite.dev/config/
export default defineConfig(() => ({
  plugins: [vue()],

  define: {
    __APP_VERSION__: JSON.stringify(version),
  },

  // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
  //
  // 1. prevent Vite from obscuring rust errors
  clearScreen: false,
  // 2. tauri expects a fixed port, fail if that port is not available
  server: {
    port: 1480,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1481,
        }
      : undefined,
    watch: {
      // 3. tell Vite to ignore watching `src-tauri`
      ignored: ["**/src-tauri/**"],
    },
  },
}));
