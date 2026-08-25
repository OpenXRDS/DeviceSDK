// A 2-D video playing on a surface in a 3-D scene, through the public API.
//
// Phase 0 of docs/video-asset-spike.md. There is no separate render path here and
// there does not need to be one: a video screen is an ordinary textured quad, and
// the only thing that makes it a video is that its texture changes every frame.
//
// What made this impossible until now: every texture the SDK could express was
// file-backed. `set_material_texture_slot` takes an asset id, which resolved to a
// URI and went through `AssetServer::load` — and a decoded frame exists only in
// memory. `XrdsAPI::create_video_texture` registers a texture the runtime owns, and
// `write_video_frame` fills it; from the material's point of view nothing has
// changed.
//
// Decoding is deliberately not the runtime's job. `xrds-media` decodes here because
// this is a desktop example; on a Quest the frames come from MediaCodec instead, and
// the runtime never learns the difference.
//
//     cargo run --example video_texture_check                    # the bundled clip
//     cargo run --example video_texture_check -- <path-to-video>  # your own
//
// With no argument it plays `crates/xrds-net/samples/sample_video_only.mp4`, which
// opens on a bright green MPAA card. That default is not a convenience — it is the
// point. An earlier pass ran against a clip averaging 38/255, where "playing
// correctly" and "not updating at all" were the same grey rectangle on screen, and
// several rounds of debugging went into a picture that had been right the whole
// time. A verification clip has to be unmistakable at a glance.
//
// What to look for:
//
//   1. A green card with white text, within a second or two. That is the video.
//   2. Right way up, and not sheared. Shear means a stride bug — a decoded row is
//      padded to the scaler's stride, not `width * 4`.
//   3. The console figures: decode and upload are reported separately, because only
//      one of them survives the move to hardware decode.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{sync_channel, Receiver, TrySendError};
use std::sync::{Arc, Mutex};
use std::time::Instant;

use xrds::sdk::{
    primitives::XrdsPlane3D,
    world::{lights::XrdsDirectionalLight, XrdsCamera},
    XrdsMaterialParams, XrdsMaterialTextureRef, XrdsMaterialTextureSamplerParams,
    XrdsMaterialTextureSlotKind, XrdsMaterialTextureUvParams,
};
use xrds::{Runtime, RuntimeParameters, XrdsAPI, XrdsApp, XrdsUpdateContext};

use bevy::ecs::system::{Commands, Local};
use bevy::render::view::screenshot::{save_to_disk, Screenshot};
use bevy::time::Time;
use bevy::prelude::Res;
use xrds_media::video::{VideoDecoder, VideoFrame};


/// One frame in flight behind the one being shown.
///
/// Deliberately tiny. A deeper queue would let the decoder run ahead and hide its
/// true cost behind buffering, which is the opposite of what a measurement wants —
/// and for playback, a late frame should be *dropped*, not queued, or the video
/// drifts further behind the longer it runs.
const QUEUE_DEPTH: usize = 1;

/// The id the material slot names, and the registry key for the texture.
const VIDEO_ID: &str = "clip";

/// Played when no path is given. Already in the repository for `xrds-net`'s
/// transport tests, so this adds no asset — and it opens on a green MPAA card,
/// which is the loudest "the video is on screen" signal available for free.
const DEFAULT_CLIP: &str = "crates/xrds-net/samples/sample_video_only.mp4";

struct VideoApp {
    /// `Mutex` only to satisfy `Sync`, which `run_xrds` requires — one thread
    /// ever touches it.
    frames: Mutex<Receiver<VideoFrame>>,
    size: (u32, u32),
    bound: bool,
    shown: u32,
    upload_total: f64,
    started: Option<Instant>,
    /// Released once the screen exists, so the decoder does not play the opening of
    /// the clip to nobody while the window is still being created.
    ready: Arc<AtomicBool>,
}

