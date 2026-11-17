use std::collections::HashSet;
use std::fs::File;
use std::io::{Read, Write};
use std::net::{Ipv4Addr, SocketAddrV4, TcpStream};
use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use adb_client::server::ADBServer;
use adb_client::server_device::ADBServerDevice;
use adb_client::{ADBDeviceExt, RebootType};
use regex::Regex;
use tauri::ipc::Channel;
use tauri::{AppHandle, Emitter};

use crate::db::Database;
use crate::error::{AppError, AppResult};
use crate::models::{
    ActiveDeviceView, AdbServerStatusEvent, AppItem, DeviceRecord, DeviceTelemetry, InstallFlags,
    InstallProgress,
};

pub fn default_server_addr() -> SocketAddrV4 {
    SocketAddrV4::new(Ipv4Addr::new(127, 0, 0, 1), 5037)
}

/// 直接向 127.0.0.1:5037 ADB Server 发送协议查询（兼容带空格的 mDNS 设备名）
pub fn adb_server_query(cmd: &str) -> AppResult<String> {
    let addr = default_server_addr();
    let mut stream = TcpStream::connect_timeout(&addr.into(), Duration::from_secs(2))
        .map_err(|e| AppError::Adb(format!("无法连接 ADB Server (5037): {e}")))?;
    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
    let _ = stream.set_write_timeout(Some(Duration::from_secs(5)));

    let packet = format!("{:04x}{cmd}", cmd.len());
    stream
        .write_all(packet.as_bytes())
        .map_err(|e| AppError::Adb(e.to_string()))?;

    let mut status = [0u8; 4];
    stream
        .read_exact(&mut status)
        .map_err(|e| AppError::Adb(e.to_string()))?;

    let mut len_buf = [0u8; 4];
    stream
        .read_exact(&mut len_buf)
        .map_err(|e| AppError::Adb(e.to_string()))?;
    let len_str = std::str::from_utf8(&len_buf).unwrap_or("0000");
    let len = usize::from_str_radix(len_str, 16).unwrap_or(0);

    let mut payload = vec![0u8; len];
    if len > 0 {
        stream
            .read_exact(&mut payload)
            .map_err(|e| AppError::Adb(e.to_string()))?;
    }
    let text = String::from_utf8_lossy(&payload).to_string();
    if &status != b"OKAY" {
        return Err(AppError::Adb(text));
    }
    Ok(text)
}

pub fn ensure_server_running(adb_path: Option<&str>) -> AppResult<String> {
    let addr = default_server_addr();
    let is_up = TcpStream::connect_timeout(&addr.into(), Duration::from_millis(300)).is_ok();
    if !is_up {
        let bin = adb_path.ok_or_else(|| {
            AppError::Toolchain("127.0.0.1:5037 未运行且未探测到 adb 可执行文件".into())
        })?;
        let out = Command::new(bin)
            .arg("start-server")
            .output()
            .map_err(|e| AppError::Adb(format!("启动 adb server 失败: {e}")))?;
        if !out.status.success() {
            let err = String::from_utf8_lossy(&out.stderr);
            return Err(AppError::Adb(format!("adb start-server 异常: {err}")));
        }
    }

    let mut server = ADBServer::new(addr);
    let ver = server
        .version()
        .map(|v| format!("{}.{}.{}", v.major, v.minor, v.revision))
        .unwrap_or_else(|_| "unknown".into());
    Ok(ver)
}

pub fn list_active_devices(db: &Database) -> AppResult<Vec<ActiveDeviceView>> {
    let raw = adb_server_query("host:devices-l")?;
    let line_re = Regex::new(
        r"^(?P<serial>.+?)\s+(?P<state>device|offline|unauthorized|authorizing|recovery|sideload|bootloader)(?:\s+(?P<rest>.*))?$",
    )
    .expect("valid devices-l regex");

    let mut out = Vec::new();
    for line in raw.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let Some(caps) = line_re.captures(line) else {
            continue;
        };
        let serial = caps["serial"].trim().to_string();
        let state = caps["state"].trim().to_string();
        let rest = caps.name("rest").map(|m| m.as_str()).unwrap_or("");

        let mut model_from_l: Option<String> = None;
        for token in rest.split_whitespace() {
            if let Some(m) = token.strip_prefix("model:") {
                model_from_l = Some(m.replace('_', " "));
            }
        }

        let transport_type = if serial.contains(':')
            || serial.contains("_adb-tls-connect._tcp")
            || serial.contains("_adb._tcp")
        {
            "tcp"
        } else {
            "usb"
        };

        let known = db.find_device_by_adb_serial(&serial).ok().flatten();
        out.push(ActiveDeviceView {
            serial,
            state,
            transport_type: transport_type.to_string(),
            device_uid: known.as_ref().map(|k| k.device_uid.clone()),
            alias: known.as_ref().and_then(|k| k.alias.clone()),
            color_tag: known.as_ref().and_then(|k| k.color_tag.clone()),
            model: known
                .as_ref()
                .and_then(|k| k.model.clone())
                .or(model_from_l),
            market_name: known.as_ref().and_then(|k| k.market_name.clone()),
            brand: known.as_ref().and_then(|k| k.brand.clone()),
            android_version: known.as_ref().and_then(|k| k.android_version.clone()),
        });
    }
    Ok(out)
}

