use super::*;
use bevy::{
    animation::{graph::AnimationGraph, AnimationClip},
    app::App,
    asset::{AssetApp, AssetPlugin},
    gltf::GltfPlugin,
    image::{Image, ImagePlugin},
    pbr::{MeshMaterial3d, StandardMaterial},
    prelude::AlphaMode,
    scene::{Scene, ScenePlugin},
    MinimalPlugins,
};
use std::path::PathBuf;
use std::time::Duration;
use xrds_scene_graph::{
    XrdsEditorMetadata, XrdsGltfAssetExportPolicy, XrdsSceneAnimationRepeatMode, XrdsSceneAsset,
    XrdsSceneAssetKind, XrdsSceneCube, XrdsSceneDocument, XrdsSceneGltfAnimationSelector,
    XrdsSceneGltfAsset, XrdsSceneGltfMorphTargetOverride, XrdsSceneGltfMorphTargetSelector,
    XrdsSceneGltfMorphTargetWeight, XrdsSceneGltfNodeAuthoring, XrdsSceneGltfNodeLocator,
    XrdsSceneGltfPlayback, XrdsSceneMaterial, XrdsSceneMaterialAlphaMode,
    XrdsSceneMaterialPbrParams, XrdsSceneMaterialTextureSlots, XrdsSceneMetadata, XrdsSceneNode,
    XrdsSceneNodeId, XrdsSceneNodePayload, XrdsSceneTransform, XrdsSourceLink,
};

const VALID_GLTF_PATH: &str = "models/TestStatus/EmbeddedTriangle.gltf";
const BROKEN_DEPENDENCY_GLTF_PATH: &str = "models/TestBrokenDependency/MissingBufferScene.gltf";
const MISSING_ROOT_SCENE_PATH: &str = "models/DoesNotExist/MissingScene.gltf#Scene0";
const MORPH_STRESS_TEST_PATH: &str = "models/MorphStressTest/MorphStressTest.gltf";
const MORPH_STRESS_TEST_EXAMPLE_PATH: &str = "models/animated/morphOriginal/MorphStressTest.gltf";
const OBSERVER_TEST_GLTF_PATH: &str = "models/DoesNotExist/ObserverScene.gltf";

fn test_app() -> App {
    let asset_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets");
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins((
        AssetPlugin {
            file_path: asset_root.to_string_lossy().into_owned(),
            ..Default::default()
        },
        bevy::animation::AnimationPlugin,
        ImagePlugin::default(),
        GltfPlugin::default(),
    ));
    app.init_asset::<Scene>();
    app.init_asset::<Mesh>();
    app.init_asset::<StandardMaterial>();
    app.init_asset::<Image>();
    app.init_asset::<AnimationClip>();
    app.init_asset::<AnimationGraph>();
    app.finish();
    app.cleanup();
    app
}

fn xrds_test_app() -> App {
    let asset_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets");
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins((
        AssetPlugin {
            file_path: asset_root.to_string_lossy().into_owned(),
            ..Default::default()
        },
        bevy::animation::AnimationPlugin,
        ImagePlugin::default(),
        GltfPlugin::default(),
    ));
    app.init_asset::<Scene>();
    app.init_asset::<Mesh>();
    app.init_asset::<StandardMaterial>();
    app.init_asset::<Image>();
    app.init_asset::<AnimationClip>();
    app.init_asset::<AnimationGraph>();
    {
        let _ = XrdsAPI::attach(&mut app);
    }
    app.finish();
    app.cleanup();
    app
}

fn xrds_real_asset_test_app() -> App {
    let asset_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets");
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins((
        AssetPlugin {
            file_path: asset_root.to_string_lossy().into_owned(),
            ..Default::default()
        },
        bevy::animation::AnimationPlugin,
        ImagePlugin::default(),
        GltfPlugin::default(),
        ScenePlugin,
    ));
    app.init_asset::<Scene>();
    app.init_asset::<Mesh>();
    app.init_asset::<StandardMaterial>();
    app.init_asset::<Image>();
    app.init_asset::<AnimationClip>();
    app.init_asset::<AnimationGraph>();
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
    {
        let _ = XrdsAPI::attach(&mut app);
    }
    app.finish();
    app.cleanup();
    app
}

fn xrds_scene_ready_observer_test_app() -> App {
    let asset_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets");
    let mut app = App::new();
    app.add_plugins(MinimalPlugins);
    app.add_plugins((
        AssetPlugin {
            file_path: asset_root.to_string_lossy().into_owned(),
            ..Default::default()
        },
        bevy::animation::AnimationPlugin,
        GltfPlugin::default(),
        ScenePlugin,
    ));
    app.init_asset::<Scene>();
    app.init_asset::<bevy::gltf::Gltf>();
    app.init_asset::<AnimationClip>();
    app.init_asset::<AnimationGraph>();
    app.init_resource::<PendingGltfAnimationRequests>();
    app.init_resource::<ActiveGltfAnimationStates>();
    app.add_observer(apply_pending_gltf_animation_requests_on_scene_ready);
    app.finish();
    app.cleanup();
    app
}

fn asset_fixture_path(relative_path: &str) -> String {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../assets")
        .join(relative_path)
        .to_string_lossy()
        .into_owned()
}

fn imported_test_document() -> XrdsSceneDocument {
    XrdsSceneDocument {
        nodes: vec![
            XrdsSceneNode {
                id: XrdsSceneNodeId(100),
                parent_id: None,
                name: "Imported Root".to_string(),
                enabled: true,
                visible: true,
                transform: XrdsSceneTransform {
                    translation: [1.0, 2.0, 3.0],
                    ..Default::default()
                },
                payload: XrdsSceneNodePayload::Empty,
                editor: XrdsEditorMetadata {
                    tags: vec!["folder".to_string(), "root".to_string()],
                    layer: Some("Scene".to_string()),
                    locked: true,
                    hidden_in_editor: false,
                    user_properties: [("group".to_string(), "layout".to_string())]
                        .into_iter()
                        .collect(),
                    source: Some(XrdsSourceLink {
                        asset_id: Some("catalog:root".to_string()),
                        source_node: Some("RootNode".to_string()),
                        import_revision: Some("rev-a".to_string()),
                    }),
                },
            },
            XrdsSceneNode {
                id: XrdsSceneNodeId(101),
                parent_id: Some(XrdsSceneNodeId(100)),
                name: "Imported Cube".to_string(),
                enabled: true,
                visible: true,
                transform: XrdsSceneTransform {
                    translation: [4.0, 5.0, 6.0],
                    ..Default::default()
                },
                payload: XrdsSceneNodePayload::Cube(XrdsSceneCube {
                    size: [2.0, 3.0, 4.0],
                    material: XrdsSceneMaterial {
                        base_color: [0.2, 0.4, 0.6, 0.8],
                        emissive: [0.1, 0.0, 0.0, 1.0],
                        opacity: 0.8,
                        unlit: true,
                        textures: XrdsSceneMaterialTextureSlots::default(),
                        pbr: XrdsSceneMaterialPbrParams {
                            metallic: 0.7,
                            perceptual_roughness: 0.25,
                            reflectance: 0.6,
                            double_sided: true,
                            alpha_mode: XrdsSceneMaterialAlphaMode::Mask,
                            alpha_cutoff: 0.42,
                        },
                    },
                }),
                editor: XrdsEditorMetadata {
                    tags: vec!["mesh".to_string(), "selectable".to_string()],
                    layer: Some("Gameplay".to_string()),
                    locked: false,
                    hidden_in_editor: true,
                    user_properties: [
                        ("inspector:expanded".to_string(), "true".to_string()),
                        ("author".to_string(), "editor-test".to_string()),
                    ]
                    .into_iter()
                    .collect(),
                    source: Some(XrdsSourceLink {
                        asset_id: Some("asset:cube".to_string()),
                        source_node: Some("CubeNode".to_string()),
                        import_revision: Some("rev-b".to_string()),
                    }),
                },
            },
        ],
        ..Default::default()
    }
}

