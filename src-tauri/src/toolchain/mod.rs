use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

use flate2::read::GzDecoder;
use regex::Regex;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use tar::Archive;
use tauri::ipc::Channel;

use crate::db::Database;
use crate::error::{AppError, AppResult};
use crate::models::{DownloadProgress, LinuxDistroInfo, ToolBinaryInfo, ToolchainProbeResult};

const SCRCPY_GITHUB_API: &str = "https://api.github.com/repos/Genymobile/scrcpy/releases/latest";

#[derive(Debug, Deserialize)]
struct GithubAsset {
    name: String,
    browser_download_url: String,
    size: u64,
}

#[derive(Debug, Deserialize)]
struct GithubRelease {
    tag_name: String,
    assets: Vec<GithubAsset>,
}

pub fn probe_toolchain(app_data_dir: &Path, db: &Database) -> AppResult<ToolchainProbeResult> {
    let os = std::env::consts::OS.to_string();
    let arch = std::env::consts::ARCH.to_string();

    let custom_adb = db
        .get_setting("toolchain.adb_path")?
        .and_then(|v| v.as_str().map(|s| s.to_string()))
        .filter(|s| !s.trim().is_empty());

    let custom_scrcpy = db
        .get_setting("toolchain.scrcpy_path")?
        .and_then(|v| v.as_str().map(|s| s.to_string()))
        .filter(|s| !s.trim().is_empty());

    let adb = probe_adb(app_data_dir, custom_adb.as_deref());
    let scrcpy = probe_scrcpy(app_data_dir, custom_scrcpy.as_deref());

    let can_auto_download_scrcpy = matches!(
        (os.as_str(), arch.as_str()),
        ("windows", _) | ("macos", "aarch64" | "x86_64") | ("linux", "x86_64")
    );

    let can_auto_download_adb =
        matches!(os.as_str(), "windows" | "macos") || (os == "linux" && arch == "x86_64");

    let linux_distro = if os == "linux" {
        Some(detect_linux_distro())
    } else {
        None
    };

    Ok(ToolchainProbeResult {
        os,
        arch,
        can_auto_download_scrcpy,
        can_auto_download_adb,
        linux_distro,
        adb,
        scrcpy,
    })
}

fn exe_name(base: &str) -> String {
    if cfg!(windows) {
        format!("{base}.exe")
    } else {
        base.to_string()
    }
}

fn probe_adb(app_data_dir: &Path, custom_path: Option<&str>) -> ToolBinaryInfo {
    let bin = exe_name("adb");

    // 1. 用户手动指定
    if let Some(p) = custom_path {
        let path = PathBuf::from(p);
        if let Some(info) = verify_adb(&path, "custom") {
            return info;
        }
    }

    // 2. 应用托管目录 (platform-tools 或 scrcpy 随附)
    let managed_pt = app_data_dir
        .join("toolchains")
        .join("platform-tools")
        .join(&bin);
    if let Some(info) = verify_adb(&managed_pt, "managed") {
        return info;
    }

    if let Some(scrcpy_dir) = find_active_managed_scrcpy_dir(app_data_dir) {
        let managed_adb = scrcpy_dir.join(&bin);
        if let Some(info) = verify_adb(&managed_adb, "managed") {
            return info;
        }
    }

    // 3. 系统 PATH
    if let Ok(path) = which::which(&bin) {
        if let Some(info) = verify_adb(&path, "system") {
            return info;
        }
    }

    // 4. 平台常见路径补充扫描
    for candidate in platform_adb_candidates(&bin) {
        if let Some(info) = verify_adb(&candidate, "system") {
            return info;
        }
    }

    ToolBinaryInfo {
        found: false,
        path: None,
        version: None,
        source: "missing".into(),
        server_path: None,
        is_outdated: false,
    }
}