static AUTO_SCAN_ENABLED: AtomicBool = AtomicBool::new(true);

pub fn set_auto_scan_enabled(enabled: bool) {
    AUTO_SCAN_ENABLED.store(enabled, Ordering::Relaxed);
}

pub fn is_auto_scan_enabled() -> bool {
    AUTO_SCAN_ENABLED.load(Ordering::Relaxed)
}

pub fn start_device_tracker(app: AppHandle, db: Arc<Database>, adb_path: Option<String>) {
    let seen_online = Arc::new(Mutex::new(HashSet::<String>::new()));

    // 自动后台定时扫描线程（每 2 秒扫描一次 devices-l，保证左上角设备栏始终自动发现并列出设备）
    std::thread::spawn(move || {
        let mut last_snapshot = String::new();
        loop {
            if !is_auto_scan_enabled() {
                std::thread::sleep(Duration::from_millis(400));
                continue;
            }
            match ensure_server_running(adb_path.as_deref()) {
                Ok(ver) => {
                    let _ = app.emit(
                        "adb://server-status",
                        AdbServerStatusEvent {
                            running: true,
                            version: Some(ver),
                            error: None,
                        },
                    );
                }
                Err(e) => {
                    let _ = app.emit(
                        "adb://server-status",
                        AdbServerStatusEvent {
                            running: false,
                            version: None,
                            error: Some(e.to_string()),
                        },
                    );
                    std::thread::sleep(Duration::from_secs(3));
                    continue;
                }
            }

            if let Ok(list) = list_active_devices(&db) {
                trigger_fingerprints_for_online(&app, &db, &list, &seen_online);

                // 清理已下线设备的 seen 标记
                let current_online: HashSet<String> = list
                    .iter()
                    .filter(|d| d.state == "device")
                    .map(|d| d.serial.clone())
                    .collect();
                if let Ok(mut guard) = seen_online.lock() {
                    guard.retain(|s| current_online.contains(s));
                }

                let snapshot = serde_json::to_string(&list).unwrap_or_default();
                if snapshot != last_snapshot {
                    last_snapshot = snapshot;
                    let _ = app.emit("adb://devices-changed", list);
                }
            }

            std::thread::sleep(Duration::from_secs(2));
        }
    });
}

fn trigger_fingerprints_for_online(
    app: &AppHandle,
    db: &Arc<Database>,
    list: &[ActiveDeviceView],
    seen_online: &Arc<Mutex<HashSet<String>>>,
) {
    for item in list {
        if item.state != "device" {
            continue;
        }
        let should_collect = {
            let mut guard = seen_online.lock().unwrap();
            if guard.contains(&item.serial) {
                false
            } else {
                guard.insert(item.serial.clone());
                true
            }
        };
        if should_collect {
            let serial = item.serial.clone();
            let db_clone = db.clone();
            let app_clone = app.clone();
            std::thread::spawn(move || {
                if collect_and_save_fingerprint(&db_clone, &serial).is_ok() {
                    if let Ok(updated) = list_active_devices(&db_clone) {
                        let _ = app_clone.emit("adb://devices-changed", updated);
                    }
                }
            });
        }
    }
}