fn imported_gltf_catalog_document() -> XrdsSceneDocument {
    XrdsSceneDocument {
        nodes: vec![
            XrdsSceneNode {
                id: XrdsSceneNodeId(200),
                parent_id: None,
                name: "Lamp A".to_string(),
                enabled: true,
                visible: true,
                transform: XrdsSceneTransform::default(),
                payload: XrdsSceneNodePayload::GltfAsset(XrdsSceneGltfAsset {
                    asset_id: Some("asset:lamp".to_string()),
                    asset_uri: asset_fixture_path(VALID_GLTF_PATH),
                    scene_index: 0,
                    export_policy: XrdsGltfAssetExportPolicy::KeepExternalReference,
                }),
                editor: XrdsEditorMetadata {
                    source: Some(XrdsSourceLink {
                        asset_id: Some("asset:lamp".to_string()),
                        source_node: Some("LampA".to_string()),
                        import_revision: Some("rev-1".to_string()),
                    }),
                    ..Default::default()
                },
            },
            XrdsSceneNode {
                id: XrdsSceneNodeId(201),
                parent_id: None,
                name: "Lamp B".to_string(),
                enabled: true,
                visible: true,
                transform: XrdsSceneTransform::default(),
                payload: XrdsSceneNodePayload::GltfAsset(XrdsSceneGltfAsset {
                    asset_id: None,
                    asset_uri: asset_fixture_path(VALID_GLTF_PATH),
                    scene_index: 0,
                    export_policy: XrdsGltfAssetExportPolicy::KeepExternalReference,
                }),
                editor: XrdsEditorMetadata::default(),
            },
            XrdsSceneNode {
                id: XrdsSceneNodeId(202),
                parent_id: None,
                name: "Triangle".to_string(),
                enabled: true,
                visible: true,
                transform: XrdsSceneTransform::default(),
                payload: XrdsSceneNodePayload::GltfAsset(XrdsSceneGltfAsset {
                    asset_id: None,
                    asset_uri: asset_fixture_path(BROKEN_DEPENDENCY_GLTF_PATH),
                    scene_index: 0,
                    export_policy: XrdsGltfAssetExportPolicy::KeepExternalReference,
                }),
                editor: XrdsEditorMetadata::default(),
            },
        ],
        ..Default::default()
    }
}

fn spawn_scene_root_entity(app: &mut App, asset_path: &str) -> Handle<XrdsGltfAsset> {
    let scene_handle = {
        let asset_server = app.world().resource::<AssetServer>();
        asset_server.load::<Scene>(asset_path.to_string())
    };
    let entity = app.world_mut().spawn(SceneRoot(scene_handle)).id();
    Handle::from(entity)
}

fn spawn_stored_gltf_entity(app: &mut App, gltf_asset_path: &str) -> Handle<XrdsGltfAsset> {
    let scene_handle = {
        let asset_server = app.world().resource::<AssetServer>();
        asset_server.load::<Scene>(format!("{gltf_asset_path}#Scene0"))
    };
    let root = app
        .world_mut()
        .spawn((
            Name::new("StoredGltfTestEntity"),
            SceneRoot(scene_handle),
            XrdsStored(
                XrdsGltfAsset::new(gltf_asset_path.to_string()).with_name("StoredGltfTestEntity"),
            ),
        ))
        .id();

    let morph_mesh = {
        let mut mesh = Mesh::from(Cuboid::default());
        mesh.set_morph_target_names(vec!["Blink".to_string(), "Smile".to_string()]);
        app.world_mut().resource_mut::<Assets<Mesh>>().add(mesh)
    };

    let animation_player = app
        .world_mut()
        .spawn((
            Name::new("MorphAnimationPlayer"),
            AnimationPlayer::default(),
        ))
        .id();

    let morph_mesh_node = app
        .world_mut()
        .spawn((Name::new("MorphMeshNode"), Mesh3d(morph_mesh)))
        .id();

    app.world_mut()
        .entity_mut(root)
        .add_children(&[animation_player, morph_mesh_node]);
    Handle::from(root)
}

fn spawn_real_stored_gltf_entity(app: &mut App, gltf_asset_path: &str) -> Handle<XrdsGltfAsset> {
    let scene_handle = {
        let asset_server = app.world().resource::<AssetServer>();
        asset_server.load::<Scene>(format!("{gltf_asset_path}#Scene0"))
    };

    let root = app
        .world_mut()
        .spawn((
            Name::new("RealStoredGltfTestEntity"),
            SceneRoot(scene_handle),
            XrdsStored(
                XrdsGltfAsset::new(gltf_asset_path.to_string())
                    .with_name("RealStoredGltfTestEntity"),
            ),
        ))
        .id();

    Handle::from(root)
}

fn seed_synthetic_gltf_asset(app: &mut App, gltf_asset_path: &str) {
    let (gltf_handle, clip_handles) = {
        let asset_server = app.world().resource::<AssetServer>();
        (
            asset_server.load::<bevy::gltf::Gltf>(gltf_asset_path.to_string()),
            [
                asset_server.load::<AnimationClip>(format!("{gltf_asset_path}#Animation0")),
                asset_server.load::<AnimationClip>(format!("{gltf_asset_path}#Animation1")),
                asset_server.load::<AnimationClip>(format!("{gltf_asset_path}#Animation2")),
            ],
        )
    };

    {
        let mut clips = app.world_mut().resource_mut::<Assets<AnimationClip>>();
        for handle in &clip_handles {
            let _ = clips.insert(handle.id(), AnimationClip::default());
        }
    }

    let mut named_animations = bevy::platform::collections::HashMap::default();
    named_animations.insert("MorphStressLoop".into(), clip_handles[2].clone());

    let _ = app
        .world_mut()
        .resource_mut::<Assets<bevy::gltf::Gltf>>()
        .insert(
            gltf_handle.id(),
            bevy::gltf::Gltf {
                scenes: Vec::new(),
                named_scenes: Default::default(),
                meshes: Vec::new(),
                named_meshes: Default::default(),
                materials: Vec::new(),
                named_materials: Default::default(),
                nodes: Vec::new(),
                named_nodes: Default::default(),
                skins: Vec::new(),
                named_skins: Default::default(),
                default_scene: None,
                animations: clip_handles.to_vec(),
                named_animations,
                source: None,
            },
        );
}

fn attach_synthetic_morph_mesh_to_root(app: &mut App, root: Entity, node_name: &str) -> Entity {
    let morph_mesh = {
        let mut mesh = Mesh::from(Cuboid::default());
        mesh.set_morph_target_names(vec!["Blink".to_string(), "Smile".to_string()]);
        app.world_mut().resource_mut::<Assets<Mesh>>().add(mesh)
    };

    let child = app
        .world_mut()
        .spawn((Name::new(node_name.to_string()), Mesh3d(morph_mesh)))
        .id();
    app.world_mut().entity_mut(root).add_child(child);
    child
}

