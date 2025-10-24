use nokhwa::pixel_format::{YuyvFormat};
use nokhwa::utils::{CameraIndex, RequestedFormat, RequestedFormatType, Resolution};
use nokhwa::{Camera};
use tokio::sync::mpsc;
use std::sync::{Arc};
use std::sync::atomic::{AtomicBool, Ordering};
use std::io::{Read};

extern crate pretty_env_logger;
extern crate log;

/*************************************************************************************
 * WebcamReader
 * Currently uses nokhwa to read webcam frames. This gives better cross-platform support
 * than opencv, which has issues on some Linux systems.
 * However, it automatically converts to MJPEG format internally.
 * 
 * It is INEVITABLE to convert MJPEG to YUV for h264 encoding.
 * 
 * In case of using YUYV format directly, it results in lower resolution (800x448) with Logitech C920.
 * Refer to test_nokhwa_camera_sup_formats() test case.
 */

pub struct WebcamReader {
    receiver: mpsc::Receiver<Vec<u8>>,
    _handle: tokio::task::JoinHandle<()>, // Use tokio JoinHandle instead of std thread
    buffer: Option<Vec<u8>>, // Add this field to store data
    shutdown_flag: Arc<AtomicBool>, // Flag to signal shutdown
}

impl WebcamReader {
    /**
     * Starts webcam capture in a separate blocking thread.
     * Returns a WebcamReader instance with a channel to receive frame data.
     * Webcam has fixed capture rate: 30 FPS
     */
    pub async fn new(device_id: u32) -> Result<Self, String> {
        let (sender, receiver) = mpsc::channel(1000);
        let shutdown_flag = Arc::new(AtomicBool::new(false));
        let shutdown_flag_clone = shutdown_flag.clone();

        // Set capture rate 30 FPS if not specified
        let mut _capture_rate: u32 = 30;

        let handle = tokio::task::spawn_blocking(move || {
            if let Err(e) = Self::capture_webcam(device_id, sender, shutdown_flag_clone, _capture_rate) {
                log::error!("Webcam capture error: {}", e);
            }
        });

        Ok(WebcamReader {
            receiver,
            _handle: handle,
            buffer: None,
            shutdown_flag,
        })
    }

    pub async fn stop_webcam(&mut self) {
        println!("Stopping webcam capture...");
        self.shutdown_flag.store(true, Ordering::Relaxed);
        
        // Wait for clean shutdown
        if let Err(_) = tokio::time::timeout(
            std::time::Duration::from_secs(5), 
            &mut self._handle
        ).await {
            println!("⚠️ Force aborting webcam capture");
            self._handle.abort();
        }
        
        println!("✅ Webcam capture stopped");
    }

