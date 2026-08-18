use super::*;
use bevy_firework::core::{EmissionPacing, ParticleSpawner};
use xrds_components::primitives::{XrdsEffect, XrdsEffectKind};
use xrds_components::EffectParams;

/// Inspects the world directly rather than round-tripping a scene document:
/// `XrdsEffect` has no `XrdsSceneNodePayload` variant until Phase 2, so the
/// export-based assertions the other primitives use aren't available yet.
///
/// Note this test also silently covers the `#[cfg(not(test))]` gate on
/// `ParticleSystemPlugin` — `bevy_firework`'s plugin calls
/// `sub_app_mut(RenderApp)`, which would panic in this headless harness, so if
/// that gate is ever dropped every test in this file starts failing.
#[test]
fn effect_spawns_a_particle_spawner_and_updates_it_in_place() {
    let mut app = xrds_test_app();

    let effect_id = {
        let mut xrds = XrdsAPI::attach(&mut app);
        let effect = xrds.spawn(&XrdsEffect::new().with_name("Sparks"));
        xrds.id_of(&effect).expect("effect should have an id")
    };

    app.update();

    let entity = {
        let xrds = XrdsAPI::attach(&mut app);
        xrds.entity_of_id(effect_id)
            .expect("effect id should resolve to an entity")
    };

    // Spawn produced a backend spawner carrying the descriptor's defaults.
    {
        let spawner = app
            .world()
            .get::<ParticleSpawner>(entity)
            .expect("spawn should have inserted a ParticleSpawner");
        let particles = &spawner.particle_settings[0];
        let emission = &spawner.emission_settings[0];

        assert_eq!(particles.lifetime.min, 1.5, "default lifetime");
        assert_eq!(particles.initial_scale.min, 0.05);
        assert_eq!(particles.initial_scale.max, 0.15);
        // Burst is the default kind, so pacing must be one-shot from
        // burst_count -- not the spawn_rate value, which Burst must ignore.
        assert!(
            matches!(emission.emission_pacing, EmissionPacing::OneShot(300)),
            "default Burst should map to OneShot(burst_count=300), got {:?}",
            emission.emission_pacing
        );
    }

    // Switch kind and retune.
    {
        let mut xrds = XrdsAPI::attach(&mut app);
        let handle: Handle<XrdsEffect> = entity.into();
        xrds.set_effect_params(
            &handle,
            EffectParams {
                kind: XrdsEffectKind::Trail,
                auto_play: true,
                // Trail must read spawn_rate and ignore burst_count entirely;
                // the deliberately different values here prove which one is used.
                burst_count: 999,
                spawn_rate: 42.0,
                lifetime_secs: 3.0,
                size_min: 0.2,
                size_max: 0.4,
                // Deliberately out of range: must be clamped to 1.0, not passed
                // through as an HDR value that would render flat white on the
                // SDK's non-HDR XR cameras.
                color_start: XrdsColor {
                    rgba: [30.0, 0.5, 0.25, 1.0],
                },
                color_end: XrdsColor {
                    rgba: [0.1, 0.0, 0.0, 0.0],
                },
                speed_min: 2.0,
                speed_max: 4.0,
                omnidirectional: false,
                spread_deg: 15.0,
                gravity: [0.0, -9.81, 0.0],
                emission_radius: 0.5,
                blend: xrds_components::primitives::XrdsEffectBlend::Add,
                size_end: 0.25,
                drag: 1.5,
                fade_edge: 0.3,
                fade_scene: 2.0,
            },
        );
    }

    app.update();

    // The stored descriptor is the source of truth for later export/clone.
    {
        let stored = app
            .world()
            .get::<XrdsStored<XrdsEffect>>(entity)
            .expect("descriptor should still be stored");
        assert_eq!(stored.0.kind, XrdsEffectKind::Trail);
        assert_eq!(stored.0.spawn_rate, 42.0);
        assert_eq!(stored.0.burst_count, 999);
        assert_eq!(stored.0.lifetime_secs, 3.0);
        assert_eq!(stored.0.spread_deg, 15.0);
    }

    // ...and the backend spawner was rebuilt from it, in place.
    {
        let spawner = app
            .world()
            .get::<ParticleSpawner>(entity)
            .expect("spawner should survive the update");
        let particles = &spawner.particle_settings[0];
        let emission = &spawner.emission_settings[0];

        assert_eq!(particles.lifetime.min, 3.0, "lifetime should have updated");
        assert_eq!(particles.initial_scale.min, 0.2);
        assert_eq!(particles.acceleration.y, -9.81);
        assert!(
            !matches!(emission.emission_pacing, EmissionPacing::OneShot(_)),
            "Trail must not use one-shot pacing, got {:?}",
            emission.emission_pacing
        );

        let start = particles.base_color.sample(0.0).expect("gradient start");
        assert_eq!(start.red, 1.0, "out-of-range red should clamp to 1.0");
        assert_eq!(start.green, 0.5, "in-range channels pass through");

        // omnidirectional=false means a directional cone, so velocity is
        // directional and the radial channel stays zero.
        assert_eq!(emission.initial_velocity_radial.min, 0.0);
        assert!((emission.initial_velocity.spread - 15f32.to_radians()).abs() < 1e-6);

        // Phase 5a fidelity fields reach the backend.
        assert_eq!(particles.blend_mode, bevy_firework::core::BlendMode::Add);
        assert_eq!(particles.linear_drag, 1.5);
        assert_eq!(particles.fade_edge, 0.3);
        assert_eq!(particles.fade_scene, 2.0);
        // size_end drives the tail of the scale curve; spawn stays at 1.0x.
        assert_eq!(particles.scale_curve.sample(0.0), Some(1.0));
        assert_eq!(particles.scale_curve.sample(1.0), Some(0.25));
    }
}

