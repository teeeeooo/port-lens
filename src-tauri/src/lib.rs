mod bubble;
mod diagnostics;
mod models;
mod ports;
mod process_control;
mod registry;
mod settings;
mod window_state;

use diagnostics::{Diagnostics, ManagedLogPaths};
use models::{ListenerInfo, ManagedApp, ManagedExitInfo, ManagedRuntime};
use process_control::{
    is_process_alive, process_ancestry, spawn_managed, terminate_tree, ProcessSnapshot,
};
use registry::AppState;
use settings::{AppSettings, SettingsPatch, SettingsStore};
use std::process::Child;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Emitter, Manager, State, WebviewWindow, WindowEvent};

const BUBBLE_EVENT: &str = "port-lens://bubble-state";
const REFRESH_EVENT: &str = "port-lens://refresh";
const EARLY_EXIT_THRESHOLD: Duration = Duration::from_secs(10);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MinimizeTarget {
    Compact,
    Tray,
}

fn minimize_target(settings: &AppSettings) -> MinimizeTarget {
    if settings.compact_mode_enabled {
        MinimizeTarget::Compact
    } else {
        MinimizeTarget::Tray
    }
}

async fn run_listener_scan(
    kind: &'static str,
    ports: Vec<u16>,
    diagnostics: Diagnostics,
) -> Result<Vec<ListenerInfo>, String> {
    let started = Instant::now();
    let result = tauri::async_runtime::spawn_blocking(move || {
        if kind == "inventory" {
            ports::list_inventory()
        } else {
            ports::list_monitored(&ports)
        }
    })
    .await
    .map_err(|error| format!("Listener scan worker failed: {error}"))?;
    let elapsed = started.elapsed();
    if elapsed >= Duration::from_secs(2) {
        diagnostics.record(
            "WARN",
            "slow_scan",
            format!("kind={kind} elapsedMs={}", elapsed.as_millis()),
        );
    }
    if let Err(error) = &result {
        diagnostics.record(
            "ERROR",
            "listener_scan",
            format!("kind={kind} error={error}"),
        );
    }
    let report = result?;
    if let Some(warning) = report.warning {
        diagnostics.record(
            "WARN",
            "listener_scan_warning",
            format!("kind={kind} warning={warning}"),
        );
    }
    Ok(report.listeners)
}

#[tauri::command]
async fn get_listeners(diagnostics: State<'_, Diagnostics>) -> Result<Vec<ListenerInfo>, String> {
    run_listener_scan("inventory", Vec::new(), diagnostics.inner().clone()).await
}

fn normalized_process_name(value: &str) -> String {
    let lower = value.trim().to_ascii_lowercase();
    lower.strip_suffix(".exe").unwrap_or(&lower).to_string()
}

fn normalized_command_line(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_ascii_lowercase()
}

fn verify_reattach_identity(
    app: &ManagedApp,
    listener: &ListenerInfo,
    ancestry: &[ProcessSnapshot],
) -> Result<u32, String> {
    if app.launch_config().is_none() {
        return Err("launch configuration is incomplete".into());
    }
    let saved_listener_pid = app
        .last_managed_pid
        .ok_or_else(|| "managed listener PID was not persisted".to_string())?;
    if saved_listener_pid != listener.pid {
        return Err("listener PID changed".into());
    }
    let saved_name = app
        .last_managed_process_name
        .as_deref()
        .ok_or_else(|| "managed process name was not persisted".to_string())?;
    if normalized_process_name(saved_name) != normalized_process_name(&listener.process_name) {
        return Err("listener process name changed".into());
    }
    let saved_listener_command = app
        .last_managed_command_line
        .as_deref()
        .ok_or_else(|| "managed listener command line was not persisted".to_string())?;
    let listener_command = listener
        .command_line
        .as_deref()
        .ok_or_else(|| "current listener command line is unavailable".to_string())?;
    if normalized_command_line(saved_listener_command) != normalized_command_line(listener_command)
    {
        return Err("listener command line changed".into());
    }

    let listener_snapshot = ancestry
        .iter()
        .find(|item| item.pid == listener.pid)
        .ok_or_else(|| "listener process is missing from its ancestry snapshot".to_string())?;
    let saved_listener_creation = app
        .last_managed_listener_creation_time
        .as_deref()
        .ok_or_else(|| "managed listener creation time was not persisted".to_string())?;
    if listener_snapshot.creation_time != saved_listener_creation {
        return Err("listener creation time changed".into());
    }

    let root_pid = app
        .last_managed_root_pid
        .ok_or_else(|| "managed root PID was not persisted".to_string())?;
    let root_snapshot = ancestry
        .iter()
        .find(|item| item.pid == root_pid)
        .ok_or_else(|| "managed root is no longer an ancestor of the listener".to_string())?;
    if normalized_process_name(&root_snapshot.process_name) != "cmd" {
        return Err("managed root is not cmd.exe".into());
    }
    let saved_root_creation = app
        .last_managed_root_creation_time
        .as_deref()
        .ok_or_else(|| "managed root creation time was not persisted".to_string())?;
    if root_snapshot.creation_time != saved_root_creation {
        return Err("managed root creation time changed".into());
    }
    let saved_root_command = app
        .last_managed_root_command_line
        .as_deref()
        .ok_or_else(|| "managed root command line was not persisted".to_string())?;
    let root_command = root_snapshot
        .command_line
        .as_deref()
        .ok_or_else(|| "managed root command line is unavailable".to_string())?;
    if normalized_command_line(saved_root_command) != normalized_command_line(root_command) {
        return Err("managed root command line changed".into());
    }
    Ok(root_pid)
}

