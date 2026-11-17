# adbmaid

基于 **Tauri v2 + Vue 3 + Rust** 构建的跨平台 Android ADB 设备可视化工作台，主打多设备高效管理、无线调试发现、拖拽安装 APK、交互式终端、实时 Logcat 与 Scrcpy 投屏联动。

## 核心特性

- **多设备自动发现与快速切换**：后台自动轮询本地 ADB Server（`127.0.0.1:5037`），支持手动开关自动扫描；左上角弹出式设备选择器支持按别名、型号、序列号搜索以及 USB / Wi-Fi 快捷筛选。
- **设备概览与身份管理**：实时查看电量、分辨率、无线 IP、存储空间与设备指纹信息（SQLite 持久化备注名、颜色标签、历史连接端点），支持一键重启至 System / Bootloader / Recovery / Sideload。
- **无线发现 (mDNS)**：融合 ADB mDNS 与原生 `mdns-sd` 双路扫描，支持 Android 11+ 无线配对码配对及 TCP/IP 连接。
- **应用管理 & 全局拖拽安装**：支持查看/搜索/启停/清理/导出/卸载应用，支持将单个或多个 `.apk` 文件直接拖入窗口任意位置快速安装（可选 `-r` / `-g` / `-d` / `-t` 参数）。
- **交互终端 & 实时 Logcat**：内置 `@xterm/xterm` 交互式 ADB Shell 终端，以及支持级别过滤、关键词搜索与暂停的实时 Logcat 查看器。
- **Scrcpy 投屏联动**：支持管理 Scrcpy 参数预设并与设备快速联动拉起投屏；提供工具链自检与内置引导下载。

## 技术栈

- **桌面宿主 / 后端**：Rust, Tauri v2, `adb_client`, `mdns-sd`, `rusqlite`
- **前端界面**：Vue 3, TypeScript, Vite, Pinia, Naive UI, `@xterm/xterm`

## 本地开发与构建

### 环境要求

- Node.js 22+ 与 `pnpm`
- Rust 工具链（stable）
- 系统已安装 `adb`（及可选 `scrcpy`），或通过应用内工具链面板自动下载配置
- Linux 编译需安装 Tauri v2 系统依赖（如 Debian/Ubuntu 下的 `libwebkit2gtk-4.1-dev`、`libayatana-appindicator3-dev`、`librsvg2-dev`、`patchelf`）

### 常用命令

```bash
# 安装前端依赖
pnpm install

# 启动 Tauri 桌面端开发模式
pnpm tauri dev

# 前端类型检查与构建
pnpm build

# 打包当前平台安装包
pnpm tauri build
```

## CI/CD 与发布产物

- **默认 CI（`push main` / `pull_request`）**：运行前端 `vue-tsc` + `vite build`，以及后端 `cargo fmt --check`、`cargo clippy --all-targets -- -D warnings`、`cargo test`。
- **手动触发 / Tag 发布（`v*`）**：并行构建多平台安装包：
  - **Linux**：`.AppImage`、`.deb`、`.rpm`、`.flatpak`
  - **Windows**：`.exe` (NSIS)、`.msi`
  - **macOS**：`.dmg`
