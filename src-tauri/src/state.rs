use std::path::PathBuf;
use std::sync::Arc;

use crate::db::Database;
use crate::logcat::LogcatManager;
use crate::mdns::MdnsWatcher;
use crate::scrcpy::ScrcpyManager;
use crate::shell::ShellManager;

pub struct AppState {
    pub app_data_dir: PathBuf,
    pub db: Arc<Database>,
    pub mdns: MdnsWatcher,
    pub shell: ShellManager,
    pub logcat: LogcatManager,
    pub scrcpy: ScrcpyManager,
}
