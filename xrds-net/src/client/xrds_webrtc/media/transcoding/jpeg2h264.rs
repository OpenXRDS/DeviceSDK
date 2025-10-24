use ffmpeg_next::{self as ffmpeg, Rational};
use ffmpeg::{codec, format, util::error::Error, decoder, encoder, color, Dictionary, frame, Packet};
use ffmpeg::encoder::find;
extern crate pretty_env_logger;
extern crate log;

pub struct H264Packet {
    pub data: Vec<u8>,
    pub pts: i64,
    pub dts: i64,
    pub is_keyframe: bool,
}

pub struct Jpeg2H264Transcoder {
    width: u32,
    height: u32,
    encoder: codec::encoder::video::Encoder,
    frame_count: u32,
}

#[allow(dead_code)]
impl Jpeg2H264Transcoder {
    pub fn new(width: u32, height: u32, fps: u32) -> Result<Self, Error> {
        ffmpeg::init().unwrap();
        unsafe {
            ffmpeg::ffi::av_log_set_level(ffmpeg::ffi::AV_LOG_ERROR);
        }

        // H.264 encoder setup with more flexible settings
        let encoder_codec = find(codec::Id::H264)
            .ok_or(Error::EncoderNotFound)?;
        let encoder_ctx = codec::context::Context::new_with_codec(encoder_codec);

        let fps_rational = Rational::new(fps as i32, 1);

        let mut encoder_setting = encoder_ctx.encoder().video()?;
        encoder_setting.set_width(width);
        encoder_setting.set_height(height);
        encoder_setting.set_format(format::Pixel::YUV420P);

        let time_base = Rational::new(1, fps as i32);
        encoder_setting.set_time_base(time_base);
        encoder_setting.set_frame_rate(Some(fps_rational));

        encoder_setting.set_bit_rate(8_000_000); // Reduced bitrate
        encoder_setting.set_max_bit_rate(12_000_000);
        encoder_setting.set_qmin(18); // Higher qmin for stability
        encoder_setting.set_qmax(28); // Lower qmax for stability
        encoder_setting.set_gop(30); 
        encoder_setting.set_color_range(color::Range::MPEG);
        encoder_setting.set_colorspace(color::Space::BT709);
        
        // Use minimal options for maximum compatibility
        let mut x264_opt = Dictionary::new();
        x264_opt.set("preset", "ultrafast");  // Fastest preset for stability
        x264_opt.set("r", &fps.to_string());
        x264_opt.set("g", "30");
        
        println!("🔧 Encoder settings: {}x{} @ {}fps, time_base={}/{}, GOP=30", 
             width, height, fps, time_base.numerator(), time_base.denominator());

        let opened_encoder = encoder_setting.open_with(x264_opt)?;

        Ok(Jpeg2H264Transcoder {
            width,
            height,
            encoder: opened_encoder,
            frame_count: 0,
        })
    }

    /// Decode JPEG bytes using fresh decoder - with better error reporting
    #[allow(dead_code)]
    fn decode_jpeg_bytes(&self, jpeg_bytes: &[u8]) -> Result<frame::Video, Error> {
        if jpeg_bytes.len() < 10 {
            log::error!("JPEG data too small: {} bytes", jpeg_bytes.len());
            return Err(Error::InvalidData);
        }
        
        // Check JPEG magic bytes
        if jpeg_bytes.len() >= 2 && (jpeg_bytes[0] != 0xFF || jpeg_bytes[1] != 0xD8) {
            log::error!("Invalid JPEG magic bytes: {:02X} {:02X}", jpeg_bytes[0], jpeg_bytes[1]);
            return Err(Error::InvalidData);
        }
        
        let decoder_codec = decoder::find(codec::Id::MJPEG)
            .ok_or(Error::DecoderNotFound)?;
        let decoder_ctx = codec::context::Context::new_with_codec(decoder_codec);
        let mut decoder = decoder_ctx.decoder().video()?;

        let mut jpeg_packet = Packet::new(jpeg_bytes.len());
        if let Some(data) = jpeg_packet.data_mut() {
            data.copy_from_slice(jpeg_bytes);
        }

        decoder.send_packet(&jpeg_packet)?;

        let mut decoded_frame = frame::Video::empty();
        match decoder.receive_frame(&mut decoded_frame) {
            Ok(()) => {
                decoded_frame.set_color_range(color::Range::JPEG);
                decoded_frame.set_color_space(color::Space::BT709);

                log::trace!("✅ Decoded JPEG: {}x{} {:?}", 
                           decoded_frame.width(), decoded_frame.height(), decoded_frame.format());
                
                Ok(decoded_frame)
            }
            Err(e) => {
                log::error!("JPEG decode failed: {:?} (size: {} bytes)", e, jpeg_bytes.len());
                Err(e)
            }
        }
    }

