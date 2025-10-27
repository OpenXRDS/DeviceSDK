/*************************************************************************************
 * Audio Capturer
 * - Captures audio from the microphone using cpal
 * - transcodes audio(PCM) to Opus format by using pcm2opus module

 */

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::SampleFormat;
use crate::client::xrds_webrtc::media::transcoding::pcm2opus::encode_pcm_to_opus;
use std::sync::Arc;
use webrtc::track::track_local::track_local_static_sample::TrackLocalStaticSample;

extern crate pretty_env_logger;
extern crate log;


static OPUS_SAMPLE_RATE: u32 = 48000;
static OPUS_CHANNELS: u16 = 2;
static OPUS_FRAME_MS: u32 = 20; // 20ms per frame

pub struct AudioCapturer {
    // Placeholder for audio capturer state
    host: cpal::Host,
    device: Option<cpal::Device>,
    supported_stream_config: Option<cpal::SupportedStreamConfig>,

    device_sample_rate: u32,
    device_channels: u16,

    audio_input_stream: Option<cpal::Stream>,
    audio_stream_shutdown: Option<std::sync::Arc<std::sync::atomic::AtomicBool>>,
    pcm_tx: Option<std::sync::mpsc::Sender<Vec<i16>>>,
    pcm_rx: Option<std::sync::mpsc::Receiver<Vec<i16>>>,

    opus_tx: Option<std::sync::mpsc::Sender<Vec<u8>>>,
    opus_rx: Option<std::sync::mpsc::Receiver<Vec<u8>>>,

    opus_encoder: Option<opus::Encoder>,
}

impl AudioCapturer {
    pub async fn new() -> Result<Self, String> {
        Ok(AudioCapturer {
            host: cpal::default_host(),
            device: None,
            supported_stream_config: None,
            device_sample_rate: 0,
            device_channels: 0,
            audio_input_stream: None,
            audio_stream_shutdown: None,
            pcm_tx: None,
            pcm_rx: None,
            opus_tx: None,
            opus_rx: None,
            opus_encoder: None,
        })
    }

