# LanPaste

局域网跨设备剪贴板。Tauri 2 + Vue 3，无服务器，token 配对，Raycast 风格浮层。

## 开发

```bash
npm install
npx tauri dev
```

Vite 在 **http://localhost:1480**。托盘常驻，默认快捷键 macOS **⌥⇧V**，Windows **Ctrl+Shift+V**。

规格：`docs/superpowers/specs/2026-09-07-lanpaste-design.md`  
设计系统：`design-system/lanpaste/MASTER.md`

## 发布

1. 把 `package.json`、`src-tauri/tauri.conf.json`、`src-tauri/Cargo.toml` 的 `version` 改成同一版本号。
2. 提交后打 tag 并推送：

```bash
git tag v0.1.0
git push origin v0.1.0
```

GitHub Actions 会构建 macOS（Apple Silicon / Intel）和 Windows 安装包，用 git-cliff 根据提交记录生成 `CHANGELOG.md`，并发布 GitHub Release。tag 带 `-`（例如 `v0.2.0-rc.1`）会标成 pre-release。

若 Action 报 `Resource not accessible by integration`，到仓库 Settings → Actions → Workflow permissions 勾选 **Read and write permissions**。当前构建未配置 Apple / Windows 签名，macOS 安装包可能需要在「系统设置 → 隐私与安全性」里允许打开。
