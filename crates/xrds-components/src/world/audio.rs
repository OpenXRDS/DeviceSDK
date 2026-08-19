use crate::{default_component_name, TransformParams, XrdsComponent, XrdsMutableComponent, XrdsObject};

/// Distance rolloff model for spatial audio.
///
/// Lives here rather than in `xrds-scene-graph` so the runtime can evaluate a
/// falloff curve without depending on the document layer — the same reason
/// [`crate::XrdsGrabType`] and [`crate::XrdsInteractionZoneShape`] live here.
/// `xrds-scene-graph` re-exports it.
///
/// The curves follow the Web Audio `PannerNode` distance models, which is what
/// `Inverse` being the default already implied.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, Default)]
pub enum XrdsAudioDistanceModel {
    /// Gain decreases linearly from `min_distance` to `max_distance`.
    Linear,
    /// Gain decreases by the inverse of distance (Web Audio default).
    #[default]
    Inverse,
    /// Gain decreases exponentially with distance.
    Exponential,
}

impl XrdsAudioDistanceModel {
    /// Gain in `0.0..=1.0` for a source `distance` metres from the listener.
    ///
    /// Deliberate deviation from Web Audio: **beyond `max_distance` the gain is
    /// zero.** Web Audio clamps the distance instead, so an `Inverse` source stays
    /// quietly audible for ever, which makes `max_distance` mean nothing an author
    /// would recognise. `Linear` already reaches zero at `max_distance`; the other
    /// two get a small step down there, accepted so that "past this it is silent"
    /// is true for all three.
    ///
    /// Degenerate inputs are clamped rather than rejected — this runs per frame per
    /// source and must not panic on an author's typo: `min` is floored just above
    /// zero, `max` is held above `min`, and a negative `rolloff` is treated as zero.
    pub fn gain(&self, distance: f32, min: f32, max: f32, rolloff: f32) -> f32 {
        // A hard floor rather than `f32::EPSILON`, which is a relative quantity:
        // `min + f32::EPSILON` rounds straight back to `min` at any magnitude
        // above ~1.0, so guarding a span that way leaves it exactly zero and the
        // divisions below produce NaN. Caught by `degenerate_inputs_*` below.
        let min = min.max(1e-6);
        let rolloff = rolloff.max(0.0);
        let d = distance.max(0.0);

        // Ordered so the two degenerate cases resolve before any arithmetic, and
        // so `max <= min` — an empty audible band — still means "silent outside
        // the full-volume radius" rather than a division by zero.
        if d > max {
            return 0.0;
        }
        if d <= min {
            // Inside `min` the source is at full volume; getting closer must not
            // amplify it.
            return 1.0;
        }

        // Past both guards `min < d <= max` holds, so `d - min` and `max - min`
        // are both strictly positive and no division here can be by zero.
        let gain = match self {
            Self::Linear => 1.0 - rolloff * (d - min) / (max - min),
            Self::Inverse => min / (min + rolloff * (d - min)),
            Self::Exponential => (d / min).powf(-rolloff),
        };

        // Fade to nothing across the last stretch of the range, rather than
        // stepping off a cliff at `max_distance`.
        //
        // `Linear` already lands on zero there, but `Inverse` and `Exponential`
        // are asymptotic and are still clearly audible when the cutoff hits — so
        // they went silent in a single frame, which a listener on a Quest called
        // "a bit dumb", and they were right: nothing in the world stops making
        // noise at a hard radius.
        //
        // The band is **half** the range, not a sliver. A first attempt used 25%
        // and was still reported as too abrupt, because the ear hears decibels
        // while this arithmetic is in amplitude: a smoothstep from 0.2 to 0 is a
        // gentle curve on paper and a fall from -14 dB to silence in practice.
        // Widening the band is what buys perceptual smoothness; the shape of the
        // interpolant barely matters by comparison.
        //
        // Values above the halfway point of the band are still exactly as
        // authored — the taper reaches 1.0 there — so this does not quietly
        // rewrite the curve an author set.
        const EDGE_FADE_FRACTION: f32 = 0.5;
        let fade_band = (max - min) * EDGE_FADE_FRACTION;
        let taper = if fade_band > 0.0 {
            ((max - d) / fade_band).clamp(0.0, 1.0)
        } else {
            1.0
        };
        // Smoothstep rather than a straight ramp: a linear fade still has a
        // corner where it meets the curve, and a slow fade-out is where a corner
        // is most audible.
        let taper = taper * taper * (3.0 - 2.0 * taper);

        (gain * taper).clamp(0.0, 1.0)
    }
}

