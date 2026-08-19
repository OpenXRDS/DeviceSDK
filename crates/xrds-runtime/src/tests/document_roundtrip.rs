use super::*;

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
    assert_eq!(material.pbr.roughness, 0.25);
    assert_eq!(material.pbr.reflectance, 0.6);
    assert!(material.pbr.double_sided);
    assert_eq!(material.pbr.alpha_mode, XrdsMaterialAlphaMode::Mask);
    assert_eq!(material.pbr.alpha_cutoff, 0.42);
    let base_color_texture = material
        .textures
        .base_color
        .as_ref()
        .expect("imported cube should preserve authored base color texture metadata");
    assert_eq!(
        base_color_texture.texture_asset_id,
        "asset:texture-cube-base"
    );
    assert_eq!(base_color_texture.uv.set, 1);
    assert_eq!(base_color_texture.uv.offset, [0.25, 0.5]);
    assert_eq!(base_color_texture.uv.scale, [2.0, 1.5]);
    assert_eq!(base_color_texture.uv.rotation_deg, 45.0);
    assert_eq!(
        base_color_texture.uv.transform_mode,
        XrdsMaterialTextureUvTransformMode::Centered
    );
    assert_eq!(
        base_color_texture.sampler.wrap_u,
        xrds_components::XrdsMaterialTextureWrapMode::MirroredRepeat
    );
    assert_eq!(
        base_color_texture.sampler.wrap_v,
        xrds_components::XrdsMaterialTextureWrapMode::ClampToEdge
    );
    assert_eq!(
        base_color_texture.sampler.min_filter,
        xrds_components::XrdsMaterialTextureFilterMode::Nearest
    );
    assert_eq!(
        base_color_texture.sampler.mipmap_filter,
        xrds_components::XrdsMaterialTextureFilterMode::Nearest
    );

    let material_handle = xrds
        .app
        .world()
        .get::<MeshMaterial3d<XrdsRuntimeMaterial>>(cube_handle.entity())
        .expect("imported cube should have a runtime extended material handle");
    let runtime_material = xrds
        .app
        .world()
        .resource::<Assets<XrdsRuntimeMaterial>>()
        .get(&material_handle.0)
        .expect("runtime material asset should exist");
    let asset_server = xrds.app.world().resource::<AssetServer>();
    let expected_base_color_texture = expected_runtime_texture_handle(
        asset_server,
        XrdsMaterialTextureSlotKind::BaseColor,
        base_color_texture,
        "environment_maps/diffuse.ktx2",
    );
    assert_eq!(runtime_material.base.metallic, 0.7);
    assert_eq!(runtime_material.base.perceptual_roughness, 0.25);
    assert_eq!(runtime_material.base.reflectance, 0.6);
    assert!(runtime_material.base.double_sided);
    assert_eq!(runtime_material.base.alpha_mode, AlphaMode::Mask(0.42));
    assert!(runtime_material.base.base_color_texture.is_none());
    assert_eq!(
        runtime_material
            .extension
            .material_uniform
            .base_color
            .uv_transform,
        runtime_texture_uv_transform(base_color_texture.uv)
    );
    assert_eq!(
        runtime_material
            .extension
            .material_uniform
            .base_color
            .uv_set,
        1
    );
    assert_eq!(
        runtime_material.extension.base_color_texture.as_ref(),
        Some(&expected_base_color_texture)
    );
    assert!(runtime_material
        .extension
        .metallic_roughness_texture
        .is_none());
    assert!(runtime_material.extension.normal_texture.is_none());
    assert!(runtime_material.extension.occlusion_texture.is_none());
    assert!(runtime_material.extension.emissive_texture.is_none());

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
fn import_scene_document_json_loads_saved_document_into_runtime() {
    let mut app = xrds_test_app();
    let document = XrdsSceneDocument {
        metadata: XrdsSceneMetadata {
            environment: Some(XrdsSceneEnvironment {
                ibl: Some(XrdsSceneIblEnvironment {
                    diffuse_asset_id: "asset:ibl-diffuse".to_string(),
                    specular_asset_id: "asset:ibl-specular".to_string(),
                    intensity: 333.0,
                }),
                ..Default::default()
            }),
            ..Default::default()
        },
        assets: vec![
            XrdsSceneAsset {
                id: "asset:ibl-diffuse".to_string(),
                uri: "environment_maps/diffuse.ktx2".to_string(),
                kind: XrdsSceneAssetKind::EnvironmentMap,
            },
            XrdsSceneAsset {
                id: "asset:ibl-specular".to_string(),
                uri: "environment_maps/specular.ktx2".to_string(),
                kind: XrdsSceneAssetKind::EnvironmentMap,
            },
        ],
        nodes: vec![XrdsSceneNode {
            id: XrdsSceneNodeId(515),
            parent_id: None,
            name: "SavedCamera".to_string(),
            enabled: true,
            visible: true,
            transform: XrdsSceneTransform::default(),
            payload: XrdsSceneNodePayload::Camera(Default::default()),
            grabbable: false,
            editor: XrdsEditorMetadata::default(),
            triggers: Vec::new(),
            watchers: Vec::new(),
        }],
        ..Default::default()
    };

    let unique_suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "xrds-runtime-import-scene-document-json-{unique_suffix}.json"
    ));

    document
        .save_json(&path)
        .expect("document should save to json before runtime import");

    let imported_ids = {
        let mut xrds = XrdsAPI::attach(&mut app);
        xrds.import_scene_document_json(&path)
            .expect("saved scene document should load and import")
    };

    fs::remove_file(&path).expect("temporary scene json should be removable");

    assert_eq!(imported_ids, vec![XrdsId(515)]);

    let camera_entity = app
        .world()
        .resource::<XrdsIdIndex>()
        .entity_of(XrdsId(515))
        .expect("saved camera should import into runtime");
    let environment = app
        .world()
        .get::<EnvironmentMapLight>(camera_entity)
        .expect("saved document import should also apply scene environment policy");
    assert_eq!(environment.intensity, 333.0);
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
    assert_eq!(exported.assets.len(), 1);
    assert_eq!(
        exported.assets[0],
        XrdsSceneAsset {
            id: "asset:texture-cube-base".to_string(),
            uri: "environment_maps/diffuse.ktx2".to_string(),
            kind: XrdsSceneAssetKind::Texture,
        }
    );
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
    assert_eq!(cube_payload.material.pbr.roughness, 0.25);
    assert_eq!(cube_payload.material.pbr.reflectance, 0.6);
    assert!(cube_payload.material.pbr.double_sided);
    assert_eq!(
        cube_payload.material.pbr.alpha_mode,
        XrdsSceneMaterialAlphaMode::Mask
    );
    assert_eq!(cube_payload.material.pbr.alpha_cutoff, 0.42);
    let exported_texture = cube_payload
        .material
        .textures
        .base_color
        .as_ref()
        .expect("exported cube should preserve texture bindings");
    assert_eq!(exported_texture.texture_asset_id, "asset:texture-cube-base");
    assert_eq!(exported_texture.uv.set, 1);
    assert_eq!(exported_texture.uv.offset, [0.25, 0.5]);
    assert_eq!(exported_texture.uv.scale, [2.0, 1.5]);
    assert_eq!(exported_texture.uv.rotation_deg, 45.0);
    assert_eq!(
        exported_texture.uv.transform_mode,
        XrdsSceneTextureUvTransformMode::Centered
    );
    assert_eq!(
        exported_texture.sampler.wrap_u,
        XrdsSceneTextureWrapMode::MirroredRepeat
    );
    assert_eq!(
        exported_texture.sampler.wrap_v,
        XrdsSceneTextureWrapMode::ClampToEdge
    );
}