async fn attempt_runtime_reattach(
    state: &AppState,
    diagnostics: &Diagnostics,
    app: ManagedApp,
    listener: ListenerInfo,
) {
    if cfg!(not(target_os = "windows")) {
        return;
    }
    {
        let mut attempts = match state.reattach_attempt_pids.lock() {
            Ok(attempts) => attempts,
            Err(_) => return,
        };
        if attempts.get(&app.id).copied() == Some(listener.pid) {
            return;
        }
        attempts.insert(app.id.clone(), listener.pid);
    }

    let listener_pid = listener.pid;
    let ancestry =
        match tauri::async_runtime::spawn_blocking(move || process_ancestry(listener_pid)).await {
            Ok(Ok(ancestry)) => ancestry,
            Ok(Err(error)) => {
                diagnostics.record(
                    "WARN",
                    "runtime_reattach",
                    format!(
                        "appId={} listenerPid={} queryError={error}",
                        app.id, listener_pid
                    ),
                );
                return;
            }
            Err(error) => {
                diagnostics.record(
                    "WARN",
                    "runtime_reattach",
                    format!(
                        "appId={} listenerPid={} workerError={error}",
                        app.id, listener_pid
                    ),
                );
                return;
            }
        };

    let root_pid = match verify_reattach_identity(&app, &listener, &ancestry) {
        Ok(root_pid) => root_pid,
        Err(reason) => {
            diagnostics.record(
                "INFO",
                "runtime_reattach_rejected",
                format!(
                    "appId={} listenerPid={} reason={reason}",
                    app.id, listener_pid
                ),
            );
            return;
        }
    };
    if !is_process_alive(root_pid) {
        diagnostics.record(
            "INFO",
            "runtime_reattach_rejected",
            format!("appId={} rootPid={root_pid} reason=root-not-alive", app.id),
        );
        return;
    }

    let mut runtimes = match state.runtime_pids.lock() {
        Ok(runtimes) => runtimes,
        Err(_) => return,
    };
    if runtimes.contains_key(&app.id) {
        return;
    }
    runtimes.insert(app.id.clone(), root_pid);
    drop(runtimes);
    if let Ok(mut reattached) = state.reattached_app_ids.lock() {
        reattached.insert(app.id.clone());
    }
    diagnostics.record(
        "INFO",
        "runtime_reattach",
        format!(
            "appId={} listenerPid={} rootPid={root_pid} verified=true",
            app.id, listener_pid
        ),
    );
}

#[tauri::command]
async fn get_monitored_listeners(
    state: State<'_, AppState>,
    diagnostics: State<'_, Diagnostics>,
) -> Result<Vec<ListenerInfo>, String> {
    let ports = state
        .apps
        .lock()
        .map_err(|_| "App registry lock is poisoned.".to_string())?
        .iter()
        .map(|app| app.port)
        .collect::<Vec<_>>();
    let diagnostics = diagnostics.inner().clone();
    let listeners = run_listener_scan("monitored", ports, diagnostics.clone()).await?;

    let apps_snapshot = {
        let mut apps = state
            .apps
            .lock()
            .map_err(|_| "App registry lock is poisoned.".to_string())?;
        let mut changed = false;
        for app in apps.iter_mut() {
            if let Some(listener) = listeners.iter().find(|listener| listener.port == app.port) {
                changed |= app.observe_listener(listener, false);
            }
        }
        if changed {
            state.persist_apps(&apps)?;
        }
        apps.clone()
    };

    let runtime_ids = state
        .runtime_pids
        .lock()
        .map_err(|_| "Runtime registry lock is poisoned.".to_string())?
        .keys()
        .cloned()
        .collect::<std::collections::HashSet<_>>();
    for app in apps_snapshot {
        if runtime_ids.contains(&app.id) {
            continue;
        }
        let Some(listener) = listeners
            .iter()
            .find(|listener| listener.port == app.port)
            .cloned()
        else {
            if let Ok(mut attempts) = state.reattach_attempt_pids.lock() {
                attempts.remove(&app.id);
            }
            continue;
        };
        attempt_runtime_reattach(&state, &diagnostics, app, listener).await;
    }
    Ok(listeners)
}

#[tauri::command]
fn get_managed_apps(state: State<'_, AppState>) -> Result<Vec<ManagedApp>, String> {
    state
        .apps
        .lock()
        .map(|apps| apps.clone())
        .map_err(|_| "App registry lock is poisoned.".to_string())
}

#[tauri::command]
fn get_managed_runtimes(state: State<'_, AppState>) -> Result<Vec<ManagedRuntime>, String> {
    let mut runtimes = state
        .runtime_pids
        .lock()
        .map_err(|_| "Runtime registry lock is poisoned.".to_string())?;
    runtimes.retain(|_, pid| is_process_alive(*pid));
    let live_ids = runtimes
        .keys()
        .cloned()
        .collect::<std::collections::HashSet<_>>();
    let mut reattached = state
        .reattached_app_ids
        .lock()
        .map_err(|_| "Reattached runtime registry lock is poisoned.".to_string())?;
    reattached.retain(|app_id| live_ids.contains(app_id));
    Ok(runtimes
        .iter()
        .map(|(app_id, root_pid)| ManagedRuntime {
            app_id: app_id.clone(),
            root_pid: *root_pid,
            reattached: reattached.contains(app_id),
        })
        .collect())
}

