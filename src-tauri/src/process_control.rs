use crate::diagnostics::ManagedLogPaths;
use serde::Deserialize;
use std::fs::OpenOptions;
use std::path::Path;
use std::process::{Child, Command, Stdio};

#[cfg(windows)]
use std::io::Read;
#[cfg(windows)]
use std::os::windows::process::CommandExt;
#[cfg(windows)]
use std::thread;
#[cfg(windows)]
use std::time::{Duration, Instant};

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;
#[cfg(windows)]
const CREATE_NEW_PROCESS_GROUP: u32 = 0x00000200;
#[cfg(windows)]
const PROCESS_QUERY_TIMEOUT: Duration = Duration::from_secs(12);

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ProcessSnapshot {
    pub pid: u32,
    pub parent_pid: u32,
    pub process_name: String,
    #[serde(default)]
    pub command_line: Option<String>,
    pub creation_time: String,
}

pub fn spawn_managed(command: &str, cwd: &str, logs: &ManagedLogPaths) -> Result<Child, String> {
    if command.trim().is_empty() {
        return Err("Start command cannot be empty.".into());
    }
    if !Path::new(cwd).is_dir() {
        return Err(format!("Working directory does not exist: {cwd}"));
    }

    let stdout = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&logs.stdout)
        .map_err(|error| format!("Failed to open App stdout log: {error}"))?;
    let stderr = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&logs.stderr)
        .map_err(|error| format!("Failed to open App stderr log: {error}"))?;

    #[cfg(windows)]
    let child = {
        let mut cmd = Command::new("cmd.exe");
        cmd.args(["/D", "/S", "/C"])
            .raw_arg(format!("\"{command}\""))
            .current_dir(cwd)
            .creation_flags(CREATE_NO_WINDOW | CREATE_NEW_PROCESS_GROUP)
            .stdin(Stdio::null())
            .stdout(Stdio::from(stdout))
            .stderr(Stdio::from(stderr));
        cmd.spawn()
    };

    #[cfg(not(windows))]
    let child = Command::new("/bin/zsh")
        .args(["-lc", command])
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(stderr))
        .spawn();

    child.map_err(|error| format!("Failed to start command: {error}"))
}

#[cfg(windows)]
pub fn process_ancestry(pid: u32) -> Result<Vec<ProcessSnapshot>, String> {
    guard_pid_for_query(pid)?;
    let script = format!(
        r#"
$current = [uint32]{pid}
$items = @()
$byId = @{{}}
try {{
  Get-CimInstance Win32_Process -Property ProcessId,ParentProcessId,Name,CommandLine,CreationDate -ErrorAction Stop | ForEach-Object {{
    $byId[[uint32]$_.ProcessId] = $_
  }}
}} catch {{
  [Console]::Error.WriteLine($_.Exception.Message)
  exit 1
}}
for ($depth = 0; $depth -lt 16 -and $current -gt 0; $depth++) {{
  $proc = $byId[$current]
  if ($null -eq $proc) {{ break }}
  $creation = ''
  if ($null -ne $proc.CreationDate) {{ $creation = $proc.CreationDate.ToUniversalTime().ToString('o') }}
  $items += [pscustomobject]@{{
    pid = [uint32]$proc.ProcessId
    parentPid = [uint32]$proc.ParentProcessId
    processName = [string]$proc.Name
    commandLine = if ($null -eq $proc.CommandLine) {{ $null }} else {{ [string]$proc.CommandLine }}
    creationTime = $creation
  }}
  $next = [uint32]$proc.ParentProcessId
  if ($next -eq 0 -or $next -eq $current) {{ break }}
  $current = $next
}}
ConvertTo-Json -InputObject @($items) -Compress
"#
    );
    let mut command = Command::new("powershell.exe");
    command
        .args(["-NoProfile", "-NonInteractive", "-Command", &script])
        .creation_flags(CREATE_NO_WINDOW);
    let (status, stdout, stderr) = run_hidden_with_timeout(command, PROCESS_QUERY_TIMEOUT)?;
    if !status.success() {
        return Err(if stderr.trim().is_empty() {
            format!("Process ancestry query exited with {status}")
        } else {
            format!("Process ancestry query: {}", stderr.trim())
        });
    }
    parse_process_ancestry(&stdout)
}

#[cfg(not(windows))]
pub fn process_ancestry(_pid: u32) -> Result<Vec<ProcessSnapshot>, String> {
    Err("Managed runtime reattach is currently supported on Windows only.".into())
}

#[cfg(windows)]
fn guard_pid_for_query(pid: u32) -> Result<(), String> {
    if pid == 0 {
        return Err("Cannot query PID 0.".into());
    }
    Ok(())
}

#[cfg(any(windows, test))]
fn parse_process_ancestry(input: &str) -> Result<Vec<ProcessSnapshot>, String> {
    let trimmed = input.trim();
    if trimmed.is_empty() || trimmed == "null" {
        return Ok(Vec::new());
    }
    serde_json::from_str(trimmed).map_err(|error| format!("Invalid process ancestry JSON: {error}"))
}

#[cfg(windows)]
fn run_hidden_with_timeout(
    mut command: Command,
    timeout: Duration,
) -> Result<(std::process::ExitStatus, String, String), String> {
    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| format!("Failed to query process ancestry: {error}"))?;
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
                return Err(format!(
                    "Process ancestry query timed out after {}s",
                    timeout.as_secs()
                ));
            }
            Err(error) => {
                return Err(format!(
                    "Failed while waiting for process ancestry query: {error}"
                ))
            }
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