fn drive_until_terminal_status(
    app: &mut App,
    handle: &Handle<XrdsGltfAsset>,
    max_updates: usize,
) -> Option<XrdsGltfLoadStatus> {
    let mut last_status = gltf_load_status_in_world(app.world(), handle);

    for _ in 0..max_updates {
        if matches!(
            last_status,
            Some(XrdsGltfLoadStatus::Loaded | XrdsGltfLoadStatus::Failed(_))
        ) {
            break;
        }

        std::thread::sleep(Duration::from_millis(2));
        app.update();
        last_status = gltf_load_status_in_world(app.world(), handle);
    }

    last_status
}

#[test]
fn gltf_load_status_transitions_from_pending_to_loaded_for_valid_scene() {
    let mut app = test_app();
    let handle = spawn_scene_root_entity(&mut app, &format!("{VALID_GLTF_PATH}#Scene0"));

    let initial_status = gltf_load_status_in_world(app.world(), &handle);
    assert!(matches!(
        initial_status,
        Some(XrdsGltfLoadStatus::NotLoaded | XrdsGltfLoadStatus::Loading)
    ));

    let final_status = drive_until_terminal_status(&mut app, &handle, 1000);
    assert_eq!(final_status, Some(XrdsGltfLoadStatus::Loaded));
}

#[test]
fn gltf_load_status_reports_root_scene_load_failures() {
    let mut app = test_app();
    let handle = spawn_scene_root_entity(&mut app, MISSING_ROOT_SCENE_PATH);

    let final_status = drive_until_terminal_status(&mut app, &handle, 300);
    match final_status {
        Some(XrdsGltfLoadStatus::Failed(message)) => {
            assert!(!message.trim().is_empty());
        }
        other => panic!("expected failed root-scene status, got {other:?}"),
    }
}

#[test]
fn gltf_load_status_reports_recursive_dependency_failures() {
    let mut app = test_app();
    let handle =
        spawn_scene_root_entity(&mut app, &format!("{BROKEN_DEPENDENCY_GLTF_PATH}#Scene0"));

    let final_status = drive_until_terminal_status(&mut app, &handle, 1000);
    match final_status {
        Some(XrdsGltfLoadStatus::Failed(message)) => {
            assert!(message.contains("missing.png") || !message.trim().is_empty());
        }
        other => panic!("expected failed dependency status, got {other:?}"),
    }

    let scene_id = app
        .world()
        .get::<SceneRoot>(handle.entity())
        .expect("test entity should keep SceneRoot")
        .id();
    let asset_server = app.world().resource::<AssetServer>();

    assert!(matches!(
        asset_server.load_state(scene_id),
        bevy::asset::LoadState::Loaded
    ));
    assert!(matches!(
        asset_server.recursive_dependency_load_state(scene_id),
        bevy::asset::RecursiveDependencyLoadState::Failed(_)
    ));
}

#[test]
fn xrds_gltf_animation_api_applies_index_playback_without_metadata_and_reports_morph_targets() {
    let mut app = xrds_test_app();

    let handle = spawn_stored_gltf_entity(&mut app, MORPH_STRESS_TEST_PATH);

    {
        let mut xrds = XrdsAPI::attach(&mut app);
        xrds.play_gltf_animation(
            &handle,
            XrdsGltfAnimationSelector::Index(2),
            XrdsGltfAnimationPlaybackOptions::default(),
        )
        .expect(
            "index-based glTF animation playback should apply when scene players already exist",
        );
    }

    assert!(!app
        .world()
        .resource::<PendingGltfAnimationRequests>()
        .requests
        .contains_key(&handle.entity()));

    let animation_state = {
        let xrds = XrdsAPI::attach(&mut app);
        xrds.gltf_animation_state(&handle)
            .expect("animation state queries should succeed for glTF handles")
            .expect(
                "index-based playback should populate XRDS glTF animation state without metadata",
            )
    };

    assert_eq!(animation_state.animation.index, 2);
    assert!(animation_state.animation.name.is_none());
    assert!(animation_state.playing);
    assert!(!animation_state.paused);

    seed_synthetic_gltf_asset(&mut app, MORPH_STRESS_TEST_PATH);

    let (animations, morph_targets) = {
        let xrds = XrdsAPI::attach(&mut app);
        (
            xrds.gltf_animations(&handle)
                .expect("synthetic glTF metadata should expose XRDS animation info"),
            xrds.gltf_morph_targets(&handle)
                .expect("synthetic realized mesh should expose XRDS morph target info"),
        )
    };

    assert!(animations.len() > 2);
    assert!(animations.iter().any(|animation| animation.index == 2));

    assert_eq!(animation_state.animation.index, 2);
    assert!(animation_state.playing);
    assert!(!animation_state.paused);
    assert!(matches!(
        animation_state.repeat,
        XrdsAnimationRepeatMode::Loop
    ));

    assert!(!morph_targets.is_empty());
    assert!(morph_targets.iter().any(|set| !set.targets.is_empty()));
}

#[test]
fn xrds_gltf_animation_api_animates_real_morph_asset_weights() {
    let mut app = xrds_real_asset_test_app();
    let handle = spawn_real_stored_gltf_entity(&mut app, MORPH_STRESS_TEST_EXAMPLE_PATH);

    {
        let mut xrds = XrdsAPI::attach(&mut app);
        xrds.play_gltf_animation(
            &handle,
            XrdsGltfAnimationSelector::Index(2),
            XrdsGltfAnimationPlaybackOptions::default(),
        )
        .expect("XRDS should accept a queued morph animation request before the real asset loads");
    }

    let mut first_snapshot: Option<Vec<f32>> = None;
    let mut animation_state = None;
    let mut weights_changed = false;

    for _ in 0..1200 {
        std::thread::sleep(Duration::from_millis(2));
        app.update();

        let xrds = XrdsAPI::attach(&mut app);
        let status = xrds.gltf_load_status(&handle);
        if status != Some(XrdsGltfLoadStatus::Loaded) {
            continue;
        }

        animation_state = xrds
            .gltf_animation_state(&handle)
            .expect("animation state queries should succeed for the real morph asset");

        let Ok(weight_sets) = xrds.gltf_morph_target_weights(&handle) else {
            continue;
        };
        if weight_sets.is_empty() {
            continue;
        }

        let flattened = weight_sets
            .iter()
            .flat_map(|set| set.weights.iter().map(|weight| weight.weight))
            .collect::<Vec<_>>();
        if flattened.is_empty() {
            continue;
        }

        match &first_snapshot {
            None => first_snapshot = Some(flattened),
            Some(previous) => {
                if previous.len() == flattened.len()
                    && previous
                        .iter()
                        .zip(&flattened)
                        .any(|(before, after)| (before - after).abs() > 1e-4)
                {
                    weights_changed = true;
                    break;
                }
            }
        }
    }

    assert_eq!(
        drive_until_terminal_status(&mut app, &handle, 1),
        Some(XrdsGltfLoadStatus::Loaded)
    );

    let animation_state = animation_state
        .expect("real morph asset should produce active XRDS animation state once loaded");
    assert_eq!(animation_state.animation.index, 2);
    assert!(animation_state.playing);
    assert!(!animation_state.paused);
    assert!(
        weights_changed,
        "real morph asset weights should change over time through the XRDS SDK playback path"
    );
}