    /*
    ** This is the main function to transcode a single JPEG image to H.264 packets.
    ** input: Single JPEG image bytes
    ** output: Vector of H264Packet structs (can be empty if encoder is buffering)
    ** Handles encoder delay and multiple output packets.
    ** It can produce 0, 1, or multiple packets per input frame.
    */
    pub fn transcode_jpeg_to_h264_packet(&mut self, jpeg_bytes: &[u8]) -> Result<Vec<H264Packet>, Error> {
        let decoded_frame = self.decode_jpeg_bytes(jpeg_bytes)?;
        let mut yuv420_frame = self.convert_frame_format_strict(&decoded_frame)?;

        let pts = self.frame_count as i64;
        yuv420_frame.set_pts(Some(pts));
        self.frame_count += 1;

        self.encoder.send_frame(&yuv420_frame)?;

        let mut packets = Vec::new();
        let mut h264_packet = Packet::empty();
        
        while self.encoder.receive_packet(&mut h264_packet).is_ok() {
            if let Some(data) = h264_packet.data() {
                let is_keyframe = h264_packet.is_key();

                let packet_pts = h264_packet.pts().unwrap_or(pts);
                let packet_dts = h264_packet.dts().unwrap_or(pts);
                packets.push(H264Packet {
                    data: data.to_vec(),
                    pts: packet_pts,
                    dts: packet_dts,
                    is_keyframe,
                });

                log::trace!("Generated H.264 packet: PTS={}, DTS={}, size={}, keyframe={}", 
                           packet_pts, packet_dts, data.len(), is_keyframe);
            }
        }

        Ok(packets)
    }

    /**
     * Used by write_h264_packets_to_mp4 and flush_to_packets
     * From YUVJ422P (JPEG decoded) to YUV420P (H.264 encoder)
     */
    fn convert_frame_format_strict(&self, src_frame: &frame::Video) -> Result<frame::Video, Error> {
        use ffmpeg::software::scaling::{context::Context, flag::Flags};

        log::trace!("Converting: {}x{} {:?} -> {}x{} YUV420P", 
                   src_frame.width(), src_frame.height(), src_frame.format(),
                   self.width, self.height);

        // ALWAYS scale to exact target dimensions, even if source matches
        // This ensures consistent frame properties for the encoder
        let scaling_flags = Flags::BILINEAR | Flags::FULL_CHR_H_INT | Flags::ACCURATE_RND;
        
        let mut scaler = Context::get(
            src_frame.format(),
            src_frame.width(),
            src_frame.height(),
            format::Pixel::YUV420P,
            self.width,
            self.height,
            scaling_flags,
        )?;

        // Set colorspace conversion for JPEG (full range) -> MPEG (limited range)
        unsafe {
            use ffmpeg::ffi::*;
            let sws_ctx = scaler.as_mut_ptr();
            let coeffs = sws_getCoefficients(SWS_CS_ITU709);
            
            if !coeffs.is_null() {
                let ret = sws_setColorspaceDetails(
                    sws_ctx,
                    coeffs, 1,  // Input: JPEG full range (0-255)
                    coeffs, 0,  // Output: MPEG limited range (16-235)
                    0,          // Brightness
                    1 << 16,    // Contrast
                    1 << 16,    // Saturation
                );
                
                if ret >= 0 {
                    log::trace!("✅ Set colorspace conversion: full->limited range");
                }
            }
        }

        let mut converted = frame::Video::empty();
        scaler.run(src_frame, &mut converted)?;
        
        // Verify output dimensions match exactly
        if converted.width() != self.width || converted.height() != self.height {
            return Err(Error::Bug); // This should never happen
        }
        
        // Set consistent frame properties
        converted.set_color_range(color::Range::MPEG);
        converted.set_color_space(color::Space::BT709);
        
        // Set consistent color primaries and transfer characteristics
        converted.set_color_primaries(color::Primaries::BT709);
        converted.set_color_transfer_characteristic(color::TransferCharacteristic::BT709);
        
        log::trace!("✅ Strict conversion: {}x{} {:?} (range: {:?})", 
                   converted.width(), converted.height(), converted.format(),
                   converted.color_range());
        
        Ok(converted)
    }

