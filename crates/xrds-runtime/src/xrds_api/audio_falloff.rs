//! Authored distance falloff for spatial audio clips.
//!
//! `XrdsSceneAudioClip` has carried `distance_model`, `min_distance`,
//! `max_distance` and `rolloff_factor` for a long time, and until 2026-08-19
//! nothing read any of them: `spawn_audio_clip_descriptor` passed only
//! `spatial: bool` to Bevy. An author could set a falloff, watch it save and
//! reload, and hear no difference. This module is what makes those fields mean
//! something.
//!
//! ## What rodio can and cannot do, established by listening
//!
//! Two properties of `rodio::source::Spatial` shape everything below, and neither
//! is discoverable from its documentation:
//!
//! 1. **`dist_modifier` — the per-ear `(1.0 / dist_sq).min(1.0)` — *is* the panner.**
//!    The near ear is closer, so it gets a larger value. There is no separate pan
//!    law.
//! 2. **`diff_modifier` is weak and inverted.** It spans only `0.5..=1.0` (a 6 dB
//!    range) and gives the ear *nearer* the source the *smaller* value. It works
//!    against the panning, and is simply overpowered by `dist_modifier` in normal use.
//!
//! A consequence worth knowing before promising anything about direction: panning
//! strength collapses with distance. A source 3 m away and hard left gives roughly
//! 22 dB between the ears; at 10 m the same source gives about 1 dB, because
//! `dist_modifier`'s ratio tends to 1 while the inverted `diff_modifier` does not.
//! Rodio pans convincingly up close and barely at all far away.
//!
//! ## How rodio's own falloff is kept out of the way
//!
//! Bevy's spatial audio is `rodio::source::Spatial`, which downmixes to mono and
//! applies a per-ear gain that is **hardcoded** (`rodio-0.20.1/src/source/spatial.rs`):
//!
//! ```text
//! let left_dist_modifier = (1.0 / left_dist_sq).min(1.0);
//! ```
//!
//! A fixed inverse-square law, clamped at 1.0, with no exponent, no maximum and no
//! model choice. It cannot be configured or disabled while `spatial` is true, so an
//! authored curve set naively on the sink would land as `authored × (1/d²)`.
//!
//! We divide it back out: the sink volume is multiplied by `d²`. Because that is a
//! single scalar applied to both channels, it scales the pair equally and leaves
//! rodio's left/right ratio — the panning — untouched.
//!
//! ### Accuracy limit, stated rather than hidden
//!
//! Rodio works from **per-ear** distances while the sink volume is one scalar, so
//! the cancellation is exact only for a listener whose ears coincide with its
//! centre. Bevy's default ear gap is `4.0` world units (`SpatialListener::default`
//! → `new(4.0)`, ears at ±2.0 on X), so the two ears can be up to 2 m nearer or
//! further than the centre, and the corrected level is correspondingly approximate
//! in the near field. It is monotone and it reaches silence exactly at
//! `max_distance`, which is what the authored fields promise; it is not a
//! calibrated absolute level.
//!
//! Closing that gap properly means not using rodio's spatialization at all —
//! `spatial: false` plus a panner of our own, which is a larger job than this
//! phase. See `docs/spatial-audio-backend-spike.md`.
//!
//! ### An approach that was tried and reverted
//!
//! A revision of this file set a per-clip `SpatialScale` small enough to pin
//! `dist_modifier` at its clamp for both ears, on the theory that what remained was
//! the panner. That is backwards, per point 1 above: it pinned the panner and left
//! only the weak inverted term, and the stereo image collapsed to centre. The
//! measured falloff numbers were *perfect* throughout — sink volume equalled the
//! authored gain to three decimals — which is exactly why it survived review and
//! had to be caught by ear. Do not reintroduce it.

use bevy::prelude::*;
use bevy::audio::{AudioSinkPlayback, SpatialAudioSink, SpatialListener, Volume};
use xrds_components::XrdsAudioDistanceModel;

use super::anchor::{pick_head_camera_entity, XrdsPlayerCamera};

