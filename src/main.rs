// This program is just a testbed for the library itself
// Refer to the lib.rs file for the actual implementation

use henx::{VideoEncoder, VideoEncoderOptions};
use scap::{
    capturer::{Capturer, Options},
    frame::FrameType,
};
use swift_rs::autoreleasepool;

fn main() {
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
    
        let mut capturer = Capturer::new(options);
        let [output_width, output_height] = capturer.get_output_frame_size();
        println!("output_width: {}, output_height: {}", output_width, output_height);
    
        let mut encoder = VideoEncoder::new(VideoEncoderOptions {
            width: output_width as usize,
            height: output_height as usize,
            path: "output.mp4".to_string(),
        });
    
        capturer.start_capture();

        for _ in 0..1_000 {
            // autoreleasepool!({
            let frame = capturer.get_next_frame().expect("couldn't get next frame");
            encoder
                .ingest_next_frame(&frame)
                .expect("frame couldn't be encoded");
            // });
        }
        capturer.stop_capture();

        encoder.finish().expect("failed to finish encoding");
    // });

    
}