#[test]
fn scene_ready_observer_applies_queued_gltf_animation_requests() {
    let mut app = xrds_scene_ready_observer_test_app();

    let scene_handle = {
        let mut scene_world = World::new();
        scene_world.spawn((Name::new("ObserverPlayer"), AnimationPlayer::default()));
        app.world_mut()
            .resource_mut::<Assets<Scene>>()
            .add(Scene::new(scene_world))
    };

    let root = app
        .world_mut()
        .spawn((
            Name::new("ObserverRoot"),
            SceneRoot(scene_handle),
            XrdsStored(XrdsGltfAsset::new(OBSERVER_TEST_GLTF_PATH).with_name("ObserverRoot")),
        ))
        .id();

    app.world_mut()
        .resource_mut::<PendingGltfAnimationRequests>()
        .requests
        .insert(
            root,
            PendingGltfAnimationRequest {
                selector: XrdsGltfAnimationSelector::Index(2),
                options: XrdsGltfAnimationPlaybackOptions::default(),
            },
        );

    for _ in 0..3 {
        app.update();
    }

    assert!(!app
        .world()
        .resource::<PendingGltfAnimationRequests>()
        .requests
        .contains_key(&root));

    let animation_state = app
        .world()
        .resource::<ActiveGltfAnimationStates>()
        .states
        .get(&root)
        .cloned()
        .expect("scene-ready observer should populate XRDS glTF animation state");

    assert_eq!(animation_state.animation.index, 2);
    assert!(animation_state.playing);
    assert!(!animation_state.paused);

    let player_entity = app
        .world()
        .get::<Children>(root)
        .expect("scene root should receive spawned scene children")
        .iter()
        .find(|child| app.world().get::<AnimationPlayer>(*child).is_some())
        .expect("spawned scene should contain an animation player descendant");

    assert!(app
        .world()
        .get::<AnimationGraphHandle>(player_entity)
        .is_some());
}

#[test]
fn xrds_gltf_morph_weight_api_reads_and_sets_weights() {
    let mut app = xrds_test_app();
    let handle = spawn_stored_gltf_entity(&mut app, MORPH_STRESS_TEST_PATH);
    seed_synthetic_gltf_asset(&mut app, MORPH_STRESS_TEST_PATH);

    let initial = {
        let xrds = XrdsAPI::attach(&mut app);
        xrds.gltf_morph_target_weights(&handle)
            .expect("synthetic realized mesh should expose XRDS morph target weights")
    };

    let first_set = initial
        .first()
        .expect("synthetic test mesh should expose at least one morph target set")
        .clone();
    assert_eq!(first_set.weights.len(), 2);
    assert!(first_set.weights.iter().all(|weight| weight.weight == 0.0));

    {
        let mut xrds = XrdsAPI::attach(&mut app);
        xrds.set_gltf_morph_target_weight(
            &handle,
            &first_set.node,
            first_set.mesh_name.as_deref(),
            XrdsGltfMorphTargetSelector::Name("Smile".to_string()),
            0.75,
        )
        .expect("setting XRDS glTF morph target weight should succeed");
    }

    let updated = {
        let xrds = XrdsAPI::attach(&mut app);
        xrds.gltf_morph_target_weights(&handle)
            .expect("updated XRDS morph target weights should be queryable")
    };

    let updated_set = updated
        .iter()
        .find(|set| set.node == first_set.node && set.mesh_name == first_set.mesh_name)
        .expect("updated weight set should still be addressable by node locator");
    let smile_weight = updated_set
        .weights
        .iter()
        .find(|weight| weight.target.name.as_deref() == Some("Smile"))
        .expect("Smile morph target should still exist");
    let blink_weight = updated_set
        .weights
        .iter()
        .find(|weight| weight.target.name.as_deref() == Some("Blink"))
        .expect("Blink morph target should still exist");

    assert_eq!(smile_weight.weight, 0.75);
    assert_eq!(blink_weight.weight, 0.0);
}

#[test]
fn export_scene_document_captures_runtime_gltf_morph_weights_in_authoring() {
    let fixture_uri = asset_fixture_path(MORPH_STRESS_TEST_PATH);
    let document = XrdsSceneDocument {
        assets: vec![XrdsSceneAsset {
            id: "asset:morph".to_string(),
            uri: fixture_uri.clone(),
            kind: XrdsSceneAssetKind::Gltf,
        }],
        nodes: vec![XrdsSceneNode {
            id: XrdsSceneNodeId(630),
            parent_id: None,
            name: "Morph".to_string(),
            enabled: true,
            visible: true,
            transform: XrdsSceneTransform::default(),
            payload: XrdsSceneNodePayload::GltfAsset(XrdsSceneGltfAsset {
                asset_id: Some("asset:morph".to_string()),
                asset_uri: fixture_uri.clone(),
                scene_index: 0,
                export_policy: XrdsGltfAssetExportPolicy::KeepExternalReference,
            }),
            editor: XrdsEditorMetadata::default(),
        }],
        gltf_node_authoring: [(
            630,
            XrdsSceneGltfNodeAuthoring {
                default_playback: Some(XrdsSceneGltfPlayback {
                    selector: XrdsSceneGltfAnimationSelector::Name("Run".to_string()),
                    repeat: XrdsSceneAnimationRepeatMode::Loop,
                    speed: 1.0,
                    start_paused: false,
                }),
                morph_target_overrides: Vec::new(),
            },
        )]
        .into_iter()
        .collect(),
        ..Default::default()
    };

    let mut app = xrds_test_app();
    {
        let mut xrds = XrdsAPI::attach(&mut app);
        xrds.import_scene_document(&document)
            .expect("scene import should succeed before runtime morph edits");
    }

    let handle = {
        let xrds = XrdsAPI::attach(&mut app);
        xrds.handle_of::<XrdsGltfAsset>(XrdsId(630))
            .expect("imported gltf node should be indexed by id")
    };

    attach_synthetic_morph_mesh_to_root(&mut app, handle.entity(), "MorphMeshNode");
    seed_synthetic_gltf_asset(&mut app, &fixture_uri);

    let first_set = {
        let xrds = XrdsAPI::attach(&mut app);
        xrds.gltf_morph_target_weights(&handle)
            .expect("runtime morph weights should be queryable before export")
            .into_iter()
            .next()
            .expect("synthetic morph mesh should exist")
    };

    {
        let mut xrds = XrdsAPI::attach(&mut app);
        xrds.set_gltf_morph_target_weight(
            &handle,
            &first_set.node,
            first_set.mesh_name.as_deref(),
            XrdsGltfMorphTargetSelector::Name("Blink".to_string()),
            0.15,
        )
        .expect("Blink weight update should succeed");
        xrds.set_gltf_morph_target_weight(
            &handle,
            &first_set.node,
            first_set.mesh_name.as_deref(),
            XrdsGltfMorphTargetSelector::Name("Smile".to_string()),
            0.9,
        )
        .expect("Smile weight update should succeed");
    }

    let exported = {
        let xrds = XrdsAPI::attach(&mut app);
        xrds.export_scene_document()
            .expect("scene export should capture current runtime morph weights")
    };

    let exported_authoring = exported
        .gltf_node_authoring
        .get(&630)
        .expect("exported scene should keep gltf authoring for the runtime-edited node");
    assert!(exported_authoring.default_playback.is_some());
    assert_eq!(exported_authoring.morph_target_overrides.len(), 1);

    let override_entry = &exported_authoring.morph_target_overrides[0];
    assert_eq!(
        override_entry.node.node_name.as_deref(),
        Some("MorphMeshNode")
    );
    assert_eq!(override_entry.node.node_index_path, vec![0]);
    assert_eq!(override_entry.weights.len(), 2);

    let blink = override_entry
        .weights
        .iter()
        .find(|weight| {
            matches!(
                &weight.selector,
                XrdsSceneGltfMorphTargetSelector::Name(name) if name == "Blink"
            )
        })
        .expect("Blink override should be exported");
    let smile = override_entry
        .weights
        .iter()
        .find(|weight| {
            matches!(
                &weight.selector,
                XrdsSceneGltfMorphTargetSelector::Name(name) if name == "Smile"
            )
        })
        .expect("Smile override should be exported");

    assert_eq!(blink.weight, 0.15);
    assert_eq!(smile.weight, 0.9);
}