/// `omnidirectional` selects a radial push from the emission point instead of a
/// cone around +Y, and must ignore `spread_deg` entirely. Worth its own test
/// because getting it wrong yields a one-sided hemisphere that still looks
/// plausible in a screenshot.
#[test]
fn omnidirectional_spread_uses_radial_velocity() {
    let mut app = xrds_test_app();

    let effect_id = {
        let mut xrds = XrdsAPI::attach(&mut app);
        let mut descriptor = XrdsEffect::new();
        descriptor.omnidirectional = true;
        // Left at a cone-shaped value on purpose: it must be ignored, not
        // combined with the radial path.
        descriptor.spread_deg = 30.0;
        descriptor.speed_min = 1.0;
        descriptor.speed_max = 2.0;
        let effect = xrds.spawn(&descriptor);
        xrds.id_of(&effect).expect("effect should have an id")
    };

    app.update();

    let entity = {
        let xrds = XrdsAPI::attach(&mut app);
        xrds.entity_of_id(effect_id).expect("entity")
    };

    let spawner = app
        .world()
        .get::<ParticleSpawner>(entity)
        .expect("ParticleSpawner");
    let emission = &spawner.emission_settings[0];

    assert_eq!(emission.initial_velocity_radial.min, 1.0);
    assert_eq!(emission.initial_velocity_radial.max, 2.0);
    assert_eq!(
        emission.initial_velocity.magnitude.max, 0.0,
        "directional velocity should be zero when emitting radially"
    );
}

/// `auto_play: false` is what makes a `Burst` re-fireable by a trigger
/// (`XrdsAction::PlayEffect`, Phase 4). Without it the backend's one-shot pacing
/// disables its own emission the instant it fires, so an authored burst would go
/// off once at scene load and never again.
#[test]
fn a_burst_that_does_not_auto_play_waits_on_demand() {
    let mut app = xrds_test_app();

    let (auto_id, idle_id) = {
        let mut xrds = XrdsAPI::attach(&mut app);

        let auto = xrds.spawn(&XrdsEffect::new().with_name("AutoBurst"));

        let mut idle_descriptor = XrdsEffect::new().with_name("IdleBurst");
        idle_descriptor.auto_play = false;
        let idle = xrds.spawn(&idle_descriptor);

        (
            xrds.id_of(&auto).expect("auto id"),
            xrds.id_of(&idle).expect("idle id"),
        )
    };

    app.update();

    let (auto_entity, idle_entity) = {
        let xrds = XrdsAPI::attach(&mut app);
        (
            xrds.entity_of_id(auto_id).expect("auto entity"),
            xrds.entity_of_id(idle_id).expect("idle entity"),
        )
    };

    let pacing_of = |app: &App, entity| {
        app.world()
            .get::<ParticleSpawner>(entity)
            .expect("ParticleSpawner")
            .emission_settings[0]
            .emission_pacing
            .clone()
    };

    assert!(
        matches!(pacing_of(&app, auto_entity), EmissionPacing::OneShot(_)),
        "auto_play burst should fire immediately via OneShot"
    );
    assert!(
        matches!(pacing_of(&app, idle_entity), EmissionPacing::OnDemand),
        "non-auto_play burst should wait for an explicit fire"
    );

    // Must stay enabled: bevy_firework's emission loop returns early on
    // `!enabled` before it ever reads the queued count, so an OnDemand spawner
    // that started disabled would silently ignore every fire request.
    let idle_spawner = app
        .world()
        .get::<ParticleSpawner>(idle_entity)
        .expect("ParticleSpawner");
    assert!(
        idle_spawner.starts_enabled,
        "an on-demand burst must start enabled or queued particles never emit"
    );
}

