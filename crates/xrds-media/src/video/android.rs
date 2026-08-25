//! Hardware video decode on Android: file → `AMediaCodec` → `AHardwareBuffer`.
//!
//! Phase 1/B1 of `docs/video-asset-spike.md`. The counterpart to [`super::decode`],
//! which decodes in software via ffmpeg for the desktop. Both end at "a frame you
//! can put on a surface"; only this one gets there without the CPU touching a pixel.
//!
//! # Why not just use the ffmpeg path here too
//!
//! It would appear to work and cost more than the frame budget. Software decode plus
//! a CPU RGBA conversion measured 0.31 ms/frame *for the upload alone* at 1920×800
//! on a desktop; at 4096×2048 on an Adreno the copy alone is ~3 ms of a 13.9 ms
//! frame, before decoding anything. The whole point of this module is that the
//! decoded frame never becomes bytes in main memory.
//!
//! # What this produces, and what it deliberately does not
//!
//! It hands back an `AHardwareBuffer` and stops. Turning that into something a
//! renderer can sample is a Vulkan/wgpu problem, not a media one, and it lives with
//! the renderer — this crate does not depend on wgpu and should not learn to.
//!
//! # The buffer is opaque, and asking nicely does not change that
//!
//! Measured on a Quest 3 (Adreno 740) before any of this was written:
//!
//! ```text
//! AImageReader created with YUV_420_888 (4096x2048)   <- the request is accepted
//! AHardwareBuffer: format=0x7fa30c06                  <- Qualcomm vendor layout
//! 720 frames, 30.0 fps sustained                      <- still zero-copy
//! VkFormat = 0 (VK_FORMAT_UNDEFINED), external = 0x1fa
//! ```
//!
//! Requesting `AIMAGE_FORMAT_YUV_420_888` instead of `PRIVATE` succeeds, runs at
//! full rate, and *still* yields a vendor-tiled buffer that Vulkan describes only by
//! external format. So there is no addressable-planes shortcut to be had, and this
//! module uses `PRIVATE` — the configuration already proven at 4096×2048 at 30 fps.
//! The consumer must import via an external format with a `VkSamplerYcbcrConversion`
//! and convert into a normal texture.
//!
//! # Provenance
//!
//! Ported from `HMDViewer` (`src/video/{mod,decoder}.rs`), which has run this
//! pipeline on a Quest 3 in production. Structure and the non-obvious constants are
//! kept deliberately close to the original so fixes can travel between them.

use ndk_sys as sys;
use std::collections::VecDeque;
use std::ffi::CStr;
use std::fmt;
use std::fs::File;
use std::os::fd::AsRawFd;
use std::path::Path;
use std::time::{Duration, Instant};

/// How many acquired images to keep alive behind the current one.
///
/// The GPU may still be sampling a buffer after a newer frame has arrived. Releasing
/// the image that owns it lets the reader recycle it underneath an in-flight draw,
/// which corrupts a frame rather than failing — the kind of bug that is invisible
/// until it is intermittent. Three is HMDViewer's proven value.
const IMAGE_KEEPALIVE: usize = 3;

/// Reader pool size. Must exceed `IMAGE_KEEPALIVE` plus the producer's in-flight
/// buffers, or the decoder stalls waiting for one to come back.
const MAX_IMAGES: i32 = 5;

/// Poll rather than block: the caller drives this from its own frame loop.
const TIMEOUT_NOW: i64 = 0;

/// A decoded frame, still resident in GPU memory.
///
/// Deliberately just a pointer. It is owned by the [`HardwareVideoDecoder`] that
/// produced it and stays valid until that decoder has advanced `IMAGE_KEEPALIVE`
/// frames past it — long enough for the current frame's draw, and not a moment
/// longer. Do not store one.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct HardwareBuffer(*mut sys::AHardwareBuffer);

impl HardwareBuffer {
    /// The raw handle, for `VkImportAndroidHardwareBufferInfoANDROID`.
    pub fn as_ptr(self) -> *mut sys::AHardwareBuffer {
        self.0
    }
}

impl fmt::Debug for HardwareBuffer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "HardwareBuffer({:p})", self.0)
    }
}

