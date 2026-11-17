use std::path::Path;
use std::sync::{Arc, Mutex};

use rusqlite::{params, Connection, OptionalExtension};
use serde_json::Value as JsonValue;

use crate::error::AppResult;
use crate::models::{
    CommandSnippet, DeviceEndpointRecord, DeviceMetaUpdate, DeviceRecord, ScrcpyOptions,
    ScrcpyProfile,
};

pub struct Database {
    conn: Arc<Mutex<Connection>>,
}

impl Database {
    pub fn open(db_path: &Path) -> AppResult<Self> {
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(db_path)?;
        conn.execute_batch(
            "
            PRAGMA journal_mode = WAL;
            PRAGMA foreign_keys = ON;
            PRAGMA synchronous = NORMAL;
            ",
        )?;
        let db = Self {
            conn: Arc::new(Mutex::new(conn)),
        };
        db.migrate()?;
        Ok(db)
    }

    fn migrate(&self) -> AppResult<()> {
        let conn = self.conn.lock().unwrap();
        let version: i32 = conn.query_row("PRAGMA user_version;", [], |row| row.get(0))?;
        if version < 1 {
            conn.execute_batch(
                "
                CREATE TABLE IF NOT EXISTS devices (
                    device_uid          TEXT PRIMARY KEY,
                    hardware_serial     TEXT,
                    boot_serial         TEXT,
                    android_id          TEXT,
                    brand               TEXT,
                    manufacturer        TEXT,
                    model               TEXT,
                    market_name         TEXT,
                    device_code         TEXT,
                    abi                 TEXT,
                    android_version     TEXT,
                    sdk_int             INTEGER,
                    build_fingerprint   TEXT,
                    alias               TEXT,
                    color_tag           TEXT,
                    notes               TEXT,
                    auto_connect        INTEGER NOT NULL DEFAULT 0,
                    default_profile_id  TEXT,
                    first_seen_at       INTEGER NOT NULL,
                    last_seen_at        INTEGER NOT NULL
                );
                CREATE INDEX IF NOT EXISTS idx_devices_hw_serial ON devices(hardware_serial);
                CREATE INDEX IF NOT EXISTS idx_devices_android_id ON devices(android_id);

                CREATE TABLE IF NOT EXISTS device_endpoints (
                    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
                    device_uid          TEXT NOT NULL REFERENCES devices(device_uid) ON DELETE CASCADE,
                    transport_type      TEXT NOT NULL,
                    adb_serial          TEXT,
                    mdns_service_name   TEXT,
                    mdns_reg_type       TEXT,
                    last_ip             TEXT,
                    last_port           INTEGER,
                    last_seen_at        INTEGER NOT NULL,
                    UNIQUE(device_uid, transport_type, adb_serial, mdns_service_name)
                );
                CREATE INDEX IF NOT EXISTS idx_endpoints_mdns_name ON device_endpoints(mdns_service_name);
                CREATE INDEX IF NOT EXISTS idx_endpoints_ip ON device_endpoints(last_ip);

                CREATE TABLE IF NOT EXISTS scrcpy_profiles (
                    id                  TEXT PRIMARY KEY,
                    name                TEXT NOT NULL,
                    is_builtin          INTEGER NOT NULL DEFAULT 0,
                    options_json        TEXT NOT NULL,
                    updated_at          INTEGER NOT NULL
                );

                CREATE TABLE IF NOT EXISTS command_snippets (
                    id                  TEXT PRIMARY KEY,
                    title               TEXT NOT NULL,
                    category            TEXT NOT NULL DEFAULT 'general',
                    command             TEXT NOT NULL,
                    auto_enter          INTEGER NOT NULL DEFAULT 1,
                    sort_order          INTEGER NOT NULL DEFAULT 0
                );

                CREATE TABLE IF NOT EXISTS app_settings (
                    key                 TEXT PRIMARY KEY,
                    value_json          TEXT NOT NULL,
                    updated_at          INTEGER NOT NULL
                );

                PRAGMA user_version = 1;
                ",
            )?;
            drop(conn);
            self.seed_defaults()?;
        }
        Ok(())
    }

