// This program is just a testbed for the library itself
// Refer to the lib.rs file for the actual implementation

// use henx::{VideoEncoder, VideoEncoderOptions};
mod capture;
mod encoder;
mod event_tracker;

use crate::capture::start;
use crate::encoder::VideoEncoder;

// use log::{info, trace, warn};

use scap::{
    capturer::{Capturer, Options},
    frame::{Frame, FrameType},
};

fn main() {
    colog::init();
    start();
}

fn old_capture() {
    if !scap::is_supported() {
        println!("❌ Platform not supported");
        return;
    }

    if !scap::has_permission() {
        println!("❌ Permission not granted");
        return;
    }

    // autoreleasepool!({
    let options = Options {
        fps: 60,
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

    let mut start_time = 0;

    let mut encoder = VideoEncoder::new(output_width, output_height, "output.mp4");

    capturer.start_capture();

    log::info!("Starting capture");

    for i in 0..1_000 {
        // autoreleasepool!({
        let frame = capturer.get_next_frame().expect("couldn't get next frame");
        match frame {
            Frame::BGRA(frame) => {
                if start_time == 0 {
                    start_time = frame.display_time;
                }
                let timestamp = frame.display_time - start_time;
                if frame.width > 0 && frame.height > 0 {
                    log::info!("new frame {}", i);
                    encoder.ingest_bgra_frame(timestamp as u64, frame.data.as_slice().into());
                }
            }
            _ => {
                log::error!("Wrong frame, {:?}", frame);
                break;
            }
        }

        // });
    }
    capturer.stop_capture();

    encoder.finish();

    log::info!("Finish capture");
    // });
}