#[test]
fn runtime_texture_uv_transform_rotates_around_center_by_default() {
    let transform = runtime_texture_uv_transform(XrdsMaterialTextureUvParams {
        rotation_deg: 90.0,
        ..Default::default()
    });

    assert_mat3_approx_eq(
        transform,
        Mat3::from_cols_array(&[0.0, 1.0, 0.0, -1.0, 0.0, 0.0, 1.0, 0.0, 1.0]),
    );
}


#[test]
fn runtime_texture_uv_transform_raw_mode_preserves_origin_rotation() {
    let transform = runtime_texture_uv_transform(XrdsMaterialTextureUvParams {
        rotation_deg: 90.0,
        transform_mode: XrdsMaterialTextureUvTransformMode::Raw,
        ..Default::default()
    });

    assert_mat3_approx_eq(
        transform,
        Mat3::from_cols_array(&[0.0, 1.0, 0.0, -1.0, 0.0, 0.0, 0.0, 0.0, 1.0]),
    );
}

#[test]
fn audio_clip_node_survives_import_export_round_trip() {
    let mut app = xrds_test_app();

    let document = XrdsSceneDocument {
        assets: vec![XrdsSceneAsset {
            id: "asset:audio-theme".to_string(),
            uri: "audio/theme.ogg".to_string(),
            kind: XrdsSceneAssetKind::Audio,
        }],
        nodes: vec![XrdsSceneNode {
            id: XrdsSceneNodeId(800),
            parent_id: None,
            name: "ThemeSource".to_string(),
            enabled: true,
            visible: true,
            transform: XrdsSceneTransform {
                translation: [2.0, 0.0, 0.0],
                ..Default::default()
            },
            payload: XrdsSceneNodePayload::AudioClip(XrdsSceneAudioClip {
                asset_id: "asset:audio-theme".to_string(),
                volume: 0.75,
                looped: true,
                spatial: false,
                autoplay: true,
                // Deliberately non-default, and deliberately not all equal to each
                // other: these four survived a round trip only from 2026-08-19.
                // Before that both conversions dropped them — document->runtime
                // omitted them outright, runtime->document reset them with
                // `..Default::default()` — so a test that used defaults here would
                // have passed against the broken code.
                distance_model: XrdsAudioDistanceModel::Exponential,
                min_distance: 2.5,
                max_distance: 30.0,
                rolloff_factor: 1.5,
            }),
            grabbable: false,
            editor: XrdsEditorMetadata::default(),
            triggers: Vec::new(),
            watchers: Vec::new(),
        }],
        ..Default::default()
    };

    let imported_ids = {
        let mut xrds = XrdsAPI::attach(&mut app);
        xrds.import_scene_document(&document)
            .expect("audio clip document import should succeed")
    };

    assert_eq!(imported_ids, vec![XrdsId(800)]);

    app.update();

    let exported = {
        let xrds = XrdsAPI::attach(&mut app);
        xrds.export_scene_document()
            .expect("export after audio clip import should succeed")
    };

    assert_eq!(exported.assets.len(), 1);
    assert_eq!(exported.assets[0].kind, XrdsSceneAssetKind::Audio);
    assert_eq!(exported.assets[0].id, "asset:audio-theme");

    let node = exported
        .node(XrdsSceneNodeId(800))
        .expect("audio clip node should be in exported document");
    assert_eq!(node.transform.translation, [2.0, 0.0, 0.0]);

    let XrdsSceneNodePayload::AudioClip(clip) = &node.payload else {
        panic!("expected AudioClip payload, got {:?}", node.payload);
    };
    assert_eq!(clip.asset_id, "asset:audio-theme");
    assert_eq!(clip.volume, 0.75);
    assert!(clip.looped);
    assert!(!clip.spatial);
    assert!(clip.autoplay);
    assert_eq!(clip.distance_model, XrdsAudioDistanceModel::Exponential);
    assert_eq!(clip.min_distance, 2.5);
    assert_eq!(clip.max_distance, 30.0);
    assert_eq!(clip.rolloff_factor, 1.5);
}