    fn seed_defaults(&self) -> AppResult<()> {
        let now = now_ms();
        let default_profiles = vec![
            (
                "builtin-balanced",
                "默认均衡",
                ScrcpyOptions {
                    max_size: Some(1920),
                    video_bit_rate: Some("8M".into()),
                    max_fps: Some(60),
                    video_codec: Some("h264".into()),
                    turn_screen_off: false,
                    stay_awake: true,
                    always_on_top: false,
                    show_touches: false,
                    read_only: false,
                    keyboard_mode: Some("sdk".into()),
                    mouse_mode: Some("sdk".into()),
                    no_audio: false,
                    audio_codec: Some("opus".into()),
                    new_display: None,
                    start_app: None,
                    window_title: None,
                    extra_args: None,
                },
            ),
            (
                "builtin-virtual-display",
                "虚拟副屏 1080p",
                ScrcpyOptions {
                    max_size: None,
                    video_bit_rate: Some("12M".into()),
                    max_fps: Some(60),
                    video_codec: Some("h264".into()),
                    turn_screen_off: false,
                    stay_awake: true,
                    always_on_top: false,
                    show_touches: false,
                    read_only: false,
                    keyboard_mode: Some("uhid".into()),
                    mouse_mode: Some("sdk".into()),
                    no_audio: false,
                    audio_codec: Some("opus".into()),
                    new_display: Some("1920x1080/320".into()),
                    start_app: None,
                    window_title: Some("adbmaid 虚拟副屏".into()),
                    extra_args: None,
                },
            ),
            (
                "builtin-gaming",
                "高帧低延迟",
                ScrcpyOptions {
                    max_size: Some(1600),
                    video_bit_rate: Some("16M".into()),
                    max_fps: Some(120),
                    video_codec: Some("h265".into()),
                    turn_screen_off: true,
                    stay_awake: true,
                    always_on_top: false,
                    show_touches: false,
                    read_only: false,
                    keyboard_mode: Some("uhid".into()),
                    mouse_mode: Some("uhid".into()),
                    no_audio: false,
                    audio_codec: Some("opus".into()),
                    new_display: None,
                    start_app: None,
                    window_title: None,
                    extra_args: None,
                },
            ),
        ];

        let conn = self.conn.lock().unwrap();
        for (id, name, opts) in default_profiles {
            let json = serde_json::to_string(&opts).unwrap_or_default();
            conn.execute(
                "INSERT OR IGNORE INTO scrcpy_profiles (id, name, is_builtin, options_json, updated_at) VALUES (?1, ?2, 1, ?3, ?4)",
                params![id, name, json, now],
            )?;
        }

        let default_snippets = vec![
            (
                "snip-top-activity",
                "查看当前前台 Activity",
                "调试",
                "dumpsys window | grep -E 'mCurrentFocus|mFocusedApp'",
                true,
                1,
            ),
            (
                "snip-ip",
                "查看无线局域网 IP",
                "网络",
                "ip addr show wlan0 | grep 'inet '",
                true,
                2,
            ),
            (
                "snip-wm-size",
                "查看屏幕分辨率与 DPI",
                "显示",
                "wm size && wm density",
                true,
                3,
            ),
            (
                "snip-battery",
                "查看电池状态",
                "系统",
                "dumpsys battery",
                true,
                4,
            ),
            (
                "snip-ps",
                "查看运行中进程",
                "系统",
                "ps -A -o PID,NAME | head -n 30",
                true,
                5,
            ),
        ];
        for (id, title, cat, cmd, auto_enter, order) in default_snippets {
            conn.execute(
                "INSERT OR IGNORE INTO command_snippets (id, title, category, command, auto_enter, sort_order) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![id, title, cat, cmd, auto_enter as i32, order],
            )?;
        }
        Ok(())
    }

