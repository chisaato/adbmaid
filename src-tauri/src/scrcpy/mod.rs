use std::collections::HashMap;
use std::io::Read;
use std::process::{Child, Command, Stdio};
use std::sync::{Arc, Mutex};

use tauri::{AppHandle, Emitter};

use crate::db::now_ms;
use crate::error::{AppError, AppResult};
use crate::models::{ScrcpyExitedEvent, ScrcpyInstance, ScrcpyOptions};

struct ScrcpyEntry {
    info: ScrcpyInstance,
    child: Arc<Mutex<Child>>,
}

#[derive(Default)]
pub struct ScrcpyManager {
    processes: Arc<Mutex<HashMap<String, ScrcpyEntry>>>,
}

impl ScrcpyManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn list_running(&self) -> Vec<ScrcpyInstance> {
        let guard = self.processes.lock().unwrap();
        guard.values().map(|e| e.info.clone()).collect()
    }

    #[allow(clippy::too_many_arguments)]
    pub fn start(
        &self,
        app: AppHandle,
        scrcpy_path: &str,
        scrcpy_server_path: Option<&str>,
        adb_path: &str,
        serial: &str,
        profile_name: Option<String>,
        options: &ScrcpyOptions,
    ) -> AppResult<ScrcpyInstance> {
        let mut cmd = Command::new(scrcpy_path);
        cmd.args(["-s", serial]);

        // 工具链环境变量强隔离（Spec 第 2.3 条红线）
        cmd.env("ADB", adb_path);
        if let Some(srv) = scrcpy_server_path {
            cmd.env("SCRCPY_SERVER_PATH", srv);
        }

        if let Some(max_size) = options.max_size {
            cmd.args(["-m", &max_size.to_string()]);
        }
        if let Some(ref br) = options.video_bit_rate {
            if !br.trim().is_empty() {
                cmd.args(["-b", br.trim()]);
            }
        }
        if let Some(fps) = options.max_fps {
            cmd.arg(format!("--max-fps={fps}"));
        }
        if let Some(ref codec) = options.video_codec {
            if !codec.trim().is_empty() {
                cmd.arg(format!("--video-codec={}", codec.trim()));
            }
        }
        if options.turn_screen_off {
            cmd.arg("-S");
        }
        if options.stay_awake {
            cmd.arg("-w");
        }
        if options.always_on_top {
            cmd.arg("--always-on-top");
        }
        if options.show_touches {
            cmd.arg("-t");
        }
        if options.read_only {
            cmd.arg("-n");
        }
        if let Some(ref kb) = options.keyboard_mode {
            if !kb.trim().is_empty() {
                cmd.arg(format!("--keyboard={}", kb.trim()));
            }
        }
        if let Some(ref ms) = options.mouse_mode {
            if !ms.trim().is_empty() {
                cmd.arg(format!("--mouse={}", ms.trim()));
            }
        }
        if options.no_audio {
            cmd.arg("--no-audio");
        } else if let Some(ref ac) = options.audio_codec {
            if !ac.trim().is_empty() {
                cmd.arg(format!("--audio-codec={}", ac.trim()));
            }
        }
        if let Some(ref nd) = options.new_display {
            if !nd.trim().is_empty() {
                cmd.arg(format!("--new-display={}", nd.trim()));
            }
        }
        if let Some(ref app_pkg) = options.start_app {
            if !app_pkg.trim().is_empty() {
                cmd.arg(format!("--start-app={}", app_pkg.trim()));
            }
        }
        if let Some(ref title) = options.window_title {
            if !title.trim().is_empty() {
                cmd.arg(format!("--window-title={}", title.trim()));
            }
        }
        if let Some(ref extra) = options.extra_args {
            for arg in extra.split_whitespace() {
                cmd.arg(arg);
            }
        }

        cmd.stdout(Stdio::null()).stderr(Stdio::piped());

        let mut child = cmd
            .spawn()
            .map_err(|e| AppError::Toolchain(format!("启动 scrcpy 失败: {e}")))?;

        let stderr = child.stderr.take();
        let instance_id = uuid::Uuid::new_v4().to_string();
        let info = ScrcpyInstance {
            instance_id: instance_id.clone(),
            serial: serial.to_string(),
            profile_name,
            started_at: now_ms(),
        };

        let child_arc = Arc::new(Mutex::new(child));
        {
            let mut guard = self.processes.lock().unwrap();
            guard.insert(
                instance_id.clone(),
                ScrcpyEntry {
                    info: info.clone(),
                    child: child_arc.clone(),
                },
            );
        }

        let procs_ref = self.processes.clone();
        let iid = instance_id.clone();
        let ser = serial.to_string();

        std::thread::spawn(move || {
            let mut err_buf = String::new();
            if let Some(mut s) = stderr {
                let _ = s.read_to_string(&mut err_buf);
            }
            let code = loop {
                let status_opt = {
                    let mut guard = child_arc.lock().unwrap();
                    guard.try_wait().ok().flatten()
                };
                if let Some(st) = status_opt {
                    break st.code();
                }
                std::thread::sleep(std::time::Duration::from_millis(200));
            };

            if let Ok(mut guard) = procs_ref.lock() {
                guard.remove(&iid);
            }

            let tail = if err_buf.trim().is_empty() {
                None
            } else {
                let lines: Vec<&str> = err_buf.lines().collect();
                let start = lines.len().saturating_sub(12);
                Some(lines[start..].join("\n"))
            };

            let _ = app.emit(
                "scrcpy://exited",
                ScrcpyExitedEvent {
                    instance_id: iid,
                    serial: ser,
                    code,
                    stderr_tail: tail,
                },
            );
        });

        Ok(info)
    }

    pub fn stop(&self, instance_id: &str) -> AppResult<()> {
        let mut guard = self.processes.lock().unwrap();
        if let Some(entry) = guard.remove(instance_id) {
            if let Ok(mut c) = entry.child.lock() {
                let _ = c.kill();
            }
        }
        Ok(())
    }
}
