# adbmaid 技术规格说明书（Spec v0.2）

> **项目暂定名**：`adbmaid`（候选品牌名：`adbmaid` / `Andmaid` / `Madroid`，本文档统一使用 `adbmaid` 作为仓库名与包名占位）
> **定位**：轻量、跨平台（Windows / macOS / Linux）的桌面端 Android 设备可视化管理工作台。

---

## 1. 项目定位与技术选型

### 1.1 核心目标
- 通过 **ADB 协议库** 管理本地及局域网 Android 设备，避免依赖反复解析 `adb` CLI 文本输出。
- 提供 **mDNS 无线发现 + 基于设备指纹的 SQLite 持久化记录**，在发现局域网无线调试设备时即可识别物理身份（“知道谁是谁”）。
- 提供开箱可用的 **应用管理**、**交互式终端（带基础能力与外观定制）**、**实时 Logcat**，以及 **原生 `scrcpy` 弹窗式投屏托管**。
- 支持 **主机工具链探测 + 自动下载**，并针对 Linux 下 `scrcpy` 分发碎片化问题提供专门策略。

### 1.2 非目标（Non-Goals）
- **不在主窗口内自研音视频解码与画面内嵌**：统一托管拉起原生 `scrcpy` 独立窗口，保证最低延迟与全特性兼容（虚拟屏、硬件解码、音频、UHID 等）。
- **默认不走 `libusb` 独占式 USB 直连**：业务通信统一经过本机 `127.0.0.1:5037` ADB Server，防止与 Android Studio、`scrcpy` 抢占 USB 接口。

### 1.3 锁定技术栈

| 层级 | 选型 | 说明 |
| :--- | :--- | :--- |
| **桌面壳 / 后端** | **Rust + Tauri v2** | 体积小、常驻内存低，通过 Command / Event / Channel 与前端通信 |
| **ADB 协议库** | **`adb_client` (3.x)** | 启用 `mdns`、`framebuffer` 特性；连接 `127.0.0.1:5037` ADB Server |
| **本地数据库** | **SQLite (`rusqlite` bundled)** | 零外部动态库依赖，存储设备指纹、端点历史、投屏预设与用户配置 |
| **前端框架** | **Vue 3 + TypeScript + Vite** | `<script setup lang="ts">`，搭配 Pinia + VueUse |
| **UI 组件库** | **Naive UI** | 独立完整桌面组件体系，**不混用 Nuxt UI / Tailwind Preflight** |
| **交互终端** | **`@xterm/xterm` + Addons** | `fit` / `search` / `web-links`（可选 `webgl`），配合 `portable-pty` 或流式通道 |

---

## 2. 核心架构红线

1. **统一经由 ADB Server（5037 端口）**
   - 启动时通过 `ADBServer::needs_server_start()` 检查 `127.0.0.1:5037`；未运行则由应用通过已探测/下载的 `adb` 可执行文件执行 `start-server`。
   - 仅 `mdns` 本地辅助扫描可独立运行，所有设备命令全部走 `ADBServer` / `ADBServerDevice`，确保与 `scrcpy` 共享通道。
2. **禁止阻塞 Tokio 异步运行时**
   - `adb_client` 为同步阻塞 IO；所有 Tauri Command 中的 ADB 调用必须放入 `tokio::task::spawn_blocking` 或独立线程池中执行。
3. **工具链环境变量强隔离**
   - 启动 `scrcpy` 子进程时，**必须显式注入环境变量**：
     - `ADB=<当前选定的 adb 可执行文件绝对路径>`
     - `SCRCPY_SERVER_PATH=<配套的 scrcpy-server 文件绝对路径>`
   - 防止 `scrcpy` 误调系统 `PATH` 中不同版本的 `adb`，导致瞬间杀掉并重启当前 `adb server`，致使设备监听、终端和 Logcat 全部断连。

---

## 3. 主机探测与自动下载（Toolchain Manager）