    pub fn get_setting(&self, key: &str) -> AppResult<Option<JsonValue>> {
        let conn = self.conn.lock().unwrap();
        let val: Option<String> = conn
            .query_row(
                "SELECT value_json FROM app_settings WHERE key = ?1",
                params![key],
                |row| row.get(0),
            )
            .optional()?;
        match val {
            Some(s) => Ok(serde_json::from_str(&s).ok()),
            None => Ok(None),
        }
    }

    pub fn set_setting(&self, key: &str, value: &JsonValue) -> AppResult<()> {
        let conn = self.conn.lock().unwrap();
        let json = serde_json::to_string(value).unwrap_or_else(|_| "null".into());
        conn.execute(
            "INSERT INTO app_settings (key, value_json, updated_at) VALUES (?1, ?2, ?3)
             ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json, updated_at = excluded.updated_at",
            params![key, json, now_ms()],
        )?;
        Ok(())
    }

    pub fn upsert_device_fingerprint(&self, mut rec: DeviceRecord) -> AppResult<DeviceRecord> {
        let conn = self.conn.lock().unwrap();
        // 优先通过 hardware_serial 或 android_id 查找已有物理设备
        let existing_uid: Option<String> = if let Some(ref hw) = rec.hardware_serial {
            conn.query_row(
                "SELECT device_uid FROM devices WHERE hardware_serial = ?1 LIMIT 1",
                params![hw],
                |row| row.get(0),
            )
            .optional()?
        } else {
            None
        }
        .or_else(|| {
            rec.android_id.as_ref().and_then(|aid| {
                conn.query_row(
                    "SELECT device_uid FROM devices WHERE android_id = ?1 LIMIT 1",
                    params![aid],
                    |row| row.get(0),
                )
                .optional()
                .ok()
                .flatten()
            })
        });

        let now = now_ms();
        if let Some(uid) = existing_uid {
            rec.device_uid = uid.clone();
            conn.execute(
                "UPDATE devices SET
                    hardware_serial = COALESCE(?2, hardware_serial),
                    boot_serial = COALESCE(?3, boot_serial),
                    android_id = COALESCE(?4, android_id),
                    brand = COALESCE(?5, brand),
                    manufacturer = COALESCE(?6, manufacturer),
                    model = COALESCE(?7, model),
                    market_name = COALESCE(?8, market_name),
                    device_code = COALESCE(?9, device_code),
                    abi = COALESCE(?10, abi),
                    android_version = COALESCE(?11, android_version),
                    sdk_int = COALESCE(?12, sdk_int),
                    build_fingerprint = COALESCE(?13, build_fingerprint),
                    last_seen_at = ?14
                 WHERE device_uid = ?1",
                params![
                    uid,
                    rec.hardware_serial,
                    rec.boot_serial,
                    rec.android_id,
                    rec.brand,
                    rec.manufacturer,
                    rec.model,
                    rec.market_name,
                    rec.device_code,
                    rec.abi,
                    rec.android_version,
                    rec.sdk_int,
                    rec.build_fingerprint,
                    now
                ],
            )?;
            drop(conn);
            return self
                .get_device_by_uid(&rec.device_uid)
                .map(|opt| opt.unwrap_or(rec));
        }

        if rec.device_uid.is_empty() {
            rec.device_uid = uuid::Uuid::new_v4().to_string();
        }
        rec.first_seen_at = now;
        rec.last_seen_at = now;
        conn.execute(
            "INSERT INTO devices (
                device_uid, hardware_serial, boot_serial, android_id,
                brand, manufacturer, model, market_name, device_code,
                abi, android_version, sdk_int, build_fingerprint,
                alias, color_tag, notes, auto_connect, default_profile_id,
                first_seen_at, last_seen_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19, ?20)",
            params![
                rec.device_uid,
                rec.hardware_serial,
                rec.boot_serial,
                rec.android_id,
                rec.brand,
                rec.manufacturer,
                rec.model,
                rec.market_name,
                rec.device_code,
                rec.abi,
                rec.android_version,
                rec.sdk_int,
                rec.build_fingerprint,
                rec.alias,
                rec.color_tag,
                rec.notes,
                rec.auto_connect as i32,
                rec.default_profile_id,
                rec.first_seen_at,
                rec.last_seen_at,
            ],
        )?;
        Ok(rec)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn upsert_endpoint(
        &self,
        device_uid: &str,
        transport_type: &str,
        adb_serial: Option<&str>,
        mdns_service_name: Option<&str>,
        mdns_reg_type: Option<&str>,
        ip: Option<&str>,
        port: Option<u16>,
    ) -> AppResult<()> {
        let conn = self.conn.lock().unwrap();
        let now = now_ms();
        conn.execute(
            "INSERT INTO device_endpoints (
                device_uid, transport_type, adb_serial, mdns_service_name,
                mdns_reg_type, last_ip, last_port, last_seen_at
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
            ON CONFLICT(device_uid, transport_type, adb_serial, mdns_service_name)
            DO UPDATE SET
                mdns_reg_type = COALESCE(excluded.mdns_reg_type, device_endpoints.mdns_reg_type),
                last_ip = COALESCE(excluded.last_ip, device_endpoints.last_ip),
                last_port = COALESCE(excluded.last_port, device_endpoints.last_port),
                last_seen_at = excluded.last_seen_at",
            params![
                device_uid,
                transport_type,
                adb_serial,
                mdns_service_name,
                mdns_reg_type,
                ip,
                port.map(|p| p as i32),
                now
            ],
        )?;
        Ok(())
    }

