use super::*;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// A minimal Bevy app whose asset root is `assets/models/animated/`.
///
/// Sample GLBs live there and can be referenced by bare filename, e.g.
/// `"buster_drone.glb"` rather than via an absolute path.
fn samples_app() -> App {
    let samples_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../assets/models/animated");

    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins((
        AssetPlugin {
            file_path: samples_root.to_string_lossy().into_owned(),
            ..Default::default()
        },
        bevy::animation::AnimationPlugin,
        ImagePlugin::default(),
        GltfPlugin::default(),
        ScenePlugin,
    ));
    app.init_asset::<bevy::gltf::Gltf>();
    app.init_asset::<Scene>();
    app.init_asset::<Mesh>();
    app.init_asset::<StandardMaterial>();
    app.init_asset::<Image>();
    app.init_asset::<AnimationClip>();
    app.init_asset::<AnimationGraph>();
    // Required for GLBs that have skeletal skins (e.g. phoenix_bird.glb).
    app.init_asset::<bevy::mesh::skinning::SkinnedMeshInverseBindposes>();
    app.finish();
    app.cleanup();
    app
}

/// Extends `samples_app()` with the Bevy type registrations that the scene
/// spawner requires when instantiating a complex GLB scene.
fn full_samples_app() -> App {
    let mut app = samples_app();
    app.register_type::<Name>();
    app.register_type::<Transform>();
    app.register_type::<GlobalTransform>();
    app.register_type::<Visibility>();
    app.register_type::<InheritedVisibility>();
    app.register_type::<ViewVisibility>();
    app.register_type::<Mesh3d>();
    app.register_type::<MeshMaterial3d<StandardMaterial>>();
    app.register_type::<AnimationPlayer>();
    app.register_type::<bevy::mesh::morph::MorphWeights>();
    app.register_type::<bevy::mesh::skinning::SkinnedMesh>();
    app
}

/// Polls until the `bevy::gltf::Gltf` asset reaches a terminal `LoadState`
/// (`Loaded` or `Failed`).  Does NOT require recursive dependencies to settle
/// because texture sub-assets may take a variable extra frame.
///
/// After reaching `Loaded`, runs one additional `app.update()` so that the
/// `Assets<Gltf>` storage is populated for the same frame the caller inspects.
fn drive_until_gltf_loaded(
    app: &mut App,
    handle: &bevy::asset::Handle<bevy::gltf::Gltf>,
    max_updates: usize,
) -> bevy::asset::LoadState {
    for _ in 0..max_updates {
        let ls = app
            .world()
            .resource::<AssetServer>()
            .load_state(handle.id());

        if matches!(
            ls,
            bevy::asset::LoadState::Loaded | bevy::asset::LoadState::Failed(_)
        ) {
            // One extra update ensures the loaded asset is written into Assets<Gltf>.
            app.update();
            return ls;
        }

        std::thread::sleep(Duration::from_millis(5));
        app.update();
    }

    app.world()
        .resource::<AssetServer>()
        .load_state(handle.id())
}

/// Convenience: queue a load and drive until settled.
fn load_sample_glb(
    app: &mut App,
    filename: &str,
    max_updates: usize,
) -> bevy::asset::LoadState {
    let handle = app
        .world()
        .resource::<AssetServer>()
        .load::<bevy::gltf::Gltf>(filename.to_string());
    app.update(); // kick off the async load request
    drive_until_gltf_loaded(app, &handle, max_updates)
}

// ---------------------------------------------------------------------------
// Tests — Bevy GltfLoader pipeline
// ---------------------------------------------------------------------------

/// `buster_drone.glb` — complex skeletal animation; verify Bevy loads it fully.
#[test]
fn buster_drone_loads_fully_in_bevy() {
    let mut app = samples_app();
    let ls = load_sample_glb(&mut app, "buster_drone.glb", 2000);
    assert!(
        matches!(ls, bevy::asset::LoadState::Loaded),
        "buster_drone.glb: expected Loaded, got {ls:?}"
    );
}