    #[allow(dead_code)]
    fn convert_frame_format(&self, src_frame: &frame::Video) -> Result<frame::Video, Error> {
        use ffmpeg::software::scaling::{context::Context, flag::Flags};

        log::debug!("Converting (flexible): {}x{} {:?} -> YUV420P", 
                   src_frame.width(), src_frame.height(), src_frame.format());

        // Handle YUVJ* formats
        let src_format = match src_frame.format() {
            format::Pixel::YUVJ420P => format::Pixel::YUV420P,
            format::Pixel::YUVJ422P => format::Pixel::YUV422P,
            format::Pixel::YUVJ444P => format::Pixel::YUV444P,
            other => other,
        };

        // Keep source dimensions to avoid scaling artifacts
        let mut scaler = Context::get(
            src_format,
            src_frame.width(),
            src_frame.height(),
            format::Pixel::YUV420P,
            src_frame.width(),   // Keep same width
            src_frame.height(),  // Keep same height
            Flags::BILINEAR,
        )?;

        let mut converted = frame::Video::empty();
        scaler.run(src_frame, &mut converted)?;
        
        // Set consistent properties
        converted.set_color_range(color::Range::MPEG);
        converted.set_color_space(color::Space::BT709);
        
        log::debug!("✅ Flexible conversion: {}x{} {:?}", 
                   converted.width(), converted.height(), converted.format());
        
        Ok(converted)
    }

    /**
     * This function is to verify if h264 packets are valid by writing them to an MP4 file.
     * It takes a slice of H264Packet structs and writes them to the specified output path.
     * The width, height, and fps parameters are used to set up the MP4 container
     * and stream correctly.
     */
    pub fn write_h264_packets_to_mp4 (
        h264_packets: &[H264Packet], 
        output_path: &str, 
        width: u32, 
        height: u32, 
        fps: u32
    ) -> Result<(), Error> {
        let mut octx = format::output(output_path)?;
        let codec = encoder::find(codec::Id::H264).ok_or(Error::EncoderNotFound)?;
        let mut ost = octx.add_stream(codec)?;

        let fps_rational = Rational::new(fps as i32, 1);
        let time_base = fps_rational.invert(); // 1/30
        ost.set_time_base(time_base);

        println!("🎬 MP4 settings: {}x{} @ {}fps", width, height, fps);
        println!("📹 Time base: {}/{}", time_base.numerator(), time_base.denominator());

        unsafe {
            let stream = ost.as_mut_ptr();
            (*stream).codecpar.as_mut().map(|codecpar| {
                (*codecpar).codec_type = ffmpeg::ffi::AVMediaType::AVMEDIA_TYPE_VIDEO;
                (*codecpar).codec_id = ffmpeg::ffi::AVCodecID::AV_CODEC_ID_H264;
                (*codecpar).width = width as i32;
                (*codecpar).height = height as i32;
                (*codecpar).format = ffmpeg::ffi::AVPixelFormat::AV_PIX_FMT_YUV420P as i32;
            });

            (*stream).duration = h264_packets.len() as i64;
            (*stream).r_frame_rate = ffmpeg::ffi::AVRational {
                num: fps as i32,
                den: 1,
            };
            (*stream).avg_frame_rate = ffmpeg::ffi::AVRational { 
                num: fps as i32, 
                den: 1 
            };
        }
        
        octx.write_header()?;

        // Get stream for rescaling
        let stream = octx.stream(0).unwrap();
        let stream_time_base = stream.time_base();
        let encoder_time_base = Rational::new(1, fps as i32); // Same as what encoder used

        println!("📹 Encoder time_base: {}/{}, Stream time_base: {}/{}", 
                 encoder_time_base.numerator(), encoder_time_base.denominator(),
                 stream_time_base.numerator(), stream_time_base.denominator());

        for (i, h264_pkt) in h264_packets.iter().enumerate() {
            if h264_pkt.data.is_empty() { continue; }

            let mut packet = Packet::new(h264_pkt.data.len());
            if let Some(data) = packet.data_mut() {
                data.copy_from_slice(&h264_pkt.data);
            }
        
            packet.set_stream(0);
            packet.set_pts(Some(h264_pkt.pts));
            packet.set_dts(Some(h264_pkt.dts));
            packet.set_duration(1);

            if h264_pkt.is_keyframe {
                packet.set_flags(ffmpeg::codec::packet::Flags::KEY);
            }

            // CRITICAL FIX: Rescale timestamps like img2vid_encoder does
            packet.rescale_ts(encoder_time_base, stream_time_base);

            if i < 5 {
                println!("📦 Packet {}: Original PTS={}, DTS={} -> Rescaled PTS={:?}, DTS={:?}", 
                         i, h264_pkt.pts, h264_pkt.dts, packet.pts(), packet.dts());
            }

            packet.write_interleaved(&mut octx)?;
        }
        
        octx.write_trailer()?;
        
        let duration_seconds = h264_packets.len() as f64 / fps as f64;
        println!("✅ MP4 written: {} frames, {:.2}s @ {}fps", 
                 h264_packets.len(), duration_seconds, fps);
        
        Ok(())
    }

