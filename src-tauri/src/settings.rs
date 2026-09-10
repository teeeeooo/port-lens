use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

const SETTINGS_FILE_NAME: &str = "settings.json";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AppSettings {
    pub language: String,
    pub bubble_scale: f64,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            language: "system".to_owned(),
            bubble_scale: 1.0,
        }
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsPatch {
    pub language: Option<String>,
    pub bubble_scale: Option<f64>,
}
pub struct SettingsStore {
    path: PathBuf,
    value: Mutex<AppSettings>,
}

impl SettingsStore {
    pub fn load(config_dir: PathBuf) -> Result<Self, String> {
        fs::create_dir_all(&config_dir)
            .map_err(|error| format!("Failed to create Port Lens config directory: {error}"))?;
        let path = config_dir.join(SETTINGS_FILE_NAME);
        let value = read_settings(&path).unwrap_or_default();
        Ok(Self {
            path,
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
        *value = normalize_settings(value.clone());
        write_settings(&self.path, &value)?;
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
    value
}

fn read_settings(path: &Path) -> Option<AppSettings> {
    let text = fs::read_to_string(path).ok()?;
    serde_json::from_str(&text).ok()
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
        });
        assert_eq!(settings.language, "system");
        assert_eq!(settings.bubble_scale, 1.3);

        let settings = normalize_settings(AppSettings {
            language: "ko".into(),
            bubble_scale: 4.0,
        });
        assert_eq!(settings.language, "ko");
        assert_eq!(settings.bubble_scale, 1.5);
    }

    #[test]
    fn persists_repeated_updates() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();

        let dir = std::env::temp_dir().join(format!("port-lens-settings-{nonce}"));
        let store = SettingsStore::load(dir.clone()).expect("settings store should load");
        let first = store
            .update(SettingsPatch {
                language: Some("ko".into()),
                bubble_scale: Some(1.4),
            })
            .expect("first settings update should persist");
        assert_eq!(first.language, "ko");
        assert_eq!(first.bubble_scale, 1.4);

        let second = store
            .update(SettingsPatch {
                language: Some("en".into()),
                bubble_scale: Some(0.8),
            })
            .expect("second settings update should persist");
        assert_eq!(second.language, "en");
        assert_eq!(second.bubble_scale, 0.8);

        let reloaded = SettingsStore::load(dir.clone()).expect("settings store should reload");
        assert_eq!(reloaded.get().unwrap(), second);
        let _ = fs::remove_dir_all(dir);
    }
}