pub fn collect_and_save_fingerprint(db: &Database, serial: &str) -> AppResult<DeviceRecord> {
    let mut dev = ADBServerDevice::new(serial.to_string(), Some(default_server_addr()));
    let cmd = concat!(
        "echo __SERIAL__=$(getprop ro.serialno); ",
        "echo __BOOT_SERIAL__=$(getprop ro.boot.serialno); ",
        "echo __ANDROID_ID__=$(settings get secure android_id 2>/dev/null); ",
        "echo __BRAND__=$(getprop ro.product.brand); ",
        "echo __MFR__=$(getprop ro.product.manufacturer); ",
        "echo __MODEL__=$(getprop ro.product.model); ",
        "echo __MARKET__=$(getprop ro.product.marketname); ",
        "echo __ODEVICE__=$(getprop ro.product.odm.marketname); ",
        "echo __DEVICE__=$(getprop ro.product.device); ",
        "echo __ABI__=$(getprop ro.product.cpu.abi); ",
        "echo __REL__=$(getprop ro.build.version.release); ",
        "echo __SDK__=$(getprop ro.build.version.sdk); ",
        "echo __FP__=$(getprop ro.build.fingerprint)"
    );
    let mut stdout = Vec::new();
    dev.shell_command(&cmd, Some(&mut stdout), None)
        .map_err(|e| AppError::Adb(e.to_string()))?;

    let text = String::from_utf8_lossy(&stdout);
    let mut map = std::collections::HashMap::new();
    for line in text.lines() {
        if let Some((k, v)) = line.split_once('=') {
            let val = v.trim();
            if !val.is_empty() && val != "null" {
                map.insert(k.trim().to_string(), val.to_string());
            }
        }
    }

    let sanitize_serial = |s: Option<&String>| -> Option<String> {
        s.and_then(|v| {
            let low = v.to_lowercase();
            if low.is_empty() || low == "unknown" || low == "0123456789abcdef" || low == "none" {
                None
            } else {
                Some(v.clone())
            }
        })
    };

    let hw_serial = sanitize_serial(map.get("__SERIAL__"))
        .or_else(|| sanitize_serial(map.get("__BOOT_SERIAL__")));
    let market_name = map
        .get("__MARKET__")
        .cloned()
        .or_else(|| map.get("__ODEVICE__").cloned());

    let rec = DeviceRecord {
        device_uid: String::new(),
        hardware_serial: hw_serial,
        boot_serial: sanitize_serial(map.get("__BOOT_SERIAL__")),
        android_id: map.get("__ANDROID_ID__").cloned(),
        brand: map.get("__BRAND__").cloned(),
        manufacturer: map.get("__MFR__").cloned(),
        model: map.get("__MODEL__").cloned(),
        market_name,
        device_code: map.get("__DEVICE__").cloned(),
        abi: map.get("__ABI__").cloned(),
        android_version: map.get("__REL__").cloned(),
        sdk_int: map.get("__SDK__").and_then(|s| s.parse::<i32>().ok()),
        build_fingerprint: map.get("__FP__").cloned(),
        alias: None,
        color_tag: None,
        notes: None,
        auto_connect: false,
        default_profile_id: None,
        first_seen_at: 0,
        last_seen_at: 0,
    };

    let saved = db.upsert_device_fingerprint(rec)?;

    // 记录当前端点
    let (transport, ip, port) = if let Some((h, p)) = serial.rsplit_once(':') {
        ("tcp", Some(h.to_string()), p.parse::<u16>().ok())
    } else if serial.contains("_adb-tls-connect._tcp") {
        ("mdns_connect", None, None)
    } else {
        ("usb", None, None)
    };
    db.upsert_endpoint(
        &saved.device_uid,
        transport,
        Some(serial),
        None,
        None,
        ip.as_deref(),
        port,
    )?;

    Ok(saved)
}

pub fn connect_tcp(db: &Database, host: &str, port: u16) -> AppResult<DeviceRecord> {
    let ip: Ipv4Addr = host
        .parse()
        .map_err(|_| AppError::InvalidArg(format!("无效 IPv4 地址: {host}")))?;
    let addr = SocketAddrV4::new(ip, port);
    let mut server = ADBServer::new(default_server_addr());
    server
        .connect_device(addr)
        .map_err(|e| AppError::Adb(format!("连接 {addr} 失败: {e}")))?;

    let serial = format!("{host}:{port}");
    collect_and_save_fingerprint(db, &serial)
}

pub fn disconnect_tcp(host: &str, port: u16) -> AppResult<()> {
    let ip: Ipv4Addr = host
        .parse()
        .map_err(|_| AppError::InvalidArg(format!("无效 IPv4 地址: {host}")))?;
    let addr = SocketAddrV4::new(ip, port);
    let mut server = ADBServer::new(default_server_addr());
    server
        .disconnect_device(addr)
        .map_err(|e| AppError::Adb(format!("断开 {addr} 失败: {e}")))
}

pub fn pair_wireless(host: &str, port: u16, code: &str) -> AppResult<()> {
    let ip: Ipv4Addr = host
        .parse()
        .map_err(|_| AppError::InvalidArg(format!("无效 IPv4 地址: {host}")))?;
    let addr = SocketAddrV4::new(ip, port);
    let mut server = ADBServer::new(default_server_addr());
    server
        .pair(addr, code.trim().to_string())
        .map_err(|e| AppError::Adb(format!("配对 {addr} 失败: {e}")))
}