#[tauri::command]
fn get_managed_exits(state: State<'_, AppState>) -> Result<Vec<ManagedExitInfo>, String> {
    state
        .last_exits
        .lock()
        .map(|exits| exits.values().cloned().collect())
        .map_err(|_| "Exit registry lock is poisoned.".to_string())
}

fn normalize_optional(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn timestamp_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_millis().min(u64::MAX as u128) as u64)
        .unwrap_or_default()
}

struct ManagedProcessWatch {
    app_id: String,
    app_name: String,
    pid: u32,
    child: Child,
    log_paths: ManagedLogPaths,
    started: Instant,
}

async fn capture_managed_identity_after_start(
    app_handle: AppHandle,
    diagnostics: Diagnostics,
    app_id: String,
    port: u16,
    root_pid: u32,
) {
    for _ in 0..20 {
        tokio::time::sleep(Duration::from_millis(500)).await;
        let state = app_handle.state::<AppState>();
        let still_owned = state
            .runtime_pids
            .lock()
            .map(|runtimes| runtimes.get(&app_id).copied() == Some(root_pid))
            .unwrap_or(false);
        if !still_owned {
            return;
        }

        let Ok(listeners) =
            run_listener_scan("managed_identity", vec![port], diagnostics.clone()).await
        else {
            continue;
        };
        let Some(listener) = listeners.into_iter().find(|listener| listener.port == port) else {
            continue;
        };

        #[cfg(windows)]
        let ancestry = {
            let listener_pid = listener.pid;
            match tauri::async_runtime::spawn_blocking(move || process_ancestry(listener_pid)).await
            {
                Ok(Ok(ancestry)) => ancestry,
                _ => continue,
            }
        };
        #[cfg(windows)]
        let Some(listener_snapshot) = ancestry.iter().find(|item| item.pid == listener.pid) else {
            continue;
        };
        #[cfg(windows)]
        let Some(root_snapshot) = ancestry.iter().find(|item| item.pid == root_pid) else {
            continue;
        };
        #[cfg(windows)]
        let Some(root_command_line) = root_snapshot.command_line.clone() else {
            continue;
        };

        let mut apps = match state.apps.lock() {
            Ok(apps) => apps,
            Err(_) => return,
        };
        let Some(app) = apps.iter_mut().find(|app| app.id == app_id) else {
            return;
        };
        #[cfg(windows)]
        let changed = app.record_managed_launch_identity(
            &listener,
            listener_snapshot.creation_time.clone(),
            root_pid,
            root_snapshot.creation_time.clone(),
            root_command_line,
        );
        #[cfg(not(windows))]
        let changed = app.observe_listener(&listener, true);

        if changed {
            if let Err(error) = state.persist_apps(&apps) {
                diagnostics.record(
                    "ERROR",
                    "managed_identity",
                    format!("appId={app_id} pid={} persistError={error}", listener.pid),
                );
                return;
            }
        }
        diagnostics.record(
            "INFO",
            "managed_identity",
            format!(
                "appId={app_id} rootPid={root_pid} listenerPid={} process={} reattachIdentity={}",
                listener.pid,
                listener.process_name,
                cfg!(windows)
            ),
        );
        return;
    }

    diagnostics.record(
        "WARN",
        "managed_identity",
        format!("appId={app_id} rootPid={root_pid} port={port} listenerNotObserved=true"),
    );
}

fn watch_managed_process(
    app_handle: AppHandle,
    diagnostics: Diagnostics,
    watch: ManagedProcessWatch,
) {
    thread::spawn(move || {
        let ManagedProcessWatch {
            app_id,
            app_name,
            pid,
            mut child,
            log_paths,
            started,
        } = watch;
        let exit_result = child.wait();
        let elapsed_ms = started.elapsed().as_millis().min(u64::MAX as u128) as u64;
        let exit_code = exit_result.as_ref().ok().and_then(|status| status.code());
        let state = app_handle.state::<AppState>();
        let expected = state
            .expected_exit_pids
            .lock()
            .map(|mut pids| pids.remove(&pid))
            .unwrap_or(false);

        let superseded = if let Ok(mut runtimes) = state.runtime_pids.lock() {
            match runtimes.get(&app_id).copied() {
                Some(current_pid) if current_pid == pid => {
                    runtimes.remove(&app_id);
                    false
                }
                Some(_) => true,
                None => false,
            }
        } else {
            false
        };

        let early_exit =
            !expected && !superseded && elapsed_ms < EARLY_EXIT_THRESHOLD.as_millis() as u64;
        if !expected && !superseded {
            let info = ManagedExitInfo {
                app_id: app_id.clone(),
                app_name: app_name.clone(),
                root_pid: pid,
                exit_code,
                elapsed_ms,
                timestamp_ms: timestamp_millis(),
                early_exit,
            };
            if let Ok(mut exits) = state.last_exits.lock() {
                exits.insert(app_id.clone(), info);
            }
        }

        diagnostics.record_managed_process_exit(&log_paths, pid, exit_code, elapsed_ms, expected);
        let level = if exit_result.is_err() {
            "ERROR"
        } else if expected {
            "INFO"
        } else {
            "WARN"
        };
        diagnostics.record(
            level,
            "managed_process_exit",
            format!(
                "appId={app_id} pid={pid} exitCode={} elapsedMs={elapsed_ms} expected={expected} superseded={superseded} earlyExit={early_exit} waitError={}",
                exit_code
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "unavailable".to_owned()),
                exit_result
                    .err()
                    .map(|error| error.to_string())
                    .unwrap_or_else(|| "none".to_owned())
            ),
        );
        let _ = app_handle.emit_to("main", REFRESH_EVENT, ());
    });
}

