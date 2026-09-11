use crate::diagnostics::Diagnostics;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

const SETTINGS_FILE_NAME: &str = "settings.json";

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowBounds {
    pub x: i32,
    pub y: i32,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowPosition {
    pub x: i32,
    pub y: i32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AppSettings {
    pub language: String,
    pub bubble_scale: f64,
    pub compact_mode_enabled: bool,
    pub expanded_bounds: Option<WindowBounds>,
    pub compact_position: Option<WindowPosition>,
    // Retained only so settings written by the hotfix preview remain readable.
    pub monitored_ports: Vec<u16>,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            language: "system".to_owned(),
            bubble_scale: 1.0,
            compact_mode_enabled: true,
            expanded_bounds: None,
            compact_position: None,
            monitored_ports: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsPatch {
    pub language: Option<String>,
    pub bubble_scale: Option<f64>,
    pub compact_mode_enabled: Option<bool>,
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
        if let Some(enabled) = patch.compact_mode_enabled {
            value.compact_mode_enabled = enabled;
        }
        *value = normalize_settings(value.clone());
        self.write_locked(&value)?;
        Ok(value.clone())
    }

    pub fn update_expanded_bounds(&self, bounds: WindowBounds) -> Result<(), String> {
        let mut value = self
            .value
            .lock()
            .map_err(|_| "Settings state lock is poisoned.".to_owned())?;
        let bounds = normalize_bounds(bounds);
        if value.expanded_bounds == Some(bounds) {
            return Ok(());
        }
        value.expanded_bounds = Some(bounds);
        self.write_locked(&value)
    }

    pub fn update_compact_position(&self, position: WindowPosition) -> Result<(), String> {
        let mut value = self
            .value
            .lock()
            .map_err(|_| "Settings state lock is poisoned.".to_owned())?;
        if value.compact_position == Some(position) {
            return Ok(());
        }
        value.compact_position = Some(position);
        self.write_locked(&value)
    }

    pub fn legacy_monitored_ports(&self) -> Result<Vec<u16>, String> {
        self.value
            .lock()
            .map(|value| value.monitored_ports.clone())
            .map_err(|_| "Settings state lock is poisoned.".to_owned())
    }

    pub fn clear_legacy_monitored_ports(&self) -> Result<(), String> {
        let mut value = self
            .value
            .lock()
            .map_err(|_| "Settings state lock is poisoned.".to_owned())?;
        if value.monitored_ports.is_empty() {
            return Ok(());
        }
        value.monitored_ports.clear();
        self.write_locked(&value)
    }

    fn write_locked(&self, value: &AppSettings) -> Result<(), String> {
        if let Err(error) = write_settings(&self.path, value) {
            self.diagnostics
                .record("ERROR", "settings_write", format!("error={error}"));
            return Err(error);
        }
        Ok(())
    }
}

fn normalize_bounds(mut bounds: WindowBounds) -> WindowBounds {
    if !bounds.width.is_finite() {
        bounds.width = 1020.0;
    }
    if !bounds.height.is_finite() {
        bounds.height = 760.0;
    }
    bounds.width = bounds.width.round().max(800.0);
    bounds.height = bounds.height.round().max(580.0);
    bounds
}

fn normalize_settings(mut value: AppSettings) -> AppSettings {
    if !matches!(value.language.as_str(), "system" | "en" | "ko") {
        value.language = "system".to_owned();
    }
    if !value.bubble_scale.is_finite() {
        value.bubble_scale = 1.0;
    }
    value.bubble_scale = (value.bubble_scale.clamp(0.7, 1.5) * 10.0).round() / 10.0;
    value.expanded_bounds = value.expanded_bounds.map(normalize_bounds);
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
    fn normalizes_language_bubble_scale_and_bounds() {
        let settings = normalize_settings(AppSettings {
            language: "xx".into(),
            bubble_scale: 1.26,
            compact_mode_enabled: true,
            expanded_bounds: Some(WindowBounds {
                x: 10,
                y: 20,
                width: 10.0,
                height: 900.4,
            }),
            compact_position: Some(WindowPosition { x: 30, y: 40 }),
            monitored_ports: vec![3101, 3000, 3101],
        });
        assert_eq!(settings.language, "system");
        assert_eq!(settings.bubble_scale, 1.3);
        assert_eq!(settings.monitored_ports, vec![3000, 3101]);
        assert_eq!(settings.expanded_bounds.unwrap().width, 800.0);
        assert_eq!(settings.expanded_bounds.unwrap().height, 900.0);
    }

    #[test]
    fn loads_legacy_settings_with_compact_enabled_by_default() {
        let settings: AppSettings =
            serde_json::from_str(r#"{"language":"ko","bubbleScale":1.2,"monitoredPorts":[3000]}"#)
                .unwrap();
        assert_eq!(settings.language, "ko");
        assert_eq!(settings.bubble_scale, 1.2);
        assert!(settings.compact_mode_enabled);
        assert_eq!(settings.monitored_ports, vec![3000]);
        assert_eq!(settings.expanded_bounds, None);
        assert_eq!(settings.compact_position, None);
    }

    #[test]
    fn persists_repeated_updates_and_window_state() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("port-lens-settings-{nonce}"));
        let log_dir = std::env::temp_dir().join(format!("port-lens-settings-log-{nonce}"));
        let diagnostics = Diagnostics::new(log_dir.clone()).unwrap();
        let store = SettingsStore::load(dir.clone(), diagnostics.clone()).unwrap();
        let first = store
            .update(SettingsPatch {
                language: Some("ko".into()),
                bubble_scale: Some(1.4),
                compact_mode_enabled: Some(false),
            })
            .unwrap();
        assert_eq!(first.language, "ko");
        assert_eq!(first.bubble_scale, 1.4);
        assert!(!first.compact_mode_enabled);

        store
            .update_expanded_bounds(WindowBounds {
                x: 100,
                y: 200,
                width: 1100.0,
                height: 800.0,
            })
            .unwrap();
        store
            .update_compact_position(WindowPosition { x: 1400, y: 700 })
            .unwrap();

        let reloaded = SettingsStore::load(dir.clone(), diagnostics).unwrap();
        let value = reloaded.get().unwrap();
        assert_eq!(value.expanded_bounds.unwrap().x, 100);
        assert_eq!(value.compact_position.unwrap().x, 1400);
        let _ = fs::remove_dir_all(dir);
        let _ = fs::remove_dir_all(log_dir);
    }
}