/// The authored falloff curve for one spatial audio clip.
///
/// Inserted at spawn only when the clip is spatial — an ambient clip has no
/// listener-relative gain to compute and keeps the flat volume Bevy applied.
#[derive(Component, Debug, Clone, Copy)]
pub(crate) struct XrdsAudioFalloff {
    pub(crate) distance_model: XrdsAudioDistanceModel,
    pub(crate) min_distance: f32,
    pub(crate) max_distance: f32,
    pub(crate) rolloff_factor: f32,
    /// The clip's authored `volume`, which the curve scales. Kept here because the
    /// sink's own volume is overwritten every frame and so cannot store it.
    pub(crate) base_volume: f32,
}

/// The mean of the two channel gains rodio will apply, reproducing its arithmetic
/// exactly (`rodio-0.20.1/src/source/spatial.rs`, `Spatial::set_positions`).
///
/// Both of rodio's terms are included, not just the distance one: `diff_modifier`
/// spans `0.5..=1.0` and so shifts the overall level by up to 6 dB on its own.
fn rodio_mean_channel_gain(emitter: Vec3, left_ear: Vec3, right_ear: Vec3) -> f32 {
    let left_dist_sq = left_ear.distance_squared(emitter);
    let right_dist_sq = right_ear.distance_squared(emitter);
    let max_diff = left_ear.distance(right_ear);
    if max_diff <= f32::EPSILON {
        return 1.0;
    }
    let left_dist = left_dist_sq.sqrt();
    let right_dist = right_dist_sq.sqrt();

    let left_diff = (((left_dist - right_dist) / max_diff + 1.0) / 4.0 + 0.5).min(1.0);
    let right_diff = (((right_dist - left_dist) / max_diff + 1.0) / 4.0 + 0.5).min(1.0);
    let left_dist_mod = (1.0 / left_dist_sq).min(1.0);
    let right_dist_mod = (1.0 / right_dist_sq).min(1.0);

    (left_diff * left_dist_mod + right_diff * right_dist_mod) / 2.0
}

/// Scalar that makes rodio's *average* output equal the authored gain.
///
/// ## Why the average, and why per ear
///
/// The first version corrected by the distance to the listener's **centre**,
/// assuming that cancels what rodio applies. It does not: rodio measures per ear,
/// and Bevy's default gap puts an ear ±2.0 world units off centre. A source passing
/// near one ear has that ear pinned at rodio's `.min(1.0)` clamp while the centre
/// distance keeps growing, so the correction over-drove it by up to ~30 dB. On a
/// Quest that came back as "when I put the sound source on left/right ear it was
/// loud, but when testing the distance, it wasn't that loud" — lateral motion
/// swamping the distance curve it was supposed to sit underneath.
///
/// Correcting against the mean of the two channels fixes the level in every
/// orientation while leaving the left/right *ratio* untouched — it is still one
/// scalar applied to both channels — so panning remains rodio's, unmodified.
fn rodio_correction(emitter: Vec3, left_ear: Vec3, right_ear: Vec3) -> f32 {
    let mean = rodio_mean_channel_gain(emitter, left_ear, right_ear);
    if mean <= f32::EPSILON {
        1.0
    } else {
        1.0 / mean
    }
}

/// Update a live clip's falloff without respawning it.
///
/// The curve is read fresh every frame by [`audio_falloff_system`], so changing
/// the component is enough — no sink rebuild, no reimport. That matters for
/// authoring: reimporting to apply a slider would respawn the entity, restart
/// every sound in the scene, and cut off the very preview the author is listening
/// to while dragging.
///
/// Returns false when the node has no falloff component, which means it is not an
/// audio clip or is not spatial. Non-spatial clips deliberately have none — they
/// play at one volume everywhere, and there is nothing to attenuate.
pub(crate) fn set_falloff_for_entity(
    world: &mut World,
    entity: Entity,
    distance_model: XrdsAudioDistanceModel,
    min_distance: f32,
    max_distance: f32,
    rolloff_factor: f32,
    base_volume: f32,
) -> bool {
    let Some(mut falloff) = world.get_mut::<XrdsAudioFalloff>(entity) else {
        return false;
    };
    falloff.distance_model = distance_model;
    falloff.min_distance = min_distance;
    falloff.max_distance = max_distance;
    falloff.rolloff_factor = rolloff_factor;
    falloff.base_volume = base_volume.clamp(0.0, 1.0);
    true
}

