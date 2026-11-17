export interface ToolBinaryInfo {
  found: boolean
  path: string | null
  version: string | null
  source: 'custom' | 'managed' | 'system' | 'missing'
  serverPath: string | null
  isOutdated: boolean
}

export interface LinuxDistroInfo {
  id: string
  prettyName: string
  installHint: string
}

export interface ToolchainProbeResult {
  os: string
  arch: string
  canAutoDownloadScrcpy: boolean
  canAutoDownloadAdb: boolean
  linuxDistro: LinuxDistroInfo | null
  adb: ToolBinaryInfo
  scrcpy: ToolBinaryInfo
}

export interface DownloadProgress {
  stage: 'fetching_meta' | 'downloading' | 'verifying' | 'extracting' | 'done' | 'error'
  receivedBytes: number
  totalBytes: number | null
  message: string
}

export interface DeviceRecord {
  deviceUid: string
  hardwareSerial: string | null
  bootSerial: string | null
  androidId: string | null
  brand: string | null
  manufacturer: string | null
  model: string | null
  marketName: string | null
  deviceCode: string | null
  abi: string | null
  androidVersion: string | null
  sdkInt: number | null
  buildFingerprint: string | null
  alias: string | null
  colorTag: string | null
  notes: string | null
  autoConnect: boolean
  defaultProfileId: string | null
  firstSeenAt: number
  lastSeenAt: number
}

export interface DeviceEndpointRecord {
  id: number
  deviceUid: string
  transportType: string
  adbSerial: string | null
  mdnsServiceName: string | null
  mdnsRegType: string | null
  lastIp: string | null
  lastPort: number | null
  lastSeenAt: number
}

export interface ActiveDeviceView {
  serial: string
  state: 'device' | 'offline' | 'unauthorized' | 'unknown' | string
  transportType: 'usb' | 'tcp'
  deviceUid: string | null
  alias: string | null
  colorTag: string | null
  model: string | null
  marketName: string | null
  brand: string | null
  androidVersion: string | null
}

export interface DeviceMetaUpdate {
  deviceUid: string
  alias?: string | null
  colorTag?: string | null
  notes?: string | null
  autoConnect?: boolean | null
  defaultProfileId?: string | null
}

export interface DeviceTelemetry {
  batteryLevel: number | null
  batteryCharging: boolean | null
  screenSize: string | null
  screenDensity: string | null
  ipAddress: string | null
  storageSummary: string | null
}

export interface MdnsDiscoveredItem {
  id: string
  serviceName: string
  regType: string
  ip: string
  port: number
  status: 'connected' | 'ready_connect' | 'ready_pair'
  confidence: 'exact' | 'history' | 'suspect' | 'unknown'
  matchedDeviceUid: string | null
  matchedAlias: string | null
  matchedModel: string | null
  matchedColorTag: string | null
  source: 'adb' | 'mdns-sd' | 'both'
}

export interface AppItem {
  packageName: string
  apkPath: string
  isSystem: boolean
  versionName: string | null
}

export interface InstallFlags {
  replace: boolean
  downgrade: boolean
  grantPermissions: boolean
  allowTest: boolean
}

export interface InstallProgress {
  index: number
  total: number
  apkPath: string
  status: 'installing' | 'success' | 'error'
  message: string
}

export interface LogEntry {
  id: number
  timestamp: string
  pid: string
  tid: string
  level: 'V' | 'D' | 'I' | 'W' | 'E' | 'F' | string
  tag: string
  message: string
  raw: string
}

export interface ScrcpyOptions {
  maxSize: number | null
  videoBitRate: string | null
  maxFps: number | null
  videoCodec: 'h264' | 'h265' | 'av1' | string | null
  turnScreenOff: boolean
  stayAwake: boolean
  alwaysOnTop: boolean
  showTouches: boolean
  readOnly: boolean
  keyboardMode: 'sdk' | 'uhid' | string | null
  mouseMode: 'sdk' | 'uhid' | string | null
  noAudio: boolean
  audioCodec: 'opus' | 'aac' | 'flac' | string | null
  newDisplay: string | null
  startApp: string | null
  windowTitle: string | null
  extraArgs: string | null
}

export interface ScrcpyProfile {
  id: string
  name: string
  isBuiltin: boolean
  options: ScrcpyOptions
  updatedAt: number
}

export interface ScrcpyInstance {
  instanceId: string
  serial: string
  profileName: string | null
  startedAt: number
}

export interface ScrcpyExitedEvent {
  instanceId: string
  serial: string
  code: number | null
  stderrTail: string | null
}

export interface CommandSnippet {
  id: string
  title: string
  category: string
  command: string
  autoEnter: boolean
  sortOrder: number
}

export interface AdbServerStatusEvent {
  running: boolean
  version: string | null
  error: string | null
}

export interface TerminalAppearance {
  theme: 'auto' | 'light-clean' | 'one-dark' | 'tokyo-night' | 'catppuccin-mocha'
  fontFamily: string
  fontSize: number
  lineHeight: number
  cursorStyle: 'block' | 'underline' | 'bar'
  cursorBlink: boolean
  scrollback: number
  copyOnSelect: boolean
}