/// `phoenix_bird.glb` — single animated mesh; verify Bevy loads it fully.
#[test]
fn phoenix_bird_loads_fully_in_bevy() {
    let mut app = samples_app();
    let ls = load_sample_glb(&mut app, "phoenix_bird.glb", 2000);
    assert!(
        matches!(ls, bevy::asset::LoadState::Loaded),
        "phoenix_bird.glb: expected Loaded, got {ls:?}"
    );
}

// magic_wand test removed — gltfglb_magic_wand_animation.glb was deleted.

// ---------------------------------------------------------------------------
// Tests — validate_gltf_source agreement
// ---------------------------------------------------------------------------

/// Cross-check: files that Bevy loads also pass `validate_gltf_source`.
#[test]
fn validate_gltf_source_agrees_with_bevy_load_outcomes() {
    let samples = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../assets/models/animated");
    let path = |name: &str| samples.join(name).to_string_lossy().into_owned();

    assert!(
        validate_gltf_source(&path("buster_drone.glb"), 0).is_ok(),
        "buster_drone.glb should pass validate_gltf_source"
    );
    assert!(
        validate_gltf_source(&path("phoenix_bird.glb"), 0).is_ok(),
        "phoenix_bird.glb should pass validate_gltf_source"
    );
}

// ---------------------------------------------------------------------------
// Tests — post-load asset contents
// ---------------------------------------------------------------------------

/// After Bevy loads the GLB, the `Gltf` struct should contain at least one
/// animation clip for each of our animated sample files.
#[test]
fn buster_drone_has_animations_after_load() {
    let mut app = samples_app();
    let handle = app
        .world()
        .resource::<AssetServer>()
        .load::<bevy::gltf::Gltf>("buster_drone.glb".to_string());
    app.update();
    drive_until_gltf_loaded(&mut app, &handle, 2000);

    let gltfs = app.world().resource::<Assets<bevy::gltf::Gltf>>();
    let gltf = gltfs
        .get(&handle)
        .expect("buster_drone.glb should be available in Assets<Gltf> after load");
    assert!(
        !gltf.animations.is_empty(),
        "buster_drone.glb should contain at least one animation, found {}",
        gltf.animations.len()
    );
}

#[test]
fn phoenix_bird_has_animations_after_load() {
    let mut app = samples_app();
    let handle = app
        .world()
        .resource::<AssetServer>()
        .load::<bevy::gltf::Gltf>("phoenix_bird.glb".to_string());
    app.update();
    drive_until_gltf_loaded(&mut app, &handle, 2000);

    let gltfs = app.world().resource::<Assets<bevy::gltf::Gltf>>();
    let gltf = gltfs
        .get(&handle)
        .expect("phoenix_bird.glb should be available in Assets<Gltf> after load");
    assert!(
        !gltf.animations.is_empty(),
        "phoenix_bird.glb should contain at least one animation, found {}",
        gltf.animations.len()
    );
}

// ---------------------------------------------------------------------------
// Tests — Scene spawning
// ---------------------------------------------------------------------------

/// Spawn a `SceneRoot` for `buster_drone.glb` and verify that Bevy's scene
/// spawner creates child entities (bones, mesh nodes, etc.).
///
/// Uses `full_samples_app()` which registers all Bevy component types that
/// the scene spawner needs to deserialize complex GLB scenes.
#[test]
fn buster_drone_scene_spawns_entities() {
    let mut app = full_samples_app();

    let scene_handle = app
        .world()
        .resource::<AssetServer>()
        .load::<Scene>("buster_drone.glb#Scene0".to_string());

    app.world_mut().spawn(SceneRoot(scene_handle));

    // Drive until scene entities appear (skeleton of buster_drone has many nodes).
    let threshold = 5;
    for _ in 0..2000 {
        app.update();
        std::thread::sleep(Duration::from_millis(5));
        if app.world().entities().len() > threshold {
            break;
        }
    }

    let entity_count = app.world().entities().len();
    assert!(
        entity_count > threshold,
        "buster_drone scene should spawn many entities (skeleton + meshes), got {entity_count}"
    );
}
