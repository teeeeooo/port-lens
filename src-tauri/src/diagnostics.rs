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
        let timestamp = now_millis();
        if let Ok(mut file) = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.log_file)
        {
            let sanitized = sanitize_line(message.as_ref());
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

    pub fn managed_action_requested(&self, action: &str, app_id: &str) {
        self.record(
            "INFO",
            "managed_action",
            format!("action={action} appId={app_id} phase=requested"),
        );
    }

    pub fn managed_action_result<T>(&self, action: &str, app_id: &str, result: &Result<T, String>) {
        match result {
            Ok(_) => self.record(
                "INFO",
                "managed_action",
                format!("action={action} appId={app_id} phase=returned result=ok"),
            ),
            Err(error) => self.record(
                "ERROR",
                "managed_action",
                format!("action={action} appId={app_id} phase=returned result=error error={error}"),
            ),
        }
    }
}

fn rotate_if_needed(path: &Path) -> std::io::Result<()> {
    rotate_if_needed_with_limit(path, MAX_LOG_BYTES)
}

fn rotate_if_needed_with_limit(path: &Path, max_bytes: u64) -> std::io::Result<()> {
    if fs::metadata(path)
        .map(|meta| meta.len())
        .unwrap_or_default()
        < max_bytes
    {
        return Ok(());
    }
    let rotated = path.with_extension("log.1");
    let _ = fs::remove_file(&rotated);
    fs::rename(path, rotated)
}

fn now_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_millis())
        .unwrap_or_default()
}

fn sanitize_line(value: &str) -> String {
    value.replace(['\r', '\n'], " ")
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn lifecycle_requests_and_results_use_only_rotating_diagnostic_log() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("port-lens-actions-{nonce}"));
        let diagnostics = Diagnostics::new(root.clone()).unwrap();
        let file = root.join("port-lens.log");
        fs::write(&file, vec![b'x'; MAX_LOG_BYTES as usize]).unwrap();
        diagnostics.managed_action_requested("start", "api");
        diagnostics.managed_action_result("start", "api", &Ok::<_, String>(()));
        diagnostics.managed_action_requested("stop", "api");
        diagnostics.managed_action_result::<()>(
            "stop",
            "api",
            &Err("access denied\ninjected".into()),
        );
        let text = fs::read_to_string(&file).unwrap();
        assert!(text.contains("action=start appId=api phase=requested"));
        assert!(text.contains("action=start appId=api phase=returned result=ok"));
        assert!(text.contains("action=stop appId=api phase=requested"));
        assert!(text.contains("result=error error=access denied injected"));
        assert_eq!(text.lines().count(), 4);
        assert!(root.join("port-lens.log.1").exists());
        assert_eq!(fs::read_dir(&root).unwrap().count(), 2);
        fs::remove_dir_all(root).unwrap();
    }
}
