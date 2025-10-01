use ffmpeg_next as ffmpeg;
use std::time::Instant;

use ffmpeg::{
    codec, encoder, format, frame, log, media, Dictionary, Packet, Rational, color,
};

const DEFAULT_X264_OPTS: &str = "preset=medium";

pub struct ImageToVideoEncoder {
    encoder: encoder::Video,
    frame_count: usize,
    starting_time: Instant,
    fps: Rational,
}

impl ImageToVideoEncoder {
    pub fn new(
        width: u32,
        height: u32,
        fps: Rational,
        octx: &mut format::context::Output,
        x264_opts: &str,
    ) -> Result<Self, ffmpeg::Error> {
        let global_header = octx.format().flags().contains(format::Flags::GLOBAL_HEADER);
        let codec = encoder::find(codec::Id::H264);
        let mut ost = octx.add_stream(codec)?;

        let mut encoder =
            codec::context::Context::new_with_codec(codec.ok_or(ffmpeg::Error::InvalidData)?)
                .encoder()
                .video()?;
        
        encoder.set_height(height);
        encoder.set_width(width);
        encoder.set_format(format::Pixel::YUV420P);
        encoder.set_frame_rate(Some(fps));

        let time_base = fps.invert();
        encoder.set_time_base(time_base);

        // Better quality settings
        encoder.set_bit_rate(8_000_000); // 8 Mbps for 1080p
        encoder.set_max_bit_rate(12_000_000); // 12 Mbps max
        encoder.set_qmin(10);  // Minimum quantizer
        encoder.set_qmax(51);  // Maximum quantizer
        encoder.set_gop(fps.numerator() as u32); // GOP size = 1 second

        encoder.set_color_range(ffmpeg::util::color::Range::MPEG);
        encoder.set_colorspace(color::Space::BT709);

        eprintln!("frame rate: {:?}", fps);

        if global_header {
            encoder.set_flags(codec::Flags::GLOBAL_HEADER);
        }

        let x264_opts = parse_opts(x264_opts.to_string())
            .or_else(|| parse_opts(DEFAULT_X264_OPTS.to_string()))
            .ok_or(ffmpeg::Error::InvalidData)?;

        eprintln!("Using x264 options: {:?}", x264_opts);

        let opened_encoder = encoder
            .open_with(x264_opts)
            .expect("error opening x264 with supplied settings");
        ost.set_parameters(&opened_encoder);
        ost.set_time_base(time_base);

        Ok(Self {
            encoder: opened_encoder,
            frame_count: 0,
            starting_time: Instant::now(),
            fps,
        })
    }

    pub fn encode_video(&mut self, img_path: &str, 
        output_path: &str, 
        octx: &mut format::context::Output) -> Result<(), String>{
        let mut jpeg_files = get_jpeg_files(&img_path).map_err(|e| {
            eprintln!("Error finding JPEG files: {}", e);
            e.to_string()  // Don't exit, return error
        })?;

        jpeg_files.sort();

        if jpeg_files.is_empty() {
            return Err("No JPEG files found".to_string());
        }

        // Load first image to get TARGET dimensions for the video
        let first_frame = load_jpeg_as_frame(&jpeg_files[0])
            .map_err(|e| format!("Failed to load first frame: {:?}", e))?;
        let target_width = first_frame.width();
        let target_height = first_frame.height();

        eprintln!("Video dimensions: {}x{}", target_width, target_height);
        eprintln!("Total frames to process: {}", jpeg_files.len());
        eprintln!("Expected duration: {:.2} seconds", 
                 jpeg_files.len() as f64 / self.fps.numerator() as f64 * self.fps.denominator() as f64);
        
        format::context::output::dump(&octx, 0, Some(&output_path));
        octx.write_header().unwrap();
        
        let encoder_time_base = self.fps.invert();
        
        // Process each JPEG file
        for (i, jpeg_file) in jpeg_files.iter().enumerate() {
            eprintln!("Processing frame {}/{}: {}", i + 1, jpeg_files.len(), jpeg_file);
            
            let decoded_frame = match load_jpeg_as_frame(jpeg_file) {
                Ok(frame) => frame,
                Err(e) => {
                    eprintln!("Warning: Failed to load image {}: {:?}", jpeg_file, e);
                    continue;  // Skip this frame and continue
                }
            };

            // Check if frame dimensions match target
            if decoded_frame.width() != target_width || decoded_frame.height() != target_height {
                eprintln!("Warning: Frame {} has different dimensions ({}x{} vs {}x{}), scaling...", 
                         jpeg_file, decoded_frame.width(), decoded_frame.height(), target_width, target_height);
            }

            // Always convert - handle both format conversion AND scaling if needed
            let mut final_frame = convert_frame_format(&decoded_frame, format::Pixel::YUV420P, target_width, target_height)
                .map_err(|e| format!("Failed to convert frame {}: {:?}", jpeg_file, e))?;
            
            self.encode_frame(&mut final_frame, octx, encoder_time_base);
        }

        self.flush(octx, encoder_time_base);
        octx.write_trailer().unwrap();
        
        eprintln!("Encoding complete. Total frames processed: {}", self.frame_count);
        Ok(())
    }