### 3.1 探测优先级与健康检查
应用维护两项核心外部依赖：`adb` 与 `scrcpy`（含 `scrcpy-server`），探测顺序如下：
1. **用户手动指定路径**（SQLite `app_settings` 中持久化）。
2. **应用托管目录**：`$APP_DATA/toolchains/scrcpy/<active_version>/` 与 `$APP_DATA/toolchains/platform-tools/`。
3. **系统环境探测**：
   - `$PATH` 中查找 `adb`、`scrcpy`。
   - 平台常见路径补充扫描：
     - **Windows**：`%LOCALAPPDATA%\Android\Sdk\platform-tools`、Scoop (`~/scoop/apps/scrcpy`)、Winget/Chocolatey 目录。
     - **macOS**：`/opt/homebrew/bin`、`/usr/local/bin`、`~/Library/Android/sdk/platform-tools`。
     - **Linux**：`/usr/bin`、`/usr/local/bin`、`~/.local/bin`、`$ANDROID_HOME/platform-tools`、Flatpak / Snap 常见导出路径。
4. **可执行性验证**：执行 `adb version` 与 `scrcpy --version`，解析版本号并检查同目录或默认路径下的 `scrcpy-server` 是否存在。

### 3.2 自动下载与校验机制
- **数据源**：默认对接 GitHub Releases（`Genymobile/scrcpy`）与 Google `platform-tools`（独立 `adb` 兜底）。
- **完整性校验**：下载官方 release 同时拉取 `SHA256SUMS.txt`，解压前强制校验压缩包 SHA-256。
- **网络与离线兜底**：
  - 支持配置 HTTP/SOCKS5 代理或 GitHub Release 下载镜像前缀（便于国内网络环境加速）。
  - 支持前端通过 Tauri `Channel` 展示下载字节进度、解压状态与错误日志。
  - 支持“**手动导入本地压缩包**”按钮，用户离线下载好官方 `.tar.gz`/`.zip` 后直接拖入完成安装。

### 3.3 平台矩阵与 Linux 专项策略

> **Linux 特别注意**：Linux 各发行版官方仓库对 `scrcpy` 打包覆盖率低且版本普遍陈旧（例如部分 LTS 仓库仍停留在 v1.x/v2.x，无法兼容新版本 Android 投屏协议），且上游官方预编译静态包仅提供 `x86_64`。因此 Linux 端必须区分架构与发行版做分级处理：

| 平台 / 架构 | `scrcpy` 自动下载支持 | 说明与兜底方案 |
| :--- | :--- | :--- |
| **Windows (`x86_64` / `x86` / `aarch64`)** | 官方 `.zip` 自动下载 | 内含 `adb.exe` + `scrcpy.exe` + `scrcpy-server`，一站式到位 |
| **macOS (`aarch64` / `x86_64`)** | 官方 `.tar.gz` 自动下载 | 内含 `adb` + `scrcpy` + `scrcpy-server`，首次解压需清除 quarantine 标记（如需要） |
| **Linux (`x86_64`)** | 官方静态包 `scrcpy-linux-x86_64-v*.tar.gz` 自动下载 | 若本机已探测到系统 `scrcpy` 但版本过旧（如 `< 3.0`），提示一键下载托管版替换；同时在设置中提供 `SDL_VIDEODRIVER`（X11/Wayland）与硬件解码开关以适配不同桌面环境 |
| **Linux (`aarch64` / 其他非 x86_64)** | **不提供官方静态包** | 探测 `/etc/os-release` 识别发行版（Arch / Debian / Ubuntu / Fedora / NixOS 等），明确提示原因并展示对应的包管理器（`apt`/`pacman`/`dnf`/Flatpak）或源码编译引导，支持绑定用户自编译路径；`adb` 可通过系统 `android-tools` 独立探测 |

---

## 4. mDNS 发现、设备指纹与 SQLite 存储设计

### 4.1 为什么需要“设备指纹”
Android 11+ 无线调试每次开启或重连时端口都会变化，且不同手机在 mDNS 广播中的服务名（如 `adb-xxxxxx-yyyyyy._adb-tls-connect._tcp`）并不直接展示“这是谁的什么手机”。通过 **USB/TCP 首次连接采集硬件指纹 + SQLite 持久绑定**，后续局域网 mDNS 扫描一出现就能立即认出设备型号与用户备注名。

### 4.2 双路 mDNS 发现引擎
为防止不同系统上 `adb server` 自带的 mDNS 后端（`openscreen` / `bonjour`）偶发不工作，后端同时支持两条发现通道并合并结果：
1. **ADB Server 通道**：周期调用或按需查询 `server.mdns_services()`，获取 `_adb-tls-connect._tcp`、`_adb-tls-pairing._tcp`、`_adb._tcp`。
2. **Rust 原生 `mdns-sd` 通道**：使用 `adb_client::mdns::MDNSDiscoveryService` 直接监听局域网组播响应，解析 IP、端口与 TXT 记录。
3. **结果融合与状态标注**：按 `ip:port` 与 `service_name` 去重，标注当前状态：`已连接`、`可直连（已配对）`、`待配对（_adb-tls-pairing）`。

