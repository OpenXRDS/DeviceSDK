use super::*;
use bevy::{
    animation::{graph::AnimationGraph, AnimationClip},
    app::App,
    asset::{AssetApp, AssetPlugin},
    camera::Exposure,
    core_pipeline::Skybox,
    gltf::GltfPlugin,
    image::{Image, ImageLoaderSettings, ImagePlugin, ImageSampler},
    pbr::{DistanceFog, FogFalloff, MeshMaterial3d, StandardMaterial},
    prelude::{AlphaMode, EnvironmentMapLight},
    scene::{Scene, ScenePlugin},
    MinimalPlugins,
};
use std::fs;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use xrds_components::{
    XrdsMaterialTextureSamplerParams, XrdsMaterialTextureSlotKind, XrdsMaterialTextureUvParams,
    XrdsMaterialTextureUvTransformMode,
};
use xrds_scene_graph::{
    XrdsEditorMetadata, XrdsGltfAssetExportPolicy, XrdsSceneAnimationRepeatMode, XrdsSceneAsset,
    XrdsSceneAssetKind, XrdsSceneCube, XrdsSceneDocument, XrdsSceneEnvironment,
    XrdsSceneExposureEnvironment, XrdsSceneFogEnvironment, XrdsSceneGltfAnimationSelector,
    XrdsSceneGltfAsset, XrdsSceneGltfMorphTargetOverride, XrdsSceneGltfMorphTargetSelector,
    XrdsSceneGltfMorphTargetWeight, XrdsSceneGltfNodeAuthoring, XrdsSceneGltfNodeLocator,
    XrdsSceneGltfPlayback, XrdsSceneIblEnvironment, XrdsSceneMaterial, XrdsSceneMaterialAlphaMode,
    XrdsSceneMaterialPbrParams, XrdsSceneMaterialTextureSlots, XrdsSceneMetadata, XrdsSceneNode,
    XrdsSceneNodeId, XrdsSceneNodePayload, XrdsSceneSkyboxEnvironment, XrdsSceneTextureFilterMode,
    XrdsSceneTextureRef, XrdsSceneTextureSamplerParams, XrdsSceneTextureUvParams,
    XrdsSceneAudioClip, XrdsSceneTextureUvTransformMode, XrdsSceneTextureWrapMode,
    XrdsSceneTransform, XrdsSourceLink,
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
    app.register_type::<MeshMaterial3d<XrdsRuntimeMaterial>>();
    app.register_type::<AnimationPlayer>();
    app.register_type::<bevy::mesh::morph::MorphWeights>();
    {
        let _ = XrdsAPI::attach(&mut app);
    }
    app.finish();
    app.cleanup();
    app
}

fn expected_runtime_texture_handle(
    asset_server: &AssetServer,
    slot: XrdsMaterialTextureSlotKind,
    texture: &XrdsMaterialTextureRef,
    uri: &str,
) -> bevy::asset::Handle<Image> {
    let uses_srgb = texture_slot_uses_srgb(slot);
    if uses_srgb && texture.sampler == XrdsMaterialTextureSamplerParams::default() {
        return asset_server.load::<Image>(uri.to_string());
    }

    let sampler = runtime_image_sampler_descriptor(texture.sampler);
    asset_server.load_with_settings::<Image, ImageLoaderSettings>(
        uri.to_string(),
        move |settings| {
            settings.is_srgb = uses_srgb;
            settings.sampler = ImageSampler::Descriptor(sampler.clone());
        },
    )
}

fn assert_mat3_approx_eq(actual: Mat3, expected: Mat3) {
    for (actual, expected) in actual
        .to_cols_array()
        .into_iter()
        .zip(expected.to_cols_array())
    {
        assert!(
            (actual - expected).abs() < 1.0e-5,
            "expected {expected}, got {actual}"
        );
    }
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
        assets: vec![XrdsSceneAsset {
            id: "asset:texture-cube-base".to_string(),
            uri: "environment_maps/diffuse.ktx2".to_string(),
            kind: XrdsSceneAssetKind::Texture,
        }],
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
                        textures: XrdsSceneMaterialTextureSlots {
                            base_color: Some(XrdsSceneTextureRef {
                                texture_asset_id: "asset:texture-cube-base".to_string(),
                                uv: XrdsSceneTextureUvParams {
                                    set: 1,
                                    offset: [0.25, 0.5],
                                    scale: [2.0, 1.5],
                                    rotation_deg: 45.0,
                                    transform_mode: XrdsSceneTextureUvTransformMode::Centered,
                                },
                                sampler: XrdsSceneTextureSamplerParams {
                                    wrap_u: XrdsSceneTextureWrapMode::MirroredRepeat,
                                    wrap_v: XrdsSceneTextureWrapMode::ClampToEdge,
                                    min_filter: XrdsSceneTextureFilterMode::Nearest,
                                    mag_filter: XrdsSceneTextureFilterMode::Linear,
                                    mipmap_filter: XrdsSceneTextureFilterMode::Nearest,
                                },
                            }),
                            ..Default::default()
                        },
                        pbr: XrdsSceneMaterialPbrParams {
                            metallic: 0.7,
                            roughness: 0.25,
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

#[path = "tests/builtins.rs"]
mod builtins;
#[path = "tests/document_roundtrip.rs"]
mod document_roundtrip;
#[path = "tests/gltf_document.rs"]
mod gltf_document;
#[path = "tests/gltf_runtime.rs"]
mod gltf_runtime;
#[path = "tests/scene_environment.rs"]
mod scene_environment;