fn probe_scrcpy(app_data_dir: &Path, custom_path: Option<&str>) -> ToolBinaryInfo {
    let bin = exe_name("scrcpy");

    // 1. 用户手动指定
    if let Some(p) = custom_path {
        let path = PathBuf::from(p);
        if let Some(info) = verify_scrcpy(&path, "custom") {
            return info;
        }
    }

    // 2. 应用托管目录
    if let Some(dir) = find_active_managed_scrcpy_dir(app_data_dir) {
        let managed = dir.join(&bin);
        if let Some(info) = verify_scrcpy(&managed, "managed") {
            return info;
        }
    }

    // 3. 系统 PATH
    if let Ok(path) = which::which(&bin) {
        if let Some(info) = verify_scrcpy(&path, "system") {
            return info;
        }
    }

    // 4. 平台常见路径
    for candidate in platform_scrcpy_candidates(&bin) {
        if let Some(info) = verify_scrcpy(&candidate, "system") {
            return info;
        }
    }

    ToolBinaryInfo {
        found: false,
        path: None,
        version: None,
        source: "missing".into(),
        server_path: None,
        is_outdated: false,
    }
}

fn find_active_managed_scrcpy_dir(app_data_dir: &Path) -> Option<PathBuf> {
    let base = app_data_dir.join("toolchains").join("scrcpy");
    if !base.exists() {
        return None;
    }
    let active_file = base.join("active");
    if let Ok(ver) = fs::read_to_string(&active_file) {
        let dir = base.join(ver.trim());
        if dir.is_dir() {
            return Some(dir);
        }
    }
    // 兜底：取第一个子目录
    fs::read_dir(&base)
        .ok()?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .find(|p| p.is_dir())
}

fn verify_adb(path: &Path, source: &str) -> Option<ToolBinaryInfo> {
    if !path.is_file() {
        return None;
    }
    let out = Command::new(path).arg("version").output().ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let version = text
        .lines()
        .find(|l| l.contains("Version") || l.contains("Android Debug Bridge"))
        .map(|l| l.trim().to_string());
    Some(ToolBinaryInfo {
        found: true,
        path: Some(path.to_string_lossy().to_string()),
        version,
        source: source.to_string(),
        server_path: None,
        is_outdated: false,
    })
}

fn verify_scrcpy(path: &Path, source: &str) -> Option<ToolBinaryInfo> {
    if !path.is_file() {
        return None;
    }
    let out = Command::new(path).arg("--version").output().ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let re = Regex::new(r"scrcpy\s+([0-9]+\.[0-9]+(?:\.[0-9]+)?)").ok()?;
    let version = re
        .captures(&text)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string());

    let is_outdated = version
        .as_deref()
        .and_then(|v| v.split('.').next())
        .and_then(|major| major.parse::<u32>().ok())
        .map(|major| major < 3)
        .unwrap_or(false);

    // 查找配套 scrcpy-server
    let server_path = find_scrcpy_server(path);

    Some(ToolBinaryInfo {
        found: true,
        path: Some(path.to_string_lossy().to_string()),
        version,
        source: source.to_string(),
        server_path: server_path.map(|p| p.to_string_lossy().to_string()),
        is_outdated,
    })
}

fn find_scrcpy_server(scrcpy_bin: &Path) -> Option<PathBuf> {
    if let Some(parent) = scrcpy_bin.parent() {
        let same_dir = parent.join("scrcpy-server");
        if same_dir.is_file() {
            return Some(same_dir);
        }
        // Linux 常见系统安装位置: /usr/share/scrcpy/scrcpy-server 或 /usr/local/share/scrcpy/scrcpy-server
        if let Some(grand) = parent.parent() {
            let share = grand.join("share").join("scrcpy").join("scrcpy-server");
            if share.is_file() {
                return Some(share);
            }
        }
    }
    for sys_path in [
        "/usr/share/scrcpy/scrcpy-server",
        "/usr/local/share/scrcpy/scrcpy-server",
        "/opt/homebrew/share/scrcpy/scrcpy-server",
    ] {
        let p = PathBuf::from(sys_path);
        if p.is_file() {
            return Some(p);
        }
    }
    None
}