#[tauri::command]
fn save_managed_app(mut app: ManagedApp, state: State<'_, AppState>) -> Result<ManagedApp, String> {
    app.id = app.id.trim().to_string();
    app.name = app.name.trim().to_string();
    app.command = normalize_optional(app.command);
    app.cwd = normalize_optional(app.cwd);
    app.last_process_name = normalize_optional(app.last_process_name);
    app.last_command_line = normalize_optional(app.last_command_line);
    app.last_managed_process_name = normalize_optional(app.last_managed_process_name);
    app.last_managed_command_line = normalize_optional(app.last_managed_command_line);
    app.last_managed_listener_creation_time =
        normalize_optional(app.last_managed_listener_creation_time);
    app.last_managed_root_creation_time = normalize_optional(app.last_managed_root_creation_time);
    app.last_managed_root_command_line = normalize_optional(app.last_managed_root_command_line);

    if app.id.is_empty() || app.name.is_empty() {
        return Err("Name is required.".into());
    }
    if app.command.is_some() != app.cwd.is_some() {
        return Err("Start command and working directory must be configured together.".into());
    }

    if state
        .runtime_pids
        .lock()
        .map_err(|_| "Runtime registry lock is poisoned.".to_string())?
        .contains_key(&app.id)
    {
        return Err("Stop this app before editing its configuration.".into());
    }

    let mut apps = state
        .apps
        .lock()
        .map_err(|_| "App registry lock is poisoned.".to_string())?;
    if apps
        .iter()
        .any(|item| item.id != app.id && item.port == app.port)
    {
        return Err(format!(
            "Port {} is already registered as an App.",
            app.port
        ));
    }

    if let Some(existing) = apps.iter_mut().find(|item| item.id == app.id) {
        app.last_managed_pid = existing.last_managed_pid;
        app.last_managed_process_name = existing.last_managed_process_name.clone();
        app.last_managed_command_line = existing.last_managed_command_line.clone();
        app.last_managed_listener_creation_time =
            existing.last_managed_listener_creation_time.clone();
        app.last_managed_root_pid = existing.last_managed_root_pid;
        app.last_managed_root_creation_time = existing.last_managed_root_creation_time.clone();
        app.last_managed_root_command_line = existing.last_managed_root_command_line.clone();
        *existing = app.clone();
    } else {
        app.last_managed_pid = None;
        app.last_managed_process_name = None;
        app.last_managed_command_line = None;
        app.last_managed_listener_creation_time = None;
        app.last_managed_root_pid = None;
        app.last_managed_root_creation_time = None;
        app.last_managed_root_command_line = None;
        apps.push(app.clone());
    }
    apps.sort_by_key(|item| item.name.to_lowercase());
    state.persist_apps(&apps)?;
    if let Ok(mut attempts) = state.reattach_attempt_pids.lock() {
        attempts.remove(&app.id);
    }
    Ok(app)
}

#[tauri::command]
fn remove_managed_app(app_id: String, state: State<'_, AppState>) -> Result<(), String> {
    if state
        .runtime_pids
        .lock()
        .map_err(|_| "Runtime registry lock is poisoned.".to_string())?
        .contains_key(&app_id)
    {
        return Err("Stop this app before removing it.".into());
    }

    let mut apps = state
        .apps
        .lock()
        .map_err(|_| "App registry lock is poisoned.".to_string())?;
    let previous_len = apps.len();
    apps.retain(|item| item.id != app_id);
    if apps.len() == previous_len {
        return Err("App was not found.".into());
    }
    state.persist_apps(&apps)?;
    state
        .last_exits
        .lock()
        .map_err(|_| "Exit registry lock is poisoned.".to_string())?
        .remove(&app_id);
    if let Ok(mut reattached) = state.reattached_app_ids.lock() {
        reattached.remove(&app_id);
    }
    if let Ok(mut attempts) = state.reattach_attempt_pids.lock() {
        attempts.remove(&app_id);
    }
    Ok(())
}