#[test]
fn export_scene_document_captures_runtime_gltf_playback_in_authoring() {
    let mut app = xrds_test_app();
    let fixture_uri = asset_fixture_path(MORPH_STRESS_TEST_PATH);
    let document = XrdsSceneDocument {
        assets: vec![XrdsSceneAsset {
            id: "asset:morph".to_string(),
            uri: fixture_uri.clone(),
            kind: XrdsSceneAssetKind::Gltf,
        }],
        nodes: vec![XrdsSceneNode {
            id: XrdsSceneNodeId(631),
            parent_id: None,
            name: "Morph".to_string(),
            enabled: true,
            visible: true,
            transform: XrdsSceneTransform::default(),
            payload: XrdsSceneNodePayload::GltfAsset(XrdsSceneGltfAsset {
                asset_id: Some("asset:morph".to_string()),
                asset_uri: fixture_uri.clone(),
                scene_index: 0,
                export_policy: XrdsGltfAssetExportPolicy::KeepExternalReference,
            }),
            editor: XrdsEditorMetadata::default(),
        }],
        ..Default::default()
    };

    {
        let mut xrds = XrdsAPI::attach(&mut app);
        xrds.import_scene_document(&document)
            .expect("scene import should succeed before runtime playback edits");
    }

    let handle = {
        let xrds = XrdsAPI::attach(&mut app);
        xrds.handle_of::<XrdsGltfAsset>(XrdsId(631))
            .expect("imported gltf node should be indexed by id")
    };

    seed_synthetic_gltf_asset(&mut app, &fixture_uri);

    {
        let mut xrds = XrdsAPI::attach(&mut app);
        xrds.play_gltf_animation(
            &handle,
            XrdsGltfAnimationSelector::Name("MorphStressLoop".to_string()),
            XrdsGltfAnimationPlaybackOptions {
                repeat: XrdsAnimationRepeatMode::Once,
                speed: 1.4,
                start_paused: true,
            },
        )
        .expect("runtime gltf playback should succeed before export");
    }

    let exported = {
        let xrds = XrdsAPI::attach(&mut app);
        xrds.export_scene_document()
            .expect("scene export should capture current runtime playback state")
    };

    let authoring = exported
        .gltf_node_authoring
        .get(&631)
        .expect("exported scene should include gltf authoring for runtime playback");
    let playback = authoring
        .default_playback
        .as_ref()
        .expect("current runtime playback should export into default_playback");

    assert!(matches!(
        &playback.selector,
        XrdsSceneGltfAnimationSelector::Name(name) if name == "MorphStressLoop"
    ));
    assert!(matches!(
        playback.repeat,
        XrdsSceneAnimationRepeatMode::Once
    ));
    assert_eq!(playback.speed, 1.4);
    assert!(playback.start_paused);
}

#[test]
fn import_scene_document_preserves_ids_hierarchy_and_material() {
    let mut app = xrds_test_app();
    let document = imported_test_document();

    let imported_ids = {
        let mut xrds = XrdsAPI::attach(&mut app);
        xrds.import_scene_document(&document)
            .expect("scene document import should succeed")
    };

    assert_eq!(imported_ids, vec![XrdsId(100), XrdsId(101)]);

    let xrds = XrdsAPI::attach(&mut app);
    let root_handle = xrds
        .handle_of::<XrdsNode>(XrdsId(100))
        .expect("root node should be indexed by imported id");
    let cube_handle = xrds
        .handle_of::<XrdsCube>(XrdsId(101))
        .expect("cube should be indexed by imported id");

    assert_eq!(xrds.id_of(&root_handle), Some(XrdsId(100)));
    assert_eq!(xrds.id_of(&cube_handle), Some(XrdsId(101)));
    assert_eq!(xrds.parent_id_of(&cube_handle), Some(XrdsId(100)));

    let cube_transform = xrds
        .app
        .world()
        .get::<Transform>(cube_handle.entity())
        .expect("imported cube should have a transform");
    assert_eq!(cube_transform.translation, Vec3::new(4.0, 5.0, 6.0));

    let material = xrds
        .material_params(&cube_handle)
        .expect("imported cube should keep authored material");
    assert_eq!(material.base_color.rgba, [0.2, 0.4, 0.6, 0.8]);
    assert_eq!(material.emissive.rgba, [0.1, 0.0, 0.0, 1.0]);
    assert_eq!(material.opacity, 0.8);
    assert!(material.unlit);
    assert_eq!(material.pbr.metallic, 0.7);
    assert_eq!(material.pbr.perceptual_roughness, 0.25);
    assert_eq!(material.pbr.reflectance, 0.6);
    assert!(material.pbr.double_sided);
    assert_eq!(material.pbr.alpha_mode, XrdsMaterialAlphaMode::Mask);
    assert_eq!(material.pbr.alpha_cutoff, 0.42);

    let material_handle = xrds
        .app
        .world()
        .get::<MeshMaterial3d<StandardMaterial>>(cube_handle.entity())
        .expect("imported cube should have a standard material handle");
    let runtime_material = xrds
        .app
        .world()
        .resource::<Assets<StandardMaterial>>()
        .get(&material_handle.0)
        .expect("standard material asset should exist");
    assert_eq!(runtime_material.metallic, 0.7);
    assert_eq!(runtime_material.perceptual_roughness, 0.25);
    assert_eq!(runtime_material.reflectance, 0.6);
    assert!(runtime_material.double_sided);
    assert_eq!(runtime_material.alpha_mode, AlphaMode::Mask(0.42));

    let root_editor = xrds
        .app
        .world()
        .get::<XrdsStoredEditorMetadata>(root_handle.entity())
        .expect("imported root should keep editor metadata");
    assert_eq!(
        root_editor.0,
        document.node(XrdsSceneNodeId(100)).unwrap().editor
    );

    let cube_editor = xrds
        .app
        .world()
        .get::<XrdsStoredEditorMetadata>(cube_handle.entity())
        .expect("imported cube should keep editor metadata");
    assert_eq!(
        cube_editor.0,
        document.node(XrdsSceneNodeId(101)).unwrap().editor
    );
}

#[test]
fn import_scene_document_rejects_duplicate_runtime_ids() {
    let mut app = xrds_test_app();
    let document = imported_test_document();

    {
        let mut xrds = XrdsAPI::attach(&mut app);
        xrds.import_scene_document(&document)
            .expect("initial scene document import should succeed");
    }

    let error = {
        let mut xrds = XrdsAPI::attach(&mut app);
        xrds.import_scene_document(&document)
            .expect_err("re-importing the same ids should fail")
    };

    assert_eq!(error, XrdsSceneImportError::DuplicateRuntimeId(XrdsId(100)));
}

