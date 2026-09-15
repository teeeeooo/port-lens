use tauri::WebviewWindow;

const GRIP_X_LOGICAL: f64 = 5.0;
const GRIP_Y_LOGICAL: f64 = 5.0;
const GRIP_WIDTH_LOGICAL: f64 = 34.0;
const GRIP_HEIGHT_LOGICAL: f64 = 36.0;

#[derive(Debug)]
pub struct NativeDragSurface {
    #[cfg(windows)]
    hwnd: usize,
}

fn scaled_geometry(bubble_scale: f64, monitor_scale: f64) -> (i32, i32, i32, i32) {
    let scale = bubble_scale * monitor_scale;
    (
        (GRIP_X_LOGICAL * scale).round() as i32,
        (GRIP_Y_LOGICAL * scale).round() as i32,
        (GRIP_WIDTH_LOGICAL * scale).round().max(1.0) as i32,
        (GRIP_HEIGHT_LOGICAL * scale).round().max(1.0) as i32,
    )
}

#[cfg(windows)]
mod windows_impl {
    use super::{scaled_geometry, NativeDragSurface};
    use std::{ffi::c_void, mem};
    use tauri::{Manager, WebviewWindow};
    use windows::core::w;
    use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, POINT, WPARAM};
    use windows::Win32::UI::Input::KeyboardAndMouse::ReleaseCapture;
    use windows::Win32::UI::WindowsAndMessaging::{
        CallWindowProcW, CreateWindowExW, DefWindowProcW, DestroyWindow, GetCursorPos,
        GetWindowLongPtrW, PostMessageW, SetLayeredWindowAttributes, SetWindowLongPtrW,
        SetWindowPos, ShowWindow, GWLP_USERDATA, GWLP_WNDPROC, HTCAPTION, HTCLIENT, HWND_TOP,
        LWA_ALPHA, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE, SW_HIDE, SW_SHOW, WM_LBUTTONDOWN,
        WM_NCDESTROY, WM_NCHITTEST, WM_NCLBUTTONDOWN, WNDPROC, WS_CHILD, WS_EX_LAYERED,
    };

    struct DragSurfaceContext {
        parent: usize,
        hover: usize,
        original_proc: isize,
    }

    fn hwnd_from_raw(raw: usize) -> HWND {
        HWND(raw as *mut c_void)
    }

    fn pack_screen_point(point: POINT) -> LPARAM {
        let packed = (point.x as u16 as u32) | ((point.y as u16 as u32) << 16);
        LPARAM(packed as i32 as isize)
    }

    unsafe fn call_original(
        context: &DragSurfaceContext,
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        let original: WNDPROC = unsafe { mem::transmute(context.original_proc) };
        unsafe { CallWindowProcW(original, hwnd, msg, wparam, lparam) }
    }

    unsafe extern "system" fn drag_surface_proc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        let context_ptr =
            unsafe { GetWindowLongPtrW(hwnd, GWLP_USERDATA) } as *mut DragSurfaceContext;
        if context_ptr.is_null() {
            return unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) };
        }
        let context = unsafe { &*context_ptr };

        match msg {
            WM_NCHITTEST => LRESULT(HTCLIENT as isize),
            WM_LBUTTONDOWN => {
                if context.hover != 0 {
                    unsafe { ShowWindow(hwnd_from_raw(context.hover), SW_HIDE) };
                }
                let _ = unsafe { ReleaseCapture() };
                let mut point = POINT::default();
                if unsafe { GetCursorPos(&mut point) }.is_ok() {
                    let _ = unsafe {
                        PostMessageW(
                            Some(hwnd_from_raw(context.parent)),
                            WM_NCLBUTTONDOWN,
                            WPARAM(HTCAPTION as usize),
                            pack_screen_point(point),
                        )
                    };
                }
                LRESULT(0)
            }
            WM_NCDESTROY => {
                let result = unsafe { call_original(context, hwnd, msg, wparam, lparam) };
                unsafe { SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0) };
                unsafe { drop(Box::from_raw(context_ptr)) };
                result
            }
            _ => unsafe { call_original(context, hwnd, msg, wparam, lparam) },
        }
    }

    pub fn create(
        window: &WebviewWindow,
        bubble_scale: f64,
        monitor_scale: f64,
    ) -> Result<NativeDragSurface, String> {
        let parent = window
            .hwnd()
            .map_err(|error| format!("failed to resolve compact parent HWND: {error}"))?;
        let hover = window
            .app_handle()
            .get_webview_window("compact-hover")
            .and_then(|hover| hover.hwnd().ok())
            .map(|hwnd| hwnd.0 as usize)
            .unwrap_or(0);
        let (x, y, width, height) = scaled_geometry(bubble_scale, monitor_scale);
        let child = unsafe {
            CreateWindowExW(
                WS_EX_LAYERED,
                w!("STATIC"),
                w!(""),
                WS_CHILD,
                x,
                y,
                width,
                height,
                Some(parent),
                None,
                None,
                None,
            )
        }
        .map_err(|error| format!("failed to create native compact drag surface: {error}"))?;

        if let Err(error) = unsafe { SetLayeredWindowAttributes(child, COLORREF(0), 1, LWA_ALPHA) }
        {
            let _ = unsafe { DestroyWindow(child) };
            return Err(format!(
                "failed to make native compact drag surface transparent: {error}"
            ));
        }

        let original_proc = unsafe { GetWindowLongPtrW(child, GWLP_WNDPROC) };
        if original_proc == 0 {
            let _ = unsafe { DestroyWindow(child) };
            return Err("failed to resolve native compact drag surface window procedure".into());
        }
        let context = Box::new(DragSurfaceContext {
            parent: parent.0 as usize,
            hover,
            original_proc,
        });
        let context_ptr = Box::into_raw(context);
        unsafe { SetWindowLongPtrW(child, GWLP_USERDATA, context_ptr as isize) };
        let replaced_proc =
            unsafe { SetWindowLongPtrW(child, GWLP_WNDPROC, drag_surface_proc as usize as isize) };
        if replaced_proc != original_proc {
            unsafe { SetWindowLongPtrW(child, GWLP_USERDATA, 0) };
            unsafe { drop(Box::from_raw(context_ptr)) };
            let _ = unsafe { DestroyWindow(child) };
            return Err("failed to install native compact drag surface window procedure".into());
        }

        Ok(NativeDragSurface {
            hwnd: child.0 as usize,
        })
    }

    pub fn show(surface: &NativeDragSurface) {
        let hwnd = hwnd_from_raw(surface.hwnd);
        unsafe { ShowWindow(hwnd, SW_SHOW) };
        let _ = unsafe {
            SetWindowPos(
                hwnd,
                Some(HWND_TOP),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
            )
        };
    }

    pub fn resize(
        surface: &NativeDragSurface,
        bubble_scale: f64,
        monitor_scale: f64,
    ) -> Result<(), String> {
        let (x, y, width, height) = scaled_geometry(bubble_scale, monitor_scale);
        unsafe {
            SetWindowPos(
                hwnd_from_raw(surface.hwnd),
                Some(HWND_TOP),
                x,
                y,
                width,
                height,
                SWP_NOACTIVATE,
            )
        }
        .map_err(|error| format!("failed to resize native compact drag surface: {error}"))
    }

    pub fn destroy(surface: NativeDragSurface) {
        let _ = unsafe { DestroyWindow(hwnd_from_raw(surface.hwnd)) };
    }
}

