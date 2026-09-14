use crate::settings::{SettingsStore, WindowPosition};
use serde::{Deserialize, Serialize};
use std::sync::{Arc, Mutex};
use tauri::{LogicalSize, Monitor, PhysicalPosition, PhysicalSize, WebviewWindow};

#[cfg(windows)]
use std::{thread, time::Duration};
#[cfg(windows)]
use windows::Win32::UI::WindowsAndMessaging::{
    SetWindowPos, HWND_TOPMOST, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
};

const BUBBLE_WIDTH: f64 = 276.0;
const BUBBLE_HEIGHT: f64 = 46.0;
const HOVER_ROW_HEIGHT: f64 = 28.0;
const HOVER_LIST_PADDING: f64 = 8.0;
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
    bubble_physical_size_for_rows(bubble_scale, monitor_scale, 0)
}

fn bubble_physical_size_for_rows(
    bubble_scale: f64,
    monitor_scale: f64,
    rows: u32,
) -> PhysicalSize<u32> {
    let visible_rows = rows.min(MAX_HOVER_ROWS);
    let hover_height = if visible_rows == 0 {
        0.0
    } else {
        HOVER_LIST_PADDING + HOVER_ROW_HEIGHT * visible_rows as f64
    };
    PhysicalSize::new(
        (BUBBLE_WIDTH * bubble_scale * monitor_scale)
            .round()
            .max(1.0) as u32,
        ((BUBBLE_HEIGHT + hover_height) * bubble_scale * monitor_scale)
            .round()
            .max(1.0) as u32,
    )
}

#[derive(Debug, Clone, Copy)]
struct ExpandedWindow {
    position: PhysicalPosition<i32>,
    size: PhysicalSize<u32>,
    maximized: bool,
    always_on_top: bool,
}

