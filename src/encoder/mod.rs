use std::ffi::c_void;
use std::time::Duration;

use cidre::arc::Retained;
use cidre::av::{
    asset::writer::Status, asset::WriterInputPixelBufAdaptor, video_settings_keys, AssetWriter,
    AssetWriterInput, FileType, MediaType, OutputSettingsAssistant, OutputSettingsPreset,
};
use cidre::cf;
use cidre::cm;
use cidre::cv::{pixel_buffer, PixelBuf, PixelFormat};
use cidre::ns::{Dictionary, Number};

pub struct VideoEncoder {
    writer: Retained<AssetWriter>,
    adaptor: Retained<WriterInputPixelBufAdaptor>,
    input: Retained<AssetWriterInput>,
    width: u32,
    height: u32,
}

impl VideoEncoder {
    pub fn new(width: u32, height: u32, out_file: &str) -> VideoEncoder {
        // log::info!("Before path");
        let path = std::path::Path::new(out_file);
        // log::info!("After path");
        let dst = cf::Url::with_path(path, false).unwrap();
        // log::info!("After dst");

        let mut writer = AssetWriter::with_url_and_file_type(dst.as_ns(), FileType::mp4()).unwrap();
        // log::info!("After writer");

        let assistant =
            OutputSettingsAssistant::with_preset(OutputSettingsPreset::h264_3840x2160())
                .expect("Failed to create output settings assistant");
        // log::info!("After assistant");
        let mut output_settings = assistant
            .video_settings()
            .expect("No assistant video settings")
            .copy_mut();
        // log::info!("After copy_mut");
        output_settings.insert(
            video_settings_keys::width(),
            Number::with_u32(width).as_id_ref(),
        );
        // log::info!("After insert width");
        output_settings.insert(
            video_settings_keys::height(),
            Number::with_u32(height).as_id_ref(),
        );
        // log::info!("After insert height");
        let mut input = AssetWriterInput::with_media_type_and_output_settings(
            MediaType::video(),
            Some(output_settings.as_ref()),
        )
        .expect("Failed to create asset writer input");
        input.set_expects_media_data_in_real_time(true);
        // log::info!("After input");

        let pixel_format = PixelFormat::_420_YP_CB_CR_8_BI_PLANAR_FULL_RANGE
            .to_cf_number()
            .as_ns()
            .as_id_ref();
        // log::info!("After pixel_format");
        let source_pixel_buffer_attributes = Dictionary::with_keys_values(
            &[pixel_buffer::keys::pixel_format().as_ns()],
            &[pixel_format],
        );
        // log::info!("After source_pixel_buffer_attributes");
        let adaptor = WriterInputPixelBufAdaptor::with_input_writer(
            &input,
            Some(source_pixel_buffer_attributes.as_ref()),
        )
        .expect("Failed to create asset writer input pixel buffer adaptor");
        // log::info!("After adaptor");

        if writer.can_add_input(&input) {
            // log::info!("Before add_input");
            writer.add_input(&input).expect("Failed to add input!");
            // log::info!("After add_input");
        }

        writer.start_writing();
        // log::info!("After start_writing");
        writer.start_session_at_src_time(cm::Time::zero());
        // log::info!("After start_session_at_src_time");

        VideoEncoder {
            writer,
            adaptor,
            input,
            width,
            height,
        }
    }

    pub fn ingest_bgra_frame(&mut self, display_time: u64, bgra_bytes_raw: &[u8]) {
        let pixel_buffer = self.create_cv_pixel_buffer_from_bgra_frame_data(
            self.width,
            self.height,
            bgra_bytes_raw,
        );
        log::info!(
            "After create_cv_pixel_buffer_from_bgra_frame_data: encoded {:?}, display {:?}",
            pixel_buffer.encoded_size(),
            pixel_buffer.display_size()
        );
        if self.input.is_ready_for_more_media_data() {
            log::info!("Before append_pixel_buf_with_pts");
            let frame_time = cm::Time::with_epoch(display_time as i64, 1_000_000_000, 0);
            log::info!("After frame_time: {:?}", frame_time);
            let result = self
                .adaptor
                .append_pixel_buf_with_pts(pixel_buffer.as_ref(), frame_time);
            log::info!("After append_pixel_buf_with_pts: {:?}", result);
            if let Err(_) = result {
                log::error!(
                    "AVAssetWriter: {}",
                    self.writer.error().unwrap().localized_description()
                );
            }
        } else {
            log::warn!("AVAssetWriter: not ready for more data");
        }
    }

    fn create_cv_pixel_buffer_from_bgra_frame_data(
        &self,
        width: u32,
        height: u32,
        bgra_bytes_raw: &[u8],
    ) -> Retained<PixelBuf> {
        // log::info!("Before create_cv_pixel_buffer_from_bgra_frame_data");
        let empty_dict = cf::Dictionary::new();
        // log::info!("After empty_dict");
        let pixel_format = cf::Number::from_four_char_code(PixelFormat::_32_BGRA.0);
        // log::info!("After pixel_format");
        let pixel_buffer_attributes = cf::Dictionary::with_keys_values(
            &[
                pixel_buffer::keys::io_surf_props(),
                pixel_buffer::keys::pixel_format(),
            ],
            &[&empty_dict, &pixel_format],
        );
        // log::info!("After pixel_buffer_attributes");
        let res = pixel_buffer::PixelBuf::with_bytes(
            width as usize,
            height as usize,
            bgra_bytes_raw.as_ptr() as *mut c_void,
            (width as usize) * 4,
            release_callback,
            std::ptr::null_mut(),
            PixelFormat::_32_BGRA,
            Some(&pixel_buffer_attributes.unwrap()),
        );
        // log::info!("After pixel_buffer");

        return res.unwrap();
    }

    pub fn finish(&mut self) {
        log::info!("Before mark as finished");
        self.input.mark_as_finished();
        log::info!("After mark as finished");
        self.writer.finish_writing();
        log::info!("After finishing writing");
        while self.writer.status() == Status::Writing {
            std::thread::sleep(Duration::from_millis(1000));
        }
        log::info!("After finished writing");
    }
}

extern "C" fn release_callback(release_ref_con: *mut c_void, base_address: *const *const c_void) {
    // println!("release_callback");
}
