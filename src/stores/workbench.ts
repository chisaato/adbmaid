import { computed, ref } from 'vue'
import { defineStore } from 'pinia'
import { useOsTheme } from 'naive-ui'
import { api } from '@/api/client'
import type {
  ActiveDeviceView,
  AdbServerStatusEvent,
  DeviceRecord,
  TerminalAppearance,
  ToolchainProbeResult,
} from '@/api/types'

export type NavKey =
  | 'devices'
  | 'mdns'
  | 'apps'
  | 'terminal'
  | 'logcat'
  | 'scrcpy'
  | 'settings'

export const DEFAULT_TERMINAL_APPEARANCE: TerminalAppearance = {
  theme: 'auto',
  fontFamily: '"JetBrains Mono", "Cascadia Mono", "Fira Code", monospace',
  fontSize: 13,
  lineHeight: 1.25,
  cursorStyle: 'bar',
  cursorBlink: true,
  scrollback: 5000,
  copyOnSelect: false,
}

export const useWorkbenchStore = defineStore('workbench', () => {
  const osTheme = useOsTheme()
  const themeMode = ref<'auto' | 'light' | 'dark'>('auto')
  const isDark = computed(() => {
    if (themeMode.value === 'dark') return true
    if (themeMode.value === 'light') return false
    return osTheme.value === 'dark'
  })

  const activeNav = ref<NavKey>('devices')
  const toolchainModalOpen = ref(false)
  const toolchain = ref<ToolchainProbeResult | null>(null)
  const serverStatus = ref<AdbServerStatusEvent>({
    running: false,
    version: null,
    error: null,
  })

  const activeDevices = ref<ActiveDeviceView[]>([])
  const knownDevices = ref<DeviceRecord[]>([])
  const selectedSerial = ref<string | null>(null)
  const autoScanEnabled = ref<boolean>(true)

  async function setAutoScanEnabled(enabled: boolean) {
    autoScanEnabled.value = enabled
    try {
      await api.setAutoScan(enabled)
      if (enabled) {
        await refreshActiveDevices()
      }
    } catch {
      // 忽略预览模式或后端未就绪异常
    }
  }

  const terminalAppearance = ref<TerminalAppearance>({ ...DEFAULT_TERMINAL_APPEARANCE })
  const logcatMaxLines = ref<number>(20000)
  // 每次全局拖拽安装完成后递增，供 AppsView 监听刷新列表
  const apkInstallSeq = ref<number>(0)

  function notifyApkInstalled() {
    apkInstallSeq.value += 1
  }

  const selectedDevice = computed(
    () => activeDevices.value.find((d) => d.serial === selectedSerial.value) ?? null
  )

  async function refreshToolchain() {
    toolchain.value = await api.probeToolchain()
    return toolchain.value
  }

  async function refreshKnownDevices() {
    knownDevices.value = await api.listKnownDevices()
  }

  async function refreshActiveDevices() {
    const list = await api.listActiveDevices()
    activeDevices.value = list
    ensureSelectedDevice(list)
  }

  function ensureSelectedDevice(list: ActiveDeviceView[]) {
    if (selectedSerial.value && list.some((d) => d.serial === selectedSerial.value)) {
      return
    }
    const online = list.find((d) => d.state === 'device') ?? list[0]
    selectedSerial.value = online ? online.serial : null
  }

  async function loadSettings() {
    const savedTheme = await api.getSetting<'auto' | 'light' | 'dark'>('ui.theme')
    if (savedTheme) themeMode.value = savedTheme

    const savedTerm = await api.getSetting<Partial<TerminalAppearance>>('terminal.appearance')
    if (savedTerm) {
      terminalAppearance.value = { ...DEFAULT_TERMINAL_APPEARANCE, ...savedTerm }
    }

    const savedMaxLines = await api.getSetting<number>('logcat.maxLines')
    if (typeof savedMaxLines === 'number' && savedMaxLines > 0) {
      logcatMaxLines.value = savedMaxLines
    }
  }

  async function setThemeMode(mode: 'auto' | 'light' | 'dark') {
    themeMode.value = mode
    await api.setSetting('ui.theme', mode)
  }

  async function saveTerminalAppearance(next: TerminalAppearance) {
    terminalAppearance.value = { ...next }
    await api.setSetting('terminal.appearance', next)
  }

  async function init() {
    await loadSettings()
    await refreshToolchain()

    await api.onDevicesChanged((list) => {
      activeDevices.value = list
      ensureSelectedDevice(list)
      void refreshKnownDevices()
    })
    await api.onServerStatus((ev) => {
      serverStatus.value = ev
    })

    try {
      const list = await api.initAdbServer()
      activeDevices.value = list
      ensureSelectedDevice(list)
    } catch {
      // 忽略初始化异常，由 serverStatus / toolchainModal 引导
    }
    await refreshKnownDevices()
  }

  return {
    themeMode,
    isDark,
    activeNav,
    toolchainModalOpen,
    toolchain,
    serverStatus,
    activeDevices,
    knownDevices,
    selectedSerial,
    selectedDevice,
    autoScanEnabled,
    terminalAppearance,
    logcatMaxLines,
    apkInstallSeq,
    init,
    refreshToolchain,
    refreshActiveDevices,
    refreshKnownDevices,
    setThemeMode,
    setAutoScanEnabled,
    saveTerminalAppearance,
    notifyApkInstalled,
  }
})