/// Keeps `SpatialListener` on the entity that actually carries the head pose.
///
/// **This is what makes spatial audio work in XR at all.** `SpatialListener` used to
/// be inserted in exactly one place — the `XrdsAPI` camera-spawn path
/// (`spawn.rs:120`) — which covers a desktop app that spawns its camera through
/// `XrdsAPI` and covers nothing else. On a Quest the player camera is built by the
/// host app from raw Bevy components and the eye cameras come from `xrds-openxr`,
/// so **no entity had the component**, with two consequences that look unrelated
/// and are not:
///
/// - [`audio_falloff_system`] found no listener, returned early, and applied no
///   attenuation whatsoever. Distance did nothing.
/// - Bevy's `EarPositions::get` fell back to `SpatialListener::default()`
///   *untransformed*, pinning the ears at the world origin. Panning was computed
///   against the origin rather than the player's head, so turning and walking
///   changed nothing.
///
/// Reported from a Quest as "I can hear the ping but hard to recognize the volume
/// and direction changing" — one cause presenting as two separate failures.
///
/// Placement follows [`pick_head_camera_entity`], the same priority the anchor
/// systems use, because the listener and the head must be the same entity. Exactly
/// one listener is kept: Bevy silently uses the first when several exist, so a
/// stale one on a scene camera would quietly win.
pub(crate) fn sync_spatial_listener_system(
    mut commands: Commands,
    cameras: Query<(Entity, &Projection, &Camera)>,
    player_camera: Query<Entity, With<XrdsPlayerCamera>>,
    listeners: Query<Entity, With<SpatialListener>>,
) {
    let Some(target) = pick_head_camera_entity(&cameras, player_camera.iter().next()) else {
        return;
    };

    let mut target_has_listener = false;
    for entity in listeners.iter() {
        if entity == target {
            target_has_listener = true;
        } else {
            commands.entity(entity).remove::<SpatialListener>();
        }
    }

    if !target_has_listener {
        commands.entity(target).insert(SpatialListener::default());
    }
}

