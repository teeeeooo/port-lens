#[cfg(windows)]
pub const DRAG_STARTED_EVENT: &str = "port-lens://compact-native-drag-started";
#[cfg(windows)]
pub const DRAG_ENDED_EVENT: &str = "port-lens://compact-native-drag-ended";

#[cfg(windows)]
mod imp {
    use super::{DRAG_ENDED_EVENT, DRAG_STARTED_EVENT};
    use crate::{bubble, diagnostics::Diagnostics, settings::SettingsStore};
    use std::sync::atomic::{AtomicBool, AtomicIsize, Ordering};
    use std::sync::mpsc::{channel, Sender};
    use std::sync::OnceLock;
    use std::thread;
    use tauri::{AppHandle, Emitter, Manager, WebviewWindow};
    use windows::core::BOOL;
    use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
    use windows::Win32::UI::Input::KeyboardAndMouse::ReleaseCapture;
    use windows::Win32::UI::Shell::{DefSubclassProc, SetWindowSubclass};
    use windows::Win32::UI::WindowsAndMessaging::{
        EnumChildWindows, GetMessagePos, GetWindowRect, SendMessageW, ShowWindowAsync, HTCAPTION,
        SW_HIDE, WM_LBUTTONDOWN, WM_NCLBUTTONDOWN,
    };

    const SUBCLASS_ID: usize = 0x504c_4452; // "PLDR"
    const OPEN_REGION_FRACTION: f64 = 60.0 / 276.0;

    static COMPACT_ACTIVE: AtomicBool = AtomicBool::new(false);
    static DRAG_ACTIVE: AtomicBool = AtomicBool::new(false);
    static HOVER_HWND: AtomicIsize = AtomicIsize::new(0);
    static EVENT_SENDER: OnceLock<Sender<DragEvent>> = OnceLock::new();

    #[derive(Debug, Clone, Copy)]
    enum DragEvent {
        Started,
        Ended,
    }

    fn send_event(event: DragEvent) {
        if let Some(sender) = EVENT_SENDER.get() {
            let _ = sender.send(event);
        }
    }

    fn point_from_message_pos(message_pos: u32) -> POINT {
        POINT {
            x: (message_pos as u16 as i16) as i32,
            y: ((message_pos >> 16) as u16 as i16) as i32,
        }
    }

    fn message_pos_lparam(message_pos: u32) -> LPARAM {
        LPARAM(message_pos as i32 as isize)
    }

    unsafe fn point_is_open_region(main_hwnd: HWND, point: POINT) -> bool {
        let mut rect = RECT::default();
        if GetWindowRect(main_hwnd, &mut rect).is_err() {
            return true;
        }
        let width = rect.right.saturating_sub(rect.left);
        if width <= 0 {
            return true;
        }
        let local_x = point.x.saturating_sub(rect.left);
        let open_width = ((width as f64) * OPEN_REGION_FRACTION).ceil() as i32;
        local_x >= width.saturating_sub(open_width)
    }

    unsafe extern "system" fn compact_mouse_subclass(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
        _subclass_id: usize,
        ref_data: usize,
    ) -> LRESULT {
        if msg != WM_LBUTTONDOWN || !COMPACT_ACTIVE.load(Ordering::Acquire) {
            return DefSubclassProc(hwnd, msg, wparam, lparam);
        }

        let main_hwnd = HWND(ref_data as *mut _);
        let message_pos = GetMessagePos();
        let point = point_from_message_pos(message_pos);
        if point_is_open_region(main_hwnd, point) {
            return DefSubclassProc(hwnd, msg, wparam, lparam);
        }

        DRAG_ACTIVE.store(true, Ordering::Release);
        let hover_raw = HOVER_HWND.load(Ordering::Acquire);
        if hover_raw != 0 {
            let _ = ShowWindowAsync(HWND(hover_raw as *mut _), SW_HIDE);
        }
        send_event(DragEvent::Started);

        let _ = ReleaseCapture();
        let _ = SendMessageW(
            main_hwnd,
            WM_NCLBUTTONDOWN,
            Some(WPARAM(HTCAPTION as usize)),
            Some(message_pos_lparam(message_pos)),
        );

        DRAG_ACTIVE.store(false, Ordering::Release);
        send_event(DragEvent::Ended);
        LRESULT(0)
    }

