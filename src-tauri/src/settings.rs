use crate::diagnostics::Diagnostics;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

const SETTINGS_FILE_NAME: &str = "settings.json";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AppSettings {
    pub language: String,
    pub bubble_scale: f64,
    pub monitored_ports: Vec<u16>,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            language: "system".to_owned(),
            bubble_scale: 1.0,
            monitored_ports: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsPatch {
    pub language: Option<String>,
    pub bubble_scale: Option<f64>,
    pub monitored_ports: Option<Vec<u16>>,
}
pub struct SettingsStore {
    path: PathBuf,
    diagnostics: Diagnostics,
    value: Mutex<AppSettings>,
}

impl SettingsStore {
    pub fn load(config_dir: PathBuf, diagnostics: Diagnostics) -> Result<Self, String> {
        if let Err(error) = fs::create_dir_all(&config_dir) {
            let message = format!("Failed to create Port Lens config directory: {error}");
            diagnostics.record("ERROR", "settings_write", &message);
            return Err(message);
        }
        let path = config_dir.join(SETTINGS_FILE_NAME);
        let value = match read_settings(&path) {
            Ok(Some(value)) => value,
            Ok(None) => AppSettings::default(),
            Err(error) => {
                diagnostics.record("ERROR", "settings_read", format!("error={error}"));
                AppSettings::default()
            }
        };
        Ok(Self {
            path,
            diagnostics,
            value: Mutex::new(normalize_settings(value)),
        })
    }

    pub fn get(&self) -> Result<AppSettings, String> {
        self.value
            .lock()
            .map(|value| value.clone())
            .map_err(|_| "Settings state lock is poisoned.".to_owned())
    }

    pub fn update(&self, patch: SettingsPatch) -> Result<AppSettings, String> {
        let mut value = self
            .value
            .lock()
            .map_err(|_| "Settings state lock is poisoned.".to_owned())?;
        if let Some(language) = patch.language {
            value.language = language;
        }
        if let Some(scale) = patch.bubble_scale {
            value.bubble_scale = scale;
        }
        if let Some(monitored_ports) = patch.monitored_ports {
            value.monitored_ports = monitored_ports;
        }
        *value = normalize_settings(value.clone());
        if let Err(error) = write_settings(&self.path, &value) {
            self.diagnostics
                .record("ERROR", "settings_write", format!("error={error}"));
            return Err(error);
        }
        Ok(value.clone())
    }
}

fn normalize_settings(mut value: AppSettings) -> AppSettings {
    if !matches!(value.language.as_str(), "system" | "en" | "ko") {
        value.language = "system".to_owned();
    }
    if !value.bubble_scale.is_finite() {
        value.bubble_scale = 1.0;
    }
    value.bubble_scale = (value.bubble_scale.clamp(0.7, 1.5) * 10.0).round() / 10.0;
    value.monitored_ports.sort_unstable();
    value.monitored_ports.dedup();
    value
}

fn read_settings(path: &Path) -> Result<Option<AppSettings>, String> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("Failed to read Port Lens settings: {error}")),
    };
    serde_json::from_str(&text)
        .map(Some)
        .map_err(|error| format!("Failed to parse Port Lens settings: {error}"))
}

fn write_settings(path: &Path, value: &AppSettings) -> Result<(), String> {
    let text = serde_json::to_string_pretty(value)
        .map_err(|error| format!("Failed to serialize Port Lens settings: {error}"))?;
    fs::write(path, format!("{text}\n"))
        .map_err(|error| format!("Failed to save Port Lens settings: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn normalizes_language_and_bubble_scale() {
        let settings = normalize_settings(AppSettings {
            language: "xx".into(),
            bubble_scale: 1.26,
            monitored_ports: vec![3101, 3000, 3101],
        });
        assert_eq!(settings.language, "system");
        assert_eq!(settings.bubble_scale, 1.3);
        assert_eq!(settings.monitored_ports, vec![3000, 3101]);

        let settings = normalize_settings(AppSettings {
            language: "ko".into(),
            bubble_scale: 4.0,
            monitored_ports: vec![],
        });
        assert_eq!(settings.language, "ko");
        assert_eq!(settings.bubble_scale, 1.5);
    }

    #[test]
    fn loads_legacy_settings_without_monitored_ports() {
        let settings: AppSettings =
            serde_json::from_str(r#"{"language":"ko","bubbleScale":1.2}"#).unwrap();
        assert_eq!(settings.language, "ko");
        assert_eq!(settings.bubble_scale, 1.2);
        assert!(settings.monitored_ports.is_empty());
    }

    #[test]
    fn persists_repeated_updates() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();

        let dir = std::env::temp_dir().join(format!("port-lens-settings-{nonce}"));
        let log_dir = std::env::temp_dir().join(format!("port-lens-settings-log-{nonce}"));
        let diagnostics = Diagnostics::new(log_dir.clone()).unwrap();
        let store = SettingsStore::load(dir.clone(), diagnostics.clone())
            .expect("settings store should load");
        let first = store
            .update(SettingsPatch {
                language: Some("ko".into()),
                bubble_scale: Some(1.4),
                monitored_ports: Some(vec![3101, 3000]),
            })
            .expect("first settings update should persist");
        assert_eq!(first.language, "ko");
        assert_eq!(first.bubble_scale, 1.4);
        assert_eq!(first.monitored_ports, vec![3000, 3101]);

        let second = store
            .update(SettingsPatch {
                language: Some("en".into()),
                bubble_scale: Some(0.8),
                monitored_ports: None,
            })
            .expect("second settings update should persist");
        assert_eq!(second.language, "en");
        assert_eq!(second.bubble_scale, 0.8);

        let reloaded =
            SettingsStore::load(dir.clone(), diagnostics).expect("settings store should reload");
        assert_eq!(reloaded.get().unwrap(), second);
        let _ = fs::remove_dir_all(dir);
        let _ = fs::remove_dir_all(log_dir);
    }
}
