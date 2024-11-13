use log::info;
use std::path::{Path, PathBuf};

use etcetera::base_strategy::{choose_base_strategy, BaseStrategy};

const APP_NAME: &str = "scooter";

pub fn cache_dir() -> PathBuf {
    let strategy = choose_base_strategy().expect("Error when finding cache directory");
    let mut path = strategy.cache_dir();
    path.push(APP_NAME);
    path
}

pub fn default_log_file() -> PathBuf {
    cache_dir().join(format!("{APP_NAME}.log"))
}

fn make_parent_dir(path: &Path) {
    if let Some(parent) = path.parent() {
        if !parent.exists() {
            std::fs::create_dir_all(parent).ok();
        }
    }
}

pub fn setup_logging() -> anyhow::Result<()> {
    let log_path = default_log_file();
    make_parent_dir(&log_path);

    let _ = simple_log::file(log_path.to_str().unwrap(), "warn", 100, 10);

    info!("Logging initialized at {:?}", log_path);
    Ok(())
}
