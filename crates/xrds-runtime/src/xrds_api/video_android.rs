//! Hardware video on Android, from file to material.
//!
//! The Android counterpart to [`super::video`]. That module takes RGBA bytes from
//! the main world; this one never has bytes at all — the frame is decoded by
//! MediaCodec into an `AHardwareBuffer`, converted by Vulkan, and handed to Bevy as
//! an already-resident GPU texture. See `docs/video-asset-spike.md`.
//!
//! # Why the work happens in the render world
//!
//! Two reasons, and the second is the one that bites:
//!
//! 1. The conversion needs the renderer's own Vulkan device, and produces a
//!    `wgpu::Texture` that only the render world can install.
//! 2. It submits to the Vulkan queue wgpu uses. Doing that from the main world races
//!    wgpu's own submissions, which Vulkan requires to be externally synchronised.
//!    Running in the render schedule puts it on the render thread at a point wgpu is
//!    not submitting.
//!
//! # How the texture reaches a material
//!
//! `RenderAssets<GpuImage>` is keyed by `AssetId<Image>`, so a placeholder `Image`
//! asset is registered in the main world — which is what a material's texture slot
//! binds to — and its `GpuImage` entry is then replaced here with one wrapping the
//! converted texture. Nothing downstream knows the difference.

use super::*;
use bevy::asset::AssetId;
use bevy::image::Image;
use bevy::render::extract_resource::ExtractResource;
use bevy::render::render_asset::RenderAssets;
use bevy::render::render_resource::{
    AddressMode, Extent3d, FilterMode, SamplerDescriptor, TextureFormat, TextureViewDescriptor,
};
use bevy::render::renderer::RenderDevice;
use bevy::render::texture::GpuImage;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

use xrds_media::video::HardwareVideoDecoder;
use xrds_openxr::video::AndroidVideoPipeline;

/// What the app has asked to play, mirrored into the render world each frame.
#[derive(Resource, Default, Clone, ExtractResource)]
pub(super) struct HardwareVideoRequests {
    pub(super) entries: Vec<HardwareVideoRequest>,
}

#[derive(Clone)]
pub(super) struct HardwareVideoRequest {
    /// The id a material's texture slot names.
    pub(super) id: String,
    pub(super) path: PathBuf,
    pub(super) asset_id: AssetId<Image>,
    pub(super) looping: bool,
}

/// Decoders and conversion pipelines, one per playing video.
///
/// Behind a `Mutex` because Bevy resources must be `Sync` and these own raw NDK and
/// Vulkan handles that are `Send` but not `Sync`. Only one system touches it, so the
/// lock is never contended — it is here to satisfy the bound honestly rather than to
/// coordinate anything.
#[derive(Resource, Default)]
pub(super) struct HardwareVideoPlayers(Mutex<HashMap<String, Player>>);

struct Player {
    decoder: HardwareVideoDecoder,
    pipeline: AndroidVideoPipeline,
    /// Decode + convert cost, reported periodically.
    ///
    /// Kept rather than removed after S6: this is the number that decides whether a
    /// video panel fits a frame budget, and it is not derivable from the frame rate
    /// once anything else is happening in the scene.
    frames: u32,
    convert_total: f64,
    /// Built once and reused: a fresh `TextureView` every frame would be a fresh
    /// object in every material bind group that samples it, for no benefit.
    gpu_image: Option<GpuImage>,
    failed: bool,
}

/// Start playing `path` into the texture named `id`.
///
/// Returns false if the clip cannot be opened; the caller gets a plain answer rather
/// than a surface that silently stays blank.
pub(super) fn play_hardware_video_in_world(
    world: &mut World,
    id: impl Into<String>,
    path: impl Into<PathBuf>,
    looping: bool,
) -> bool {
    let id = id.into();
    let path = path.into();

    // A header read, not a decoder. Opening `HardwareVideoDecoder` here would
    // configure and start a second hardware codec session purely to ask how big the
    // frames are, and the render world opens its own anyway.
    let (width, height) = match xrds_media::video::probe_video_size(&path) {
        Ok(size) => size,
        Err(e) => {
            warn!("hardware video '{id}': cannot read {}: {e}", path.display());
            return false;
        }
    };

    super::video::create_video_texture_in_world(world, id.clone(), width, height);
    let Some(asset_id) = super::video::external_video_texture_id_in_world(world, &id) else {
        warn!("hardware video '{id}': texture registration failed");
        return false;
    };

    let mut requests = world.resource_mut::<HardwareVideoRequests>();
    requests.entries.retain(|entry| entry.id != id);
    requests.entries.push(HardwareVideoRequest {
        id,
        path,
        asset_id,
        looping,
    });
    true
}

/// Whether `id` currently has a decoder running.
pub(super) fn is_playing_in_world(world: &World, id: &str) -> bool {
    world
        .get_resource::<HardwareVideoRequests>()
        .is_some_and(|requests| requests.entries.iter().any(|entry| entry.id == id))
}

