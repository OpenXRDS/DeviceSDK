//! Runtime-owned textures, for video and anything else generated per frame.
//!
//! # The gap this closes
//!
//! Every texture the SDK could express was file-backed:
//! `resolved_texture_handle_for_material_slot` resolved an asset id to a URI and
//! called `AssetServer::load`. There was no branch where a texture is something the
//! runtime *owns* rather than something it loads — so a video frame, which exists
//! only in memory, could not reach a material by any public route. The material type
//! itself (`XrdsRuntimeMaterial`) is `pub(super)`, so there was no expert-layer
//! escape either.
//!
//! A texture registered here resolves ahead of the asset catalog, so an author binds
//! it exactly like a file texture:
//!
//! ```ignore
//! api.create_video_texture("lobby", 1920, 1080);
//! api.set_material_texture_slot(&screen, BaseColor, Some(XrdsMaterialTextureRef {
//!     texture_asset_id: "lobby".into(),
//!     ..Default::default()
//! }));
//! // then, per frame, from wherever the frames come from:
//! api.write_video_frame("lobby", &rgba);
//! ```
//!
//! "This surface shows this video" is the same sentence as "this surface shows this
//! texture", which is what the texture-slot API already says. Nothing downstream —
//! material, mesh, shader, renderer — knows the difference.
//!
//! # Why the runtime does not decode
//!
//! Frames are *pushed in*, and this module has no opinion about where they came
//! from. Decoding lives in `xrds-media`, which is desktop-only and carries ffmpeg;
//! a runtime that depended on it could not ship to a headset, where frames come from
//! `MediaCodec` instead. Same separation as `xrds-net` and `xrds-media`: one
//! produces frames, the other consumes them, and neither depends on the other's
//! platform.

use super::*;
use bevy::asset::RenderAssetUsages;
use bevy::image::Image;
use bevy::render::render_resource::{Extent3d, TextureDimension, TextureFormat};
use std::collections::HashMap;

/// Textures the runtime writes into, keyed by the id a material slot names.
#[derive(Resource, Default)]
pub(super) struct XrdsVideoTextures {
    entries: HashMap<String, VideoTexture>,
}

struct VideoTexture {
    handle: bevy::asset::Handle<Image>,
    width: u32,
    height: u32,
}

impl XrdsVideoTextures {
    fn handle_of(&self, id: &str) -> Option<bevy::asset::Handle<Image>> {
        self.entries.get(id).map(|e| e.handle.clone())
    }
}

/// A texture the runtime owns and the caller fills, resolvable by `id` from any
/// material texture slot.
///
/// Idempotent for an unchanged size: re-registering the same id at the same
/// dimensions keeps the existing handle, so a caller that cannot easily tell
/// whether it has already registered does not silently orphan the texture a
/// material is already pointing at. A *different* size replaces it, because the
/// buffer length would no longer match.
pub(super) fn create_video_texture_in_world(
    world: &mut World,
    id: impl Into<String>,
    width: u32,
    height: u32,
) {
    let id = id.into();

    if let Some(existing) = world.resource::<XrdsVideoTextures>().entries.get(&id) {
        if existing.width == width && existing.height == height {
            return;
        }
    }

    // Opaque mid-grey rather than transparent black: an unfilled video surface
    // should read as "nothing here yet" and not as a hole in the scene. Black is
    // also what a decoded fade-in looks like, and the two should not be confusable.
    let mut initial = vec![0u8; (width as usize) * (height as usize) * 4];
    for px in initial.chunks_exact_mut(4) {
        px.copy_from_slice(&[40, 40, 40, 255]);
    }

    let image = Image::new(
        Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        TextureDimension::D2,
        initial,
        // Srgb: decoders emit display-referred colour, and calling it linear washes
        // the whole picture out.
        TextureFormat::Rgba8UnormSrgb,
        // MAIN_WORLD as well as RENDER_WORLD. Without it Bevy drops the CPU copy
        // after the first upload and every later write goes nowhere — silently,
        // leaving the first frame frozen on screen.
        RenderAssetUsages::RENDER_WORLD | RenderAssetUsages::MAIN_WORLD,
    );

    let handle = world.resource_mut::<Assets<Image>>().add(image);
    world.resource_mut::<XrdsVideoTextures>().entries.insert(
        id,
        VideoTexture {
            handle,
            width,
            height,
        },
    );
}

