//! Ergonomic entry points: capture-crate outputs in, xrds-net-ready streams out.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{channel, Receiver, RecvTimeoutError};
use std::sync::Arc;
use std::time::Duration;

use crate::audio::AudioFormat;
use crate::transcoding::jpeg2h264::Jpeg2H264Transcoder;
use crate::transcoding::pcm2opus::encode_pcm_to_opus;
use crate::video::FrameReader;

const OPUS_SAMPLE_RATE: u32 = 48_000;
const OPUS_CHANNELS: u16 = 2;
const OPUS_FRAME_MS: u32 = 20;

/// H264 Annex-B NAL start code.
const START_CODE: [u8; 4] = [0, 0, 0, 1];

/// A running JPEG->H264 transcode session. Dropping it stops the worker and
/// joins it (bounded by its 100ms `recv_timeout`, no arbitrary sleep).
pub struct H264StreamEncoder {
    shutdown: Arc<AtomicBool>,
    worker: Option<std::thread::JoinHandle<()>>,
}

impl H264StreamEncoder {
    pub fn stop(&mut self) {
        self.shutdown.store(true, Ordering::Relaxed);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

impl Drop for H264StreamEncoder {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Transcode a JPEG frame stream (e.g. from [`crate::video::Webcam::open`]) to
/// an H264 Annex-B byte stream — ready for `xrds_net::VideoSource::EncodedH264`
/// (which expects a `Box<dyn Read + Send>` of Annex-B bytes).
pub fn encode_jpeg_stream_to_h264(
    frame_rx: Receiver<Vec<u8>>,
    width: u32,
    height: u32,
    fps: u32,
) -> Result<(H264StreamEncoder, FrameReader), String> {
    let (out_tx, out_rx) = channel::<Vec<u8>>();
    let (ready_tx, ready_rx) = channel::<Result<(), String>>();
    let shutdown = Arc::new(AtomicBool::new(false));
    let stop = shutdown.clone();

    let worker = std::thread::spawn(move || {
        // Create the encoder here, on this dedicated fresh OS thread, not on
        // whatever (possibly shared/pooled) thread called this function. On
        // Windows, ffmpeg's h264_mf hardware encoder requires COM in MTA mode;
        // a thread reused from a pool (e.g. a tokio worker) may already have
        // COM set to STA by unrelated prior work, which makes h264_mf's own
        // init fail with "COM must not be in STA mode". A brand new thread
        // has no COM state yet, avoiding the conflict.
        let mut transcoder = match Jpeg2H264Transcoder::new(width, height, fps) {
            Ok(t) => {
                let _ = ready_tx.send(Ok(()));
                t
            }
            Err(e) => {
                let _ = ready_tx.send(Err(e.to_string()));
                return;
            }
        };

        log::info!("h264 stream encoder: worker started");
        loop {
            if stop.load(Ordering::Relaxed) {
                break;
            }
            match frame_rx.recv_timeout(Duration::from_millis(100)) {
                Ok(jpeg_frame) => match transcoder.transcode_jpeg_to_h264_packet(&jpeg_frame) {
                    Ok(pkts) => {
                        if send_annexb_packets(&out_tx, pkts).is_err() {
                            return; // consumer gone
                        }
                    }
                    Err(e) => log::error!("jpeg->h264 transcode error: {e:?}"),
                },
                Err(RecvTimeoutError::Timeout) => continue,
                Err(RecvTimeoutError::Disconnected) => break, // source ended
            }
        }
        if let Ok(final_pkts) = transcoder.flush_to_packets() {
            let _ = send_annexb_packets(&out_tx, final_pkts);
        }
        log::info!("h264 stream encoder: worker stopped");
    });

    match ready_rx.recv() {
        Ok(Ok(())) => {}
        Ok(Err(e)) => return Err(e),
        Err(_) => return Err("h264 encoder thread ended before initializing".to_string()),
    }

    Ok((
        H264StreamEncoder {
            shutdown,
            worker: Some(worker),
        },
        FrameReader::new(out_rx),
    ))
}

fn send_annexb_packets(
    out_tx: &std::sync::mpsc::Sender<Vec<u8>>,
    pkts: Vec<crate::transcoding::jpeg2h264::H264Packet>,
) -> Result<(), ()> {
    for pkt in pkts {
        let mut bytes = Vec::with_capacity(START_CODE.len() + pkt.data.len());
        bytes.extend_from_slice(&START_CODE);
        bytes.extend_from_slice(&pkt.data);
        out_tx.send(bytes).map_err(|_| ())?;
    }
    Ok(())
}

/// A running PCM->Opus encode session. Dropping it stops the worker and joins
/// it (bounded by its 100ms `recv_timeout`, no arbitrary sleep).
pub struct OpusStreamEncoder {
    shutdown: Arc<AtomicBool>,
    worker: Option<std::thread::JoinHandle<()>>,
}

impl OpusStreamEncoder {
    pub fn stop(&mut self) {
        self.shutdown.store(true, Ordering::Relaxed);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

impl Drop for OpusStreamEncoder {
    fn drop(&mut self) {
        self.stop();
    }
}

/// Resample + encode a PCM stream (e.g. from
/// [`crate::audio::Microphone::open_default`]) to Opus frames — ready for
/// `xrds_net::AudioSource` (which expects a `Receiver<Vec<u8>>` of Opus
/// frames).
pub fn encode_pcm_stream_to_opus(
    pcm_rx: Receiver<Vec<i16>>,
    format: AudioFormat,
) -> Result<(OpusStreamEncoder, Receiver<Vec<u8>>), String> {
    let mut encoder = opus::Encoder::new(
        OPUS_SAMPLE_RATE,
        opus::Channels::Stereo,
        opus::Application::Audio,
    )
    .map_err(|e| format!("failed to create Opus encoder: {e:?}"))?;

    let (opus_tx, opus_rx) = channel::<Vec<u8>>();
    let shutdown = Arc::new(AtomicBool::new(false));
    let stop = shutdown.clone();

    let frame_samples_per_channel = (format.sample_rate / 1000 * OPUS_FRAME_MS) as usize;
    let frame_total_samples = frame_samples_per_channel * format.channels as usize;

    let worker = std::thread::spawn(move || {
        log::info!(
            "opus stream encoder: {}Hz {} ch -> {}Hz {} ch",
            format.sample_rate,
            format.channels,
            OPUS_SAMPLE_RATE,
            OPUS_CHANNELS
        );
        let mut acc: Vec<i16> = Vec::new();
        loop {
            if stop.load(Ordering::Relaxed) {
                break;
            }
            match pcm_rx.recv_timeout(Duration::from_millis(100)) {
                Ok(chunk) => {
                    acc.extend_from_slice(&chunk);
                    while acc.len() >= frame_total_samples {
                        let frame: Vec<i16> = acc.drain(..frame_total_samples).collect();
                        let resampled = resample_and_convert(
                            &frame,
                            format.sample_rate,
                            format.channels,
                            OPUS_SAMPLE_RATE,
                            OPUS_CHANNELS,
                        );
                        match encode_pcm_to_opus(&mut encoder, &resampled) {
                            Ok(opus_frame) => {
                                if opus_tx.send(opus_frame).is_err() {
                                    return; // consumer gone
                                }
                            }
                            Err(e) => log::error!("opus encoding error: {e}"),
                        }
                    }
                }
                Err(RecvTimeoutError::Timeout) => continue,
                Err(RecvTimeoutError::Disconnected) => break, // source ended
            }
        }
        log::info!("opus stream encoder: worker stopped");
    });

    Ok((
        OpusStreamEncoder {
            shutdown,
            worker: Some(worker),
        },
        opus_rx,
    ))
}

/// Resample interleaved PCM `i16` between sample rates, preserving channel
/// interleaving (frame-based, not flat-sample, indexing).
///
/// Same channel count: nearest-neighbor resample per channel. Mono<->stereo:
/// duplicate/average. Other channel-count changes: nearest-neighbor per output
/// channel, clamping to the last available input channel.
fn resample_and_convert(
    input: &[i16],
    input_rate: u32,
    input_channels: u16,
    output_rate: u32,
    output_channels: u16,
) -> Vec<i16> {
    if input_channels == 0 || input.is_empty() {
        return Vec::new();
    }

    let input_channels = input_channels as usize;
    let output_channels_usize = output_channels as usize;
    let frames_in = input.len() / input_channels;
    if frames_in == 0 {
        return Vec::new();
    }

    let ratio = output_rate as f64 / input_rate as f64;
    let frames_out = ((frames_in as f64) * ratio).round().max(1.0) as usize;

    let mut output = Vec::with_capacity(frames_out * output_channels_usize);
    for out_frame in 0..frames_out {
        let in_frame = ((out_frame as f64 / ratio) as usize).min(frames_in - 1);
        let base = in_frame * input_channels;

        match (input_channels, output_channels_usize) {
            (a, b) if a == b => {
                output.extend_from_slice(&input[base..base + a]);
            }
            (1, 2) => {
                let s = input[base];
                output.push(s);
                output.push(s);
            }
            (2, 1) => {
                let l = input[base] as i32;
                let r = input[base + 1] as i32;
                output.push(((l + r) / 2) as i16);
            }
            (_, out_ch) => {
                for ch in 0..out_ch {
                    let src_ch = ch.min(input_channels - 1);
                    output.push(input[base + src_ch]);
                }
            }
        }
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_rate_same_channels_is_identity() {
        let input = vec![1i16, 2, 3, 4, 5, 6]; // 3 stereo frames
        let out = resample_and_convert(&input, 48_000, 2, 48_000, 2);
        assert_eq!(out, input);
    }

    #[test]
    fn preserves_stereo_interleaving_when_upsampling() {
        // Two stereo frames: (L=100,R=-100), (L=200,R=-200)
        let input = vec![100i16, -100, 200, -200];
        let out = resample_and_convert(&input, 24_000, 2, 48_000, 2);

        // Every frame must keep the L/R sign relationship (L positive, R negative);
        // this is exactly the invariant a flat-index resample would violate.
        assert_eq!(out.len() % 2, 0, "output must stay frame-aligned (even length)");
        for frame in out.chunks_exact(2) {
            assert!(frame[0] > 0, "L channel should stay positive, got {frame:?}");
            assert!(frame[1] < 0, "R channel should stay negative, got {frame:?}");
        }
    }

    #[test]
    fn mono_to_stereo_duplicates_channel() {
        let input = vec![42i16, 43];
        let out = resample_and_convert(&input, 48_000, 1, 48_000, 2);
        assert_eq!(out, vec![42, 42, 43, 43]);
    }

    #[test]
    fn stereo_to_mono_averages_channels() {
        let input = vec![10i16, 20];
        let out = resample_and_convert(&input, 48_000, 2, 48_000, 1);
        assert_eq!(out, vec![15]);
    }

    #[test]
    fn empty_input_yields_empty_output() {
        assert_eq!(resample_and_convert(&[], 48_000, 2, 48_000, 2), Vec::<i16>::new());
    }
}