    /*
        1. Open the webcam device using nokhwa.
        2. Continuously capture frames in a loop.
        3. Send each captured frame via the provided mpsc sender.
        4. Check the shutdown_flag to exit the loop and stop capturing.
        5. On shutdown, close the camera stream cleanly.
     */
    fn capture_webcam(
        device_id: u32,
        sender: mpsc::Sender<Vec<u8>>,
        shutdown_flag: Arc<AtomicBool>,
        _capture_rate: u32,
    ) -> Result<(), String> {
        log::info!("Opening webcam device: {} with nokhwa", device_id);

        // Try different resolutions to avoid hardware conflicts
        let resolutions_to_try = vec![
            Resolution::new(1920, 1080),
            Resolution::new(1280, 720),
            Resolution::new(1024, 768),
            Resolution::new(800, 600),
            Resolution::new(640, 480),
        ];

        let mut camera = None;
        let mut last_error = String::new();

        for resolution in &resolutions_to_try {
            println!("Trying resolution: {}x{}", resolution.width(), resolution.height());

            // even though we request YuyvFormat, it returns RgbFormat frames due to the camera support. 
            let requested = RequestedFormat::new::<YuyvFormat>(
                RequestedFormatType::AbsoluteHighestResolution,
            );

            match Camera::new(CameraIndex::Index(device_id), requested) { 
                Ok(mut cam) => {
                    match cam.open_stream() {
                        Ok(_) => {
                            camera = Some(cam);
                            log::info!("✅ Camera {} opened successfully at {}x{}", 
                                device_id, resolution.width(), resolution.height());
                            break;
                        }
                        Err(e) => {
                            last_error = format!("Resolution {}x{}: {}", resolution.width(), resolution.height(), e);
                            println!("❌ Failed to open stream at {}x{}: {}", resolution.width(), resolution.height(), e);
                        }
                    }
                }
                Err(e) => {
                    last_error = format!("Resolution {}x{}: {}",
                        resolution.width(), resolution.height(), e);
                    println!("❌ Failed to create camera at {}x{}: {}",
                        resolution.width(), resolution.height(), e);
                }
            }
        }

        let mut camera = camera.ok_or(format!("Failed to open camera at any resolution. Last error: {}", last_error))?;

        let target_fps = _capture_rate as f64;
        let frame_duration = std::time::Duration::from_secs_f64(1.0 / target_fps);
        let mut last_frame_time = std::time::Instant::now();

        loop {  // Capture loop - one iteration per complete frame
            if shutdown_flag.load(Ordering::Relaxed) {
                log::info!("🛑 Shutdown signal received, stopping capture...");
                break;
            }

            let now = std::time::Instant::now();
            let elapsed = now.duration_since(last_frame_time);

            // Only capture if enough time has passed for target FPS
            if elapsed >= frame_duration {
                match camera.frame() {
                    Ok(frame) => {
                        let image_buffer = frame.buffer().to_vec();
                        // println!("FrameFormat: {:?}", frame.source_frame_format());

                        if let Err(e) = sender.blocking_send(image_buffer) {
                            log::error!("❌ Failed to send frame data: {}", e);
                            break;
                        }

                        last_frame_time = now;
                        log::debug!("📸 Frame captured, actual interval: {:.1}ms", elapsed.as_secs_f64() * 1000.0);
                    }
                    Err(e) => {
                        log::error!("❌ Error capturing frame: {}", e);
                        std::thread::sleep(std::time::Duration::from_millis(100));
                    }
                }
            } else {
                // Wait for the remaining time
                let remaining = frame_duration - elapsed;
                if remaining > std::time::Duration::from_millis(1) {
                    std::thread::sleep(remaining);
                }
            }
        }

        // Clean shutdown
        let _ = camera.stop_stream();
        log::info!("📷 Camera stream closed");
        Ok(())
    }

    pub async fn list_available_devices() -> Result<Vec<String>, String> {
        log::info!("Enumerating available webcam devices with nokhwa...");

        let devices = nokhwa::query(nokhwa::utils::ApiBackend::Auto)
            .map_err(|e| format!("Failed to query devices: {}", e))?;

        let device_list: Vec<String> = devices
            .into_iter()
            .enumerate()
            .map(|(i, info)| format!("{}: {}", i, info.human_name()))
            .collect();

        if device_list.is_empty() {
            Err("No webcam devices found".to_string())
        } else {
            println!("Found {} webcam devices: {:?}", device_list.len(), device_list);
            Ok(device_list)
        }
    }

