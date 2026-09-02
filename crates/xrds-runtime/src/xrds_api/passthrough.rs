//! Projects `XrdsSceneMetadata::xr_blend_mode` onto the XR compositor.
//!
//! Until 2026-08-19 the field was inert: `grep -rn xr_blend_mode` over the tree
//! returned only its own definition. `XrdsXrBlendMode::AlphaBlend` could be
//! authored, saved and reloaded, and the scene stayed opaque. This is S4 of
//! `docs/done/small-phases-plan.md`.
//!
//! ## `AlphaBlend` does not mean `EnvironmentBlendMode::ALPHA_BLEND`
//!
//! The plan originally assumed it did. It does not, and implementing it that way
//! would have been worse than leaving the field inert. `EnvironmentBlendMode` is a
//! mandatory global `xrEndFrame` parameter deciding how the *entire frame* blends
//! with reality: selecting `ALPHA_BLEND` makes the real world show through wherever
//! any content's alpha is below 1.0 — unlit panels, particle trails, text atlases —
//! regardless of what the author wanted anywhere else in the scene.
//!
//! Passthrough is instead an `XR_FB_passthrough` composition layer submitted
//! *beneath* the projection layer, with the environment mode left `OPAQUE` and the
//! projection layer flagged `BLEND_TEXTURE_SOURCE_ALPHA | UNPREMULTIPLIED_ALPHA`.
//! Verified against a shipped Quest 3 passthrough application.
//!
//! ## Why this is state plus a system, not a one-shot on import
//!
//! The first version applied the mode directly during `import_scene_document`. On
//! device that silently did nothing, and the log said why:
//!
//! ```text
//! loaded 'scene.json' — 4 entities   <- import runs first
//! OpenXR session created             <- session exists only after
//! XR: passthrough layer created
//! ```
//!
//! The scene is imported **before** the OpenXR session exists, so neither
//! `OpenXrPassthroughEnabled` nor the XR cameras were there to configure, and the
//! authored value was dropped. The request is therefore stored in
//! [`XrdsRequestedBlendMode`] — which exists from app start, independent of XR —
//! and a system applies it once the session catches up, and re-applies it whenever
//! either side changes.
//!
//! ## Why the camera clear matters as much as the layer
//!
//! Passthrough is only *visible* where the scene renders alpha below 1.0. A layer
//! beneath an opaque frame shows nothing at all, so switching the mode on would
//! look like it had done nothing — the same silent no-op this tier exists to
//! remove. Enabling passthrough therefore also clears the XR cameras to fully
//! transparent, which is what turns "a layer is being submitted" into "the author
//! can see the room".

use bevy::camera::ClearColorConfig;
use bevy::prelude::*;
use xrds_openxr::{OpenXrCameraIndex, OpenXrPassthroughEnabled};
use xrds_scene_graph::XrdsXrBlendMode;

/// The author's `visible` on a passthrough window, held apart from Bevy's
/// [`Visibility`] because it is **necessary but not sufficient**.
///
/// A window is shown only when this is true *and* passthrough is on. Keeping the
/// intent here rather than in `Visibility` is what lets the entity spawn hidden
/// without losing what the author asked for, and it means the two conditions are
/// combined in exactly one place ([`sync_passthrough_system`]) rather than raced
/// between spawn and a later sync.
#[derive(Component, Debug, Clone, Copy)]
pub(crate) struct XrdsPassthroughWindowVisible(pub(crate) bool);

/// What the loaded scene asked for, independent of whether XR is up yet.
///
/// Exists from app start so an import that lands before the OpenXR session — which
/// is the normal order, not an edge case — still records the author's intent.
#[derive(Resource, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct XrdsRequestedBlendMode(pub(crate) XrdsXrBlendMode);

/// Records the scene's blend mode. Applied later by [`sync_passthrough_system`].
pub(crate) fn apply_xr_blend_mode(world: &mut World, mode: XrdsXrBlendMode) {
    world.insert_resource(XrdsRequestedBlendMode(mode));
    debug!("[passthrough] scene requests {mode:?}");
}

/// Keeps the compositor and the XR cameras in step with the authored blend mode.
///
/// Runs every frame but writes only on a real change, so the steady state is two
/// resource reads. It cannot be a startup or on-import system: the session, the
/// passthrough resource and the eye cameras all appear later than the scene, and
/// the eye cameras can be respawned during a session.
pub(crate) fn sync_passthrough_system(
    requested: Option<Res<XrdsRequestedBlendMode>>,
    enabled: Option<ResMut<OpenXrPassthroughEnabled>>,
    mut cameras: Query<&mut Camera, With<OpenXrCameraIndex>>,
    mut windows: Query<(&XrdsPassthroughWindowVisible, &mut Visibility)>,
) {
    let wants = matches!(
        requested.map(|r| r.0).unwrap_or_default(),
        XrdsXrBlendMode::AlphaBlend
    );

    // Absent until an XR session exists; on desktop it never appears and this is a
    // no-op, so the authored field round-trips unharmed through the editor.
    //
    // Passthrough windows spawn hidden and are revealed only below, so this early
    // return is also what keeps a desktop build from drawing an alpha-0 quad as a
    // black rectangle. That is deliberate: the desktop case is handled by *not*
    // reaching the code that shows them, rather than by a check that has to be
    // remembered.
    let Some(mut enabled) = enabled else { return };

    if enabled.0 != wants {
        enabled.0 = wants;
        info!(
            "[passthrough] {}",
            if wants { "enabled" } else { "disabled" }
        );
    }

    // Applied unconditionally rather than only on change: eye cameras are spawned
    // after the session and can be replaced, so a camera created later must still
    // pick up the current mode. `ClearColorConfig` is not `PartialEq`, so the
    // already-correct case is matched rather than compared — the write is trivial
    // either way, but skipping it avoids waking Bevy's change detection every frame
    // for two cameras.
    let already_correct = |c: &ClearColorConfig| match (wants, c) {
        (true, ClearColorConfig::Custom(color)) => *color == Color::NONE,
        (false, ClearColorConfig::Default) => true,
        _ => false,
    };

    for mut camera in cameras.iter_mut() {
        if already_correct(&camera.clear_color) {
            continue;
        }
        camera.clear_color = if wants {
            ClearColorConfig::Custom(Color::NONE)
        } else {
            ClearColorConfig::Default
        };
        debug!("[passthrough] eye camera clear updated (passthrough={wants})");
    }

    // A window clears its region to *fully clear* passthrough, which only means
    // anything while there is a passthrough layer beneath to reveal. With
    // passthrough off it would be a black rectangle over the scene, so it is not
    // shown at all — the video sphere simply covers that region like any other.
    //
    // Applied here for the same reason the camera clear is: windows can be spawned,
    // reimported and respawned long after the session exists, so this has to be a
    // system that keeps them in step rather than a decision made once at spawn.
    for (authored, mut visibility) in windows.iter_mut() {
        let target = if wants && authored.0 {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        if *visibility != target {
            *visibility = target;
        }
    }
}
