// use swift_rs::swift;
// pub use swift_rs::{Int, SRData, SRString};

// swift!(pub fn encoder_init(
//     width: Int,
//     height: Int,
//     out_file: SRString
// ) -> *mut std::ffi::c_void);

// swift!(pub fn encoder_ingest_yuv_frame(
//     enc: *mut std::ffi::c_void,
//     width: Int,
//     height: Int,
//     display_time: Int,
//     luminance_stride: Int,
//     luminance_bytes: SRData,
//     chrominance_stride: Int,
//     chrominance_bytes: SRData
// ));

// swift!(pub fn encoder_ingest_bgra_frame(
//     enc: *mut std::ffi::c_void,
//     width: Int,
//     height: Int,
//     display_time: Int,
//     bytes_per_row: Int,
//     bgra_bytes_raw: SRData
// ));

// swift!(pub fn encoder_finish(enc: *mut std::ffi::c_void));

use std::ffi::c_void;
use std::time::Duration;

use cidre::av::{AssetWriter, asset::writer::Status, AssetWriterInput, asset::WriterInputPixelBufAdaptor, OutputSettingsPreset, FileType, MediaType, video_settings_keys, OutputSettingsAssistant};
use cidre::ns::{Dictionary, Number};
use cidre::cv::{pixel_buffer, PixelFormat, PixelBuf};
use cidre::arc::Retained;
use cidre::cf;
use cidre::cm;

pub struct Encoder {
    writer: Retained<AssetWriter>,
    adaptor: Retained<WriterInputPixelBufAdaptor>,
    input: Retained<AssetWriterInput>,
    width: u32,
    height: u32,
}

impl Encoder {
    pub fn new(width: u32, height: u32, out_file: &str) -> Encoder {
        let path = std::path::Path::new(out_file);
        let dst = cf::Url::with_path(path, false).unwrap();
    
        let mut writer = AssetWriter::with_url_and_file_type(dst.as_ns(), FileType::mp4()).unwrap();
    
    
        let assistant = OutputSettingsAssistant::with_preset(OutputSettingsPreset::h264_3840x2160())
            .expect("Failed to create output settings assistant");
    
        let mut output_settings = assistant
            .video_settings()
            .expect("No assistant video settings")
            .copy_mut();
    
        output_settings.insert(
            video_settings_keys::width(),
            Number::with_u32(width).as_id_ref(),
        );
    
        output_settings.insert(
            video_settings_keys::height(),
            Number::with_u32(height).as_id_ref(),
        );
    
        let mut input = AssetWriterInput::with_media_type_and_output_settings(MediaType::video(), Some(output_settings.as_ref())).expect("Failed to create asset writer input");
        input.set_expects_media_data_in_real_time(true);
    
        let pixel_format = PixelFormat::_420_YP_CB_CR_8_BI_PLANAR_FULL_RANGE.to_cf_number().as_ns().as_id_ref();
        let mut source_pixel_buffer_attributes = Dictionary::with_keys_values(
            &[pixel_buffer::keys::pixel_format().as_ns()],
            &[pixel_format]
        );
        
        let adaptor = WriterInputPixelBufAdaptor::with_input_writer(&input, Some(source_pixel_buffer_attributes.as_ref())).expect("Failed to create asset writer input pixel buffer adaptor");
        
        if writer.can_add_input(&input) {
            writer.add_input(&input);
        }
    
        writer.start_writing();
        writer.start_session_at_src_time(cm::Time::zero());
    
        Encoder {
            writer,
            adaptor,
            input,
            width,
            height,
        }
    }

    pub fn ingest_bgra_frame(&mut self, display_time: u64, bgra_bytes_raw: &[u8]) {
        //let adaptor = enc.get_pixel_buffer_adaptor().expect("Failed to get pixel buffer adaptor");
        //adaptor.ingest_pixel_buffer(pixel_buffer, display_time);
    
        let pixel_buffer = self.create_cv_pixel_buffer_from_bgra_frame_data(self.width, self.height, bgra_bytes_raw);
        if self.input.is_ready_for_more_media_data() {
            let frame_time = cm::Time::with_epoch(display_time as i64, 1_000_000_000, 0);
            let result = self.adaptor.append_pixel_buf_with_pts(pixel_buffer.as_ref(), frame_time);
            if let Err(error) = result {
                println!("AVAssetWriter: {}", self.writer.error().unwrap().localized_description());
            }
        } else {
            println!("AVAssetWriter: not ready for more data");
        }
    }

    fn create_cv_pixel_buffer_from_bgra_frame_data(&self,width: u32, height: u32, bgra_bytes_raw: &[u8]) -> Retained<PixelBuf> {
        let empty_dict = cf::Dictionary::new();
        let pixel_format = cf::Number::from_four_char_code(PixelFormat::_32_BGRA.0);
        let pixel_buffer_attributes = cf::Dictionary::with_keys_values(&[
            pixel_buffer::keys::io_surf_props(),
            pixel_buffer::keys::pixel_format()
        ], &[
            &empty_dict,
            &pixel_format
        ]);
        let res = pixel_buffer::PixelBuf::with_bytes(
            width as usize,
            height as usize,
            bgra_bytes_raw.as_ptr() as *mut c_void,
            (width as usize) * 4,
            release_callback,
            std::ptr::null_mut(),
            PixelFormat::_32_BGRA,
            Some(&pixel_buffer_attributes.unwrap())
        );
        
        
        return res.unwrap();
    }

    pub fn finish(&mut self) {
        self.input.mark_as_finished();
        self.writer.finish_writing();
        self.writer.status();
        while self.writer.status() == Status::Writing {
            std::thread::sleep(Duration::from_millis(1000));
        }
    }
}


extern "C" fn release_callback(release_ref_con: *mut c_void, base_address: *const *const c_void) {
    println!("release_callback");
}
