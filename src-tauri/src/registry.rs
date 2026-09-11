use crate::diagnostics::Diagnostics;
use crate::models::{ManagedApp, ManagedExitInfo};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::io::ErrorKind;
use std::path::PathBuf;
use std::sync::Mutex;

pub struct AppState {
    config_path: PathBuf,
    diagnostics: Diagnostics,
    pub apps: Mutex<Vec<ManagedApp>>,
    pub runtime_pids: Mutex<HashMap<String, u32>>,
    pub expected_exit_pids: Mutex<HashSet<u32>>,
    pub last_exits: Mutex<HashMap<String, ManagedExitInfo>>,
}

impl AppState {
    pub fn load(config_path: PathBuf, diagnostics: Diagnostics) -> Self {
        let apps = match fs::read_to_string(&config_path) {
            Ok(text) => match serde_json::from_str::<Vec<ManagedApp>>(&text) {
                Ok(apps) => apps,
                Err(error) => {
                    diagnostics.record("ERROR", "registry_read", format!("parse error={error}"));
                    Vec::new()
                }
            },
            Err(error) if error.kind() == ErrorKind::NotFound => Vec::new(),
            Err(error) => {
                diagnostics.record("ERROR", "registry_read", format!("read error={error}"));
                Vec::new()
            }
        };

        Self {
            config_path,
            diagnostics,
            apps: Mutex::new(apps),
            runtime_pids: Mutex::new(HashMap::new()),
            expected_exit_pids: Mutex::new(HashSet::new()),
            last_exits: Mutex::new(HashMap::new()),
        }
    }

    pub fn persist_apps(&self, apps: &[ManagedApp]) -> Result<(), String> {
        let result = (|| {
            if let Some(parent) = self.config_path.parent() {
                fs::create_dir_all(parent)
                    .map_err(|error| format!("Failed to create config directory: {error}"))?;
            }
            let json = serde_json::to_string_pretty(apps)
                .map_err(|error| format!("Failed to serialize app registry: {error}"))?;
            fs::write(&self.config_path, json)
                .map_err(|error| format!("Failed to save app registry: {error}"))
        })();
        if let Err(error) = &result {
            self.diagnostics
                .record("ERROR", "registry_write", format!("error={error}"));
        }
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn persists_and_loads_managed_apps() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("port-lens-registry-{nonce}.json"));
        let log_dir = std::env::temp_dir().join(format!("port-lens-registry-log-{nonce}"));
        let diagnostics = Diagnostics::new(log_dir.clone()).unwrap();
        let state = AppState::load(path.clone(), diagnostics.clone());
        let app = ManagedApp {
            id: "api".into(),
            name: "API".into(),
            port: 3101,
            command: Some("npm run dev".into()),
            cwd: Some("/tmp".into()),
            last_process_name: Some("node".into()),
            last_command_line: None,
        };

        state.persist_apps(std::slice::from_ref(&app)).unwrap();
        let loaded = AppState::load(path.clone(), diagnostics);
        assert_eq!(*loaded.apps.lock().unwrap(), vec![app]);
        let _ = fs::remove_file(path);
        let _ = fs::remove_dir_all(log_dir);
    }
}
