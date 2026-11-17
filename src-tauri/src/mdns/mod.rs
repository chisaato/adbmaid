use std::collections::{HashMap, HashSet};
use std::net::{SocketAddr, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use adb_client::mdns::MDNSDiscoveryService;
use tauri::{AppHandle, Emitter};

use crate::adb::{adb_server_query, list_active_devices};
use crate::db::Database;
use crate::error::AppResult;
use crate::models::{DeviceRecord, MdnsDiscoveredItem};

#[derive(Default)]
pub struct MdnsWatcher {
    running: Arc<AtomicBool>,
    native_cache: Arc<Mutex<HashMap<String, RawMdnsEntry>>>,
}

#[derive(Clone, Debug)]
struct RawMdnsEntry {
    service_name: String,
    reg_type: String,
    ip: String,
    port: u16,
    source: String,
}

impl MdnsWatcher {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn scan_once(&self, db: &Database) -> AppResult<Vec<MdnsDiscoveredItem>> {
        let mut merged: HashMap<String, RawMdnsEntry> = HashMap::new();

        // 1. ADB Server host:mdns:services 通道（按 \t 分割，兼容带空格的 (2) 服务名）
        if let Ok(raw) = adb_server_query("host:mdns:services") {
            for line in raw.lines() {
                let parts: Vec<&str> = line.split('\t').collect();
                if parts.len() >= 3 {
                    let service_name = parts[0].trim().to_string();
                    let reg_type = parts[1].trim().to_string();
                    let addr_str = parts[2].trim();
                    if let Some((ip, port_str)) = addr_str.rsplit_once(':') {
                        if let Ok(port) = port_str.parse::<u16>() {
                            let key = format!("{ip}:{port}");
                            merged.insert(
                                key,
                                RawMdnsEntry {
                                    service_name,
                                    reg_type,
                                    ip: ip.to_string(),
                                    port,
                                    source: "adb".into(),
                                },
                            );
                        }
                    }
                }
            }
        }

        // 2. Rust 原生 mdns-sd 通道缓存
        if let Ok(cache) = self.native_cache.lock() {
            for (key, entry) in cache.iter() {
                merged
                    .entry(key.clone())
                    .and_modify(|e| {
                        e.source = "both".into();
                    })
                    .or_insert_with(|| entry.clone());
            }
        }

        // 3. 当前已连接设备集合（使用兼容空格的 list_active_devices）
        let active_serials: HashSet<String> = list_active_devices(db)
            .unwrap_or_default()
            .into_iter()
            .filter(|d| d.state == "device")
            .map(|d| d.serial)
            .collect();

        // 4. 过滤已失效的旧 mDNS 缓存端口，并执行指纹身份匹配
        let mut items: Vec<MdnsDiscoveredItem> = merged
            .into_values()
            .filter(|entry| {
                let addr_key = format!("{}:{}", entry.ip, entry.port);
                if is_already_connected(&entry.service_name, &addr_key, &active_serials) {
                    return true;
                }
                // 对未连接的节点做 250ms TCP 探测，剔除手机换端口后残留的旧 mDNS 缓存
                if let Ok(sa) = addr_key.parse::<SocketAddr>() {
                    TcpStream::connect_timeout(&sa, Duration::from_millis(250)).is_ok()
                } else {
                    false
                }
            })
            .map(|entry| match_entry_with_db(db, entry, &active_serials))
            .collect();

        items.sort_by(|a, b| a.ip.cmp(&b.ip).then(a.port.cmp(&b.port)));
        Ok(items)
    }

    pub fn set_watch(&self, enabled: bool, app: AppHandle, db: Arc<Database>) {
        if !enabled {
            self.running.store(false, Ordering::SeqCst);
            return;
        }
        if self.running.swap(true, Ordering::SeqCst) {
            return;
        }

        let running = self.running.clone();
        let cache = self.native_cache.clone();

        let running_native = running.clone();
        let cache_native = cache.clone();
        std::thread::spawn(move || {
            let (tx, rx) = std::sync::mpsc::channel();
            if let Ok(mut discovery) = MDNSDiscoveryService::new() {
                if discovery.start(tx).is_ok() {
                    while running_native.load(Ordering::SeqCst) {
                        match rx.recv_timeout(Duration::from_millis(500)) {
                            Ok(dev) => {
                                let port = dev.port().get();
                                let (service_name, reg_type) = parse_fullname(&dev.fullname);
                                for ip in dev.ipv4_addresses() {
                                    let ip_str = ip.to_string();
                                    let key = format!("{ip_str}:{port}");
                                    if let Ok(mut guard) = cache_native.lock() {
                                        guard.insert(
                                            key,
                                            RawMdnsEntry {
                                                service_name: service_name.clone(),
                                                reg_type: reg_type.clone(),
                                                ip: ip_str,
                                                port,
                                                source: "mdns-sd".into(),
                                            },
                                        );
                                    }
                                }
                            }
                            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                            Err(_) => break,
                        }
                    }
                    let _ = discovery.shutdown();
                }
            }
        });

        let running_poll = self.running.clone();
        let cache_poll = self.native_cache.clone();
        std::thread::spawn(move || {
            while running_poll.load(Ordering::SeqCst) {
                let watcher_view = MdnsWatcher {
                    running: running_poll.clone(),
                    native_cache: cache_poll.clone(),
                };
                if let Ok(items) = watcher_view.scan_once(&db) {
                    let _ = app.emit("mdns://services-updated", items);
                }
                std::thread::sleep(Duration::from_secs(2));
            }
        });
    }
}

fn is_already_connected(
    service_name: &str,
    addr_key: &str,
    active_serials: &HashSet<String>,
) -> bool {
    if active_serials.contains(addr_key) {
        return true;
    }
    let tls_prefix = format!("{service_name}.");
    active_serials.iter().any(|s| s.starts_with(&tls_prefix))
}

fn parse_fullname(fullname: &str) -> (String, String) {
    if let Some((svc, rest)) = fullname.split_once('.') {
        let reg = rest.trim_end_matches(".local.").trim_end_matches('.');
        (svc.to_string(), reg.to_string())
    } else {
        (fullname.to_string(), "_adb-tls-connect._tcp".to_string())
    }
}

fn match_entry_with_db(
    db: &Database,
    entry: RawMdnsEntry,
    active_serials: &HashSet<String>,
) -> MdnsDiscoveredItem {
    let addr_key = format!("{}:{}", entry.ip, entry.port);
    let status = if is_already_connected(&entry.service_name, &addr_key, active_serials) {
        "connected".to_string()
    } else if entry.reg_type.contains("pairing") {
        "ready_pair".to_string()
    } else {
        "ready_connect".to_string()
    };

    let mut confidence = "unknown".to_string();
    let mut matched: Option<DeviceRecord> = None;

    if let Some(serial_candidate) = extract_serial_from_mdns(&entry.service_name) {
        if let Ok(Some(dev)) = db.find_device_by_hw_serial(&serial_candidate) {
            confidence = "exact".to_string();
            matched = Some(dev);
        }
    }

    if matched.is_none() {
        if let Ok(Some(dev)) = db.find_device_by_mdns_name(&entry.service_name) {
            confidence = "history".to_string();
            matched = Some(dev);
        }
    }

    if matched.is_none() {
        if let Ok(Some(dev)) = db.find_device_by_ip(&entry.ip) {
            confidence = "suspect".to_string();
            matched = Some(dev);
        }
    }

    MdnsDiscoveredItem {
        id: format!("{}:{}", entry.service_name, addr_key),
        service_name: entry.service_name,
        reg_type: entry.reg_type,
        ip: entry.ip,
        port: entry.port,
        status,
        confidence,
        matched_device_uid: matched.as_ref().map(|d| d.device_uid.clone()),
        matched_alias: matched.as_ref().and_then(|d| d.alias.clone()),
        matched_model: matched
            .as_ref()
            .and_then(|d| d.market_name.clone().or_else(|| d.model.clone())),
        matched_color_tag: matched.as_ref().and_then(|d| d.color_tag.clone()),
        source: entry.source,
    }
}

fn extract_serial_from_mdns(service_name: &str) -> Option<String> {
    // 去掉 mDNS 重名后缀如 " (2)"
    let clean = service_name
        .split_once(" (")
        .map(|(head, _)| head)
        .unwrap_or(service_name);
    let rest = clean.strip_prefix("adb-")?;
    if let Some((serial, _suffix)) = rest.rsplit_once('-') {
        if !serial.is_empty() {
            return Some(serial.to_string());
        }
    }
    None
}
