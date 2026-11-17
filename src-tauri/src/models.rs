use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolBinaryInfo {
    pub found: bool,
    pub path: Option<String>,
    pub version: Option<String>,
    pub source: String, // "custom" | "managed" | "system" | "missing"
    pub server_path: Option<String>,
    pub is_outdated: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LinuxDistroInfo {
    pub id: String,
    pub pretty_name: String,
    pub install_hint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolchainProbeResult {
    pub os: String,
    pub arch: String,
    pub can_auto_download_scrcpy: bool,
    pub can_auto_download_adb: bool,
    pub linux_distro: Option<LinuxDistroInfo>,
    pub adb: ToolBinaryInfo,
    pub scrcpy: ToolBinaryInfo,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DownloadProgress {
    pub stage: String, // "fetching_meta" | "downloading" | "verifying" | "extracting" | "done" | "error"
    pub received_bytes: u64,
    pub total_bytes: Option<u64>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceRecord {
    pub device_uid: String,
    pub hardware_serial: Option<String>,
    pub boot_serial: Option<String>,
    pub android_id: Option<String>,
    pub brand: Option<String>,
    pub manufacturer: Option<String>,
    pub model: Option<String>,
    pub market_name: Option<String>,
    pub device_code: Option<String>,
    pub abi: Option<String>,
    pub android_version: Option<String>,
    pub sdk_int: Option<i32>,
    pub build_fingerprint: Option<String>,
    pub alias: Option<String>,
    pub color_tag: Option<String>,
    pub notes: Option<String>,
    pub auto_connect: bool,
    pub default_profile_id: Option<String>,
    pub first_seen_at: i64,
    pub last_seen_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceEndpointRecord {
    pub id: i64,
    pub device_uid: String,
    pub transport_type: String,
    pub adb_serial: Option<String>,
    pub mdns_service_name: Option<String>,
    pub mdns_reg_type: Option<String>,
    pub last_ip: Option<String>,
    pub last_port: Option<u16>,
    pub last_seen_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActiveDeviceView {
    pub serial: String,
    pub state: String,          // "device" | "offline" | "unauthorized" | "unknown"
    pub transport_type: String, // "usb" | "tcp"
    pub device_uid: Option<String>,
    pub alias: Option<String>,
    pub color_tag: Option<String>,
    pub model: Option<String>,
    pub market_name: Option<String>,
    pub brand: Option<String>,
    pub android_version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceMetaUpdate {
    pub device_uid: String,
    pub alias: Option<String>,
    pub color_tag: Option<String>,
    pub notes: Option<String>,
    pub auto_connect: Option<bool>,
    pub default_profile_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceTelemetry {
    pub battery_level: Option<i32>,
    pub battery_charging: Option<bool>,
    pub screen_size: Option<String>,
    pub screen_density: Option<String>,
    pub ip_address: Option<String>,
    pub storage_summary: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MdnsDiscoveredItem {
    pub id: String,
    pub service_name: String,
    pub reg_type: String, // "_adb-tls-connect._tcp" | "_adb-tls-pairing._tcp" | "_adb._tcp"
    pub ip: String,
    pub port: u16,
    pub status: String,     // "connected" | "ready_connect" | "ready_pair"
    pub confidence: String, // "exact" | "history" | "suspect" | "unknown"
    pub matched_device_uid: Option<String>,
    pub matched_alias: Option<String>,
    pub matched_model: Option<String>,
    pub matched_color_tag: Option<String>,
    pub source: String, // "adb" | "mdns-sd" | "both"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppItem {
    pub package_name: String,
    pub apk_path: String,
    pub is_system: bool,
    pub version_name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallFlags {
    pub replace: bool,
    pub downgrade: bool,
    pub grant_permissions: bool,
    pub allow_test: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallProgress {
    pub index: usize,
    pub total: usize,
    pub apk_path: String,
    pub status: String, // "installing" | "success" | "error"
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LogEntry {
    pub id: u64,
    pub timestamp: String,
    pub pid: String,
    pub tid: String,
    pub level: String, // V | D | I | W | E | F
    pub tag: String,
    pub message: String,
    pub raw: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScrcpyOptions {
    pub max_size: Option<u32>,
    pub video_bit_rate: Option<String>,
    pub max_fps: Option<u32>,
    pub video_codec: Option<String>, // h264 | h265 | av1
    pub turn_screen_off: bool,
    pub stay_awake: bool,
    pub always_on_top: bool,
    pub show_touches: bool,
    pub read_only: bool,
    pub keyboard_mode: Option<String>, // sdk | uhid
    pub mouse_mode: Option<String>,    // sdk | uhid
    pub no_audio: bool,
    pub audio_codec: Option<String>, // opus | aac | flac
    pub new_display: Option<String>, // e.g. "1920x1080/320"
    pub start_app: Option<String>,
    pub window_title: Option<String>,
    pub extra_args: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScrcpyProfile {
    pub id: String,
    pub name: String,
    pub is_builtin: bool,
    pub options: ScrcpyOptions,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScrcpyInstance {
    pub instance_id: String,
    pub serial: String,
    pub profile_name: Option<String>,
    pub started_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScrcpyExitedEvent {
    pub instance_id: String,
    pub serial: String,
    pub code: Option<i32>,
    pub stderr_tail: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CommandSnippet {
    pub id: String,
    pub title: String,
    pub category: String,
    pub command: String,
    pub auto_enter: bool,
    pub sort_order: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AdbServerStatusEvent {
    pub running: bool,
    pub version: Option<String>,
    pub error: Option<String>,
}