#[test]
fn environment_map_assets_survive_import_export_round_trip() {
    let mut app = xrds_test_app();

    let document = XrdsSceneDocument {
        metadata: XrdsSceneMetadata {
            environment: Some(XrdsSceneEnvironment {
                ibl: Some(XrdsSceneIblEnvironment {
                    diffuse_asset_id: "asset:ibl-diffuse".to_string(),
                    specular_asset_id: "asset:ibl-specular".to_string(),
                    intensity: 750.0,
                }),
                skybox: Some(XrdsSceneSkyboxEnvironment {
                    texture_asset_id: "asset:skybox".to_string(),
                    brightness: 1.2,
                }),
                ..Default::default()
            }),
            ..Default::default()
        },
        assets: vec![
            XrdsSceneAsset {
                id: "asset:ibl-diffuse".to_string(),
                uri: "environment_maps/diffuse.ktx2".to_string(),
                kind: XrdsSceneAssetKind::EnvironmentMap,
            },
            XrdsSceneAsset {
                id: "asset:ibl-specular".to_string(),
                uri: "environment_maps/specular.ktx2".to_string(),
                kind: XrdsSceneAssetKind::EnvironmentMap,
            },
            XrdsSceneAsset {
                id: "asset:skybox".to_string(),
                uri: "environment_maps/specular.ktx2".to_string(),
                kind: XrdsSceneAssetKind::EnvironmentMap,
            },
        ],
        ..Default::default()
    };

    {
        let mut xrds = XrdsAPI::attach(&mut app);
        xrds.import_scene_document(&document)
            .expect("environment map document import should succeed");
    }

    let exported = {
        let xrds = XrdsAPI::attach(&mut app);
        xrds.export_scene_document()
            .expect("export after environment map import should succeed")
    };

    assert_eq!(exported.assets.len(), 3);
    for asset in &exported.assets {
        assert_eq!(
            asset.kind,
            XrdsSceneAssetKind::EnvironmentMap,
            "asset '{}' should round-trip as EnvironmentMap",
            asset.id
        );
    }

    let ibl = exported
        .ibl_environment()
        .expect("IBL environment should be preserved after round-trip");
    assert_eq!(ibl.diffuse_asset_id, "asset:ibl-diffuse");
    assert_eq!(ibl.specular_asset_id, "asset:ibl-specular");
    assert_eq!(ibl.intensity, 750.0);

    let skybox = exported
        .skybox_environment()
        .expect("skybox should be preserved after round-trip");
    assert_eq!(skybox.texture_asset_id, "asset:skybox");
    assert_eq!(skybox.brightness, 1.2);
}