#[derive(Debug, Default)]
struct BubbleState {
    collapsed: bool,
    hover_rows: u32,
    hover_origin: Option<PhysicalPosition<i32>>,
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

#[derive(Debug, Clone, Copy, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BubbleDragOffset {
    pub offset_ratio_x: Option<f64>,
    pub offset_ratio_y: Option<f64>,
    pub persist: Option<bool>,
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

fn bottom_anchored_position(
    monitor: &Monitor,
    position: PhysicalPosition<i32>,
    old_size: PhysicalSize<u32>,
    new_size: PhysicalSize<u32>,
) -> PhysicalPosition<i32> {
    clamp_position(
        monitor,
        new_size,
        position.x as i64,
        position.y as i64 + old_size.height as i64 - new_size.height as i64,
    )
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
        let _ = refresh_taskbar_z_order(&window);
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

pub fn collapse(
    window: &WebviewWindow,
    controller: &BubbleController,
    settings: &SettingsStore,
) -> Result<BubblePayload, String> {
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
    let size = window.outer_size().map_err(window_error)?;
    let maximized = window.is_maximized().map_err(window_error)?;
    let always_on_top = window.is_always_on_top().map_err(window_error)?;
    state.expanded = Some(ExpandedWindow {
        position,
        size,
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
        let desired_y = position.y + (size.height as i32 - bubble_size.height as i32) / 2;
        default_collapsed_position(&monitor, bubble_size, desired_y)
    };

    apply_collapsed_window(window, target, bubble_size)?;
    settings.update_compact_position(WindowPosition {
        x: target.x,
        y: target.y,
    })?;
    state.hover_rows = 0;
    state.hover_origin = None;
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

pub fn set_hover_rows(
    window: &WebviewWindow,
    controller: &BubbleController,
    settings: &SettingsStore,
    rows: u32,
) -> Result<BubblePayload, String> {
    let (current_rows, hover_origin) = {
        let state = controller
            .state
            .lock()
            .map_err(|_| "Bubble state lock is poisoned.".to_string())?;
        if !state.collapsed {
            return Ok(BubblePayload { collapsed: false });
        }
        (state.hover_rows, state.hover_origin)
    };
    let next_rows = rows.min(MAX_HOVER_ROWS);
    if current_rows == next_rows {
        return current(controller);
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
        .ok_or_else(|| "No monitor is available for compact hover.".to_string())?;
    let new_size = bubble_physical_size_for_rows(
        current_settings.bubble_scale,
        monitor.scale_factor(),
        next_rows,
    );
    let target = if next_rows == 0 {
        let origin = hover_origin.unwrap_or(position);
        clamp_position(&monitor, new_size, origin.x as i64, origin.y as i64)
    } else {
        bottom_anchored_position(&monitor, position, old_size, new_size)
    };
    set_compact_geometry(window, target, new_size)?;
    refresh_taskbar_z_order(window)?;

    let mut state = controller
        .state
        .lock()
        .map_err(|_| "Bubble state lock is poisoned.".to_string())?;
    if current_rows == 0 && next_rows > 0 {
        state.hover_origin = Some(position);
    } else if next_rows == 0 {
        state.hover_origin = None;
    }
    state.hover_rows = next_rows;
    drop(state);
    current(controller)
}

pub fn move_to_cursor(
    window: &WebviewWindow,
    controller: &BubbleController,
    settings: &SettingsStore,
    offset: BubbleDragOffset,
) -> Result<BubblePayload, String> {
    {
        let mut state = controller
            .state
            .lock()
            .map_err(|_| "Bubble state lock is poisoned.".to_string())?;
        if !state.collapsed {
            return Ok(BubblePayload { collapsed: false });
        }
        state.hover_rows = 0;
        state.hover_origin = None;
    }
    let cursor = window.cursor_position().map_err(window_error)?;
    let monitor = window
        .monitor_from_point(cursor.x, cursor.y)
        .map_err(window_error)?
        .or_else(|| window.current_monitor().ok().flatten())
        .ok_or_else(|| "No monitor is available while moving the compact bubble.".to_string())?;
    let current_settings = settings.get()?;
    let size = bubble_physical_size(current_settings.bubble_scale, monitor.scale_factor());
    let ratio_x = offset.offset_ratio_x.unwrap_or(0.5).clamp(0.0, 1.0);
    let ratio_y = offset.offset_ratio_y.unwrap_or(0.5).clamp(0.0, 1.0);
    let target = clamp_position(
        &monitor,
        size,
        (cursor.x - ratio_x * size.width as f64).round() as i64,
        (cursor.y - ratio_y * size.height as f64).round() as i64,
    );
    set_compact_geometry(window, target, size)?;
    refresh_taskbar_z_order(window)?;
    if offset.persist.unwrap_or(false) {
        settings.update_compact_position(WindowPosition {
            x: target.x,
            y: target.y,
        })?;
    }
    current(controller)
}

pub fn expand(
    window: &WebviewWindow,
    controller: &BubbleController,
    focus: bool,
) -> Result<BubblePayload, String> {
    let mut state = controller
        .state
        .lock()
        .map_err(|_| "Bubble state lock is poisoned.".to_string())?;
    if !state.collapsed {
        window.show().map_err(window_error)?;
        if window.is_minimized().map_err(window_error)? {
            window.unminimize().map_err(window_error)?;
        }
        if focus {
            window.set_focus().map_err(window_error)?;
        }
        return Ok(BubblePayload { collapsed: false });
    }

    let expanded = state
        .expanded
        .ok_or_else(|| "No expanded window state is available.".to_string())?;
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
    window.set_size(expanded.size).map_err(window_error)?;
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
    state.hover_rows = 0;
    state.hover_origin = None;
    state.z_order_generation = state.z_order_generation.wrapping_add(1);
    state.collapsed = false;
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
    fn hover_size_grows_for_apps_and_caps_at_eight_rows() {
        assert_eq!(
            bubble_physical_size_for_rows(1.0, 1.0, 3),
            PhysicalSize::new(276, 138)
        );
        assert_eq!(
            bubble_physical_size_for_rows(1.0, 1.0, 8),
            PhysicalSize::new(276, 278)
        );
        assert_eq!(
            bubble_physical_size_for_rows(1.0, 1.0, 20),
            PhysicalSize::new(276, 278)
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
