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
/// # Current check: can the player walk into a zone?
///
/// This is the whole point of `docs/player-body-collider-plan.md`. Before that work
/// the answer was no — `zone_collision_system` needs colliders on *both* bodies and
/// nothing ever gave the player one, so a correctly authored zone produced zero
/// events. Two earlier device attempts were spent assuming the volume had been
/// missed before checking whether the event could fire at all.
///
/// So the scene is deliberately minimal and the zone is deliberately **visible**:
///
/// - `PAD` — a flat, bright, unlit slab marking exactly where the zone is. An
///   unmarked trigger volume is indistinguishable from a broken one (recipe Trap 6).
/// - `ZONE` — a box sensor covering the pad, generous in Y so it cannot be stepped
///   over or under.
/// - `PLUME` — `auto_play = false`, directly above the pad. If it is already
///   running the test proves nothing.
/// - Track `on_enter`, bound to `ZoneEnter` on the zone: `PlayEffect` at 0s.
///   `when_finished: Keep` so the plume stays up and the result is unambiguous
///   rather than being cleaned up a frame later.
///
/// Positions are relative to xrds-app's default spawn `(0, 1.6, 8)` looking down
/// -Z — not the origin. The pad sits at z = 6.5, a step and a half ahead, close
/// enough to find without locomotion but far enough that the player does not start
/// inside it (starting inside would mean no *enter* transition at all).
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

    // --- The visible marker -------------------------------------------------
    // Unlit so it reads as a marker rather than as lit geometry, and thin enough
    // to be obviously a floor decal rather than an obstacle. Slightly above the
    // ground plane to avoid z-fighting with it.
    const PAD_X: f32 = 0.0;
    const PAD_Z: f32 = 6.5;
    const PAD_HALF: f32 = 0.9;

    let mut pad = XrdsCube::new().with_name("PAD");
    pad.size = [PAD_HALF * 2.0, 0.04, PAD_HALF * 2.0];
    pad.transform.translation = [PAD_X, 0.02, PAD_Z];
    let mut pad_mat = xrds_components::XrdsMaterialParams::default();
    pad_mat.base_color = XrdsColor {
        rgba: [0.15, 0.85, 0.35, 1.0],
    };
    pad_mat.unlit = true;
    doc.nodes.push(XrdsSceneNode::from_xrds_cube(
        id(),
        None,
        &pad,
        Some(pad_mat),
    ));

    // --- The effect the zone fires ------------------------------------------
    let mut plume = XrdsEffect::new()
        .with_name("PLUME")
        .with_kind(XrdsEffectKind::Trail);
    // Above the pad, high enough to be visible while standing on it — looking
    // straight down at your own feet is an awkward way to check a result.
    plume.transform.translation = [PAD_X, 1.4, PAD_Z];
    plume.auto_play = false;
    plume.spawn_rate = 250.0;
    plume.lifetime_secs = 1.2;
    plume.size_min = 0.08;
    plume.size_max = 0.16;
    plume.omnidirectional = false;
    plume.spread_deg = 25.0;
    plume.gravity = [0.0, 0.8, 0.0];
    plume.color_start = XrdsColor {
        rgba: [1.0, 0.75, 0.25, 1.0],
    };
    plume.color_end = XrdsColor {
        rgba: [0.5, 0.15, 0.0, 0.0],
    };
    let plume_id = id();
    doc.nodes
        .push(XrdsSceneNode::from_xrds_effect(plume_id, None, &plume));

    doc.tracks = vec![XrdsNamedTrack {
        name: "on_enter".to_string(),
        track: XrdsTrack {
            assets: vec![XrdsTrackAsset {
                target: XrdsActionTarget::Node(plume_id),
                keys: vec![XrdsTrackKey {
                    at_secs: 0.0,
                    action: XrdsAction::PlayEffect { count: None },
                }],
                // Keep, so the plume stays visible after the Track completes.
                // Restore would soft-stop it almost immediately and the result
                // would be a flicker nobody could judge.
                when_finished: XrdsWhenFinished::Keep,
            }],
            ..Default::default()
        },
    }];

    // --- The zone itself ----------------------------------------------------
    // Box, not sphere: a box of the pad's footprint is unambiguous about where the
    // boundary is, which matters when the whole question is whether crossing it
    // registers. Tall (±1.5m about y=1.0) so neither a standing nor a crouching
    // player can pass through without overlapping.
    let mut zone_node = XrdsSceneNode::from_xrds_node(
        id(),
        None,
        &xrds_components::world::XrdsNode {
            name: "ZONE".to_string(),
            enabled: true,
            visible: true,
            transform: Default::default(),
        },
    );
    zone_node.transform.translation = [PAD_X, 1.0, PAD_Z];
    zone_node.payload = XrdsSceneNodePayload::InteractionZone(XrdsSceneInteractionZone {
        shape: xrds_components::XrdsInteractionZoneShape::Box {
            half_extents: [PAD_HALF, 1.5, PAD_HALF],
        },
        grab_type: xrds_components::XrdsGrabType::None,
        hoverable: false,
    });
    zone_node.triggers = vec![XrdsTriggerBinding {
        trigger: XrdsTriggerKind::ZoneEnter,
        track: Some("on_enter".to_string()),
        ..Default::default()
    }];
    doc.nodes.push(zone_node);

    doc.save_json(std::path::Path::new(&out))
        .expect("device-check scene should save");
    println!("[gen] wrote {out} with {} nodes", doc.nodes.len());
}