fn platform_adb_candidates(bin: &str) -> Vec<PathBuf> {
    let mut list = Vec::new();
    if let Ok(ah) = std::env::var("ANDROID_HOME") {
        list.push(PathBuf::from(ah).join("platform-tools").join(bin));
    }
    if let Some(home) = dirs::home_dir() {
        #[cfg(target_os = "windows")]
        {
            if let Ok(local) = std::env::var("LOCALAPPDATA") {
                list.push(
                    PathBuf::from(local)
                        .join("Android")
                        .join("Sdk")
                        .join("platform-tools")
                        .join(bin),
                );
            }
            list.push(home.join("scoop").join("shims").join(bin));
        }
        #[cfg(target_os = "macos")]
        {
            list.push(PathBuf::from("/opt/homebrew/bin").join(bin));
            list.push(PathBuf::from("/usr/local/bin").join(bin));
            list.push(
                home.join("Library")
                    .join("Android")
                    .join("sdk")
                    .join("platform-tools")
                    .join(bin),
            );
        }
        #[cfg(target_os = "linux")]
        {
            list.push(PathBuf::from("/usr/bin").join(bin));
            list.push(PathBuf::from("/usr/local/bin").join(bin));
            list.push(home.join(".local").join("bin").join(bin));
            list.push(
                home.join("Android")
                    .join("Sdk")
                    .join("platform-tools")
                    .join(bin),
            );
        }
    }
    list
}

fn platform_scrcpy_candidates(bin: &str) -> Vec<PathBuf> {
    let mut list = Vec::new();
    if let Some(home) = dirs::home_dir() {
        #[cfg(target_os = "windows")]
        {
            list.push(home.join("scoop").join("shims").join(bin));
        }
        #[cfg(target_os = "macos")]
        {
            list.push(PathBuf::from("/opt/homebrew/bin").join(bin));
            list.push(PathBuf::from("/usr/local/bin").join(bin));
        }
        #[cfg(target_os = "linux")]
        {
            list.push(PathBuf::from("/usr/bin").join(bin));
            list.push(PathBuf::from("/usr/local/bin").join(bin));
            list.push(home.join(".local").join("bin").join(bin));
        }
    }
    list
}

fn detect_linux_distro() -> LinuxDistroInfo {
    let content = fs::read_to_string("/etc/os-release").unwrap_or_default();
    let mut id = String::from("linux");
    let mut pretty = String::from("Linux");
    for line in content.lines() {
        if let Some(val) = line.strip_prefix("ID=") {
            id = val.trim_matches('"').to_lowercase();
        } else if let Some(val) = line.strip_prefix("PRETTY_NAME=") {
            pretty = val.trim_matches('"').to_string();
        }
    }
    let install_hint = match id.as_str() {
        "arch" | "manjaro" | "endeavouros" => {
            "sudo pacman -S android-tools scrcpy".to_string()
        }
        "fedora" | "rhel" | "centos" => {
            "sudo dnf install android-tools && # scrcpy 可通过 COPR (zeno/scrcpy) 或源码编译安装".to_string()
        }
        "ubuntu" | "debian" | "linuxmint" | "pop" => {
            "sudo apt update && sudo apt install adb # 注意：apt 仓库 scrcpy 版本通常较旧，建议源码编译或使用 x86_64 官方静态包".to_string()
        }
        "nixos" => "nix-shell -p android-tools scrcpy".to_string(),
        _ => "请使用发行版包管理器安装 android-tools，并从 Genymobile/scrcpy 源码编译安装 scrcpy 3.x".to_string(),
    };
    LinuxDistroInfo {
        id,
        pretty_name: pretty,
        install_hint,
    }
}