#[test]
fn export_scene_document_round_trips_built_in_runtime_state() {
    let mut app = xrds_test_app();
    let document = imported_test_document();

    {
        let mut xrds = XrdsAPI::attach(&mut app);
        xrds.import_scene_document(&document)
            .expect("initial scene document import should succeed");
    }

    let exported = {
        let xrds = XrdsAPI::attach(&mut app);
        xrds.export_scene_document_with_metadata(XrdsSceneMetadata {
            name: "Round Trip Export".to_string(),
            ..Default::default()
        })
        .expect("scene document export should succeed")
    };

    assert_eq!(exported.metadata.name, "Round Trip Export");
    assert!(exported.assets.is_empty());
    assert_eq!(exported.nodes.len(), 2);

    let exported_root = exported
        .node(XrdsSceneNodeId(100))
        .expect("root node should be exported");
    assert_eq!(exported_root.parent_id, None);
    assert_eq!(
        exported_root.editor,
        document
            .node(XrdsSceneNodeId(100))
            .expect("root node should exist in input document")
            .editor
    );

    let exported_cube = exported
        .node(XrdsSceneNodeId(101))
        .expect("cube node should be exported");
    assert_eq!(exported_cube.parent_id, Some(XrdsSceneNodeId(100)));
    assert_eq!(exported_cube.transform.translation, [4.0, 5.0, 6.0]);
    assert_eq!(
        exported_cube.editor,
        document
            .node(XrdsSceneNodeId(101))
            .expect("cube node should exist in input document")
            .editor
    );

    let XrdsSceneNodePayload::Cube(cube_payload) = &exported_cube.payload else {
        panic!("expected exported cube payload");
    };
    assert_eq!(cube_payload.size, [2.0, 3.0, 4.0]);
    assert_eq!(cube_payload.material.base_color, [0.2, 0.4, 0.6, 0.8]);
    assert_eq!(cube_payload.material.emissive, [0.1, 0.0, 0.0, 1.0]);
    assert_eq!(cube_payload.material.opacity, 0.8);
    assert!(cube_payload.material.unlit);
    assert_eq!(cube_payload.material.pbr.metallic, 0.7);
    assert_eq!(cube_payload.material.pbr.perceptual_roughness, 0.25);
    assert_eq!(cube_payload.material.pbr.reflectance, 0.6);
    assert!(cube_payload.material.pbr.double_sided);
    assert_eq!(
        cube_payload.material.pbr.alpha_mode,
        XrdsSceneMaterialAlphaMode::Mask
    );
    assert_eq!(cube_payload.material.pbr.alpha_cutoff, 0.42);
}

#[test]
fn export_scene_document_reconstructs_gltf_asset_catalog() {
    let mut app = xrds_test_app();
    let document = imported_gltf_catalog_document();
    let primary_fixture_uri = asset_fixture_path(VALID_GLTF_PATH);
    let secondary_fixture_uri = asset_fixture_path(BROKEN_DEPENDENCY_GLTF_PATH);

    {
        let mut xrds = XrdsAPI::attach(&mut app);
        xrds.import_scene_document(&document)
            .expect("gltf catalog import should succeed");
    }

    let exported = {
        let xrds = XrdsAPI::attach(&mut app);
        xrds.export_scene_document()
            .expect("scene document export should succeed")
    };

    assert_eq!(exported.assets.len(), 2);

    let lamp_asset = exported
        .assets
        .iter()
        .find(|asset| asset.uri == primary_fixture_uri)
        .expect("fixture gltf asset should be reconstructed");
    assert_eq!(lamp_asset.id, "asset:lamp");
    assert_eq!(lamp_asset.kind, XrdsSceneAssetKind::Gltf);

    let triangle_asset = exported
        .assets
        .iter()
        .find(|asset| asset.uri == secondary_fixture_uri)
        .expect("second fixture gltf asset should be reconstructed");
    assert_eq!(triangle_asset.kind, XrdsSceneAssetKind::Gltf);
    assert!(triangle_asset.id.starts_with("gltf-"));
    assert!(!triangle_asset.id.is_empty());
}

#[test]
fn import_scene_document_resolves_catalog_backed_gltf_references() {
    let fixture_uri = asset_fixture_path(VALID_GLTF_PATH);
    let document = XrdsSceneDocument {
        assets: vec![XrdsSceneAsset {
            id: "asset:lamp".to_string(),
            uri: fixture_uri.clone(),
            kind: XrdsSceneAssetKind::Gltf,
        }],
        nodes: vec![XrdsSceneNode {
            id: XrdsSceneNodeId(500),
            parent_id: None,
            name: "Lamp".to_string(),
            enabled: true,
            visible: true,
            transform: XrdsSceneTransform::default(),
            payload: XrdsSceneNodePayload::GltfAsset(XrdsSceneGltfAsset {
                asset_id: Some("asset:lamp".to_string()),
                asset_uri: "missing/Fallback.gltf".to_string(),
                scene_index: 0,
                export_policy: XrdsGltfAssetExportPolicy::KeepExternalReference,
            }),
            editor: XrdsEditorMetadata::default(),
        }],
        ..Default::default()
    };

    let mut app = xrds_test_app();
    {
        let mut xrds = XrdsAPI::attach(&mut app);
        xrds.import_scene_document(&document)
            .expect("catalog-backed gltf import should succeed");
    }

    let exported = {
        let xrds = XrdsAPI::attach(&mut app);
        xrds.export_scene_document()
            .expect("scene document export should succeed")
    };

    let XrdsSceneNodePayload::GltfAsset(asset) = &exported.nodes[0].payload else {
        panic!("expected exported gltf asset payload");
    };
    assert_eq!(asset.asset_uri, fixture_uri);
}

#[test]
fn import_export_scene_document_preserves_gltf_node_authoring() {
    let fixture_uri = asset_fixture_path(MORPH_STRESS_TEST_PATH);
    let document = XrdsSceneDocument {
        assets: vec![XrdsSceneAsset {
            id: "asset:morph".to_string(),
            uri: fixture_uri.clone(),
            kind: XrdsSceneAssetKind::Gltf,
        }],
        nodes: vec![XrdsSceneNode {
            id: XrdsSceneNodeId(600),
            parent_id: None,
            name: "Morph".to_string(),
            enabled: true,
            visible: true,
            transform: XrdsSceneTransform::default(),
            payload: XrdsSceneNodePayload::GltfAsset(XrdsSceneGltfAsset {
                asset_id: Some("asset:morph".to_string()),
                asset_uri: fixture_uri,
                scene_index: 0,
                export_policy: XrdsGltfAssetExportPolicy::KeepExternalReference,
            }),
            editor: XrdsEditorMetadata::default(),
        }],
        gltf_node_authoring: [(
            600,
            XrdsSceneGltfNodeAuthoring {
                default_playback: Some(XrdsSceneGltfPlayback {
                    selector: XrdsSceneGltfAnimationSelector::Name("Run".to_string()),
                    repeat: XrdsSceneAnimationRepeatMode::Loop,
                    speed: 1.25,
                    start_paused: false,
                }),
                morph_target_overrides: vec![XrdsSceneGltfMorphTargetOverride {
                    node: XrdsSceneGltfNodeLocator {
                        node_index_path: vec![1, 2],
                        node_name: Some("Face".to_string()),
                    },
                    mesh_name: Some("HeadMesh".to_string()),
                    weights: vec![
                        XrdsSceneGltfMorphTargetWeight {
                            selector: XrdsSceneGltfMorphTargetSelector::Name("Smile".to_string()),
                            weight: 0.8,
                        },
                        XrdsSceneGltfMorphTargetWeight {
                            selector: XrdsSceneGltfMorphTargetSelector::Index(2),
                            weight: 0.35,
                        },
                    ],
                }],
            },
        )]
        .into_iter()
        .collect(),
        ..Default::default()
    };

    let mut app = xrds_test_app();
    {
        let mut xrds = XrdsAPI::attach(&mut app);
        xrds.import_scene_document(&document)
            .expect("scene document import should preserve gltf authoring");
    }

    let exported = {
        let xrds = XrdsAPI::attach(&mut app);
        xrds.export_scene_document()
            .expect("scene document export should preserve gltf authoring")
    };

    assert_eq!(
        exported.gltf_node_authoring.get(&600),
        document.gltf_node_authoring.get(&600)
    );
}