/// Whether `id` is playing and already set to `looping`.
pub(super) fn is_playing_as_in_world(world: &World, id: &str, looping: bool) -> bool {
    world
        .get_resource::<HardwareVideoRequests>()
        .is_some_and(|requests| {
            requests
                .entries
                .iter()
                .any(|entry| entry.id == id && entry.looping == looping)
        })
}

/// Stop a hardware video, dropping its decoder and conversion pipeline.
pub(super) fn stop_hardware_video_in_world(world: &mut World, id: &str) {
    world
        .resource_mut::<HardwareVideoRequests>()
        .entries
        .retain(|entry| entry.id != id);
}

/// Keep every material sampling a hardware video marked modified.
///
/// The same Bevy gap the desktop path works around, for the same reason: a
/// material's bind group is built once and nothing invalidates it when the image
/// behind it changes. Here the texture object is stable, so only the *first* correct
/// build matters — but that build races the frame on which the render world first
/// installs the real texture, and losing that race means a surface frozen on the
/// placeholder. Touching every frame is cheaper than reasoning about the ordering.
pub(super) fn rebind_hardware_video_materials(world: &mut World) {
    let ids: Vec<String> = {
        let requests = world.resource::<HardwareVideoRequests>();
        if requests.entries.is_empty() {
            return;
        }
        requests.entries.iter().map(|e| e.id.clone()).collect()
    };
    for id in ids {
        if let Some(handle) = super::video::video_texture_handle_in_world(world, &id) {
            super::video::rebind_materials_sampling(world, &handle);
        }
    }
}

/// Decode, convert, and install one frame per video, per rendered frame.
pub(super) fn update_hardware_video(
    requests: Res<HardwareVideoRequests>,
    players: Res<HardwareVideoPlayers>,
    render_device: Res<RenderDevice>,
    mut images: ResMut<RenderAssets<GpuImage>>,
) {
    if requests.entries.is_empty() {
        return;
    }
    let Ok(mut players) = players.0.lock() else {
        return;
    };

    for request in &requests.entries {
        if !players.contains_key(&request.id) {
            let decoder = match HardwareVideoDecoder::open(&request.path, request.looping) {
                Ok(decoder) => decoder,
                Err(e) => {
                    error!("hardware video '{}': decoder: {e}", request.id);
                    continue;
                }
            };
            let pipeline = match AndroidVideoPipeline::new(render_device.wgpu_device()) {
                Ok(pipeline) => pipeline,
                Err(e) => {
                    error!("hardware video '{}': pipeline: {e:#}", request.id);
                    continue;
                }
            };
            info!(
                "hardware video '{}': {}x{} from {}",
                request.id,
                decoder.width(),
                decoder.height(),
                request.path.display()
            );
            players.insert(
                request.id.clone(),
                Player {
                    decoder,
                    pipeline,
                    frames: 0,
                    convert_total: 0.0,
                    gpu_image: None,
                    failed: false,
                },
            );
        }

        let Some(player) = players.get_mut(&request.id) else {
            continue;
        };
        if player.failed {
            continue;
        }

        let (width, height) = (player.decoder.width(), player.decoder.height());
        let buffer = match player.decoder.next_buffer() {
            Ok(Some(buffer)) => buffer,
            Ok(None) => {
                // Nothing new yet; whatever was installed last frame still stands.
                if let Some(gpu_image) = &player.gpu_image {
                    images.insert(request.asset_id, gpu_image.clone());
                }
                continue;
            }
            Err(e) => {
                error!("hardware video '{}': decode: {e}", request.id);
                player.failed = true;
                continue;
            }
        };

        let started = std::time::Instant::now();
        let texture = match player
            .pipeline
            .frame_to_texture(buffer.as_ptr().cast(), width, height)
        {
            Ok(texture) => {
                player.frames += 1;
                player.convert_total += started.elapsed().as_secs_f64();
                if player.frames % 300 == 0 {
                    log::info!(
                        "hardware video '{}': {:.2} ms/frame over {} frames",
                        request.id,
                        player.convert_total / player.frames as f64 * 1000.0,
                        player.frames,
                    );
                }
                texture.clone()
            }
            Err(e) => {
                error!("hardware video '{}': convert: {e:#}", request.id);
                player.failed = true;
                continue;
            }
        };

        if player.gpu_image.is_none() {
            let view = texture.create_view(&TextureViewDescriptor::default());
            let sampler = render_device.create_sampler(&SamplerDescriptor {
                label: Some("XrdsHardwareVideo"),
                mag_filter: FilterMode::Linear,
                min_filter: FilterMode::Linear,
                // Clamp: a video frame has edges, and wrapping smears the opposite
                // side of the picture into view.
                address_mode_u: AddressMode::ClampToEdge,
                address_mode_v: AddressMode::ClampToEdge,
                address_mode_w: AddressMode::ClampToEdge,
                ..Default::default()
            });
            player.gpu_image = Some(GpuImage {
                texture: texture.into(),
                texture_view: view.into(),
                texture_format: TextureFormat::Rgba8UnormSrgb,
                sampler,
                size: Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
            });
            info!("hardware video '{}': texture installed", request.id);
        }

        if let Some(gpu_image) = &player.gpu_image {
            images.insert(request.asset_id, gpu_image.clone());
        }
    }
}