/// Applies each spatial clip's authored falloff to its sink, every frame.
///
/// Runs in `Update`. A clip whose sink has not been created yet — Bevy inserts it
/// an observer tick after `AudioPlayer` appears, and `AudioPlayer` itself waits on
/// `pre_validate_audio_decoders_system` — is simply skipped and picked up on a
/// later frame.
pub(crate) fn audio_falloff_system(
    listener: Query<(&GlobalTransform, &SpatialListener)>,
    mut clips: Query<(&Name, &GlobalTransform, &XrdsAudioFalloff, &mut SpatialAudioSink)>,
    time: Res<Time>,
    mut since_last_log: Local<f32>,
) {
    // Bevy itself picks the first listener when several exist (and warns), so
    // matching that here keeps the gain consistent with the panning.
    let Some((listener_tf, ears)) = listener.iter().next() else {
        return;
    };
    let listener_pos = listener_tf.translation();
    // The same ear positions rodio will be given, so the correction below cancels
    // what actually happens rather than an idealisation of it. `spatial_scale` is
    // left at Bevy's default, so no scaling is applied here either.
    let left_ear = listener_tf.transform_point(ears.left_ear_offset);
    let right_ear = listener_tf.transform_point(ears.right_ear_offset);

    // Throttled because this runs per frame per source: at 90 Hz an unthrottled
    // line would bury every other log on device within seconds (Trap 2 in
    // docs/quest-device-test-recipe.md).
    *since_last_log += time.delta_secs();
    let log_this_frame = *since_last_log >= 1.0;
    if log_this_frame {
        *since_last_log = 0.0;
    }

    for (name, clip_tf, falloff, mut sink) in clips.iter_mut() {
        let emitter = clip_tf.translation();
        let distance = emitter.distance(listener_pos);

        let gain = falloff.distance_model.gain(
            distance,
            falloff.min_distance,
            falloff.max_distance,
            falloff.rolloff_factor,
        );

        // One scalar for both channels, so it cannot disturb the left/right ratio
        // rodio computed — panning stays rodio's, untouched. What it does is
        // cancel rodio's own level so the authored curve is what is heard.
        let volume =
            falloff.base_volume * gain * rodio_correction(emitter, left_ear, right_ear);
        sink.set_volume(Volume::Linear(volume));

        if log_this_frame {
            // Distance and gain together, because either alone is ambiguous: a
            // silent source at 3 m could be a working curve or a broken one, and
            // only the pair says which.
            debug!(
                "[audio-falloff] '{name}' d={distance:.2}m model={:?} \
                 min={:.1} max={:.1} rolloff={:.1} -> gain={gain:.3} sink_volume={volume:.3}",
                falloff.distance_model,
                falloff.min_distance,
                falloff.max_distance,
                falloff.rolloff_factor,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Bevy's default ear positions for a listener at the origin facing -Z.
    fn default_ears() -> (Vec3, Vec3) {
        (Vec3::new(-2.0, 0.0, 0.0), Vec3::new(2.0, 0.0, 0.0))
    }

    /// The correction must cancel what rodio actually applies, so the authored
    /// gain is the level heard. Asserted against rodio's own arithmetic.
    #[test]
    fn correction_cancels_rodios_mean_output() {
        let (left, right) = default_ears();
        for emitter in [
            Vec3::new(0.0, 0.0, -3.0),  // straight ahead
            Vec3::new(0.0, 0.0, -12.0), // far ahead
            Vec3::new(-2.0, 0.0, 0.0),  // on the left ear
            Vec3::new(5.0, 0.0, -1.0),  // off to the right
            Vec3::new(0.0, 3.0, 0.0),   // overhead
        ] {
            let mean = rodio_mean_channel_gain(emitter, left, right);
            let net = mean * rodio_correction(emitter, left, right);
            assert!(
                (net - 1.0).abs() < 1e-4,
                "net gain for emitter {emitter:?} was {net}, expected 1.0",
            );
        }
    }

    /// The bug a listener caught on a Quest: correcting by the *centre* distance
    /// while rodio measures per ear over-drove a source passing close to one ear.
    /// With Bevy's ±2.0 ear offsets the old scheme boosted it enormously; the
    /// mean-based correction must stay sane in the same geometry.
    #[test]
    fn lateral_sources_are_not_over_amplified() {
        let (left, right) = default_ears();
        let beside_the_left_ear = Vec3::new(-2.2, 0.0, 0.0);
        let centre_distance = beside_the_left_ear.length();

        let old_scheme = centre_distance * centre_distance; // correct-by-centre
        let corrected = rodio_correction(beside_the_left_ear, left, right);

        assert!(
            corrected < old_scheme,
            "mean correction {corrected} should be gentler than centre correction {old_scheme}",
        );
    }

    /// Guards the regression that a `SpatialScale` override caused: rodio's
    /// per-ear `dist_modifier` IS the panning, so anything that flattens it
    /// destroys the stereo image while leaving the falloff numbers looking
    /// perfect. This asserts the near ear stays louder — the property that
    /// actually failed — using rodio's own two terms.
    #[test]
    fn rodios_near_ear_stays_louder_than_the_far_ear() {
        let channel_gain = |near: f32, far: f32, gap: f32| {
            let diff = ((near - far) / gap + 1.0) / 4.0 + 0.5;
            let dist = (1.0 / (near * near)).min(1.0);
            diff.min(1.0) * dist
        };
        // Source hard left of a listener with Bevy's default 4.0 ear gap.
        for (near, far) in [(1.0_f32, 5.0_f32), (3.0, 7.0), (8.0, 12.0)] {
            let near_gain = channel_gain(near, far, 4.0);
            let far_gain = channel_gain(far, near, 4.0);
            assert!(
                near_gain > far_gain,
                "near ear at {near} m ({near_gain}) should beat far ear at {far} m ({far_gain})",
            );
        }
    }
}
