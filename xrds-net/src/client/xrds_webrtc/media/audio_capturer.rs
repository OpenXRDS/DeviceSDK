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
    // Only store what we actually need to keep
    audio_input_stream: Option<cpal::Stream>,
    audio_stream_shutdown: Option<Arc<std::sync::atomic::AtomicBool>>,
    
    // Store channels that won't be moved
    pcm_tx: Option<std::sync::mpsc::Sender<Vec<i16>>>,
    opus_rx: Option<std::sync::mpsc::Receiver<Vec<u8>>>,

    // Keep minimal device info for debugging/logging
    device_info: Option<AudioDeviceInfo>,
}

#[derive(Debug, Clone)]
struct AudioDeviceInfo {
    sample_rate: u32,
    channels: u16,
    format: String,
}

impl AudioCapturer {
    pub async fn new() -> Result<Self, String> {  // Remove async - not needed
        Ok(AudioCapturer {
            audio_input_stream: None,
            audio_stream_shutdown: None,
            pcm_tx: None,
            opus_rx: None,
            device_info: None,
        })
    }

    pub fn init(&mut self) -> Result<(), String> {
        // Use local variables instead of storing everything
        let host = cpal::default_host();
        let device = host.default_input_device().ok_or("No default input device found")?;
        let supported_config = device.default_input_config()
            .map_err(|e| format!("Failed to get default input config: {}", e))?;

        let device_sample_rate = supported_config.sample_rate().0;
        let device_channels = supported_config.channels();
        
        // Store minimal info for later use
        self.device_info = Some(AudioDeviceInfo {
            sample_rate: device_sample_rate,
            channels: device_channels,
            format: format!("{:?}", supported_config.sample_format()),
        });

        println!("Audio device: {}Hz, {} channels, format: {:?}", 
            device_sample_rate, device_channels, supported_config.sample_format());

        // Create channels - only store what we need
        let (pcm_tx, pcm_rx) = std::sync::mpsc::channel();
        let (opus_tx, opus_rx) = std::sync::mpsc::channel();
        
        self.pcm_tx = Some(pcm_tx.clone());
        self.opus_rx = Some(opus_rx);

        // Create encoder locally
        let opus_encoder = opus::Encoder::new(OPUS_SAMPLE_RATE, opus::Channels::Stereo, opus::Application::Audio)
            .map_err(|e| format!("Failed to create Opus encoder: {:?}", e))?;

        // Start processing thread immediately with all needed data
        self.start_processing_thread(pcm_rx, opus_tx, opus_encoder, device_sample_rate, device_channels)?;

        // Start audio stream
        self.start_audio_stream(device, supported_config, pcm_tx)?;

        println!("AudioCapturer initialized successfully");
        Ok(())
    }

    fn start_processing_thread(
        &mut self,
        pcm_rx: std::sync::mpsc::Receiver<Vec<i16>>,
        opus_tx: std::sync::mpsc::Sender<Vec<u8>>,
        mut encoder: opus::Encoder,
        device_sample_rate: u32,
        device_channels: u16,
    ) -> Result<(), String> {
        let shutdown_flag = Arc::new(std::sync::atomic::AtomicBool::new(false));
        self.audio_stream_shutdown = Some(shutdown_flag.clone());

        let device_frame_samples_per_channel = (device_sample_rate / 1000 * OPUS_FRAME_MS) as i32;
        let device_frame_total_samples = (device_frame_samples_per_channel * device_channels as i32) as usize;

        std::thread::spawn(move || {
            println!("Audio processing thread started ({}Hz {} ch -> {}Hz {} ch)", 
                device_sample_rate, device_channels, OPUS_SAMPLE_RATE, OPUS_CHANNELS);
            
            let mut acc: Vec<i16> = Vec::new();
            let mut frame_count = 0;

            loop {
                if shutdown_flag.load(std::sync::atomic::Ordering::Relaxed) {
                    break;
                }

                match pcm_rx.recv_timeout(std::time::Duration::from_millis(100)) {
                    Ok(pcm_chunk) => {
                        acc.extend_from_slice(&pcm_chunk);
                        
                        while acc.len() >= device_frame_total_samples {
                            let device_frame: Vec<i16> = acc.drain(0..device_frame_total_samples).collect();
                            
                            let resampled_frame = resample_and_convert(
                                &device_frame, device_sample_rate, device_channels, 
                                OPUS_SAMPLE_RATE, OPUS_CHANNELS
                            );
                            
                            match encode_pcm_to_opus(&mut encoder, &resampled_frame) {
                                Ok(opus_frame) => {
                                    frame_count += 1;
                                    if opus_tx.send(opus_frame).is_err() {
                                        break;
                                    }
                                    if frame_count % 50 == 0 {
                                        log::trace!("Processed {} audio frames", frame_count);
                                    }
                                }
                                Err(e) => log::error!("Opus encoding error: {}", e),
                            }
                        }
                    }
                    Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
                    Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
                }
            }
            
            println!("Audio processing thread ended: {} frames", frame_count);
        });

        Ok(())
    }

