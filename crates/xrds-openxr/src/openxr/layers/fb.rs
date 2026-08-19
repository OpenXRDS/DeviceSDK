use bevy::prelude::*;

use crate::openxr::{
    layers::{OpenXrCompositionLayer, OpenXrLayerBuilder},
    resources::{OpenXrPassthroughEnabled, OpenXrPassthroughLayerHandle},
};

/// `XR_FB_passthrough`'s composition layer.
///
/// This is how passthrough is actually done on Quest. The obvious-looking route —
/// setting `EnvironmentBlendMode::ALPHA_BLEND` — is a different mechanism and the
/// wrong one: that enum is a mandatory global `xrEndFrame` parameter governing how
/// the *whole frame* blends with reality, so selecting it makes the real world show
/// through wherever any content's alpha is below 1.0, whatever the app intended.
/// The environment mode stays `OPAQUE`; reality arrives through this layer,
/// submitted *beneath* the projection layer, which is itself flagged
/// `BLEND_TEXTURE_SOURCE_ALPHA | UNPREMULTIPLIED_ALPHA` so the scene composites
/// over it.
///
/// Verified against a shipped Quest 3 passthrough app; the recipe and its
/// provenance are in `docs/small-phases-plan.md` S4.
#[derive(Clone)]
pub struct OpenXrCompositionLayerPassthroughFB {
    inner: openxr::sys::CompositionLayerPassthroughFB,
}

impl OpenXrCompositionLayer for OpenXrCompositionLayerPassthroughFB {
    fn as_raw(&self) -> &openxr::sys::CompositionLayerBaseHeader {
        // `CompositionLayerBaseHeader` is the common prefix of every composition
        // layer struct, and `CompositionLayerPassthroughFB` begins with exactly
        // that prefix (`ty`, `next`, `flags`, `space`), so reinterpreting the
        // reference is sound. openxr 0.19 has no safe wrapper for this layer type,
        // which is why this is done by hand here rather than by the crate.
        // `OpenXrCompositionLayerProjection::as_raw` does the same for its type.
        unsafe {
            #[allow(clippy::missing_transmute_annotations)]
            std::mem::transmute(&self.inner)
        }
    }
}

impl OpenXrCompositionLayerPassthroughFB {
    pub fn new(layer_handle: openxr::sys::PassthroughLayerFB) -> Self {
        Self {
            inner: openxr::sys::CompositionLayerPassthroughFB {
                ty: openxr::sys::CompositionLayerPassthroughFB::TYPE,
                next: std::ptr::null(),
                flags: openxr::CompositionLayerFlags::EMPTY,
                // The spec allows NULL here: a passthrough layer is not positioned
                // in a reference space, it fills the view.
                space: openxr::sys::Space::NULL,
                layer_handle,
            },
        }
    }
}

/// Submits the passthrough layer, but only while a scene asks for it.
///
/// Registered once at session creation when the device supports passthrough;
/// returns `None` on every frame where `OpenXrPassthroughEnabled` is false, which
/// is how the effect is switched off — `xrEndFrame` takes a fresh layer list each
/// frame, so omission is the mechanism.
#[derive(Clone)]
pub struct OpenXrCompositionLayerPassthroughFBBuilder;

impl OpenXrLayerBuilder for OpenXrCompositionLayerPassthroughFBBuilder {
    fn build(&self, world: &World) -> Option<Box<dyn OpenXrCompositionLayer>> {
        if !world
            .get_resource::<OpenXrPassthroughEnabled>()
            .is_some_and(|e| e.0)
        {
            return None;
        }
        let handle = world.get_resource::<OpenXrPassthroughLayerHandle>()?;
        Some(Box::new(OpenXrCompositionLayerPassthroughFB::new(handle.0)))
    }
}