#[cfg(windows)]
fn spawn_pipe_reader(mut pipe: impl Read + Send + 'static) -> thread::JoinHandle<String> {
    thread::spawn(move || {
        let mut value = String::new();
        let _ = pipe.read_to_string(&mut value);
        value
    })
}

pub fn terminate_tree(pid: u32) -> Result<(), String> {
    guard_pid(pid)?;

    #[cfg(windows)]
    let output = Command::new("taskkill.exe")
        .args(["/PID", &pid.to_string(), "/T", "/F"])
        .creation_flags(CREATE_NO_WINDOW)
        .output();

    #[cfg(not(windows))]
    let output = Command::new("kill")
        .args(["-TERM", &pid.to_string()])
        .output();

    let output = output.map_err(|error| format!("Failed to terminate PID {pid}: {error}"))?;
    if output.status.success() || !is_process_alive(pid) {
        Ok(())
    } else {
        let message = String::from_utf8_lossy(&output.stderr).trim().to_string();
        Err(if message.is_empty() {
            format!("Failed to terminate PID {pid}.")
        } else {
            message
        })
    }
}

pub fn is_process_alive(pid: u32) -> bool {
    if pid == 0 {
        return false;
    }

    #[cfg(windows)]
    {
        let output = Command::new("tasklist.exe")
            .args(["/FI", &format!("PID eq {pid}"), "/FO", "CSV", "/NH"])
            .creation_flags(CREATE_NO_WINDOW)
            .output();
        output
            .map(|result| {
                let text = String::from_utf8_lossy(&result.stdout);
                result.status.success() && text.contains(&format!("\"{pid}\""))
            })
            .unwrap_or(false)
    }

    #[cfg(not(windows))]
    {
        Command::new("ps")
            .args(["-p", &pid.to_string(), "-o", "pid="])
            .output()
            .map(|result| result.status.success() && !result.stdout.is_empty())
            .unwrap_or(false)
    }
}

fn guard_pid(pid: u32) -> Result<(), String> {
    if pid == 0 || pid == 4 || pid == std::process::id() {
        return Err(format!("Refusing to terminate protected PID {pid}."));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::diagnostics::Diagnostics;
    use std::fs;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    #[test]
    fn rejects_protected_pids() {
        assert!(guard_pid(0).is_err());
        assert!(guard_pid(4).is_err());
        assert!(guard_pid(std::process::id()).is_err());
    }

    #[test]
    fn managed_process_captures_stdout_and_stderr() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("port-lens-capture-{nonce}"));
        fs::create_dir_all(&root).unwrap();
        let diagnostics = Diagnostics::new(root.join("logs")).unwrap();
        let logs = diagnostics
            .prepare_managed_logs("capture-test", "Capture Test")
            .unwrap();
        let mut child = spawn_managed(
            "echo port-lens-stdout && echo port-lens-stderr 1>&2",
            root.to_str().unwrap(),
            &logs,
        )
        .unwrap();
        assert!(child.wait().unwrap().success());

        let mut captured = false;
        for _ in 0..40 {
            let stdout = fs::read_to_string(&logs.stdout).unwrap_or_default();
            let stderr = fs::read_to_string(&logs.stderr).unwrap_or_default();
            if stdout.contains("port-lens-stdout") && stderr.contains("port-lens-stderr") {
                captured = true;
                break;
            }
            std::thread::sleep(Duration::from_millis(50));
        }
        assert!(captured, "expected stdout and stderr markers in App logs");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn parses_process_ancestry_json() {
        let json = r#"[{"pid":42,"parentPid":41,"processName":"node.exe","commandLine":"node server.js","creationTime":"2026-09-11T00:00:00.0000000Z"},{"pid":41,"parentPid":7,"processName":"cmd.exe","commandLine":"cmd.exe /D /S /C node server.js","creationTime":"2026-09-10T23:59:59.0000000Z"}]"#;
        let result = parse_process_ancestry(json).unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].pid, 42);
        assert_eq!(result[1].process_name, "cmd.exe");
    }

    #[cfg(windows)]
    #[test]
    fn windows_process_ancestry_includes_current_process() {
        let result = process_ancestry(std::process::id()).unwrap();
        assert!(result.iter().any(|item| item.pid == std::process::id()));
    }

    #[cfg(windows)]
    #[test]
    fn windows_shell_preserves_quoted_powershell_paths() {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("port lens quoted {nonce}"));
        fs::create_dir_all(&root).unwrap();
        let script = root.join("quoted script.ps1");
        let marker = root.join("quoted output.txt");
        fs::write(
            &script,
            "param([string]$OutputPath)\nSet-Content -LiteralPath $OutputPath -Value 'quoted-ok'\n",
        )
        .unwrap();
        let diagnostics = Diagnostics::new(root.join("logs")).unwrap();
        let logs = diagnostics
            .prepare_managed_logs("quoted-command-test", "Quoted Command Test")
            .unwrap();
        let command = format!(
            r#"powershell.exe -NoProfile -ExecutionPolicy Bypass -File "{}" -OutputPath "{}""#,
            script.display(),
            marker.display()
        );

        let mut child = spawn_managed(&command, root.to_str().unwrap(), &logs).unwrap();
        assert!(child.wait().unwrap().success());
        assert_eq!(fs::read_to_string(&marker).unwrap().trim(), "quoted-ok");
        let _ = fs::remove_dir_all(root);
    }
}
