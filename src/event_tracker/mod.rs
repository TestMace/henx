use crossbeam::channel::{Receiver, Sender};
use std::sync::{
    atomic::{AtomicBool, AtomicI32},
    Arc,
};

#[cfg(target_os = "macos")]
mod mac;

#[cfg(target_os = "windows")]
mod win;

#[derive(Debug)]
pub enum TrackerEventType {
    ScrollWheel,
    LeftMouseDown,
    RightMouseDown,
    LeftMouseDragged,
    RightMouseDragged,
    #[cfg(target_os = "windows")]
    LeftMouseUp,
    #[cfg(target_os = "windows")]
    RightMouseUp,
    Disable,
}

#[derive(Debug)]
pub struct TrackerEvent {
    pub pid: i64,
    pub event_type: TrackerEventType,
    pub location: (f64, f64),
}

pub struct EventTracker {
    sender: Sender<TrackerEvent>,
    pub events: Receiver<TrackerEvent>,
    pub tracking_enabled: Arc<AtomicBool>,
    pub is_inited: Arc<AtomicBool>,
    #[cfg(target_os = "windows")]
    pub last_click_location_x: Arc<AtomicI32>,
    #[cfg(target_os = "windows")]
    pub last_click_location_y: Arc<AtomicI32>,
}

pub trait EventTrackerHandles {
    fn new() -> Self;
    fn init(&self);
    fn enable_tracking(&self);
    fn disable_tracking(&self);
    fn events(&self) -> Receiver<TrackerEvent>;
}
