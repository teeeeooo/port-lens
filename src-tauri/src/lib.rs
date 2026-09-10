mod bubble;
mod models;
mod ports;
mod process_control;
mod registry;

use models::{ListenerInfo, ManagedApp, ManagedRuntime};
use process_control::{is_process_alive, spawn_managed, terminate_tree};
use registry::AppState;
use std::thread;
use std::time::Duration;
use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{Emitter, Manager, State, WebviewWindow, WindowEvent};

const BUBBLE_EVENT: &str = "port-lens://bubble-state";
const REFRESH_EVENT: &str = "port-lens://refresh";

#[tauri::command]
fn get_listeners() -> Result<Vec<ListenerInfo>, String> {
    ports::list_listeners()
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

#[tauri::command]
fn save_managed_app(mut app: ManagedApp, state: State<'_, AppState>) -> Result<ManagedApp, String> {
    app.id = app.id.trim().to_string();
    app.name = app.name.trim().to_string();
    app.command = app.command.trim().to_string();
    app.cwd = app.cwd.trim().to_string();

    if app.id.is_empty() || app.name.is_empty() || app.command.is_empty() || app.cwd.is_empty() {
        return Err("Name, command, and working directory are required.".into());
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
        return Err("Managed app was not found.".into());
    }
    state.persist_apps(&apps)
}

fn start_by_id(app_id: &str, state: &AppState) -> Result<ManagedRuntime, String> {
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
        .ok_or_else(|| "Managed app was not found.".to_string())?;

    if let Some(blocker) = ports::list_listeners()?
        .into_iter()
        .find(|item| item.port == app.port)
    {
        return Err(format!(
            "Port {} is already occupied by {} (PID {}).",
            app.port, blocker.process_name, blocker.pid
        ));
    }

    let pid = spawn_managed(&app.command, &app.cwd)?;
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

#[tauri::command]
fn start_managed_app(app_id: String, state: State<'_, AppState>) -> Result<ManagedRuntime, String> {
    start_by_id(&app_id, &state)
}

#[tauri::command]
fn stop_managed_app(app_id: String, state: State<'_, AppState>) -> Result<(), String> {
    stop_by_id(&app_id, &state)
}

#[tauri::command]
fn restart_managed_app(
    app_id: String,
    state: State<'_, AppState>,
) -> Result<ManagedRuntime, String> {
    stop_by_id(&app_id, &state)?;
    thread::sleep(Duration::from_millis(300));
    start_by_id(&app_id, &state)
}

#[tauri::command]
fn kill_listener_process(pid: u32, port: u16, state: State<'_, AppState>) -> Result<(), String> {
    let listeners = ports::list_listeners()?;
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
        return Err("Use the managed app Stop action for processes started by Port Lens.".into());
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
    if target_ports.iter().any(|port| managed_ports.contains(port)) {
        return Err(
            "This process owns a port assigned to a running managed app. Use Stop instead.".into(),
        );
    }

    terminate_tree(pid)
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

#[tauri::command]
fn collapse_to_bubble(
    window: WebviewWindow,
    controller: State<'_, bubble::BubbleController>,
) -> Result<bubble::BubblePayload, String> {
    let payload = bubble::collapse(&window, &controller)?;
    emit_bubble_state(&window, payload);
    Ok(payload)
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
        .setup(|app| {
            let config_path = app
                .path()
                .app_config_dir()
                .map_err(|error| format!("Failed to resolve config directory: {error}"))?
                .join("managed-apps.json");
            app.manage(AppState::load(config_path));
            app.manage(bubble::BubbleController::default());

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
                            if let Ok(payload) = bubble::collapse(&window, &controller) {
                                emit_bubble_state(&window, payload);
                            }
                        }
                    }
                    "refresh" => {
                        let _ = app.emit_to("main", REFRESH_EVENT, ());
                    }
                    "quit" => app.exit(0),
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
            if window.label() == "main" {
                if let WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            get_listeners,
            get_managed_apps,
            get_managed_runtimes,
            save_managed_app,
            remove_managed_app,
            start_managed_app,
            stop_managed_app,
            restart_managed_app,
            kill_listener_process,
            get_bubble_state,
            collapse_to_bubble,
            expand_from_bubble
        ])
        .run(tauri::generate_context!())
        .expect("error while running Port Lens");
}