pub async fn download_scrcpy(
    app_data_dir: &Path,
    mirror_prefix: Option<&str>,
    proxy_url: Option<&str>,
    on_progress: &Channel<DownloadProgress>,
) -> AppResult<String> {
    let _ = on_progress.send(DownloadProgress {
        stage: "fetching_meta".into(),
        received_bytes: 0,
        total_bytes: None,
        message: "正在查询 Genymobile/scrcpy 最新 Release...".into(),
    });

    let client = build_http_client(proxy_url)?;
    let release: GithubRelease = client
        .get(SCRCPY_GITHUB_API)
        .header("User-Agent", "adbmaid/0.1")
        .send()
        .await
        .map_err(|e| AppError::Network(e.to_string()))?
        .error_for_status()
        .map_err(|e| AppError::Network(e.to_string()))?
        .json()
        .await
        .map_err(|e| AppError::Network(e.to_string()))?;

    let asset_pattern = expected_scrcpy_asset_prefix()?;
    let asset = release
        .assets
        .iter()
        .find(|a| {
            a.name.starts_with(asset_pattern)
                && (a.name.ends_with(".tar.gz") || a.name.ends_with(".zip"))
        })
        .ok_or_else(|| {
            AppError::NotFound(format!("未找到匹配当前平台的官方包: {asset_pattern}"))
        })?;

    let sha_asset = release.assets.iter().find(|a| a.name == "SHA256SUMS.txt");

    let download_url = apply_mirror(&asset.browser_download_url, mirror_prefix);
    let tmp_dir = app_data_dir.join("toolchains").join("tmp");
    fs::create_dir_all(&tmp_dir)?;
    let archive_path = tmp_dir.join(&asset.name);

    // 流式下载
    let mut resp = client
        .get(&download_url)
        .header("User-Agent", "adbmaid/0.1")
        .send()
        .await
        .map_err(|e| AppError::Network(e.to_string()))?
        .error_for_status()
        .map_err(|e| AppError::Network(e.to_string()))?;

    let total = resp.content_length().or(Some(asset.size));
    let mut file = File::create(&archive_path)?;
    let mut received: u64 = 0;

    while let Some(chunk) = resp
        .chunk()
        .await
        .map_err(|e| AppError::Network(e.to_string()))?
    {
        file.write_all(&chunk)?;
        received += chunk.len() as u64;
        let _ = on_progress.send(DownloadProgress {
            stage: "downloading".into(),
            received_bytes: received,
            total_bytes: total,
            message: format!("正在下载 {}", asset.name),
        });
    }
    file.flush()?;

    // SHA-256 校验
    if let Some(sha_item) = sha_asset {
        let _ = on_progress.send(DownloadProgress {
            stage: "verifying".into(),
            received_bytes: received,
            total_bytes: total,
            message: "正在校验 SHA-256 完整性...".into(),
        });
        let sha_url = apply_mirror(&sha_item.browser_download_url, mirror_prefix);
        let sha_text = client
            .get(&sha_url)
            .header("User-Agent", "adbmaid/0.1")
            .send()
            .await
            .map_err(|e| AppError::Network(e.to_string()))?
            .text()
            .await
            .map_err(|e| AppError::Network(e.to_string()))?;

        verify_sha256_sums(&archive_path, &asset.name, &sha_text)?;
    }

    let _ = on_progress.send(DownloadProgress {
        stage: "extracting".into(),
        received_bytes: received,
        total_bytes: total,
        message: "正在解压并安装到托管目录...".into(),
    });

    let version_dir = app_data_dir
        .join("toolchains")
        .join("scrcpy")
        .join(&release.tag_name);
    extract_archive(&archive_path, &version_dir)?;
    flatten_single_subdir(&version_dir)?;

    fs::write(
        app_data_dir
            .join("toolchains")
            .join("scrcpy")
            .join("active"),
        &release.tag_name,
    )?;
    let _ = fs::remove_file(&archive_path);

    let _ = on_progress.send(DownloadProgress {
        stage: "done".into(),
        received_bytes: received,
        total_bytes: total,
        message: format!("scrcpy {} 安装完成", release.tag_name),
    });

    Ok(release.tag_name)
}

pub fn import_local_archive(
    app_data_dir: &Path,
    archive_path: &Path,
    kind: &str,
) -> AppResult<String> {
    if !archive_path.is_file() {
        return Err(AppError::NotFound("压缩包文件不存在".into()));
    }
    let target_dir = match kind {
        "scrcpy" => {
            let tag = "local-imported";
            let dir = app_data_dir.join("toolchains").join("scrcpy").join(tag);
            if dir.exists() {
                fs::remove_dir_all(&dir)?;
            }
            extract_archive(archive_path, &dir)?;
            flatten_single_subdir(&dir)?;
            fs::write(
                app_data_dir
                    .join("toolchains")
                    .join("scrcpy")
                    .join("active"),
                tag,
            )?;
            dir
        }
        "adb" => {
            let dir = app_data_dir.join("toolchains").join("platform-tools");
            if dir.exists() {
                fs::remove_dir_all(&dir)?;
            }
            extract_archive(archive_path, &dir)?;
            flatten_single_subdir(&dir)?;
            dir
        }
        _ => return Err(AppError::InvalidArg("不支持的工具链类型".into())),
    };
    Ok(target_dir.to_string_lossy().to_string())
}