### 4.3 指纹采集与身份识别算法

#### 指纹采集（设备连上后异步触发）
通过 `ADBServerDevice` 一次性读取基础只读属性（单条复合 shell 命令减少 RTT）：
- `ro.serialno` / `ro.boot.serialno`（硬件序列号；需过滤 `unknown`、`0123456789ABCDEF` 等无效占位值）
- `settings get secure android_id`（辅助唯一标识，区分同型号/同占位序列号设备）
- `ro.product.manufacturer`、`ro.product.brand`、`ro.product.model`、`ro.product.marketname`（或常见 OEM 市场名称属性）
- `ro.product.device`、`ro.product.cpu.abi`、`ro.build.version.release`、`ro.build.version.sdk`、`ro.build.fingerprint`

#### 身份生成与匹配优先级
每台物理设备在 SQLite 中生成稳定的 `device_uid`（逻辑 UUID 或基于有效硬件序列号/指纹的确定性 Hash）。当 mDNS 扫描到新端点时，按以下优先级匹配已知设备：

| 匹配级别 | 匹配依据 | UI 识别置信度展示 |
| :--- | :--- | :--- |
| **1. 精确序列号匹配** | 从 AOSP 标准 mDNS 服务名 `adb-<serial>-<suffix>` 提取 `<serial>`，命中 `devices.hardware_serial` | **已识别（精确）**：直接显示备注名 + 机型 + 颜色标签 |
| **2. 历史服务名匹配** | `service_name` 命中 `device_endpoints` 历史记录（适配部分魔改服务名的 OEM ROM） | **已识别（历史服务）** |
| **3. 网段/IP 弱提示** | 最近一次活跃 `ip` 相同且端口属于无线调试，但服务名全新（例如重新配对后） | **疑似设备**：提示“可能是 [某某设备]，连接后确认” |
| **4. 未知设备** | 未命中任何记录 | **新设备**：展示原始 mDNS 名与 `IP:Port` |

> **可选自动校准**：针对“已识别”且用户开启了 `auto_connect` 的 `_adb-tls-connect._tcp` 设备，在允许自动连接模式下自动执行 `server.connect_device(addr)`，连接成功后回写更新 `last_seen_at` 与最新端点。

### 4.4 SQLite 表结构草案

数据库文件存放于 `$APP_DATA/adbmaid.db`，采用 WAL 模式并通过 `PRAGMA user_version` 管理迁移。

