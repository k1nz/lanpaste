# Overlay (⌘⇧V)

Overrides `MASTER.md` for the clipboard history palette.

## Purpose

Keyboard-first Raycast Clipboard History: one chronological list of all types, preview on the right, paste and dismiss.

## Layout (fixed)

```
┌ search ─────────────── 全部类型 ▾ ┐
│ Today                              │
│  row                               │  preview
│  row (selected)                    │  Information
│ Yesterday                          │
├ Clipboard History · 粘贴到 App · ⌘K ┤
```

- No back-stack in v1 except closing nested Actions. Do not add a Raycast "root command" home.
- Type control is a **filter**, not a folder.

## Behavior

- Open: focus search, restore last selected id if still present, else first row.
- Enter / bottom primary action: paste into frontmost app, then hide window.
- Esc: hide, do not paste.
- Right-click and ⌘K share the same action list; include **同步到**.
- List is virtualized when > 200 rows.
- File rows that still need download: show size + pending; selecting them does not start transfer until Paste.

## Copy

Search placeholder: `搜索历史…`  
Empty: `无匹配条目` + `试试清空类型过滤`  
Bottom leading label: `Clipboard History`
