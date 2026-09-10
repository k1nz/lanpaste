import { beforeEach, describe, expect, it } from "vitest";
import { localizeError } from "./errors";
import { setResolvedLocale, t } from "./i18n";

beforeEach(() => {
  setResolvedLocale("en-US");
});

describe("localizeError", () => {
  it("maps the backend's machine-readable codes", () => {
    expect(localizeError("offline")).toBe(t("err.offline"));
    expect(localizeError("rejected")).toBe(t("err.rejected"));
    expect(localizeError("payload_too_large")).toBe(t("err.tooLarge"));
    expect(localizeError("source_file_gone")).toBe(t("err.sourceGone"));
  });

  it("maps a broken pin, which used to fall through to the generic message", () => {
    expect(localizeError("trust_broken")).toBe(t("err.trustBroken"));
    expect(localizeError("trust_broken")).not.toBe(t("err.generic"));
  });

  it("maps the Chinese strings the backend still emits", () => {
    expect(localizeError("设备离线")).toBe(t("err.offline"));
    expect(localizeError("条目不存在")).toBe(t("err.notFound"));
  });

  it("prefers the more specific token failure", () => {
    expect(localizeError("token_expired")).toBe(t("err.tokenExpired"));
    expect(localizeError("token_invalid")).toBe(t("err.tokenInvalid"));
  });

  it("falls back to the generic message for anything unrecognised", () => {
    expect(localizeError("")).toBe(t("err.generic"));
    expect(localizeError(null)).toBe(t("err.generic"));
    expect(localizeError(new Error("kaboom"))).toBe(t("err.generic"));
  });

  it("no longer claims to understand a disk-space failure nothing can raise", () => {
    // The wording was removed because no code path produced it; make sure no
    // future rule reintroduces a mapping for a message that cannot occur.
    expect(localizeError("no space left on device")).toBe(t("err.generic"));
  });
});
