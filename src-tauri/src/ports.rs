use crate::models::ListenerInfo;
#[cfg(any(target_os = "macos", test))]
use std::collections::HashMap;
use std::collections::HashSet;
use std::process::Command;

#[cfg(any(windows, target_os = "macos"))]
use std::io::Read;
#[cfg(windows)]
use std::os::windows::process::CommandExt;
#[cfg(any(windows, target_os = "macos"))]
use std::process::Stdio;
#[cfg(any(windows, target_os = "macos"))]
use std::thread;
#[cfg(any(windows, target_os = "macos"))]
use std::time::{Duration, Instant};

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;
#[cfg(any(windows, target_os = "macos"))]
const SCAN_TIMEOUT: Duration = Duration::from_secs(8);

pub struct ListenerScan {
    pub listeners: Vec<ListenerInfo>,
    pub warning: Option<String>,
}

pub fn list_inventory() -> Result<ListenerScan, String> {
    list_platform(None, false)
}

pub fn list_monitored(ports: &[u16]) -> Result<ListenerScan, String> {
    if ports.is_empty() {
        return Ok(ListenerScan {
            listeners: Vec::new(),
            warning: None,
        });
    }
    list_platform(Some(ports), true)
}

fn list_platform(ports: Option<&[u16]>, enrich_command_line: bool) -> Result<ListenerScan, String> {
    #[cfg(windows)]
    {
        return list_windows(ports, enrich_command_line);
    }
    #[cfg(target_os = "macos")]
    {
        return list_macos(ports, enrich_command_line);
    }
    #[allow(unreachable_code)]
    Err("Port Lens currently supports listener discovery on Windows and macOS.".into())
}

#[cfg(windows)]
fn list_windows(ports: Option<&[u16]>, enrich_command_line: bool) -> Result<ListenerScan, String> {
    let script = build_windows_script(ports, enrich_command_line);
    let mut command = Command::new("powershell.exe");
    command
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .creation_flags(CREATE_NO_WINDOW);
    let (status, stdout, stderr) =
        run_command_with_timeout(command, "PowerShell listener scan", SCAN_TIMEOUT)?;
    if !status.success() {
        return Err(command_failure("PowerShell listener scan", status, &stderr));
    }
    Ok(ListenerScan {
        listeners: parse_windows_json(&stdout)?,
        warning: nonempty_warning(stderr),
    })
}

#[cfg(any(windows, test))]
fn build_windows_script(ports: Option<&[u16]>, enrich_command_line: bool) -> String {
    let port_filter = ports
        .map(|values| {
            values
                .iter()
                .map(u16::to_string)
                .collect::<Vec<_>>()
                .join(",")
        })
        .filter(|value| !value.is_empty());
    let connection_command = match port_filter {
        Some(values) => format!(
            "$wantedPorts = @({values}); $connections = @(Get-NetTCPConnection -State Listen -LocalPort $wantedPorts -ErrorAction SilentlyContinue)"
        ),
        None => "$connections = @(Get-NetTCPConnection -State Listen -ErrorAction SilentlyContinue)".to_string(),
    };
    let command_line_block = if enrich_command_line {
        r#"
$commands = @{}
$cimFailures = @()
foreach ($processId in $processIds) {
  try {
    $cim = Get-CimInstance Win32_Process -Filter "ProcessId = $processId" -ErrorAction Stop
    if ($null -ne $cim -and $null -ne $cim.CommandLine) { $commands[[uint32]$processId] = [string]$cim.CommandLine }
  } catch {
    $cimFailures += "pid=$processId $($_.Exception.Message)"
  }
}
if ($cimFailures.Count -gt 0) { [Console]::Error.WriteLine("CIM " + ($cimFailures -join '; ')) }
"#
    } else {
        "$commands = @{}\n"
    };
    format!(
        r#"
{connection_command}
$processIds = @($connections | Select-Object -ExpandProperty OwningProcess -Unique)
$names = @{{}}
if ($processIds.Count -gt 0) {{
  Get-Process -Id $processIds -ErrorAction SilentlyContinue | ForEach-Object {{ $names[[uint32]$_.Id] = $_.ProcessName }}
}}
{command_line_block}
$items = @($connections | ForEach-Object {{
  $pidValue = [uint32]$_.OwningProcess
  $name = $names[$pidValue]
  if ([string]::IsNullOrWhiteSpace($name)) {{ $name = 'Unknown' }}
  [pscustomobject]@{{
    protocol = 'TCP'
    localAddress = $_.LocalAddress
    port = [uint16]$_.LocalPort
    pid = $pidValue
    processName = $name
    commandLine = $commands[$pidValue]
  }}
}})
ConvertTo-Json -InputObject $items -Compress
"#
    )
}

#[cfg(any(windows, target_os = "macos"))]
fn run_command_with_timeout(
    mut command: Command,
    label: &str,
    timeout: Duration,
) -> Result<(std::process::ExitStatus, String, String), String> {
    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("Failed to run {label}: {error}"))?;
    let stdout = child.stdout.take().map(spawn_pipe_reader);
    let stderr = child.stderr.take().map(spawn_pipe_reader);
    let started = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if started.elapsed() < timeout => thread::sleep(Duration::from_millis(25)),
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("{label} timed out after {}s", timeout.as_secs()));
            }
            Err(error) => return Err(format!("Failed while waiting for {label}: {error}")),
        }
    };
    let stdout = stdout
        .and_then(|reader| reader.join().ok())
        .unwrap_or_default();
    let stderr = stderr
        .and_then(|reader| reader.join().ok())
        .unwrap_or_default();
    Ok((status, stdout, stderr))
}