#[test]
fn live_material_edit_appears_in_exported_document() {
    let mut app = xrds_test_app();

    {
        let mut xrds = XrdsAPI::attach(&mut app);
        xrds.import_scene_document(&imported_test_document())
            .expect("import should succeed");
    }

    app.update();

    {
        let mut xrds = XrdsAPI::attach(&mut app);
        let cube = xrds
            .handle_of::<XrdsCube>(XrdsId(101))
            .expect("cube should be indexed");

        xrds.set_material_base_color(&cube, XrdsColor { rgba: [1.0, 0.0, 0.0, 1.0] });
        xrds.set_material_pbr_params(
            &cube,
            XrdsMaterialPbrParams {
                metallic: 0.0,
                roughness: 1.0,
                ..Default::default()
            },
        );
    }

    app.update();

    let exported = {
        let xrds = XrdsAPI::attach(&mut app);
        xrds.export_scene_document()
            .expect("export after material edit should succeed")
    };

    let XrdsSceneNodePayload::Cube(cube) = &exported
        .node(XrdsSceneNodeId(101))
        .expect("cube node should be in export")
        .payload
    else {
        panic!("expected cube payload");
    };

    assert_eq!(cube.material.base_color, [1.0, 0.0, 0.0, 1.0]);
    assert_eq!(cube.material.pbr.metallic, 0.0);
    assert_eq!(cube.material.pbr.roughness, 1.0);
}

#[test]
fn live_rename_appears_in_exported_document() {
    let mut app = xrds_test_app();

    {
        let mut xrds = XrdsAPI::attach(&mut app);
        xrds.import_scene_document(&imported_test_document())
            .expect("import should succeed");
    }

    app.update();

    {
        let mut xrds = XrdsAPI::attach(&mut app);
        let root = xrds
            .handle_of::<XrdsNode>(XrdsId(100))
            .expect("root node should be indexed");

        xrds.queue_update(&root, NamePatch {
            name: "Renamed Root".to_string(),
        });
    }

    app.update();

    let exported = {
        let xrds = XrdsAPI::attach(&mut app);
        xrds.export_scene_document()
            .expect("export after rename should succeed")
    };

    let root = exported
        .node(XrdsSceneNodeId(100))
        .expect("root node should be in export");
    assert_eq!(root.name, "Renamed Root");
}