fn expected_scrcpy_asset_prefix() -> AppResult<&'static str> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("linux", "x86_64") => Ok("scrcpy-linux-x86_64-"),
        ("windows", "x86_64") => Ok("scrcpy-win64-"),
        ("windows", "x86") => Ok("scrcpy-win32-"),
        ("macos", "aarch64") => Ok("scrcpy-macos-aarch64-"),
        ("macos", "x86_64") => Ok("scrcpy-macos-x86_64-"),
        (os, arch) => Err(AppError::Toolchain(format!(
            "当前平台 ({os}/{arch}) 无官方 scrcpy 预编译静态包，请使用包管理器或手动指定路径"
        ))),
    }
}

fn build_http_client(proxy_url: Option<&str>) -> AppResult<reqwest::Client> {
    let mut builder = reqwest::Client::builder().timeout(std::time::Duration::from_secs(120));
    if let Some(p) = proxy_url.filter(|s| !s.trim().is_empty()) {
        let proxy = reqwest::Proxy::all(p).map_err(|e| AppError::Network(e.to_string()))?;
        builder = builder.proxy(proxy);
    }
    builder
        .build()
        .map_err(|e| AppError::Network(e.to_string()))
}

fn apply_mirror(url: &str, mirror_prefix: Option<&str>) -> String {
    if let Some(prefix) = mirror_prefix.filter(|s| !s.trim().is_empty()) {
        let p = prefix.trim_end_matches('/');
        format!("{p}/{url}")
    } else {
        url.to_string()
    }
}

fn verify_sha256_sums(file_path: &Path, filename: &str, sha_sums_text: &str) -> AppResult<()> {
    let expected = sha_sums_text
        .lines()
        .find_map(|line| {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 && parts[1].ends_with(filename) {
                Some(parts[0].to_lowercase())
            } else {
                None
            }
        })
        .ok_or_else(|| AppError::Verify(format!("SHA256SUMS.txt 中未找到 {filename}")))?;

    let mut f = File::open(file_path)?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 8192];
    loop {
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    let actual = hex::encode(hasher.finalize());
    if actual != expected {
        return Err(AppError::Verify(format!(
            "SHA-256 校验失败: 期望 {expected}, 实际 {actual}"
        )));
    }
    Ok(())
}

fn extract_archive(archive_path: &Path, dest_dir: &Path) -> AppResult<()> {
    fs::create_dir_all(dest_dir)?;
    let name = archive_path
        .file_name()
        .map(|s| s.to_string_lossy().to_lowercase())
        .unwrap_or_default();

    if name.ends_with(".tar.gz") || name.ends_with(".tgz") {
        let tar_gz = File::open(archive_path)?;
        let tar = GzDecoder::new(tar_gz);
        let mut archive = Archive::new(tar);
        archive.unpack(dest_dir)?;
    } else if name.ends_with(".zip") {
        let file = File::open(archive_path)?;
        let mut archive =
            zip::ZipArchive::new(file).map_err(|e| AppError::Internal(e.to_string()))?;
        for i in 0..archive.len() {
            let mut entry = archive
                .by_index(i)
                .map_err(|e| AppError::Internal(e.to_string()))?;
            let outpath = match entry.enclosed_name() {
                Some(p) => dest_dir.join(p),
                None => continue,
            };
            if entry.is_dir() {
                fs::create_dir_all(&outpath)?;
            } else {
                if let Some(p) = outpath.parent() {
                    fs::create_dir_all(p)?;
                }
                let mut outfile = File::create(&outpath)?;
                std::io::copy(&mut entry, &mut outfile)?;
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    if let Some(mode) = entry.unix_mode() {
                        let _ = fs::set_permissions(&outpath, fs::Permissions::from_mode(mode));
                    }
                }
            }
        }
    } else {
        return Err(AppError::InvalidArg("仅支持 .tar.gz / .zip 格式".into()));
    }
    Ok(())
}

fn flatten_single_subdir(dir: &Path) -> AppResult<()> {
    let entries: Vec<PathBuf> = fs::read_dir(dir)?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .collect();
    if entries.len() == 1 && entries[0].is_dir() {
        let inner = &entries[0];
        for item in fs::read_dir(inner)? {
            let item = item?;
            let dest = dir.join(item.file_name());
            fs::rename(item.path(), dest)?;
        }
        let _ = fs::remove_dir_all(inner);
    }
    Ok(())
}
