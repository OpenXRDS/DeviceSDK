use crate::{
    default_component_name, TransformParams, XrdsColor, XrdsComponent, XrdsMutableComponent,
    XrdsObject,
};

/// How an [`XrdsEffect`] emits particles.
///
/// Deliberately two kinds rather than a general emission model — the same
/// reasoning as `XrdsPointLight` exposing a curated subset of Bevy's light API.
/// An author needing more drops to the expert layer and uses the backend crate
/// directly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum XrdsEffectKind {
    /// One-shot spawn — an impact, an explosion, a spark hit. Sized by
    /// [`XrdsEffect::burst_count`].
    Burst,
    /// Continuous emission — smoke, a sparkle trail, an ambient plume. Paced by
    /// [`XrdsEffect::spawn_rate`].
    Trail,
}

/// How a particle's colour combines with what is already on screen.
///
/// A curated three of `bevy_firework`'s five modes. `Opaque` and `Premultiplied`
/// are omitted as expert-layer concerns.
///
/// # Currently has no visible effect
///
/// **`bevy_firework` 0.8 ignores this.** Its render pipeline hardcodes
/// `BlendState::ALPHA_BLENDING` (`render.rs:875`) rather than deriving the blend
/// state from `alpha_mode`, and while the value does reach the shader's uniform,
/// `particles.wgsl` never reads it. All three modes therefore render identically.
///
/// Confirmed on a Quest 3 with three otherwise-identical effects side by side —
/// `Blend`, `Add` and `Multiply` were indistinguishable.
///
/// The field is kept because the value is plumbed correctly all the way to the
/// backend and will start working the moment upstream honours it; removing it
/// would mean re-adding the same field through five layers later. It is marked as
/// non-functional in the editor so nobody tunes it expecting a result.
///
/// An earlier note here claimed `Add` was the way to get a glow without a bloom
/// pass. That was wrong — with this backend there is currently no way to do it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum XrdsEffectBlend {
    /// Normal alpha blending. Smoke, dust, debris.
    Blend,
    /// Additive: overlapping particles brighten each other. Fire, sparks, magic.
    Add,
    /// Multiplicative: darkens what is behind. Soot, shadow puffs.
    Multiply,
}

/// A particle effect.
///
/// Named `XrdsEffect`, **not** `XrdsEmitter`: "emitter" already means a node
/// that *fires a trigger* elsewhere in this codebase (see `trigger_action.rs`),
/// and reusing it would collide with an established, unrelated meaning.
///
/// The parameter set is intentionally small and backend-agnostic. No type from
/// the particle backend appears here, so the backend stays swappable — which is
/// not hypothetical caution: the first backend chosen for this feature
/// (`bevy_hanabi`) had to be replaced after it turned out to render nothing on
/// Qualcomm Adreno GPUs. See `docs/done/vfx-particle-effects-plan.md`.
#[derive(Debug, Clone)]
pub struct XrdsEffect {
    pub name: String,
    pub enabled: bool,
    pub visible: bool,
    pub transform: TransformParams,
    pub kind: XrdsEffectKind,
    /// Whether the effect starts emitting the moment it exists.
    ///
    /// `true` (the default) means a `Trail` begins running and a `Burst` fires
    /// once, immediately — the "just works when you spawn it" behaviour.
    ///
    /// `false` leaves the effect idle, waiting to be fired by a trigger or Track
    /// (`XrdsAction::PlayEffect`).
    ///
    /// This governs behaviour **at load only**. `PlayEffect` fires an effect
    /// whichever way this is set — it adjusts the backend's pacing as needed — so
    /// leaving it `true` on a trigger-driven effect is not broken, merely an
    /// extra burst nobody asked for when the scene opens.
    ///
    /// Authoring tools may reasonably default this differently from the Rust
    /// default — a burst dragged into a scene almost always wants a trigger.
    pub auto_play: bool,
    /// Total particles emitted per firing. Used only when [`Self::kind`] is
    /// [`XrdsEffectKind::Burst`]; ignored for `Trail`.
    ///
    /// Deliberately a separate field from [`Self::spawn_rate`] rather than one
    /// overloaded number: the two mean genuinely different things (a count
    /// versus a frequency), and sharing a field made switching `kind` silently
    /// reinterpret the value.
    pub burst_count: u32,
    /// Particles emitted per second. Used only when [`Self::kind`] is
    /// [`XrdsEffectKind::Trail`]; ignored for `Burst`.
    ///
    /// Simulation is CPU-side, so this budgets in the *thousands* of live
    /// particles, not the hundreds of thousands a GPU-compute system would
    /// allow. Live count settles around `spawn_rate * lifetime_secs`.
    pub spawn_rate: f32,
    /// Seconds each particle lives before disappearing.
    pub lifetime_secs: f32,
    /// Particle scale, sampled uniformly between the two per particle. Set both
    /// to the same value for a uniform size.
    pub size_min: f32,
    pub size_max: f32,
    /// Colour at spawn, interpolated to [`Self::color_end`] over the particle's
    /// lifetime.
    ///
    /// **Keep components ≤ 1.0.** Values above 1.0 are an HDR/bloom idiom, and
    /// the SDK's XR eye cameras carry neither `Hdr` nor `Bloom` — on those,
    /// anything brighter clamps and every particle renders pure white. Observed
    /// on a Quest 3, not theorised. Alpha is respected, so fading out via
    /// `color_end`'s alpha is the normal way to make particles disappear
    /// smoothly.
    pub color_start: XrdsColor,
    pub color_end: XrdsColor,
    /// Initial speed, sampled uniformly between the two per particle.
    pub speed_min: f32,
    pub speed_max: f32,
    /// When `true`, particles fly outward from the emission point in every
    /// direction and [`Self::spread_deg`] is ignored.
    ///
    /// An explicit flag rather than inferring it from `spread_deg >= 180`:
    /// omnidirectional and cone emission are separate behaviours in the backend
    /// (one pushes radially from the spawn point, the other picks a direction),
    /// and a "180 degree cone" silently produced a lopsided hemisphere that
    /// still looked plausible at a glance.
    pub omnidirectional: bool,
    /// Cone half-angle around the effect's local +Y, in degrees. `0` gives a
    /// straight jet, `45` a modest spray. Ignored when [`Self::omnidirectional`]
    /// is `true`.
    pub spread_deg: f32,
    /// Constant acceleration applied to every particle, in world units per
    /// second squared. `[0.0, -9.81, 0.0]` approximates earth gravity;
    /// `[0.0; 3]` leaves particles drifting.
    pub gravity: [f32; 3],
    /// Radius of the sphere particles spawn within. `0` spawns them all from a
    /// single point.
    pub emission_radius: f32,
    /// How particle colour combines with the scene. See [`XrdsEffectBlend`].
    pub blend: XrdsEffectBlend,
    /// Size multiplier at the end of a particle's life, relative to its spawn
    /// size. `1.0` holds size constant, `0.0` shrinks to nothing, `>1.0` grows.
    ///
    /// A single scalar rather than an editable curve on purpose: it covers
    /// grow/shrink, which is the common case, and mirrors the
    /// `color_start`/`color_end` pair already here. A real curve editor is out of
    /// scope for this plan.
    pub size_end: f32,
    /// Velocity damping per second. `0` lets particles coast forever; higher
    /// values settle them, which is what makes smoke look like smoke rather than
    /// shrapnel.
    pub drag: f32,
    /// Softens each particle's own edge, `0.0`–`1.0`. `0` leaves a hard-edged
    /// quad; the default rounds it into a puff.
    pub fade_edge: f32,
    /// Fades particles out where they intersect scene geometry, removing the hard
    /// cut line where a plume meets the floor. Larger values fade over a longer
    /// range. `0` disables it.
    pub fade_scene: f32,
}