    /*
        Read a single complete JPEG frame from the webcam stream.
        Waits up to timeout_secs for a complete frame.
        Returns the JPEG byte data of the frame.
     */
    pub async fn read_single_frame(&mut self, timeout_secs: u64) -> Result<Vec<u8>, String> {
        log::trace!("Waiting to read a single JPEG frame from webcam...");
        // CLEAR BUFFER at start to ensure we start fresh for each frame capture
        self.buffer = None;
        
        let start_time = std::time::Instant::now();
        let timeout = std::time::Duration::from_secs(timeout_secs);
        let mut frame_data = Vec::new();
        
        // Use a small buffer for reading chunks
        let mut chunk_buffer = vec![0u8; 8 * 1024 * 1024]; // 6MB chunks
        
        loop {  // Wait for complete frame or timeout
            if start_time.elapsed() > timeout {
                return Err("Timeout waiting for frame".to_string());
            }
            // Use the Read trait implementation on self
            match self.read(&mut chunk_buffer) {
                Ok(bytes_read) if bytes_read > 0 => {
                    chunk_buffer.truncate(bytes_read);
                    frame_data.extend_from_slice(&chunk_buffer);
                    
                    log::debug!("📥 Read chunk: {} bytes (total: {})", bytes_read, frame_data.len());

                    // Check if we have a complete JPEG frame (look for JPEG SOI and EOI markers)
                    if frame_data.len() > 8 &&
                            frame_data[0] == 0xFF && frame_data[1] == 0xD8 && // SOI
                            frame_data[frame_data.len() - 2] == 0xFF && frame_data[frame_data.len() - 1] == 0xD9 // EOI
                        {
                            log::debug!("✅ Complete JPEG frame captured, size: {} bytes", frame_data.len());
                            return Ok(frame_data);
                        }
                }
                Ok(_) => {
                    // No data available, wait briefly
                    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    // No data available, wait briefly
                    tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
                }
                Err(e) => {
                    return Err(format!("Read error: {}", e));
                }
            }
        }
    }
}

/* Read buffer via channels */
impl std::io::Read for WebcamReader {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        // read data from receiver
        if let Some(data) = self.buffer.take() {
            let len = std::cmp::min(buf.len(), data.len());
            buf[..len].copy_from_slice(&data[..len]);
            if len < data.len() {
                // If there's remaining data, store it back in buffer
                self.buffer = Some(data[len..].to_vec());
            }
            Ok(len)
        } else {
            match self.receiver.try_recv() {
                Ok(data) => {
                    let len = std::cmp::min(buf.len(), data.len());
                    buf[..len].copy_from_slice(&data[..len]);
                    if len < data.len() {
                        // If there's remaining data, store it in buffer
                        self.buffer = Some(data[len..].to_vec());
                    }
                    Ok(len)
                }
                Err(mpsc::error::TryRecvError::Empty) => {
                    // No data available right now
                    Err(std::io::Error::new(std::io::ErrorKind::WouldBlock, "No data available"))
                }
                Err(mpsc::error::TryRecvError::Disconnected) => {
                    // Channel closed, no more data will come
                    Err(std::io::Error::new(std::io::ErrorKind::UnexpectedEof, "Channel disconnected"))
                }
            }
        }
    }
}

impl Drop for WebcamReader {
    fn drop(&mut self) {
        self._handle.abort();
        println!("WebcamReader dropped");
    }
}

/************************* UNIT TEST ***********************/

#[tokio::test]
async fn test_client_webrtc_available_webcam() {
    println!("=== Testing Webcam Availability with nokhwa ===");

    // List available webcam devices
    let devices_result = WebcamReader::list_available_devices().await;
    match devices_result {
        Ok(devices) => {
            println!("Available webcam devices: {:?}", devices);
            if devices.is_empty() {
                println!("⚠️ No webcam devices found");
            } else {
                println!("✅ Webcam devices found");
            }
        }
        Err(e) => {
            println!("⚠️ Failed to list webcam devices: {}", e);
            assert!(false, "Failed to list webcam devices");
        }
    }

    println!("=== Webcam Availability Test Complete ===");
}

#[tokio::test]
#[cfg(not(target_arch="wasm32"))]
async fn test_nokhwa() {
    // first camera in system
    use nokhwa::pixel_format::YuyvFormat;
    let index = CameraIndex::Index(0);
    
    // request yuv format
    let requested = RequestedFormat::new::<YuyvFormat>(
        RequestedFormatType::AbsoluteHighestResolution,
    );
    let mut camera = Camera::new(index, requested).unwrap();

    // get a frame
    let frame = camera.frame().unwrap();
    println!("Captured Single Frame of {}", frame.buffer().len());

    let frame_format = frame.source_frame_format();
    println!("Frame format: {:?}", frame_format);

    // write frame as it is
    std::fs::create_dir_all("./test_output").unwrap();
    std::fs::write("./test_output/nokhwa_test.jpg", frame.buffer()).unwrap();
}