#[cfg(any(windows, target_os = "macos"))]
fn spawn_pipe_reader(mut pipe: impl Read + Send + 'static) -> thread::JoinHandle<String> {
    thread::spawn(move || {
        let mut value = String::new();
        let _ = pipe.read_to_string(&mut value);
        value
    })
}

#[cfg(any(windows, target_os = "macos"))]
fn command_failure(label: &str, status: std::process::ExitStatus, stderr: &str) -> String {
    if stderr.trim().is_empty() {
        format!("{label} exited with {status}")
    } else {
        format!("{label}: {}", stderr.trim())
    }
}

#[cfg(any(windows, target_os = "macos"))]
fn nonempty_warning(value: String) -> Option<String> {
    let trimmed = value.trim();
    (!trimmed.is_empty()).then(|| trimmed.to_string())
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
fn list_macos(ports: Option<&[u16]>, enrich_command_line: bool) -> Result<ListenerScan, String> {
    let mut command = Command::new("lsof");
    command.args(["-nP", "-iTCP", "-sTCP:LISTEN", "-Fpcn"]);
    let (status, stdout, stderr) =
        run_command_with_timeout(command, "lsof listener scan", SCAN_TIMEOUT)?;
    if !status.success() && stdout.is_empty() && !stderr.trim().is_empty() {
        return Err(command_failure("lsof listener scan", status, &stderr));
    }
    let mut warnings = nonempty_warning(stderr).into_iter().collect::<Vec<_>>();
    let mut listeners = parse_lsof(&stdout);
    if let Some(wanted) = ports {
        let wanted = wanted.iter().copied().collect::<HashSet<_>>();
        listeners.retain(|listener| wanted.contains(&listener.port));
    }
    if enrich_command_line {
        if let Some(warning) = enrich_macos_command_lines(&mut listeners)? {
            warnings.push(warning);
        }
    }
    Ok(ListenerScan {
        listeners,
        warning: (!warnings.is_empty()).then(|| warnings.join("; ")),
    })
}

#[cfg(target_os = "macos")]
fn enrich_macos_command_lines(listeners: &mut [ListenerInfo]) -> Result<Option<String>, String> {
    let mut pids = listeners.iter().map(|item| item.pid).collect::<Vec<_>>();
    pids.sort_unstable();
    pids.dedup();
    if pids.is_empty() {
        return Ok(None);
    }

    let pid_list = pids
        .iter()
        .map(u32::to_string)
        .collect::<Vec<_>>()
        .join(",");
    let mut command = Command::new("ps");
    command.args(["-ww", "-p", &pid_list, "-o", "pid=,command="]);
    let (status, stdout, stderr) =
        run_command_with_timeout(command, "ps command-line scan", SCAN_TIMEOUT)?;
    if !status.success() {
        return Err(command_failure("ps command-line scan", status, &stderr));
    }
    let commands = parse_ps_command_lines(&stdout);
    for listener in listeners {
        listener.command_line = commands.get(&listener.pid).cloned();
    }
    Ok(nonempty_warning(stderr))
}

#[cfg(any(target_os = "macos", test))]
fn parse_ps_command_lines(input: &str) -> HashMap<u32, String> {
    let mut commands = HashMap::new();
    for line in input.lines() {
        let trimmed = line.trim_start();
        let Some(split_at) = trimmed.find(char::is_whitespace) else {
            continue;
        };
        let (pid_text, command_text) = trimmed.split_at(split_at);
        let Ok(pid) = pid_text.parse::<u32>() else {
            continue;
        };
        let command = command_text.trim();
        if !command.is_empty() {
            commands.insert(pid, command.to_string());
        }
    }
    commands
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
                            command_line: None,
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
    fn windows_inventory_script_skips_cim_enrichment() {
        let script = build_windows_script(None, false);
        assert!(script.contains("Get-NetTCPConnection -State Listen"));
        assert!(script.contains("Get-Process -Id $processIds"));
        assert!(!script.contains("Get-CimInstance Win32_Process"));
        assert!(!script.contains("-LocalPort $wantedPorts"));
    }

    #[test]
    fn windows_monitored_script_targets_ports_and_enriches_processes() {
        let script = build_windows_script(Some(&[3000, 3101]), true);
        assert!(script.contains("$wantedPorts = @(3000,3101)"));
        assert!(script.contains("-LocalPort $wantedPorts"));
        assert!(script.contains("Get-CimInstance Win32_Process"));
        assert!(script.contains("[Console]::Error.WriteLine"));
    }

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

    #[test]
    fn parses_ps_command_lines() {
        let text = "  42 node /Users/test/project/server.js --port 3000\n   9 python3 -m uvicorn app:api\n";
        let result = parse_ps_command_lines(text);
        assert_eq!(
            result.get(&42).map(String::as_str),
            Some("node /Users/test/project/server.js --port 3000")
        );
        assert_eq!(
            result.get(&9).map(String::as_str),
            Some("python3 -m uvicorn app:api")
        );
    }
}