#[derive(Debug)]
pub enum Error {
    /// An NDK call returned a non-OK status.
    Ndk {
        call: &'static str,
        status: sys::media_status_t,
    },
    /// An NDK call returned null where a handle was required.
    Null(&'static str),
    NoVideoTrack,
    NoDecoderFor(String),
    Io(std::io::Error),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Ndk { call, status } => write!(f, "{call} failed: {status:?}"),
            Error::Null(call) => write!(f, "{call} returned null"),
            Error::NoVideoTrack => write!(f, "no video track in the container"),
            Error::NoDecoderFor(mime) => write!(f, "no hardware decoder for {mime}"),
            Error::Io(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(e: std::io::Error) -> Self {
        Error::Io(e)
    }
}

type Result<T> = std::result::Result<T, Error>;

fn ok(status: sys::media_status_t, call: &'static str) -> Result<()> {
    if status == sys::media_status_t::AMEDIA_OK {
        Ok(())
    } else {
        Err(Error::Ndk { call, status })
    }
}

/// The `AImageReader` the decoder renders into, and the window it exposes.
struct Reader {
    reader: *mut sys::AImageReader,
    window: *mut sys::ANativeWindow,
}

impl Reader {
    fn new(width: i32, height: i32) -> Result<Self> {
        unsafe {
            let mut reader = std::ptr::null_mut();
            ok(
                sys::AImageReader_newWithUsage(
                    width,
                    height,
                    // See the module docs: `YUV_420_888` is accepted here and buys
                    // nothing — the buffer is vendor-tiled either way.
                    sys::AIMAGE_FORMATS::AIMAGE_FORMAT_PRIVATE.0 as i32,
                    sys::AHardwareBuffer_UsageFlags::AHARDWAREBUFFER_USAGE_GPU_SAMPLED_IMAGE.0,
                    MAX_IMAGES,
                    &mut reader,
                ),
                "AImageReader_newWithUsage",
            )?;
            if reader.is_null() {
                return Err(Error::Null("AImageReader_newWithUsage"));
            }

            let mut window = std::ptr::null_mut();
            ok(
                sys::AImageReader_getWindow(reader, &mut window),
                "AImageReader_getWindow",
            )?;
            if window.is_null() {
                return Err(Error::Null("AImageReader_getWindow"));
            }
            Ok(Self { reader, window })
        }
    }
}

/// An output buffer the codec has produced but that is not yet due.
struct PendingFrame {
    index: usize,
    pts_us: i64,
    has_data: bool,
    eos: bool,
}

/// Hardware decode of a video file into GPU-resident frames.
///
/// Pull-based, like [`super::VideoDecoder`]: the caller drives it once per rendered
/// frame and gets back the newest buffer, or `None` if nothing new has arrived.
/// Output is paced to the sample presentation clock and the file loops at EOS.
pub struct HardwareVideoDecoder {
    // Declaration order is load bearing: `Drop` runs fields in order, and the codec
    // must stop producing before the reader it produces into goes away.
    codec: *mut sys::AMediaCodec,
    extractor: *mut sys::AMediaExtractor,
    reader: Reader,
    /// Keeps the fd behind the extractor alive.
    _file: File,

    acquired: VecDeque<*mut sys::AImage>,
    current: Option<HardwareBuffer>,

    sent_eos: bool,
    pending: Option<PendingFrame>,
    playback_start: Option<Instant>,

    width: u32,
    height: u32,
}

// Same reasoning as the ffmpeg decoder: these NDK objects are not safe for
// *concurrent* use, but are fine from any single thread. Every method takes
// `&mut self`, so concurrent access is not expressible; the raw pointers simply
// carry no `Send`, which is all this asserts.
unsafe impl Send for HardwareVideoDecoder {}

impl HardwareVideoDecoder {
    /// Open a video file and start its hardware decoder.
    pub fn open(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let file = File::open(path)?;
        let length = file.metadata()?.len();

        unsafe {
            let extractor = sys::AMediaExtractor_new();
            if extractor.is_null() {
                return Err(Error::Null("AMediaExtractor_new"));
            }
            ok(
                sys::AMediaExtractor_setDataSourceFd(
                    extractor,
                    file.as_raw_fd(),
                    0,
                    length as i64,
                ),
                "AMediaExtractor_setDataSourceFd",
            )?;

            let mut selected = None;
            for i in 0..sys::AMediaExtractor_getTrackCount(extractor) {
                let format = sys::AMediaExtractor_getTrackFormat(extractor, i);
                let mut mime_ptr: *const std::os::raw::c_char = std::ptr::null();
                if sys::AMediaFormat_getString(format, sys::AMEDIAFORMAT_KEY_MIME, &mut mime_ptr) {
                    let mime = CStr::from_ptr(mime_ptr).to_string_lossy().into_owned();
                    if mime.starts_with("video/") {
                        selected = Some((i, format, mime));
                        break;
                    }
                }
                sys::AMediaFormat_delete(format);
            }
            let Some((track, format, mime)) = selected else {
                sys::AMediaExtractor_delete(extractor);
                return Err(Error::NoVideoTrack);
            };

            let (mut width, mut height) = (0, 0);
            sys::AMediaFormat_getInt32(format, sys::AMEDIAFORMAT_KEY_WIDTH, &mut width);
            sys::AMediaFormat_getInt32(format, sys::AMEDIAFORMAT_KEY_HEIGHT, &mut height);
            log::info!("hardware decode: {mime} {width}x{height} ({})", path.display());

            ok(
                sys::AMediaExtractor_selectTrack(extractor, track),
                "AMediaExtractor_selectTrack",
            )?;

            // The reader must exist before the codec, because the codec is
            // configured to render into its window.
            let reader = Reader::new(width, height)?;

            let Ok(mime_c) = std::ffi::CString::new(mime.as_str()) else {
                return Err(Error::NoDecoderFor(mime));
            };
            let codec = sys::AMediaCodec_createDecoderByType(mime_c.as_ptr());
            if codec.is_null() {
                return Err(Error::NoDecoderFor(mime));
            }
            ok(
                sys::AMediaCodec_configure(codec, format, reader.window, std::ptr::null_mut(), 0),
                "AMediaCodec_configure",
            )?;
            sys::AMediaFormat_delete(format);
            ok(sys::AMediaCodec_start(codec), "AMediaCodec_start")?;

            Ok(Self {
                codec,
                extractor,
                reader,
                _file: file,
                acquired: VecDeque::new(),
                current: None,
                sent_eos: false,
                pending: None,
                playback_start: None,
                width: width as u32,
                height: height as u32,
            })
        }
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    /// Pump the pipeline and return the newest frame's buffer.
    ///
    /// Call once per rendered frame. Returns the previous frame's buffer when
    /// nothing new is due — a video that is merely slower than the display should
    /// hold its last frame, not flicker — and `None` only before the first frame
    /// arrives.
    pub fn next_buffer(&mut self) -> Result<Option<HardwareBuffer>> {
        self.feed_input()?;
        self.drain_output()?;
        self.acquire_latest()?;
        Ok(self.current)
    }

    fn feed_input(&mut self) -> Result<()> {
        unsafe {
            while !self.sent_eos {
                let index = sys::AMediaCodec_dequeueInputBuffer(self.codec, TIMEOUT_NOW);
                if index < 0 {
                    break;
                }
                let mut capacity = 0usize;
                let buf =
                    sys::AMediaCodec_getInputBuffer(self.codec, index as usize, &mut capacity);
                if buf.is_null() {
                    return Err(Error::Null("AMediaCodec_getInputBuffer"));
                }

                let sample_size =
                    sys::AMediaExtractor_readSampleData(self.extractor, buf, capacity);
                if sample_size < 0 {
                    // End of file. Signal EOS so the codec drains what it holds,
                    // then loop once the last frame has been released.
                    ok(
                        sys::AMediaCodec_queueInputBuffer(
                            self.codec,
                            index as usize,
                            0,
                            0,
                            0,
                            sys::AMEDIACODEC_BUFFER_FLAG_END_OF_STREAM as u32,
                        ),
                        "AMediaCodec_queueInputBuffer(EOS)",
                    )?;
                    self.sent_eos = true;
                } else {
                    let pts = sys::AMediaExtractor_getSampleTime(self.extractor);
                    ok(
                        sys::AMediaCodec_queueInputBuffer(
                            self.codec,
                            index as usize,
                            0,
                            sample_size as usize,
                            pts as u64,
                            0,
                        ),
                        "AMediaCodec_queueInputBuffer",
                    )?;
                    sys::AMediaExtractor_advance(self.extractor);
                }
            }
        }
        Ok(())
    }

    fn drain_output(&mut self) -> Result<()> {
        unsafe {
            if self.pending.is_none() {
                let mut info = sys::AMediaCodecBufferInfo {
                    offset: 0,
                    size: 0,
                    presentationTimeUs: 0,
                    flags: 0,
                };
                let index =
                    sys::AMediaCodec_dequeueOutputBuffer(self.codec, &mut info, TIMEOUT_NOW);
                if index >= 0 {
                    self.pending = Some(PendingFrame {
                        index: index as usize,
                        pts_us: info.presentationTimeUs,
                        has_data: info.size > 0,
                        eos: info.flags & sys::AMEDIACODEC_BUFFER_FLAG_END_OF_STREAM as u32 != 0,
                    });
                }
                // TRY_AGAIN_LATER / FORMAT_CHANGED / BUFFERS_CHANGED: nothing to do.
            }

            let Some(frame) = &self.pending else {
                return Ok(());
            };

            // Pace to the presentation clock. Without this the decoder runs as fast
            // as the hardware allows and the clip plays at several times speed —
            // the same failure the desktop path hit, where it looked exactly like a
            // frozen texture once the clip ran off its end.
            let start = self
                .playback_start
                .get_or_insert_with(|| Instant::now() - Duration::from_micros(frame.pts_us.max(0) as u64));
            if (start.elapsed().as_micros() as i64) < frame.pts_us {
                return Ok(());
            }

            let frame = self.pending.take().expect("checked immediately above");
            ok(
                // `render = has_data` is what actually sends the frame to the
                // reader's surface; false discards it.
                sys::AMediaCodec_releaseOutputBuffer(self.codec, frame.index, frame.has_data),
                "AMediaCodec_releaseOutputBuffer",
            )?;

            if frame.eos {
                self.restart()?;
            }
        }
        Ok(())
    }

    /// Pick up whatever the reader has, keeping recent images alive behind it.
    fn acquire_latest(&mut self) -> Result<()> {
        unsafe {
            let mut image = std::ptr::null_mut();
            let status = sys::AImageReader_acquireLatestImage(self.reader.reader, &mut image);
            if status != sys::media_status_t::AMEDIA_OK || image.is_null() {
                // NO_BUFFER_AVAILABLE and friends are normal: frames arrive
                // asynchronously after releaseOutputBuffer, so most polls find
                // nothing. Keep showing the last frame.
                return Ok(());
            }

            self.acquired.push_back(image);
            while self.acquired.len() > IMAGE_KEEPALIVE {
                if let Some(old) = self.acquired.pop_front() {
                    sys::AImage_delete(old);
                }
            }

            let mut buffer = std::ptr::null_mut();
            ok(
                sys::AImage_getHardwareBuffer(image, &mut buffer),
                "AImage_getHardwareBuffer",
            )?;
            if buffer.is_null() {
                return Err(Error::Null("AImage_getHardwareBuffer"));
            }
            self.current = Some(HardwareBuffer(buffer));
        }
        Ok(())
    }

    /// Loop playback: flush the codec, rewind, restart pacing.
    fn restart(&mut self) -> Result<()> {
        unsafe {
            ok(sys::AMediaCodec_flush(self.codec), "AMediaCodec_flush")?;
            ok(
                sys::AMediaExtractor_seekTo(
                    self.extractor,
                    0,
                    sys::SeekMode::AMEDIAEXTRACTOR_SEEK_PREVIOUS_SYNC,
                ),
                "AMediaExtractor_seekTo",
            )?;
        }
        self.sent_eos = false;
        self.pending = None;
        self.playback_start = None;
        Ok(())
    }
}

impl Drop for HardwareVideoDecoder {
    fn drop(&mut self) {
        unsafe {
            // Stop the producer first, then release images, then the reader.
            sys::AMediaCodec_stop(self.codec);
            sys::AMediaCodec_delete(self.codec);
            sys::AMediaExtractor_delete(self.extractor);
            for image in self.acquired.drain(..) {
                sys::AImage_delete(image);
            }
            sys::AImageReader_delete(self.reader.reader);
        }
    }
}
