use crate::models::ManagedApp;
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Mutex;

pub struct AppState {
    config_path: PathBuf,
    pub apps: Mutex<Vec<ManagedApp>>,
    pub runtime_pids: Mutex<HashMap<String, u32>>,
}

impl AppState {
    pub fn load(config_path: PathBuf) -> Self {
        let apps = fs::read_to_string(&config_path)
            .ok()
            .and_then(|text| serde_json::from_str::<Vec<ManagedApp>>(&text).ok())
            .unwrap_or_default();

        Self {
            config_path,
            apps: Mutex::new(apps),
            runtime_pids: Mutex::new(HashMap::new()),
        }
    }

    pub fn persist_apps(&self, apps: &[ManagedApp]) -> Result<(), String> {
        if let Some(parent) = self.config_path.parent() {
            fs::create_dir_all(parent)
                .map_err(|error| format!("Failed to create config directory: {error}"))?;
        }
        let json = serde_json::to_string_pretty(apps)
            .map_err(|error| format!("Failed to serialize app registry: {error}"))?;
        fs::write(&self.config_path, json)
            .map_err(|error| format!("Failed to save app registry: {error}"))
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
        let state = AppState::load(path.clone());
        let app = ManagedApp {
            id: "api".into(),
            name: "API".into(),
            port: 3101,
            command: "npm run dev".into(),
            cwd: "/tmp".into(),
        };

        state.persist_apps(std::slice::from_ref(&app)).unwrap();
        let loaded = AppState::load(path.clone());
        assert_eq!(*loaded.apps.lock().unwrap(), vec![app]);
        let _ = fs::remove_file(path);
    }
}
