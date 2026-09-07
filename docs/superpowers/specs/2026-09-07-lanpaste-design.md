# LanPaste 设计文档

**日期:** 2026-09-07  
**状态:** 已通过产品讨论，待实现计划  
**栈:** Tauri 2 + Vue 3 + Rust（v1 只发 macOS）  
**UI 约束:** [design-system/lanpaste/MASTER.md](../../../design-system/lanpaste/MASTER.md)（ui-ux-pro-max，按 Raycast 覆写）

---

## 1. 产品一句话

LanPaste 是局域网、无服务器的跨设备剪贴板。本机始终有一份 Raycast 式历史；已配对设备之间默认同步（可按设备开关），也可以把某一条「同步到」指定机器。文件只传元数据，对端真正粘贴时才拉字节。

## 2. 目标与非目标

### 目标（v1）

- macOS 托盘常驻；`⌘⇧V` 浮层快搜快贴（贴完即关）；设置单独窗口
- 发现局域网内其他 LanPaste、token 配对、管理设备（备注、移除、发送/接收、自动写入剪贴板）
- 类型：文本、颜色、URL、HTML、RTF、图片、文件；**同一时间线混排**，按复制时间倒序
- 混合同步 + 体积封顶（默认 20MB）+ 图片立即传 + 文件粘贴才传
- 本地 SQLite + blob；按条数/体积自动清理
- 非对称身份（Ed25519）+ 钉扎 TLS；全程仅局域网，无云、无中继、无账号

### 非目标（v1 明确不做）

- Windows / Linux 发行（目录按以后能加来切）
- 类型文件夹导航（CrossPaste 交互）
- OCR、MCP、CLI、浏览器扩展、插件、异地/中继
- 浮层 Light Mode、营销落地页式 UI

## 3. 用户与成功标准

个人/小团队在同一 Wi‑Fi 或有线局域网，2–N 台 Mac。成功看起来像：

1. 两台机器打开 LanPaste，附近列表能看见对方
2. 点添加，对着目标机弹出的 6 位 token 输入，进入「我的设备」
3. 这边复制一段文本或一张截图，那边浮层顶部出现；若开了自动写入剪贴板，那边可直接粘贴
4. 复制一个文件，那边历史里能看到文件名，点粘贴后才开始传，传完贴出
5. 右键「同步到」只打选定那一台；超 20MB 的条目不会自动广播

## 4. 系统架构

一个 Tauri 进程，两扇窗 + 托盘。

```
Vue overlay / settings / tray
        │  commands + events  （不持有私钥、不发 HTTP）
        ▼
Rust core
  clipboard │ store │ crypto │ device │ net │ cleanup
        │
        ▼
局域网：每台既是 HTTPS 服务端也是客户端
  mDNS `_lanpaste._tcp`  ⇄  Axum + 证书钉扎
```

模块职责：

| 单元 | 做什么 | 依赖 |
|------|--------|------|
| `clipboard` | 监听系统剪贴板、写入、向前台应用粘贴 | macOS 原生 API（trait，供以后 Win 实现） |
| `store` | SQLite 元数据 + blob 目录 + 内容哈希去重 | 磁盘 |
| `crypto` | 本机 Ed25519 身份、证书、加密会话校验 | 密钥文件 |
| `device` | 附近列表、已配对、备注、发送/接收/写剪贴板开关 | store + mdns |
| `net` | mDNS、Axum、配对、同步、文件 GET | crypto + device + store |
| `cleanup` | 按策略删条目和未引用 blob | store |
| Vue overlay | 列表/预览/Actions | 只 IPC |
| Vue settings | 设备与策略 | 只 IPC |

## 5. 配对与信任

1. 双方 mDNS 广播：实例 ID、设备名、端口、公钥指纹
2. A 在设置「附近」点 B 的 **添加** → `POST /pair/request`
3. B **立刻**弹出置顶窗，显示 6 位 token（约 60s 过期，超时轮换）
4. 用户在 A 输入 token → `POST /pair/confirm`（token + A 的 Ed25519 公钥）
5. B 一次性校验成功后双方钉扎公钥，条目进入「我的设备」
6. 已配对设备不再出现在「附近」

之后每次 HTTPS：对端证书必须匹配钉扎公钥，否则断开，设置里标「信任失效，需重新配对」。剪贴板内容只在这条钉扎通道上走（TLS 1.3）。Token 不参与后续同步。

管理：

- **备注**仅本机
- 每台：**允许发送 / 允许接收 / 自动写入剪贴板**（默认三者都开）
- **移除**：删本地钉扎，并尽力 `POST /pair/revoke`；对方再用旧钥匙会被 401。再连必须重新走 token

## 6. 数据模型

一次复制 = 1 条 **Pasteboard** + N 个 **Item**（网页可同时有文本 + HTML）。

**列表交互（Raycast，不是 CrossPaste）：**

- 所有类型在**同一个**列表
- 只按 `copied_at` 倒序；时间分组仅为 Today / Yesterday / 更早
- 「全部类型」是过滤器，不是文件夹
- 一条 Pasteboard 只占一行；主类型只决定图标和预览，不决定排序

主类型选择（仅用于那一行的外观）：文件 → 颜色 → HTML → RTF → 图片 → URL → 文本。

本地：复制后立即写入 SQLite；图片进 blob；**文件收入本机 blob**（或硬链），避免原文件被删后对端贴失败。

出网：

