# LanPaste 实现约定（subagent 共享）

前后端必须遵守本文件与 `src/shared/types.ts`。不要各写一套字段名。

## 窗口

| label | URL | 尺寸 | 装饰 |
|-------|-----|------|------|
| overlay | `/?window=overlay` | 780×520 | 无标题栏、始终置顶、启动时隐藏 |
| settings | `/?window=settings` | 720×560 | 有标题栏、启动时隐藏 |
| pairing-show | `/?window=pairing-show` | 360×220 | 无标题栏、始终置顶、隐藏 |
| pairing-input | `/?window=pairing-input` | 400×200 | 无标题栏、始终置顶、隐藏 |

没有 `main` 窗口。托盘左键显示 overlay；菜单：打开历史、设置、退出。全局快捷键默认 `Cmd+Shift+V`。

## IPC commands（camelCase JSON）

全部返回 `Result`，错误用可读字符串（可映射 i18n key）。

- `list_history` `{ query?: string, typeFilter?: PasteType | "all" }` → `HistoryEntry[]`
- `get_entry` `{ id: string }` → `HistoryEntry | null`
- `paste_entry` `{ id: string }` → `()`（文件未下载则先下载；失败抛错，不关浮层由前端决定）
- `hide_overlay` → `()`
- `show_overlay` → `()`
- `show_settings` → `()`
- `copy_entry_to_clipboard` `{ id: string }`
- `sync_to` `{ id: string, deviceId: string }`
- `delete_entry` `{ id: string }`
- `reveal_in_finder` `{ id: string }`
- `list_nearby` → `NearbyDevice[]`
- `list_paired` → `PairedDevice[]`
- `start_pair` `{ instanceId: string }`（发起方：打开 pairing-input；目标方由事件打开 pairing-show）
- `submit_pair_token` `{ instanceId: string, token: string }`
- `cancel_pair`
- `update_device_note` `{ instanceId: string, note: string }`
- `update_device_flags` `{ instanceId, allowSend, allowReceive, autoWriteClipboard }`
- `remove_device` `{ instanceId }`
- `get_settings` → `AppSettings`
- `update_settings` `{ ...partial }`
- `frontmost_app_name` → `string`

## Events

- `history-changed`
- `devices-changed`
- `pairing-show` payload `{ token: string, expiresAt: number }`
- `pairing-hide`
- `pairing-input` payload `{ instanceId: string, deviceName: string }`
- `transfer-progress` payload `{ id: string, received: number, total: number }`
- `overlay-shown`

## Rust 模块

`src-tauri/src/{clipboard,store,crypto,device,net,cleanup,ipc}.rs` + `lib.rs` 组装。

协议：mDNS `_lanpaste._tcp`；Axum HTTPS 钉扎；`POST /pair/request|confirm|revoke`；`POST /sync/entry` 幂等；`GET /files/:id` Range。文件自动同步不含字节。20MB 按整条体积。离线自动同步队列落盘。

测试：阈值、主类型、去重、文件不出字节、token 错/过期。

## 前端落地后的 IPC 注意

Vue 已接好契约 command。Rust 侧请保证：

1. `overlay-shown` / `pairing-show` / `pairing-input` 在对应 webview 就绪后再发一次（或提供 `get_pairing_state`），避免 listen 尚未注册就丢事件。
2. `pairing-hide` 同时关掉 show / input 两扇窗；input 取消走 `cancel_pair`。
3. `sync_to` 的 `deviceId` 是 `PairedDevice.instanceId`。
4. `copiedAt` / `expiresAt`：毫秒优先；若 `< 1e12` 前端会当秒乘 1000。
5. `list_nearby` 排除已配对。
6. `frontmost_app_name` 空字符串时底栏只显示「粘贴」。
7. `update_settings` 收 partial。


`src/overlay` `src/settings` `src/pairing` `src/shared`。设计系统：`design-system/lanpaste/`。Phosphor Vue。Vue 不 fetch 局域网。
