use nokhwa::pixel_format::RgbFormat;
use nokhwa::utils::{RequestedFormat, RequestedFormatType, CameraIndex, Resolution};
use nokhwa::{Camera};
use tokio::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::io::Read;

extern crate pretty_env_logger;
extern crate log;

pub struct WebcamReader {
    receiver: mpsc::Receiver<Vec<u8>>,
    _handle: tokio::task::JoinHandle<()>, // Use tokio JoinHandle instead of std thread
    buffer: Option<Vec<u8>>, // Add this field to store data
    buffer_offset: usize, // Track current offset in buffer
}

impl WebcamReader {
    pub async fn new(device_id: u32) -> Result<Self, String> {
        let (sender, receiver) = mpsc::channel(1000);

        // Keep camera alive for the entire session
        let camera = Arc::new(Mutex::new(None));
        let camera_clone = camera.clone();

        let handle = tokio::task::spawn_blocking(move || {
            if let Err(e) = Self::capture_webcam(device_id, sender, camera_clone) {
                eprintln!("Webcam capture error: {}", e);
            }
        });

        Ok(WebcamReader {
            receiver,
            _handle: handle,
            buffer: None, // Initialize buffer
            buffer_offset: 0, // Initialize buffer offset
        })
    }

    pub async fn stop_webcam(&self) {
        // Dropping the sender will close the channel and stop the capture loop
        println!("Stopping webcam capture...");
        self._handle.abort();
    }

