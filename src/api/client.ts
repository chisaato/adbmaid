import { Channel, invoke } from '@tauri-apps/api/core'
import { listen, type UnlistenFn } from '@tauri-apps/api/event'
import type {
  ActiveDeviceView,
  AdbServerStatusEvent,
  AppItem,
  CommandSnippet,
  DeviceEndpointRecord,
  DeviceMetaUpdate,
  DeviceRecord,
  DeviceTelemetry,
  DownloadProgress,
  InstallFlags,
  InstallProgress,
  LogEntry,
  MdnsDiscoveredItem,
  ScrcpyExitedEvent,
  ScrcpyInstance,
  ScrcpyOptions,
  ScrcpyProfile,
  ToolchainProbeResult,
} from './types'

export const isTauriEnv = (): boolean =>
  typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window

// ================= 浏览器预览模式 Mock 数据（非 Tauri 环境自动启用） =================

const mockKnownDevices: DeviceRecord[] = [
  {
    deviceUid: 'uid-xiaomi-14',
    hardwareSerial: 'a1b2c3d4',
    bootSerial: 'a1b2c3d4',
    androidId: '98f1e2c3a4b5d678',
    brand: 'Xiaomi',
    manufacturer: 'Xiaomi',
    model: '23116PN5BC',
    marketName: 'Xiaomi 14',
    deviceCode: 'houji',
    abi: 'arm64-v8a',
    androidVersion: '15',
    sdkInt: 35,
    buildFingerprint: 'Xiaomi/houji/houji:15/AP3A.240905.015/OS2.0.10.0:user/release-keys',
    alias: '主力测试机',
    colorTag: '#18a058',
    notes: 'HyperOS 2.0，日常主力调试机',
    autoConnect: true,
    defaultProfileId: 'builtin-balanced',
    firstSeenAt: Date.now() - 86400_000 * 14,
    lastSeenAt: Date.now() - 30_000,
  },
  {
    deviceUid: 'uid-pixel-8',
    hardwareSerial: '3A191FDJG00291',
    bootSerial: '3A191FDJG00291',
    androidId: '1122334455667788',
    brand: 'Google',
    manufacturer: 'Google',
    model: 'Pixel 8',
    marketName: 'Pixel 8',
    deviceCode: 'shiba',
    abi: 'arm64-v8a',
    androidVersion: '15',
    sdkInt: 35,
    buildFingerprint: 'google/shiba/shiba:15/AP4A.241205.013/12621605:user/release-keys',
    alias: '原生 AOSP 机',
    colorTag: '#2080f0',
    notes: 'Android 15 QPR 兼容性验证',
    autoConnect: false,
    defaultProfileId: 'builtin-virtual-display',
    firstSeenAt: Date.now() - 86400_000 * 30,
    lastSeenAt: Date.now() - 3600_000 * 5,
  },
]

const mockActiveDevices: ActiveDeviceView[] = [
  {
    serial: 'a1b2c3d4',
    state: 'device',
    transportType: 'usb',
    deviceUid: 'uid-xiaomi-14',
    alias: '主力测试机',
    colorTag: '#18a058',
    model: '23116PN5BC',
    marketName: 'Xiaomi 14',
    brand: 'Xiaomi',
    androidVersion: '15',
  },
  {
    serial: '192.168.1.108:38421',
    state: 'device',
    transportType: 'tcp',
    deviceUid: 'uid-pixel-8',
    alias: '原生 AOSP 机',
    colorTag: '#2080f0',
    model: 'Pixel 8',
    marketName: 'Pixel 8',
    brand: 'Google',
    androidVersion: '15',
  },
]