#[test]
fn import_scene_document_queues_default_gltf_playback_from_authoring() {
    let fixture_uri = asset_fixture_path(MORPH_STRESS_TEST_PATH);
    let document = XrdsSceneDocument {
        assets: vec![XrdsSceneAsset {
            id: "asset:morph".to_string(),
            uri: fixture_uri.clone(),
            kind: XrdsSceneAssetKind::Gltf,
        }],
        nodes: vec![XrdsSceneNode {
            id: XrdsSceneNodeId(610),
            parent_id: None,
            name: "Morph".to_string(),
            enabled: true,
            visible: true,
            transform: XrdsSceneTransform::default(),
            payload: XrdsSceneNodePayload::GltfAsset(XrdsSceneGltfAsset {
                asset_id: Some("asset:morph".to_string()),
                asset_uri: fixture_uri,
                scene_index: 0,
                export_policy: XrdsGltfAssetExportPolicy::KeepExternalReference,
            }),
            editor: XrdsEditorMetadata::default(),
        }],
        gltf_node_authoring: [(
            610,
            XrdsSceneGltfNodeAuthoring {
                default_playback: Some(XrdsSceneGltfPlayback {
                    selector: XrdsSceneGltfAnimationSelector::Name("Run".to_string()),
                    repeat: XrdsSceneAnimationRepeatMode::Once,
                    speed: 1.5,
                    start_paused: true,
                }),
                morph_target_overrides: Vec::new(),
            },
        )]
        .into_iter()
        .collect(),
        ..Default::default()
    };

    let mut app = xrds_test_app();
    {
        let mut xrds = XrdsAPI::attach(&mut app);
        xrds.import_scene_document(&document)
            .expect("scene import should accept authored gltf playback policy");
    }

    let handle = {
        let xrds = XrdsAPI::attach(&mut app);
        xrds.handle_of::<XrdsGltfAsset>(XrdsId(610))
            .expect("imported gltf node should be indexed by id")
    };

    let request = app
        .world()
        .resource::<PendingGltfAnimationRequests>()
        .requests
        .get(&handle.entity())
        .expect("default playback should queue a pending gltf animation request");

    assert!(matches!(
        &request.selector,
        XrdsGltfAnimationSelector::Name(name) if name == "Run"
    ));
    assert!(matches!(
        request.options.repeat,
        XrdsAnimationRepeatMode::Once
    ));
    assert_eq!(request.options.speed, 1.5);
    assert!(request.options.start_paused);
}

#[test]
fn import_scene_document_applies_authored_gltf_morph_target_overrides_when_ready() {
    let fixture_uri = asset_fixture_path(MORPH_STRESS_TEST_PATH);
    let document = XrdsSceneDocument {
        assets: vec![XrdsSceneAsset {
            id: "asset:morph".to_string(),
            uri: fixture_uri.clone(),
            kind: XrdsSceneAssetKind::Gltf,
        }],
        nodes: vec![XrdsSceneNode {
            id: XrdsSceneNodeId(620),
            parent_id: None,
            name: "Morph".to_string(),
            enabled: true,
            visible: true,
            transform: XrdsSceneTransform::default(),
            payload: XrdsSceneNodePayload::GltfAsset(XrdsSceneGltfAsset {
                asset_id: Some("asset:morph".to_string()),
                asset_uri: fixture_uri.clone(),
                scene_index: 0,
                export_policy: XrdsGltfAssetExportPolicy::KeepExternalReference,
            }),
            editor: XrdsEditorMetadata::default(),
        }],
        gltf_node_authoring: [(
            620,
            XrdsSceneGltfNodeAuthoring {
                default_playback: None,
                morph_target_overrides: vec![XrdsSceneGltfMorphTargetOverride {
                    node: XrdsSceneGltfNodeLocator {
                        node_index_path: vec![0],
                        node_name: Some("MorphMeshNode".to_string()),
                    },
                    mesh_name: None,
                    weights: vec![
                        XrdsSceneGltfMorphTargetWeight {
                            selector: XrdsSceneGltfMorphTargetSelector::Index(0),
                            weight: 0.2,
                        },
                        XrdsSceneGltfMorphTargetWeight {
                            selector: XrdsSceneGltfMorphTargetSelector::Name("Smile".to_string()),
                            weight: 0.85,
                        },
                    ],
                }],
            },
        )]
        .into_iter()
        .collect(),
        ..Default::default()
    };

    let mut app = xrds_test_app();
    {
        let mut xrds = XrdsAPI::attach(&mut app);
        xrds.import_scene_document(&document)
            .expect("scene import should accept authored gltf morph overrides");
    }

    let handle = {
        let xrds = XrdsAPI::attach(&mut app);
        xrds.handle_of::<XrdsGltfAsset>(XrdsId(620))
            .expect("imported gltf node should be indexed by id")
    };

    assert!(app
        .world()
        .resource::<PendingGltfMorphTargetOverrideRequests>()
        .entities
        .contains(&handle.entity()));

    let morph_mesh_entity =
        attach_synthetic_morph_mesh_to_root(&mut app, handle.entity(), "MorphMeshNode");
    seed_synthetic_gltf_asset(&mut app, &fixture_uri);

    apply_pending_gltf_morph_target_override_requests_system(app.world_mut());

    assert!(!app
        .world()
        .resource::<PendingGltfMorphTargetOverrideRequests>()
        .entities
        .contains(&handle.entity()));

    let applied = app
        .world()
        .get::<bevy::mesh::morph::MeshMorphWeights>(morph_mesh_entity)
        .expect("realized morph mesh should receive authored override weights");
    assert_eq!(applied.weights(), &[0.2, 0.85]);
}