    /**
     * This function is also to verify if h264 packets are valid by writing them to an MP4 file.
     * It flushes the encoder and collects all remaining packets.
     * Returns a vector of H264Packet structs.
     */
    pub fn flush_to_packets(&mut self) -> Result<Vec<H264Packet>, Error> {
        self.encoder.send_eof()?;

        let mut packets = Vec::new();
        let mut packet = Packet::empty();

        while self.encoder.receive_packet(&mut packet).is_ok() {
            if let Some(data) = packet.data() {
                let pts = packet.pts().unwrap_or(self.frame_count as i64);
                let dts = packet.dts().unwrap_or(self.frame_count as i64);
                
                packets.push(H264Packet {
                    data: data.to_vec(),
                    pts,
                    dts,
                    is_keyframe: packet.is_key(),
                });
                
                log::trace!("Flush packet: PTS={}, DTS={}, size={}, keyframe={}", 
                           pts, dts, data.len(), packet.is_key());
            }
        }

        Ok(packets)
    }
}

#[allow(dead_code)]
fn get_jpeg_files(input_spec: &str) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    use std::fs;
    use std::path::Path;
    
    let path = Path::new(input_spec);
    
    if path.is_dir() {
        // If it's a directory, read all JPEG files from it
        let mut files = Vec::new();
        for entry in fs::read_dir(path)? {
            let entry = entry?;
            let file_path = entry.path();
            if let Some(ext) = file_path.extension() {
                if ext.to_str().unwrap_or("").to_lowercase() == "jpg" 
                    || ext.to_str().unwrap_or("").to_lowercase() == "jpeg" {
                    if let Some(path_str) = file_path.to_str() {
                        files.push(path_str.to_string());
                    }
                }
            }
        }
        files.sort();
        Ok(files)
    } else if input_spec.contains("%d") || input_spec.contains("%0") {
        // Handle numbered sequence pattern like "frame_%04d.jpg"
        let mut files = Vec::new();
        let mut index = 0;
        
        loop {
            let filename = if input_spec.contains("%04d") {
                input_spec.replace("%04d", &format!("{:04}", index))
            } else if input_spec.contains("%03d") {
                input_spec.replace("%03d", &format!("{:03}", index))
            } else if input_spec.contains("%02d") {
                input_spec.replace("%02d", &format!("{:02}", index))
            } else if input_spec.contains("%d") {
                input_spec.replace("%d", &index.to_string())
            } else {
                break;
            };
            
            if Path::new(&filename).exists() {
                files.push(filename);
                index += 1;
            } else {
                break;
            }
        }
        
        if files.is_empty() {
            return Err(format!("No files found matching pattern: {}", input_spec).into());
        }
        
        Ok(files)
    } else {
        // Assume it's a single file
        if path.exists() {
            Ok(vec![input_spec.to_string()])
        } else {
            Err(format!("File not found: {}", input_spec).into())
        }
    }
}