    fn start_audio_stream(
        &mut self,
        device: cpal::Device,
        config: cpal::SupportedStreamConfig,
        pcm_tx: std::sync::mpsc::Sender<Vec<i16>>,
    ) -> Result<(), String> {
        let stream = match config.sample_format() {
            SampleFormat::F32 => device.build_input_stream(
                &config.config(),
                move |data: &[f32], _| {
                    let pcm_data: Vec<i16> = data.iter()
                        .map(|&s| (s.clamp(-1.0, 1.0) * i16::MAX as f32) as i16)
                        .collect();
                    let _ = pcm_tx.send(pcm_data);
                },
                |err| eprintln!("Audio stream error: {}", err),
            ),
            SampleFormat::I16 => device.build_input_stream(
                &config.config(),
                move |data: &[i16], _| {
                    let _ = pcm_tx.send(data.to_vec());
                },
                |err| eprintln!("Audio stream error: {}", err),
            ),
            SampleFormat::U16 => device.build_input_stream(
                &config.config(),
                move |data: &[u16], _| {
                    let pcm_data: Vec<i16> = data.iter()
                        .map(|&s| (s as i32 - 0x8000) as i16)
                        .collect();
                    let _ = pcm_tx.send(pcm_data);
                },
                |err| eprintln!("Audio stream error: {}", err),
            ),
        };

        let stream = stream.map_err(|e| e.to_string())?;
        stream.play().map_err(|e| e.to_string())?;
        self.audio_input_stream = Some(stream);
        
        Ok(())
    }

    // Simplified WebRTC integration
    pub async fn connect_to_webrtc(&mut self, audio_track: Arc<TrackLocalStaticSample>) -> Result<(), String> {
        let opus_rx = self.opus_rx.take().ok_or("Audio not initialized")?;
        
        tokio::spawn(async move {
            let mut sample_count = 0;
            
            while let Ok(opus_frame) = opus_rx.recv() {
                sample_count += 1;
                log::trace!("Received Opus frame #{}", sample_count);
                let sample = webrtc::media::Sample {
                    data: bytes::Bytes::from(opus_frame),
                    duration: std::time::Duration::from_millis(20),
                    ..Default::default()
                };
                
                if let Err(e) = audio_track.write_sample(&sample).await {
                    log::error!("WebRTC write error: {:?}", e);
                    break;
                }
            }
        });
        
        Ok(())
    }

    pub fn stop_capture(&mut self) {
        if let Some(shutdown) = &self.audio_stream_shutdown {
            shutdown.store(true, std::sync::atomic::Ordering::Relaxed);
        }
        
        if let Some(stream) = self.audio_input_stream.take() {
            let _ = stream.pause();
        }
        
        std::thread::sleep(std::time::Duration::from_millis(500));
        self.audio_stream_shutdown = None;
    }
}

pub fn resample_and_convert(input: &[i16], input_rate:u32, input_channels: u16, output_rate: u32, output_channels: u16) -> Vec<i16> {
    // 16kHz(PCM) stereo -> 48kHz(OPUS) stereo: 3배 업샘플링
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