    fn encode_frame(&mut self, frame: &mut frame::Video, octx: &mut format::context::Output, encoder_time_base: Rational) {
        // Set PTS BEFORE incrementing frame_count (PTS should start at 0)
        let pts = self.frame_count as i64;
        frame.set_pts(Some(pts));
        
        println!("Frame {}: PTS = {}, time_base = {:?}", self.frame_count, pts, encoder_time_base);
        
        self.encoder.send_frame(frame).unwrap();
        self.receive_and_process_encoded_packets(octx, encoder_time_base);
        
        self.frame_count += 1;  // Increment AFTER setting PTS
        
        if self.frame_count % 30 == 0 {
            eprintln!(
                "Processed {} frames in {:.2} seconds",
                self.frame_count,
                self.starting_time.elapsed().as_secs_f64()
            );
        }
    }

    fn receive_and_process_encoded_packets(&mut self, octx: &mut format::context::Output,
        encoder_time_base: Rational,
    ) {
        let mut encoded = Packet::empty();
        let stream = octx.stream(0).unwrap();
        let stream_time_base = stream.time_base();
        
        // println!("Encoder time_base: {:?}, Stream time_base: {:?}", encoder_time_base, stream_time_base);
        
        while self.encoder.receive_packet(&mut encoded).is_ok() {
            encoded.set_stream(0);
            
            // Rescale from encoder time base to stream time base
            encoded.rescale_ts(encoder_time_base, stream_time_base);
            
            encoded.write_interleaved(octx).unwrap();
        }
    }


    fn flush(&mut self, octx: &mut format::context::Output, time_base: Rational) {
        self.encoder.send_eof().unwrap();
        self.receive_and_process_encoded_packets(octx, time_base);
    }    
}

fn parse_opts<'a>(s: String) -> Option<Dictionary<'a>> {
    let mut dict = Dictionary::new();
    for keyval in s.split_terminator(',') {
        let tokens: Vec<&str> = keyval.split('=').collect();
        match tokens[..] {
            [key, val] => dict.set(key, val),
            _ => return None,
        }
    }
    Some(dict)
}

fn load_jpeg_as_frame(jpeg_path: &str) -> Result<frame::Video, ffmpeg::Error> {
    // Open the JPEG file as input
    let mut ictx = format::input(&jpeg_path)?;
    
    // Find the video stream (JPEG is treated as a single-frame video)
    let input_stream = ictx
        .streams()
        .best(media::Type::Video)
        .ok_or(ffmpeg::Error::StreamNotFound)?;
    
    // Create decoder for the JPEG
    let context = ffmpeg::codec::context::Context::from_parameters(input_stream.parameters())?;
    let mut decoder = context.decoder().video()?;
    
    // Read the packet and decode it
    let (_, packet) = ictx.packets().next().ok_or(ffmpeg::Error::Eof)?;
    decoder.send_packet(&packet)?;
    
    let mut decoded_frame = frame::Video::empty();
    decoder.receive_frame(&mut decoded_frame)?;
    
    // Explicitly set color properties for YUV422P from JPEG
    decoded_frame.set_color_range(color::Range::JPEG);  // Full range
    decoded_frame.set_color_space(color::Space::BT709);

    Ok(decoded_frame)
}

fn convert_frame_format(src_frame: &frame::Video, target_format: format::Pixel, target_width: u32, target_height: u32) -> Result<frame::Video, ffmpeg::Error> {
    use ffmpeg::software::scaling::{context::Context, flag::Flags};
    
    eprintln!("Converting: {}x{} {:?} -> {}x{} {:?}", 
             src_frame.width(), src_frame.height(), src_frame.format(),
             target_width, target_height, target_format);

    // Create scaler - use the exact same dimensions to avoid "input changed"
    let mut scaler = Context::get(
        src_frame.format(),
        src_frame.width(),
        src_frame.height(),
        target_format,
        src_frame.width(),   // Keep same width
        src_frame.height(),  // Keep same height  
        Flags::BILINEAR,
    )?;

    // Create output frame and let FFmpeg allocate it
    let mut converted_frame = frame::Video::empty();
    
    // Run conversion - this should allocate the frame automatically
    scaler.run(src_frame, &mut converted_frame)?;
    
    // Set metadata after conversion
    converted_frame.set_color_range(color::Range::MPEG);
    converted_frame.set_color_space(color::Space::BT709);
    
    eprintln!("✅ Conversion successful: {}x{} {:?}", 
             converted_frame.width(), converted_frame.height(), converted_frame.format());
    
    Ok(converted_frame)
}

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