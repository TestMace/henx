use scap::capturer::{Capturer, Options};
use scap::frame::{BGRAFrame, Frame, FrameType};

use crate::encoder::VideoEncoder;
use crate::event_tracker::{EventTracker, EventTrackerHandles, TrackerEventType};
use crossbeam::channel::{unbounded, Receiver, Sender};
use std::sync::{Arc, LazyLock, Mutex};
use std::thread::JoinHandle;
use std::{process, thread, time};

#[derive(Debug, Copy, Clone)]
enum CaptureStatus {
    Recording,
    Paused,
    Stopped,
}

struct CaptureState {
    status: Mutex<CaptureStatus>,
    step_num: Mutex<u32>,
    clicks: Mutex<Vec<(f64, f64)>>,
}

enum EncoderMessage {
    EncodeFrame(EncodeFrameMessage),
    Finish,
}

#[derive(Debug)]
struct EncodeFrameMessage {
    frame: BGRAFrame,
    step_num: u32,
    index: u32,
}

static CAPTURE_STATE: LazyLock<Arc<CaptureState>> = LazyLock::new(|| {
    Arc::new(CaptureState {
        status: Mutex::new(CaptureStatus::Stopped),
        step_num: Mutex::new(0),
        clicks: Mutex::new(vec![]),
    })
});

static EVENT_TRACKER_INST: LazyLock<Arc<Mutex<Option<EventTracker>>>> =
    LazyLock::new(|| Arc::new(Mutex::new(None)));

const FPS: u32 = 12;

pub fn start() {
    if !scap::is_supported() {
        println!("❌ Platform not supported");
        return;
    }

    if !scap::has_permission() {
        println!("❌ Permission not granted");
        return;
    }

    init_event_tracker();

    let tracker = EVENT_TRACKER_INST.lock().unwrap();
    tracker.as_ref().unwrap().enable_tracking();
    drop(tracker);

    set_capture_status(CaptureStatus::Recording);

    let events_stream_handle = listen_event_tracker();

    start_capture_frames();

    events_stream_handle.join().unwrap();
}

fn init_event_tracker() {
    let mut tracker = EVENT_TRACKER_INST.lock().unwrap_or_else(|err| {
        log::error!("Failed lock EVENT_TRACKER_INST: {err}");
        process::exit(1);
    });
    if tracker.is_none() {
        *tracker = Some(EventTracker::new());
    }
    tracker.as_ref().unwrap().init();
}

fn listen_event_tracker() -> thread::JoinHandle<()> {
    return thread::spawn(move || {
        loop {
            let tracker = EVENT_TRACKER_INST.lock().unwrap();
            if tracker.is_none() {
                log::warn!("Tracker is none, skip event tracking loop iteration",);
                break;
            }
            let events_stream = tracker.as_ref().unwrap().events();
            drop(tracker);
            let event = events_stream.recv().unwrap();

            // if if_stop_capture() {
            //     println!("I AM DEAD EVENT TRACKER");
            //     break;
            // }

            log::info!("Tracker event received - : {:?}", event);

            let mut step = CAPTURE_STATE.as_ref().step_num.lock().unwrap();
            let mut clicks = CAPTURE_STATE.as_ref().clicks.lock().unwrap();
            match event.event_type {
                TrackerEventType::LeftMouseDown | TrackerEventType::RightMouseDown => {
                    clicks.push(event.location);
                    *step += 1;
                }
                TrackerEventType::Disable => {
                    println!("I AM DEAD EVENT TRACKER");
                    break;
                }
                _ => {}
            }
        }
    });
}

fn set_capture_status(status: CaptureStatus) {
    let capture_state = CAPTURE_STATE.as_ref();
    let mut s = capture_state.status.lock().unwrap();
    *s = status;
    log::info!("Capture status set to {:?}", status);
}