```sql
-- 1. 物理设备主表（核心指纹与用户自定义元信息）
CREATE TABLE devices (
    device_uid          TEXT PRIMARY KEY,          -- 内部稳定 ID (UUID 或确定性指纹哈希)
    hardware_serial     TEXT,                      -- 真实 ro.serialno (排除占位值)
    boot_serial         TEXT,                      -- ro.boot.serialno
    android_id          TEXT,                      -- settings secure android_id
    brand               TEXT,                      -- e.g. Xiaomi
    manufacturer        TEXT,                      -- e.g. Xiaomi
    model               TEXT,                      -- e.g. 23116PN5BC
    market_name         TEXT,                      -- e.g. Xiaomi 14
    device_code         TEXT,                      -- ro.product.device
    abi                 TEXT,                      -- e.g. arm64-v8a
    android_version     TEXT,                      -- e.g. 15
    sdk_int             INTEGER,                   -- e.g. 35
    build_fingerprint   TEXT,
    alias               TEXT,                      -- 用户自定义备注名，如 "主力机 / 测试机A"
    color_tag           TEXT,                      -- 前端标识色，如 "#18a058"
    notes               TEXT,                      -- 设备备注
    auto_connect        INTEGER NOT NULL DEFAULT 0,-- mDNS 发现后是否自动 connect
    default_profile_id  TEXT,                      -- 绑定的默认 scrcpy 预设 ID
    first_seen_at       INTEGER NOT NULL,          -- Unix ms
    last_seen_at        INTEGER NOT NULL
);

CREATE INDEX idx_devices_hw_serial ON devices(hardware_serial);
CREATE INDEX idx_devices_android_id ON devices(android_id);

-- 2. 设备传输端点历史表（一台手机对应 USB / TCP / 多个 mDNS 服务名）
CREATE TABLE device_endpoints (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    device_uid          TEXT NOT NULL REFERENCES devices(device_uid) ON DELETE CASCADE,
    transport_type      TEXT NOT NULL,             -- 'usb' | 'tcp' | 'mdns_connect' | 'mdns_pairing'
    adb_serial          TEXT,                      -- ADB Server 内的标识，如 '192.168.1.50:41823' 或 USB serial
    mdns_service_name   TEXT,                      -- e.g. 'adb-xxxx-yyyy'
    mdns_reg_type       TEXT,                      -- e.g. '_adb-tls-connect._tcp.local.'
    last_ip             TEXT,
    last_port           INTEGER,
    last_seen_at        INTEGER NOT NULL,
    UNIQUE(device_uid, transport_type, adb_serial, mdns_service_name)
);

CREATE INDEX idx_endpoints_mdns_name ON device_endpoints(mdns_service_name);
CREATE INDEX idx_endpoints_ip ON device_endpoints(last_ip);

-- 3. scrcpy 参数预设表
CREATE TABLE scrcpy_profiles (
    id                  TEXT PRIMARY KEY,
    name                TEXT NOT NULL,             -- e.g. "默认均衡", "虚拟副屏 1080p", "高帧游戏"
    is_builtin          INTEGER NOT NULL DEFAULT 0,
    options_json        TEXT NOT NULL,             -- ScrcpyOptions JSON
    updated_at          INTEGER NOT NULL
);

-- 4. 终端快捷命令片段表
CREATE TABLE command_snippets (
    id                  TEXT PRIMARY KEY,
    title               TEXT NOT NULL,
    category            TEXT NOT NULL DEFAULT 'general',
    command             TEXT NOT NULL,
    auto_enter          INTEGER NOT NULL DEFAULT 1,
    sort_order          INTEGER NOT NULL DEFAULT 0
);

-- 5. 全局应用设置（工具链路径、终端外观、Logcat 默认上限、代理等）
CREATE TABLE app_settings (
    key                 TEXT PRIMARY KEY,
    value_json          TEXT NOT NULL,
    updated_at          INTEGER NOT NULL
);
```

---

## 5. 功能模块详细规格

### 5.1 设备中心 & 无线调试（Devices & mDNS）
- **实时在线列表**：后台独立线程运行 `server.track_devices()`，状态变更（`device` / `unauthorized` / `offline`）即时通过 `adb://devices-changed` 推送前端；自动关联 SQLite 中的设备备注名与颜色标签。
- **已知设备库视图**：即使设备当前离线，也可在“已记录设备”列表中查看历史设备、修改备注名、绑定默认投屏预设、查看历史连接端点。
- **无线连接面板**：
  - 手动 `IP:Port` 连接 / 断开 / 开启 USB 设备 `tcpip 5555`。
  - Android 11+ 配对弹窗（输入 `IP:Port` + 6 位配对码，调用 `server.pair()`）。
  - mDNS 雷达列表：实时展示局域网发现设备、匹配置信度徽标，点击“连接”或“配对”一键完成。

### 5.2 应用管理（App Manager）
- **列表与筛选**：通过 `pm list packages -f` 获取包名与 APK 路径，支持按“第三方 / 系统 / 全部”分栏及关键词过滤；按需批量补充版本名/版本号（避免启动时全量 `dumpsys` 阻塞）。
- **APK 安装**：
  - 支持文件选择或全窗口拖拽单个/多个 `.apk` 安装。
  - 可选常用安装参数：`-r`（覆盖安装）、`-d`（允许降级）、`-g`（自动授予运行时权限）、`-t`（允许测试包）。
- **单应用操作**：启动主 Activity、强行停止（`am force-stop`）、清除应用数据（`pm clear`）、导出 APK 至电脑（`device.pull`）、卸载（`device.uninstall`）。

### 5.3 交互式终端（Terminal）
- **会话机制**：
  - 支持多标签页、同设备多终端实例。
  - **推荐底层实现**：使用 `portable-pty` 拉起 `<选定 adb 路径> -s <serial> shell`（通过 `ADB` 与协议层共用同一个 5037 Server），天然获得完整的 PTY 窗口行列自适应（`cols`/`rows` resize）、`Ctrl+C` / `Ctrl+Z` 信号传递与 `top` / `vi` / `htop` 完美兼容；同时保留 `adb_client::shell` 作为无外部二进制依赖时的备用通路。