pub fn enable_tcpip(adb_path: &str, serial: &str, port: u16) -> AppResult<String> {
    let out = Command::new(adb_path)
        .args(["-s", serial, "tcpip", &port.to_string()])
        .output()?;
    let text =
        String::from_utf8_lossy(&out.stdout).to_string() + &String::from_utf8_lossy(&out.stderr);
    if !out.status.success() {
        return Err(AppError::Adb(text));
    }
    Ok(text.trim().to_string())
}

pub fn reboot_device(serial: &str, mode: &str) -> AppResult<()> {
    let mut dev = ADBServerDevice::new(serial.to_string(), Some(default_server_addr()));
    let rtype = match mode {
        "bootloader" => RebootType::Bootloader,
        "recovery" => RebootType::Recovery,
        "sideload" => RebootType::Sideload,
        _ => RebootType::System,
    };
    dev.reboot(rtype).map_err(|e| AppError::Adb(e.to_string()))
}

pub fn get_device_telemetry(serial: &str) -> AppResult<DeviceTelemetry> {
    let mut dev = ADBServerDevice::new(serial.to_string(), Some(default_server_addr()));
    let cmd = concat!(
        "echo __BAT__=$(dumpsys battery | grep -E 'level:|status:' | tr '\\n' ' '); ",
        "echo __SIZE__=$(wm size | head -n 1); ",
        "echo __DENS__=$(wm density | head -n 1); ",
        "echo __IP__=$(ip -f inet addr show wlan0 2>/dev/null | grep -oE 'inet [0-9.]+' | head -n 1); ",
        "echo __DF__=$(df -h /data | tail -n 1)"
    );
    let mut stdout = Vec::new();
    dev.shell_command(&cmd, Some(&mut stdout), None)
        .map_err(|e| AppError::Adb(e.to_string()))?;

    let text = String::from_utf8_lossy(&stdout);
    let mut battery_level = None;
    let mut battery_charging = None;
    let mut screen_size = None;
    let mut screen_density = None;
    let mut ip_address = None;
    let mut storage_summary = None;

    for line in text.lines() {
        if let Some(v) = line.strip_prefix("__BAT__=") {
            if let Some(idx) = v.find("level:") {
                let after = v[idx + 6..].trim();
                if let Some(num) = after.split_whitespace().next() {
                    battery_level = num.parse::<i32>().ok();
                }
            }
            if let Some(idx) = v.find("status:") {
                let after = v[idx + 7..].trim();
                if let Some(num) = after.split_whitespace().next() {
                    battery_charging = Some(num == "2" || num == "5");
                }
            }
        } else if let Some(v) = line.strip_prefix("__SIZE__=") {
            screen_size = Some(v.replace("Physical size:", "").trim().to_string());
        } else if let Some(v) = line.strip_prefix("__DENS__=") {
            screen_density = Some(v.replace("Physical density:", "").trim().to_string());
        } else if let Some(v) = line.strip_prefix("__IP__=") {
            let ip = v.replace("inet", "").trim().to_string();
            if !ip.is_empty() {
                ip_address = Some(ip);
            }
        } else if let Some(v) = line.strip_prefix("__DF__=") {
            let parts: Vec<&str> = v.split_whitespace().collect();
            if parts.len() >= 5 {
                storage_summary = Some(format!(
                    "已用 {} / 共 {} ({})",
                    parts[2], parts[1], parts[4]
                ));
            }
        }
    }

    Ok(DeviceTelemetry {
        battery_level,
        battery_charging,
        screen_size,
        screen_density,
        ip_address,
        storage_summary,
    })
}

pub fn list_apps(serial: &str, scope: &str) -> AppResult<Vec<AppItem>> {
    let mut dev = ADBServerDevice::new(serial.to_string(), Some(default_server_addr()));
    let flag = match scope {
        "user" => "-3",
        "system" => "-s",
        _ => "",
    };
    let cmd = if flag.is_empty() {
        "pm list packages -f".to_string()
    } else {
        format!("pm list packages -f {flag}")
    };
    let mut stdout = Vec::new();
    dev.shell_command(&cmd, Some(&mut stdout), None)
        .map_err(|e| AppError::Adb(e.to_string()))?;

    let text = String::from_utf8_lossy(&stdout);
    let mut list = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        let Some(rest) = line.strip_prefix("package:") else {
            continue;
        };
        if let Some((apk_path, pkg)) = rest.rsplit_once('=') {
            let is_system = apk_path.starts_with("/system")
                || apk_path.starts_with("/product")
                || apk_path.starts_with("/vendor")
                || apk_path.starts_with("/apex");
            list.push(AppItem {
                package_name: pkg.trim().to_string(),
                apk_path: apk_path.trim().to_string(),
                is_system,
                version_name: None,
            });
        }
    }
    list.sort_by(|a, b| a.package_name.cmp(&b.package_name));
    Ok(list)
}