#[derive(Debug, Clone)]
pub struct XrdsAudioClip {
    pub name: String,
    pub transform: TransformParams,
    pub visible: bool,
    /// Catalog asset id referencing an `XrdsSceneAssetKind::Audio` asset.
    pub audio_asset_id: String,
    /// Playback volume in the range 0.0–1.0.
    pub volume: f32,
    pub looped: bool,
    /// When `true` the clip is attenuated by 3-D distance from the listener.
    /// When `false` the clip plays at the same level everywhere in the scene.
    pub spatial: bool,
    pub autoplay: bool,
    // ── Distance falloff. Only consulted when `spatial` is true. ──
    pub distance_model: XrdsAudioDistanceModel,
    /// Full volume at or within this distance, in metres.
    pub min_distance: f32,
    /// Silent beyond this distance, in metres.
    pub max_distance: f32,
    /// How steeply gain falls between `min_distance` and `max_distance`.
    pub rolloff_factor: f32,
}

impl XrdsAudioClip {
    pub fn new(audio_asset_id: impl Into<String>) -> Self {
        Self {
            name: default_component_name::<Self>(),
            transform: TransformParams::default(),
            visible: true,
            audio_asset_id: audio_asset_id.into(),
            volume: 1.0,
            looped: false,
            spatial: true,
            autoplay: false,
            distance_model: XrdsAudioDistanceModel::default(),
            min_distance: 1.0,
            max_distance: 50.0,
            rolloff_factor: 1.0,
        }
    }

    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    /// Set the distance falloff curve. See [`XrdsAudioDistanceModel::gain`].
    pub fn with_falloff(
        mut self,
        model: XrdsAudioDistanceModel,
        min_distance: f32,
        max_distance: f32,
        rolloff_factor: f32,
    ) -> Self {
        self.distance_model = model;
        self.min_distance = min_distance;
        self.max_distance = max_distance;
        self.rolloff_factor = rolloff_factor;
        self
    }
}

impl XrdsObject for XrdsAudioClip {
    fn name(&self) -> &str {
        &self.name
    }

    fn is_visible(&self) -> bool {
        self.visible
    }
}

impl XrdsComponent for XrdsAudioClip {
    fn local_transform(&self) -> &TransformParams {
        &self.transform
    }

    fn local_transform_mut(&mut self) -> &mut TransformParams {
        &mut self.transform
    }
}

impl XrdsMutableComponent for XrdsAudioClip {
    fn set_name(&mut self, name: String) {
        self.name = name;
    }

    fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }
}

#[cfg(test)]
mod tests {
    use super::XrdsAudioDistanceModel::*;
    use super::*;

    const MIN: f32 = 2.0;
    const MAX: f32 = 20.0;

    fn gain(model: XrdsAudioDistanceModel, d: f32) -> f32 {
        model.gain(d, MIN, MAX, 1.0)
    }

    #[test]
    fn full_volume_at_and_within_min_distance() {
        for model in [Linear, Inverse, Exponential] {
            assert_eq!(gain(model, MIN), 1.0, "{model:?} at min");
            assert_eq!(gain(model, 0.5), 1.0, "{model:?} closer than min");
            assert_eq!(gain(model, 0.0), 1.0, "{model:?} on top of listener");
        }
    }

    #[test]
    fn silent_beyond_max_distance() {
        // The documented deviation from Web Audio, which clamps instead and would
        // leave an Inverse source quietly audible for ever.
        for model in [Linear, Inverse, Exponential] {
            assert_eq!(gain(model, MAX + 0.01), 0.0, "{model:?} past max");
            assert_eq!(gain(model, 10_000.0), 0.0, "{model:?} far past max");
        }
    }