#[test]
fn built_in_geometry_commit_helpers_update_runtime_and_exported_document() {
    let mut app = xrds_test_app();

    let (cube_id, cylinder_id, sphere_id, plane_id, tetrahedron_id) = {
        let mut xrds = XrdsAPI::attach(&mut app);
        let cube = xrds.spawn(&XrdsCube::new().with_name("Cube"));
        let cylinder = xrds.spawn(&XrdsCylinder::new().with_name("Cylinder"));
        let sphere = xrds.spawn(&XrdsSphere::new().with_name("Sphere"));
        let plane = xrds.spawn(&XrdsPlane3D::new().with_name("Plane"));
        let tetrahedron = xrds.spawn(&XrdsTetrahedron::new().with_name("Tetrahedron"));

        xrds.set_cube_geometry(
            &cube,
            CubeGeometryParams {
                size: [2.0, 3.0, 4.0],
            },
        )
        .set_cylinder_geometry(
            &cylinder,
            CylinderGeometryParams {
                radius: 0.75,
                height: 5.0,
            },
        )
        .set_sphere_geometry(&sphere, SphereGeometryParams { radius: 1.25 })
        .set_plane_geometry(&plane, Plane3DGeometryParams { size: [6.0, 8.0] })
        .set_tetrahedron_geometry(
            &tetrahedron,
            TetrahedronGeometryParams {
                vertices: [
                    [0.0, 0.0, 0.0],
                    [2.0, 0.0, 0.0],
                    [0.0, 3.0, 0.0],
                    [0.0, 0.0, 4.0],
                ],
            },
        );

        (
            xrds.id_of(&cube).expect("cube should have an id"),
            xrds.id_of(&cylinder).expect("cylinder should have an id"),
            xrds.id_of(&sphere).expect("sphere should have an id"),
            xrds.id_of(&plane).expect("plane should have an id"),
            xrds.id_of(&tetrahedron)
                .expect("tetrahedron should have an id"),
        )
    };

    app.update();

    let exported = {
        let xrds = XrdsAPI::attach(&mut app);
        xrds.export_scene_document()
            .expect("scene document export should succeed")
    };

    let XrdsSceneNodePayload::Cube(cube) = &exported
        .node(XrdsSceneNodeId(cube_id.0))
        .expect("cube node should be exported")
        .payload
    else {
        panic!("expected cube payload");
    };
    assert_eq!(cube.size, [2.0, 3.0, 4.0]);

    let XrdsSceneNodePayload::Cylinder(cylinder) = &exported
        .node(XrdsSceneNodeId(cylinder_id.0))
        .expect("cylinder node should be exported")
        .payload
    else {
        panic!("expected cylinder payload");
    };
    assert_eq!(cylinder.radius, 0.75);
    assert_eq!(cylinder.height, 5.0);

    let XrdsSceneNodePayload::Sphere(sphere) = &exported
        .node(XrdsSceneNodeId(sphere_id.0))
        .expect("sphere node should be exported")
        .payload
    else {
        panic!("expected sphere payload");
    };
    assert_eq!(sphere.radius, 1.25);

    let XrdsSceneNodePayload::Plane3D(plane) = &exported
        .node(XrdsSceneNodeId(plane_id.0))
        .expect("plane node should be exported")
        .payload
    else {
        panic!("expected plane payload");
    };
    assert_eq!(plane.size, [6.0, 8.0]);

    let XrdsSceneNodePayload::Tetrahedron(tetrahedron) = &exported
        .node(XrdsSceneNodeId(tetrahedron_id.0))
        .expect("tetrahedron node should be exported")
        .payload
    else {
        panic!("expected tetrahedron payload");
    };
    assert_eq!(
        tetrahedron.vertices,
        [
            [0.0, 0.0, 0.0],
            [2.0, 0.0, 0.0],
            [0.0, 3.0, 0.0],
            [0.0, 0.0, 4.0],
        ]
    );
}

#[test]
fn built_in_light_commit_helpers_update_runtime_and_exported_document() {
    let mut app = xrds_test_app();

    let (point_id, directional_id, spot_id, ambient_id) = {
        let mut xrds = XrdsAPI::attach(&mut app);
        let point = xrds.spawn(&XrdsPointLight::new().with_name("Point"));
        let directional = xrds.spawn(&XrdsDirectionalLight::new().with_name("Directional"));
        let spot = xrds.spawn(&XrdsSpotLight::new().with_name("Spot"));
        let ambient = xrds.spawn(&XrdsAmbientLight::new().with_name("Ambient"));

        xrds.set_point_light_params(
            &point,
            PointLightParams {
                color: XrdsColor::srgb(1.0, 0.25, 0.1),
                intensity: 42_000.0,
                range: 18.0,
                radius: 0.4,
                shadows: true,
            },
        )
        .set_directional_light_params(
            &directional,
            DirectionalLightParams {
                color: XrdsColor::srgb(0.5, 0.6, 1.0),
                illuminance: 12_345.0,
                shadows: true,
            },
        )
        .set_spot_light_params(
            &spot,
            SpotLightParams {
                color: XrdsColor::srgb(0.9, 0.8, 0.5),
                intensity: 7_500.0,
                range: 14.0,
                inner_angle: 0.15,
                outer_angle: 0.6,
                shadows: true,
            },
        )
        .set_ambient_light_params(
            &ambient,
            AmbientLightParams {
                color: XrdsColor::srgb(0.2, 0.3, 0.4),
                brightness: 2.5,
                affects_lightmapped_meshes: true,
            },
        );

        (
            xrds.id_of(&point).expect("point light should have an id"),
            xrds.id_of(&directional)
                .expect("directional light should have an id"),
            xrds.id_of(&spot).expect("spot light should have an id"),
            xrds.id_of(&ambient)
                .expect("ambient light should have an id"),
        )
    };

    app.update();

    let xrds = XrdsAPI::attach(&mut app);
    let point_handle = xrds
        .handle_of::<XrdsPointLight>(point_id)
        .expect("point light handle should resolve");
    let point_runtime = xrds
        .get_component::<PointLight, _>(&point_handle)
        .expect("point light component should exist");
    assert_eq!(point_runtime.intensity, 42_000.0);
    assert_eq!(point_runtime.range, 18.0);
    assert_eq!(point_runtime.radius, 0.4);
    assert!(point_runtime.shadows_enabled);

    let directional_handle = xrds
        .handle_of::<XrdsDirectionalLight>(directional_id)
        .expect("directional light handle should resolve");
    let directional_runtime = xrds
        .get_component::<DirectionalLight, _>(&directional_handle)
        .expect("directional light component should exist");
    assert_eq!(directional_runtime.illuminance, 12_345.0);
    assert!(directional_runtime.shadows_enabled);

    let spot_handle = xrds
        .handle_of::<XrdsSpotLight>(spot_id)
        .expect("spot light handle should resolve");
    let spot_runtime = xrds
        .get_component::<SpotLight, _>(&spot_handle)
        .expect("spot light component should exist");
    assert_eq!(spot_runtime.intensity, 7_500.0);
    assert_eq!(spot_runtime.range, 14.0);
    assert_eq!(spot_runtime.inner_angle, 0.15);
    assert_eq!(spot_runtime.outer_angle, 0.6);
    assert!(spot_runtime.shadows_enabled);

    let ambient_runtime = xrds
        .app
        .world()
        .get_resource::<AmbientLight>()
        .expect("ambient light resource should exist");
    assert_eq!(ambient_runtime.brightness, 2.5);
    assert!(ambient_runtime.affects_lightmapped_meshes);

    let exported = xrds
        .export_scene_document()
        .expect("scene document export should succeed");

    let XrdsSceneNodePayload::PointLight(point) = &exported
        .node(XrdsSceneNodeId(point_id.0))
        .expect("point node should be exported")
        .payload
    else {
        panic!("expected point light payload");
    };
    assert_eq!(point.intensity, 42_000.0);
    assert_eq!(point.range, 18.0);
    assert_eq!(point.radius, 0.4);
    assert!(point.shadows);

    let XrdsSceneNodePayload::DirectionalLight(directional) = &exported
        .node(XrdsSceneNodeId(directional_id.0))
        .expect("directional node should be exported")
        .payload
    else {
        panic!("expected directional light payload");
    };
    assert_eq!(directional.illuminance, 12_345.0);
    assert!(directional.shadows);

    let XrdsSceneNodePayload::SpotLight(spot) = &exported
        .node(XrdsSceneNodeId(spot_id.0))
        .expect("spot node should be exported")
        .payload
    else {
        panic!("expected spot light payload");
    };
    assert_eq!(spot.intensity, 7_500.0);
    assert_eq!(spot.range, 14.0);
    assert_eq!(spot.inner_angle, 0.15);
    assert_eq!(spot.outer_angle, 0.6);
    assert!(spot.shadows);

    let XrdsSceneNodePayload::AmbientLight(ambient) = &exported
        .node(XrdsSceneNodeId(ambient_id.0))
        .expect("ambient node should be exported")
        .payload
    else {
        panic!("expected ambient light payload");
    };
    assert_eq!(ambient.brightness, 2.5);
    assert!(ambient.affects_lightmapped_meshes);
}
