/*
Copyright 2025 KETI

Licensed under the Apache License, Version 2.0 (the "License");
you may not use this file except in compliance with the License.
You may obtain a copy of the License at

     https://www.apache.org/licenses/LICENSE-2.0

Unless required by applicable law or agreed to in writing, software
distributed under the License is distributed on an "AS IS" BASIS,
WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
See the License for the specific language governing permissions and
limitations under the License.
*/

//! Writes an already-encoded Opus frame stream to a WebRTC audio track.
//!
//! This is transport only — no PCM, no resampling, no Opus encoding. That used
//! to live here (as the old `AudioCapturer`, and later a PCM-consuming
//! `AudioEncoder`), but codec encoding is a media concern, not a networking
//! one (see `docs/done/xrds-net-capture-decoupling.md`); it now lives in
//! `xrds_media::transcoding`. This module only bridges the caller's
//! [`AudioSource`] (a plain `std::sync::mpsc::Receiver`, so the caller isn't
//! forced to depend on tokio) onto the async `write_sample` call.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::RecvTimeoutError;
use std::sync::Arc;
use std::time::Duration;

use webrtc::media::Sample;
use webrtc::track::track_local::track_local_static_sample::TrackLocalStaticSample;

use crate::client::xrds_webrtc::media::source::AudioSource;

/// Assumed Opus frame duration for track-write pacing. 20ms is the standard
/// Opus frame size and what `xrds_media::transcoding::encode_pcm_stream_to_opus`
/// produces; xrds-net has no way to know the true value since it never
/// encodes, so this is a best-effort convention, matching how the video path
/// assumes a fixed sample duration too.
const OPUS_FRAME_DURATION: Duration = Duration::from_millis(20);

/// Writes an [`AudioSource`]'s Opus frames to a WebRTC audio track.
///
/// Runs a bridge thread (blocking `recv` on the caller's sync channel) feeding
/// an async write task (`write_sample`), since the two can't share one
/// executor. Dropping (or calling [`AudioTrackWriter::stop`]) signals both to
/// stop and joins them — no arbitrary sleep, the bridge checks the shutdown
/// flag at least every 100ms (its `recv_timeout`).
pub struct AudioTrackWriter {
    shutdown: Arc<AtomicBool>,
    bridge: Option<std::thread::JoinHandle<()>>,
    write_task: Option<tokio::task::JoinHandle<()>>,
}

impl AudioTrackWriter {
    pub fn spawn(source: AudioSource, track: Arc<TrackLocalStaticSample>) -> Self {
        let AudioSource { rx: opus_rx } = source;

        let (bridge_tx, mut bridge_rx) = tokio::sync::mpsc::channel::<Vec<u8>>(64);
        let shutdown = Arc::new(AtomicBool::new(false));
        let stop = shutdown.clone();

        let bridge = std::thread::spawn(move || {
            loop {
                if stop.load(Ordering::Relaxed) {
                    break;
                }
                match opus_rx.recv_timeout(Duration::from_millis(100)) {
                    Ok(frame) => {
                        if bridge_tx.blocking_send(frame).is_err() {
                            return; // write task gone
                        }
                    }
                    Err(RecvTimeoutError::Timeout) => continue,
                    Err(RecvTimeoutError::Disconnected) => break, // source ended
                }
            }
        });

        let write_task = tokio::spawn(async move {
            while let Some(opus_frame) = bridge_rx.recv().await {
                let sample = Sample {
                    data: bytes::Bytes::from(opus_frame),
                    duration: OPUS_FRAME_DURATION,
                    ..Default::default()
                };
                if let Err(e) = track.write_sample(&sample).await {
                    log::error!("audio track write error: {e:?}");
                    break;
                }
            }
        });

        Self {
            shutdown,
            bridge: Some(bridge),
            write_task: Some(write_task),
        }
    }

    /// Signal the bridge thread to stop, join it, then await the write task's
    /// completion (bounded to 5s in case the track is stuck).
    pub async fn stop(&mut self) {
        self.shutdown.store(true, Ordering::Relaxed);
        if let Some(bridge) = self.bridge.take() {
            let _ = bridge.join();
        }
        if let Some(write_task) = self.write_task.take() {
            let _ = tokio::time::timeout(Duration::from_secs(5), write_task).await;
        }
    }
}

impl Drop for AudioTrackWriter {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Relaxed);
        // Best-effort on drop: we can't `.await` here, so just signal and let
        // the bridge thread's own join (if `stop()` was called) or process
        // exit handle cleanup. Callers that need a guaranteed clean shutdown
        // should call `stop().await` explicitly (as `WebRTCClient::stop_stream`
        // does).
        if let Some(bridge) = self.bridge.take() {
            let _ = bridge.join();
        }
    }
}
