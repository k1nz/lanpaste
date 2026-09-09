# LanPaste

**English** | [简体中文](README.zh-CN.md)

Copy on this computer. Paste on the other one.

LanPaste is a clipboard for the computers on your desk. It connects Mac and Windows machines over the same Wi‑Fi or wired network. Anything you copy on one machine — a link, a color code, a screenshot, or a file — becomes available on the other within moments, ready to paste. History is kept on both machines, and all data stays on your own network. There is no account, no cloud, and nothing goes to the internet. LanPaste runs quietly in the menu bar / system tray until you need it.

## Features

- **No learning curve.** Copy and paste exactly as you always do; LanPaste simply makes the content available on your other machines.
- **Private by design.** Clipboard items are transferred only between machines you have paired, on your own network. No server, no account.
- **Searchable history.** A single shortcut opens a search overlay: find the item you copied, preview it, and press **Enter** to paste it into the app you are using.
- **Large files, no surprises.** A copied file is downloaded only when you paste it, so a large video will not silently fill the other computer's disk.

## Getting started

1. **Install LanPaste on both computers.** Download the appropriate build for each platform from [Releases](https://github.com/k1nz/lanpaste/releases). The application is not yet signed, so the system may ask you to allow it on first launch — see [First launch](#first-launch).
2. **Pair the machines.** Open **Settings** (from the tray menu, or press `⌘,` / `Ctrl+,` inside the overlay), go to **Devices**, and the other machine should appear under *Nearby*. Click **Add**, then enter the 6-digit code shown on the other screen.
3. **Done.** Copy on one machine, paste on the other. Each machine needs to be paired only once.

Both computers must be running LanPaste and be connected to the **same local network** (the same Wi‑Fi, or wired and wireless devices on the same router).

### First launch

Because the application is not yet signed, the system may warn you the first time you open it:

- **Mac:** System Settings → Privacy & Security → Open Anyway, or Control-click the app → Open.
- **Windows:** If SmartScreen appears, select More info → Run anyway.

## Everyday use

Open the overlay with a shortcut, or click the tray icon:

- **Mac:** Option‑Shift‑V (`⌥⇧V`)
- **Windows:** Ctrl‑Shift‑V

Once it is open:

- **Type** to search your history.
- **↑ / ↓** to move between results; **Enter** to paste and close.
- **Esc** to dismiss.
- **⌘K** / **Ctrl+K** for more actions — copy on this machine, sync to one device, delete, and more.
- Filter by type (text, image, file, …) when needed.

The shortcut can be changed in **Settings → Shortcuts**.

### Files

A copied file appears on the other computer as its name and size; the contents are downloaded only when you paste it. Large files therefore consume no disk space until you actually need them. If a transfer fails, the item stays in history — press **Enter** to retry.

### Sending to one computer

By default, new items sync to all paired machines. To share something with a specific machine only — or when the content exceeds the auto-sync size limit — open **Actions → Sync to** and select a device. Offline devices cannot be selected.

## Per-device settings

In **Settings → Devices**, each paired computer has independent switches:

- **Allow send** — this machine may push clipboard items to it.
- **Allow receive** — this machine may accept items from it.
- **Auto-write clipboard** — incoming text (and similar content) is placed on the system clipboard immediately, so you can paste it without opening the overlay. Files still download only when you paste.

You can add a **note** (visible only to you) or **Remove** a device. Removing requires pairing again before sharing can resume.

In **Settings → Storage**, set the maximum size for auto-synced items (default 20 MB) and how much history to keep (default 500 items / 1 GB).

## Privacy

LanPaste has no account and does not connect to any server; content is transferred only between machines you have paired, over your local network.

Pairing relies on a short code shown on the other screen. Anyone who can see that screen could enter it, so pair the machines while you are in front of both. After pairing, each machine remembers the other until you remove the device.

The application is intended for networks you trust, such as home or office networks. A public café Wi‑Fi is not a suitable environment. LanPaste does not sync over the internet, and there is currently no Linux build.

## Building from source

To compile it yourself:

```bash
npm install
npx tauri dev
```
