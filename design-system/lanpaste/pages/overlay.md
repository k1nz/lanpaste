# Overlay (⌥⇧V / Ctrl+Shift+V)

Overrides `MASTER.md` for the clipboard history palette. Default shortcut is Option+Shift+V on macOS and Ctrl+Shift+V on Windows; it is user-editable in **Settings → Shortcuts**.

## Purpose

Keyboard-first Raycast Clipboard History: one chronological list of all types, preview on the right, paste and dismiss.

## Layout (fixed)

```
┌ search ─────────────── overlay.allTypes ▾ ┐
│ Today                              │
│  row                               │  preview
│  row (selected)                    │  Information
│ Yesterday                          │
├ Clipboard History · 粘贴到 App · ⌘K / Ctrl+K ┤
```

- No back-stack in v1 except closing nested Actions. Do not add a Raycast "root command" home.
- Type control is a **filter**, not a folder.

## Behavior

- Open: focus search, restore last selected id if still present, else first row.
- Enter / bottom primary action: paste into frontmost app, then hide window.
- Esc: hide, do not paste.
- Click away / lose focus: hide the window (blur dismisses).
- Right-click and ⌘K share the same action list; include **同步到**.
- List is virtualized when > 200 rows.
- Remote file rows: show name + size as pending; selecting one does not start the transfer — only Paste (Enter) fetches the bytes.

## Copy

Search placeholder: `overlay.searchPlaceholder`  
Empty: `overlay.emptyTitle` + `overlay.emptyHint`  
Bottom leading label: `overlay.clipboardHistory`

Each key has a zh-CN value in `src/locales/zh-CN.json` and an en-US value in `src/locales/en-US.json` — e.g. `overlay.clipboardHistory` is 剪贴板历史 in zh-CN and "Clipboard History" in en-US. Both catalogues carry the same key set.
