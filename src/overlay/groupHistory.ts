import type { HistoryEntry } from "../shared/types";
import { groupDateLabel } from "../shared/format";

export interface HistoryGroup {
  id: string;
  label: string;
  items: HistoryEntry[];
}

export type FlatRow =
  | { kind: "header"; key: string; label: string }
  | { kind: "item"; key: string; item: HistoryEntry };

export function sortNewestFirst(items: HistoryEntry[]): HistoryEntry[] {
  return items.slice().sort((a, b) => b.copiedAt - a.copiedAt);
}

export function groupHistory(items: HistoryEntry[]): HistoryGroup[] {
  const map = new Map<string, HistoryEntry[]>();
  const order: string[] = [];
  for (const item of sortNewestFirst(items)) {
    const label = groupDateLabel(item.copiedAt);
    let bucket = map.get(label);
    if (!bucket) {
      bucket = [];
      map.set(label, bucket);
      order.push(label);
    }
    bucket.push(item);
  }
  return order.map((label) => ({ id: label, label, items: map.get(label)! }));
}

export function flattenGroups(groups: HistoryGroup[]): FlatRow[] {
  const rows: FlatRow[] = [];
  for (const group of groups) {
    rows.push({ kind: "header", key: `h:${group.id}`, label: group.label });
    for (const item of group.items) {
      rows.push({ kind: "item", key: item.id, item });
    }
  }
  return rows;
}

export const ITEM_ROW_H = 38;
export const HEADER_ROW_H = 22;

export function rowHeight(row: FlatRow): number {
  return row.kind === "header" ? HEADER_ROW_H : ITEM_ROW_H;
}
