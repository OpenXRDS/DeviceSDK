//! File → RGBA frames, for playback rather than transport.
//!
//! The "media import and playback" half of this crate's mandate, which its own
//! module docs have listed as planned since the capture/transport split. Phase 0 of
//! `docs/video-asset-spike.md`.
//!
//! **Desktop only, and deliberately so.** This decodes in software via ffmpeg and
//! converts to RGBA on the CPU, which is the wrong shape for a headset: a Quest
//! needs MediaCodec's hardware decoder writing into an `AHardwareBuffer` the GPU
//! samples directly, with no CPU copy at any point. That path is the spike's real
//! subject; this exists to settle the Bevy-side question — how a frame becomes a
//! material — without Android in the way.
//!
//! Do not reach for this to play video on a headset. It will appear to work and
//! cost more than the frame budget.

use ffmpeg_next as ffmpeg;
use ffmpeg::format::Pixel;
use ffmpeg::software::scaling::{context::Context as Scaler, flag::Flags};
use std::path::Path;

/// One decoded frame, already in the layout a GPU texture wants.
pub struct VideoFrame {
    pub width: u32,
    pub height: u32,
    /// Tightly packed RGBA8, `width * height * 4` bytes.
    pub rgba: Vec<u8>,
    /// Presentation time from the start of the stream.
    pub pts_secs: f64,
}

/// Pull-based software decoder over a video file.
///
/// Pull rather than push so the caller owns pacing. A renderer that has fallen
/// behind should skip frames rather than queue them, and only the caller knows
/// which frame is wanted now — see `next_frame`.
pub struct VideoDecoder {
    input: ffmpeg::format::context::Input,
    decoder: ffmpeg::decoder::Video,
    scaler: Scaler,
    stream_index: usize,
    time_base: f64,
    width: u32,
    height: u32,
    frame_rate: f64,
}

// Moved to a worker thread, which is the only way it is useful: software decode is
// slower than real time at 4K even on a desktop, so decoding on the render thread
// would pace the renderer to the decoder.
//
// ffmpeg's `AVFormatContext`, `AVCodecContext` and `SwsContext` are not safe for
// *concurrent* use, but are fine to use from any single thread. This struct owns all
// three exclusively and every method takes `&mut self`, so concurrent access is not
// expressible. The raw pointers simply carry no `Send`, which is what this asserts —
// nothing more.
unsafe impl Send for VideoDecoder {}

impl VideoDecoder {
    /// Open a file and prepare its best video stream for decoding.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, ffmpeg::Error> {
        // Idempotent, and cheap after the first call — callers should not have to
        // know whether something else already initialised ffmpeg.
        ffmpeg::init()?;

        let input = ffmpeg::format::input(&path.as_ref())?;
        let stream = input
            .streams()
            .best(ffmpeg::media::Type::Video)
            .ok_or(ffmpeg::Error::StreamNotFound)?;
        let stream_index = stream.index();
        let time_base = f64::from(stream.time_base().numerator())
            / f64::from(stream.time_base().denominator());
        let frame_rate = {
            let r = stream.avg_frame_rate();
            if r.denominator() == 0 {
                30.0
            } else {
                f64::from(r.numerator()) / f64::from(r.denominator())
            }
        };

        let decoder = ffmpeg::codec::context::Context::from_parameters(stream.parameters())?
            .decoder()
            .video()?;
        let (width, height) = (decoder.width(), decoder.height());

        // Straight to RGBA: what `bevy_image`'s `Rgba8UnormSrgb` wants, so the
        // upload is a memcpy rather than a conversion in a hot system.
        let scaler = Scaler::get(
            decoder.format(),
            width,
            height,
            Pixel::RGBA,
            width,
            height,
            Flags::BILINEAR,
        )?;