    #[test]
    fn linear_reaches_zero_at_max_and_half_at_midpoint() {
        assert!(gain(Linear, MAX).abs() < 1e-6);
        let mid = MIN + (MAX - MIN) / 2.0;
        assert!((gain(Linear, mid) - 0.5).abs() < 1e-6, "got {}", gain(Linear, mid));
    }

    #[test]
    fn inverse_halves_at_twice_the_min_distance() {
        // min / (min + rolloff * (d - min)) = 2 / (2 + 2) = 0.5
        assert!((gain(Inverse, 2.0 * MIN) - 0.5).abs() < 1e-6);
    }

    #[test]
    fn every_model_decreases_monotonically() {
        for model in [Linear, Inverse, Exponential] {
            let mut previous = f32::INFINITY;
            let mut d = MIN;
            while d <= MAX {
                let g = gain(model, d);
                assert!(g <= previous + 1e-6, "{model:?} rose at {d} m");
                assert!((0.0..=1.0).contains(&g), "{model:?} out of range at {d} m");
                previous = g;
                d += 0.25;
            }
        }
    }

    #[test]
    fn higher_rolloff_attenuates_harder() {
        for model in [Linear, Inverse, Exponential] {
            let gentle = model.gain(10.0, MIN, MAX, 0.5);
            let steep = model.gain(10.0, MIN, MAX, 2.0);
            assert!(steep < gentle, "{model:?}: {steep} should be under {gentle}");
        }
    }

    #[test]
    fn zero_rolloff_never_attenuates() {
        for model in [Linear, Inverse, Exponential] {
            assert_eq!(model.gain(10.0, MIN, MAX, 0.0), 1.0, "{model:?}");
        }
    }

    /// Every model must arrive at silence smoothly, not step off a cliff. Before
    /// the edge taper, `Inverse` was still at ~0.18 when the cutoff hit and dropped
    /// to zero in one frame — audible, and reported from a device as "a bit dumb".
    #[test]
    fn every_model_approaches_silence_without_a_step() {
        for model in [Linear, Inverse, Exponential] {
            let just_inside = gain(model, MAX - 0.05);
            assert!(
                just_inside < 0.02,
                "{model:?} was still at {just_inside} right before max_distance",
            );
            // And no jump across the boundary itself.
            let outside = gain(model, MAX + 0.05);
            assert!(
                (just_inside - outside).abs() < 0.02,
                "{model:?} stepped from {just_inside} to {outside} at the boundary",
            );
        }
    }

    /// The taper must not disturb the authored curve away from the boundary,
    /// otherwise it silently rewrites every value an author set.
    #[test]
    fn taper_leaves_the_body_of_the_curve_alone() {
        // Fade band is the last 25%, so everything below 75% of the span is exact.
        let mid = MIN + (MAX - MIN) * 0.5;
        assert!((gain(Linear, mid) - 0.5).abs() < 1e-6);
        assert!((gain(Inverse, 2.0 * MIN) - 0.5).abs() < 1e-6);
    }

    /// Runs every frame per source, so an author's typo must clamp rather than
    /// produce NaN, infinity, or a panic — any of which reaches the audio thread.
    #[test]
    fn degenerate_inputs_stay_finite_and_in_range() {
        let nonsense = [
            (0.0_f32, 0.0_f32, 0.0_f32),   // all zero
            (10.0, 1.0, 1.0),              // max below min
            (5.0, 5.0, 1.0),               // max equal to min
            (-1.0, -5.0, -2.0),            // all negative
            (1.0, 50.0, f32::INFINITY),    // infinite rolloff
        ];
        for model in [Linear, Inverse, Exponential] {
            for (min, max, rolloff) in nonsense {
                for d in [0.0_f32, 1.0, 25.0, 1e6] {
                    let g = model.gain(d, min, max, rolloff);
                    assert!(g.is_finite(), "{model:?} produced {g} for {min}/{max}/{rolloff} at {d}");
                    assert!((0.0..=1.0).contains(&g), "{model:?} produced {g}");
                }
            }
        }
    }
}
