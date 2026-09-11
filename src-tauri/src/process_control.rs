use crate::diagnostics::ManagedLogPaths;
use std::fs::OpenOptions;
use std::path::Path;
use std::process::{Command, Stdio};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;
#[cfg(windows)]
const CREATE_NEW_PROCESS_GROUP: u32 = 0x00000200;

pub fn spawn_managed(command: &str, cwd: &str, logs: &ManagedLogPaths) -> Result<u32, String> {
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
    let child = Command::new("cmd.exe")
        .args(["/D", "/S", "/C", command])
        .current_dir(cwd)
        .creation_flags(CREATE_NO_WINDOW | CREATE_NEW_PROCESS_GROUP)
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(stderr))
        .spawn();

    #[cfg(not(windows))]
    let child = Command::new("/bin/zsh")
        .args(["-lc", command])
        .current_dir(cwd)
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(stderr))
        .spawn();

    child
        .map(|child| child.id())
        .map_err(|error| format!("Failed to start command: {error}"))
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
        spawn_managed(
            "echo port-lens-stdout && echo port-lens-stderr 1>&2",
            root.to_str().unwrap(),
            &logs,
        )
        .unwrap();

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
}