        Ok(Self {
            input,
            decoder,
            scaler,
            stream_index,
            time_base,
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

    /// Decode the next frame, or `None` at end of stream.
    ///
    /// One packet can yield zero frames (B-frame reordering, or a decoder still
    /// filling its buffer), so this loops until a frame emerges rather than
    /// treating an empty receive as the end.
    pub fn next_frame(&mut self) -> Result<Option<VideoFrame>, ffmpeg::Error> {
        let mut decoded = ffmpeg::frame::Video::empty();

        loop {
            // Anything the decoder is already holding, before reading more.
            if self.decoder.receive_frame(&mut decoded).is_ok() {
                return Ok(Some(self.convert(&mut decoded)?));
            }

            let Some((stream, packet)) = self.input.packets().next() else {
                // Drain: a decoder can hold several frames past the last packet.
                self.decoder.send_eof()?;
                return if self.decoder.receive_frame(&mut decoded).is_ok() {
                    Ok(Some(self.convert(&mut decoded)?))
                } else {
                    Ok(None)
                };
            };

            if stream.index() == self.stream_index {
                self.decoder.send_packet(&packet)?;
            }
        }
    }

    fn convert(&mut self, decoded: &mut ffmpeg::frame::Video) -> Result<VideoFrame, ffmpeg::Error> {
        let mut rgba = ffmpeg::frame::Video::empty();
        self.scaler.run(decoded, &mut rgba)?;

        // `data(0)` is padded to the scaler's stride, which is not necessarily
        // `width * 4`. Copying row by row rather than wholesale is the difference
        // between a correct image and a diagonally sheared one.
        let stride = rgba.stride(0);
        let row_bytes = self.width as usize * 4;
        let mut out = Vec::with_capacity(row_bytes * self.height as usize);
        let plane = rgba.data(0);
        for y in 0..self.height as usize {
            let start = y * stride;
            out.extend_from_slice(&plane[start..start + row_bytes]);
        }

        Ok(VideoFrame {
            width: self.width,
            height: self.height,
            rgba: out,
            pts_secs: decoded.pts().unwrap_or(0) as f64 * self.time_base,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Runs only when XRDS_TEST_VIDEO points at a clip. Opt-in because no video
    /// belongs in the repository — they are tens to hundreds of megabytes, and git
    /// keeps them forever.
    ///
    /// Reports throughput as well as correctness: software decode plus a CPU RGBA
    /// conversion is the part that will *not* transfer to a headset, and knowing
    /// what it costs on a desktop is what makes that concrete rather than asserted.
    #[test]
    fn xxx_decode_a_real_clip() {
        let Ok(path) = std::env::var("XRDS_TEST_VIDEO") else { return };

        let mut decoder = VideoDecoder::open(&path).expect("should open");
        let (w, h) = (decoder.width(), decoder.height());
        println!("[video] {w}x{h} @ {:.1} fps", decoder.frame_rate());

        let want: u32 = std::env::var("XRDS_TEST_VIDEO_FRAMES")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(30);
        // Accumulated around `next_frame` only. Timing the whole loop would fold in
        // the per-frame inspection below, which is what made an earlier run report
        // 303 ms/frame for a decoder that costs 20.
        let mut decode_time = std::time::Duration::ZERO;
        let mut count = 0;
        let mut last_pts = -1.0;
        while count < want {
            let t = std::time::Instant::now();
            let next = decoder.next_frame().expect("decode should not fail");
            decode_time += t.elapsed();
            match next {
                Some(frame) => {
                    assert_eq!(frame.width, w);
                    assert_eq!(frame.height, h);
                    assert_eq!(
                        frame.rgba.len(),
                        (w * h * 4) as usize,
                        "a short buffer means the stride copy is wrong, which renders as shear"
                    );
                    assert!(
                        frame.pts_secs >= last_pts,
                        "presentation times must not go backwards: {} then {}",
                        last_pts,
                        frame.pts_secs
                    );
                    // Mean pixel value, on the sampled frames only.
                    //
                    // It distinguishes "the decoder produced black" from "the frame
                    // never reached the GPU" — which look identical on screen and
                    // have nothing in common as bugs. It answered exactly that once:
                    // a quad showing black turned out to be a clip that opens on a
                    // fade from black, with nothing wrong anywhere.
                    //
                    // Computed inside the `if`, not outside it. Summing 8.4M pixels
                    // per frame took the decode figure from 19.8 ms to 303 ms — a
                    // measurement destroyed by its own instrumentation.
                    if count < 3 || count % 60 == 0 || count == want - 1 {
                        let sum: u64 = frame.rgba.chunks_exact(4)
                            .map(|p| p[0] as u64 + p[1] as u64 + p[2] as u64)
                            .sum();
                        let mean = sum as f64 / (frame.rgba.len() as f64 / 4.0 * 3.0);
                        let alpha_min =
                            frame.rgba.chunks_exact(4).map(|p| p[3]).min().unwrap_or(0);
                        println!(
                            "[video] frame {count}: mean rgb {mean:.1}/255, min alpha {alpha_min}"
                        );
                    }
                    last_pts = frame.pts_secs;
                    count += 1;
                }
                None => break,
            }
        }

        let elapsed = decode_time;
        let per_frame = elapsed.as_secs_f64() / count as f64;
        println!(
            "[video] {count} frames in {elapsed:?} — {:.1} ms/frame, {:.1} fps decode ceiling",
            per_frame * 1000.0,
            1.0 / per_frame
        );
        println!(
            "[video] {:.1} MB/frame of RGBA, {:.2} GB/s at {:.0} fps playback",
            (w * h * 4) as f64 / 1e6,
            (w * h * 4) as f64 * decoder.frame_rate() / 1e9,
            decoder.frame_rate()
        );
        assert!(count > 0, "no frames decoded");
    }

    /// Writes decoded frames out as PNG so they can be inspected directly.
    ///
    /// A mean pixel value is a poor proxy for "does this look right". Shear, a
    /// channel swap and a genuinely dark clip all reduce to numbers that need
    /// interpreting, and interpreting them wrong cost this spike three rounds of
    /// debugging. The frames themselves need no interpretation.
    ///
    ///     XRDS_TEST_VIDEO=clip.mp4 XRDS_TEST_VIDEO_DUMP=out/ cargo test     ///         -p xrds-media --features playback xxx_dump -- --nocapture
    #[test]
    fn xxx_dump_frames_as_png() {
        let (Ok(path), Ok(dir)) = (
            std::env::var("XRDS_TEST_VIDEO"),
            std::env::var("XRDS_TEST_VIDEO_DUMP"),
        ) else {
            return;
        };
        std::fs::create_dir_all(&dir).expect("dump directory should be creatable");

        let mut decoder = VideoDecoder::open(&path).expect("should open");
        let (w, h) = (decoder.width(), decoder.height());

        // Every 30th frame over the first 300. Consecutive frames of a video are
        // nearly identical, so dumping the first N would answer nothing about
        // whether the picture changes over time — which is the actual question.
        let mut count = 0;
        while count < 300 {
            let Some(frame) = decoder.next_frame().expect("decode should not fail") else {
                break;
            };
            if count % 30 == 0 {
                let img: image::RgbaImage =
                    image::ImageBuffer::from_raw(w, h, frame.rgba.clone())
                        .expect("buffer should match the declared size");
                let out = format!("{dir}/frame_{count:04}.png");
                img.save(&out).expect("png should write");
                println!("[video] wrote {out}");
            }
            count += 1;
        }
        assert!(count > 0, "no frames decoded");
    }
}