#[test]
fn light_nodes_survive_import_export_round_trip() {
    let mut app = xrds_test_app();

    let document = XrdsSceneDocument {
        nodes: vec![
            XrdsSceneNode {
                id: XrdsSceneNodeId(900),
                parent_id: None,
                name: "Sun".to_string(),
                enabled: true,
                visible: true,
                transform: XrdsSceneTransform {
                    translation: [0.0, 5.0, 0.0],
                    ..Default::default()
                },
                payload: XrdsSceneNodePayload::DirectionalLight(XrdsSceneDirectionalLight {
                    color: [1.0, 0.9, 0.8, 1.0],
                    illuminance: 8000.0,
                    shadows: true,
                }),
                grabbable: false,
                editor: XrdsEditorMetadata::default(),
                triggers: Vec::new(),
                watchers: Vec::new(),
            },
            XrdsSceneNode {
                id: XrdsSceneNodeId(901),
                parent_id: None,
                name: "Lamp".to_string(),
                enabled: true,
                visible: true,
                transform: XrdsSceneTransform {
                    translation: [2.0, 3.0, 0.0],
                    ..Default::default()
                },
                payload: XrdsSceneNodePayload::PointLight(XrdsScenePointLight {
                    color: [0.8, 0.8, 1.0, 1.0],
                    intensity: 500.0,
                    range: 8.0,
                    radius: 0.1,
                    shadows: false,
                }),
                grabbable: false,
                editor: XrdsEditorMetadata::default(),
                triggers: Vec::new(),
                watchers: Vec::new(),
            },
            XrdsSceneNode {
                id: XrdsSceneNodeId(902),
                parent_id: None,
                name: "Spot".to_string(),
                enabled: true,
                visible: true,
                transform: XrdsSceneTransform::default(),
                payload: XrdsSceneNodePayload::SpotLight(XrdsSceneSpotLight {
                    color: [1.0, 1.0, 0.8, 1.0],
                    intensity: 1200.0,
                    range: 15.0,
                    inner_angle: 0.2,
                    outer_angle: 0.5,
                    shadows: true,
                }),
                grabbable: false,
                editor: XrdsEditorMetadata::default(),
                triggers: Vec::new(),
                watchers: Vec::new(),
            },
            XrdsSceneNode {
                id: XrdsSceneNodeId(903),
                parent_id: None,
                name: "Sky".to_string(),
                enabled: true,
                visible: true,
                transform: XrdsSceneTransform::default(),
                payload: XrdsSceneNodePayload::AmbientLight(XrdsSceneAmbientLight {
                    color: [0.5, 0.5, 0.6, 1.0],
                    brightness: 0.3,
                    affects_baked_lighting: true,
                }),
                grabbable: false,
                editor: XrdsEditorMetadata::default(),
                triggers: Vec::new(),
                watchers: Vec::new(),
            },
        ],
        ..Default::default()
    };

    {
        let mut xrds = XrdsAPI::attach(&mut app);
        xrds.import_scene_document(&document)
            .expect("light nodes import should succeed");
    }

    app.update();

    let exported = {
        let xrds = XrdsAPI::attach(&mut app);
        xrds.export_scene_document()
            .expect("export after light import should succeed")
    };

    let dir_node = exported.node(XrdsSceneNodeId(900)).expect("directional light should export");
    assert_eq!(dir_node.transform.translation, [0.0, 5.0, 0.0]);
    let XrdsSceneNodePayload::DirectionalLight(dir) = &dir_node.payload else {
        panic!("expected DirectionalLight, got {:?}", dir_node.payload);
    };
    assert_eq!(dir.color, [1.0, 0.9, 0.8, 1.0]);
    assert_eq!(dir.illuminance, 8000.0);
    assert!(dir.shadows);

    let pt_node = exported.node(XrdsSceneNodeId(901)).expect("point light should export");
    let XrdsSceneNodePayload::PointLight(pt) = &pt_node.payload else {
        panic!("expected PointLight, got {:?}", pt_node.payload);
    };
    assert_eq!(pt.color, [0.8, 0.8, 1.0, 1.0]);
    assert_eq!(pt.intensity, 500.0);
    assert_eq!(pt.range, 8.0);
    assert_eq!(pt.radius, 0.1);
    assert!(!pt.shadows);

    let spot_node = exported.node(XrdsSceneNodeId(902)).expect("spot light should export");
    let XrdsSceneNodePayload::SpotLight(spot) = &spot_node.payload else {
        panic!("expected SpotLight, got {:?}", spot_node.payload);
    };
    assert_eq!(spot.intensity, 1200.0);
    assert_eq!(spot.inner_angle, 0.2);
    assert_eq!(spot.outer_angle, 0.5);
    assert!(spot.shadows);

    let amb_node = exported.node(XrdsSceneNodeId(903)).expect("ambient light should export");
    let XrdsSceneNodePayload::AmbientLight(amb) = &amb_node.payload else {
        panic!("expected AmbientLight, got {:?}", amb_node.payload);
    };
    assert_eq!(amb.color, [0.5, 0.5, 0.6, 1.0]);
    assert_eq!(amb.brightness, 0.3);
    assert!(amb.affects_baked_lighting);
}

