# Design System Master File

> **LOGIC:** When building a specific page, first check `design-system/lanpaste/pages/[page-name].md`.
> If that file exists, its rules **override** this Master file.
> If not, strictly follow the rules below.

---

**Project:** LanPaste  
**Generated:** 2026-09-07  
**Source:** ui-ux-pro-max (`--design-system`, variance 3 / motion 3 / density 8)  
**Visual north star:** Raycast Clipboard History (dark command palette), not a marketing landing page.

The raw `--design-system` output recommended Exaggerated Minimalism + landing CTA + vault green. **Those defaults are rejected.** This file is the overridden source of truth for a dense, keyboard-first macOS and Windows overlay.

**Design Dials:** Variance 3/10 (Centered / Minimal) | Motion 3/10 (Subtle) | Density 8/10 (Dense)

---

## Global Rules

### Color Palette

Dark-first. Overlay and pairing prompt are dark only in v1. Settings may later follow system appearance; do not ship a light overlay in v1.

Tokens combine ui-ux-pro-max **Modern Dark / Glassmorphism** with Raycast selection chrome (macOS blue, not emerald CTA).

| Role | Hex | CSS Variable |
|------|-----|--------------|
| Background deep | `#0A0A0C` | `--color-background` |
| Surface / glass | `rgba(28, 28, 30, 0.72)` | `--color-surface` |
| Surface solid | `#1C1C1E` | `--color-surface-solid` |
| Elevated row | `rgba(255, 255, 255, 0.06)` | `--color-elevated` |
| Foreground | `#EDEDEF` | `--color-foreground` |
| Foreground muted | `#8A8F98` | `--color-muted` |
| Border | `rgba(255, 255, 255, 0.08)` | `--color-border` |
| Selection / accent | `#0A84FF` | `--color-accent` |
| On accent | `#FFFFFF` | `--color-on-accent` |
| Destructive | `#FF453A` | `--color-destructive` |
| Ring / focus | `#0A84FF` | `--color-ring` |
| Success | `#30D158` | `--color-success` |

**Do not use** `#059669` vault green, oversized hero type, or white modal sheets on the overlay.

### Typography

- **UI font:** `-apple-system, BlinkMacSystemFont, "SF Pro Text", "Inter", system-ui, sans-serif`
- **Mono (paths, hashes, token):** `ui-monospace, "SF Mono", Menlo, monospace`
- Overlay does **not** load Google Fonts at runtime (startup latency). Inter is an optional fallback if bundled later.
- Sizes (dense): caption 11px, body 13px, title 15px. Line-height 1.35. No heading larger than 20px in the overlay.

### Spacing

*Density: 8/10 — Dense / Dashboard*

| Token | Value | Usage |
|-------|-------|-------|
| `--space-xs` | `2px` | Hairline gaps |
| `--space-sm` | `4px` | Icon-to-label |
| `--space-md` | `8px` | Row padding-y, list gap |
| `--space-lg` | `12px` | Pane padding |
| `--space-xl` | `16px` | Window chrome padding |
| `--space-2xl` | `24px` | Settings section stack |
| `--space-3xl` | `32px` | Settings page margins |

Row height in the history list: **36–40px**. Do not use 24px marketing card padding on list rows.

### Shadow / Glass

| Token | Value | Usage |
|-------|-------|-------|
| `--blur` | `20px` | Overlay `backdrop-filter` |
| `--radius-window` | `12px` | Overlay and settings window |
| `--radius-row` | `6px` | Selected row |
| `--shadow-window` | `0 12px 40px rgba(0,0,0,0.45)` | Floating window only |

No stacked card shadows, no 3D, no ambient animated blobs.

---

## Component Specs

### Overlay window

- Size ≈ 780×520, centered, always on top while visible, no native title bar.
- Chrome: search field + type filter on top; split list | preview; Raycast-style action bar at bottom.
- Selected row: fill `--color-accent`, text `--color-on-accent`.
- Unselected hover: `--color-elevated` only (no translateY, no scale).

### List row

- Left: type glyph (Phosphor, 16px, `regular` weight) or thumbnail.
- Middle: single-line truncated title.
- Right (optional): relative time, muted.
- Time section labels (`Today` / `Yesterday` / date): 11px, `--color-muted`.

### Buttons

