use std::{
    cmp::Ordering,
    fs::{self, File},
    io::Write,
    path::{Path, PathBuf},
};

use etcetera::base_strategy::{choose_base_strategy, BaseStrategy};

pub fn cache_dir() -> PathBuf {
    let strategy = choose_base_strategy().expect("Error when finding cache directory");
    let mut path = strategy.cache_dir();
    path.push("scout");
    path
}

pub fn default_log_file() -> PathBuf {
    cache_dir().join("scout.log")
}

fn make_parent_dir(path: &Path) {
    if let Some(parent) = path.parent() {
        if !parent.exists() {
            std::fs::create_dir_all(parent).ok();
        }
    }
}

pub struct Log {
    file: File,
    level: LogLevel,
}

#[derive(Debug, PartialEq, Eq, Copy, Clone)]
pub enum LogLevel {
    Info = 1,
    Warn = 2,
    Error = 3,
}

impl PartialOrd for LogLevel {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for LogLevel {
    fn cmp(&self, other: &Self) -> Ordering {
        (*self as i32).cmp(&(*other as i32))
    }
}

impl Log {
    pub fn new(level: LogLevel) -> Log {
        let path = default_log_file();
        make_parent_dir(&path);
        let file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .expect("Error when opening log file");
        Log { file, level }
    }

    pub fn info(&mut self, line: &str) {
        if self.level >= LogLevel::Info {
            self.file
                .write_all(["Info:", line, "\n"].join(" ").as_bytes())
                .expect("Failed to write log line");
        }
    }
}