async fn start_by_id(
    app_id: &str,
    state: &AppState,
    diagnostics: Diagnostics,
    app_handle: &AppHandle,
) -> Result<ManagedRuntime, String> {
    {
        let mut runtimes = state
            .runtime_pids
            .lock()
            .map_err(|_| "Runtime registry lock is poisoned.".to_string())?;
        if let Some(pid) = runtimes.get(app_id).copied() {
            if is_process_alive(pid) {
                let reattached = state
                    .reattached_app_ids
                    .lock()
                    .map(|items| items.contains(app_id))
                    .unwrap_or(false);
                return Ok(ManagedRuntime {
                    app_id: app_id.to_string(),
                    root_pid: pid,
                    reattached,
                });
            }
            runtimes.remove(app_id);
        }
    }

    let app = state
        .apps
        .lock()
        .map_err(|_| "App registry lock is poisoned.".to_string())?
        .iter()
        .find(|item| item.id == app_id)
        .cloned()
        .ok_or_else(|| "App was not found.".to_string())?;
    let (command, cwd) = app.launch_config().ok_or_else(|| {
        "Configure a start command and working directory before starting this App.".to_string()
    })?;

    if let Some(blocker) = run_listener_scan("targeted", vec![app.port], diagnostics.clone())
        .await?
        .into_iter()
        .next()
    {
        return Err(format!(
            "Port {} is already occupied by {} (PID {}).",
            app.port, blocker.process_name, blocker.pid
        ));
    }

    let log_paths = diagnostics.prepare_managed_logs(&app.id, &app.name)?;
    let started = Instant::now();
    let child = spawn_managed(command, cwd, &log_paths)?;
    let pid = child.id();

    state
        .last_exits
        .lock()
        .map_err(|_| "Exit registry lock is poisoned.".to_string())?
        .remove(&app.id);
    state
        .runtime_pids
        .lock()
        .map_err(|_| "Runtime registry lock is poisoned.".to_string())?
        .insert(app.id.clone(), pid);
    if let Ok(mut reattached) = state.reattached_app_ids.lock() {
        reattached.remove(&app.id);
    }
    if let Ok(mut attempts) = state.reattach_attempt_pids.lock() {
        attempts.remove(&app.id);
    }

    diagnostics.record(
        "INFO",
        "managed_process_start",
        format!(
            "appId={} pid={pid} port={} logDir={}",
            app.id,
            app.port,
            log_paths.directory.display()
        ),
    );
    tauri::async_runtime::spawn(capture_managed_identity_after_start(
        app_handle.clone(),
        diagnostics.clone(),
        app.id.clone(),
        app.port,
        pid,
    ));
    watch_managed_process(
        app_handle.clone(),
        diagnostics,
        ManagedProcessWatch {
            app_id: app.id.clone(),
            app_name: app.name.clone(),
            pid,
            child,
            log_paths,
            started,
        },
    );

    Ok(ManagedRuntime {
        app_id: app.id,
        root_pid: pid,
        reattached: false,
    })
}

fn stop_by_id(app_id: &str, state: &AppState) -> Result<(), String> {
    let pid = state
        .runtime_pids
        .lock()
        .map_err(|_| "Runtime registry lock is poisoned.".to_string())?
        .get(app_id)
        .copied()
        .ok_or_else(|| "This app is not attached to a Port Lens managed runtime.".to_string())?;
    let reattached = state
        .reattached_app_ids
        .lock()
        .map_err(|_| "Reattached runtime registry lock is poisoned.".to_string())?
        .contains(app_id);

    if is_process_alive(pid) {
        if !reattached {
            state
                .expected_exit_pids
                .lock()
                .map_err(|_| "Expected-exit registry lock is poisoned.".to_string())?
                .insert(pid);
        }
        if let Err(error) = terminate_tree(pid) {
            if !reattached {
                if let Ok(mut expected) = state.expected_exit_pids.lock() {
                    expected.remove(&pid);
                }
            }
            return Err(error);
        }
    }
    state
        .runtime_pids
        .lock()
        .map_err(|_| "Runtime registry lock is poisoned.".to_string())?
        .remove(app_id);
    if let Ok(mut reattached_ids) = state.reattached_app_ids.lock() {
        reattached_ids.remove(app_id);
    }
    if let Ok(mut attempts) = state.reattach_attempt_pids.lock() {
        attempts.remove(app_id);
    }
    Ok(())
}

fn record_failure<T>(
    diagnostics: &Diagnostics,
    event: &str,
    context: &str,
    result: &Result<T, String>,
) {
    if let Err(error) = result {
        diagnostics.record("ERROR", event, format!("{context} error={error}"));
    }
}

#[tauri::command]
async fn start_managed_app(
    app_handle: AppHandle,
    app_id: String,
    state: State<'_, AppState>,
    diagnostics: State<'_, Diagnostics>,
) -> Result<ManagedRuntime, String> {
    let diagnostics = diagnostics.inner().clone();
    let result = start_by_id(&app_id, &state, diagnostics.clone(), &app_handle).await;
    record_failure(
        &diagnostics,
        "managed_action",
        &format!("action=start appId={app_id}"),
        &result,
    );
    result
}

#[tauri::command]
fn stop_managed_app(
    app_id: String,
    state: State<'_, AppState>,
    diagnostics: State<'_, Diagnostics>,
) -> Result<(), String> {
    let result = stop_by_id(&app_id, &state);
    record_failure(
        &diagnostics,
        "managed_action",
        &format!("action=stop appId={app_id}"),
        &result,
    );
    result
}

#[tauri::command]
async fn restart_managed_app(
    app_handle: AppHandle,
    app_id: String,
    state: State<'_, AppState>,
    diagnostics: State<'_, Diagnostics>,
) -> Result<ManagedRuntime, String> {
    let diagnostics = diagnostics.inner().clone();
    let result = async {
        stop_by_id(&app_id, &state)?;
        tauri::async_runtime::spawn_blocking(|| thread::sleep(Duration::from_millis(300)))
            .await
            .map_err(|error| format!("Restart delay worker failed: {error}"))?;
        start_by_id(&app_id, &state, diagnostics.clone(), &app_handle).await
    }
    .await;
    record_failure(
        &diagnostics,
        "managed_action",
        &format!("action=restart appId={app_id}"),
        &result,
    );
    result
}