    pub fn list_known_devices(&self) -> AppResult<Vec<DeviceRecord>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT device_uid, hardware_serial, boot_serial, android_id,
                    brand, manufacturer, model, market_name, device_code,
                    abi, android_version, sdk_int, build_fingerprint,
                    alias, color_tag, notes, auto_connect, default_profile_id,
                    first_seen_at, last_seen_at
             FROM devices ORDER BY last_seen_at DESC",
        )?;
        let rows = stmt.query_map([], row_to_device)?;
        let mut list = Vec::new();
        for r in rows {
            list.push(r?);
        }
        Ok(list)
    }

    pub fn get_device_by_uid(&self, uid: &str) -> AppResult<Option<DeviceRecord>> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT device_uid, hardware_serial, boot_serial, android_id,
                    brand, manufacturer, model, market_name, device_code,
                    abi, android_version, sdk_int, build_fingerprint,
                    alias, color_tag, notes, auto_connect, default_profile_id,
                    first_seen_at, last_seen_at
             FROM devices WHERE device_uid = ?1",
            params![uid],
            row_to_device,
        )
        .optional()
        .map_err(Into::into)
    }

    pub fn find_device_by_adb_serial(&self, adb_serial: &str) -> AppResult<Option<DeviceRecord>> {
        let conn = self.conn.lock().unwrap();
        let uid: Option<String> = conn
            .query_row(
                "SELECT device_uid FROM device_endpoints WHERE adb_serial = ?1 ORDER BY last_seen_at DESC LIMIT 1",
                params![adb_serial],
                |row| row.get(0),
            )
            .optional()?;
        if let Some(uid) = uid {
            drop(conn);
            return self.get_device_by_uid(&uid);
        }
        // 也尝试直接匹配 hardware_serial
        conn.query_row(
            "SELECT device_uid, hardware_serial, boot_serial, android_id,
                    brand, manufacturer, model, market_name, device_code,
                    abi, android_version, sdk_int, build_fingerprint,
                    alias, color_tag, notes, auto_connect, default_profile_id,
                    first_seen_at, last_seen_at
             FROM devices WHERE hardware_serial = ?1 LIMIT 1",
            params![adb_serial],
            row_to_device,
        )
        .optional()
        .map_err(Into::into)
    }

    pub fn find_device_by_hw_serial(&self, hw_serial: &str) -> AppResult<Option<DeviceRecord>> {
        let conn = self.conn.lock().unwrap();
        conn.query_row(
            "SELECT device_uid, hardware_serial, boot_serial, android_id,
                    brand, manufacturer, model, market_name, device_code,
                    abi, android_version, sdk_int, build_fingerprint,
                    alias, color_tag, notes, auto_connect, default_profile_id,
                    first_seen_at, last_seen_at
             FROM devices WHERE hardware_serial = ?1 LIMIT 1",
            params![hw_serial],
            row_to_device,
        )
        .optional()
        .map_err(Into::into)
    }

    pub fn find_device_by_mdns_name(&self, service_name: &str) -> AppResult<Option<DeviceRecord>> {
        let conn = self.conn.lock().unwrap();
        let uid: Option<String> = conn
            .query_row(
                "SELECT device_uid FROM device_endpoints WHERE mdns_service_name = ?1 ORDER BY last_seen_at DESC LIMIT 1",
                params![service_name],
                |row| row.get(0),
            )
            .optional()?;
        drop(conn);
        match uid {
            Some(u) => self.get_device_by_uid(&u),
            None => Ok(None),
        }
    }

    pub fn find_device_by_ip(&self, ip: &str) -> AppResult<Option<DeviceRecord>> {
        let conn = self.conn.lock().unwrap();
        let uid: Option<String> = conn
            .query_row(
                "SELECT device_uid FROM device_endpoints WHERE last_ip = ?1 ORDER BY last_seen_at DESC LIMIT 1",
                params![ip],
                |row| row.get(0),
            )
            .optional()?;
        drop(conn);
        match uid {
            Some(u) => self.get_device_by_uid(&u),
            None => Ok(None),
        }
    }

    pub fn update_device_meta(&self, update: &DeviceMetaUpdate) -> AppResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "UPDATE devices SET
                alias = COALESCE(?2, alias),
                color_tag = COALESCE(?3, color_tag),
                notes = COALESCE(?4, notes),
                auto_connect = COALESCE(?5, auto_connect),
                default_profile_id = COALESCE(?6, default_profile_id)
             WHERE device_uid = ?1",
            params![
                update.device_uid,
                update.alias,
                update.color_tag,
                update.notes,
                update.auto_connect.map(|b| b as i32),
                update.default_profile_id,
            ],
        )?;
        Ok(())
    }

    pub fn list_device_endpoints(&self, device_uid: &str) -> AppResult<Vec<DeviceEndpointRecord>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, device_uid, transport_type, adb_serial, mdns_service_name,
                    mdns_reg_type, last_ip, last_port, last_seen_at
             FROM device_endpoints WHERE device_uid = ?1 ORDER BY last_seen_at DESC",
        )?;
        let rows = stmt.query_map(params![device_uid], |row| {
            let port: Option<i32> = row.get(7)?;
            Ok(DeviceEndpointRecord {
                id: row.get(0)?,
                device_uid: row.get(1)?,
                transport_type: row.get(2)?,
                adb_serial: row.get(3)?,
                mdns_service_name: row.get(4)?,
                mdns_reg_type: row.get(5)?,
                last_ip: row.get(6)?,
                last_port: port.map(|p| p as u16),
                last_seen_at: row.get(8)?,
            })
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    pub fn list_scrcpy_profiles(&self) -> AppResult<Vec<ScrcpyProfile>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, name, is_builtin, options_json, updated_at FROM scrcpy_profiles ORDER BY is_builtin DESC, updated_at DESC",
        )?;
        let rows = stmt.query_map([], |row| {
            let json_str: String = row.get(3)?;
            let options: ScrcpyOptions = serde_json::from_str(&json_str).unwrap_or(ScrcpyOptions {
                max_size: Some(1920),
                video_bit_rate: Some("8M".into()),
                max_fps: Some(60),
                video_codec: Some("h264".into()),
                turn_screen_off: false,
                stay_awake: true,
                always_on_top: false,
                show_touches: false,
                read_only: false,
                keyboard_mode: Some("sdk".into()),
                mouse_mode: Some("sdk".into()),
                no_audio: false,
                audio_codec: Some("opus".into()),
                new_display: None,
                start_app: None,
                window_title: None,
                extra_args: None,
            });
            Ok(ScrcpyProfile {
                id: row.get(0)?,
                name: row.get(1)?,
                is_builtin: row.get::<_, i32>(2)? != 0,
                options,
                updated_at: row.get(4)?,
            })
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    pub fn save_scrcpy_profile(&self, profile: &ScrcpyProfile) -> AppResult<()> {
        let conn = self.conn.lock().unwrap();
        let json = serde_json::to_string(&profile.options).unwrap_or_default();
        conn.execute(
            "INSERT INTO scrcpy_profiles (id, name, is_builtin, options_json, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5)
             ON CONFLICT(id) DO UPDATE SET name = excluded.name, options_json = excluded.options_json, updated_at = excluded.updated_at",
            params![profile.id, profile.name, profile.is_builtin as i32, json, now_ms()],
        )?;
        Ok(())
    }

    pub fn delete_scrcpy_profile(&self, id: &str) -> AppResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "DELETE FROM scrcpy_profiles WHERE id = ?1 AND is_builtin = 0",
            params![id],
        )?;
        Ok(())
    }

    pub fn list_command_snippets(&self) -> AppResult<Vec<CommandSnippet>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn.prepare(
            "SELECT id, title, category, command, auto_enter, sort_order FROM command_snippets ORDER BY sort_order ASC, title ASC",
        )?;
        let rows = stmt.query_map([], |row| {
            Ok(CommandSnippet {
                id: row.get(0)?,
                title: row.get(1)?,
                category: row.get(2)?,
                command: row.get(3)?,
                auto_enter: row.get::<_, i32>(4)? != 0,
                sort_order: row.get(5)?,
            })
        })?;
        let mut out = Vec::new();
        for r in rows {
            out.push(r?);
        }
        Ok(out)
    }

    pub fn save_command_snippet(&self, snippet: &CommandSnippet) -> AppResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute(
            "INSERT INTO command_snippets (id, title, category, command, auto_enter, sort_order)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)
             ON CONFLICT(id) DO UPDATE SET title = excluded.title, category = excluded.category, command = excluded.command, auto_enter = excluded.auto_enter, sort_order = excluded.sort_order",
            params![
                snippet.id,
                snippet.title,
                snippet.category,
                snippet.command,
                snippet.auto_enter as i32,
                snippet.sort_order
            ],
        )?;
        Ok(())
    }

    pub fn delete_command_snippet(&self, id: &str) -> AppResult<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute("DELETE FROM command_snippets WHERE id = ?1", params![id])?;
        Ok(())
    }
}

fn row_to_device(row: &rusqlite::Row<'_>) -> rusqlite::Result<DeviceRecord> {
    Ok(DeviceRecord {
        device_uid: row.get(0)?,
        hardware_serial: row.get(1)?,
        boot_serial: row.get(2)?,
        android_id: row.get(3)?,
        brand: row.get(4)?,
        manufacturer: row.get(5)?,
        model: row.get(6)?,
        market_name: row.get(7)?,
        device_code: row.get(8)?,
        abi: row.get(9)?,
        android_version: row.get(10)?,
        sdk_int: row.get(11)?,
        build_fingerprint: row.get(12)?,
        alias: row.get(13)?,
        color_tag: row.get(14)?,
        notes: row.get(15)?,
        auto_connect: row.get::<_, i32>(16)? != 0,
        default_profile_id: row.get(17)?,
        first_seen_at: row.get(18)?,
        last_seen_at: row.get(19)?,
    })
}

pub fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}