impl XrdsApp for VideoApp {
    fn setup(&mut self, api: &mut XrdsAPI<'_>) {
        // `looking_at` sets orientation, not placement — without moving it back the
        // camera sits at the origin, inside the quad.
        let camera = api.spawn(&XrdsCamera::new().with_name("VideoCamera").looking_at([0.0, 0.0, 0.0]));
        api.set_translation(&camera, [0.0, 0.0, 3.0]);

        api.spawn(&XrdsDirectionalLight::new().with_name("Key"));

        // The screen. A plane stood upright and sized to the *source's* aspect, so a
        // stretched picture means a bug rather than a mismatched quad — 2.40:1 and
        // 16:9 clips otherwise look equally wrong on a fixed-shape screen.
        let (vw, vh) = self.size;
        let height = 3.2 * vh as f32 / vw as f32;
        let mut screen = XrdsPlane3D::new().with_name("Screen");
        screen.size = [3.2, height];
        screen.transform.rotation_quat_xyzw = [
            std::f32::consts::FRAC_PI_4.sin(),
            0.0,
            0.0,
            std::f32::consts::FRAC_PI_4.cos(),
        ];
        let screen = api.spawn(&screen);

        // Unlit. A screen showing a picture emits that picture; it does not reflect
        // a key light. Left lit, the video is modulated by scene lighting — the
        // bright green MPAA card renders as dark olive — which both looks wrong and
        // makes a working screen easy to mistake for a broken one.
        if let Some(params) = api.material_params(&screen) {
            api.set_material_params(
                &screen,
                XrdsMaterialParams {
                    unlit: true,
                    ..params
                },
            );
        }

        api.create_video_texture(VIDEO_ID, self.size.0, self.size.1);
        api.set_material_texture_slot(
            &screen,
            XrdsMaterialTextureSlotKind::BaseColor,
            // Constructed in full: `XrdsMaterialTextureRef` has no `Default`, and
            // rightly so — a texture reference with an empty asset id names nothing.
            Some(XrdsMaterialTextureRef {
                texture_asset_id: VIDEO_ID.to_string(),
                uv: XrdsMaterialTextureUvParams::default(),
                sampler: XrdsMaterialTextureSamplerParams::default(),
            }),
        );
        // Read back rather than assume: `set_material_texture_slot` silently does
        // nothing when the entity's material does not exist yet, and `spawn` may be
        // deferred.
        //
        // Note what this does and does not prove. It reads the *authored* slot
        // record, so a `yes` means the scene says the right thing — not that a GPU
        // texture is bound. Treating it as the stronger claim is what sent an
        // earlier debugging pass looking downstream of a problem that was not there.
        let bound = api
            .material_textures(&screen)
            .and_then(|slots| slots.get(XrdsMaterialTextureSlotKind::BaseColor).cloned())
            .map(|r| r.texture_asset_id == VIDEO_ID)
            .unwrap_or(false);
        self.bound = bound;
        println!(
            "[video] base-colour slot bound to '{VIDEO_ID}': {}",
            if bound { "yes" } else { "NO — the material did not exist yet" }
        );

        // A test pattern, written once before any real frame arrives. It makes the
        // three failure modes tell themselves apart on screen:
        //
        //   checkerboard  -> binding, UVs and writes all work; the problem is frames
        //   flat grey     -> bound, but writes are not landing
        //   anything else -> the binding did not take
        //
        // Cheaper than another round of guessing, which is what the last two
        // debugging passes cost.
        let (w, h) = self.size;
        let mut pattern = vec![0u8; (w as usize) * (h as usize) * 4];
        for y in 0..h as usize {
            for x in 0..w as usize {
                let on = ((x / 128) + (y / 128)) % 2 == 0;
                let px = &mut pattern[(y * w as usize + x) * 4..][..4];
                px.copy_from_slice(if on { &[220, 30, 200, 255] } else { &[30, 200, 90, 255] });
            }
        }
        let ok = api.write_video_frame(VIDEO_ID, &pattern);
        println!("[video] test-pattern write: {}", if ok { "accepted" } else { "REJECTED" });

        // XRDS_VIDEO_SCREENSHOT=<dir> captures the window a few seconds apart and
        // writes PNGs. It exists because "does the picture change" is the one
        // question this example cannot answer from inside itself, and answering it
        // by asking a person to look at a screen is how a texture that had been
        // frozen since frame one survived three rounds of debugging.
        if std::env::var("XRDS_VIDEO_SCREENSHOT").is_ok() {
            api.add_update_system(capture_screenshots);
        }

        // Last, once there is a bound surface to receive frames.
        self.ready.store(true, Ordering::Release);
    }

    fn update(&mut self, ctx: &mut XrdsUpdateContext<'_>) {
        if !self.bound {
            return;
        }
        // XRDS_VIDEO_PATTERN_ONLY=1 stops the frame pump, leaving the checkerboard
        // written at setup on screen. It splits "the texture never reaches the
        // material" from "the frames are the problem" — the two remaining
        // explanations for a surface that stays grey while every call reports
        // success.
        if std::env::var("XRDS_VIDEO_PATTERN_ONLY").is_ok() {
            return;
        }

        // Newest frame only; anything behind it is dropped. Playback that queues
        // rather than drops falls further behind every frame.
        let mut newest = None;
        {
            let rx = self.frames.lock().unwrap();
            while let Ok(frame) = rx.try_recv() {
                newest = Some(frame);
            }
        }
        let Some(frame) = newest else { return };

        let started = *self.started.get_or_insert_with(Instant::now);

        let upload_start = Instant::now();
        let ok = ctx.write_video_frame(VIDEO_ID, &frame.rgba);
        if !ok && self.shown == 0 {
            println!(
                "[video] write REJECTED — {} bytes for {}x{}",
                frame.rgba.len(),
                self.size.0,
                self.size.1
            );
        }
        self.upload_total += upload_start.elapsed().as_secs_f64();
        self.shown += 1;

        if self.shown % 30 == 0 {
            let wall = started.elapsed().as_secs_f64();
            println!(
                "[video] {} frames shown in {:.1}s ({:.1} fps effective) — upload {:.2} ms/frame, {:.2} GB/s",
                self.shown,
                wall,
                self.shown as f64 / wall,
                self.upload_total / self.shown as f64 * 1000.0,
                (self.size.0 * self.size.1 * 4) as f64 * (self.shown as f64 / wall) / 1e9,
            );
        }
    }
}