const mockProfiles: ScrcpyProfile[] = [
  {
    id: 'builtin-balanced',
    name: '默认均衡',
    isBuiltin: true,
    updatedAt: Date.now(),
    options: {
      maxSize: 1920,
      videoBitRate: '8M',
      maxFps: 60,
      videoCodec: 'h264',
      turnScreenOff: false,
      stayAwake: true,
      alwaysOnTop: false,
      showTouches: false,
      readOnly: false,
      keyboardMode: 'sdk',
      mouseMode: 'sdk',
      noAudio: false,
      audioCodec: 'opus',
      newDisplay: null,
      startApp: null,
      windowTitle: null,
      extraArgs: null,
    },
  },
  {
    id: 'builtin-virtual-display',
    name: '虚拟副屏 1080p',
    isBuiltin: true,
    updatedAt: Date.now(),
    options: {
      maxSize: null,
      videoBitRate: '12M',
      maxFps: 60,
      videoCodec: 'h264',
      turnScreenOff: false,
      stayAwake: true,
      alwaysOnTop: false,
      showTouches: false,
      readOnly: false,
      keyboardMode: 'uhid',
      mouseMode: 'sdk',
      noAudio: false,
      audioCodec: 'opus',
      newDisplay: '1920x1080/320',
      startApp: null,
      windowTitle: 'adbmaid 虚拟副屏',
      extraArgs: null,
    },
  },
]

const mockSnippets: CommandSnippet[] = [
  {
    id: 'snip-1',
    title: '查看当前前台 Activity',
    category: '调试',
    command: "dumpsys window | grep -E 'mCurrentFocus|mFocusedApp'",
    autoEnter: true,
    sortOrder: 1,
  },
  {
    id: 'snip-2',
    title: '查看无线局域网 IP',
    category: '网络',
    command: "ip addr show wlan0 | grep 'inet '",
    autoEnter: true,
    sortOrder: 2,
  },
  {
    id: 'snip-3',
    title: '查看屏幕分辨率与 DPI',
    category: '显示',
    command: 'wm size && wm density',
    autoEnter: true,
    sortOrder: 3,
  },
]

const mockSettings = new Map<string, unknown>()
const mockScrcpyRunning: ScrcpyInstance[] = []
const mockTerminalEchoers = new Map<string, (data: Uint8Array) => void>()
const mockLogTimers = new Map<string, number>()

// ================= API 封装 =================

