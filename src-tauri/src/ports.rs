use crate::models::ListenerInfo;
use std::collections::HashSet;
use std::process::Command;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

pub fn list_listeners() -> Result<Vec<ListenerInfo>, String> {
    #[cfg(windows)]
    {
        return list_windows_listeners();
    }
    #[cfg(target_os = "macos")]
    {
        return list_macos_listeners();
    }
    #[allow(unreachable_code)]
    Err("Port Lens currently supports listener discovery on Windows and macOS.".into())
}

#[cfg(windows)]
fn list_windows_listeners() -> Result<Vec<ListenerInfo>, String> {
    let script = r#"
$connections = @(Get-NetTCPConnection -State Listen -ErrorAction SilentlyContinue)
$names = @{}
foreach ($processId in @($connections | Select-Object -ExpandProperty OwningProcess -Unique)) {
  try { $names[[uint32]$processId] = (Get-Process -Id $processId -ErrorAction Stop).ProcessName }
  catch { $names[[uint32]$processId] = 'Unknown' }
}
$items = @($connections | ForEach-Object {
  [pscustomobject]@{
    protocol = 'TCP'
    localAddress = $_.LocalAddress
    port = [uint16]$_.LocalPort
    pid = [uint32]$_.OwningProcess
    processName = $names[[uint32]$_.OwningProcess]
  }
})
ConvertTo-Json -InputObject $items -Compress
"#;

    let output = Command::new("powershell.exe")
        .args(["-NoProfile", "-NonInteractive", "-Command", script])
        .creation_flags(CREATE_NO_WINDOW)
        .output()
        .map_err(|error| format!("Failed to run PowerShell: {error}"))?;

    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }

    parse_windows_json(&String::from_utf8_lossy(&output.stdout))
}

#[cfg(any(windows, test))]
fn parse_windows_json(input: &str) -> Result<Vec<ListenerInfo>, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() || trimmed == "null" {
        return Ok(Vec::new());
    }
    let mut listeners: Vec<ListenerInfo> =
        serde_json::from_str(trimmed).map_err(|error| format!("Invalid listener JSON: {error}"))?;
    normalize(&mut listeners);
    Ok(listeners)
}

#[cfg(target_os = "macos")]
fn list_macos_listeners() -> Result<Vec<ListenerInfo>, String> {
    let output = Command::new("lsof")
        .args(["-nP", "-iTCP", "-sTCP:LISTEN", "-Fpcn"])
        .output()
        .map_err(|error| format!("Failed to run lsof: {error}"))?;

    if !output.status.success() && output.stdout.is_empty() {
        return Err(String::from_utf8_lossy(&output.stderr).trim().to_string());
    }
    Ok(parse_lsof(&String::from_utf8_lossy(&output.stdout)))
}

#[cfg(any(target_os = "macos", test))]
fn parse_lsof(input: &str) -> Vec<ListenerInfo> {
    let mut listeners = Vec::new();
    let mut pid = 0_u32;
    let mut process_name = String::from("Unknown");

    for line in input.lines() {
        let Some(kind) = line.chars().next() else {
            continue;
        };
        let value = &line[1..];
        match kind {
            'p' => pid = value.parse().unwrap_or(0),
            'c' => process_name = value.to_string(),
            'n' if pid > 0 => {
                if let Some((address, port_text)) = value.rsplit_once(':') {
                    if let Ok(port) = port_text.parse::<u16>() {
                        listeners.push(ListenerInfo {
                            protocol: "TCP".into(),
                            local_address: address.to_string(),
                            port,
                            pid,
                            process_name: process_name.clone(),
                        });
                    }
                }
            }
            _ => {}
        }
    }

    normalize(&mut listeners);
    listeners
}

fn normalize(listeners: &mut Vec<ListenerInfo>) {
    let mut seen = HashSet::new();
    listeners.retain(|item| {
        seen.insert((
            item.protocol.clone(),
            item.local_address.clone(),
            item.port,
            item.pid,
        ))
    });
    listeners.sort_by_key(|item| (item.port, item.pid, item.local_address.clone()));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_windows_listener_json() {
        let json = r#"[{"protocol":"TCP","localAddress":"0.0.0.0","port":3000,"pid":42,"processName":"node"}]"#;
        let result = parse_windows_json(json).unwrap();
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].port, 3000);
        assert_eq!(result[0].process_name, "node");
    }

    #[test]
    fn parses_lsof_listener_records() {
        let text = "p42\ncnode\nn*:3000\nn127.0.0.1:3101\np9\ncpython3\nn[::1]:8000\n";
        let result = parse_lsof(text);
        assert_eq!(result.len(), 3);
        assert_eq!(result[0].port, 3000);
        assert_eq!(result[2].process_name, "python3");
    }
}