#[tauri::command]
async fn kill_listener_process(
    pid: u32,
    port: u16,
    state: State<'_, AppState>,
    diagnostics: State<'_, Diagnostics>,
) -> Result<(), String> {
    let diagnostics = diagnostics.inner().clone();
    let result = async {
        let listeners = run_listener_scan("targeted", vec![port], diagnostics.clone()).await?;
        if !listeners
            .iter()
            .any(|listener| listener.pid == pid && listener.port == port)
        {
            return Err("The selected listener changed. Refresh the list and try again.".into());
        }

        let runtimes = state
            .runtime_pids
            .lock()
            .map_err(|_| "Runtime registry lock is poisoned.".to_string())?;
        if runtimes.values().any(|managed| *managed == pid) {
            return Err("Use the App Stop action for processes started by Port Lens.".into());
        }

        let managed_ports = state
            .apps
            .lock()
            .map_err(|_| "App registry lock is poisoned.".to_string())?
            .iter()
            .filter(|app| runtimes.contains_key(&app.id))
            .map(|app| app.port)
            .collect::<Vec<_>>();
        drop(runtimes);

        let target_ports = listeners
            .iter()
            .filter(|listener| listener.pid == pid)
            .map(|listener| listener.port)
            .collect::<Vec<_>>();
        if target_ports
            .iter()
            .any(|target| managed_ports.contains(target))
        {
            return Err(
                "This process owns a Port assigned to a running App. Use Stop instead.".into(),
            );
        }

        terminate_tree(pid)
    }
    .await;
    record_failure(
        &diagnostics,
        "kill_action",
        &format!("pid={pid} port={port}"),
        &result,
    );
    result
}

#[tauri::command]
fn get_settings(
    store: State<'_, SettingsStore>,
    diagnostics: State<'_, Diagnostics>,
) -> Result<AppSettings, String> {
    let result = store.get();
    record_failure(&diagnostics, "settings_read", "get", &result);
    result
}

#[tauri::command]
fn open_logs(diagnostics: State<'_, Diagnostics>) -> Result<(), String> {
    let result = diagnostics.open_log_folder();
    record_failure(&diagnostics, "diagnostics", "open_logs", &result);
    result
}

#[tauri::command]
fn open_managed_app_logs(
    app_id: String,
    state: State<'_, AppState>,
    diagnostics: State<'_, Diagnostics>,
) -> Result<(), String> {
    let exists = state
        .apps
        .lock()
        .map_err(|_| "App registry lock is poisoned.".to_string())?
        .iter()
        .any(|app| app.id == app_id);
    if !exists {
        return Err("App was not found.".into());
    }
    let result = diagnostics.open_managed_log_folder(&app_id);
    record_failure(
        &diagnostics,
        "managed_output",
        &format!("action=open_logs appId={app_id}"),
        &result,
    );
    result
}

#[tauri::command]
fn update_settings(
    patch: SettingsPatch,
    window: WebviewWindow,
    store: State<'_, SettingsStore>,
    controller: State<'_, bubble::BubbleController>,
    diagnostics: State<'_, Diagnostics>,
) -> Result<AppSettings, String> {
    let result = (|| {
        let previous = store.get()?;
        let settings = store.update(patch)?;
        if previous.compact_mode_enabled
            && !settings.compact_mode_enabled
            && bubble::is_collapsed(&controller)?
        {
            let payload = bubble::expand(&window, &controller, true)?;
            emit_bubble_state(&window, payload);
        } else {
            bubble::resize_collapsed(&window, &controller, &store)?;
        }
        Ok(settings)
    })();
    record_failure(&diagnostics, "settings_write", "update", &result);
    result
}

fn emit_bubble_state(window: &WebviewWindow, payload: bubble::BubblePayload) {
    let _ = window.emit(BUBBLE_EVENT, payload);
}

#[tauri::command]
fn get_bubble_state(
    controller: State<'_, bubble::BubbleController>,
) -> Result<bubble::BubblePayload, String> {
    bubble::current(&controller)
}

fn collapse_window(
    window: &WebviewWindow,
    controller: &bubble::BubbleController,
    settings: &SettingsStore,
) -> Result<bubble::BubblePayload, String> {
    window_state::persist_now(window)?;
    let payload = bubble::collapse(window, controller, settings)?;
    emit_bubble_state(window, payload);
    Ok(payload)
}

#[tauri::command]
fn collapse_to_bubble(
    window: WebviewWindow,
    controller: State<'_, bubble::BubbleController>,
    settings: State<'_, SettingsStore>,
) -> Result<bubble::BubblePayload, String> {
    collapse_window(&window, &controller, &settings)
}

#[tauri::command]
fn minimize_main_window(
    window: WebviewWindow,
    controller: State<'_, bubble::BubbleController>,
    settings: State<'_, SettingsStore>,
) -> Result<bubble::BubblePayload, String> {
    match minimize_target(&settings.get()?) {
        MinimizeTarget::Compact => collapse_window(&window, &controller, &settings),
        MinimizeTarget::Tray => {
            window
                .hide()
                .map_err(|error| format!("Failed to hide Port Lens window: {error}"))?;
            bubble::current(&controller)
        }
    }
}

