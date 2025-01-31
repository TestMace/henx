// #[cfg(target_os = "macos")]
// mod mac;

// #[cfg(target_os = "macos")]
// use mac::{encoder_finish, encoder_ingest_bgra_frame, encoder_ingest_yuv_frame, encoder_init, Int};
// #[cfg(target_os = "macos")]
// use swift_rs::autoreleasepool;
// #[cfg(target_os = "windows")]
// use windows_capture::encoder::{AudioSettingsBuilder, ContainerSettingsBuilder, ContainerSettingsSubType, VideoSettingsBuilder, VideoSettingsSubType};
// #[cfg(target_os = "windows")]
// use windows_capture::encoder::{
//     VideoEncoder as WVideoEncoder,
// };

// use anyhow::Error;
// use scap::frame::Frame;

// mod utils;

// pub struct VideoEncoder {
//     first_timestamp: u64,

//     #[cfg(target_os = "macos")]
//     encoder: *mut std::ffi::c_void,

//     #[cfg(target_os = "windows")]
//     encoder: Option<WVideoEncoder>,
// }

// #[derive(Debug)]
// pub struct VideoEncoderOptions {
//     pub width: usize,
//     pub height: usize,
//     pub path: String,
// }

// fn convert_bgra_to_rgb(frame_data: &Vec<u8>) -> Vec<u8> {
//     let width = frame_data.len();
//     let width_without_alpha = (width / 4) * 3;

//     let mut data: Vec<u8> = vec![0; width_without_alpha];

//     for (src, dst) in frame_data.chunks_exact(4).zip(data.chunks_exact_mut(3)) {
//         dst[0] = src[2];
//         dst[1] = src[1];
//         dst[2] = src[0];
//     }

//     data
// }

// fn convert_bgra_to_rgba_and_flip(rgba_data: &[u8], width: usize, height: usize) -> Vec<u8> {
//     let mut bgra_data = vec![0; rgba_data.len()];
//     for y in 0..height {
//         for x in 0..width {
//             let rgba_index = (y * width + x) * 4;
//             let bgra_index = ((height - y - 1) * width + x) * 4;
//             // 转换为 BGRA
//             bgra_data[bgra_index] = rgba_data[rgba_index]; // B
//             bgra_data[bgra_index + 1] = rgba_data[rgba_index + 1]; // G
//             bgra_data[bgra_index + 2] = rgba_data[rgba_index + 2]; // R
//             bgra_data[bgra_index + 3] = rgba_data[rgba_index + 3]; // A
//         }
//     }
//     bgra_data
// }

// impl VideoEncoder {
//     pub fn new(options: VideoEncoderOptions) -> Self {
//         #[cfg(target_os = "windows")]
//         let encoder = Some(
//             WVideoEncoder::new(
//                 VideoSettingsBuilder::new(options.width as u32, options.height as u32).sub_type(VideoSettingsSubType::H264),
//                 AudioSettingsBuilder::default().disabled(true),
//                 ContainerSettingsBuilder::default(),
//                 options.path,
//             )
//             .expect("Failed to create video encoder"),
//         );

//         #[cfg(target_os = "macos")]
//         let encoder = unsafe {
//             encoder_init(
//                 options.width as Int,
//                 options.height as Int,
//                 options.path.as_str().into(),
//             )
//         };

//         Self {
//             encoder,
//             first_timestamp: 0,
//         }
//     }

//     pub fn ingest_next_frame(&mut self, frame: &Frame) -> Result<(), Error> {
//         match frame {
//             Frame::BGRA(frame) => {
//                 if self.first_timestamp == 0 {
//                     self.first_timestamp = frame.display_time;
//                 }

//                 let timestamp = frame.display_time - self.first_timestamp;

//                 #[cfg(target_os = "windows")]
//                 {
//                     let timestamp_nanos = std::time::Duration::from_nanos(timestamp);

//                     // TODO: why does the magic number 10 work here?
//                     let timestamp_micros = timestamp_nanos.as_micros() as i64;
//                     let timestamp_micros_10 = timestamp_micros * 10;
//                     println!("Vector length {}", frame.data.len());

//                     let buffer = utils::flip_image_vertical_bgra(
//                         &frame.data,
//                         frame.width as usize,
//                         frame.height as usize,
//                     );

//                     if self.encoder.is_some() {
//                         self.encoder
//                             .as_mut()
//                             .unwrap()
//                             .send_frame_buffer(&buffer, timestamp_micros_10)
//                             .expect("failed to send frame");
//                     }
//                 }

//                 #[cfg(target_os = "macos")]
//                 unsafe {
//                     autoreleasepool!({
//                         // println!("BGRA frame length: {}", frame.data.len());
//                         if frame.data.len() > 0 {
//                             encoder_ingest_bgra_frame(
//                                 self.encoder,
//                                 frame.width as Int,
//                                 frame.height as Int,
//                                 timestamp as Int,
//                                 frame.width as Int,
//                                 frame.data.as_slice().into(),
//                             );
//                             drop(frame);
//                         } else {
//                             println!("BGRA frame data is empty");
//                         }

//                     });
//                 }
//             }
//             Frame::YUVFrame(frame) => {
//                 println!("YUV frame timestamp: {}, {}, {}", frame.display_time, frame.width, frame.height);
//                 #[cfg(target_os = "macos")]
//                 {
//                     if self.first_timestamp == 0 {
//                         self.first_timestamp = frame.display_time;
//                     }

//                     let timestamp = frame.display_time - self.first_timestamp;

//                     #[cfg(target_os = "macos")]
//                     unsafe {
//                         autoreleasepool!({
//                             encoder_ingest_yuv_frame(
//                                 self.encoder,
//                                 frame.width as Int,
//                                 frame.height as Int,
//                                 timestamp as Int,
//                                 frame.luminance_stride as Int,
//                                 frame.luminance_bytes.as_slice().into(),
//                                 frame.chrominance_stride as Int,
//                                 frame.chrominance_bytes.as_slice().into(),
//                             );
//                             drop(frame);
//                         })
//                     }
//                 }
//             }
//             _ => {
//                 println!("henx doesn't support this pixel format yet")
//             }
//         }

//         Ok(())
//     }

//     pub fn finish(&mut self) -> Result<(), Error> {
//         #[cfg(target_os = "windows")]
//         {
//             self.encoder
//                 .take()
//                 .unwrap()
//                 .finish()
//                 .expect("Failed to finish encoding");
//         }

//         #[cfg(target_os = "macos")]
//         unsafe {
//             encoder_finish(self.encoder);
//         }
//         Ok(())
//     }
// }