#[test]
fn camera_node_survives_import_export_round_trip() {
    let mut app = xrds_test_app();

    let document = XrdsSceneDocument {
        nodes: vec![XrdsSceneNode {
            id: XrdsSceneNodeId(910),
            parent_id: None,
            name: "MainCam".to_string(),
            enabled: true,
            visible: true,
            transform: XrdsSceneTransform {
                translation: [0.0, 2.0, 5.0],
                ..Default::default()
            },
            payload: XrdsSceneNodePayload::Camera(XrdsSceneCamera {
                projection: XrdsSceneCameraProjection::Perspective {
                    fov_deg: 75.0,
                    near: 0.05,
                    far: Some(500.0),
                    order: 0,
                },
                look_at: None,
            }),
            grabbable: false,
            editor: XrdsEditorMetadata::default(),
            triggers: Vec::new(),
            watchers: Vec::new(),
        }],
        ..Default::default()
    };

    {
        let mut xrds = XrdsAPI::attach(&mut app);
        xrds.import_scene_document(&document)
            .expect("camera node import should succeed");
    }

    app.update();

    let exported = {
        let xrds = XrdsAPI::attach(&mut app);
        xrds.export_scene_document()
            .expect("export after camera import should succeed")
    };

    let cam_node = exported.node(XrdsSceneNodeId(910)).expect("camera should export");
    assert_eq!(cam_node.name, "MainCam");
    assert_eq!(cam_node.transform.translation, [0.0, 2.0, 5.0]);
    let XrdsSceneNodePayload::Camera(cam) = &cam_node.payload else {
        panic!("expected Camera payload, got {:?}", cam_node.payload);
    };
    let XrdsSceneCameraProjection::Perspective { fov_deg, near, far, .. } = cam.projection else {
        panic!("expected Perspective projection, got {:?}", cam.projection);
    };
    assert_eq!(fov_deg, 75.0);
    assert_eq!(near, 0.05);
    assert_eq!(far, Some(500.0));
}

