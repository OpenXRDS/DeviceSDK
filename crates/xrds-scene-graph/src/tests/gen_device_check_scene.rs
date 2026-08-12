use super::*;

/// On-device verification fixture. Not a real test — it only runs when
/// XRDS_GEN_DEVICE_SCENE points somewhere, so `cargo test` is unaffected.
///
/// Kept rather than deleted, but treat the *contents* as scratch: edit it to build
/// whatever the current device check needs. Hand-written `scene.json` drifts from
/// the schema, whereas this goes through the real `from_xrds_*` constructors and
/// `save_json`, so whatever it emits is a valid document by construction.
///
/// Driven by `docs/quest-device-test-recipe.md`, which carries the deploy sequence
/// and the traps.
///
/// Builds a scene answering the three open questions in one look:
///
/// 1. **Is `Add` blending actually visible on Adreno?** Two trails, identical
///    except for blend mode, side by side. "Hard to notice" was reported against a
///    single sparse trail, where Add and Blend genuinely do look alike — additive
///    only pays off where particles *overlap*, so both are tuned dense and slow.
/// 2. **Do the Phase 5a fidelity fields work on device?** The right-hand pair
///    exercises `size_end`, `drag` and `fade_scene` (the last is only judgeable
///    where particles meet the floor, so that one emits downward into it).
/// 3. **Does a Track fire and stop an effect on device?** A zone in front of the
///    player runs a Track: `PlayEffect` at 0s, `StopEffect` at 3s. Walking in
///    starts the plume; it should fade out rather than vanish.
///
/// Positions are relative to xrds-app's default spawn `(0, 1.6, 8)` looking down
/// -Z — not the origin. Authoring at z≈-1.5 puts things 9.5m behind the action,
/// which is exactly how the first device check ended up looking at nothing.
#[test]
fn xxx_gen_device_check_scene() {
    let Ok(out) = std::env::var("XRDS_GEN_DEVICE_SCENE") else {
        return;
    };

    let mut doc = XrdsSceneDocument::default();
    let mut next = 1u64;
    let mut id = || {
        let v = XrdsSceneNodeId(next);
        next += 1;
        v
    };

    let mut ground = XrdsPlane3D::new().with_name("Ground");
    ground.size = [24.0, 24.0];
    ground.transform.translation = [0.0, 0.0, 4.0];
    doc.nodes
        .push(XrdsSceneNode::from_xrds_plane3d(id(), None, &ground, None));

    let mut sun = XrdsDirectionalLight::new().with_name("Sun");
    sun.transform.translation = [2.0, 5.0, 6.0];
    doc.nodes
        .push(XrdsSceneNode::from_xrds_directional_light(id(), None, &sun));

    // --- 1. Blend vs Add, identical otherwise -------------------------------
    // Dense, large, slow: particles pile up near the emitter so overlap is heavy,
    // which is the only condition under which additive differs visibly.
    // Three modes side by side, not two. If all three look identical the blend
    // field is not reaching the material at all; if Multiply visibly darkens but
    // Add does not, that is a much more specific fact than "Add looks the same".
    //
    // Brighter and denser than the previous attempt, which was dimmed so far that
    // Blend and Add converged. Additive only separates from Blend where particles
    // pile up *and* there is enough signal to accumulate.
    let dense = |name: &str, x: f32, blend: XrdsEffectBlend| {
        let mut fx = XrdsEffect::new()
            .with_name(name)
            .with_kind(XrdsEffectKind::Trail);
        fx.transform.translation = [x, 1.3, 6.3];
        fx.blend = blend;
        fx.spawn_rate = 500.0;
        fx.lifetime_secs = 1.1;
        fx.size_min = 0.14;
        fx.size_max = 0.28;
        fx.speed_min = 0.05;
        fx.speed_max = 0.30;
        fx.omnidirectional = true;
        fx.gravity = [0.0, 0.10, 0.0];
        fx.emission_radius = 0.13;
        // Alpha 1.0: with Add, a low alpha scales the contribution down and was
        // part of why the two modes converged before.
        fx.color_start = XrdsColor { rgba: [0.60, 0.34, 0.08, 1.0] };
        fx.color_end = XrdsColor { rgba: [0.35, 0.12, 0.02, 0.0] };
        fx
    };
    doc.nodes.push(XrdsSceneNode::from_xrds_effect(
        id(),
        None,
        &dense("A_BLEND", -1.6, XrdsEffectBlend::Blend),
    ));
    doc.nodes.push(XrdsSceneNode::from_xrds_effect(
        id(),
        None,
        &dense("B_ADD", 0.0, XrdsEffectBlend::Add),
    ));
    doc.nodes.push(XrdsSceneNode::from_xrds_effect(
        id(),
        None,
        &dense("C_MULTIPLY", 1.6, XrdsEffectBlend::Multiply),
    ));

    // --- 2. Fidelity fields -------------------------------------------------
    // Emits *downward* so particles intersect the floor: fade_scene is only
    // judgeable at that intersection.
    let mut fade = XrdsEffect::new()
        .with_name("D_FADE_FLOOR")
        .with_kind(XrdsEffectKind::Trail);
    fade.transform.translation = [2.6, 0.6, 6.0];
    fade.spawn_rate = 200.0;
    fade.lifetime_secs = 1.4;
    fade.size_min = 0.18;
    fade.size_max = 0.30;
    fade.omnidirectional = false;
    fade.spread_deg = 35.0;
    fade.speed_min = 0.3;
    fade.speed_max = 0.7;
    fade.gravity = [0.0, -1.6, 0.0];
    fade.fade_scene = 3.0;
    fade.size_end = 1.8;
    fade.drag = 1.2;
    fade.color_start = XrdsColor { rgba: [0.55, 0.65, 0.80, 0.8] };
    fade.color_end = XrdsColor { rgba: [0.20, 0.25, 0.35, 0.0] };
    doc.nodes
        .push(XrdsSceneNode::from_xrds_effect(id(), None, &fade));

    // --- 3. Track-driven fire and stop --------------------------------------
    let mut triggered = XrdsEffect::new()
        .with_name("E_TRIGGERED")
        .with_kind(XrdsEffectKind::Trail);
    // Directly above the grab cube, so cause and effect are unmistakable.
    triggered.transform.translation = [0.0, 2.0, 7.2];
    // auto_play off: nothing until the Track fires it. This is the whole point —
    // if it is already running, the test proves nothing.
    triggered.auto_play = false;
    triggered.spawn_rate = 250.0;
    triggered.lifetime_secs = 1.2;
    triggered.size_min = 0.08;
    triggered.size_max = 0.16;
    triggered.omnidirectional = false;
    triggered.spread_deg = 25.0;
    triggered.gravity = [0.0, 0.8, 0.0];
    triggered.blend = XrdsEffectBlend::Add;
    triggered.color_start = XrdsColor { rgba: [0.20, 0.70, 0.45, 1.0] };
    triggered.color_end = XrdsColor { rgba: [0.05, 0.25, 0.15, 0.0] };
    let triggered_id = id();
    doc.nodes.push(XrdsSceneNode::from_xrds_effect(
        triggered_id,
        None,
        &triggered,
    ));

    doc.tracks = vec![XrdsNamedTrack {
        name: "fire_and_stop".to_string(),
        track: XrdsTrack {
            assets: vec![XrdsTrackAsset {
                target: XrdsActionTarget::Node(triggered_id),
                keys: vec![
                    XrdsTrackKey { at_secs: 0.0, action: XrdsAction::PlayEffect { count: None } },
                    XrdsTrackKey { at_secs: 3.0, action: XrdsAction::StopEffect },
                ],
                // Restore would soft-stop it at completion anyway; Keep makes the
                // authored StopEffect at 3s the thing actually being observed.
                when_finished: XrdsWhenFinished::Keep,
            }],
            ..Default::default()
        },
    }];

    // A zone just in front of the spawn, so a single step forward enters it.
    let mut zone = XrdsSceneNode::from_xrds_node(
        id(),
        None,
        &xrds_components::world::XrdsNode {
            name: "G_ZONE_ON_PAD".to_string(),
            enabled: true,
            visible: true,
            transform: Default::default(),
        },
    );
    // Trigger: GRAB a cube, not walk into a zone.
    //
    // ZoneEnter cannot be fired by the player at all. `zone_collision_system`
    // consumes avian3d `CollisionStart`/`CollisionEnd`, so both bodies need
    // colliders — the zone gets `Collider + Sensor + CollisionEventsEnabled`
    // (spawn.rs), but nothing ever gives the player camera or player root a
    // collider. So walking into a zone produces no event, which is why the first
    // two attempts here saw nothing. Logged as an SDK gap; using a grab in the
    // meantime because a cube has a mesh, hence an Aabb, hence grab reach.
    let mut handle_cube = XrdsCube::new().with_name("F_GRAB_ME_CUBE");
    handle_cube.size = [0.22, 0.22, 0.22];
    // Waist-to-chest height, an arm's length ahead of the spawn: reachable
    // without walking, and locomotion works now if it is not.
    handle_cube.transform.translation = [0.0, 1.15, 7.2];
    let mut cube_mat = xrds_components::XrdsMaterialParams::default();
    cube_mat.base_color = XrdsColor { rgba: [0.15, 0.85, 0.35, 1.0] };
    cube_mat.unlit = true;
    let cube_id = id();
    let mut cube_node =
        XrdsSceneNode::from_xrds_cube(cube_id, None, &handle_cube, Some(cube_mat));
    // Grab needs arming per-node; without this the cube is inert scenery.
    cube_node.grabbable = true;
    cube_node.triggers = vec![XrdsTriggerBinding {
        trigger: XrdsTriggerKind::Grabbed,
        track: Some("fire_and_stop".to_string()),
        ..Default::default()
    }];
    doc.nodes.push(cube_node);

    doc.save_json(std::path::Path::new(&out))
        .expect("device-check scene should save");
    println!("[gen] wrote {out} with {} nodes", doc.nodes.len());
}