fn start_capture_frames() {
    thread::spawn(move || {
        let mut recorder = prepare_recorder();
        recorder.start_capture();

        let mut prev_step: u32 = 0;

        let mut step_frame_counter = 0;
        let mut current_sender: Option<&Sender<EncoderMessage>> = None;
        let mut senders: Vec<Sender<EncoderMessage>> = vec![];
        let mut encoder_threads: Vec<JoinHandle<()>> = vec![];
        let mut steps_to_finish: Vec<u32> = vec![];

        // frames loop
        loop {
            let frame = recorder.get_next_frame().expect("Error getting next frame");
            if if_paused_capture() {
                // todo fix timestamp on resume
                continue;
            }
            if if_stop_capture() {
                log::info!("Frames loop ended");
                break;
            }

            match frame {
                Frame::BGRA(frame) => {
                    if frame.width == 0 || frame.height == 0 {
                        continue;
                    }
                    let step_num = CAPTURE_STATE.as_ref().step_num.lock().unwrap();
                    let curr_step_num = *step_num;
                    drop(step_num);

                    let click_happened = prev_step != curr_step_num;
                    if click_happened {
                        log::info!(
                            "New step, start new encoder thread ({:?} -> {:?})",
                            prev_step,
                            curr_step_num
                        );
                        step_frame_counter = 1;
                        let (frames_sender, frames_receiver) = unbounded::<EncoderMessage>();
                        let encoder_thread = spawn_encoder_thread(
                            frames_receiver,
                            curr_step_num, // Use curr_step_num for both screenshot and video
                            frame.width as u32,
                            frame.height as u32,
                        );
                        encoder_threads.push(encoder_thread);
                        if prev_step != 0 {
                            // stop prev encoder
                            match current_sender {
                                Some(sender) => {
                                    sender.send(EncoderMessage::Finish).unwrap();
                                }
                                None => {}
                            }
                        }
                        senders.push(frames_sender);
                        current_sender = senders.last();
                        prev_step = curr_step_num;
                    } else {
                        if current_sender.is_none() {
                            continue;
                        }
                        step_frame_counter += 1;
                        current_sender
                            .as_ref()
                            .unwrap()
                            .send(EncoderMessage::EncodeFrame(EncodeFrameMessage {
                                frame: frame.clone(),
                                step_num: curr_step_num, // Use curr_step_num here
                                index: step_frame_counter,
                            }))
                            .unwrap();
                        log::info!(
                            "Sent frame to encoder thread, s[{}], i[{}]",
                            curr_step_num,
                            step_frame_counter
                        );
                        steps_to_finish.push(curr_step_num); // Use curr_step_num here
                    }
                }
                _ => {
                    log::error!("Wrong frame, {:?}", frame);
                    break;
                }
            }
        }

        for sender in senders {
            sender
                .send(EncoderMessage::Finish)
                .expect("Failed to send finish message");
            drop(sender);
        }

        for thread in encoder_threads {
            thread.join().expect("Failed to join encoder thread");
        }

        log::info!(">>> Recording to be stopped");
        recorder.stop_capture();
        log::info!(">>> Recording stopped");
    });
}