#[test]
fn trigger_bindings_survive_import_export_round_trip() {
    let mut app = xrds_test_app();

    let bindings = vec![XrdsTriggerBinding {
        trigger: XrdsTriggerKind::ZoneEnter,
        track: Some("teleport".to_string()),
        effect: Default::default(),
        disabled: false,
        hand: None,
    }];
    let tracks = vec![xrds_scene_graph::XrdsNamedTrack {
        name: "teleport".to_string(),
        track: xrds_scene_graph::XrdsTrack {
            assets: vec![xrds_scene_graph::XrdsTrackAsset { when_finished: Default::default(),
                target: XrdsActionTarget::SelfNode,
                keys: vec![xrds_scene_graph::XrdsTrackKey {
                    at_secs: 0.0,
                    action: XrdsAction::SetTransform {
                            position: Some([1.0, 0.0, 2.0]),
                            rotation: None,
                            scale: None,
                            duration_secs: 0.0,
                            ease: XrdsEaseCurve::Linear,
                        },
                }],
            }],
            ..Default::default()
        },
    }];

    let document = XrdsSceneDocument {
        nodes: vec![XrdsSceneNode {
            id: XrdsSceneNodeId(920),
            parent_id: None,
            name: "TeleportPad".to_string(),
            enabled: true,
            visible: true,
            transform: XrdsSceneTransform::default(),
            // Deliberately Empty, not InteractionZone: triggers are meant to
            // work on any node regardless of payload kind (that's the whole
            // point of the top-level-field fix from earlier in this design
            // track) — this also sidesteps a separate, pre-existing gap
            // where InteractionZone payloads aren't yet round-trippable
            // through export at all (export_scene_node_in_world in
            // helper.rs has no case for XrdsInteractionZone), which isn't
            // this test's concern to fix.
            payload: XrdsSceneNodePayload::Empty,
            grabbable: false,
            editor: XrdsEditorMetadata::default(),
            triggers: bindings.clone(),
            watchers: Vec::new(),
        }],
        tracks: tracks.clone(),
        ..Default::default()
    };

    {
        let mut xrds = XrdsAPI::attach(&mut app);
        xrds.import_scene_document(&document)
            .expect("trigger-binding node import should succeed");
    }

    app.update();

    // The runtime component should exist regardless of export — this is the
    // actual "spawn the definition, don't enqueue anything at import time"
    // guarantee from the design doc.
    {
        let world = app.world();
        let id_index = world.resource::<XrdsIdIndex>();
        let entity = id_index.entity_of(XrdsId(920)).expect("entity should be indexed");
        let stored = world
            .get::<crate::xrds_api::trigger_action::XrdsTriggerBindings>(entity)
            .expect("XrdsTriggerBindings should be spawned at import");
        assert_eq!(stored.0, bindings);
    }

    let exported = {
        let xrds = XrdsAPI::attach(&mut app);
        xrds.export_scene_document()
            .expect("export after trigger-binding import should succeed")
    };

    let node = exported.node(XrdsSceneNodeId(920)).expect("node should export");
    assert_eq!(node.triggers, bindings);
    // The Track a binding names has to survive too, or the exported document
    // would round-trip a binding that resolves to nothing.
    assert_eq!(exported.tracks, tracks);
}

#[test]
fn text3d_node_survives_import_export_round_trip() {
    let mut app = xrds_test_app();

    let document = XrdsSceneDocument {
        nodes: vec![XrdsSceneNode {
            id: XrdsSceneNodeId(920),
            parent_id: None,
            name: "Label".to_string(),
            enabled: true,
            visible: true,
            transform: XrdsSceneTransform {
                translation: [1.0, 2.0, 0.0],
                ..Default::default()
            },
            payload: XrdsSceneNodePayload::Text(XrdsSceneText {
                text: "Hello XR".to_string(),
                font_size: 32.0,
                color: [0.2, 0.8, 1.0, 1.0],
                alignment: XrdsSceneTextAlignment::Left,
                ..Default::default()
            }),
            grabbable: false,
            editor: XrdsEditorMetadata::default(),
            triggers: Vec::new(),
            watchers: Vec::new(),
        }],
        ..Default::default()
    };

    {
        let mut xrds = XrdsAPI::attach(&mut app);
        xrds.import_scene_document(&document)
            .expect("text3d node import should succeed");
    }

    app.update();

    let exported = {
        let xrds = XrdsAPI::attach(&mut app);
        xrds.export_scene_document()
            .expect("export after text3d import should succeed")
    };

    let text_node = exported.node(XrdsSceneNodeId(920)).expect("text3d node should export");
    assert_eq!(text_node.name, "Label");
    assert_eq!(text_node.transform.translation, [1.0, 2.0, 0.0]);
    let XrdsSceneNodePayload::Text(text) = &text_node.payload else {
        panic!("expected Text payload, got {:?}", text_node.payload);
    };
    assert_eq!(text.text, "Hello XR");
    assert_eq!(text.font_size, 32.0);
    assert_eq!(text.color, [0.2, 0.8, 1.0, 1.0]);
    assert_eq!(text.alignment, XrdsSceneTextAlignment::Left);
}