```css
.btn-primary {
  background: var(--color-accent);
  color: var(--color-on-accent);
  padding: 6px 12px;
  border-radius: 6px;
  font-size: 13px;
  font-weight: 500;
  cursor: pointer;
  transition: opacity 160ms ease;
}
.btn-primary:hover { opacity: 0.9; }
.btn-ghost {
  background: transparent;
  color: var(--color-foreground);
  border: 1px solid var(--color-border);
  padding: 6px 12px;
  border-radius: 6px;
  cursor: pointer;
}
```

Do not lift buttons on hover (`translateY`).

### Search input

- Borderless, 15px text, caret `--color-accent`.
- Focus ring 2px `--color-ring` only when needed for a11y; overlay search is auto-focused on open so keep the ring subtle (inset, not a fat halo).

### Actions panel (⌘K)

- Same glass surface, list of commands with shortcut hints on the right (muted, 11px).
- Highlight follows keyboard, not pointer-only.

### Pairing token prompt

- Independent always-on-top window.
- Token: 32px tabular/mono, letter-spacing 0.2em.
- Countdown muted. No decoration besides the number.

---

## Style Guidelines

**Style:** Glassmorphism overlay + Raycast command palette (not Exaggerated Minimalism, not Cyberpunk, not Terminal green).

**Keywords:** frosted glass, dense list, keyboard-first, dark vibrancy, type-specific icons, split preview.

**Key effects:** `backdrop-filter: blur(20px) saturate(140%)`; hairline `rgba(255,255,255,0.08)` border; macOS window shadow.

---

## Motion

Subtle only. No GSAP ScrollTrigger (this is not a landing page).

- Overlay in/out: 120–180ms opacity + 6px translateY, `cubic-bezier(0.16, 1, 0.3, 1)`
- Selection change: instant background (keyboard speed > animation)
- File transfer progress: determinate bar, 160ms width ease
- `prefers-reduced-motion: reduce` → opacity only, no translate

---

## Icons

- Library: **Phosphor** via `@phosphor-icons/vue` (not emoji, not mixed families).
- Default weight: `regular`. Active/selected may use `fill` only for the current row icon.
- Sizes: 16px in list, 20px in action bar / settings nav.
- Suggested: `Clipboard`, `Copy`, `MagnifyingGlass`, `Image`, `File`, `FileText`, `Link`, `Palette` (color), `Gear`, `Desktop`, `Plus`, `Trash`, `Paperclip`.

---

## Vue constraints (ui-ux-pro-max `--stack vue`)

- `v-for` **must** use stable `item.id` keys. Never index keys on the history list.
- Long history: `v-memo="[item.id, item.selected, item.title]"` on rows; virtualize if count > 200.
- Settings panes and pairing prompt: `defineAsyncComponent` is fine; overlay shell stays eager.
- All clickable elements: `cursor: pointer`.
- Visible `:focus-visible` ring using `--color-ring`.
- Every user-visible string lives in `src/locales/*.json` and is read through `t()` — never hard-code copy in a component.
- `en-US.json` and `zh-CN.json` must carry the same key set (key-parity test in `src-tauri/src/i18n.rs`).
- Design docs reference the key name (`overlay.clipboardHistory`), not a literal string in one language, as the spec.

---

## Anti-Patterns (Do NOT Use)

- ❌ Type folders / CrossPaste category navigation
- ❌ Emojis as icons
- ❌ Landing-page hero, oversized type, marketing CTAs
- ❌ Vault green / neon cyberpunk / matrix terminal
- ❌ Layout-shifting hover (`scale`, `translateY` on rows)
- ❌ Invisible focus, `outline: none` with no replacement
- ❌ Instant visual jumps except list selection
- ❌ Fetching LAN/HTTPS from Vue; Vue does not hold private keys
- ❌ Light overlay in v1
- ❌ Google Fonts network request on overlay open

---

## Pre-Delivery Checklist

- [ ] Looks like Raycast clipboard: search, mixed-type chronological list, preview, bottom actions
- [ ] No emojis as icons (Phosphor only)
- [ ] Contrast: primary text ≥ 4.5:1, muted ≥ 3:1 on `--color-surface-solid`
- [ ] Keyboard: ↑↓, Enter paste+close, Esc close, ⌘K / Ctrl+K actions — no keyboard trap
- [ ] Empty search: “无结果” plus clear-filter hint, not a blank pane
- [ ] `prefers-reduced-motion` respected
- [ ] Overlay verified at 780×520; settings at ~720×560
- [ ] Clickable elements have `cursor: pointer` and 150–200ms opacity transitions