/// A `Trail` uses the opposite mechanism: pacing stays rate-based and
/// `starts_enabled` is what idles it.
#[test]
fn a_trail_that_does_not_auto_play_starts_disabled() {
    let mut app = xrds_test_app();

    let effect_id = {
        let mut xrds = XrdsAPI::attach(&mut app);
        let mut descriptor = XrdsEffect::new()
            .with_name("IdlePlume")
            .with_kind(XrdsEffectKind::Trail);
        descriptor.auto_play = false;
        descriptor.spawn_rate = 50.0;
        let effect = xrds.spawn(&descriptor);
        xrds.id_of(&effect).expect("id")
    };

    app.update();

    let entity = {
        let xrds = XrdsAPI::attach(&mut app);
        xrds.entity_of_id(effect_id).expect("entity")
    };

    let spawner = app
        .world()
        .get::<ParticleSpawner>(entity)
        .expect("ParticleSpawner");

    assert!(
        !spawner.starts_enabled,
        "an idle trail should not begin emitting"
    );
    assert!(
        !matches!(
            spawner.emission_settings[0].emission_pacing,
            EmissionPacing::OnDemand
        ),
        "a trail keeps rate-based pacing; only its enabled flag changes"
    );
}

/// Guards the one integration point the compiler cannot check: the
/// `XrdsStored<XrdsEffect>` branch in `helper.rs`. Omitting it silently exports
/// the node as `Empty`, so the effect vanishes from a saved scene with no error
/// anywhere — the exact class of bug that has bitten panels in this codebase
/// before. Also covers the document -> runtime direction by reimporting.
#[test]
fn effect_survives_a_document_round_trip() {
    let mut app = xrds_test_app();

    let effect_id = {
        let mut xrds = XrdsAPI::attach(&mut app);
        let mut descriptor = XrdsEffect::new()
            .with_name("RoundTrip")
            .with_kind(XrdsEffectKind::Trail);
        descriptor.auto_play = false;
        descriptor.burst_count = 77;
        descriptor.spawn_rate = 250.0;
        descriptor.lifetime_secs = 4.25;
        descriptor.size_min = 0.11;
        descriptor.size_max = 0.22;
        descriptor.color_start = XrdsColor { rgba: [0.9, 0.8, 0.7, 1.0] };
        descriptor.color_end = XrdsColor { rgba: [0.1, 0.2, 0.3, 0.0] };
        descriptor.speed_min = 1.25;
        descriptor.speed_max = 3.75;
        descriptor.omnidirectional = false;
        descriptor.spread_deg = 33.0;
        descriptor.gravity = [0.5, -2.5, 1.5];
        descriptor.emission_radius = 0.42;
        let effect = xrds.spawn(&descriptor);
        xrds.id_of(&effect).expect("id")
    };

    app.update();

    let exported = {
        let xrds = XrdsAPI::attach(&mut app);
        xrds.export_scene_document().expect("export should succeed")
    };

    // Round-trip through an actual file with the same API a user calls, so this
    // covers the serde attributes too, not just the in-memory conversion.
    let unique = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_nanos();
    let path = std::env::temp_dir().join(format!("xrds-effect-roundtrip-{unique}.json"));
    exported
        .save_json(&path)
        .expect("document should save to json");

    let reparsed = xrds_scene_graph::XrdsSceneDocument::load_json(&path)
        .expect("saved document should load back");

    let node = reparsed
        .node(xrds_scene_graph::XrdsSceneNodeId(effect_id.0))
        .expect("effect node should be present after a round trip");
    assert_eq!(node.name, "RoundTrip");

    let xrds_scene_graph::XrdsSceneNodePayload::Effect(effect) = &node.payload else {
        panic!(
            "expected an Effect payload, got {:?} -- if this is Empty, the              XrdsStored<XrdsEffect> branch in helper.rs is missing",
            node.payload
        );
    };

    assert_eq!(effect.kind, xrds_scene_graph::XrdsSceneEffectKind::Trail);
    assert!(!effect.auto_play);
    assert_eq!(effect.burst_count, 77);
    assert_eq!(effect.spawn_rate, 250.0);
    assert_eq!(effect.lifetime_secs, 4.25);
    assert_eq!(effect.size_min, 0.11);
    assert_eq!(effect.size_max, 0.22);
    assert_eq!(effect.color_start, [0.9, 0.8, 0.7, 1.0]);
    assert_eq!(effect.color_end, [0.1, 0.2, 0.3, 0.0]);
    assert_eq!(effect.speed_min, 1.25);
    assert_eq!(effect.speed_max, 3.75);
    assert!(!effect.omnidirectional);
    assert_eq!(effect.spread_deg, 33.0);
    assert_eq!(effect.gravity, [0.5, -2.5, 1.5]);
    assert_eq!(effect.emission_radius, 0.42);

    // Document -> runtime: importing the reloaded document must rebuild a
    // working spawner, with auto_play=false still meaning on-demand.
    let mut fresh = xrds_test_app();
    {
        let mut xrds = XrdsAPI::attach(&mut fresh);
        xrds.import_scene_document_json(&path)
            .expect("saved scene document should load and import");
    }
    fresh.update();

    let imported_entity = {
        let xrds = XrdsAPI::attach(&mut fresh);
        xrds.entity_of_id(effect_id).expect("imported entity")
    };
    let spawner = fresh
        .world()
        .get::<ParticleSpawner>(imported_entity)
        .expect("import should have produced a ParticleSpawner");
    // This descriptor is a Trail, so auto_play=false idles it via
    // `starts_enabled` and pacing stays rate-based -- OnDemand is the Burst
    // path. Asserting the Trail half of the mapping table here.
    assert!(
        !spawner.starts_enabled,
        "auto_play=false should survive the round trip as an idle trail"
    );
    assert!(
        !matches!(
            spawner.emission_settings[0].emission_pacing,
            EmissionPacing::OnDemand
        ),
        "a Trail keeps rate-based pacing regardless of auto_play"
    );
    assert_eq!(spawner.particle_settings[0].lifetime.min, 4.25);
    assert!((spawner.emission_settings[0].initial_velocity.spread - 33f32.to_radians()).abs() < 1e-6);

    let _ = std::fs::remove_file(&path);
}