/**
 * Figure out what formats are supported by the camera
 */
#[tokio::test]
async fn test_nokhwa_camera_sup_formats() {
    // first camera in system
    let index = CameraIndex::Index(0);
    let requested = RequestedFormat::new::<YuyvFormat>(
        RequestedFormatType::AbsoluteHighestResolution,
    );
    let mut camera = Camera::new(index, requested).unwrap();

    let formats = camera.compatible_camera_formats().unwrap();
    println!("Supported formats for camera 0:");
    for format in formats {
        if format.frame_rate() >= 30 {
            println!(" - {:?} (selected)", format);
        }
    }
}

#[tokio::test]
async fn test_client_webcam_capture_frame() {
    pretty_env_logger::init();
    let mut reader = WebcamReader::new(0).await.expect("Failed to create WebcamReader");
    let timeout_secs = 3;

    let jpeg_frame = reader.read_single_frame(timeout_secs).await.expect("test.Failed to capture frame");

    // write to file for manual inspection
    std::fs::write("test_output/test_frame.jpg", &jpeg_frame).expect("Failed to write frame to file");

    println!("✅ Frame written to test_frame.jpg for inspection");
    reader.stop_webcam().await;
}

#[tokio::test]
async fn test_client_webcam_capture_multiple_frame() {
    pretty_env_logger::init();
    let mut reader = WebcamReader::new(0).await.expect("Failed to create WebcamReader");
    let timeout_secs = 10;

    for i in 0..240 {
        let jpg_frame = reader.read_single_frame(timeout_secs).await.expect("test.Failed to capture frame");

        // write to file for manual inspection
        std::fs::write(format!("test_output/input_images3/test_frame_{:03}.jpg", i), &jpg_frame).expect("Failed to write frame to file");

        println!("✅ Frame written to test_frame_{:03}.jpg for inspection", i);
        std::thread::sleep(std::time::Duration::from_millis(100));
    }

    reader.stop_webcam().await;
}

/**
 * ImageToVideoEncoder is just to verify the encoding process from jpeg to mp4.
 */
#[tokio::test]
async fn test_client_convert_jpeg_to_mp4() {
    use crate::client::xrds_webrtc::media::transcoding::img2vid_encoder::ImageToVideoEncoder;
    pretty_env_logger::init();
    use ffmpeg::ffi::*;
    unsafe {
        av_log_set_level(AV_LOG_ERROR);
    }

    let input_pattern = "./test_output/input_images2";
    let output_path = "test_output/output_video.mp4";
    let fps: i32 = 30;

    use ffmpeg_next as ffmpeg;
    use ffmpeg_next::Rational;

    ffmpeg::init().unwrap();

    let mut octx = ffmpeg::format::output(&output_path).unwrap();

    // let x264_opt_str = "rate_control=quality,quality=18,colorprim=bt709,transfer=bt709,colormatrix=bt709";
    // High quality (equivalent to crf=16)
    // let x264_opt_str = "rate_control=quality,quality=16,hw_encoding=1";
    let x264_opt_str = "bitrate=10000000"; // 10 Mbps
    
    let fps_rational = Rational::new(fps as i32, 1);
    let mut encoder = ImageToVideoEncoder::new(
        1920, 1080, fps_rational, &mut octx, x264_opt_str
    ).unwrap();

    // Don't panic immediately, provide better error info
    match encoder.encode_video(input_pattern, &output_path, &mut octx) {
        Ok(_) => println!("✅ JPEG to MP4 conversion successful"),
        Err(e) => {
            eprintln!("❌ JPEG to MP4 conversion failed: {}", e);
            panic!("Failed to convert JPEG to MP4: {}", e);
        }
    }
}