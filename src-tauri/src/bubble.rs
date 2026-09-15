use crate::settings::{SettingsStore, WindowPosition};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use tauri::{LogicalSize, Manager, Monitor, PhysicalPosition, PhysicalSize, WebviewWindow};

#[cfg(windows)]
use std::{thread, time::Duration};
#[cfg(windows)]
use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_LBUTTON};
#[cfg(windows)]
use windows::Win32::UI::WindowsAndMessaging::{
    SetWindowPos, HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SWP_NOZORDER,
};

const BUBBLE_WIDTH: f64 = 276.0;
const BUBBLE_HEIGHT: f64 = 46.0;
const HOVER_ROW_HEIGHT: f64 = 28.0;
const HOVER_PANEL_PADDING: f64 = 14.0;
const MAX_HOVER_ROWS: u32 = 8;
const DESKTOP_EDGE_MARGIN: i32 = 12;
const MIN_WIDTH: f64 = 800.0;
const MIN_HEIGHT: f64 = 580.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CompactPositionPolicy {
    Windows,
    Desktop,
}

impl CompactPositionPolicy {
    fn current() -> Self {
        if cfg!(target_os = "windows") {
            Self::Windows
        } else {
            Self::Desktop
        }
    }

    fn edge_margin(self) -> i32 {
        match self {
            Self::Windows => 0,
            Self::Desktop => DESKTOP_EDGE_MARGIN,
        }
    }

    fn allows_reserved_area(self) -> bool {
        matches!(self, Self::Windows)
    }
}

fn bubble_physical_size(bubble_scale: f64, monitor_scale: f64) -> PhysicalSize<u32> {
    PhysicalSize::new(
        (BUBBLE_WIDTH * bubble_scale * monitor_scale)
            .round()
            .max(1.0) as u32,
        (BUBBLE_HEIGHT * bubble_scale * monitor_scale)
            .round()
            .max(1.0) as u32,
    )
}

fn hover_panel_physical_size(
    bubble_scale: f64,
    monitor_scale: f64,
    rows: u32,
    width: u32,
) -> PhysicalSize<u32> {
    let visible_rows = rows.clamp(1, MAX_HOVER_ROWS);
    let logical_height =
        (HOVER_PANEL_PADDING + HOVER_ROW_HEIGHT * visible_rows as f64) * bubble_scale + 2.0;
    PhysicalSize::new(
        width.max(1),
        (logical_height * monitor_scale).round().max(1.0) as u32,
    )
}

#[derive(Debug, Clone, Copy)]
struct ExpandedWindow {
    position: PhysicalPosition<i32>,
    inner_size: PhysicalSize<u32>,
    maximized: bool,
    always_on_top: bool,
}

#[derive(Debug, Default)]
struct BubbleState {
    collapsed: bool,
    z_order_generation: u64,
    expanded: Option<ExpandedWindow>,
}

#[derive(Debug, Default)]
pub struct BubbleController {
    state: Arc<Mutex<BubbleState>>,
}