/// Replace a registered texture's pixels. `rgba` must be tightly packed RGBA8.
///
/// Returns false if the id is unknown or the buffer is the wrong length — a wrong
/// length is a caller bug that would otherwise render as a diagonally sheared image,
/// which is a hard thing to trace back to its cause.
pub(super) fn write_video_frame_in_world(world: &mut World, id: &str, rgba: &[u8]) -> bool {
    let Some(entry) = world.resource::<XrdsVideoTextures>().entries.get(id) else {
        return false;
    };
    let expected = (entry.width as usize) * (entry.height as usize) * 4;
    if rgba.len() != expected {
        warn!(
            "video frame for '{id}' is {} bytes, expected {expected} \
             ({}x{} RGBA) — ignoring",
            rgba.len(),
            entry.width,
            entry.height
        );
        return false;
    }
    let handle = entry.handle.clone();

    {
        let mut images = world.resource_mut::<Assets<Image>>();
        let Some(image) = images.get_mut(&handle) else {
            return false;
        };
        let Some(data) = image.data.as_mut() else {
            return false;
        };
        data.copy_from_slice(rgba);
    }

    rebind_materials_sampling(world, &handle);
    true
}

/// Re-prepare every material that samples `handle`, so its bind group picks up the
/// texture that was just uploaded.
///
/// # Why this is necessary, and why nothing warns you that it is
///
/// Writing to the image is only half of an update. Bevy re-uploads a modified image
/// by building an **entirely new** `wgpu::Texture` —
/// `GpuImage::prepare_asset` calls `create_texture_with_data` rather than writing
/// into the existing one — and `bevy_pbr` contains no `AssetEvent<Image>` handling
/// at all. A material's bind group is allocated once, capturing the `TextureView` as
/// it stood at that moment, and nothing invalidates it when the image behind it is
/// replaced.
///
/// So the surface keeps sampling the *first* texture forever. Every step reports
/// success — the write lands, `AssetEvent::Modified` fires, the render asset
/// re-prepares, the upload is measurable — and the picture never changes. It cost
/// this spike several rounds of debugging, because the failure is invisible from the
/// main world: the only wrong thing is which texture a bind group in the render
/// world points at.
///
/// Touching the material emits `AssetEvent::Modified` for it in turn, which makes
/// its `PreparedMaterial` re-prepare and re-allocate the bind group against the
/// current `RenderAssets<GpuImage>`.
///
/// # This is a known Bevy gap, and it is not close to being fixed
///
/// Not our discovery and not our invention — the same workaround (`get_mut` the
/// material after modifying the image) is what everyone hitting this arrives at, and
/// it has been reported repeatedly since Bevy 0.6:
///
/// - <https://github.com/bevyengine/bevy/issues/3674> (0.6, 2D)
/// - <https://github.com/bevyengine/bevy/issues/17350> (0.15 regression)
/// - <https://github.com/bevyengine/bevy/pull/20575> — an attempt at the real fix,
///   marking an asset modified when a *dependency* is modified. **Closed**, blocked
///   on cost (it scanned every asset) and on granularity: not every asset should
///   rebuild when a dependency changes, and there was no opt-in. The design
///   discussion moved toward many-to-many asset relationships
///   (<https://github.com/bevyengine/bevy/issues/11266>), which is architectural
///   work with no timeline.
///
/// So treat this as **long-lived**, not as a stopgap awaiting an upstream release.
/// If Bevy ever does mark dependents modified, delete this — until then it is load
/// bearing, and the cost note below is the part that matters.
///
/// The scan is over materials, not entities, and runs once per written frame. Scenes
/// hold materials in the tens to low hundreds, so this is a few microseconds against
/// a frame that also uploads megabytes; caching the dependents would have to be
/// invalidated whenever a slot is rebound, which is more ways to be subtly wrong than
/// the scan is worth.
pub(super) fn rebind_materials_sampling(world: &mut World, handle: &bevy::asset::Handle<Image>) {
    let id = handle.id();

    let Some(materials) = world.get_resource::<Assets<XrdsRuntimeMaterial>>() else {
        return;
    };
    let dependents: Vec<_> = materials
        .iter()
        .filter(|(_, material)| {
            let ext = &material.extension;
            // Every slot, not just base colour: a video is the obvious case, but
            // this is the general "runtime-owned texture" path, and an emissive or
            // occlusion slot fed per frame would otherwise freeze in exactly the
            // same way and be far harder to recognise.
            [
                ext.base_color_texture.as_ref(),
                ext.metallic_roughness_texture.as_ref(),
                ext.normal_texture.as_ref(),
                ext.occlusion_texture.as_ref(),
                ext.emissive_texture.as_ref(),
                material.base.base_color_texture.as_ref(),
            ]
            .into_iter()
            .flatten()
            .any(|h| h.id() == id)
        })
        .map(|(asset_id, _)| asset_id)
        .collect();

    if !dependents.is_empty() {
        let mut materials = world.resource_mut::<Assets<XrdsRuntimeMaterial>>();
        for asset_id in dependents {
            // The mutable borrow is the entire point — it is what marks the material
            // modified. There is nothing to change on it.
            let _ = materials.get_mut(asset_id);
        }
    }

    rebind_standard_materials_sampling(world, id);
}