fn spawn_encoder_thread(
    receiver: Receiver<EncoderMessage>,
    step_num: u32,
    width: u32,
    height: u32,
) -> thread::JoinHandle<()> {
    let encoder_thread = thread::spawn(move || {
        log::info!("Spawning encoder thread for step: {:?}", step_num);
        let video_path = format!("{}.mp4", step_num);
        #[cfg(target_os = "windows")]
        let mut encoder = WVideoEncoder::new(
            VideoSettingsBuilder::new(width, height).sub_type(VideoSettingsSubType::H264),
            AudioSettingsBuilder::default().disabled(true),
            ContainerSettingsBuilder::default(),
            video_path,
        )
        .expect("Failed to create video encoder");
        #[cfg(target_os = "macos")]
        let mut encoder = VideoEncoder::new(width, height, &video_path);
        log::info!("Encoder initialized for step: {:?}", step_num);
        let mut start_time = 0;
        let mut finish_on_empty = false;
        let mut step_number = 0;
        let mut finish_encoder_flag = false;
        let mut has_video = false;
        loop {
            if finish_on_empty && receiver.is_empty() {
                finish_encoder_flag = true;
                break;
            }
            match receiver.recv() {
                Ok(message) => match message {
                    EncoderMessage::EncodeFrame(frame_message) => {
                        let EncodeFrameMessage {
                            frame,
                            step_num,
                            index,
                        } = frame_message;
                        log::info!(
                            "Encode frame message received: s[{:?}] idx[{}], {:?}x{:?}",
                            step_num,
                            index,
                            frame.width,
                            frame.height
                        );

                        step_number = step_num;

                        if frame.width == 0 || frame.height == 0 {
                            continue;
                        }

                        if start_time == 0 {
                            start_time = frame.display_time;
                        }
                        let timestamp = frame.display_time - start_time;

                        has_video = true;
                        #[cfg(target_os = "windows")]
                        {
                            let timestamp_nanos = std::time::Duration::from_nanos(timestamp);
                            let timestamp_micros = timestamp_nanos.as_micros() as i64;
                            let timestamp_micros_10 = timestamp_micros * 10;
                            let buffer = flip_image_vertical_bgra(
                                &frame.data,
                                width as usize,
                                height as usize,
                            );

                            encoder
                                .send_frame_buffer(&buffer, timestamp_micros_10)
                                .expect("failed to send frame");
                        }

                        #[cfg(target_os = "macos")]
                        {
                            if frame.data.len() > 0 {
                                log::info!("BGRA frame length: {}", frame.data.len());
                                encoder.ingest_bgra_frame(
                                    timestamp as u64,
                                    frame.data.as_slice().into(),
                                );
                                log::info!("BGRA frame sent to encoder");
                                drop(frame);
                                log::info!("BGRA frame dropped");
                            } else {
                                log::info!("BGRA frame data is empty");
                            }
                        }
                    }
                    EncoderMessage::Finish => {
                        finish_on_empty = true;
                        log::info!(
                            "Encoder thread marked as finishing for step: {:?}",
                            step_num
                        );
                    }
                },
                Err(err) => {
                    log::error!(
                        "Error reading encoder messages, step {}, err: {:?}",
                        step_number,
                        err
                    );
                    finish_encoder_flag = true;
                    break;
                }
            }
        }
        // loop end

        if finish_encoder_flag && has_video {
            log::info!("Finishing encoder for step: {:?}", step_number);
            #[cfg(target_os = "windows")]
            {
                if let Err(e) = encoder.finish() {
                    log::error!("Failed to finish encoder {}: {}", step_number, e);
                }
            }

            #[cfg(target_os = "macos")]
            {
                encoder.finish();
                log::info!("Encoder finished for step {}", step_number);
            }
        }
    });

    encoder_thread
}

fn prepare_recorder() -> Capturer {
    let options = Options {
        fps: FPS,
        target: None,
        show_cursor: true,
        show_highlight: true,
        output_type: FrameType::BGRAFrame,
        output_resolution: scap::capturer::Resolution::Captured,
        ..Default::default()
    };

    let mut capturer = Capturer::build(options).unwrap();
    let [output_width, output_height] = capturer.get_output_frame_size();
    println!(
        "output_width: {}, output_height: {}",
        output_width, output_height
    );

    return capturer;
}

fn if_paused_capture() -> bool {
    let capture_state = CAPTURE_STATE.as_ref();
    let status = capture_state.status.lock().unwrap();
    match *status {
        CaptureStatus::Paused => true,
        _ => false,
    }
}

fn if_stop_capture() -> bool {
    let capture_state = CAPTURE_STATE.as_ref();
    let status = capture_state.status.lock().unwrap();
    match *status {
        CaptureStatus::Recording => false,
        _ => {
            log::info!("Capture will be stopped");
            true
        }
    }
}