    pub fn start_capture(&mut self) -> Result<(), String> {
        // Ensure initialization is complete
        if self.pcm_tx.is_none() || self.pcm_rx.is_none() || self.opus_encoder.is_none() {
            return Err("AudioCapturer not properly initialized. Call init() first.".to_string());
        }

        println!("Audio capture started.");

        // Create shutdown flag for the processing thread
        let shutdown_flag = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        self.audio_stream_shutdown = Some(shutdown_flag.clone());

        // Start the PCM processing thread BEFORE starting the audio stream
        self.spawn_processing_thread()?;

        // Now start the audio input stream
        let tx_cb = self.pcm_tx.as_ref().unwrap().clone();
        let stream = match self.supported_stream_config.as_ref().unwrap().sample_format() {
            SampleFormat::F32 => self.device.as_ref().unwrap().build_input_stream(
                &self.supported_stream_config.as_ref().unwrap().config(),
                move |data: &[f32], _: &cpal::InputCallbackInfo| {
                    let pcm_data: Vec<i16> = data.iter()
                        .map(|&s| (s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16)
                        .collect();
                    let _ = tx_cb.send(pcm_data); // Ignore send errors (non-blocking)
                },
                move |err| {
                    eprintln!("Error occurred on input stream: {}", err);
                },
            ),
            SampleFormat::I16 => self.device.as_ref().unwrap().build_input_stream(
                &self.supported_stream_config.as_ref().unwrap().config(),
                move |data: &[i16], _: &cpal::InputCallbackInfo| {
                    let _ = tx_cb.send(data.to_vec());
                },
                move |err| {
                    eprintln!("Error occurred on input stream: {}", err);
                },
            ),
            SampleFormat::U16 => self.device.as_ref().unwrap().build_input_stream(
                &self.supported_stream_config.as_ref().unwrap().config(),
                move |data: &[u16], _: &cpal::InputCallbackInfo| {
                    let pcm_data: Vec<i16> = data.iter()
                        .map(|&s| (s as i32 - 0x8000) as i16)
                        .collect();
                    let _ = tx_cb.send(pcm_data);
                },
                move |err| {
                    eprintln!("Error occurred on input stream: {}", err);
                },
            ),
        };

        let stream = stream.map_err(|e| e.to_string())?;
        stream.play().map_err(|e| e.to_string())?;
        self.audio_input_stream = Some(stream);

        Ok(())
    }

    /**
     * Spawn the PCM processing thread
     */
    fn spawn_processing_thread(&mut self) -> Result<(), String> {
        // Move necessary data to thread
        let pcm_rx = self.pcm_rx.take().ok_or("PCM receiver not initialized")?;
        let opus_tx = self.opus_tx.as_ref().unwrap().clone();
        let shutdown_flag = self.audio_stream_shutdown.as_ref().unwrap().clone();
        
        // Move encoder to thread (we'll need to restructure this)
        let mut encoder = self.opus_encoder.take().ok_or("Opus encoder not initialized")?;
        
        // Audio parameters
        let device_sample_rate = self.device_sample_rate;
        let device_channels = self.device_channels;
        let device_frame_samples_per_channel = (device_sample_rate / 1000 * OPUS_FRAME_MS) as i32;
        let device_frame_total_samples = (device_frame_samples_per_channel * device_channels as i32) as usize;

        std::thread::spawn(move || {
            println!("PCM processing thread started ({}Hz {} ch -> {}Hz {} ch)", 
                device_sample_rate, device_channels, OPUS_SAMPLE_RATE, OPUS_CHANNELS);
            
            let mut acc: Vec<i16> = Vec::new();
            let mut frame_count = 0;

            loop {
                if shutdown_flag.load(std::sync::atomic::Ordering::Relaxed) {
                    println!("PCM processing thread stopping");
                    break;
                }

                match pcm_rx.recv_timeout(std::time::Duration::from_millis(500)) {
                    Ok(pcm_chunk) => {
                        acc.extend_from_slice(&pcm_chunk);
                        
                        while acc.len() >= device_frame_total_samples {
                            let device_frame: Vec<i16> = acc.drain(0..device_frame_total_samples).collect();
                            
                            // Resample and convert channels
                            let resampled_frame = resample_and_convert(
                                &device_frame, 
                                device_sample_rate, 
                                device_channels, 
                                OPUS_SAMPLE_RATE, 
                                OPUS_CHANNELS
                            );
                            
                            // Encode to Opus
                            match encode_pcm_to_opus(&mut encoder, &resampled_frame) {
                                Ok(opus_frame) => {
                                    frame_count += 1;
                                    
                                    if let Err(_) = opus_tx.send(opus_frame) {
                                        println!("Opus channel disconnected, stopping processing thread");
                                        break;
                                    }
                                    
                                    // Log progress every second (48kHz/960 = 50 frames per second)
                                    if frame_count % 50 == 0 {
                                        println!("Processed {} Opus frames ({:.1}s)", 
                                            frame_count, frame_count as f32 / 50.0);
                                    }
                                }
                                Err(e) => {
                                    eprintln!("Opus encoding error: {}", e);
                                }
                            }
                        }
                    }
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                        // Timeout is normal - continue
                        continue;
                    }
                    Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                        println!("PCM channel disconnected, stopping processing thread");
                        break;
                    }
                }
            }
            
            println!("PCM processing thread ended. Total frames processed: {}", frame_count);
        });

