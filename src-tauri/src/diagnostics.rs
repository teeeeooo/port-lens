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
const MAX_MANAGED_LOG_BYTES: u64 = 5 * 1024 * 1024;

#[derive(Debug, Clone)]
pub struct ManagedLogPaths {
    pub directory: PathBuf,
    pub stdout: PathBuf,
    pub stderr: PathBuf,
}

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

    pub fn prepare_managed_logs(
        &self,
        app_id: &str,
        app_name: &str,
    ) -> Result<ManagedLogPaths, String> {
        let paths = self.managed_log_paths(app_id);
        fs::create_dir_all(&paths.directory)
            .map_err(|error| format!("Failed to create App log directory: {error}"))?;
        rotate_if_needed_with_limit(&paths.stdout, MAX_MANAGED_LOG_BYTES)
            .map_err(|error| format!("Failed to rotate App stdout log: {error}"))?;
        rotate_if_needed_with_limit(&paths.stderr, MAX_MANAGED_LOG_BYTES)
            .map_err(|error| format!("Failed to rotate App stderr log: {error}"))?;
        let timestamp = now_millis();
        let header = format!(
            "\n=== Port Lens run {timestamp} · {} ===\n",
            sanitize_line(app_name)
        );
        append_text(&paths.stdout, &header)
            .map_err(|error| format!("Failed to initialize App stdout log: {error}"))?;
        append_text(&paths.stderr, &header)
            .map_err(|error| format!("Failed to initialize App stderr log: {error}"))?;
        Ok(paths)
    }

    pub fn open_managed_log_folder(&self, app_id: &str) -> Result<(), String> {
        let paths = self.managed_log_paths(app_id);
        fs::create_dir_all(&paths.directory)
            .map_err(|error| format!("Failed to create App log directory: {error}"))?;
        open_folder(&paths.directory)
    }

    fn managed_log_paths(&self, app_id: &str) -> ManagedLogPaths {
        let directory = self
            .log_dir
            .join("managed-apps")
            .join(sanitize_path_component(app_id));
        ManagedLogPaths {
            stdout: directory.join("stdout.log"),
            stderr: directory.join("stderr.log"),
            directory,
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

fn append_text(path: &Path, text: &str) -> std::io::Result<()> {
    let mut file = OpenOptions::new().create(true).append(true).open(path)?;
    file.write_all(text.as_bytes())
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

fn sanitize_path_component(value: &str) -> String {
    let mut sanitized = value
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_') {
                ch
            } else {
                '_'
            }
        })
        .collect::<String>();
    sanitized.truncate(80);
    if sanitized.is_empty() {
        "app".to_owned()
    } else {
        sanitized
    }
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
    fn managed_log_paths_are_sanitized_and_initialized() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("port-lens-output-{nonce}"));
        let diagnostics = Diagnostics::new(root.clone()).unwrap();
        let paths = diagnostics
            .prepare_managed_logs("../bad/app:id", "API\nServer")
            .unwrap();

        assert!(paths.directory.starts_with(root.join("managed-apps")));
        assert_eq!(paths.directory.file_name().unwrap(), "___bad_app_id");
        assert!(fs::read_to_string(&paths.stdout)
            .unwrap()
            .contains("API Server"));
        assert!(fs::read_to_string(&paths.stderr)
            .unwrap()
            .contains("API Server"));
        let _ = fs::remove_dir_all(root);
    }
}
