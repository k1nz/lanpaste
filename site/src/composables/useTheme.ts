import { onMounted, ref, watch } from "vue";

export type ThemeMode = "light" | "dark";

const STORAGE_KEY = "lanpaste-theme";

function readStored(): ThemeMode {
  try {
    const stored = localStorage.getItem(STORAGE_KEY);
    if (stored === "light" || stored === "dark") return stored;
  } catch {
    /* ignore */
  }
  return "dark";
}

function applyClass(mode: ThemeMode) {
  document.documentElement.classList.toggle("dark", mode === "dark");
}

export function useTheme() {
  const theme = ref<ThemeMode>(readStored());

  function setTheme(next: ThemeMode) {
    theme.value = next;
  }

  function toggle() {
    theme.value = theme.value === "dark" ? "light" : "dark";
  }

  watch(
    theme,
    (mode) => {
      applyClass(mode);
      try {
        localStorage.setItem(STORAGE_KEY, mode);
      } catch {
        /* ignore */
      }
    },
    { immediate: true },
  );

  onMounted(() => {
    theme.value = readStored();
    applyClass(theme.value);
  });

  return { theme, setTheme, toggle };
}