- **基础交互能力**：
  - 终端内文本搜索（`@xterm/addon-search`：`Ctrl/Cmd + F`、大小写/正则切换、上下跳转）。
  - URL 识别点击（`@xterm/addon-web-links`）。
  - 复制/粘贴快捷键（Windows/Linux `Ctrl+Shift+C/V` 与可选“选中即自动复制”）、右键上下文菜单、一键清屏、断开重连。
  - **快捷命令栏**：侧边/顶部可折叠常用指令片段（支持从 SQLite `command_snippets` 增删改），支持点击“填入命令行”或“直接执行”。
- **外观与个性化设置（持久化到 `app_settings`）**：
  - **内置配色方案**：跟随 Naive UI 明暗主题自动切换（如 `Light Clean` / `One Dark` / `Tokyo Night` / `Catppuccin Mocha`），支持独立选择终端主题。
  - **排版参数**：字体族（优先等宽字体栈，支持自定义系统字体名如 `JetBrains Mono` / `Cascadia Mono`）、字号（11–22px 滑块）、行高（1.0–1.6）、光标样式（`block` / `underline` / `bar`）与闪烁开关、滚回缓冲区行数（默认 5000 行）。

### 5.4 实时 Logcat（Logcat Viewer）
- **采集与解析**：
  - 每个设备的 Logcat 会话在 Rust 后端独立线程运行 `logcat -v threadtime` 流式读取。
  - 后端正则/状态机解析为结构化日志：`{ id, timestamp, pid, tid, level, tag, message, raw }`，按 50ms 批处理窗口通过 Tauri `Channel` 批量发送前端，避免高频 IPC 抖动。
- **过滤与排查能力**：
  - **级别过滤**：`V` / `D` / `I` / `W` / `E` / `F` 独立开关或最低级别选择。
  - **多维检索**：Tag 过滤、关键词/正则表达式过滤（支持排除表达式）。
  - **按应用包名过滤**：选择设备已安装包名后，后端通过 `pidof <pkg>`（或定时轮询 `ps -A`）将包名映射到当前进程 PID 进行过滤，并在进程重启导致 PID 变化时自动刷新跟踪。
  - **缓冲区选择**：支持 `main`、`system`、`crash`、`events`、`all` 切换。
- **视图控制与性能**：
  - 前端使用虚拟滚动列表渲染，设置内存环形缓冲区上限（默认 20,000 行，可配置），支持“暂停滚动 / 自动滚底”、“清空设备缓冲区（`logcat -c`）”、“清空当前视图”、“导出 `.log` 文件”。

### 5.5 scrcpy 投屏托管（Scrcpy Launcher）
- **进程池管理**：后端维护 `HashMap<InstanceId, ScrcpyProcess>`，支持同设备按不同参数（例如主屏 + 虚拟副屏）多开进程，监听退出码与 `stderr` 实时反馈给前端（不再出现“启动报错但界面显示成功”）。
- **参数可视化配置（支持全局预设 + 设备默认绑定）**：
  - **视频**：最大分辨率（原始 / 1920 / 1280 / 1024）、码率（如 8M / 16M）、最大帧率（60 / 90 / 120）、视频编码（H.264 / H.265 / AV1）。
  - **显示与控制**：投屏时熄屏（`-S`）、保持唤醒（`-w`）、窗口置顶（`--always-on-top`）、显示触摸点（`-t`）、只读模式（`-n`）、键盘/鼠标模式（SDK / UHID）。
  - **音频**：开启/关闭转发（`--no-audio`）、音频编解码（Opus / AAC / FLAC）。
  - **虚拟屏（scrcpy 3.0+）**：`--new-display=<W>x<H>/<DPI>` 与 `--start-app=<package>`，适合把手机某个 App 单独作为桌面小窗运行。

---

## 6. 前后端 IPC 契约草案

### 6.1 Tauri Commands