/// The same, for Bevy's own `StandardMaterial`.
///
/// XRDS meshes carry `XrdsRuntimeMaterial`, so the scan above covers everything the
/// default layer can author. But glTF-imported meshes keep the loader's
/// `StandardMaterial`, and the expert layer can bind a runtime texture to one
/// directly — and those would freeze on frame one in exactly the same way, with the
/// added difficulty that nothing in the XRDS material path is involved to look at.
///
/// Cheap to cover and expensive to omit, so it is covered.
fn rebind_standard_materials_sampling(world: &mut World, id: bevy::asset::AssetId<Image>) {
    let Some(materials) = world.get_resource::<Assets<StandardMaterial>>() else {
        return;
    };
    let dependents: Vec<_> = materials
        .iter()
        .filter(|(_, m)| {
            [
                m.base_color_texture.as_ref(),
                m.emissive_texture.as_ref(),
                m.metallic_roughness_texture.as_ref(),
                m.normal_map_texture.as_ref(),
                m.occlusion_texture.as_ref(),
            ]
            .into_iter()
            .flatten()
            .any(|h| h.id() == id)
        })
        .map(|(asset_id, _)| asset_id)
        .collect();

    if dependents.is_empty() {
        return;
    }

    let mut materials = world.resource_mut::<Assets<StandardMaterial>>();
    for asset_id in dependents {
        let _ = materials.get_mut(asset_id);
    }
}

/// Register a texture whose *contents* come from outside Bevy entirely.
///
/// Used by the Android hardware path, where frames are decoded and converted on the
/// GPU and never exist as bytes the main world could write. The placeholder created
/// here gives Bevy a real `GpuImage` to start from — so an unfilled video surface
/// reads as "nothing yet" rather than as a missing texture — and the render world
/// replaces that entry once frames arrive.
pub(super) fn external_video_texture_id_in_world(
    world: &mut World,
    id: &str,
) -> Option<bevy::asset::AssetId<Image>> {
    world
        .resource::<XrdsVideoTextures>()
        .entries
        .get(id)
        .map(|entry| entry.handle.id())
}

pub(super) fn remove_video_texture_in_world(world: &mut World, id: &str) {
    world.resource_mut::<XrdsVideoTextures>().entries.remove(id);
}

/// The handle for `id`, if the runtime owns a texture by that name.
///
/// Consulted by `resolved_texture_handle_for_material_slot` *before* the asset
/// catalog, so a runtime texture shadows a file of the same id rather than the
/// other way round. That ordering matters: a video that stopped resolving because
/// someone imported a still image with a clashing id would be a baffling failure.
pub(super) fn video_texture_handle_in_world(
    world: &World,
    id: &str,
) -> Option<bevy::asset::Handle<Image>> {
    world.get_resource::<XrdsVideoTextures>()?.handle_of(id)
}
