use std::path::{Path, PathBuf};

use etcetera::base_strategy::{choose_base_strategy, BaseStrategy};

pub fn _cache_dir() -> PathBuf {
    let strategy = choose_base_strategy().expect("Error when finding cache directory");
    let mut path = strategy.cache_dir();
    path.push("scout");
    path
}

pub fn _default_log_file() -> PathBuf {
    _cache_dir().join("scout.log")
}

fn _make_parent_dir(path: &Path) {
    if let Some(parent) = path.parent() {
        if !parent.exists() {
            std::fs::create_dir_all(parent).ok();
        }
    }
}
