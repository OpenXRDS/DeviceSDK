//! A decoder that keeps time, on a thread of its own.
//!
//! [`VideoDecoder`] is deliberately pull-based and unpaced — the caller owns timing.
//! That is right for a measurement harness and wrong for playback, where two things
//! must be true and neither is obvious:
//!
//! - **Frames come out at the clip's rate, not the decoder's.** Software decode is
//!   far faster than real time: 270 fps for a 24 fps clip, measured. Unpaced, a clip
//!   plays at eleven times speed and runs off its end within seconds, and what
//!   remains on screen is the last frame, frozen — which looks exactly like a
//!   texture that stopped updating.
//! - **End of stream is a decision, not an accident.** Looping is the default an
//!   author wants for a screen; the caller chooses. A clip that merely stops leaves
//!   a frozen final frame, which is indistinguishable from a texture that died —
//!   both cost this project real debugging time, which is why a *check* harness
//!   should always loop even when a scene need not.
//!
//! Decoding also has to be off the render thread: it is slower than real time at 4K
//! even on a desktop, so decoding inline would pace the renderer to the decoder.

use std::sync::mpsc::{sync_channel, Receiver, TrySendError};
use std::time::{Duration, Instant};

use super::{VideoDecoder, VideoFrame};

/// One frame in flight behind the one being shown.
///
/// Deliberately tiny. A deeper queue lets the decoder run ahead and hides its true
/// cost behind buffering, and for playback a late frame should be *dropped* rather
/// than queued — otherwise the video drifts further behind the longer it runs.
const QUEUE_DEPTH: usize = 1;

/// A video file decoding in the background at its own presentation rate.
///
/// Dropping this stops the decoder: the receiver goes away and the thread's next
/// send fails, which is how it learns to exit.
pub struct PacedVideo {
    frames: Receiver<VideoFrame>,
    width: u32,
    height: u32,
    frame_rate: f64,
}

impl PacedVideo {
    /// Open a clip and start decoding it, paced.
    ///
    /// `looping` restarts at end of stream. When false the thread simply exits, and
    /// the surface keeps the last frame — which is the honest end state for a clip
    /// that has finished, and the reason a *check* harness should always loop: a
    /// frozen final frame and a dead decoder look identical.
    pub fn open(
        path: impl AsRef<std::path::Path>,
        looping: bool,
    ) -> Result<Self, ffmpeg_next::Error> {
        let path = path.as_ref().to_path_buf();
        let decoder = VideoDecoder::open(&path)?;
        let (width, height, frame_rate) = (decoder.width(), decoder.height(), decoder.frame_rate());

        let (tx, frames) = sync_channel::<VideoFrame>(QUEUE_DEPTH);
        std::thread::spawn(move || {
            let mut decoder = decoder;
            let mut epoch = Instant::now();
            loop {
                match decoder.next_frame() {
                    Ok(Some(frame)) => {
                        // Hold the frame until its presentation time is due.
                        let due = Duration::from_secs_f64(frame.pts_secs.max(0.0));
                        if let Some(wait) = due.checked_sub(epoch.elapsed()) {
                            std::thread::sleep(wait);
                        }
                        match tx.try_send(frame) {
                            // Full means the consumer has not taken the previous
                            // frame yet. Dropping is correct: it only ever wants the
                            // newest one, and blocking here would make the decoder
                            // the pacer by another route.
                            Ok(()) | Err(TrySendError::Full(_)) => {}
                            Err(TrySendError::Disconnected(_)) => return,
                        }
                    }
                    Ok(None) if looping => match VideoDecoder::open(&path) {
                        Ok(fresh) => {
                            decoder = fresh;
                            epoch = Instant::now();
                        }
                        Err(e) => {
                            log::error!("video loop: cannot reopen {}: {e}", path.display());
                            return;
                        }
                    },
                    Ok(None) => return,
                    Err(e) => {
                        log::error!("video decode {}: {e}", path.display());
                        return;
                    }
                }
            }
        });

        Ok(Self {
            frames,
            width,
            height,
            frame_rate,
        })
    }

    pub fn width(&self) -> u32 {
        self.width
    }
    pub fn height(&self) -> u32 {
        self.height
    }
    /// Nominal frames per second, or 30 when the container does not say.
    pub fn frame_rate(&self) -> f64 {
        self.frame_rate
    }

    /// The newest decoded frame, or `None` if none has arrived since the last call.
    ///
    /// Drains rather than takes one: anything behind the newest frame is already
    /// late, and showing it would only put the picture further behind.
    pub fn newest_frame(&self) -> Option<VideoFrame> {
        let mut newest = None;
        while let Ok(frame) = self.frames.try_recv() {
            newest = Some(frame);
        }
        newest
    }
}
