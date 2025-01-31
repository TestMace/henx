use crossbeam::channel::{unbounded, Receiver, Sender};
use std::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use std::sync::Arc;
use std::thread;
use windows::Win32::Foundation::{LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, DispatchMessageW, GetMessageW, GetWindowThreadProcessId, SetWindowsHookExW,
    TranslateMessage, UnhookWindowsHookEx, WindowFromPoint, HHOOK, MSG, MSLLHOOKSTRUCT,
    WH_MOUSE_LL, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE, WM_MOUSEWHEEL, WM_RBUTTONDOWN,
    WM_RBUTTONUP,
};

use windows::Win32::UI::Input::KeyboardAndMouse::{GetAsyncKeyState, VK_LBUTTON, VK_RBUTTON};

use super::{EventTracker, EventTrackerHandles, TrackerEvent, TrackerEventType};

struct HookContext {
    tracking_enabled: Arc<AtomicBool>,
    sender: Arc<Sender<TrackerEvent>>,
    last_click_location_x: Arc<AtomicI32>,
    last_click_location_y: Arc<AtomicI32>,
}

unsafe extern "system" fn mouse_proc(code: i32, w_param: WPARAM, l_param: LPARAM) -> LRESULT {
    if code < 0 {
        return CallNextHookEx(HHOOK::default(), code, w_param, l_param);
    }

    // Get the context from the user data
    let context_ptr = GetHookContext();
    if let Some(context) = context_ptr {
        let tracking = (*context).tracking_enabled.load(Ordering::SeqCst);
        let last_click_location_x = (*context).last_click_location_x.load(Ordering::SeqCst);
        let last_click_location_y = (*context).last_click_location_y.load(Ordering::SeqCst);

        if tracking {
            let event_type = match w_param.0 as u32 {
                WM_MOUSEWHEEL => TrackerEventType::ScrollWheel,
                WM_LBUTTONDOWN => TrackerEventType::LeftMouseDown,
                WM_RBUTTONDOWN => TrackerEventType::RightMouseDown,
                WM_LBUTTONUP => TrackerEventType::LeftMouseUp,
                WM_RBUTTONUP => TrackerEventType::RightMouseUp,
                WM_MOUSEMOVE => {
                    // Check if mouse buttons are being held during movement
                    if (GetAsyncKeyState(VK_LBUTTON.0 as i32) & 0x8000_u16 as i16) != 0 {
                        TrackerEventType::LeftMouseDragged
                    } else if (GetAsyncKeyState(VK_RBUTTON.0 as i32) & 0x8000_u16 as i16) != 0 {
                        TrackerEventType::RightMouseDragged
                    } else {
                        // If no buttons are held, ignore the move event
                        return CallNextHookEx(HHOOK::default(), code, w_param, l_param);
                    }
                }
                _ => return CallNextHookEx(HHOOK::default(), code, w_param, l_param),
            };

            // Rest of your existing code...
            let mouse_struct = *(l_param.0 as *const MSLLHOOKSTRUCT);
            let (x, y) = (mouse_struct.pt.x as f64, mouse_struct.pt.y as f64);

            match event_type {
                TrackerEventType::LeftMouseDown | TrackerEventType::RightMouseDown => {
                    (*context)
                        .last_click_location_x
                        .store(x as i32, Ordering::SeqCst);
                    (*context)
                        .last_click_location_y
                        .store(y as i32, Ordering::SeqCst);
                }
                TrackerEventType::LeftMouseUp | TrackerEventType::RightMouseUp => {
                    (*context).last_click_location_x.store(0, Ordering::SeqCst);
                    (*context).last_click_location_y.store(0, Ordering::SeqCst);
                    return CallNextHookEx(HHOOK::default(), code, w_param, l_param);
                }
                TrackerEventType::LeftMouseDragged | TrackerEventType::RightMouseDragged => {
                    if (last_click_location_x == 0
                        || last_click_location_y == 0
                        || (x - last_click_location_x as f64).abs() < 10.0
                        || (y - last_click_location_y as f64).abs() < 10.0)
                    {
                        return CallNextHookEx(HHOOK::default(), code, w_param, l_param);
                    }
                }
                _ => {}
            }

            let mut pid: u32 = 0;
            let hwnd = WindowFromPoint(POINT {
                x: x as i32,
                y: y as i32,
            });

            if hwnd.0 != std::ptr::null_mut() {
                let _thread_id = GetWindowThreadProcessId(hwnd, Some(&mut pid));
            }

            let event = TrackerEvent {
                pid: pid as i64,
                event_type,
                location: (x, y),
            };

            if let Err(e) = (*context).sender.send(event) {
                log::error!("Failed to send event: {:?}", e);
            }
        }
    }

    CallNextHookEx(HHOOK::default(), code, w_param, l_param)
}

impl EventTrackerHandles for EventTracker {
    fn new(_app: Arc<AppHandle>) -> Self {
        let (sender, events) = unbounded();

        EventTracker {
            tracking_enabled: Arc::new(AtomicBool::new(false)),
            sender,
            events,
            is_inited: Arc::new(AtomicBool::new(false)),
            last_click_location_x: Arc::new(AtomicI32::new(0)),
            last_click_location_y: Arc::new(AtomicI32::new(0)),
        }
    }

    fn init(&self) {
        if (self.is_inited.load(Ordering::SeqCst)) {
            return;
        }
        let tracking_enabled = Arc::clone(&self.tracking_enabled);
        let sender = Arc::new(self.sender.clone());
        let last_click_location_x = Arc::clone(&self.last_click_location_x);
        let last_click_location_y = Arc::clone(&self.last_click_location_y);

        let context = Box::new(HookContext {
            tracking_enabled,
            last_click_location_x,
            last_click_location_y,
            sender,
        });

        thread::spawn(move || unsafe {
            // Store context for the hook procedure
            SetHookContext(Box::into_raw(context));

            let hook = SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_proc), None, 0)
                .expect("Failed to set mouse hook");

            let mut msg = MSG::default();
            while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }

            UnhookWindowsHookEx(hook);
            // Clean up context
            let _ = Box::from_raw(GetHookContext().unwrap());
        });
        self.is_inited.store(true, Ordering::SeqCst);
    }

    fn enable_tracking(&self) {
        self.tracking_enabled.store(true, Ordering::SeqCst);
    }

    fn disable_tracking(&self) {
        // self.sender.send(TrackerEvent {
        //     pid: 0,
        //     event_type: TrackerEventType::Disable,
        //     location: (0.0, 0.0),
        // });
        self.tracking_enabled.store(false, Ordering::SeqCst);
    }

    fn events(&self) -> Receiver<TrackerEvent> {
        self.events.clone()
    }
}

// Global storage for hook context
static mut HOOK_CONTEXT: *mut HookContext = std::ptr::null_mut();

unsafe fn SetHookContext(context: *mut HookContext) {
    HOOK_CONTEXT = context;
}

unsafe fn GetHookContext() -> Option<*mut HookContext> {
    if HOOK_CONTEXT.is_null() {
        None
    } else {
        Some(HOOK_CONTEXT)
    }
}
