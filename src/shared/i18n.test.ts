import { beforeEach, describe, expect, it } from "vitest";
import enUS from "../locales/en-US.json";
import zhCN from "../locales/zh-CN.json";
import { displaySource, displayTitle, resolveLocale, setResolvedLocale, t } from "./i18n";

beforeEach(() => {
  setResolvedLocale("en-US");
});

describe("locale catalogs", () => {
  it("carry the same key set, so neither language can silently lose a string", () => {
    const en = Object.keys(enUS).sort();
    const zh = Object.keys(zhCN).sort();
    expect(zh.filter((k) => !(k in enUS))).toEqual([]);
    expect(en.filter((k) => !(k in zhCN))).toEqual([]);
    expect(zh).toEqual(en);
  });

  it("has no empty values", () => {
    const empty = Object.entries({ ...enUS, ...zhCN })
      .filter(([, value]) => typeof value !== "string" || value.trim() === "")
      .map(([key]) => key);
    expect(empty).toEqual([]);
  });

  it("translates rather than copying the English through", () => {
    // A handful of keys are legitimately identical across languages (product
    // name, key names). Sample keys that must differ.
    for (const key of ["overlay.searchPlaceholder", "overlay.delete", "err.offline"] as const) {
      expect(zhCN[key]).not.toBe(enUS[key]);
    }
  });
});

describe("resolveLocale", () => {
  it("honours an explicit choice", () => {
    expect(resolveLocale("zh-CN", "en-US")).toBe("zh-CN");
    expect(resolveLocale("en-US", "zh-CN")).toBe("en-US");
  });

  it("follows the system when asked to", () => {
    expect(resolveLocale("system", "zh-CN")).toBe("zh-CN");
    expect(resolveLocale("system", "en-US")).toBe("en-US");
  });
});

describe("t", () => {
  it("fills in placeholders", () => {
    expect(t("overlay.pasteTo", { app: "Finder" })).toContain("Finder");
  });
});

describe("displayTitle", () => {
  it("shows the text behind an opaque RTF or HTML label", () => {
    expect(displayTitle(enUS["type.rtf"], "rtf", { text: "quarterly  numbers" })).toBe(
      "quarterly numbers",
    );
    expect(displayTitle("<p>hello</p>", "html", { text: "hello" })).toBe("hello");
  });

  it("leaves a real title alone", () => {
    expect(displayTitle("Meeting notes", "text", { text: "ignored" })).toBe("Meeting notes");
  });

  it("keeps a generic label generic", () => {
    expect(displayTitle(enUS["overlay.genericClipboard"], "text")).toBe(
      t("overlay.genericClipboard"),
    );
    expect(displayTitle(enUS["type.image"], "image")).toBe(t("type.image"));
  });
});

describe("displaySource", () => {
  it("labels items that came from this machine", () => {
    expect(displaySource(null)).toBe(t("overlay.local"));
    expect(displaySource(enUS["overlay.local"])).toBe(t("overlay.local"));
  });

  it("names a peer as the peer is named locally", () => {
    expect(displaySource("Studio Mac")).toBe("Studio Mac");
  });
});