#[tauri::command]
fn move_compact_bubble(
    window: WebviewWindow,
    controller: State<'_, bubble::BubbleController>,
    settings: State<'_, SettingsStore>,
    offset: bubble::BubbleDragOffset,
) -> Result<bubble::BubblePayload, String> {
    bubble::move_to_cursor(&window, &controller, &settings, offset)
}

#[tauri::command]
fn expand_from_bubble(
    window: WebviewWindow,
    controller: State<'_, bubble::BubbleController>,
) -> Result<bubble::BubblePayload, String> {
    let payload = bubble::expand(&window, &controller, true)?;
    emit_bubble_state(&window, payload);
    Ok(payload)
}

fn show_main_window(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let controller = app.state::<bubble::BubbleController>();
        if let Ok(payload) = bubble::expand(&window, &controller, true) {
            emit_bubble_state(&window, payload);
        }
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            show_main_window(app);
        }))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .setup(|app| {
            let config_dir = app
                .path()
                .app_config_dir()
                .map_err(|error| format!("Failed to resolve config directory: {error}"))?;
            let log_dir = app
                .path()
                .app_log_dir()
                .map_err(|error| format!("Failed to resolve log directory: {error}"))?;
            let diagnostics = Diagnostics::new(log_dir)?;
            diagnostics.install_panic_hook();
            diagnostics.record(
                "INFO",
                "startup",
                format!(
                    "version={} os={}",
                    env!("CARGO_PKG_VERSION"),
                    std::env::consts::OS
                ),
            );
            app.manage(diagnostics.clone());
            let app_state =
                AppState::load(config_dir.join("managed-apps.json"), diagnostics.clone());
            let settings_store = SettingsStore::load(config_dir, diagnostics)?;
            let legacy_ports = settings_store.legacy_monitored_ports()?;
            if !legacy_ports.is_empty() {
                let mut apps = app_state
                    .apps
                    .lock()
                    .map_err(|_| "App registry lock is poisoned.".to_string())?;
                let mut changed = false;
                for port in legacy_ports {
                    if apps.iter().any(|app| app.port == port) {
                        continue;
                    }
                    apps.push(ManagedApp {
                        id: format!("legacy-monitor-{port}"),
                        name: format!("Port {port}"),
                        port,
                        command: None,
                        cwd: None,
                        last_process_name: None,
                        last_command_line: None,
                        last_managed_pid: None,
                        last_managed_process_name: None,
                        last_managed_command_line: None,
                        last_managed_listener_creation_time: None,
                        last_managed_root_pid: None,
                        last_managed_root_creation_time: None,
                        last_managed_root_command_line: None,
                    });
                    changed = true;
                }
                if changed {
                    apps.sort_by_key(|item| item.name.to_lowercase());
                    app_state.persist_apps(&apps)?;
                }
                drop(apps);
                settings_store.clear_legacy_monitored_ports()?;
            }
            let initial_settings = settings_store.get()?;
            app.manage(app_state);
            app.manage(settings_store);
            app.manage(bubble::BubbleController::default());
            app.manage(window_state::WindowBoundsController::default());
            if let Some(window) = app.get_webview_window("main") {
                window_state::restore_initial(&window, &initial_settings)?;
            }

            let show_item = MenuItem::with_id(app, "show", "Open Port Lens", true, None::<&str>)?;
            let bubble_item =
                MenuItem::with_id(app, "bubble", "Show compact bubble", true, None::<&str>)?;
            let refresh_item =
                MenuItem::with_id(app, "refresh", "Refresh now", true, None::<&str>)?;
            let quit_item = MenuItem::with_id(app, "quit", "Quit Port Lens", true, None::<&str>)?;
            let menu =
                Menu::with_items(app, &[&show_item, &bubble_item, &refresh_item, &quit_item])?;
            let mut tray_builder = TrayIconBuilder::with_id("main-tray")
                .tooltip("Port Lens")
                .menu(&menu)
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "show" => show_main_window(app),
                    "bubble" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let controller = app.state::<bubble::BubbleController>();
                            let settings = app.state::<SettingsStore>();
                            if settings
                                .get()
                                .map(|value| value.compact_mode_enabled)
                                .unwrap_or(false)
                            {
                                let _ = collapse_window(&window, &controller, &settings);
                            }
                        }
                    }
                    "refresh" => {
                        let _ = app.emit_to("main", REFRESH_EVENT, ());
                    }
                    "quit" => {
                        app.state::<Diagnostics>()
                            .record("INFO", "shutdown", "explicit tray quit");
                        app.exit(0);
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        show_main_window(tray.app_handle());
                    }
                });

            #[cfg(target_os = "macos")]
            {
                let tray_icon =
                    tauri::image::Image::from_bytes(include_bytes!("../icons/tray-port-lens.png"))?;
                tray_builder = tray_builder.icon(tray_icon).icon_as_template(true);
            }
            #[cfg(not(target_os = "macos"))]
            {
                let icon = app
                    .default_window_icon()
                    .cloned()
                    .ok_or("Default application icon is missing")?;
                tray_builder = tray_builder.icon(icon);
            }

            tray_builder.build(app)?;
            Ok(())
        })
        .on_window_event(|window, event| {
            if window.label() != "main" {
                return;
            }
            match event {
                WindowEvent::CloseRequested { api, .. } => {
                    api.prevent_close();
                    if let Some(webview) = window.app_handle().get_webview_window("main") {
                        let _ = window_state::persist_now(&webview);
                    }
                    window.app_handle().state::<Diagnostics>().record(
                        "INFO",
                        "shutdown",
                        "main window close",
                    );
                    window.app_handle().exit(0);
                }
                WindowEvent::Moved(_) | WindowEvent::Resized(_) => {
                    if let Some(webview) = window.app_handle().get_webview_window("main") {
                        let minimized = webview.is_minimized().unwrap_or(false);
                        if minimized {
                            let settings = window.app_handle().state::<SettingsStore>();
                            match settings.get().map(|value| minimize_target(&value)) {
                                Ok(MinimizeTarget::Compact) => {
                                    let _ = webview.unminimize();
                                    let controller =
                                        window.app_handle().state::<bubble::BubbleController>();
                                    let _ = collapse_window(&webview, &controller, &settings);
                                }
                                Ok(MinimizeTarget::Tray) => {
                                    let _ = webview.hide();
                                }
                                Err(error) => {
                                    window.app_handle().state::<Diagnostics>().record(
                                        "ERROR",
                                        "settings_read",
                                        format!("native minimize error={error}"),
                                    );
                                }
                            }
                        } else {
                            window_state::schedule_persist(webview);
                        }
                    }
                }
                _ => {}
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_listeners,
            get_monitored_listeners,
            get_managed_apps,
            get_managed_runtimes,
            get_managed_exits,
            save_managed_app,
            remove_managed_app,
            start_managed_app,
            stop_managed_app,
            restart_managed_app,
            kill_listener_process,
            get_settings,
            open_logs,
            open_managed_app_logs,
            update_settings,
            get_bubble_state,
            collapse_to_bubble,
            minimize_main_window,
            move_compact_bubble,
            expand_from_bubble
        ])
        .run(tauri::generate_context!())
        .expect("error while running Port Lens");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn minimize_policy_uses_compact_by_default_and_tray_when_disabled() {
        let defaults = AppSettings::default();
        assert_eq!(minimize_target(&defaults), MinimizeTarget::Compact);

        let tray = AppSettings {
            compact_mode_enabled: false,
            ..AppSettings::default()
        };
        assert_eq!(minimize_target(&tray), MinimizeTarget::Tray);
    }

    fn reattach_fixture() -> (ManagedApp, ListenerInfo, Vec<ProcessSnapshot>) {
        let app = ManagedApp {
            id: "api".into(),
            name: "API".into(),
            port: 3000,
            command: Some("node server.js".into()),
            cwd: Some(r"C:\app".into()),
            last_process_name: Some("node".into()),
            last_command_line: Some("node server.js".into()),
            last_managed_pid: Some(4200),
            last_managed_process_name: Some("node".into()),
            last_managed_command_line: Some("node server.js".into()),
            last_managed_listener_creation_time: Some("listener-created".into()),
            last_managed_root_pid: Some(4100),
            last_managed_root_creation_time: Some("root-created".into()),
            last_managed_root_command_line: Some("cmd.exe /D /S /C node server.js".into()),
        };
        let listener = ListenerInfo {
            protocol: "TCP".into(),
            local_address: "0.0.0.0".into(),
            port: 3000,
            pid: 4200,
            process_name: "node.exe".into(),
            command_line: Some("node   server.js".into()),
        };
        let ancestry = vec![
            ProcessSnapshot {
                pid: 4200,
                parent_pid: 4150,
                process_name: "node.exe".into(),
                command_line: Some("node server.js".into()),
                creation_time: "listener-created".into(),
            },
            ProcessSnapshot {
                pid: 4150,
                parent_pid: 4100,
                process_name: "pwsh.exe".into(),
                command_line: Some("pwsh -File start.ps1".into()),
                creation_time: "shell-created".into(),
            },
            ProcessSnapshot {
                pid: 4100,
                parent_pid: 4000,
                process_name: "cmd.exe".into(),
                command_line: Some("cmd.exe /D /S /C node server.js".into()),
                creation_time: "root-created".into(),
            },
        ];
        (app, listener, ancestry)
    }

    #[test]
    fn reattach_requires_exact_persisted_process_generation_and_root_ancestry() {
        let (app, listener, ancestry) = reattach_fixture();
        assert_eq!(
            verify_reattach_identity(&app, &listener, &ancestry),
            Ok(4100)
        );
    }

    #[test]
    fn reattach_rejects_pid_reuse_via_creation_time() {
        let (app, listener, mut ancestry) = reattach_fixture();
        ancestry[0].creation_time = "different-generation".into();
        assert!(verify_reattach_identity(&app, &listener, &ancestry)
            .unwrap_err()
            .contains("creation time changed"));
    }

    #[test]
    fn reattach_rejects_listener_without_saved_root_in_ancestry() {
        let (app, listener, mut ancestry) = reattach_fixture();
        ancestry.pop();
        assert!(verify_reattach_identity(&app, &listener, &ancestry)
            .unwrap_err()
            .contains("no longer an ancestor"));
    }
}
