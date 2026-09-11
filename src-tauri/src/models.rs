use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ListenerInfo {
    pub protocol: String,
    pub local_address: String,
    pub port: u16,
    pub pid: u32,
    pub process_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command_line: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ManagedApp {
    pub id: String,
    pub name: String,
    pub port: u16,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub command: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cwd: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_process_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_command_line: Option<String>,
}

impl ManagedApp {
    pub fn launch_config(&self) -> Option<(&str, &str)> {
        Some((self.command.as_deref()?, self.cwd.as_deref()?))
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ManagedRuntime {
    pub app_id: String,
    pub root_pid: u32,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ManagedExitInfo {
    pub app_id: String,
    pub app_name: String,
    pub root_pid: u32,
    pub exit_code: Option<i32>,
    pub elapsed_ms: u64,
    pub timestamp_ms: u64,
    pub early_exit: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn monitoring_only_app_has_no_launch_config() {
        let app = ManagedApp {
            id: "api".into(),
            name: "API".into(),
            port: 3101,
            command: None,
            cwd: None,
            last_process_name: Some("node".into()),
            last_command_line: None,
        };
        assert_eq!(app.launch_config(), None);
    }

    #[test]
    fn launch_config_requires_both_command_and_directory() {
        let mut app = ManagedApp {
            id: "api".into(),
            name: "API".into(),
            port: 3101,
            command: Some("npm run start".into()),
            cwd: None,
            last_process_name: None,
            last_command_line: None,
        };
        assert_eq!(app.launch_config(), None);
        app.cwd = Some("/tmp/api".into());
        assert_eq!(app.launch_config(), Some(("npm run start", "/tmp/api")));
    }
}