#[tokio::test]
async fn test_client_jpeg_to_h264() {
    std::env::set_var("RUST_LOG", "trace");
    pretty_env_logger::init();

    unsafe {
        ffmpeg::ffi::av_log_set_level(ffmpeg::ffi::AV_LOG_ERROR);
    }

    let jpeg_files = get_jpeg_files("test_output/input_images3/test_frame_%03d.jpg")
        .expect("Failed to get JPEG files");
    
    if jpeg_files.is_empty() {
        println!("⚠️ No JPEG files found");
        return;
    }

    let mut all_h264_packets: Vec<H264Packet> = Vec::new();
    let mut transcoder = Jpeg2H264Transcoder::new(1920, 1080, 30)
        .expect("Failed to create transcoder");
    
    println!("Found {} JPEG files", jpeg_files.len());
    
    let mut successful_frames = 0;
    let mut packets_generated = 0;
    
    // Process all frames and collect packets
    for (i, jpeg_file) in jpeg_files.iter().enumerate() {
        if i % 50 == 0 {
            println!("Processing frame {}/{}: {}", i + 1, jpeg_files.len(), jpeg_file);
        }
        
        match std::fs::read(jpeg_file) {
            Ok(jpeg_data) => {
                match transcoder.transcode_jpeg_to_h264_packet(&jpeg_data) {
                    Ok(packets) => {
                        successful_frames += 1;
                        packets_generated += packets.len();
                        
                        if !packets.is_empty() {
                            log::trace!("Frame {}: Generated {} H.264 packets", i, packets.len());
                        } else {
                            log::trace!("Frame {}: No packets yet (encoder delay)", i);
                        }
                        
                        all_h264_packets.extend(packets);
                    }
                    Err(e) => {
                        eprintln!("❌ Frame {} transcoding failed: {:?} (file: {})", i, e, jpeg_file);
                        continue;
                    }
                }
            }
            Err(e) => {
                eprintln!("❌ Failed to read JPEG file {}: {:?}", jpeg_file, e);
                continue;
            }
        }
    }
    
    println!("📊 Processed {} frames, generated {} packets so far", successful_frames, packets_generated);
    
    // CRITICAL: Flush to get remaining packets
    match transcoder.flush_to_packets() {
        Ok(flush_packets) => {
            if !flush_packets.is_empty() {
                println!("🔄 Flushed {} additional packets from encoder", flush_packets.len());
                all_h264_packets.extend(flush_packets);
            }
        }
        Err(e) => eprintln!("Flush failed: {:?}", e),
    }

    println!("📦 Total H.264 packets: {} (from {} input frames)", 
             all_h264_packets.len(), jpeg_files.len());

    if all_h264_packets.is_empty() {
        println!("❌ No H.264 packets generated! Check encoder settings.");
        return;
    }

    // Sort by DTS to ensure proper order
    all_h264_packets.sort_by_key(|p| p.dts);
    
    // Reassign sequential timestamps for consistent playback
    for (i, packet) in all_h264_packets.iter_mut().enumerate() {
        packet.pts = i as i64;
        packet.dts = i as i64;
    }

    println!("🎬 Reassigned timestamps: 0 to {}", all_h264_packets.len() - 1);

    // Write MP4
    std::fs::create_dir_all("test_output").ok();
    
    match Jpeg2H264Transcoder::write_h264_packets_to_mp4(
        &all_h264_packets,
        "test_output/h264_enhanced.mp4",
        1920, 1080, 30,
    ) {
        Ok(_) => {
            let actual_duration = all_h264_packets.len() as f64 / 30.0;
            let expected_duration = jpeg_files.len() as f64 / 30.0;
            println!("✅ Enhanced MP4 file written successfully");
            println!("📹 Actual duration: {:.2}s ({} frames)", actual_duration, all_h264_packets.len());
            println!("📹 Expected duration: {:.2}s ({} frames)", expected_duration, jpeg_files.len());
            
            let efficiency = (all_h264_packets.len() as f64 / jpeg_files.len() as f64) * 100.0;
            println!("📊 Frame efficiency: {:.1}%", efficiency);

            // ADDED: Verify timing
            if all_h264_packets.len() >= 2 {
                let first_pts = all_h264_packets[0].pts;
                let second_pts = all_h264_packets[1].pts;
                let frame_interval = second_pts - first_pts;
                let calculated_fps = 30.0 / frame_interval as f64;
                println!("🔍 Frame interval: {} time_base units", frame_interval);
                println!("🔍 Calculated FPS from PTS: {:.1}", calculated_fps);
            }

            // ADDING DEBUG TIMING INFO
            println!("🔍 Debug timing:");
            println!("   Total packets: {}", all_h264_packets.len());
            println!("   Expected duration: {:.2}s", all_h264_packets.len() as f64 / 30.0);
            println!("   Time scale: 1000");
            println!("   Expected duration in time units: {}", (all_h264_packets.len() as f64 / 30.0 * 1000.0) as i64);
            println!("   Frame duration: {:.3}", 1000.0 / 30.0);

            if all_h264_packets.len() >= 2 {
                // Check actual PTS values after conversion
                let pts_0 = (0.0f64 * (1000.0 / 30.0)).round() as i64;
                let pts_1 = (1.0f64 * (1000.0 / 30.0)).round() as i64;
                let pts_2 = (2.0f64 * (1000.0 / 30.0)).round() as i64;
                
                println!("   PTS progression: {} -> {} -> {} (intervals: {}, {})",
                         pts_0, pts_1, pts_2, pts_1 - pts_0, pts_2 - pts_1);
                println!("   Calculated fps: {:.2}", 1000.0 / (pts_1 - pts_0) as f64);
            }
        }
        Err(e) => eprintln!("❌ Failed to write enhanced MP4: {:?}", e),
    }
}