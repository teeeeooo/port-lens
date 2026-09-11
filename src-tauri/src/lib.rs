mod bubble;
mod diagnostics;
mod models;
mod ports;
mod process_control;
mod registry;
mod settings;
mod window_state;

use diagnostics::Diagnostics;
use models::{ListenerInfo, ManagedApp, ManagedRuntime};
use process_control::{is_process_alive, spawn_managed, terminate_tree};
use registry::AppState;
use settings::{AppSettings, SettingsPatch, SettingsStore};
use std::thread;
use std::time::{Duration, Instant};
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{Emitter, Manager, State, WebviewWindow, WindowEvent};

const BUBBLE_EVENT: &str = "port-lens://bubble-state";
const REFRESH_EVENT: &str = "port-lens://refresh";

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
    let listeners = run_listener_scan("monitored", ports, diagnostics.inner().clone()).await?;

    let mut apps = state
        .apps
        .lock()
        .map_err(|_| "App registry lock is poisoned.".to_string())?;
    let mut changed = false;
    for app in apps.iter_mut() {
        if let Some(listener) = listeners.iter().find(|listener| listener.port == app.port) {
            if app.last_process_name.is_none() {
                app.last_process_name = Some(listener.process_name.clone());
                changed = true;
            }
            if app.last_command_line.is_none() && listener.command_line.is_some() {
                app.last_command_line = listener.command_line.clone();
                changed = true;
            }
        }
    }
    if changed {
        state.persist_apps(&apps)?;
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
    Ok(runtimes
        .iter()
        .map(|(app_id, root_pid)| ManagedRuntime {
            app_id: app_id.clone(),
            root_pid: *root_pid,
        })
        .collect())
}

fn normalize_optional(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

#[tauri::command]
fn save_managed_app(mut app: ManagedApp, state: State<'_, AppState>) -> Result<ManagedApp, String> {
    app.id = app.id.trim().to_string();
    app.name = app.name.trim().to_string();
    app.command = normalize_optional(app.command);
    app.cwd = normalize_optional(app.cwd);
    app.last_process_name = normalize_optional(app.last_process_name);
    app.last_command_line = normalize_optional(app.last_command_line);

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
        *existing = app.clone();
    } else {
        apps.push(app.clone());
    }
    apps.sort_by_key(|item| item.name.to_lowercase());
    state.persist_apps(&apps)?;
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
    state.persist_apps(&apps)
}

async fn start_by_id(
    app_id: &str,
    state: &AppState,
    diagnostics: Diagnostics,
) -> Result<ManagedRuntime, String> {
    {
        let mut runtimes = state
            .runtime_pids
            .lock()
            .map_err(|_| "Runtime registry lock is poisoned.".to_string())?;
        if let Some(pid) = runtimes.get(app_id).copied() {
            if is_process_alive(pid) {
                return Ok(ManagedRuntime {
                    app_id: app_id.to_string(),
                    root_pid: pid,
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

    if let Some(blocker) = run_listener_scan("targeted", vec![app.port], diagnostics)
        .await?
        .into_iter()
        .next()
    {
        return Err(format!(
            "Port {} is already occupied by {} (PID {}).",
            app.port, blocker.process_name, blocker.pid
        ));
    }

    let pid = spawn_managed(command, cwd)?;
    state
        .runtime_pids
        .lock()
        .map_err(|_| "Runtime registry lock is poisoned.".to_string())?
        .insert(app.id.clone(), pid);

    Ok(ManagedRuntime {
        app_id: app.id,
        root_pid: pid,
    })
}

fn stop_by_id(app_id: &str, state: &AppState) -> Result<(), String> {
    let pid = state
        .runtime_pids
        .lock()
        .map_err(|_| "Runtime registry lock is poisoned.".to_string())?
        .get(app_id)
        .copied()
        .ok_or_else(|| {
            "This app was not started by Port Lens in the current session.".to_string()
        })?;

    if is_process_alive(pid) {
        terminate_tree(pid)?;
    }
    state
        .runtime_pids
        .lock()
        .map_err(|_| "Runtime registry lock is poisoned.".to_string())?
        .remove(app_id);
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
    app_id: String,
    state: State<'_, AppState>,
    diagnostics: State<'_, Diagnostics>,
) -> Result<ManagedRuntime, String> {
    let diagnostics = diagnostics.inner().clone();
    let result = start_by_id(&app_id, &state, diagnostics.clone()).await;
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
        start_by_id(&app_id, &state, diagnostics.clone()).await
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
            save_managed_app,
            remove_managed_app,
            start_managed_app,
            stop_managed_app,
            restart_managed_app,
            kill_listener_process,
            get_settings,
            open_logs,
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
}