#[test]
fn panel_templates_survive_import_export_round_trip() {
    // The registry has to come back out for the same reason `tracks` does: a
    // Panel node carries only a `template_id`, so an export that drops the
    // registry produces a document whose panels resolve to nothing. Exactly the
    // failure the `tracks` export fixed, one registry later.
    let mut app = xrds_test_app();

    let panels = vec![xrds_scene_graph::XrdsPanelTemplate {
        id: xrds_scene_graph::XrdsPanelTemplateId(3),
        name: "Console".to_string(),
        elements: vec![xrds_scene_graph::XrdsPanelElement {
            name: "Go".to_string(),
            kind: xrds_scene_graph::XrdsSceneWorldWidget::Button(
                xrds_scene_graph::XrdsSceneWorldButton {
                    label: "Go".to_string(),
                    ..Default::default()
                },
            ),
        }],
        ..Default::default()
    }];

    let document = XrdsSceneDocument {
        nodes: vec![XrdsSceneNode {
            id: XrdsSceneNodeId(930),
            parent_id: None,
            name: "WallPanel".to_string(),
            enabled: true,
            visible: true,
            transform: XrdsSceneTransform::default(),
            payload: XrdsSceneNodePayload::Panel(xrds_scene_graph::XrdsScenePanelInstance {
                template_id: xrds_scene_graph::XrdsPanelTemplateId(3),
                ..Default::default()
            }),
            grabbable: false,
            editor: XrdsEditorMetadata::default(),
            triggers: Vec::new(),
            watchers: Vec::new(),
        }],
        panels: panels.clone(),
        ..Default::default()
    };

    {
        let mut xrds = XrdsAPI::attach(&mut app);
        xrds.import_scene_document(&document).expect("panel import should succeed");
    }
    app.update();

    let exported = {
        let xrds = XrdsAPI::attach(&mut app);
        xrds.export_scene_document().expect("export after panel import should succeed")
    };

    assert_eq!(exported.panels, panels, "the panel registry must survive export");
}

#[test]
fn panel_templates_survive_the_reimport_path_too() {
    // `reimport_scene_in_world` and `XrdsAPI::import_scene_document` do not share
    // a body, so a registry wired into one is not wired into the other. That
    // asymmetry is exactly how `tag_player_anchor_entities` came to be missing
    // from the import path, and asserting only the path used above would let the
    // same gap reopen here.
    let mut app = xrds_test_app();

    let panels = vec![xrds_scene_graph::XrdsPanelTemplate {
        id: xrds_scene_graph::XrdsPanelTemplateId(4),
        name: "Reimported".to_string(),
        ..Default::default()
    }];
    let document = XrdsSceneDocument { panels: panels.clone(), ..Default::default() };

    crate::xrds_api::reimport::reimport_scene_in_world(app.world_mut(), &document)
        .expect("reimport should succeed");
    app.update();

    let exported = {
        let xrds = XrdsAPI::attach(&mut app);
        xrds.export_scene_document().expect("export after reimport should succeed")
    };
    assert_eq!(exported.panels, panels);
}

#[test]
fn importing_a_document_without_panels_clears_a_previous_registry() {
    // The registry is replaced wholesale, matching every other tag_*/sync_*
    // helper: the document is authoritative state, not something to merge into.
    // Merging instead would resurrect templates the author deleted.
    let mut app = xrds_test_app();

    let with_panels = XrdsSceneDocument {
        panels: vec![xrds_scene_graph::XrdsPanelTemplate {
            id: xrds_scene_graph::XrdsPanelTemplateId(5),
            name: "Doomed".to_string(),
            ..Default::default()
        }],
        ..Default::default()
    };
    {
        let mut xrds = XrdsAPI::attach(&mut app);
        xrds.import_scene_document(&with_panels).expect("first import should succeed");
    }
    app.update();

    {
        let mut xrds = XrdsAPI::attach(&mut app);
        xrds.import_scene_document(&XrdsSceneDocument::default())
            .expect("second import should succeed");
    }
    app.update();

    let exported = {
        let xrds = XrdsAPI::attach(&mut app);
        xrds.export_scene_document().expect("export should succeed")
    };
    assert!(exported.panels.is_empty(), "stale templates came back: {:?}", exported.panels);
}