pub fn install_apks(
    adb_path: &str,
    serial: &str,
    apk_paths: &[String],
    flags: &InstallFlags,
    on_progress: &Channel<InstallProgress>,
) -> AppResult<()> {
    let total = apk_paths.len();
    for (idx, apk) in apk_paths.iter().enumerate() {
        if !Path::new(apk).is_file() {
            let _ = on_progress.send(InstallProgress {
                index: idx + 1,
                total,
                apk_path: apk.clone(),
                status: "error".into(),
                message: "APK 文件不存在".into(),
            });
            continue;
        }
        let _ = on_progress.send(InstallProgress {
            index: idx + 1,
            total,
            apk_path: apk.clone(),
            status: "installing".into(),
            message: "正在安装...".into(),
        });

        let mut args = vec!["-s", serial, "install"];
        if flags.replace {
            args.push("-r");
        }
        if flags.downgrade {
            args.push("-d");
        }
        if flags.grant_permissions {
            args.push("-g");
        }
        if flags.allow_test {
            args.push("-t");
        }
        args.push(apk);

        let out = Command::new(adb_path).args(&args).output()?;
        let msg = String::from_utf8_lossy(&out.stdout).to_string()
            + &String::from_utf8_lossy(&out.stderr);
        if out.status.success() {
            let _ = on_progress.send(InstallProgress {
                index: idx + 1,
                total,
                apk_path: apk.clone(),
                status: "success".into(),
                message: msg.trim().to_string(),
            });
        } else {
            let _ = on_progress.send(InstallProgress {
                index: idx + 1,
                total,
                apk_path: apk.clone(),
                status: "error".into(),
                message: msg.trim().to_string(),
            });
        }
    }
    Ok(())
}

pub fn app_action(
    serial: &str,
    pkg: &str,
    action: &str,
    dest_path: Option<&str>,
) -> AppResult<String> {
    let mut dev = ADBServerDevice::new(serial.to_string(), Some(default_server_addr()));
    match action {
        "launch" => {
            let cmd = format!("monkey -p {pkg} -c android.intent.category.LAUNCHER 1");
            let mut out = Vec::new();
            dev.shell_command(&cmd, Some(&mut out), None)
                .map_err(|e| AppError::Adb(e.to_string()))?;
            Ok(String::from_utf8_lossy(&out).trim().to_string())
        }
        "stop" => {
            let cmd = format!("am force-stop {pkg}");
            dev.shell_command(&cmd, None, None)
                .map_err(|e| AppError::Adb(e.to_string()))?;
            Ok(format!("已强行停止 {pkg}"))
        }
        "clear" => {
            let cmd = format!("pm clear {pkg}");
            let mut out = Vec::new();
            dev.shell_command(&cmd, Some(&mut out), None)
                .map_err(|e| AppError::Adb(e.to_string()))?;
            Ok(String::from_utf8_lossy(&out).trim().to_string())
        }
        "uninstall" => {
            dev.uninstall(pkg, None)
                .map_err(|e| AppError::Adb(e.to_string()))?;
            Ok(format!("已卸载 {pkg}"))
        }
        "pull" => {
            let dest = dest_path
                .ok_or_else(|| AppError::InvalidArg("导出 APK 需要提供本地保存路径".into()))?;
            let mut path_out = Vec::new();
            dev.shell_command(&format!("pm path {pkg}"), Some(&mut path_out), None)
                .map_err(|e| AppError::Adb(e.to_string()))?;
            let remote_apk = String::from_utf8_lossy(&path_out)
                .lines()
                .next()
                .and_then(|l| l.strip_prefix("package:"))
                .map(|s| s.trim().to_string())
                .ok_or_else(|| AppError::NotFound(format!("未找到 {pkg} 的 APK 路径")))?;

            let mut file = File::create(dest)?;
            dev.pull(&remote_apk, &mut file)
                .map_err(|e| AppError::Adb(e.to_string()))?;
            Ok(format!("已导出至 {dest}"))
        }
        _ => Err(AppError::InvalidArg(format!("未知应用操作: {action}"))),
    }
}