/// Regression test for the first cut of `PlayEffect`, which silently did nothing
/// for two of the four authorable combinations.
///
/// `queue_particles` only adds to `manual_queued_count`, and bevy_firework only
/// drains that under `EmissionPacing::OnDemand`. So a `Trail` (rate pacing) never
/// read the queue at all, and a `Burst` with `auto_play: true` (one-shot pacing,
/// self-disabling at load) ignored it too. Both looked like "the Sequencer isn't
/// running my action".
///
/// All four combinations must now report success.
#[test]
fn play_effect_fires_every_authorable_combination() {
    use xrds_runtime_fire_helper::fire;

    for (kind, auto_play, label) in [
        (XrdsEffectKind::Burst, false, "burst/idle"),
        (XrdsEffectKind::Burst, true, "burst/auto"),
        (XrdsEffectKind::Trail, false, "trail/idle"),
        (XrdsEffectKind::Trail, true, "trail/auto"),
    ] {
        let mut app = xrds_test_app();
        let id = {
            let mut xrds = XrdsAPI::attach(&mut app);
            let mut descriptor = XrdsEffect::new().with_name(label).with_kind(kind);
            descriptor.auto_play = auto_play;
            let handle = xrds.spawn(&descriptor);
            xrds.id_of(&handle).expect("id")
        };
        app.update();
        let entity = {
            let xrds = XrdsAPI::attach(&mut app);
            xrds.entity_of_id(id).expect("entity")
        };

        assert!(
            fire(app.world_mut(), entity, None),
            "PlayEffect should fire {label}"
        );

        // A Burst must end up on OnDemand pacing with a pending queue; a Trail
        // must end up enabled. Either way the effect is now live.
        let spawner = app
            .world()
            .get::<ParticleSpawner>(entity)
            .expect("spawner should still exist");
        match kind {
            XrdsEffectKind::Burst => {
                assert!(
                    matches!(
                        spawner.emission_settings[0].emission_pacing,
                        EmissionPacing::OnDemand
                    ),
                    "{label}: firing a burst must leave it on OnDemand pacing so the \
                     queued count is actually drained"
                );
                let data = app
                    .world()
                    .get::<bevy_firework::core::ParticleSpawnerData>(entity)
                    .expect("spawner data");
                assert_eq!(
                    data.manual_queued_count, 300,
                    "{label}: the authored burst_count should be queued"
                );
            }
            XrdsEffectKind::Trail => {
                assert!(
                    spawner.starts_enabled,
                    "{label}: firing a trail must enable its emission"
                );
            }
        }
    }
}

