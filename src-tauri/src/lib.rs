mod adb;
mod db;
mod error;
mod logcat;
mod mdns;
mod models;
mod scrcpy;
mod shell;
mod state;
mod toolchain;

use std::path::PathBuf;
use std::sync::Arc;

use serde_json::Value as JsonValue;
use tauri::ipc::Channel;
use tauri::{AppHandle, Manager, State};

use crate::db::Database;
use crate::error::{AppError, AppResult};
use crate::logcat::LogcatManager;
use crate::mdns::MdnsWatcher;
use crate::models::{
    ActiveDeviceView, AppItem, CommandSnippet, DeviceEndpointRecord, DeviceMetaUpdate,
    DeviceRecord, DeviceTelemetry, DownloadProgress, InstallFlags, InstallProgress, LogEntry,
    MdnsDiscoveredItem, ScrcpyInstance, ScrcpyOptions, ScrcpyProfile, ToolchainProbeResult,
};
use crate::scrcpy::ScrcpyManager;
use crate::shell::ShellManager;
use crate::state::AppState;

// ================= Toolchain Commands =================

#[tauri::command]
async fn toolchain_probe(state: State<'_, AppState>) -> AppResult<ToolchainProbeResult> {
    let dir = state.app_data_dir.clone();
    let db = state.db.clone();
    tokio::task::spawn_blocking(move || toolchain::probe_toolchain(&dir, &db))
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
}

#[tauri::command]
async fn toolchain_download(
    state: State<'_, AppState>,
    target: String,
    mirror: Option<String>,
    on_progress: Channel<DownloadProgress>,
) -> AppResult<String> {
    if target != "scrcpy" {
        return Err(AppError::InvalidArg(
            "当前原型自动下载仅支持 scrcpy（内含 adb + scrcpy-server）".into(),
        ));
    }
    let proxy: Option<String> = state
        .db
        .get_setting("network.proxy")?
        .and_then(|v| v.as_str().map(|s| s.to_string()));
    toolchain::download_scrcpy(
        &state.app_data_dir,
        mirror.as_deref(),
        proxy.as_deref(),
        &on_progress,
    )
    .await
}

#[tauri::command]
async fn toolchain_import_archive(
    state: State<'_, AppState>,
    path: String,
    kind: String,
) -> AppResult<String> {
    let dir = state.app_data_dir.clone();
    tokio::task::spawn_blocking(move || {
        toolchain::import_local_archive(&dir, PathBuf::from(path).as_path(), &kind)
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))?
}

#[tauri::command]
async fn toolchain_save_paths(
    state: State<'_, AppState>,
    adb_path: Option<String>,
    scrcpy_path: Option<String>,
) -> AppResult<()> {
    if let Some(p) = adb_path {
        state
            .db
            .set_setting("toolchain.adb_path", &JsonValue::String(p))?;
    }
    if let Some(p) = scrcpy_path {
        state
            .db
            .set_setting("toolchain.scrcpy_path", &JsonValue::String(p))?;
    }
    Ok(())
}

// ================= Device & ADB Commands =================

#[tauri::command]
async fn adb_init_server(
    app: AppHandle,
    state: State<'_, AppState>,
) -> AppResult<Vec<ActiveDeviceView>> {
    let probe = toolchain::probe_toolchain(&state.app_data_dir, &state.db)?;
    let adb_path = probe.adb.path.clone();
    let db = state.db.clone();
    adb::start_device_tracker(app, db.clone(), adb_path);
    tokio::task::spawn_blocking(move || adb::list_active_devices(&db))
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
}

#[tauri::command]
async fn device_list_active(state: State<'_, AppState>) -> AppResult<Vec<ActiveDeviceView>> {
    let db = state.db.clone();
    tokio::task::spawn_blocking(move || adb::list_active_devices(&db))
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
}

#[tauri::command]
async fn device_list_known(state: State<'_, AppState>) -> AppResult<Vec<DeviceRecord>> {
    state.db.list_known_devices()
}

#[tauri::command]
async fn device_endpoints(
    state: State<'_, AppState>,
    device_uid: String,
) -> AppResult<Vec<DeviceEndpointRecord>> {
    state.db.list_device_endpoints(&device_uid)
}

#[tauri::command]
async fn device_update_meta(state: State<'_, AppState>, update: DeviceMetaUpdate) -> AppResult<()> {
    state.db.update_device_meta(&update)
}

#[tauri::command]
async fn device_connect_tcp(
    state: State<'_, AppState>,
    host: String,
    port: u16,
) -> AppResult<DeviceRecord> {
    let db = state.db.clone();
    tokio::task::spawn_blocking(move || adb::connect_tcp(&db, &host, port))
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
}

#[tauri::command]
async fn device_disconnect_tcp(host: String, port: u16) -> AppResult<()> {
    tokio::task::spawn_blocking(move || adb::disconnect_tcp(&host, port))
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
}