pub fn create(
    window: &WebviewWindow,
    bubble_scale: f64,
    monitor_scale: f64,
) -> Result<Option<NativeDragSurface>, String> {
    #[cfg(windows)]
    {
        windows_impl::create(window, bubble_scale, monitor_scale).map(Some)
    }
    #[cfg(not(windows))]
    {
        let _ = (window, scaled_geometry(bubble_scale, monitor_scale));
        Ok(None)
    }
}

pub fn show(surface: Option<&NativeDragSurface>) {
    #[cfg(windows)]
    if let Some(surface) = surface {
        windows_impl::show(surface);
    }
    #[cfg(not(windows))]
    let _ = surface;
}

pub fn resize(
    surface: Option<&NativeDragSurface>,
    bubble_scale: f64,
    monitor_scale: f64,
) -> Result<(), String> {
    #[cfg(windows)]
    if let Some(surface) = surface {
        return windows_impl::resize(surface, bubble_scale, monitor_scale);
    }
    let _ = (surface, bubble_scale, monitor_scale);
    Ok(())
}

pub fn destroy(surface: Option<NativeDragSurface>) {
    #[cfg(windows)]
    if let Some(surface) = surface {
        windows_impl::destroy(surface);
    }
    #[cfg(not(windows))]
    let _ = surface;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_grip_geometry_matches_compact_css() {
        assert_eq!(scaled_geometry(1.0, 1.0), (5, 5, 34, 36));
        assert_eq!(scaled_geometry(0.7, 2.0), (7, 7, 48, 50));
    }
}
