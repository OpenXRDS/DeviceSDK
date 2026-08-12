//! Particle effects via `XrdsEffect` — the two emission kinds side by side.
//!
//! Run with: `cargo run --example particle_effect`
//!
//! What to look for: a continuously emitting orange plume orbiting the centre
//! (`Trail`), and a one-off spray that fires once at startup (`Burst`).
//!
//! Two things worth knowing while reading this:
//!
//! - **Colours are deliberately ≤ 1.0.** Values above 1.0 are an HDR/bloom
//!   idiom; the SDK's XR cameras have no HDR pass, so brighter values clamp and
//!   every particle renders flat white on a headset. `XrdsEffect` clamps for you,
//!   but authoring in range keeps what you write and what you see identical.
//! - **`Burst` fires once, when it spawns.** Re-firing on demand is what
//!   `XrdsAction::PlayEffect` is for, which arrives with the trigger/Track
//!   integration — see `docs/done/vfx-particle-effects-plan.md`. Until then a
//!   repeating pulse is better expressed as a `Trail`.

use xrds::sdk::{
    primitives::{XrdsEffect, XrdsEffectKind, XrdsPlane3D},
    world::XrdsCamera,
    XrdsColor,
};
use xrds::{Handle, Runtime, RuntimeParameters, XrdsAPI, XrdsApp, XrdsUpdateContext};

#[derive(Default)]
struct Handler {
    plume: Option<Handle<XrdsEffect>>,
}

pub fn main() {
    let runtime = Runtime::new(RuntimeParameters {
        app_name: "ParticleEffect".to_owned(),
        ..Default::default()
    });
    runtime
        .run_xrds(Handler::default())
        .expect("Could not run application");
}

impl XrdsApp for Handler {
    fn setup(&mut self, api: &mut XrdsAPI<'_>) {
        self.plume = Some(setup(api));
    }

    fn update(&mut self, ctx: &mut XrdsUpdateContext<'_>) {
        // Orbit the trail so it is obvious the particles are emitted in world
        // space and left behind, rather than rigidly following the emitter.
        let Some(plume) = &self.plume else {
            return;
        };
        let t = ctx.elapsed_secs();
        ctx.set_translation(plume, [t.cos() * 1.5, 0.2, t.sin() * 1.5]);
    }
}

fn setup(api: &mut XrdsAPI<'_>) -> Handle<XrdsEffect> {
    api.spawn(&{
        let mut camera = XrdsCamera::new()
            .with_name("ParticleCamera")
            .looking_at([0.0, 0.5, 0.0]);
        camera.transform.translation = [0.0, 2.0, 5.0];
        camera
    });

    // Ground plane purely for depth reference — particles are much easier to
    // judge against a surface than floating in a void.
    api.spawn(&{
        let mut plane = XrdsPlane3D::new().with_name("Ground");
        plane.size = [12.0, 12.0];
        plane
    });

    // Trail: continuous emission, paced by `spawn_rate`. Live particle count
    // settles around spawn_rate * lifetime_secs (~150 here).
    let plume = api.spawn(&{
        let mut effect = XrdsEffect::new()
            .with_name("Plume")
            .with_kind(XrdsEffectKind::Trail);
        effect.spawn_rate = 100.0;
        effect.lifetime_secs = 1.5;
        effect.size_min = 0.04;
        effect.size_max = 0.10;
        effect.speed_min = 0.6;
        effect.speed_max = 1.2;
        // A narrow cone about local +Y: a plume, not a sphere.
        effect.omnidirectional = false;
        effect.spread_deg = 20.0;
        // Slight upward drift, like hot smoke.
        effect.gravity = [0.0, 0.6, 0.0];
        effect.emission_radius = 0.05;
        effect.color_start = XrdsColor {
            rgba: [1.0, 0.75, 0.30, 1.0],
        };
        effect.color_end = XrdsColor {
            rgba: [0.35, 0.10, 0.05, 0.0],
        };
        effect
    });

    // Burst: one-shot, sized by `burst_count`. `spawn_rate` is ignored here.
    api.spawn(&{
        let mut effect = XrdsEffect::new()
            .with_name("Burst")
            .with_kind(XrdsEffectKind::Burst);
        effect.transform.translation = [0.0, 1.0, 0.0];
        effect.burst_count = 400;
        effect.lifetime_secs = 2.0;
        // Outward in every direction, falling under gravity.
        effect.omnidirectional = true;
        effect.speed_min = 1.5;
        effect.speed_max = 3.0;
        effect.gravity = [0.0, -4.0, 0.0];
        effect.color_start = XrdsColor {
            rgba: [0.55, 0.85, 1.0, 1.0],
        };
        effect.color_end = XrdsColor {
            rgba: [0.10, 0.25, 0.60, 0.0],
        };
        effect
    });

    plume
}