/// An explicit `count` overrides the authored `burst_count`, so one effect node
/// can be fired at different intensities from different triggers.
#[test]
fn play_effect_count_overrides_the_authored_burst_count() {
    use xrds_runtime_fire_helper::fire;

    let mut app = xrds_test_app();
    let id = {
        let mut xrds = XrdsAPI::attach(&mut app);
        let mut descriptor = XrdsEffect::new().with_name("Override");
        descriptor.auto_play = false;
        descriptor.burst_count = 10;
        let handle = xrds.spawn(&descriptor);
        xrds.id_of(&handle).expect("id")
    };
    app.update();
    let entity = {
        let xrds = XrdsAPI::attach(&mut app);
        xrds.entity_of_id(id).expect("entity")
    };

    assert!(fire(app.world_mut(), entity, Some(777)));
    let data = app
        .world()
        .get::<bevy_firework::core::ParticleSpawnerData>(entity)
        .expect("spawner data");
    assert_eq!(data.manual_queued_count, 777);
}

/// Firing something that is not an effect must fail rather than panic — the
/// runtime cannot rely on the authoring-time diagnostic, because
/// `SelfNode`/`TriggerSource` targets only resolve when the trigger fires.
#[test]
fn play_effect_on_a_non_effect_node_reports_failure() {
    use xrds_runtime_fire_helper::fire;

    let mut app = xrds_test_app();
    let id = {
        let mut xrds = XrdsAPI::attach(&mut app);
        let handle = xrds.spawn(&XrdsCube::new().with_name("NotAnEffect"));
        xrds.id_of(&handle).expect("id")
    };
    app.update();
    let entity = {
        let xrds = XrdsAPI::attach(&mut app);
        xrds.entity_of_id(id).expect("entity")
    };

    assert!(!fire(app.world_mut(), entity, None));
}

/// `fire_effect_in_world` is `pub(super)` on the api module; this re-exposes it
/// for the tests without widening its real visibility.
mod xrds_runtime_fire_helper {
    pub fn stop(world: &mut bevy::prelude::World, target: bevy::prelude::Entity) -> bool {
        crate::xrds_api::stop_effect_in_world(world, target)
    }

    pub fn fire(
        world: &mut bevy::prelude::World,
        target: bevy::prelude::Entity,
        count: Option<u32>,
    ) -> bool {
        crate::xrds_api::fire_effect_in_world(world, target, count)
    }
}

