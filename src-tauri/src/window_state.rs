use crate::bubble::BubbleController;
use crate::settings::{AppSettings, SettingsStore, WindowBounds};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::time::Duration;
use tauri::{LogicalSize, Manager, Monitor, PhysicalPosition, WebviewWindow};

const MIN_WIDTH: f64 = 800.0;
const MIN_HEIGHT: f64 = 580.0;
const PERSIST_DELAY: Duration = Duration::from_millis(400);

#[derive(Default)]
pub struct WindowBoundsController {
    generation: AtomicU64,
    worker_running: AtomicBool,
}

fn window_error(error: tauri::Error) -> String {
    format!("window state operation failed: {error}")
}

pub fn restore_initial(window: &WebviewWindow, settings: &AppSettings) -> Result<(), String> {
    window
        .set_min_size(Some(LogicalSize::new(MIN_WIDTH, MIN_HEIGHT)))
        .map_err(window_error)?;
    if let Some(bounds) = settings.expanded_bounds {
        window
            .set_size(LogicalSize::new(bounds.width, bounds.height))
            .map_err(window_error)?;
        if saved_position_is_visible(window, bounds)? {
            window
                .set_position(PhysicalPosition::new(bounds.x, bounds.y))
                .map_err(window_error)?;
        }
    }
    fit_to_current_work_area(window)
}

fn fit_to_current_work_area(window: &WebviewWindow) -> Result<(), String> {
    let Some(monitor) = window.current_monitor().map_err(window_error)? else {
        return Ok(());
    };
    let work = monitor.work_area();
    let scale = monitor.scale_factor();
    let outer = window.outer_size().map_err(window_error)?;
    let inner = window.inner_size().map_err(window_error)?;
    let frame_width = outer.width.saturating_sub(inner.width);
    let frame_height = outer.height.saturating_sub(inner.height);
    let max_inner_width = work.size.width.saturating_sub(frame_width).max(1);
    let max_inner_height = work.size.height.saturating_sub(frame_height).max(1);
    let max_logical =
        tauri::PhysicalSize::new(max_inner_width, max_inner_height).to_logical::<f64>(scale);
    window
        .set_min_size(Some(LogicalSize::new(
            MIN_WIDTH.min(max_logical.width),
            MIN_HEIGHT.min(max_logical.height),
        )))
        .map_err(window_error)?;
    let target_inner_width = inner.width.min(max_inner_width);
    let target_inner_height = inner.height.min(max_inner_height);

    if target_inner_width != inner.width || target_inner_height != inner.height {
        window
            .set_size(
                tauri::PhysicalSize::new(target_inner_width, target_inner_height)
                    .to_logical::<f64>(scale),
            )
            .map_err(window_error)?;
    }

    let outer = window.outer_size().map_err(window_error)?;
    let position = window.outer_position().map_err(window_error)?;
    let min_x = work.position.x as i64;
    let min_y = work.position.y as i64;
    let max_x = min_x + work.size.width as i64 - outer.width as i64;
    let max_y = min_y + work.size.height as i64 - outer.height as i64;
    let target = PhysicalPosition::new(
        (position.x as i64).clamp(min_x, max_x.max(min_x)) as i32,
        (position.y as i64).clamp(min_y, max_y.max(min_y)) as i32,
    );
    if target != position {
        window.set_position(target).map_err(window_error)?;
    }
    Ok(())
}

pub fn schedule_persist(window: WebviewWindow) {
    let app = window.app_handle().clone();
    let controller = app.state::<WindowBoundsController>();
    controller.generation.fetch_add(1, Ordering::Release);
    if controller.worker_running.swap(true, Ordering::AcqRel) {
        return;
    }

    tauri::async_runtime::spawn(async move {
        loop {
            let controller = app.state::<WindowBoundsController>();
            let observed = controller.generation.load(Ordering::Acquire);
            tokio::time::sleep(PERSIST_DELAY).await;
            if controller.generation.load(Ordering::Acquire) != observed {
                continue;
            }

            let bubble_controller = app.state::<BubbleController>();
            if crate::bubble::is_collapsed(&bubble_controller).unwrap_or(false)
                && crate::bubble::is_compact_drag_input_active()
            {
                tokio::time::sleep(Duration::from_millis(50)).await;
                continue;
            }

            if let Some(window) = app.get_webview_window("main") {
                let _ = persist_now(&window);
            }

            controller.worker_running.store(false, Ordering::Release);
            if controller.generation.load(Ordering::Acquire) == observed {
                break;
            }
            if controller.worker_running.swap(true, Ordering::AcqRel) {
                break;
            }
        }
    });
}

pub fn persist_now(window: &WebviewWindow) -> Result<(), String> {
    let app = window.app_handle();
    let controller = app.state::<BubbleController>();
    let settings = app.state::<SettingsStore>();
    if crate::bubble::is_collapsed(&controller)? {
        let result = crate::bubble::persist_compact_position(window, &controller, &settings);
        let release = crate::bubble::set_native_move_active(&controller, false);
        return match (result, release) {
            (Ok(()), Ok(())) => Ok(()),
            (Err(error), _) => Err(error),
            (Ok(()), Err(error)) => Err(error),
        };
    }
    let Some(bounds) = capture_expanded_bounds(window)? else {
        return Ok(());
    };
    settings.update_expanded_bounds(bounds)
}

fn capture_expanded_bounds(window: &WebviewWindow) -> Result<Option<WindowBounds>, String> {
    if window.is_minimized().map_err(window_error)?
        || window.is_maximized().map_err(window_error)?
    {
        return Ok(None);
    }
    let scale = window.scale_factor().map_err(window_error)?;
    let position = window.outer_position().map_err(window_error)?;
    let size = window.outer_size().map_err(window_error)?;
    let logical = size.to_logical::<f64>(scale);
    if logical.width < MIN_WIDTH || logical.height < MIN_HEIGHT {
        return Ok(None);
    }
    Ok(Some(WindowBounds {
        x: position.x,
        y: position.y,
        width: logical.width.round(),
        height: logical.height.round(),
    }))
}

fn saved_position_is_visible(window: &WebviewWindow, bounds: WindowBounds) -> Result<bool, String> {
    let monitors = window.available_monitors().map_err(window_error)?;
    Ok(monitors
        .iter()
        .any(|monitor| bounds_intersect_monitor(bounds, monitor)))
}

fn bounds_intersect_monitor(bounds: WindowBounds, monitor: &Monitor) -> bool {
    let work = monitor.work_area();
    let width = (bounds.width * monitor.scale_factor()).round().max(1.0) as i64;
    let height = (bounds.height * monitor.scale_factor()).round().max(1.0) as i64;
    rects_intersect(
        (bounds.x as i64, bounds.y as i64, width, height),
        (
            work.position.x as i64,
            work.position.y as i64,
            work.size.width as i64,
            work.size.height as i64,
        ),
    )
}

fn rects_intersect(left: (i64, i64, i64, i64), right: (i64, i64, i64, i64)) -> bool {
    left.0 + left.2 > right.0
        && left.0 < right.0 + right.2
        && left.1 + left.3 > right.1
        && left.1 < right.1 + right.3
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rectangle_visibility_requires_real_overlap() {
        assert!(rects_intersect((10, 10, 100, 100), (0, 0, 50, 50)));
        assert!(!rects_intersect((50, 0, 100, 100), (0, 0, 50, 50)));
        assert!(!rects_intersect((-100, -100, 20, 20), (0, 0, 50, 50)));
    }
}