#[derive(Debug, Clone, Copy, Serialize)]
pub struct BubblePayload {
    pub collapsed: bool,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BubbleDragOffset {
    pub offset_ratio_x: f64,
    pub offset_ratio_y: f64,
    #[serde(default)]
    pub hide_hover: bool,
}

fn window_error(error: tauri::Error) -> String {
    format!("bubble window operation failed: {error}")
}

pub fn current(controller: &BubbleController) -> Result<BubblePayload, String> {
    Ok(BubblePayload {
        collapsed: is_collapsed(controller)?,
    })
}

pub fn is_collapsed(controller: &BubbleController) -> Result<bool, String> {
    controller
        .state
        .lock()
        .map(|state| state.collapsed)
        .map_err(|_| "Bubble state lock is poisoned.".to_string())
}

#[cfg(windows)]
pub fn is_compact_drag_input_active() -> bool {
    unsafe { (GetAsyncKeyState(VK_LBUTTON.0 as i32) as u16 & 0x8000) != 0 }
}

#[cfg(not(windows))]
pub fn is_compact_drag_input_active() -> bool {
    false
}

fn clamp_i32(value: i64, min: i64, max: i64) -> i32 {
    value.clamp(min, max.max(min)) as i32
}

fn clamp_to_work_area(
    work_position: PhysicalPosition<i32>,
    work_size: PhysicalSize<u32>,
    size: PhysicalSize<u32>,
    desired_x: i64,
    desired_y: i64,
    edge_margin: i32,
) -> PhysicalPosition<i32> {
    let min_x = work_position.x as i64 + edge_margin as i64;
    let max_x =
        work_position.x as i64 + work_size.width as i64 - size.width as i64 - edge_margin as i64;
    let min_y = work_position.y as i64 + edge_margin as i64;
    let max_y =
        work_position.y as i64 + work_size.height as i64 - size.height as i64 - edge_margin as i64;
    PhysicalPosition::new(
        clamp_i32(desired_x, min_x, max_x),
        clamp_i32(desired_y, min_y, max_y),
    )
}

fn clamp_position(
    monitor: &Monitor,
    size: PhysicalSize<u32>,
    desired_x: i64,
    desired_y: i64,
) -> PhysicalPosition<i32> {
    let policy = CompactPositionPolicy::current();
    if policy.allows_reserved_area() {
        clamp_to_work_area(
            *monitor.position(),
            *monitor.size(),
            size,
            desired_x,
            desired_y,
            policy.edge_margin(),
        )
    } else {
        let work = monitor.work_area();
        clamp_to_work_area(
            work.position,
            work.size,
            size,
            desired_x,
            desired_y,
            policy.edge_margin(),
        )
    }
}

fn drag_desired_position(
    cursor_x: f64,
    cursor_y: f64,
    size: PhysicalSize<u32>,
    offset_ratio_x: f64,
    offset_ratio_y: f64,
) -> (i64, i64) {
    let ratio_x = offset_ratio_x.clamp(0.0, 1.0);
    let ratio_y = offset_ratio_y.clamp(0.0, 1.0);
    (
        (cursor_x - ratio_x * size.width as f64).round() as i64,
        (cursor_y - ratio_y * size.height as f64).round() as i64,
    )
}

fn hover_panel_position_in_bounds(
    bounds_position: PhysicalPosition<i32>,
    bounds_size: PhysicalSize<u32>,
    main_position: PhysicalPosition<i32>,
    main_size: PhysicalSize<u32>,
    panel_size: PhysicalSize<u32>,
    edge_margin: i32,
) -> PhysicalPosition<i32> {
    let bounds_top = bounds_position.y as i64 + edge_margin as i64;
    let bounds_bottom = bounds_position.y as i64 + bounds_size.height as i64 - edge_margin as i64;
    let main_top = main_position.y as i64;
    let main_bottom = main_top + main_size.height as i64;
    let panel_height = panel_size.height as i64;
    let space_above = main_top - bounds_top;
    let space_below = bounds_bottom - main_bottom;
    let desired_y = if space_above >= panel_height {
        main_top - panel_height
    } else if space_below >= panel_height {
        main_bottom
    } else if space_above >= space_below {
        bounds_top
    } else {
        bounds_bottom - panel_height
    };
    clamp_to_work_area(
        bounds_position,
        bounds_size,
        panel_size,
        main_position.x as i64,
        desired_y,
        edge_margin,
    )
}

fn hover_panel_position(
    monitor: &Monitor,
    main_position: PhysicalPosition<i32>,
    main_size: PhysicalSize<u32>,
    panel_size: PhysicalSize<u32>,
) -> PhysicalPosition<i32> {
    let policy = CompactPositionPolicy::current();
    if policy.allows_reserved_area() {
        hover_panel_position_in_bounds(
            *monitor.position(),
            *monitor.size(),
            main_position,
            main_size,
            panel_size,
            policy.edge_margin(),
        )
    } else {
        let work = monitor.work_area();
        hover_panel_position_in_bounds(
            work.position,
            work.size,
            main_position,
            main_size,
            panel_size,
            policy.edge_margin(),
        )
    }
}

fn default_collapsed_position(
    monitor: &Monitor,
    size: PhysicalSize<u32>,
    desired_y: i32,
) -> PhysicalPosition<i32> {
    let work = monitor.work_area();
    let edge_margin = CompactPositionPolicy::current().edge_margin();
    let desired_x =
        work.position.x as i64 + work.size.width as i64 - size.width as i64 - edge_margin as i64;
    clamp_to_work_area(
        work.position,
        work.size,
        size,
        desired_x,
        desired_y as i64,
        edge_margin,
    )
}

fn monitor_for_saved_position(
    window: &WebviewWindow,
    saved: WindowPosition,
) -> Result<Option<Monitor>, String> {
    window
        .monitor_from_point(saved.x as f64, saved.y as f64)
        .map_err(window_error)
}

#[cfg(any(windows, test))]
fn rect_overlaps_reserved_area(
    position: PhysicalPosition<i32>,
    size: PhysicalSize<u32>,
    screen_position: PhysicalPosition<i32>,
    screen_size: PhysicalSize<u32>,
    work_position: PhysicalPosition<i32>,
    work_size: PhysicalSize<u32>,
) -> bool {
    let left = position.x as i64;
    let top = position.y as i64;
    let right = left + size.width as i64;
    let bottom = top + size.height as i64;
    let screen_left = screen_position.x as i64;
    let screen_top = screen_position.y as i64;
    let screen_right = screen_left + screen_size.width as i64;
    let screen_bottom = screen_top + screen_size.height as i64;
    let intersects_screen =
        left < screen_right && right > screen_left && top < screen_bottom && bottom > screen_top;
    if !intersects_screen {
        return false;
    }
    let work_left = work_position.x as i64;
    let work_top = work_position.y as i64;
    let work_right = work_left + work_size.width as i64;
    let work_bottom = work_top + work_size.height as i64;
    left < work_left || top < work_top || right > work_right || bottom > work_bottom
}

#[cfg(windows)]
fn overlaps_reserved_area(
    position: PhysicalPosition<i32>,
    size: PhysicalSize<u32>,
    monitor: &Monitor,
) -> bool {
    let work = monitor.work_area();
    rect_overlaps_reserved_area(
        position,
        size,
        *monitor.position(),
        *monitor.size(),
        work.position,
        work.size,
    )
}

#[cfg(windows)]
fn refresh_taskbar_z_order(window: &WebviewWindow) -> Result<(), String> {
    let position = window.outer_position().map_err(window_error)?;
    let size = window.outer_size().map_err(window_error)?;
    let center_x = position.x as f64 + size.width as f64 / 2.0;
    let center_y = position.y as f64 + size.height as f64 / 2.0;
    let Some(monitor) = window
        .monitor_from_point(center_x, center_y)
        .map_err(window_error)?
        .or_else(|| window.current_monitor().ok().flatten())
    else {
        return Ok(());
    };
    if !overlaps_reserved_area(position, size, &monitor) {
        return Ok(());
    }
    let hwnd = window.hwnd().map_err(window_error)?;
    unsafe {
        SetWindowPos(
            hwnd,
            Some(HWND_TOPMOST),
            0,
            0,
            0,
            0,
            SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
        )
        .map_err(|error| format!("failed to keep compact bubble above taskbar: {error}"))?;
    }
    Ok(())
}

#[cfg(not(windows))]
fn refresh_taskbar_z_order(_window: &WebviewWindow) -> Result<(), String> {
    Ok(())
}

#[cfg(windows)]
fn start_taskbar_z_order_keeper(
    window: WebviewWindow,
    state: Arc<Mutex<BubbleState>>,
    generation: u64,
) {
    thread::spawn(move || loop {
        let keep_running = state
            .lock()
            .map(|state| state.collapsed && state.z_order_generation == generation)
            .unwrap_or(false);
        if !keep_running {
            break;
        }
        if !is_compact_drag_input_active() {
            let _ = refresh_taskbar_z_order(&window);
        }
        thread::sleep(Duration::from_millis(250));
    });
}

#[cfg(not(windows))]
fn start_taskbar_z_order_keeper(
    _window: WebviewWindow,
    _state: Arc<Mutex<BubbleState>>,
    _generation: u64,
) {
}

#[cfg(windows)]
fn set_compact_geometry(
    window: &WebviewWindow,
    target: PhysicalPosition<i32>,
    size: PhysicalSize<u32>,
) -> Result<(), String> {
    let hwnd = window.hwnd().map_err(window_error)?;
    unsafe {
        SetWindowPos(
            hwnd,
            Some(HWND_TOPMOST),
            target.x,
            target.y,
            size.width as i32,
            size.height as i32,
            SWP_NOACTIVATE,
        )
        .map_err(|error| format!("failed to resize compact bubble: {error}"))?;
    }
    Ok(())
}

#[cfg(not(windows))]
fn set_compact_geometry(
    window: &WebviewWindow,
    target: PhysicalPosition<i32>,
    size: PhysicalSize<u32>,
) -> Result<(), String> {
    window.set_size(size).map_err(window_error)?;
    window.set_position(target).map_err(window_error)
}

#[cfg(windows)]
fn set_compact_position(
    window: &WebviewWindow,
    target: PhysicalPosition<i32>,
) -> Result<(), String> {
    window.set_position(target).map_err(window_error)
}

#[cfg(not(windows))]
fn set_compact_position(
    window: &WebviewWindow,
    target: PhysicalPosition<i32>,
) -> Result<(), String> {
    window.set_position(target).map_err(window_error)
}

fn apply_collapsed_window(
    window: &WebviewWindow,
    target: PhysicalPosition<i32>,
    size: PhysicalSize<u32>,
) -> Result<(), String> {
    window
        .set_min_size(None::<LogicalSize<f64>>)
        .map_err(window_error)?;
    window
        .set_max_size(None::<LogicalSize<f64>>)
        .map_err(window_error)?;
    window.set_resizable(false).map_err(window_error)?;
    window.set_decorations(false).map_err(window_error)?;
    window.set_shadow(false).map_err(window_error)?;
    window.set_always_on_top(true).map_err(window_error)?;
    let _ = window.set_skip_taskbar(true);
    set_compact_geometry(window, target, size)?;
    window.show().map_err(window_error)
}

pub fn hide_hover_panel(window: &WebviewWindow) -> Result<(), String> {
    if let Some(hover) = window.app_handle().get_webview_window("compact-hover") {
        hover.hide().map_err(window_error)?;
    }
    Ok(())
}

pub fn show_hover_panel(
    window: &WebviewWindow,
    controller: &BubbleController,
    settings: &SettingsStore,
    rows: u32,
) -> Result<(), String> {
    if rows == 0 || !is_collapsed(controller)? {
        return hide_hover_panel(window);
    }
    let hover = window
        .app_handle()
        .get_webview_window("compact-hover")
        .ok_or_else(|| "Compact hover window is unavailable.".to_string())?;
    let current_settings = settings.get()?;
    let main_position = window.outer_position().map_err(window_error)?;
    let main_size = window.outer_size().map_err(window_error)?;
    let center_x = main_position.x as f64 + main_size.width as f64 / 2.0;
    let center_y = main_position.y as f64 + main_size.height as f64 / 2.0;
    let monitor = window
        .monitor_from_point(center_x, center_y)
        .map_err(window_error)?
        .or_else(|| window.current_monitor().ok().flatten())
        .ok_or_else(|| "No monitor is available for compact hover.".to_string())?;
    let panel_size = hover_panel_physical_size(
        current_settings.bubble_scale,
        monitor.scale_factor(),
        rows,
        main_size.width,
    );
    let target = hover_panel_position(&monitor, main_position, main_size, panel_size);
    set_compact_geometry(&hover, target, panel_size)?;
    hover.show().map_err(window_error)
}

pub fn move_to_cursor(
    window: &WebviewWindow,
    controller: &BubbleController,
    settings: &SettingsStore,
    offset: BubbleDragOffset,
) -> Result<(), String> {
    if !is_collapsed(controller)? {
        return Ok(());
    }
    if offset.hide_hover {
        hide_hover_panel(window)?;
    }

    let cursor = window.cursor_position().map_err(window_error)?;
    let monitor = window
        .monitor_from_point(cursor.x, cursor.y)
        .map_err(window_error)?
        .or_else(|| window.current_monitor().ok().flatten())
        .ok_or_else(|| "No monitor is available while moving the compact bubble.".to_string())?;
    let current_settings = settings.get()?;
    let size = bubble_physical_size(current_settings.bubble_scale, monitor.scale_factor());
    let (desired_x, desired_y) = drag_desired_position(
        cursor.x,
        cursor.y,
        size,
        offset.offset_ratio_x,
        offset.offset_ratio_y,
    );
    let target = clamp_position(&monitor, size, desired_x, desired_y);
    set_compact_position(window, target)
}

pub fn persist_compact_position(
    window: &WebviewWindow,
    controller: &BubbleController,
    settings: &SettingsStore,
) -> Result<(), String> {
    if !is_collapsed(controller)? {
        return Ok(());
    }
    let position = window.outer_position().map_err(window_error)?;
    let size = window.outer_size().map_err(window_error)?;
    let center_x = position.x as f64 + size.width as f64 / 2.0;
    let center_y = position.y as f64 + size.height as f64 / 2.0;
    let monitor = window
        .monitor_from_point(center_x, center_y)
        .map_err(window_error)?
        .or_else(|| window.current_monitor().ok().flatten())
        .ok_or_else(|| "No monitor is available while saving compact position.".to_string())?;
    let target = clamp_position(&monitor, size, position.x as i64, position.y as i64);
    if target != position {
        set_compact_position(window, target)?;
    }
    settings.update_compact_position(WindowPosition {
        x: target.x,
        y: target.y,
    })?;
    refresh_taskbar_z_order(window)
}

pub fn collapse(
    window: &WebviewWindow,
    controller: &BubbleController,
    settings: &SettingsStore,
) -> Result<BubblePayload, String> {
    hide_hover_panel(window)?;
    let mut state = controller
        .state
        .lock()
        .map_err(|_| "Bubble state lock is poisoned.".to_string())?;
    if state.collapsed {
        return Ok(BubblePayload { collapsed: true });
    }

    let current_settings = settings.get()?;
    if !current_settings.compact_mode_enabled {
        return Err("Compact mode is disabled.".to_owned());
    }

    let position = window.outer_position().map_err(window_error)?;
    let outer_size = window.outer_size().map_err(window_error)?;
    let inner_size = window.inner_size().map_err(window_error)?;
    let maximized = window.is_maximized().map_err(window_error)?;
    let always_on_top = window.is_always_on_top().map_err(window_error)?;
    state.expanded = Some(ExpandedWindow {
        position,
        inner_size,
        maximized,
        always_on_top,
    });

    if maximized {
        window.unmaximize().map_err(window_error)?;
    }

    let monitor = match current_settings.compact_position {
        Some(saved) => monitor_for_saved_position(window, saved)?
            .or_else(|| window.current_monitor().ok().flatten()),
        None => window.current_monitor().map_err(window_error)?,
    }
    .ok_or_else(|| "No monitor is available for the compact bubble.".to_string())?;
    let bubble_size = bubble_physical_size(current_settings.bubble_scale, monitor.scale_factor());
    let target = if let Some(saved) = current_settings.compact_position {
        clamp_position(&monitor, bubble_size, saved.x as i64, saved.y as i64)
    } else {
        let desired_y = position.y + (outer_size.height as i32 - bubble_size.height as i32) / 2;
        default_collapsed_position(&monitor, bubble_size, desired_y)
    };

    apply_collapsed_window(window, target, bubble_size)?;
    settings.update_compact_position(WindowPosition {
        x: target.x,
        y: target.y,
    })?;
    state.z_order_generation = state.z_order_generation.wrapping_add(1);
    let generation = state.z_order_generation;
    state.collapsed = true;
    drop(state);
    refresh_taskbar_z_order(window)?;
    start_taskbar_z_order_keeper(window.clone(), controller.state.clone(), generation);
    Ok(BubblePayload { collapsed: true })
}

pub fn resize_collapsed(
    window: &WebviewWindow,
    controller: &BubbleController,
    settings: &SettingsStore,
) -> Result<(), String> {
    hide_hover_panel(window)?;
    if !is_collapsed(controller)? {
        return Ok(());
    }
    let current_settings = settings.get()?;
    let position = window.outer_position().map_err(window_error)?;
    let old_size = window.outer_size().map_err(window_error)?;
    let center_x = position.x as f64 + old_size.width as f64 / 2.0;
    let center_y = position.y as f64 + old_size.height as f64 / 2.0;
    let monitor = window
        .monitor_from_point(center_x, center_y)
        .map_err(window_error)?
        .or_else(|| window.current_monitor().ok().flatten())
        .ok_or_else(|| "No monitor is available for the compact bubble.".to_string())?;
    let new_size = bubble_physical_size(current_settings.bubble_scale, monitor.scale_factor());
    let target = clamp_position(
        &monitor,
        new_size,
        position.x as i64 + (old_size.width as i64 - new_size.width as i64) / 2,
        position.y as i64 + (old_size.height as i64 - new_size.height as i64) / 2,
    );
    set_compact_geometry(window, target, new_size)?;
    refresh_taskbar_z_order(window)?;
    settings.update_compact_position(WindowPosition {
        x: target.x,
        y: target.y,
    })
}

pub fn expand(
    window: &WebviewWindow,
    controller: &BubbleController,
    focus: bool,
) -> Result<BubblePayload, String> {
    hide_hover_panel(window)?;
    let expanded = {
        let mut state = controller
            .state
            .lock()
            .map_err(|_| "Bubble state lock is poisoned.".to_string())?;
        if !state.collapsed {
            None
        } else {
            let expanded = state
                .expanded
                .ok_or_else(|| "No expanded window state is available.".to_string())?;
            state.z_order_generation = state.z_order_generation.wrapping_add(1);
            state.collapsed = false;
            Some(expanded)
        }
    };

    let Some(expanded) = expanded else {
        window.show().map_err(window_error)?;
        if window.is_minimized().map_err(window_error)? {
            window.unminimize().map_err(window_error)?;
        }
        if focus {
            window.set_focus().map_err(window_error)?;
        }
        return Ok(BubblePayload { collapsed: false });
    };

    window
        .set_min_size(Some(LogicalSize::new(MIN_WIDTH, MIN_HEIGHT)))
        .map_err(window_error)?;
    window
        .set_max_size(None::<LogicalSize<f64>>)
        .map_err(window_error)?;
    window.set_resizable(true).map_err(window_error)?;
    window.set_decorations(true).map_err(window_error)?;
    window.set_shadow(true).map_err(window_error)?;
    window
        .set_always_on_top(expanded.always_on_top)
        .map_err(window_error)?;
    let _ = window.set_skip_taskbar(false);
    window.set_size(expanded.inner_size).map_err(window_error)?;
    window
        .set_position(expanded.position)
        .map_err(window_error)?;
    window.show().map_err(window_error)?;
    if expanded.maximized {
        window.maximize().map_err(window_error)?;
    }
    if focus {
        window.set_focus().map_err(window_error)?;
    }
    Ok(BubblePayload { collapsed: false })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bubble_size_scales_with_user_setting_and_monitor_dpi() {
        assert_eq!(bubble_physical_size(0.7, 2.0), PhysicalSize::new(386, 64));
        assert_eq!(bubble_physical_size(1.0, 1.0), PhysicalSize::new(276, 46));
        assert_eq!(bubble_physical_size(1.5, 1.0), PhysicalSize::new(414, 69));
    }

    #[test]
    fn hover_panel_size_grows_for_apps_and_caps_at_eight_rows() {
        assert_eq!(
            hover_panel_physical_size(1.0, 1.0, 3, 276),
            PhysicalSize::new(276, 100)
        );
        assert_eq!(
            hover_panel_physical_size(1.0, 1.0, 8, 276),
            PhysicalSize::new(276, 240)
        );
        assert_eq!(
            hover_panel_physical_size(1.0, 1.0, 20, 276),
            PhysicalSize::new(276, 240)
        );
    }

    #[test]
    fn hover_panel_prefers_above_and_falls_below_near_top_edge() {
        let bounds_position = PhysicalPosition::new(0, 0);
        let bounds_size = PhysicalSize::new(1920, 1080);
        let panel_size = PhysicalSize::new(276, 240);
        assert_eq!(
            hover_panel_position_in_bounds(
                bounds_position,
                bounds_size,
                PhysicalPosition::new(1500, 700),
                PhysicalSize::new(276, 46),
                panel_size,
                0,
            ),
            PhysicalPosition::new(1500, 460)
        );
        assert_eq!(
            hover_panel_position_in_bounds(
                bounds_position,
                bounds_size,
                PhysicalPosition::new(1500, 20),
                PhysicalSize::new(276, 46),
                panel_size,
                0,
            ),
            PhysicalPosition::new(1500, 66)
        );
    }

    #[test]
    fn drag_position_preserves_grab_ratio_and_clamps_ratios() {
        let size = PhysicalSize::new(200, 100);
        assert_eq!(
            drag_desired_position(500.0, 400.0, size, 0.25, 0.5),
            (450, 350)
        );
        assert_eq!(
            drag_desired_position(500.0, 400.0, size, -1.0, 2.0),
            (500, 300)
        );
    }

    #[test]
    fn clamp_handles_ranges_smaller_than_the_window() {
        assert_eq!(clamp_i32(500, 100, 50), 100);
    }

    #[test]
    fn windows_policy_removes_compact_edge_gap_and_allows_reserved_area() {
        assert_eq!(CompactPositionPolicy::Windows.edge_margin(), 0);
        assert!(CompactPositionPolicy::Windows.allows_reserved_area());
        assert_eq!(
            CompactPositionPolicy::Desktop.edge_margin(),
            DESKTOP_EDGE_MARGIN
        );
        assert!(!CompactPositionPolicy::Desktop.allows_reserved_area());
    }

    #[cfg(windows)]
    #[test]
    fn native_windows_uses_zero_margin_policy() {
        assert_eq!(
            CompactPositionPolicy::current(),
            CompactPositionPolicy::Windows
        );
        assert_eq!(CompactPositionPolicy::current().edge_margin(), 0);
    }

    #[cfg(not(windows))]
    #[test]
    fn native_non_windows_keeps_desktop_margin_policy() {
        assert_eq!(
            CompactPositionPolicy::current(),
            CompactPositionPolicy::Desktop
        );
        assert_eq!(
            CompactPositionPolicy::current().edge_margin(),
            DESKTOP_EDGE_MARGIN
        );
    }

    #[test]
    fn zero_margin_clamps_to_work_area_edges() {
        let work_position = PhysicalPosition::new(100, 50);
        let work_size = PhysicalSize::new(1920, 1040);
        let bubble_size = PhysicalSize::new(276, 46);

        let target =
            clamp_to_work_area(work_position, work_size, bubble_size, i64::MAX, i64::MAX, 0);

        assert_eq!(target, PhysicalPosition::new(1744, 1044));
    }

    #[test]
    fn desktop_margin_is_still_preserved() {
        let target = clamp_to_work_area(
            PhysicalPosition::new(-1920, 0),
            PhysicalSize::new(1920, 1080),
            PhysicalSize::new(276, 46),
            i64::MIN,
            i64::MAX,
            DESKTOP_EDGE_MARGIN,
        );

        assert_eq!(target, PhysicalPosition::new(-1908, 1022));
    }

    #[test]
    fn reserved_area_detection_covers_bottom_and_left_taskbars() {
        let screen_position = PhysicalPosition::new(0, 0);
        let screen_size = PhysicalSize::new(1920, 1080);
        assert!(!rect_overlaps_reserved_area(
            PhysicalPosition::new(100, 100),
            PhysicalSize::new(276, 46),
            screen_position,
            screen_size,
            PhysicalPosition::new(0, 0),
            PhysicalSize::new(1920, 1040),
        ));
        assert!(rect_overlaps_reserved_area(
            PhysicalPosition::new(100, 1034),
            PhysicalSize::new(276, 46),
            screen_position,
            screen_size,
            PhysicalPosition::new(0, 0),
            PhysicalSize::new(1920, 1040),
        ));
        assert!(rect_overlaps_reserved_area(
            PhysicalPosition::new(0, 100),
            PhysicalSize::new(276, 46),
            screen_position,
            screen_size,
            PhysicalPosition::new(48, 0),
            PhysicalSize::new(1872, 1080),
        ));
    }
}
