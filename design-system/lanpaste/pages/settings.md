# Settings window

Overrides `MASTER.md` for the standalone settings window.

## Purpose

Device pairing, per-device policy, storage cleanup, shortcut editor. Not a clipboard browser.

## Layout

Left nav (narrow, ~180px) + detail pane. Window has a native title bar (unlike the overlay).

Nav items: 通用 · 设备 · 存储与清理 · 快捷键 · 关于

## Devices pane

1. **附近** — unpaired mDNS results; trailing **添加**.
2. **我的设备** — online green dot, note name, overflow menu (修改备注 / 移除), toggles: 允许发送、允许接收、自动写入剪贴板.

Already-paired devices must not also appear under 附近.

## Pairing prompt

Not part of this window's layout. It is a separate always-on-top dialog defined in MASTER. Settings can be closed; the token dialog still appears.

## Density

Slightly looser than overlay (`--space-xl` section padding) but still dense. No marketing cards.
