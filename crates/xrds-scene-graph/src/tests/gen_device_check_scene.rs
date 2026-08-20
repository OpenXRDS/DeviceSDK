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
/// # Current check: does Adreno sample `Rgb9e5Ufloat`?
///
/// The last open question from `docs/editor-task-queue-and-hdr-conversion.md`.
///
/// An environment map is `Rgba16Float` today — 8 bytes per pixel, and a 512² cube
/// costs 16.78 MB of VRAM. The obvious fix would be a GPU-compressed HDR format,
/// but **there is no such path on a Quest**: Bevy's `CompressedImageFormats` offers
/// only `ASTC_LDR`, `BC` and `ETC2`, and its own source notes that ASTC HDR is not
/// supported by wgpu at all. BC6H is real but desktop-only.
///
/// `Rgb9e5Ufloat` — shared-exponent RGB, 4 bytes per pixel — halves that to 8.39 MB
/// and is **not a compressed format**, so it needs no `CompressedImageFormats`
/// support and loads with `NONE`. Verified on desktop through Bevy's own
/// `ktx2_buffer_to_image`. Whether an Adreno GPU actually samples it is the one
/// thing a desktop check cannot answer, and the reason this scene exists.
///
/// ## Running it
///
/// The `.ktx2` files are produced outside the tree (see the conversion probe in the
/// plan doc). Lay the scene directory out the way the runtime expects — asset URIs
/// are bare filenames because the asset-server root *is* `assets/`:
///
/// ```text
///   scene_dir/
///     scene.json
///     assets/
///       sky.ktx2
/// ```
///
/// Set `XRDS_GEN_DEVICE_SCENE_RGBA16F=1` to emit the control half of the A/B, which
/// uses the format we already ship. Both halves reference `sky.ktx2`, so swap the
/// file rather than the scene — that keeps the document identical and leaves the
/// texture format as the only difference.
///
/// ## What to look for
///
/// 1. **The sky renders.** A correct panorama with a visible horizon means Adreno
///    sampled the format. Black sky with the cube still visible means it did not.
/// 2. The cube, lit and casting a shadow — proves the frame is rendering at all, so
///    a black sky cannot be mistaken for a dead app.
/// 3. `grep -i "rgb9e5\|Rgb9e5Ufloat\|not supported\|validation" run.txt` — wgpu
///    names an unsupported texture format explicitly rather than failing silently.
#[test]
fn xxx_gen_device_check_scene() {
    let Ok(out) = std::env::var("XRDS_GEN_DEVICE_SCENE") else {
        return;
    };

    // The control half of the A/B: the format we already ship. Same document
    // otherwise, so the texture format is the only variable.
    let control = std::env::var("XRDS_GEN_DEVICE_SCENE_RGBA16F").is_ok();

    let mut doc = XrdsSceneDocument::default();
    let mut next = 1u64;
    let mut id = || {
        let v = XrdsSceneNodeId(next);
        next += 1;
        v
    };

    // Bare filename: the runtime's asset-server root is already `assets/`, so an
    // "assets/" prefix here would resolve to `assets/assets/sky.ktx2`.
    doc.assets.push(XrdsSceneAsset {
        id: "sky".to_string(),
        uri: "sky.ktx2".to_string(),
        kind: XrdsSceneAssetKind::EnvironmentMap,
    });

    let env = doc.metadata.environment.get_or_insert_with(Default::default);
    env.skybox = Some(XrdsSceneSkyboxEnvironment {
        texture_asset_id: "sky".to_string(),
        // Matches the shipped maps' authored brightness. A skybox that renders but
        // is scaled to black would be indistinguishable from an unsupported format,
        // which is the one confusion this check cannot afford.
        brightness: 1000.0,
        yaw_deg: 0.0,
    });

    // Ground, so the horizon has something to meet.
    let mut ground = XrdsPlane3D::new().with_name("Ground");
    ground.size = [200.0, 200.0];
    ground.transform.translation = [0.0, 0.0, 0.0];
    doc.nodes
        .push(XrdsSceneNode::from_xrds_plane3d(id(), None, &ground, None));

    let mut sun = XrdsDirectionalLight::new().with_name("Sun");
    sun.transform.translation = [0.0, 5.0, -3.0];
    sun.transform.rotation_quat_xyzw = [-0.4226, 0.0, 0.0, 0.9063];
    sun.illuminance = 10_000.0;
    sun.shadows = true;
    doc.nodes
        .push(XrdsSceneNode::from_xrds_directional_light(id(), None, &sun));

    // The liveness check. If the sky is black but this cube is lit and casting a
    // shadow, the app is fine and the *format* is the failure — which is exactly
    // the distinction being tested.
    let mut cube = XrdsCube::new().with_name("Cube");
    cube.size = [1.0, 1.0, 1.0];
    cube.transform.translation = [0.0, 0.5, 6.0];
    doc.nodes
        .push(XrdsSceneNode::from_xrds_cube(id(), None, &cube, None));

    doc.save_json(std::path::Path::new(&out))
        .expect("device-check scene should save");
    println!(
        "[gen] wrote {out} with {} nodes, skybox=sky.ktx2 ({})",
        doc.nodes.len(),
        if control { "control: rgba16f" } else { "under test: rgb9e5" }
    );
}