    fn capture_webcam(
        device_id: u32,
        sender: mpsc::Sender<Vec<u8>>,
        camera_arc: Arc<Mutex<Option<Camera>>>
    ) -> Result<(), String> {
        println!("Opening webcam device: {} with nokhwa", device_id);

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

            let requested = RequestedFormat::new::<RgbFormat>(
                RequestedFormatType::AbsoluteHighestResolution,
            );

            match Camera::new(CameraIndex::Index(device_id), requested) {
                Ok(mut cam) => {
                    match cam.open_stream() {
                        Ok(_) => {
                            camera = Some(cam);
                            println!("✅ Camera {} opened successfully at {}x{}",
                                device_id, resolution.width(), resolution.height());
                            break;
                        }
                        Err(e) => {
                            last_error = format!("Resolution {}x{}: {}",
                                resolution.width(), resolution.height(), e);
                            println!("❌ Failed to open stream at {}x{}: {}",
                                resolution.width(), resolution.height(), e);
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

        let camera_instance = camera.ok_or(format!("Failed to open camera at any resolution. Last error: {}", last_error))?;

        // Store camera in Arc Mutex to keep it alive for the entire session
        *camera_arc.lock().unwrap() = Some(camera_instance);
        
        // Get a clone of the Arc to work with
        let camera_arc_clone = camera_arc.clone();        

        let mut frame_count = 0;
        let start_time = std::time::Instant::now();
        let mut accumulated_rgb_data = Vec::new();
        let mut expected_rgb_size: Option<usize> = None;
        let mut frame_width: Option<u32> = None;
        let mut frame_height: Option<u32> = None;

        loop {  // Main pumping loop - one iteration per complete frame
            // Get camera from Arc Mutex
            let mut camera_guard = camera_arc_clone.lock().unwrap();
            let camera = camera_guard.as_mut().unwrap();

            loop {  // Data accumulation loop - accumulate until we have a complete frame
                // Capture frame - may be partial
                let frame = match camera.frame() {
                    Ok(f) => f,
                    Err(e) => {
                        println!("❌ (camera.frame)Failed to capture frame: {}", e);
                        // Try to reopen stream on error
                        if let Err(reopen_err) = camera.open_stream() {
                            println!("❌ Failed to reopen stream: {}", reopen_err);
                            return Err("Camera stream failed".to_string());
                        }
                        continue;
                    }
                };

                // Set frame dimensions from first successful frame
                if frame_width.is_none() {
                    frame_width = Some(frame.resolution().width());
                    frame_height = Some(frame.resolution().height());
                    expected_rgb_size = Some((frame_width.unwrap() * frame_height.unwrap() * 3) as usize);
                    println!("📊 Target frame size: {}x{} = {} RGB bytes", 
                        frame_width.unwrap(), frame_height.unwrap(), expected_rgb_size.unwrap());
                }

                let image_buffer = frame.buffer().to_vec();
                accumulated_rgb_data.extend_from_slice(&image_buffer);

                // println!("📥 Accumulated {} bytes (total: {}/{})", 
                //     image_buffer.len(), accumulated_rgb_data.len(), 
                //     expected_rgb_size.unwrap_or(0));

                // Check if we have enough data for a complete frame
                if accumulated_rgb_data.len() >= expected_rgb_size.unwrap_or(0) {
                    // We have a complete frame! Extract it
                    let complete_rgb_data = accumulated_rgb_data[..expected_rgb_size.unwrap()].to_vec();
                    
                    // Create frame with header
                    let mut frame_data = Vec::new();
                    frame_data.extend_from_slice(b"FRAME"); // 5-byte header
                    frame_data.extend_from_slice(&(frame_width.unwrap() as u32).to_le_bytes());
                    frame_data.extend_from_slice(&(frame_height.unwrap() as u32).to_le_bytes());
                    frame_data.extend_from_slice(&complete_rgb_data);

                    log::info!("📊 Frame {}: header {} + RGB {} = total {} bytes",
                        frame_count + 1, 13, complete_rgb_data.len(), frame_data.len());
                    
                    // Send complete frame via channel
                    if let Err(e) = sender.blocking_send(frame_data) {
                        log::error!("❌ Failed to send frame via channel: {}", e);
                        return Err("Channel send failed".to_string());
                    }
                    
                    frame_count += 1;
                    
                    // Keep any remaining data for the next frame
                    accumulated_rgb_data = accumulated_rgb_data[expected_rgb_size.unwrap()..].to_vec();
                    
                    // Performance logging every 30 frames
                    if frame_count % 30 == 0 {
                        let elapsed = start_time.elapsed();
                        let fps = frame_count as f64 / elapsed.as_secs_f64();
                        println!("📊 Sent {} complete frames, FPS: {:.2}", frame_count, fps);
                    }
                    
                    // Break out of accumulation loop to send next frame
                    break;
                } else {
                    // Not enough data yet, continue accumulating
                    let progress = (accumulated_rgb_data.len() as f64 / expected_rgb_size.unwrap_or(1) as f64) * 100.0;
                    // println!("📊 Accumulating: {}/{} bytes ({:.1}%)", 
                    //     accumulated_rgb_data.len(), expected_rgb_size.unwrap_or(0), progress);
                }
            }

            // Small delay to control frame rate
            std::thread::sleep(std::time::Duration::from_millis(33)); // ~30fps
        }
    }

    fn is_valid_frame(data: &[u8]) -> bool {
        // Check for our custom frame header
        data.len() >= 13 && &data[0..5] == b"FRAME"
    }

    pub async fn list_available_devices() -> Result<Vec<String>, String> {
        println!("Enumerating available webcam devices with nokhwa...");

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

    /** For TESTING */
    pub async fn read_single_frame(&mut self, timeout_secs: u64) -> Result<Vec<u8>, String> {
        println!("📸 Capturing single frame...");
        
        let start_time = std::time::Instant::now();
        let timeout = std::time::Duration::from_secs(timeout_secs);
        let mut frame_data = Vec::new();
        let mut header_parsed = false;
        let mut expected_frame_size: Option<usize> = None;
        
        while start_time.elapsed() < timeout {
            // Use a small buffer for reading chunks
            let mut chunk_buffer = vec![0u8; 64 * 1024 * 1024]; // 64MB chunks
            
            // Use the Read trait implementation on self
            match self.read(&mut chunk_buffer) {
                Ok(bytes_read) if bytes_read > 0 => {
                    chunk_buffer.truncate(bytes_read);
                    frame_data.extend_from_slice(&chunk_buffer);
                    
                    println!("📥 Read chunk: {} bytes (total: {})", bytes_read, frame_data.len());
                    
                    // Parse header if we haven't yet and have enough data
                    if !header_parsed && frame_data.len() >= 13 {
                        if Self::is_valid_frame(&frame_data) {
                            let width = u32::from_le_bytes([
                                frame_data[5], frame_data[6], frame_data[7], frame_data[8]
                            ]);
                            let height = u32::from_le_bytes([
                                frame_data[9], frame_data[10], frame_data[11], frame_data[12]
                            ]);
                            
                            expected_frame_size = Some(13 + (width * height * 3) as usize);
                            header_parsed = true;
                            
                            println!("📊 Frame header parsed: {}x{}, expected total size: {} bytes", 
                                width, height, expected_frame_size.unwrap());
                        } else {
                            println!("⚠️ Invalid frame header, clearing buffer");
                            frame_data.clear();
                            header_parsed = false;
                            continue;
                        }
                    }
                    
                    // Check if we have a complete frame
                    if let Some(expected_size) = expected_frame_size {
                        if frame_data.len() >= expected_size {
                            // We have a complete frame!
                            let complete_frame = frame_data[..expected_size].to_vec();
                            
                            let elapsed = start_time.elapsed();
                            println!("✅ Complete frame captured in {:.2?}: {} bytes", 
                                elapsed, complete_frame.len());
                            
                            return Ok(complete_frame);
                        } else {
                            // Progress update
                            let progress = (frame_data.len() as f64 / expected_size as f64) * 100.0;
                            println!("📊 Frame accumulation: {}/{} bytes ({:.1}%)", 
                                frame_data.len(), expected_size, progress);
                        }
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
        
        Err(format!("Timeout after {} seconds while capturing frame. Got {} bytes", 
            timeout_secs, frame_data.len()))

        // stop webcam capture
    }
}

/* Read buffer via channels */
impl std::io::Read for WebcamReader {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        // Try to get new data via sender
        match self.receiver.try_recv() {
            Ok(data) => {
                let len = std::cmp::min(buf.len(), data.len());
                buf[..len].copy_from_slice(&data[..len]);

                println!("Read {} bytes from webcam buffer", len);
                // If there's remaining data, store it in buffer with offset
                if data.len() > len {
                    self.buffer = Some(data);
                    self.buffer_offset = len; // Set offset to where we left off
                }

                Ok(len)
            }
            Err(mpsc::error::TryRecvError::Empty) => {
                Err(std::io::Error::new(
                    std::io::ErrorKind::WouldBlock,
                    "No data available"
                ))
            }
            Err(mpsc::error::TryRecvError::Disconnected) => Ok(0), // EOF
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
async fn test_nokhwa() {
    // first camera in system
    let index = CameraIndex::Index(0); 
    // request the absolute highest resolution CameraFormat that can be decoded to RGB.
    let requested = RequestedFormat::new::<RgbFormat>(RequestedFormatType::AbsoluteHighestFrameRate);
    // make the camera
    let mut camera = Camera::new(index, requested).unwrap();

    // get a frame
    let frame = camera.frame().unwrap();
    println!("Captured Single Frame of {}", frame.buffer().len());
    // decode into an ImageBuffer
    let decoded = frame.decode_image::<RgbFormat>().unwrap();
    println!("Decoded Frame of {}", decoded.len());
}

#[tokio::test]
async fn test_client_webrtc_capture_frame() {
    pretty_env_logger::init_custom_env("RUST_LOG=info");
    let mut reader = WebcamReader::new(0).await.expect("Failed to create WebcamReader");
    let timeout_secs = 10;

    let raw_frame = reader.read_single_frame(timeout_secs).await.expect("test.Failed to capture frame");

    // convert raw_frame to image
    assert!(raw_frame.len() > 13, "Frame data too small");

    let width = u32::from_le_bytes([raw_frame[5], raw_frame[6], raw_frame[7], raw_frame[8]]);
    let height = u32::from_le_bytes([raw_frame[9], raw_frame[10], raw_frame[11], raw_frame[12]]);
    let expected_size = 13 + (width * height * 3) as usize;
    assert_eq!(raw_frame.len(), expected_size, "Frame size mismatch");

    println!("✅ Captured frame: {}x{}, size: {} bytes", width, height, raw_frame.len());

    // Get rid of header for image saving
    let rgb_data = &raw_frame[13..];

    // write to file for manual inspection
    std::fs::write("test_output/test_frame.raw", &rgb_data).expect("Failed to write frame to file");
    
    println!("✅ Frame written to test_frame.raw for inspection");
    reader.stop_webcam().await;
}