export const api = {
  async probeToolchain(): Promise<ToolchainProbeResult> {
    if (!isTauriEnv()) {
      return {
        os: 'linux',
        arch: 'x86_64',
        canAutoDownloadScrcpy: true,
        canAutoDownloadAdb: true,
        linuxDistro: {
          id: 'fedora',
          prettyName: 'Fedora Linux 45 (原型预览)',
          installHint: 'sudo dnf install android-tools',
        },
        adb: {
          found: true,
          path: '/usr/bin/adb',
          version: 'Android Debug Bridge version 1.0.41',
          source: 'system',
          serverPath: null,
          isOutdated: false,
        },
        scrcpy: {
          found: true,
          path: '/usr/local/bin/scrcpy',
          version: '3.1',
          source: 'managed',
          serverPath: '/usr/local/share/scrcpy/scrcpy-server',
          isOutdated: false,
        },
      }
    }
    return invoke('toolchain_probe')
  },

  async downloadToolchain(
    target: 'scrcpy' | 'adb',
    mirror: string | undefined,
    onProgress: (p: DownloadProgress) => void
  ): Promise<string> {
    if (!isTauriEnv()) {
      onProgress({ stage: 'downloading', receivedBytes: 512000, totalBytes: 1024000, message: '模拟下载中...' })
      await new Promise((r) => setTimeout(r, 500))
      onProgress({ stage: 'done', receivedBytes: 1024000, totalBytes: 1024000, message: 'v3.1 下载完成（预览模式）' })
      return 'v3.1'
    }
    const ch = new Channel<DownloadProgress>()
    ch.onmessage = onProgress
    return invoke('toolchain_download', { target, mirror: mirror || null, onProgress: ch })
  },

  async importToolchainArchive(path: string, kind: 'scrcpy' | 'adb'): Promise<string> {
    if (!isTauriEnv()) return path
    return invoke('toolchain_import_archive', { path, kind })
  },

  async saveToolchainPaths(adbPath?: string, scrcpyPath?: string): Promise<void> {
    if (!isTauriEnv()) return
    return invoke('toolchain_save_paths', { adbPath: adbPath ?? null, scrcpyPath: scrcpyPath ?? null })
  },

  async initAdbServer(): Promise<ActiveDeviceView[]> {
    if (!isTauriEnv()) return [...mockActiveDevices]
    return invoke('adb_init_server')
  },

  async setAutoScan(enabled: boolean): Promise<void> {
    if (!isTauriEnv()) return
    return invoke('adb_set_auto_scan', { enabled })
  },

  async listActiveDevices(): Promise<ActiveDeviceView[]> {
    if (!isTauriEnv()) return [...mockActiveDevices]
    return invoke('device_list_active')
  },

  async listKnownDevices(): Promise<DeviceRecord[]> {
    if (!isTauriEnv()) return [...mockKnownDevices]
    return invoke('device_list_known')
  },

  async listDeviceEndpoints(deviceUid: string): Promise<DeviceEndpointRecord[]> {
    if (!isTauriEnv()) {
      return [
        {
          id: 1,
          deviceUid,
          transportType: 'usb',
          adbSerial: 'a1b2c3d4',
          mdnsServiceName: null,
          mdnsRegType: null,
          lastIp: null,
          lastPort: null,
          lastSeenAt: Date.now() - 30000,
        },
        {
          id: 2,
          deviceUid,
          transportType: 'mdns_connect',
          adbSerial: '192.168.1.108:38421',
          mdnsServiceName: 'adb-a1b2c3d4-7kX9pQ',
          mdnsRegType: '_adb-tls-connect._tcp',
          lastIp: '192.168.1.108',
          lastPort: 38421,
          lastSeenAt: Date.now() - 120000,
        },
      ]
    }
    return invoke('device_endpoints', { deviceUid })
  },

  async updateDeviceMeta(update: DeviceMetaUpdate): Promise<void> {
    if (!isTauriEnv()) {
      const target = mockKnownDevices.find((d) => d.deviceUid === update.deviceUid)
      if (target) {
        if (update.alias !== undefined) target.alias = update.alias
        if (update.colorTag !== undefined) target.colorTag = update.colorTag
        if (update.notes !== undefined) target.notes = update.notes
        if (update.autoConnect !== undefined && update.autoConnect !== null)
          target.autoConnect = update.autoConnect
        if (update.defaultProfileId !== undefined) target.defaultProfileId = update.defaultProfileId
      }
      return
    }
    return invoke('device_update_meta', { update })
  },

  async connectTcp(host: string, port: number): Promise<DeviceRecord> {
    if (!isTauriEnv()) {
      const serial = `${host}:${port}`
      if (!mockActiveDevices.find((d) => d.serial === serial)) {
        mockActiveDevices.push({
          serial,
          state: 'device',
          transportType: 'tcp',
          deviceUid: 'uid-xiaomi-14',
          alias: '主力测试机',
          colorTag: '#18a058',
          model: '23116PN5BC',
          marketName: 'Xiaomi 14',
          brand: 'Xiaomi',
          androidVersion: '15',
        })
      }
      return mockKnownDevices[0]
    }
    return invoke('device_connect_tcp', { host, port })
  },

  async disconnectTcp(host: string, port: number): Promise<void> {
    if (!isTauriEnv()) {
      const idx = mockActiveDevices.findIndex((d) => d.serial === `${host}:${port}`)
      if (idx >= 0) mockActiveDevices.splice(idx, 1)
      return
    }
    return invoke('device_disconnect_tcp', { host, port })
  },

  async pairWireless(host: string, port: number, code: string): Promise<void> {
    if (!isTauriEnv()) {
      await new Promise((r) => setTimeout(r, 300))
      if (code.length < 6) throw new Error('配对码应为 6 位数字')
      return
    }
    return invoke('device_pair_wireless', { host, port, code })
  },

  async enableTcpip(serial: string, port = 5555): Promise<string> {
    if (!isTauriEnv()) return `restarting in TCP mode port: ${port} (${serial})`
    return invoke('device_tcpip', { serial, port })
  },

  async rebootDevice(serial: string, mode: 'system' | 'bootloader' | 'recovery' | 'sideload'): Promise<void> {
    if (!isTauriEnv()) return
    return invoke('device_reboot', { serial, mode })
  },

  async getDeviceTelemetry(serial: string): Promise<DeviceTelemetry> {
    if (!isTauriEnv()) {
      return {
        batteryLevel: serial.includes(':') ? 78 : 92,
        batteryCharging: !serial.includes(':'),
        screenSize: '1200x2670',
        screenDensity: '480',
        ipAddress: '192.168.1.108',
        storageSummary: '已用 94G / 共 224G (42%)',
      }
    }
    return invoke('device_telemetry', { serial })
  },

  async mdnsScanOnce(): Promise<MdnsDiscoveredItem[]> {
    if (!isTauriEnv()) {
      return [
        {
          id: '1',
          serviceName: 'adb-a1b2c3d4-7kX9pQ',
          regType: '_adb-tls-connect._tcp',
          ip: '192.168.1.108',
          port: 41823,
          status: 'ready_connect',
          confidence: 'exact',
          matchedDeviceUid: 'uid-xiaomi-14',
          matchedAlias: '主力测试机',
          matchedModel: 'Xiaomi 14',
          matchedColorTag: '#18a058',
          source: 'both',
        },
        {
          id: '2',
          serviceName: 'adb-3A191FDJG00291-mN4vB',
          regType: '_adb-tls-connect._tcp',
          ip: '192.168.1.109',
          port: 38421,
          status: 'connected',
          confidence: 'exact',
          matchedDeviceUid: 'uid-pixel-8',
          matchedAlias: '原生 AOSP 机',
          matchedModel: 'Pixel 8',
          matchedColorTag: '#2080f0',
          source: 'adb',
        },
        {
          id: '3',
          serviceName: 'adb-unknown-pair-9921',
          regType: '_adb-tls-pairing._tcp',
          ip: '192.168.1.115',
          port: 37119,
          status: 'ready_pair',
          confidence: 'unknown',
          matchedDeviceUid: null,
          matchedAlias: null,
          matchedModel: null,
          matchedColorTag: null,
          source: 'mdns-sd',
        },
      ]
    }
    return invoke('mdns_scan_once')
  },

  async mdnsWatch(enabled: boolean): Promise<void> {
    if (!isTauriEnv()) return
    return invoke('mdns_watch', { enabled })
  },

  async listApps(serial: string, scope: 'user' | 'system' | 'all'): Promise<AppItem[]> {
    if (!isTauriEnv()) {
      const userApps: AppItem[] = [
        { packageName: 'com.tencent.mm', apkPath: '/data/app/~~abc==/com.tencent.mm/base.apk', isSystem: false, versionName: '8.0.50' },
        { packageName: 'com.eg.android.AlipayGphone', apkPath: '/data/app/~~def==/com.eg.android.AlipayGphone/base.apk', isSystem: false, versionName: '10.6.10' },
        { packageName: 'org.mozilla.firefox', apkPath: '/data/app/~~ghi==/org.mozilla.firefox/base.apk', isSystem: false, versionName: '132.0' },
        { packageName: 'com.example.adbmaid.demo', apkPath: '/data/app/~~jkl==/com.example.adbmaid.demo/base.apk', isSystem: false, versionName: '1.2.0' },
      ]
      const sysApps: AppItem[] = [
        { packageName: 'com.android.settings', apkPath: '/system_ext/priv-app/Settings/Settings.apk', isSystem: true, versionName: '15' },
        { packageName: 'com.android.systemui', apkPath: '/system_ext/priv-app/SystemUI/SystemUI.apk', isSystem: true, versionName: '15' },
      ]
      if (scope === 'user') return userApps
      if (scope === 'system') return sysApps
      return [...userApps, ...sysApps]
    }
    return invoke('app_list', { serial, scope })
  },

  async installApks(
    serial: string,
    apkPaths: string[],
    flags: InstallFlags,
    onProgress: (p: InstallProgress) => void
  ): Promise<void> {
    if (!isTauriEnv()) {
      for (let i = 0; i < apkPaths.length; i++) {
        onProgress({ index: i + 1, total: apkPaths.length, apkPath: apkPaths[i], status: 'installing', message: '正在安装...' })
        await new Promise((r) => setTimeout(r, 400))
        onProgress({ index: i + 1, total: apkPaths.length, apkPath: apkPaths[i], status: 'success', message: 'Success (预览模拟)' })
      }
      return
    }
    const ch = new Channel<InstallProgress>()
    ch.onmessage = onProgress
    return invoke('app_install', { serial, apkPaths, flags, onProgress: ch })
  },

  async appAction(
    serial: string,
    pkg: string,
    action: 'launch' | 'stop' | 'clear' | 'uninstall' | 'pull',
    destPath?: string
  ): Promise<string> {
    if (!isTauriEnv()) {
      return `已执行 ${action} -> ${pkg} (${serial})`
    }
    return invoke('app_action', { serial, pkg, action, destPath: destPath ?? null })
  },

  async shellOpen(
    serial: string,
    cols: number,
    rows: number,
    onOutput: (data: Uint8Array) => void
  ): Promise<string> {
    if (!isTauriEnv()) {
      const sid = `mock-pty-${Date.now()}`
      mockTerminalEchoers.set(sid, onOutput)
      const enc = new TextEncoder()
      setTimeout(() => {
        onOutput(enc.encode(`\r\n\x1b[32madbmaid PTY 原型终端\x1b[0m (${serial})\r\n\x1b[36mhouji:/ $\x1b[0m `))
      }, 60)
      return sid
    }
    const ch = new Channel<number[]>()
    ch.onmessage = (bytes) => onOutput(new Uint8Array(bytes))
    return invoke('shell_open', { serial, cols, rows, onOutput: ch })
  },

  async shellWrite(sessionId: string, data: Uint8Array): Promise<void> {
    if (!isTauriEnv()) {
      const cb = mockTerminalEchoers.get(sessionId)
      if (cb) {
        const text = new TextDecoder().decode(data)
        if (text === '\r') {
          cb(new TextEncoder().encode('\r\n\x1b[36mhouji:/ $\x1b[0m '))
        } else {
          cb(data)
        }
      }
      return
    }
    return invoke('shell_write', { sessionId, data: Array.from(data) })
  },

  async shellResize(sessionId: string, cols: number, rows: number): Promise<void> {
    if (!isTauriEnv()) return
    return invoke('shell_resize', { sessionId, cols, rows })
  },

  async shellClose(sessionId: string): Promise<void> {
    if (!isTauriEnv()) {
      mockTerminalEchoers.delete(sessionId)
      return
    }
    return invoke('shell_close', { sessionId })
  },

  async listSnippets(): Promise<CommandSnippet[]> {
    if (!isTauriEnv()) return [...mockSnippets]
    return invoke('snippet_list')
  },

  async saveSnippet(snippet: CommandSnippet): Promise<void> {
    if (!isTauriEnv()) {
      const idx = mockSnippets.findIndex((s) => s.id === snippet.id)
      if (idx >= 0) mockSnippets[idx] = snippet
      else mockSnippets.push(snippet)
      return
    }
    return invoke('snippet_save', { snippet })
  },

  async deleteSnippet(id: string): Promise<void> {
    if (!isTauriEnv()) {
      const idx = mockSnippets.findIndex((s) => s.id === id)
      if (idx >= 0) mockSnippets.splice(idx, 1)
      return
    }
    return invoke('snippet_delete', { id })
  },

  async logcatStart(
    _serial: string,
    buffers: string[],
    clearFirst: boolean,
    onBatch: (entries: LogEntry[]) => void
  ): Promise<string> {
    if (!isTauriEnv()) {
      const id = `mock-log-${Date.now()}`
      let seq = 1
      const levels = ['V', 'D', 'I', 'W', 'E']
      const tags = ['ActivityManager', 'WindowManager', 'OkHttp', 'Compose', 'adbmaidDemo']
      const timer = window.setInterval(() => {
        const now = new Date()
        const ts = now.toTimeString().slice(0, 8) + '.' + String(now.getMilliseconds()).padStart(3, '0')
        const lvl = levels[seq % levels.length]
        const tag = tags[seq % tags.length]
        onBatch([
          {
            id: seq++,
            timestamp: `11-18 ${ts}`,
            pid: String(2400 + (seq % 3) * 100),
            tid: '2451',
            level: lvl,
            tag,
            message: `[${buffers.join(',')}] 模拟日志事件 #${seq} (clear=${clearFirst})`,
            raw: `11-18 ${ts} 2400 2451 ${lvl} ${tag}: 模拟日志事件 #${seq}`,
          },
        ])
      }, 600)
      mockLogTimers.set(id, timer)
      return id
    }
    const ch = new Channel<LogEntry[]>()
    ch.onmessage = onBatch
    return invoke('logcat_start', { serial: _serial, buffers, clearFirst, onBatch: ch })
  },

  async logcatResolvePkgPid(serial: string, pkg: string): Promise<string[]> {
    if (!isTauriEnv()) {
      if (!pkg) return []
      return ['2400', '2500']
    }
    return invoke('logcat_resolve_pkg_pid', { serial, pkg })
  },

  async logcatStop(streamId: string): Promise<void> {
    if (!isTauriEnv()) {
      const t = mockLogTimers.get(streamId)
      if (t) {
        clearInterval(t)
        mockLogTimers.delete(streamId)
      }
      return
    }
    return invoke('logcat_stop', { streamId })
  },

  async scrcpyStart(serial: string, profileName: string | null, options: ScrcpyOptions): Promise<ScrcpyInstance> {
    if (!isTauriEnv()) {
      const inst: ScrcpyInstance = {
        instanceId: `mock-scrcpy-${Date.now()}`,
        serial,
        profileName,
        startedAt: Date.now(),
      }
      mockScrcpyRunning.push(inst)
      return inst
    }
    return invoke('scrcpy_start', { serial, profileName, options })
  },

  async scrcpyStop(instanceId: string): Promise<void> {
    if (!isTauriEnv()) {
      const idx = mockScrcpyRunning.findIndex((i) => i.instanceId === instanceId)
      if (idx >= 0) mockScrcpyRunning.splice(idx, 1)
      return
    }
    return invoke('scrcpy_stop', { instanceId })
  },

  async scrcpyListRunning(): Promise<ScrcpyInstance[]> {
    if (!isTauriEnv()) return [...mockScrcpyRunning]
    return invoke('scrcpy_list_running')
  },

  async scrcpyListProfiles(): Promise<ScrcpyProfile[]> {
    if (!isTauriEnv()) return [...mockProfiles]
    return invoke('scrcpy_list_profiles')
  },

  async scrcpySaveProfile(profile: ScrcpyProfile): Promise<void> {
    if (!isTauriEnv()) {
      const idx = mockProfiles.findIndex((p) => p.id === profile.id)
      if (idx >= 0) mockProfiles[idx] = profile
      else mockProfiles.push(profile)
      return
    }
    return invoke('scrcpy_save_profile', { profile })
  },

  async scrcpyDeleteProfile(id: string): Promise<void> {
    if (!isTauriEnv()) {
      const idx = mockProfiles.findIndex((p) => p.id === id && !p.isBuiltin)
      if (idx >= 0) mockProfiles.splice(idx, 1)
      return
    }
    return invoke('scrcpy_delete_profile', { id })
  },

  async getSetting<T = unknown>(key: string): Promise<T | null> {
    if (!isTauriEnv()) return (mockSettings.get(key) as T) ?? null
    return invoke('settings_get', { key })
  },

  async setSetting(key: string, value: unknown): Promise<void> {
    if (!isTauriEnv()) {
      mockSettings.set(key, value)
      return
    }
    return invoke('settings_set', { key, value })
  },

  async getAppDataDir(): Promise<string> {
    if (!isTauriEnv()) return '~/.local/share/com.adbmaid.app (预览模式)'
    return invoke('app_data_info')
  },

  // ================= Events =================
  async onDevicesChanged(cb: (devices: ActiveDeviceView[]) => void): Promise<UnlistenFn> {
    if (!isTauriEnv()) return () => {}
    return listen<ActiveDeviceView[]>('adb://devices-changed', (e) => cb(e.payload))
  },

  async onMdnsUpdated(cb: (items: MdnsDiscoveredItem[]) => void): Promise<UnlistenFn> {
    if (!isTauriEnv()) return () => {}
    return listen<MdnsDiscoveredItem[]>('mdns://services-updated', (e) => cb(e.payload))
  },

  async onScrcpyExited(cb: (ev: ScrcpyExitedEvent) => void): Promise<UnlistenFn> {
    if (!isTauriEnv()) return () => {}
    return listen<ScrcpyExitedEvent>('scrcpy://exited', (e) => cb(e.payload))
  },

  async onServerStatus(cb: (ev: AdbServerStatusEvent) => void): Promise<UnlistenFn> {
    if (!isTauriEnv()) return () => {}
    return listen<AdbServerStatusEvent>('adb://server-status', (e) => cb(e.payload))
  },
}
