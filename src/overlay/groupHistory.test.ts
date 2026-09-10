import { beforeEach, describe, expect, it } from "vitest";
import { setResolvedLocale } from "../shared/i18n";
import type { HistoryEntry } from "../shared/types";
import { flattenGroups, groupHistory, rowHeight, sortNewestFirst } from "./groupHistory";

function entry(id: string, copiedAt: number): HistoryEntry {
  return {
    id,
    copiedAt,
    sourceDeviceId: null,
    sourceDeviceName: "This computer",
    primaryType: "text",
    title: id,
    preview: {},
    needsFileDownload: false,
    fileDownloadState: "idle",
  };
}

const START_OF_TODAY = new Date();
START_OF_TODAY.setHours(9, 0, 0, 0);

function hoursAgo(h: number): number {
  return START_OF_TODAY.getTime() - h * 3_600_000;
}

beforeEach(() => {
  setResolvedLocale("en-US");
});

describe("sortNewestFirst", () => {
  it("orders by copy time, newest first, without mutating the input", () => {
    const input = [entry("old", 1_000), entry("new", 3_000), entry("mid", 2_000)];
    expect(sortNewestFirst(input).map((e) => e.id)).toEqual(["new", "mid", "old"]);
    expect(input.map((e) => e.id)).toEqual(["old", "new", "mid"]);
  });
});

describe("groupHistory", () => {
  it("buckets items under their date label, in order", () => {
    const groups = groupHistory([
      entry("today-a", hoursAgo(1)),
      entry("today-b", hoursAgo(3)),
      entry("yesterday", hoursAgo(30)),
    ]);

    expect(groups.map((g) => g.label)).toEqual(["Today", "Yesterday"]);
    expect(groups[0].items.map((e) => e.id)).toEqual(["today-a", "today-b"]);
    expect(groups[1].items.map((e) => e.id)).toEqual(["yesterday"]);
  });

  it("keeps a single entry per label even when times interleave", () => {
    const groups = groupHistory([entry("a", hoursAgo(2)), entry("b", hoursAgo(4))]);
    expect(groups).toHaveLength(1);
    expect(groups[0].items).toHaveLength(2);
  });

  it("returns nothing for no items", () => {
    expect(groupHistory([])).toEqual([]);
  });
});

describe("flattenGroups", () => {
  it("puts a header before each group's rows", () => {
    const rows = flattenGroups([groupHistory([entry("x", hoursAgo(1))])[0]]);

    expect(rows.map((r) => r.kind)).toEqual(["header", "item"]);
    const [header] = rows;
    if (header.kind !== "header") throw new Error("expected a header row first");
    expect(header.label).toBe("Today");
    expect(rows[1].key).toBe("x");
  });

  it("heights headers and rows differently", () => {
    const rows = flattenGroups([groupHistory([entry("x", hoursAgo(1))])[0]]);
    expect(rowHeight(rows[0])).toBeLessThan(rowHeight(rows[1]));
  });
});
