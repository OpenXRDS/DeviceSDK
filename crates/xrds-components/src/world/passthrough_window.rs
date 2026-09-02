//! A window in the wall of a blended environment: a region cleared to fully
//! clear passthrough.

use crate::{TransformParams, XrdsComponent, XrdsMutableComponent, XrdsObject};

/// A rectangle that clears itself to *fully clear* passthrough.
///
/// The case it exists for is a 360 video over passthrough — the room is dimly
/// visible everywhere at the video's opacity, and this makes one rectangle
/// *really* the room. Nothing about it is video-specific, though: any scene
/// blending over reality can use one.
///
/// **A window in a wall.** The surface writes alpha 0 in the opaque pass, so it
/// wins the depth test against anything behind it and the compositor shows
/// passthrough there at 100%. Everything *behind* the plane is outside the room,
/// and outside is reality; everything *in front* is still drawn over reality. So
/// there is no list of things to exclude: geometry is the region, and depth
/// decides what it replaces.
///
/// **There is no opacity field, deliberately.** Whatever is behind is
/// depth-rejected either way, so partial alpha would tint the room toward black
/// rather than let any video through. Full clarity or nothing.
///
/// **It does not exist when passthrough is off.** With no composition layer
/// beneath, an alpha-0 surface is a black rectangle — worse than inert, because
/// the author sees black, ships it, and it is correct on device. So this spawns
/// with `Visibility::Hidden` regardless of `visible`, and only the runtime's
/// passthrough sync reveals it. See `docs/video-360-plan.md`.
#[derive(Debug, Clone)]
pub struct XrdsPassthroughWindow {
    pub name: String,
    pub transform: TransformParams,
    /// The author's intent, which is *necessary but not sufficient*: a window is
    /// shown only when this is true **and** passthrough is on.
    pub visible: bool,
    /// Metres, along local X.
    pub width: f32,
    /// Metres, along local Y.
    pub height: f32,
}

impl Default for XrdsPassthroughWindow {
    fn default() -> Self {
        Self {
            name: "PassthroughWindow".to_string(),
            transform: TransformParams::default(),
            visible: true,
            width: 1.0,
            height: 1.0,
        }
    }
}

impl XrdsObject for XrdsPassthroughWindow {
    fn name(&self) -> &str {
        &self.name
    }

    fn is_visible(&self) -> bool {
        self.visible
    }
}

impl XrdsComponent for XrdsPassthroughWindow {
    fn local_transform(&self) -> &TransformParams {
        &self.transform
    }

    fn local_transform_mut(&mut self) -> &mut TransformParams {
        &mut self.transform
    }
}

impl XrdsMutableComponent for XrdsPassthroughWindow {
    fn set_name(&mut self, name: String) {
        self.name = name;
    }

    fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }
}
