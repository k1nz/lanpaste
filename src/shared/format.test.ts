import { afterEach, describe, expect, it, vi } from "vitest";
import {
  eventToShortcut,
  formatBytes,
  formatShortcut,
  hasImagePreview,
  isImageFileName,
  parseShortcut,
  relativeTime,
  toMs,
} from "./format";
import type { HistoryEntry } from "./types";

function entry(overrides: Partial<HistoryEntry> = {}): HistoryEntry {
  return {
    id: "e1",
    copiedAt: Date.now(),
    sourceDeviceId: null,
    sourceDeviceName: "This computer",
    primaryType: "text",
    title: "hello",
    preview: {},
    needsFileDownload: false,
    fileDownloadState: "idle",
    ...overrides,
  };
}

describe("toMs", () => {
  it("promotes second-precision timestamps to milliseconds", () => {
    expect(toMs(1_700_000_000)).toBe(1_700_000_000_000);
  });

  it("leaves millisecond timestamps alone", () => {
    expect(toMs(1_700_000_000_000)).toBe(1_700_000_000_000);
  });
});

describe("formatBytes", () => {
  it("scales through the units", () => {
    expect(formatBytes(0)).toBe("0 B");
    expect(formatBytes(512)).toBe("512 B");
    expect(formatBytes(1024)).toBe("1.0 KB");
    expect(formatBytes(1024 * 1024)).toBe("1.0 MB");
    expect(formatBytes(1024 * 1024 * 1024)).toBe("1.0 GB");
  });

  it("drops the decimal once the number is large enough not to need it", () => {
    expect(formatBytes(20 * 1024)).toBe("20 KB");
    expect(formatBytes(20 * 1024 * 1024)).toBe("20 MB");
  });

  it("renders unusable input as a dash rather than NaN", () => {
    expect(formatBytes(-1)).toBe("—");
    expect(formatBytes(Number.POSITIVE_INFINITY)).toBe("—");
  });
});

describe("relativeTime", () => {
  it("stays quiet for very recent items", () => {
    expect(relativeTime(Date.now())).toBe("");
  });

  it("reports minutes and hours", () => {
    expect(relativeTime(Date.now() - 10 * 60_000)).toBe("10m");
    expect(relativeTime(Date.now() - 2 * 3_600_000)).toBe("2h");
  });

  it("gives up past a day, where the date group label takes over", () => {
    expect(relativeTime(Date.now() - 2 * 86_400_000)).toBe("");
  });

  it("accepts second-precision timestamps", () => {
    const tenMinutesAgo = Math.floor((Date.now() - 10 * 60_000) / 1000);
    expect(relativeTime(tenMinutesAgo)).toBe("10m");
  });
});

describe("parseShortcut", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  function asWindows() {
    vi.stubGlobal("navigator", { platform: "Win32", userAgent: "Windows" });
  }

  function asMac() {
    vi.stubGlobal("navigator", { platform: "MacIntel", userAgent: "Mac OS X" });
  }

  it("splits a chord into its keys", () => {
    expect(parseShortcut("Option+Shift+V")).toEqual(["option", "shift", "V"]);
  });

  it("normalises the alternative spellings Tauri accepts", () => {
    expect(parseShortcut("Alt+Shift+KeyV")).toEqual(["option", "shift", "V"]);
    expect(parseShortcut("Cmd+Digit1")).toEqual(["command", "1"]);
    expect(parseShortcut("Control+ArrowUp")).toEqual(["control", "Up"]);
  });

  it("resolves CommandOrControl against the platform", () => {
    asWindows();
    expect(parseShortcut("CommandOrControl+Shift+V")).toEqual(["control", "shift", "V"]);
    asMac();
    expect(parseShortcut("CommandOrControl+Shift+V")).toEqual(["command", "shift", "V"]);
  });

  it("ignores empty segments", () => {
    expect(parseShortcut("Shift++V")).toEqual(["shift", "V"]);
  });
});

describe("formatShortcut", () => {
  it("spells the keys out for assistive technology", () => {
    expect(formatShortcut("Option+Shift+V")).toBe("Option Shift V");
  });
});

describe("eventToShortcut", () => {
  function key(init: Partial<KeyboardEvent>): KeyboardEvent {
    return init as KeyboardEvent;
  }

  it("builds a Tauri chord from a keydown", () => {
    expect(
      eventToShortcut(key({ key: "v", code: "KeyV", metaKey: true, shiftKey: true })),
    ).toBe("Command+Shift+V");
  });

  it("refuses a bare key that would shadow normal typing", () => {
    expect(eventToShortcut(key({ key: "v", code: "KeyV" }))).toBeNull();
  });

  it("waits for a non-modifier key", () => {
    expect(eventToShortcut(key({ key: "Shift", code: "ShiftLeft", shiftKey: true }))).toBeNull();
  });

  it("allows a function key with no modifier", () => {
    expect(eventToShortcut(key({ key: "F5", code: "F5" }))).toBe("F5");
  });
});

describe("image detection", () => {
  it("recognises image filenames regardless of case or directory", () => {
    expect(isImageFileName("photo.PNG")).toBe(true);
    expect(isImageFileName("/tmp/shots/capture.jpeg")).toBe(true);
    expect(isImageFileName("notes.txt")).toBe(false);
    expect(isImageFileName(undefined)).toBe(false);
  });

  it("treats a thumbnailed entry as previewable", () => {
    expect(hasImagePreview(entry({ preview: { imageThumb: "data:image/png;base64,AA" } }))).toBe(
      true,
    );
    expect(hasImagePreview(entry({ preview: { fileName: "shot.png" } }))).toBe(true);
    expect(hasImagePreview(entry({ preview: { text: "plain" } }))).toBe(false);
  });
});