        Ok(())
    }

    pub fn stop_capture(&mut self) {
        println!("Stopping audio capture...");
        
        // Signal the processing thread to stop
        if let Some(shutdown_flag) = &self.audio_stream_shutdown {
            shutdown_flag.store(true, std::sync::atomic::Ordering::Relaxed);
        }
        
        // Stop the audio stream
        if let Some(stream) = self.audio_input_stream.take() {
            let _ = stream.pause();
            println!("Audio input stream stopped");
        }
        
        // Give the processing thread time to finish
        std::thread::sleep(std::time::Duration::from_millis(1000));
        
        // Clean up
        self.audio_stream_shutdown = None;
        println!("Audio capture stopped");
    }

    pub fn init(&mut self) -> Result<(), String> {
        let host = cpal::default_host();
        let device = host.default_input_device().ok_or("No default input device found")?;
        let supported_stream_config = device.default_input_config()
            .map_err(|e| format!("Failed to get default input config: {}", e))?;

        let device_sample_rate = supported_stream_config.sample_rate().0;
        let device_channels = supported_stream_config.channels();
        
        println!("Audio device config: {}Hz, {} channels, format: {:?}", 
            device_sample_rate, device_channels, supported_stream_config.sample_format());

        // Assign device info
        self.host = host;
        self.device = Some(device);
        self.supported_stream_config = Some(supported_stream_config);
        self.device_sample_rate = device_sample_rate;
        self.device_channels = device_channels;

        // Create channels
        let (pcm_tx, pcm_rx) = std::sync::mpsc::channel();
        self.pcm_tx = Some(pcm_tx);
        self.pcm_rx = Some(pcm_rx);

        let (opus_tx, opus_rx) = std::sync::mpsc::channel();
        self.opus_tx = Some(opus_tx);
        self.opus_rx = Some(opus_rx);

        // Initialize Opus encoder
        let opus_encoder = opus::Encoder::new(OPUS_SAMPLE_RATE, opus::Channels::Stereo, opus::Application::Audio)
            .map_err(|e| format!("Failed to create Opus encoder: {:?}", e))?;
        self.opus_encoder = Some(opus_encoder);

        println!("AudioCapturer initialized successfully");
        Ok(())
    }

    /**
     * Start capture and directly write to WebRTC track
     */
    pub async fn start_capture_direct_to_webrtc(&mut self, audio_track: Arc<TrackLocalStaticSample>) -> Result<(), String> {
        // Start regular capture first
        self.start_capture()?;
        
        // Then add WebRTC writer task
        let opus_rx = self.opus_rx.take().ok_or("Opus receiver not available")?;
        
        tokio::spawn(async move {
            let mut sample_count = 0;
            println!("🎯 WebRTC audio writer task started");
            
            while let Ok(opus_frame) = opus_rx.recv() {
                sample_count += 1;
                
                let sample = webrtc::media::Sample {
                    data: bytes::Bytes::from(opus_frame),
                    duration: std::time::Duration::from_millis(20),
                    ..Default::default()
                };
                
                if let Err(e) = audio_track.write_sample(&sample).await {
                    eprintln!("❌ WebRTC audio write error: {:?}", e);
                } else if sample_count % 50 == 0 {
                    println!("📡 Sent {} samples to WebRTC track", sample_count);
                }
            }
            
            println!("🔚 WebRTC audio writer ended: {} samples", sample_count);
        });
        
        println!("✅ Audio capture direct to WebRTC started successfully");
        Ok(())
    }
}

pub fn resample_and_convert(input: &[i16], input_rate:u32, input_channels: u16, output_rate: u32, output_channels: u16) -> Vec<i16> {
    // 16kHz stereo -> 48kHz stereo: 3배 업샘플링
    let ratio = output_rate as f32 / input_rate as f32; // 3.0
    let output_len = ((input.len() as f32 * ratio) as usize / output_channels as usize) * output_channels as usize;
    let mut output = Vec::with_capacity(output_len);

    if input_channels == output_channels {
        // 채널 수가 같으면 단순 업샘플링
        for i in 0..output_len {
            let input_idx = ((i as f32 / ratio) as usize).min(input.len() - 1);
            output.push(input[input_idx]);
        }
    } else if input_channels == 2 && output_channels == 2 {
        // 스테레오 -> 스테레오 리샘플링
        let frames_out = output_len / 2;
        for frame in 0..frames_out {
            let input_frame = ((frame as f32 / ratio) as usize).min(input.len() / 2 - 1);
            output.push(input[input_frame * 2]);     // left
            output.push(input[input_frame * 2 + 1]); // right
        }
    } else {
        // 다른 채널 조합은 일단 단순 복사/확장
        for i in 0..output_len {
            let input_idx = ((i as f32 / ratio) as usize).min(input.len() - 1);
            output.push(input[input_idx]);
        }
    }
    
    output
}