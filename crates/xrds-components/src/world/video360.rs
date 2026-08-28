//! A 360 video surface: the viewer stands inside it and the video is the world.

use crate::{TransformParams, XrdsComponent, XrdsMutableComponent, XrdsObject};

/// An inside-facing sphere carrying an equirectangular 360 video.
///
/// A node of its own rather than "a sphere with the right boxes ticked", because
/// the arrangement that makes 360 video work — inward-facing geometry,
/// equirectangular UVs with the seam behind the viewer, unlit shading — is four
/// things an author can silently get wrong, and none of them is an XRDS concept.
/// See `docs/video-360-plan.md`.
///
/// **The viewer does not move relative to it.** A 360 scene puts the viewer at the
/// centre and keeps them there, so this is a static node: no camera parenting, no
/// anchor. `radius` only has to clear roomscale drift and stay inside the camera's
/// far plane.
#[derive(Debug, Clone)]
pub struct XrdsVideo360 {
    pub name: String,
    pub transform: TransformParams,
    pub visible: bool,
    /// Catalog asset id referencing an `XrdsSceneAssetKind::Video` asset.
    pub video_asset_id: String,
    /// Metres. Large enough that a standing viewer never approaches the surface,
    /// small enough to stay inside the camera's far plane.
    pub radius: f32,
    /// `1.0` is fully opaque, which is what a VR 360 scene wants.
    ///
    /// **Below 1.0 is what makes this work over passthrough**, since the app
    /// layer's alpha is what reveals the room beneath. It is authorable rather
    /// than fixed because the usable value is something you tune while wearing the
    /// headset — the reference implementation this was ported from ships 0.8 and
    /// made it overridable on the device for exactly that reason.
    pub opacity: f32,
    /// Whether playback starts as soon as the scene loads.
    ///
    /// Defaults **off**, like every other video in this SDK: a decoder is the
    /// expensive part and starting one unasked is the wrong default. A 360 video
    /// is the one case where an author usually does want it on, which is a reason
    /// to make it easy to tick, not a reason to change the default.
    pub autoplay: bool,
}

impl Default for XrdsVideo360 {
    fn default() -> Self {
        Self {
            name: "Video360".to_string(),
            transform: TransformParams::default(),
            visible: true,
            video_asset_id: String::new(),
            // 50 m clears any roomscale space by a wide margin and sits well
            // inside a default far plane.
            radius: 50.0,
            opacity: 1.0,
            autoplay: false,
        }
    }
}

impl XrdsObject for XrdsVideo360 {
    fn name(&self) -> &str {
        &self.name
    }

    fn is_visible(&self) -> bool {
        self.visible
    }
}

impl XrdsComponent for XrdsVideo360 {
    fn local_transform(&self) -> &TransformParams {
        &self.transform
    }

    fn local_transform_mut(&mut self) -> &mut TransformParams {
        &mut self.transform
    }
}

impl XrdsMutableComponent for XrdsVideo360 {
    fn set_name(&mut self, name: String) {
        self.name = name;
    }

    fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }
}
