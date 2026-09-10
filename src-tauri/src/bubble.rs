use serde::Serialize;
use std::sync::Mutex;
use tauri::{LogicalSize, PhysicalPosition, PhysicalSize, WebviewWindow};

const BUBBLE_WIDTH: f64 = 276.0;
const BUBBLE_HEIGHT: f64 = 46.0;
const EDGE_MARGIN: i32 = 12;
const MIN_WIDTH: f64 = 800.0;
const MIN_HEIGHT: f64 = 580.0;

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

fn window_error(error: tauri::Error) -> String {
    format!("bubble window operation failed: {error}")
}

pub fn current(controller: &BubbleController) -> Result<BubblePayload, String> {
    let state = controller
        .state
        .lock()
        .map_err(|_| "Bubble state lock is poisoned.".to_string())?;
    Ok(BubblePayload {
        collapsed: state.collapsed,
    })
}

fn clamp_i32(value: i64, min: i64, max: i64) -> i32 {
    value.clamp(min, max.max(min)) as i32
}

fn collapsed_position(
    work_x: i32,
    work_y: i32,
    work_width: u32,
    work_height: u32,
    bubble_width: u32,
    bubble_height: u32,
    desired_y: i32,
) -> PhysicalPosition<i32> {
    let x = work_x as i64 + work_width as i64 - bubble_width as i64 - EDGE_MARGIN as i64;
    let min_y = work_y as i64 + EDGE_MARGIN as i64;
    let max_y = work_y as i64 + work_height as i64 - bubble_height as i64 - EDGE_MARGIN as i64;
    PhysicalPosition::new(
        x.max(work_x as i64 + EDGE_MARGIN as i64) as i32,
        clamp_i32(desired_y as i64, min_y, max_y),
    )
}

pub fn collapse(
    window: &WebviewWindow,
    controller: &BubbleController,
) -> Result<BubblePayload, String> {
    let mut state = controller
        .state
        .lock()
        .map_err(|_| "Bubble state lock is poisoned.".to_string())?;
    if state.collapsed {
        return Ok(BubblePayload { collapsed: true });
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
    let monitor = window
        .current_monitor()
        .map_err(window_error)?
        .ok_or_else(|| "No monitor is available for the compact bubble.".to_string())?;
    let scale = monitor.scale_factor();
    let work = monitor.work_area();
    let bubble_size = PhysicalSize::new(
        (BUBBLE_WIDTH * scale).round() as u32,
        (BUBBLE_HEIGHT * scale).round() as u32,
    );
    let desired_y = position.y + (size.height as i32 - bubble_size.height as i32) / 2;
    let target = collapsed_position(
        work.position.x,
        work.position.y,
        work.size.width,
        work.size.height,
        bubble_size.width,
        bubble_size.height,
        desired_y,
    );

    window
        .set_min_size(Some(bubble_size))
        .map_err(window_error)?;
    window
        .set_max_size(Some(bubble_size))
        .map_err(window_error)?;
    window.set_resizable(false).map_err(window_error)?;
    window.set_decorations(false).map_err(window_error)?;
    window.set_shadow(false).map_err(window_error)?;
    window.set_always_on_top(true).map_err(window_error)?;
    let _ = window.set_skip_taskbar(true);
    window.set_size(bubble_size).map_err(window_error)?;
    window.set_position(target).map_err(window_error)?;
    window.show().map_err(window_error)?;

    state.collapsed = true;
    Ok(BubblePayload { collapsed: true })
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
    fn collapsed_position_docks_right_and_clamps_vertical_position() {
        let position = collapsed_position(0, 24, 1440, 876, 276, 46, 1200);
        assert_eq!(position.x, 1152);
        assert_eq!(position.y, 842);
    }

    #[test]
    fn collapsed_position_respects_nonzero_monitor_origin() {
        let position = collapsed_position(-1920, 0, 1920, 1080, 276, 46, 400);
        assert_eq!(position.x, -288);
        assert_eq!(position.y, 400);
    }
}
