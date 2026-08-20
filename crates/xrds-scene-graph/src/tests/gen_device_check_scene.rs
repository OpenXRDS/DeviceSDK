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
/// # Current check: procedural atmosphere on Adreno
///
/// Step 0b of `docs/editor-task-queue-and-hdr-conversion.md`. Bevy's `Atmosphere`
/// (Hillaire 2020) gives a computed sky whose sun comes from the scene's own
/// directional light, so sky, sun position and shadow direction agree — something a
/// captured panorama cannot do. It works on desktop. Whether it is *affordable* on a
/// Quest is the open question, and the whole reason this is a spike:
///
/// - `Atmosphere` requires Bevy's `Hdr`, which adds a float intermediate render
///   target. Paid twice here, once per eye.
/// - It maintains LUTs and runs a `render_sky` pass per view.
/// - Nobody has run it on an Adreno GPU. `bevy_hanabi` was adopted and then
///   rejected for exactly this reason, so the check comes before the commitment.
///
/// ## The scene, and why it is this shape
///
/// Deliberately minimal, so the frame cost that shows up is the atmosphere's rather
/// than the content's:
///
/// - `Sun` — a directional light. **Required**: with no directional light there is
///   no sun, and the sky renders as a flat unlit shell.
/// - `Ground` — a plane, so aerial perspective and the shadow have something to
///   fall on. The sky is only interesting against a horizon.
/// - `Cube` — one shadow caster, to confirm the shadow direction agrees with where
///   the sun appears.
///
/// No passthrough: the runtime suppresses atmosphere under it, deliberately, since
/// a computed sky would paint over the real world.
///
/// ## What to look for
///
/// 1. A blue sky with a horizon, and haze on the distant ground.
/// 2. The sun disc, up and behind the spawn — the light points down and toward -Z,
///    so the sun sits opposite, up and toward +Z. Turn around and look up ~50°.
/// 3. The cube's shadow pointing away from it.
///
/// ## And the number that decides the feature
///
/// Frame rate. `[XR-DIAG] update_view_proj#N` logs every 90 frames, so two
/// consecutive timestamps give the period of 90 frames:
///
/// ```bash
/// grep -oE "^[0-9-]+ [0-9:.]+.*update_view_proj#[0-9]+" run.txt | tail -20
/// ```
///
/// Compare against the same scene with `atmosphere: None` — that A/B is the point,
/// since an absolute figure says nothing without a baseline.
#[test]
fn xxx_gen_device_check_scene() {
    let Ok(out) = std::env::var("XRDS_GEN_DEVICE_SCENE") else {
        return;
    };

    // Set XRDS_GEN_DEVICE_SCENE_NO_ATMOSPHERE=1 to emit the baseline half of the
    // A/B. Same scene otherwise, so the only difference in the frame timing is the
    // feature under test.
    let with_atmosphere = std::env::var("XRDS_GEN_DEVICE_SCENE_NO_ATMOSPHERE").is_err();

    let mut doc = XrdsSceneDocument::default();
    let mut next = 1u64;
    let mut id = || {
        let v = XrdsSceneNodeId(next);
        next += 1;
        v
    };

    if with_atmosphere {
        doc.metadata
            .environment
            .get_or_insert_with(Default::default)
            .atmosphere = Some(Default::default());
    }

    let mut ground = XrdsPlane3D::new().with_name("Ground");
    // Large: aerial perspective is a distance effect, and a small pad would show
    // none of it.
    ground.size = [200.0, 200.0];
    ground.transform.translation = [0.0, 0.0, 0.0];
    doc.nodes
        .push(XrdsSceneNode::from_xrds_plane3d(id(), None, &ground, None));

    // Points down and toward -Z, so the sun sits up and toward +Z — behind the
    // spawn, which faces -Z. Matches the editor's default scene so desktop and
    // device show the same sky.
    let mut sun = XrdsDirectionalLight::new().with_name("Sun");
    sun.transform.translation = [0.0, 5.0, -3.0];
    sun.transform.rotation_quat_xyzw = [-0.4226, 0.0, 0.0, 0.9063];
    sun.illuminance = 10_000.0;
    sun.shadows = true;
    doc.nodes
        .push(XrdsSceneNode::from_xrds_directional_light(id(), None, &sun));

    // One shadow caster, a step in front of the spawn at (0, 1.6, 8).
    let mut cube = XrdsCube::new().with_name("Cube");
    cube.size = [1.0, 1.0, 1.0];
    cube.transform.translation = [0.0, 0.5, 6.0];
    doc.nodes
        .push(XrdsSceneNode::from_xrds_cube(id(), None, &cube, None));

    doc.save_json(std::path::Path::new(&out))
        .expect("device-check scene should save");
    println!(
        "[gen] wrote {out} with {} nodes, atmosphere={}",
        doc.nodes.len(),
        with_atmosphere
    );
}
