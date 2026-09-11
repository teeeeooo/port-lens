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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_managed_pid: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_managed_process_name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_managed_command_line: Option<String>,
}

impl ManagedApp {
    pub fn launch_config(&self) -> Option<(&str, &str)> {
        Some((self.command.as_deref()?, self.cwd.as_deref()?))
    }

    pub fn observe_listener(&mut self, listener: &ListenerInfo, managed: bool) -> bool {
        let mut changed = false;
        if self.last_process_name.is_none() {
            self.last_process_name = Some(listener.process_name.clone());
            changed = true;
        }
        if self.last_command_line.is_none() && listener.command_line.is_some() {
            self.last_command_line = listener.command_line.clone();
            changed = true;
        }
        if managed {
            if self.last_managed_pid != Some(listener.pid) {
                self.last_managed_pid = Some(listener.pid);
                changed = true;
            }
            if self.last_managed_process_name.as_deref() != Some(listener.process_name.as_str()) {
                self.last_managed_process_name = Some(listener.process_name.clone());
                changed = true;
            }
            if let Some(command_line) = &listener.command_line {
                if self.last_managed_command_line.as_deref() != Some(command_line.as_str()) {
                    self.last_managed_command_line = Some(command_line.clone());
                    changed = true;
                }
            }
        }
        changed
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
            last_managed_pid: None,
            last_managed_process_name: None,
            last_managed_command_line: None,
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
            last_managed_pid: None,
            last_managed_process_name: None,
            last_managed_command_line: None,
        };
        assert_eq!(app.launch_config(), None);
        app.cwd = Some("/tmp/api".into());
        assert_eq!(app.launch_config(), Some(("npm run start", "/tmp/api")));
    }
    #[test]
    fn managed_listener_observation_persists_relaunch_identity() {
        let mut app = ManagedApp {
            id: "api".into(),
            name: "API".into(),
            port: 3101,
            command: Some("node server.js".into()),
            cwd: Some("/tmp/api".into()),
            last_process_name: Some("node".into()),
            last_command_line: Some("node old.js".into()),
            last_managed_pid: None,
            last_managed_process_name: None,
            last_managed_command_line: None,
        };
        let listener = ListenerInfo {
            protocol: "TCP".into(),
            local_address: "0.0.0.0".into(),
            port: 3101,
            pid: 4242,
            process_name: "node".into(),
            command_line: Some("node server.js".into()),
        };

        assert!(app.observe_listener(&listener, true));
        assert_eq!(app.last_managed_pid, Some(4242));
        assert_eq!(app.last_managed_process_name.as_deref(), Some("node"));
        assert_eq!(
            app.last_managed_command_line.as_deref(),
            Some("node server.js")
        );
        assert!(!app.observe_listener(&listener, true));
    }
}