impl XrdsEffect {
    pub fn new() -> Self {
        Self {
            name: default_component_name::<Self>(),
            enabled: true,
            visible: true,
            transform: TransformParams::default(),
            kind: XrdsEffectKind::Burst,
            // True, so that simply spawning a default effect visibly does
            // something. An inert default would be a poor first experience for
            // the non-expert path this SDK targets; trigger-driven effects opt
            // out explicitly.
            auto_play: true,
            burst_count: 300,
            spawn_rate: 100.0,
            lifetime_secs: 1.5,
            size_min: 0.05,
            size_max: 0.15,
            // LDR on purpose — see the field docs. These are the values
            // verified rendering correctly on Quest 3 hardware.
            color_start: XrdsColor {
                rgba: [1.0, 0.85, 0.35, 1.0],
            },
            color_end: XrdsColor {
                rgba: [0.5, 0.08, 0.0, 0.0],
            },
            speed_min: 0.8,
            speed_max: 1.6,
            // Default kind is Burst, which reads best as an outward pop.
            // spread_deg is still given a usable value so switching the flag off
            // doesn't produce a degenerate zero-width jet.
            omnidirectional: true,
            spread_deg: 45.0,
            gravity: [0.0, -1.2, 0.0],
            emission_radius: 0.05,
            blend: XrdsEffectBlend::Blend,
            size_end: 1.0,
            // These three mirror bevy_firework's own ParticleSettings defaults
            // (0.2 / 0.7 / 1.0), which the spawner was already inheriting via
            // `..Default::default()`. Exposing them therefore changes nothing
            // visually until an author touches them.
            drag: 0.2,
            fade_edge: 0.7,
            fade_scene: 1.0,
        }
    }

    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    pub fn with_kind(mut self, kind: XrdsEffectKind) -> Self {
        self.kind = kind;
        self
    }
}

impl Default for XrdsEffect {
    fn default() -> Self {
        Self::new()
    }
}

impl XrdsObject for XrdsEffect {
    fn name(&self) -> &str {
        &self.name
    }

    fn is_enabled(&self) -> bool {
        self.enabled
    }

    fn is_visible(&self) -> bool {
        self.visible
    }
}

impl XrdsComponent for XrdsEffect {
    fn local_transform(&self) -> &TransformParams {
        &self.transform
    }

    fn local_transform_mut(&mut self) -> &mut TransformParams {
        &mut self.transform
    }
}

impl XrdsMutableComponent for XrdsEffect {
    fn set_name(&mut self, name: String) {
        self.name = name;
    }

    fn set_visible(&mut self, visible: bool) {
        self.visible = visible;
    }
}