#[tauri::command]
async fn device_pair_wireless(host: String, port: u16, code: String) -> AppResult<()> {
    tokio::task::spawn_blocking(move || adb::pair_wireless(&host, port, &code))
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
}

#[tauri::command]
async fn device_tcpip(state: State<'_, AppState>, serial: String, port: u16) -> AppResult<String> {
    let probe = toolchain::probe_toolchain(&state.app_data_dir, &state.db)?;
    let adb_path = probe
        .adb
        .path
        .ok_or_else(|| AppError::Toolchain("未找到 adb 可执行文件".into()))?;
    tokio::task::spawn_blocking(move || adb::enable_tcpip(&adb_path, &serial, port))
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
}

#[tauri::command]
fn adb_set_auto_scan(enabled: bool) {
    adb::set_auto_scan_enabled(enabled);
}

#[tauri::command]
async fn device_reboot(serial: String, mode: String) -> AppResult<()> {
    tokio::task::spawn_blocking(move || adb::reboot_device(&serial, &mode))
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
}

#[tauri::command]
async fn device_telemetry(serial: String) -> AppResult<DeviceTelemetry> {
    tokio::task::spawn_blocking(move || adb::get_device_telemetry(&serial))
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
}

// ================= mDNS Commands =================

#[tauri::command]
async fn mdns_scan_once(state: State<'_, AppState>) -> AppResult<Vec<MdnsDiscoveredItem>> {
    let db = state.db.clone();
    // 借用 native_cache 快照在阻塞线程扫描，避免阻塞 Tokio
    state.mdns.scan_once(&db)
}

#[tauri::command]
async fn mdns_watch(app: AppHandle, state: State<'_, AppState>, enabled: bool) -> AppResult<()> {
    state.mdns.set_watch(enabled, app, state.db.clone());
    Ok(())
}

// ================= App Manager Commands =================

#[tauri::command]
async fn app_list(serial: String, scope: String) -> AppResult<Vec<AppItem>> {
    tokio::task::spawn_blocking(move || adb::list_apps(&serial, &scope))
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
}

#[tauri::command]
async fn app_install(
    state: State<'_, AppState>,
    serial: String,
    apk_paths: Vec<String>,
    flags: InstallFlags,
    on_progress: Channel<InstallProgress>,
) -> AppResult<()> {
    let probe = toolchain::probe_toolchain(&state.app_data_dir, &state.db)?;
    let adb_path = probe
        .adb
        .path
        .ok_or_else(|| AppError::Toolchain("未找到 adb 可执行文件".into()))?;
    tokio::task::spawn_blocking(move || {
        adb::install_apks(&adb_path, &serial, &apk_paths, &flags, &on_progress)
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))?
}

#[tauri::command]
async fn app_action(
    serial: String,
    pkg: String,
    action: String,
    dest_path: Option<String>,
) -> AppResult<String> {
    tokio::task::spawn_blocking(move || {
        adb::app_action(&serial, &pkg, &action, dest_path.as_deref())
    })
    .await
    .map_err(|e| AppError::Internal(e.to_string()))?
}

// ================= Shell / Terminal Commands =================

#[tauri::command]
async fn shell_open(
    state: State<'_, AppState>,
    serial: String,
    cols: u16,
    rows: u16,
    on_output: Channel<Vec<u8>>,
) -> AppResult<String> {
    let probe = toolchain::probe_toolchain(&state.app_data_dir, &state.db)?;
    let adb_path = probe
        .adb
        .path
        .ok_or_else(|| AppError::Toolchain("未找到 adb 可执行文件".into()))?;
    state.shell.open(&adb_path, &serial, cols, rows, on_output)
}

#[tauri::command]
async fn shell_write(
    state: State<'_, AppState>,
    session_id: String,
    data: Vec<u8>,
) -> AppResult<()> {
    state.shell.write(&session_id, &data)
}

#[tauri::command]
async fn shell_resize(
    state: State<'_, AppState>,
    session_id: String,
    cols: u16,
    rows: u16,
) -> AppResult<()> {
    state.shell.resize(&session_id, cols, rows)
}

#[tauri::command]
async fn shell_close(state: State<'_, AppState>, session_id: String) -> AppResult<()> {
    state.shell.close(&session_id)
}

#[tauri::command]
async fn snippet_list(state: State<'_, AppState>) -> AppResult<Vec<CommandSnippet>> {
    state.db.list_command_snippets()
}

#[tauri::command]
async fn snippet_save(state: State<'_, AppState>, snippet: CommandSnippet) -> AppResult<()> {
    state.db.save_command_snippet(&snippet)
}

#[tauri::command]
async fn snippet_delete(state: State<'_, AppState>, id: String) -> AppResult<()> {
    state.db.delete_command_snippet(&id)
}

// ================= Logcat Commands =================