/// Capture the window at three spaced moments, then stop.
///
/// Spaced seconds apart rather than on consecutive frames: adjacent video frames are
/// nearly identical, so shots taken back to back would look the same whether or not
/// the texture is updating — which is the exact confusion being ruled out.
fn capture_screenshots(mut commands: Commands, time: Res<Time>, mut taken: Local<usize>) {
    const AT_SECONDS: [f32; 3] = [2.0, 5.0, 8.0];
    if *taken >= AT_SECONDS.len() || time.elapsed_secs() < AT_SECONDS[*taken] {
        return;
    }
    let dir = std::env::var("XRDS_VIDEO_SCREENSHOT").unwrap_or_else(|_| ".".to_string());
    let path = format!("{dir}/shot_{}.png", *taken);
    println!("[video] capturing {path}");
    commands
        .spawn(Screenshot::primary_window())
        .observe(save_to_disk(path));
    *taken += 1;
}

fn main() {
    // Bundled by default. See the header for why the default clip is a loud one.
    let path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| DEFAULT_CLIP.to_string());

    let mut decoder = VideoDecoder::open(&path).expect("could not open the video");
    let (w, h, fps) = (decoder.width(), decoder.height(), decoder.frame_rate());
    println!("[video] source {w}x{h} @ {fps:.1} fps");

    let (tx, rx) = sync_channel::<VideoFrame>(QUEUE_DEPTH);
    let ready = Arc::new(AtomicBool::new(false));

    // Decode off the render thread. It is slower than real time at 4K even on a
    // desktop, and blocking `update()` on it would measure the decoder rather than
    // the upload — which is the half that matters here.
    let decode_ready = Arc::clone(&ready);
    let clip = path.clone();
    std::thread::spawn(move || {
        // Do not start until there is something to draw on.
        //
        // Opening a window takes a few seconds, and an unpaced decoder gets a long
        // way into the clip in that time — 900 frames, measured. Every one of them
        // is dropped, so playback appears to begin near the end of a short clip, or
        // past it. That is not a rendering bug but it looks exactly like one.
        while !decode_ready.load(Ordering::Acquire) {
            std::thread::sleep(std::time::Duration::from_millis(16));
        }

        let mut decoded = 0u32;
        let mut epoch = Instant::now();
        let measure_start = Instant::now();
        loop {
            match decoder.next_frame() {
                Ok(Some(frame)) => {
                    decoded += 1;

                    // Pace to the presentation clock. Without this the decoder is
                    // the pacer, and it is far faster than real time — 270 fps for a
                    // 24 fps clip here, playing it at eleven times speed and running
                    // off the end within seconds. What remains on screen is the last
                    // frame, frozen: a still picture that every diagnostic upstream
                    // reports as a healthy, updating texture.
                    let due = std::time::Duration::from_secs_f64(frame.pts_secs.max(0.0));
                    if let Some(wait) = due.checked_sub(epoch.elapsed()) {
                        std::thread::sleep(wait);
                    }

                    if decoded % 120 == 0 {
                        println!(
                            "[video] decoded {decoded} frames, at {:.2}s of the clip",
                            frame.pts_secs
                        );
                    }
                    // Drop rather than block: the renderer only ever wants the
                    // newest frame, so waiting for it to catch up would make the
                    // decoder the pacer again by another route.
                    match tx.try_send(frame) {
                        Ok(()) | Err(TrySendError::Full(_)) => {}
                        Err(TrySendError::Disconnected(_)) => return,
                    }
                }
                Ok(None) => {
                    // Loop rather than stop. A check that ends with a frozen final
                    // frame is indistinguishable from a check that never updated,
                    // which is the confusion this whole example exists to avoid.
                    println!(
                        "[video] end of stream after {decoded} frames in {:.1}s — looping",
                        measure_start.elapsed().as_secs_f64()
                    );
                    match VideoDecoder::open(&clip) {
                        Ok(fresh) => {
                            decoder = fresh;
                            decoded = 0;
                            epoch = Instant::now();
                        }
                        Err(e) => {
                            eprintln!("[video] could not reopen for looping: {e}");
                            return;
                        }
                    }
                }
                Err(e) => {
                    eprintln!("[video] decode error: {e}");
                    return;
                }
            }
        }
    });

    let _ = Runtime::new(RuntimeParameters::default()).run_xrds(VideoApp {
        frames: Mutex::new(rx),
        size: (w, h),
        bound: false,
        shown: 0,
        upload_total: 0.0,
        started: None,
        ready,
    });
}
