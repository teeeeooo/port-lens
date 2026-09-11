use crate::settings::{SettingsStore, WindowPosition};
use serde::{Deserialize, Serialize};
use std::sync::Mutex;
use tauri::{LogicalSize, Monitor, PhysicalPosition, PhysicalSize, WebviewWindow};

const BUBBLE_WIDTH: f64 = 276.0;
const BUBBLE_HEIGHT: f64 = 46.0;
const EDGE_MARGIN: i32 = 12;
const MIN_WIDTH: f64 = 800.0;
const MIN_HEIGHT: f64 = 580.0;

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
    expanded: Option<ExpandedWindow>,
}

#[derive(Debug, Default)]
pub struct BubbleController {
    state: Mutex<BubbleState>,
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

fn clamp_position(
    monitor: &Monitor,
    size: PhysicalSize<u32>,
    desired_x: i64,
    desired_y: i64,
) -> PhysicalPosition<i32> {
    let work = monitor.work_area();
    let min_x = work.position.x as i64 + EDGE_MARGIN as i64;
    let max_x =
        work.position.x as i64 + work.size.width as i64 - size.width as i64 - EDGE_MARGIN as i64;
    let min_y = work.position.y as i64 + EDGE_MARGIN as i64;
    let max_y =
        work.position.y as i64 + work.size.height as i64 - size.height as i64 - EDGE_MARGIN as i64;
    PhysicalPosition::new(
        clamp_i32(desired_x, min_x, max_x),
        clamp_i32(desired_y, min_y, max_y),
    )
}

fn default_collapsed_position(
    monitor: &Monitor,
    size: PhysicalSize<u32>,
    desired_y: i32,
) -> PhysicalPosition<i32> {
    let work = monitor.work_area();
    let desired_x =
        work.position.x as i64 + work.size.width as i64 - size.width as i64 - EDGE_MARGIN as i64;
    clamp_position(monitor, size, desired_x, desired_y as i64)
}

fn monitor_for_saved_position(
    window: &WebviewWindow,
    saved: WindowPosition,
) -> Result<Option<Monitor>, String> {
    window
        .monitor_from_point(saved.x as f64, saved.y as f64)
        .map_err(window_error)
}

fn apply_collapsed_window(
    window: &WebviewWindow,
    target: PhysicalPosition<i32>,
    size: PhysicalSize<u32>,
) -> Result<(), String> {
    window.set_min_size(Some(size)).map_err(window_error)?;
    window.set_max_size(Some(size)).map_err(window_error)?;
    window.set_resizable(false).map_err(window_error)?;
    window.set_decorations(false).map_err(window_error)?;
    window.set_shadow(false).map_err(window_error)?;
    window.set_always_on_top(true).map_err(window_error)?;
    let _ = window.set_skip_taskbar(true);
    window.set_size(size).map_err(window_error)?;
    window.set_position(target).map_err(window_error)?;
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
    state.collapsed = true;
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
    apply_collapsed_window(window, target, new_size)?;
    settings.update_compact_position(WindowPosition {
        x: target.x,
        y: target.y,
    })
}

pub fn move_to_cursor(
    window: &WebviewWindow,
    controller: &BubbleController,
    settings: &SettingsStore,
    offset: BubbleDragOffset,
) -> Result<BubblePayload, String> {
    if !is_collapsed(controller)? {
        return current(controller);
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
    window.set_min_size(Some(size)).map_err(window_error)?;
    window.set_max_size(Some(size)).map_err(window_error)?;
    window.set_size(size).map_err(window_error)?;
    window.set_position(target).map_err(window_error)?;
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
    fn clamp_handles_ranges_smaller_than_the_window() {
        assert_eq!(clamp_i32(500, 100, 50), 100);
    }
}