#[tauri::command]
async fn logcat_start(
    state: State<'_, AppState>,
    serial: String,
    buffers: Vec<String>,
    clear_first: Option<bool>,
    on_batch: Channel<Vec<LogEntry>>,
) -> AppResult<String> {
    let probe = toolchain::probe_toolchain(&state.app_data_dir, &state.db)?;
    let adb_path = probe
        .adb
        .path
        .ok_or_else(|| AppError::Toolchain("未找到 adb 可执行文件".into()))?;
    state.logcat.start(
        &adb_path,
        &serial,
        &buffers,
        clear_first.unwrap_or(false),
        on_batch,
    )
}

#[tauri::command]
async fn logcat_resolve_pkg_pid(serial: String, pkg: String) -> AppResult<Vec<String>> {
    tokio::task::spawn_blocking(move || logcat::resolve_pkg_pids(&serial, &pkg))
        .await
        .map_err(|e| AppError::Internal(e.to_string()))?
}

#[tauri::command]
async fn logcat_stop(state: State<'_, AppState>, stream_id: String) -> AppResult<()> {
    state.logcat.stop(&stream_id)
}

// ================= Scrcpy Commands =================

#[tauri::command]
async fn scrcpy_start(
    app: AppHandle,
    state: State<'_, AppState>,
    serial: String,
    profile_name: Option<String>,
    options: ScrcpyOptions,
) -> AppResult<ScrcpyInstance> {
    let probe = toolchain::probe_toolchain(&state.app_data_dir, &state.db)?;
    let adb_path = probe
        .adb
        .path
        .ok_or_else(|| AppError::Toolchain("未找到 adb 可执行文件".into()))?;
    let scrcpy_path = probe
        .scrcpy
        .path
        .ok_or_else(|| AppError::Toolchain("未找到 scrcpy 可执行文件".into()))?;
    state.scrcpy.start(
        app,
        &scrcpy_path,
        probe.scrcpy.server_path.as_deref(),
        &adb_path,
        &serial,
        profile_name,
        &options,
    )
}

#[tauri::command]
async fn scrcpy_stop(state: State<'_, AppState>, instance_id: String) -> AppResult<()> {
    state.scrcpy.stop(&instance_id)
}

#[tauri::command]
async fn scrcpy_list_running(state: State<'_, AppState>) -> AppResult<Vec<ScrcpyInstance>> {
    Ok(state.scrcpy.list_running())
}

#[tauri::command]
async fn scrcpy_list_profiles(state: State<'_, AppState>) -> AppResult<Vec<ScrcpyProfile>> {
    state.db.list_scrcpy_profiles()
}

#[tauri::command]
async fn scrcpy_save_profile(state: State<'_, AppState>, profile: ScrcpyProfile) -> AppResult<()> {
    state.db.save_scrcpy_profile(&profile)
}

#[tauri::command]
async fn scrcpy_delete_profile(state: State<'_, AppState>, id: String) -> AppResult<()> {
    state.db.delete_scrcpy_profile(&id)
}

// ================= Settings Commands =================

#[tauri::command]
async fn settings_get(state: State<'_, AppState>, key: String) -> AppResult<Option<JsonValue>> {
    state.db.get_setting(&key)
}

#[tauri::command]
async fn settings_set(state: State<'_, AppState>, key: String, value: JsonValue) -> AppResult<()> {
    state.db.set_setting(&key, &value)
}

#[tauri::command]
async fn app_data_info(state: State<'_, AppState>) -> AppResult<String> {
    Ok(state.app_data_dir.to_string_lossy().to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let app_data_dir = app
                .path()
                .app_data_dir()
                .unwrap_or_else(|_| PathBuf::from(".adbmaid-data"));
            let db_path = app_data_dir.join("adbmaid.db");
            let db = Arc::new(Database::open(&db_path)?);

            app.manage(AppState {
                app_data_dir,
                db,
                mdns: MdnsWatcher::new(),
                shell: ShellManager::new(),
                logcat: LogcatManager::new(),
                scrcpy: ScrcpyManager::new(),
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            toolchain_probe,
            toolchain_download,
            toolchain_import_archive,
            toolchain_save_paths,
            adb_init_server,
            adb_set_auto_scan,
            device_list_active,
            device_list_known,
            device_endpoints,
            device_update_meta,
            device_connect_tcp,
            device_disconnect_tcp,
            device_pair_wireless,
            device_tcpip,
            device_reboot,
            device_telemetry,
            mdns_scan_once,
            mdns_watch,
            app_list,
            app_install,
            app_action,
            shell_open,
            shell_write,
            shell_resize,
            shell_close,
            snippet_list,
            snippet_save,
            snippet_delete,
            logcat_start,
            logcat_resolve_pkg_pid,
            logcat_stop,
            scrcpy_start,
            scrcpy_stop,
            scrcpy_list_running,
            scrcpy_list_profiles,
            scrcpy_save_profile,
            scrcpy_delete_profile,
            settings_get,
            settings_set,
            app_data_info,
        ])
        .run(tauri::generate_context!())
        .expect("error while running adbmaid application");
}