| 模块 | Command | 参数摘要 | 说明 |
| :--- | :--- | :--- | :--- |
| **Toolchain** | `toolchain_probe` | `{}` | 探测本机 `adb` / `scrcpy` 路径、版本与平台下载能力 |
| **Toolchain** | `toolchain_download` | `{ target: 'scrcpy' \| 'adb', mirror?: string, onProgress: Channel }` | 下载并校验安装对应平台官方包 |
| **Toolchain** | `toolchain_import_archive` | `{ path: string, kind: 'scrcpy' \| 'adb' }` | 离线手动导入本地压缩包 |
| **Toolchain** | `toolchain_save_paths` | `{ adbPath?: string, scrcpyPath?: string }` | 保存用户自定义路径 |
| **Device** | `adb_init_server` | `{}` | 确保 5037 服务就绪并启动 `track_devices` |
| **Device** | `device_list_known` | `{}` | 从 SQLite 读取已知设备指纹与备注列表 |
| **Device** | `device_update_meta` | `{ deviceUid, alias?, colorTag?, notes?, autoConnect?, defaultProfileId? }` | 更新设备备注与绑定配置 |
| **Device** | `device_connect_tcp` | `{ host: string, port: number }` | 连接无线设备并触发指纹采集入库 |
| **Device** | `device_pair_wireless` | `{ host: string, port: number, code: string }` | Android 11+ 无线配对 |
| **Device** | `mdns_scan_once` / `mdns_watch` | `{ enabled?: boolean }` | 启动/停止双路 mDNS 扫描并带出指纹匹配结果 |
| **App** | `app_list` | `{ serial: string, scope: 'user' \| 'system' \| 'all' }` | 列出已安装应用 |
| **App** | `app_install` | `{ serial: string, apkPaths: string[], flags: InstallFlags, onProgress: Channel }` | 批量安装 APK |
| **App** | `app_action` | `{ serial: string, pkg: string, action: 'launch' \| 'stop' \| 'clear' \| 'uninstall' \| 'pull', destPath?: string }` | 执行应用单步操作 |
| **Shell** | `shell_open` | `{ serial: string, cols: number, rows: number, onOutput: Channel<Uint8Array> }` | 打开 PTY 终端会话，返回 `sessionId` |
| **Shell** | `shell_write` | `{ sessionId: string, data: Uint8Array }` | 写入终端输入 |
| **Shell** | `shell_resize` | `{ sessionId: string, cols: number, rows: number }` | 同步 PTY 行列数 |
| **Shell** | `shell_close` | `{ sessionId: string }` | 关闭会话 |
| **Logcat** | `logcat_start` | `{ serial: string, buffers: string[], clearFirst?: boolean, onBatch: Channel<LogEntry[]> }` | 启动 Logcat 采集，返回 `streamId` |
| **Logcat** | `logcat_resolve_pkg_pid` | `{ serial: string, pkg: string }` | 解析目标包名当前 PID 列表 |
| **Logcat** | `logcat_stop` | `{ streamId: string }` | 停止采集流 |
| **Scrcpy** | `scrcpy_start` | `{ serial: string, options: ScrcpyOptions }` | 启动投屏进程，注入隔离环境变量 |
| **Scrcpy** | `scrcpy_stop` | `{ instanceId: string }` | 停止指定投屏进程 |
| **Settings** | `settings_get` / `settings_set` | `{ key: string, value?: JsonValue }` | 读写终端外观、网络镜像等配置 |

### 6.2 Tauri Events（后端主动推送）

| Event 名称 | 载荷摘要 | 用途 |
| :--- | :--- | :--- |
| `adb://devices-changed` | `ActiveDeviceView[]`（含在线状态 + 已关联 `deviceUid` / `alias`） | 刷新在线设备栏与工作区上下文 |
| `mdns://services-updated` | `MdnsDiscoveredItem[]`（含置信度与匹配到的已知设备信息） | 实时更新无线雷达列表 |
| `scrcpy://exited` | `{ instanceId, serial, code?, stderrTail? }` | 更新投屏状态并提示异常原因 |
| `adb://server-status` | `{ running: boolean, version?: string, error?: string }` | 顶部/状态栏工具链健康提示 |

---

## 7. 前端 UI 结构规划（Vue 3 + Naive UI）

采用经典的桌面端三区工作台布局（`NLayout` + `NLayoutSider` + `NLayoutContent`）：

- **全局顶部/左侧边栏（设备切换器）**：
  - 展示在线设备（优先显示 `alias` + 型号 + USB/Wi-Fi 图标 + 颜色标识条）与“无线发现 / 已知设备”入口。
  - 底部常驻工具链状态胶囊（`adb` / `scrcpy` 就绪状态，未就绪时点击弹出工具链向导）。