/// `StopEffect` must be a *soft* stop, and the mechanism is what guarantees it:
/// it clears `ParticleSpawnerData::emission` and must **never** touch
/// `ParticleSpawner`. Any change to the spawner triggers bevy_firework's
/// `sync_spawner_data`, which resets `ParticleSpawnerData::particles` — a hard
/// kill that yanks live particles out of the air instead of letting them fade.
///
/// **What this test cannot check:** `ParticleSpawnerData::active()` going false,
/// or particles actually ageing out. `ParticleSystemPlugin` is `#[cfg(not(test))]`
/// (it needs the RenderApp), so bevy_firework's own systems never run here and
/// `emission` stays empty regardless. `EmissionData` has private fields and no
/// constructor, so a populated emission cannot be faked either. The visible
/// behaviour needs a desktop/on-device check; what is guarded here is the
/// invariant that makes it soft rather than hard.
#[test]
fn stop_effect_leaves_the_spawner_untouched_so_live_particles_can_fade() {
    use xrds_runtime_fire_helper::stop;

    let mut app = xrds_test_app();
    let id = {
        let mut xrds = XrdsAPI::attach(&mut app);
        let mut descriptor = XrdsEffect::new()
            .with_name("Plume")
            .with_kind(XrdsEffectKind::Trail);
        descriptor.auto_play = false;
        descriptor.spawn_rate = 120.0;
        let handle = xrds.spawn(&descriptor);
        xrds.id_of(&handle).expect("id")
    };
    app.update();
    let entity = {
        let xrds = XrdsAPI::attach(&mut app);
        xrds.entity_of_id(id).expect("entity")
    };

    let before = app
        .world()
        .get::<ParticleSpawner>(entity)
        .expect("spawner")
        .clone();

    assert!(stop(app.world_mut(), entity), "StopEffect should report success");

    let after = app.world().get::<ParticleSpawner>(entity).expect("spawner");
    assert_eq!(
        after.emission_settings.len(),
        before.emission_settings.len(),
        "StopEffect must not rewrite ParticleSpawner — that would wipe live particles"
    );
    assert_eq!(
        after.starts_enabled, before.starts_enabled,
        "StopEffect must not rewrite ParticleSpawner"
    );
    assert!(
        app.world()
            .get::<bevy_firework::core::ParticleSpawnerData>(entity)
            .expect("spawner data")
            .emission
            .is_empty(),
        "emission must be cleared, which is what makes active() false at runtime"
    );

    // Firing twice, stopping twice: a Track may legitimately do either, and
    // neither should be reported as a failure.
    assert!(stop(app.world_mut(), entity), "a second StopEffect is a no-op, not a failure");
}

/// Stopping something that is not an effect must fail rather than panic — the
/// authoring diagnostic cannot cover `SelfNode`/`TriggerSource` targets.
#[test]
fn stop_effect_on_a_non_effect_node_reports_failure() {
    use xrds_runtime_fire_helper::stop;

    let mut app = xrds_test_app();
    let id = {
        let mut xrds = XrdsAPI::attach(&mut app);
        let handle = xrds.spawn(&XrdsCube::new().with_name("NotAnEffect"));
        xrds.id_of(&handle).expect("id")
    };
    app.update();
    let entity = {
        let xrds = XrdsAPI::attach(&mut app);
        xrds.entity_of_id(id).expect("entity")
    };

    assert!(!stop(app.world_mut(), entity));
}

/// A stopped Burst must still be re-fireable. Its pacing is already `OnDemand`,
/// so a naive "only rebuild when the pacing is wrong" check would skip the
/// rebuild and queue into a spawner whose emission was cleared, where nothing
/// would ever drain it.
#[test]
fn a_stopped_burst_can_be_fired_again() {
    use xrds_runtime_fire_helper::{fire, stop};

    let mut app = xrds_test_app();
    let id = {
        let mut xrds = XrdsAPI::attach(&mut app);
        let mut descriptor = XrdsEffect::new().with_name("Spark");
        descriptor.auto_play = false;
        let handle = xrds.spawn(&descriptor);
        xrds.id_of(&handle).expect("id")
    };
    app.update();
    let entity = {
        let xrds = XrdsAPI::attach(&mut app);
        xrds.entity_of_id(id).expect("entity")
    };

    assert!(stop(app.world_mut(), entity));
    assert!(
        fire(app.world_mut(), entity, Some(50)),
        "a stopped burst should still fire"
    );

    let data = app
        .world()
        .get::<bevy_firework::core::ParticleSpawnerData>(entity)
        .expect("spawner data");
    assert_eq!(
        data.manual_queued_count, 50,
        "the requested count must be queued after a stop/re-fire cycle"
    );
}
