use core_foundation::runloop::{kCFRunLoopCommonModes, CFRunLoop};
use core_graphics::event::{
    CGEventTap, CGEventTapLocation, CGEventTapOptions, CGEventTapPlacement, CGEventType, EventField,
};
use crossbeam::channel::{unbounded, Receiver};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::thread;

use super::{EventTracker, EventTrackerHandles, TrackerEvent, TrackerEventType};

impl EventTrackerHandles for EventTracker {
    fn new() -> Self {
        let (sender, events) = unbounded();

        EventTracker {
            tracking_enabled: Arc::new(AtomicBool::new(false)),
            sender,
            events,
            is_inited: Arc::new(AtomicBool::new(false)),
        }
    }

    fn init(&self) {
        if self.is_inited.load(Ordering::SeqCst) {
            return;
        }
        let tracking_enabled_clone = Arc::clone(&self.tracking_enabled);
        let sender_arc = Arc::new(self.sender.clone());

        thread::spawn(move || {
            let current = CFRunLoop::get_current();
            match CGEventTap::new(
                CGEventTapLocation::HID,
                CGEventTapPlacement::HeadInsertEventTap,
                CGEventTapOptions::Default,
                vec![CGEventType::LeftMouseDown, CGEventType::RightMouseDown],
                |_a, _b, event| {
                    let tracking = tracking_enabled_clone.load(Ordering::SeqCst);
                    if tracking {
                        let pid =
                            event.get_integer_value_field(EventField::EVENT_TARGET_UNIX_PROCESS_ID);

                        #[cfg(debug_assertions)]
                        {
                            let window_id = event.get_integer_value_field(
                                EventField::MOUSE_EVENT_WINDOW_UNDER_MOUSE_POINTER,
                            );
                            let window_id_2 =
                                event.get_integer_value_field(EventField::MOUSE_EVENT_WINDOW_UNDER_MOUSE_POINTER_THAT_CAN_HANDLE_THIS_EVENT);
                            log::info!(
                                "Event: {:?} - {:?};  pid {}; wid {}; wid2 {}",
                                event.get_type(),
                                event.location(),
                                pid,
                                window_id,
                                window_id_2
                            );
                        }

                        let event = TrackerEvent {
                            pid,
                            event_type: TrackerEventType::from(event.get_type()),
                            location: (event.location().x, event.location().y),
                        };

                        if let Err(e) = sender_arc.send(event) {
                            log::error!("Failed to send event: {:?}", e);
                        }
                    }

                    None
                },
            ) {
                Ok(tap) => unsafe {
                    let loop_source = tap
                        .mach_port
                        .create_runloop_source(0)
                        .expect("Somethings is bad ");
                    current.add_source(&loop_source, kCFRunLoopCommonModes);
                    tap.enable();
                    CFRunLoop::run_current();
                },
                Err(_) => assert!(false),
            }
        });
        self.is_inited.store(true, Ordering::SeqCst);
    }

    fn enable_tracking(&self) {
        self.tracking_enabled.store(true, Ordering::SeqCst);
    }

    fn disable_tracking(&self) {
        self.tracking_enabled.store(false, Ordering::SeqCst);
    }

    fn events(&self) -> Receiver<TrackerEvent> {
        self.events.clone()
    }
}

impl From<CGEventType> for TrackerEventType {
    fn from(value: CGEventType) -> Self {
        return match value {
            CGEventType::ScrollWheel => TrackerEventType::ScrollWheel,
            CGEventType::LeftMouseDown => TrackerEventType::LeftMouseDown,
            CGEventType::RightMouseDown => TrackerEventType::RightMouseDown,
            CGEventType::LeftMouseDragged => TrackerEventType::LeftMouseDragged,
            CGEventType::RightMouseDragged => TrackerEventType::RightMouseDragged,
            _ => panic!("Unknown event type"),
        };
    }
}