- **主功能导航（针对当前选中设备）**：
  1. **设备概览**：硬件指纹、电量/存储/分辨率状态、快捷操作（截屏、重启、开启 `tcpip 5555`、编辑设备备注与默认投屏方案）。
  2. **无线发现（mDNS）**：局域网雷达扫描、已识别设备标牌、一键连接 / 配对向导。
  3. **应用管理**：`NDataTable` + 拖拽上传区（支持批量 APK 安装进度抽屉）。
  4. **终端**：顶部多 Tab 标签栏 + `@xterm/xterm` 画布 + 右侧可折叠快捷命令抽屉 + 终端外观设置弹窗。
  5. **Logcat**：顶部过滤工具栏（级别标签、包名选择器、Tag/正则搜索、暂停/清屏/导出）+ 虚拟滚动日志流。
  6. **投屏（Scrcpy）**：左侧参数分组表单（视频 / 显示 / 音频 / 虚拟屏）+ 预设管理 + 底部运行中投屏实例卡片。
- **全局设置**：主题（跟随系统/浅色/深色）、工具链探测与下载管理（含代理/镜像与 Linux 引导）、终端默认外观、数据库备份/路径查看。

---

## 8. 推荐仓库目录结构

```text
adbmaid/
├── SPEC.md
├── package.json
├── vite.config.ts
├── src/
│   ├── api/                    # Tauri invoke / event / channel 类型安全封装
│   ├── stores/                 # Pinia: devices, mdns, apps, terminal, logcat, scrcpy, settings
│   ├── components/
│   │   ├── DeviceBadge.vue     # 融合指纹别名、颜色标签与传输类型的设备徽章
│   │   ├── ToolchainModal.vue  # 主机探测 / 自动下载 / 离线导入向导
│   │   └── TerminalPane.vue    # xterm.js 实例封装（搜索栏、右键菜单、主题响应）
│   ├── views/
│   │   ├── DevicesView.vue
│   │   ├── MdnsRadarView.vue
│   │   ├── AppsView.vue
│   │   ├── TerminalView.vue
│   │   ├── LogcatView.vue
│   │   ├── ScrcpyView.vue
│   │   └── SettingsView.vue
│   └── App.vue                 # NConfigProvider + NMessageProvider + NLayout
└── src-tauri/
    ├── Cargo.toml
    ├── tauri.conf.json
    └── src/
        ├── db/                 # SQLite 连接池、migrations、设备指纹 DAO
        ├── toolchain/          # 主机探测、os-release 识别、GitHub 下载与 SHA256 校验
        ├── adb/                # ADBServer 守护、track_devices、指纹采集、app 操作
        ├── mdns/               # ADB mDNS + mdns-sd 双路发现与指纹匹配引擎
        ├── shell/              # portable-pty 会话池
        ├── logcat/             # logcat 流式采集、threadtime 解析与批处理推送
        ├── scrcpy/             # 参数构建、环境变量隔离、Child 进程池
        ├── state.rs
        └── lib.rs
```

---

## 9. 分阶段实施路线（Milestones）

### Milestone 1：骨架、工具链与设备底座
- 初始化 Tauri v2 + Vue 3 + Naive UI 项目骨架与 SQLite 迁移。
- 实现 `Toolchain Manager`（探测本机 `adb`/`scrcpy`、Win/macOS/Linux x86_64 自动下载与校验、Linux arm64/非官方平台引导）。
- 跑通 `ADBServer` 启动、`track_devices` 实时列表及设备基础信息采集。

### Milestone 2：设备指纹 + mDNS 发现 + scrcpy 托管
- 完成设备指纹采集、SQLite `devices` / `device_endpoints` 持久化与备注编辑。
- 实现双路 mDNS 雷达及“连接前身份识别”。
- 完成 `scrcpy` 参数面板、预设保存与隔离环境变量拉起/进程监控。

### Milestone 3：应用管理 + 完整外观终端 + 实时 Logcat
- 实现 `AppsView`（列表、搜索、批量拖拽安装、卸载/导出/强停）。
- 实现多标签 `xterm.js` + `portable-pty` 终端，补齐搜索、快捷指令库、字体/配色主题配置。
- 实现 `Logcat` 流式解析、虚拟滚动、包名转 PID 过滤与日志导出。
