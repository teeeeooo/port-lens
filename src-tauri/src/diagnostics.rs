use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;
const MAX_LOG_BYTES: u64 = 1024 * 1024;

#[derive(Clone)]
pub struct Diagnostics {
    log_dir: PathBuf,
    log_file: PathBuf,
    lock: Arc<Mutex<()>>,
}

impl Diagnostics {
    pub fn new(log_dir: PathBuf) -> Result<Self, String> {
        fs::create_dir_all(&log_dir)
            .map_err(|error| format!("Failed to create Port Lens log directory: {error}"))?;
        Ok(Self {
            log_file: log_dir.join("port-lens.log"),
            log_dir,
            lock: Arc::new(Mutex::new(())),
        })
    }
    pub fn record(&self, level: &str, event: &str, message: impl AsRef<str>) {
        let Ok(_guard) = self.lock.lock() else {
            return;
        };
        let _ = rotate_if_needed(&self.log_file);
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|value| value.as_millis())
            .unwrap_or_default();
        if let Ok(mut file) = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.log_file)
        {
            let sanitized = message.as_ref().replace(['\r', '\n'], " ");
            let _ = writeln!(file, "{timestamp}\t{level}\t{event}\t{sanitized}");
        }
    }

    pub fn install_panic_hook(&self) {
        let diagnostics = self.clone();
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |info| {
            diagnostics.record("ERROR", "panic", panic_message(info));
            previous(info);
        }));
    }

    pub fn open_log_folder(&self) -> Result<(), String> {
        open_folder(&self.log_dir)
    }
}

fn rotate_if_needed(path: &Path) -> std::io::Result<()> {
    if fs::metadata(path)
        .map(|meta| meta.len())
        .unwrap_or_default()
        < MAX_LOG_BYTES
    {
        return Ok(());
    }
    let rotated = path.with_extension("log.1");
    let _ = fs::remove_file(&rotated);
    fs::rename(path, rotated)
}
fn panic_message(info: &std::panic::PanicHookInfo<'_>) -> String {
    let payload = if let Some(value) = info.payload().downcast_ref::<&str>() {
        (*value).to_string()
    } else if let Some(value) = info.payload().downcast_ref::<String>() {
        value.clone()
    } else {
        "non-string panic payload".to_string()
    };
    match info.location() {
        Some(location) => format!(
            "{}:{}:{} {payload}",
            location.file(),
            location.line(),
            location.column()
        ),
        None => payload,
    }
}

#[cfg(windows)]
fn open_folder(path: &Path) -> Result<(), String> {
    Command::new("explorer.exe")
        .arg(path)
        .creation_flags(CREATE_NO_WINDOW)
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("Failed to open log folder: {error}"))
}

#[cfg(target_os = "macos")]
fn open_folder(path: &Path) -> Result<(), String> {
    Command::new("open")
        .arg(path)
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("Failed to open log folder: {error}"))
}

#[cfg(not(any(windows, target_os = "macos")))]
fn open_folder(_path: &Path) -> Result<(), String> {
    Err("Opening the log folder is not supported on this platform.".into())
}