| 条件 | 行为 |
|------|------|
| ≤20MB 自动同步 | 文本/URL/颜色/HTML/RTF 全文；图片全文；文件仅元数据（名、大小、哈希、下载凭证） |
| >20MB | 本机历史保留；不出网，除非「同步到」 |
| 「同步到」 | 只发给选定设备；忽略自动阈值；文件仍先元数据 |

阈值可在设置改，默认 20MB。

存储：SQLite 可搜文本、按类型筛；blob 按内容哈希去重。清理：设置里保留天数 / 最大条数 / 最大体积（默认建议 500 条且 1GB，先到先限）；正在 GET 的文件不删；哈希仍被引用则只减计数。

## 7. 主路径

**本机复制：** 剪贴板变化 → 与上一条内容哈希相同则忽略 → 拆 items → 入库 → 浮层顶插入 → 若 ≤20MB 且存在「本机允许发送 + 对端允许接收 + 在线」的已配对设备 → 自动同步。

**自动同步：** `POST /sync/entry`（加密通道上的元数据；图片带字节；文件不带字节）。对端写入历史。若对端开了「自动写入剪贴板」且本条不是待拉取文件，则写入对端系统剪贴板。

**同步到：** 浮层右键或 ⌘K → 选出一台（离线不可点）→ 同一 API 只打这一台。对端拒收要明确报错，不静默吞。

**对端粘贴文件：** 浮层选中 → Paste → `GET /files/:id`（钉扎 TLS + 下载凭证，支持 Range）→ 进度 → 写入 blob 与系统剪贴板 → 向前台应用粘贴 → 浮层关闭。失败则条目留在历史可重试，不假装贴出。

失败：

- 自动同步时对端离线：入队，上线补发
- 「同步到」离线：当场提示
- 证书不对：断开 + 信任失效
- 源文件已被清理：提示源设备已无此文件
- 传输中断：可续传，浮层失败态，不卡死

## 8. 界面与快捷键

视觉与组件规则以 `design-system/lanpaste/` 为准。实现任何 Vue 页面前：先读 `MASTER.md`，再读 `pages/overlay.md` 或 `pages/settings.md`。

| 表面 | 作用 |
|------|------|
| 浮层 | 搜索、类型过滤、时间线、预览、Information、粘贴、同步到 |
| 设置 | 通用、设备、存储与清理、快捷键、关于 |
| 配对弹窗 | 目标机 token，不依赖设置是否打开 |
| 托盘 | 常驻；单击打开浮层；菜单含设置 / 退出 |

键盘：`⌘⇧V` 开浮层（可改）、↑↓ 移动、Enter 粘贴并关、Esc 关、⌘K Actions、⌘, 设置。

Actions：粘贴、复制到本机剪贴板、同步到、在访达中显示（文件）、删除、复制颜色值/URL。

## 9. 协议要点

- 服务名：`_lanpaste._tcp`；端口由本机选择并写入 TXT
- 明文 HTTP 不做业务；自签证书与 Ed25519 身份绑定，客户端钉扎
- `POST /pair/request` `POST /pair/confirm` `POST /pair/revoke`
- `POST /sync/entry` 幂等（同一 pasteboard id 重复投递不插第二行）
- `GET /files/:id` 需配对身份；凭证与条目绑定；支持 `Range`
- 未配对只发现，任何剪贴板路径都 401

错误码要能映射到 UI 文案：token 错误、token 过期、已移除、拒收、离线、体积/磁盘不足。

## 10. 工程结构

```
src/                 Vue 3 + Vite
  overlay/
  settings/
  shared/            设计 token、Phosphor 图标、i18n
src-tauri/src/
  clipboard/
  store/
  crypto/
  device/
  net/
  cleanup/
design-system/lanpaste/   前端硬约束
```

Rust 依赖方向：Tauri 2、global-shortcut、Axum、rustls、mdns-sd、rusqlite、ed25519-dalek、AEAD 仅用于需要应用层封装的字段（若 TLS 钉扎已覆盖传输，不要再叠一套无文档的双加密）。剪贴板必须能取 HTML/RTF/文件，不能只用纯文本 crate。

前端：`@phosphor-icons/vue`；历史列表 `v-for` 用稳定 id；>200 条虚拟列表；Vue 不 `fetch` 设备 IP。

文案 v1 以简体中文为主，键值放 i18n，避免写死散落。

## 11. 测试

| 层 | 覆盖 |
|----|------|
| 规则单测 | 20MB 阈值、主类型、去重、清理顺序、文件自动同步不含字节 |
| 配对 | token 错/过期/一次性；移除后 401；钉扎不匹配断开 |
| 同步 | 两进程不同端口；元数据到达；GET 才有文件 blob；「同步到」只打一台 |
| Vue | 时间倒序、类型过滤不拆文件夹、空搜索态 |

v1 不上不稳定的真双机 GUI e2e。

## 12. 实现时的 UI 技能约束

写或改任何界面代码：

1. 读 `design-system/lanpaste/MASTER.md`
2. 若有对应 `pages/*.md`，以其为准覆盖 Master
3. 对照 MASTER 的 Pre-Delivery Checklist
4. 需要补 UX/栈细节时，用 ui-ux-pro-max 的 `search.py`（`--domain ux` / `--stack vue`），不要重新发明落地页风格

视觉对标用户提供的 Raycast 剪贴板截图：暗色玻璃、左列表右预览、底栏 Actions、类型混排时间线。
