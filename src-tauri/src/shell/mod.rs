use std::collections::HashMap;
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};

use portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtySize};
use tauri::ipc::Channel;

use crate::error::{AppError, AppResult};

struct PtySession {
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn Write + Send>,
    child: Box<dyn portable_pty::Child + Send + Sync>,
}

#[derive(Default)]
pub struct ShellManager {
    sessions: Arc<Mutex<HashMap<String, PtySession>>>,
}

impl ShellManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn open(
        &self,
        adb_path: &str,
        serial: &str,
        cols: u16,
        rows: u16,
        on_output: Channel<Vec<u8>>,
    ) -> AppResult<String> {
        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(PtySize {
                rows: rows.max(4),
                cols: cols.max(20),
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| AppError::Internal(format!("创建 PTY 失败: {e}")))?;

        let mut cmd = CommandBuilder::new(adb_path);
        cmd.arg("-s");
        cmd.arg(serial);
        cmd.arg("shell");
        cmd.env("TERM", "xterm-256color");

        let child = pair
            .slave
            .spawn_command(cmd)
            .map_err(|e| AppError::Internal(format!("启动 adb shell 失败: {e}")))?;
        drop(pair.slave);

        let mut reader = pair
            .master
            .try_clone_reader()
            .map_err(|e| AppError::Internal(format!("获取 PTY reader 失败: {e}")))?;
        let writer = pair
            .master
            .take_writer()
            .map_err(|e| AppError::Internal(format!("获取 PTY writer 失败: {e}")))?;

        let session_id = uuid::Uuid::new_v4().to_string();
        {
            let mut guard = self.sessions.lock().unwrap();
            guard.insert(
                session_id.clone(),
                PtySession {
                    master: pair.master,
                    writer,
                    child,
                },
            );
        }

        let sessions_ref = self.sessions.clone();
        let sid_clone = session_id.clone();
        std::thread::spawn(move || {
            let mut buf = [0u8; 4096];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        if on_output.send(buf[..n].to_vec()).is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
            if let Ok(mut guard) = sessions_ref.lock() {
                if let Some(mut sess) = guard.remove(&sid_clone) {
                    let _ = sess.child.kill();
                }
            }
        });

        Ok(session_id)
    }

    pub fn write(&self, session_id: &str, data: &[u8]) -> AppResult<()> {
        let mut guard = self.sessions.lock().unwrap();
        let sess = guard
            .get_mut(session_id)
            .ok_or_else(|| AppError::NotFound("终端会话不存在或已关闭".into()))?;
        sess.writer.write_all(data)?;
        sess.writer.flush()?;
        Ok(())
    }

    pub fn resize(&self, session_id: &str, cols: u16, rows: u16) -> AppResult<()> {
        let guard = self.sessions.lock().unwrap();
        let sess = guard
            .get(session_id)
            .ok_or_else(|| AppError::NotFound("终端会话不存在".into()))?;
        sess.master
            .resize(PtySize {
                rows: rows.max(4),
                cols: cols.max(20),
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| AppError::Internal(e.to_string()))?;
        Ok(())
    }

    pub fn close(&self, session_id: &str) -> AppResult<()> {
        let mut guard = self.sessions.lock().unwrap();
        if let Some(mut sess) = guard.remove(session_id) {
            let _ = sess.child.kill();
        }
        Ok(())
    }
}
