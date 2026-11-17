use std::collections::HashMap;
use std::io::{BufRead, BufReader};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use adb_client::server_device::ADBServerDevice;
use adb_client::ADBDeviceExt;
use regex::Regex;
use tauri::ipc::Channel;

use crate::adb::default_server_addr;
use crate::error::{AppError, AppResult};
use crate::models::LogEntry;

struct LogcatStream {
    stop_flag: Arc<AtomicBool>,
    child: Arc<Mutex<Child>>,
}

#[derive(Default)]
pub struct LogcatManager {
    streams: Arc<Mutex<HashMap<String, LogcatStream>>>,
    next_id: Arc<AtomicU64>,
}

impl LogcatManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn start(
        &self,
        adb_path: &str,
        serial: &str,
        buffers: &[String],
        clear_first: bool,
        on_batch: Channel<Vec<LogEntry>>,
    ) -> AppResult<String> {
        if clear_first {
            let _ = Command::new(adb_path)
                .args(["-s", serial, "logcat", "-c"])
                .output();
        }

        let mut cmd = Command::new(adb_path);
        cmd.args(["-s", serial, "logcat", "-v", "threadtime"]);
        for b in buffers {
            let trimmed = b.trim();
            if !trimmed.is_empty() {
                cmd.args(["-b", trimmed]);
            }
        }
        cmd.stdout(Stdio::piped()).stderr(Stdio::null());

        let mut child = cmd
            .spawn()
            .map_err(|e| AppError::Adb(format!("启动 logcat 失败: {e}")))?;

        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| AppError::Internal("无法捕获 logcat stdout".into()))?;

        let stream_id = uuid::Uuid::new_v4().to_string();
        let stop_flag = Arc::new(AtomicBool::new(false));
        let child_arc = Arc::new(Mutex::new(child));

        {
            let mut guard = self.streams.lock().unwrap();
            guard.insert(
                stream_id.clone(),
                LogcatStream {
                    stop_flag: stop_flag.clone(),
                    child: child_arc.clone(),
                },
            );
        }

        let id_counter = self.next_id.clone();
        let streams_ref = self.streams.clone();
        let sid_clone = stream_id.clone();

        std::thread::spawn(move || {
            let re = Regex::new(
                r"^(\d{2}-\d{2}\s+\d{2}:\d{2}:\d{2}\.\d{3})\s+(\d+)\s+(\d+)\s+([VDIWEF])\s+([^:]+?)\s*:\s?(.*)$",
            )
            .expect("valid logcat regex");

            let (tx, rx) = std::sync::mpsc::channel::<LogEntry>();
            let stop_reader = stop_flag.clone();

            std::thread::spawn(move || {
                let reader = BufReader::new(stdout);
                for line in reader.lines() {
                    if stop_reader.load(Ordering::SeqCst) {
                        break;
                    }
                    let Ok(raw) = line else { break };
                    let id = id_counter.fetch_add(1, Ordering::Relaxed) + 1;
                    let entry = parse_log_line(id, &raw, &re);
                    if tx.send(entry).is_err() {
                        break;
                    }
                }
            });

            let mut batch: Vec<LogEntry> = Vec::with_capacity(256);
            let mut last_flush = Instant::now();

            while !stop_flag.load(Ordering::SeqCst) {
                match rx.recv_timeout(Duration::from_millis(50)) {
                    Ok(entry) => {
                        batch.push(entry);
                        if batch.len() >= 200 || last_flush.elapsed() >= Duration::from_millis(50) {
                            if on_batch.send(std::mem::take(&mut batch)).is_err() {
                                break;
                            }
                            last_flush = Instant::now();
                        }
                    }
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                        if !batch.is_empty() {
                            if on_batch.send(std::mem::take(&mut batch)).is_err() {
                                break;
                            }
                            last_flush = Instant::now();
                        }
                    }
                    Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                        if !batch.is_empty() {
                            let _ = on_batch.send(std::mem::take(&mut batch));
                        }
                        break;
                    }
                }
            }

            if let Ok(mut c) = child_arc.lock() {
                let _ = c.kill();
                let _ = c.wait();
            }
            if let Ok(mut guard) = streams_ref.lock() {
                guard.remove(&sid_clone);
            }
        });

        Ok(stream_id)
    }

    pub fn stop(&self, stream_id: &str) -> AppResult<()> {
        let mut guard = self.streams.lock().unwrap();
        if let Some(stream) = guard.remove(stream_id) {
            stream.stop_flag.store(true, Ordering::SeqCst);
            if let Ok(mut c) = stream.child.lock() {
                let _ = c.kill();
            }
        }
        Ok(())
    }
}

fn parse_log_line(id: u64, raw: &str, re: &Regex) -> LogEntry {
    if let Some(caps) = re.captures(raw) {
        LogEntry {
            id,
            timestamp: caps.get(1).map(|m| m.as_str()).unwrap_or("").to_string(),
            pid: caps.get(2).map(|m| m.as_str()).unwrap_or("").to_string(),
            tid: caps.get(3).map(|m| m.as_str()).unwrap_or("").to_string(),
            level: caps.get(4).map(|m| m.as_str()).unwrap_or("I").to_string(),
            tag: caps
                .get(5)
                .map(|m| m.as_str().trim())
                .unwrap_or("")
                .to_string(),
            message: caps.get(6).map(|m| m.as_str()).unwrap_or("").to_string(),
            raw: raw.to_string(),
        }
    } else {
        LogEntry {
            id,
            timestamp: String::new(),
            pid: String::new(),
            tid: String::new(),
            level: "I".into(),
            tag: "raw".into(),
            message: raw.to_string(),
            raw: raw.to_string(),
        }
    }
}

pub fn resolve_pkg_pids(serial: &str, pkg: &str) -> AppResult<Vec<String>> {
    let mut dev = ADBServerDevice::new(serial.to_string(), Some(default_server_addr()));
    let mut out = Vec::new();
    dev.shell_command(&format!("pidof {pkg}"), Some(&mut out), None)
        .map_err(|e| AppError::Adb(e.to_string()))?;
    let text = String::from_utf8_lossy(&out);
    let pids: Vec<String> = text
        .split_whitespace()
        .filter(|s| s.chars().all(|c| c.is_ascii_digit()))
        .map(|s| s.to_string())
        .collect();
    Ok(pids)
}