    struct InstallContext {
        main_hwnd: HWND,
        installed_children: usize,
    }

    unsafe extern "system" fn subclass_child(hwnd: HWND, lparam: LPARAM) -> BOOL {
        let context = &mut *(lparam.0 as *mut InstallContext);
        if SetWindowSubclass(
            hwnd,
            Some(compact_mouse_subclass),
            SUBCLASS_ID,
            context.main_hwnd.0 as usize,
        )
        .as_bool()
        {
            context.installed_children += 1;
        }
        true.into()
    }

    fn install_subclasses(window: &WebviewWindow) -> Result<usize, String> {
        let main_hwnd = window
            .hwnd()
            .map_err(|error| format!("Failed to resolve compact HWND: {error}"))?;
        let hover_hwnd = window
            .app_handle()
            .get_webview_window("compact-hover")
            .and_then(|hover| hover.hwnd().ok())
            .map(|hwnd| hwnd.0 as isize)
            .unwrap_or(0);
        HOVER_HWND.store(hover_hwnd, Ordering::Release);

        unsafe {
            if !SetWindowSubclass(
                main_hwnd,
                Some(compact_mouse_subclass),
                SUBCLASS_ID,
                main_hwnd.0 as usize,
            )
            .as_bool()
            {
                return Err("Failed to install compact drag subclass on main HWND.".into());
            }
            let mut context = InstallContext {
                main_hwnd,
                installed_children: 0,
            };
            let _ = EnumChildWindows(
                Some(main_hwnd),
                Some(subclass_child),
                LPARAM((&mut context as *mut InstallContext) as isize),
            );
            Ok(context.installed_children)
        }
    }

    pub fn initialize(app: &AppHandle) {
        if EVENT_SENDER.get().is_some() {
            return;
        }
        let (sender, receiver) = channel::<DragEvent>();
        if EVENT_SENDER.set(sender).is_err() {
            return;
        }
        let app = app.clone();
        thread::spawn(move || {
            while let Ok(event) = receiver.recv() {
                match event {
                    DragEvent::Started => {
                        let _ = app.emit_to("main", DRAG_STARTED_EVENT, ());
                    }
                    DragEvent::Ended => {
                        let app_for_main = app.clone();
                        let _ = app.run_on_main_thread(move || {
                            let Some(window) = app_for_main.get_webview_window("main") else {
                                return;
                            };
                            let controller = app_for_main.state::<bubble::BubbleController>();
                            let settings = app_for_main.state::<SettingsStore>();
                            if bubble::is_collapsed(&controller).unwrap_or(false) {
                                if let Err(error) = bubble::persist_compact_position(
                                    &window,
                                    &controller,
                                    &settings,
                                ) {
                                    app_for_main.state::<Diagnostics>().record(
                                        "WARN",
                                        "compact_drag_persist",
                                        format!("error={error}"),
                                    );
                                }
                            }
                            let _ = app_for_main.emit_to("main", DRAG_ENDED_EVENT, ());
                        });
                    }
                }
            }
        });
    }

    pub fn install(window: &WebviewWindow) -> Result<(), String> {
        let installed_children = install_subclasses(window)?;
        let diagnostics = window.app_handle().state::<Diagnostics>();
        diagnostics.record(
            if installed_children == 0 {
                "WARN"
            } else {
                "INFO"
            },
            "compact_drag_hook",
            format!("childSubclasses={installed_children}"),
        );
        Ok(())
    }

    pub fn set_compact_active(active: bool) {
        COMPACT_ACTIVE.store(active, Ordering::Release);
        if !active {
            DRAG_ACTIVE.store(false, Ordering::Release);
        }
    }

    pub fn is_drag_active() -> bool {
        DRAG_ACTIVE.load(Ordering::Acquire)
    }
}

#[cfg(windows)]
pub use imp::{initialize, install, is_drag_active, set_compact_active};

#[cfg(not(windows))]
pub fn initialize(_app: &tauri::AppHandle) {}
#[cfg(not(windows))]
pub fn install(_window: &tauri::WebviewWindow) -> Result<(), String> {
    Ok(())
}
#[cfg(not(windows))]
pub fn set_compact_active(_active: bool) {}
#[cfg(not(windows))]
pub fn is_drag_active() -> bool {
    false
}
