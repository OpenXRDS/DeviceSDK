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
    assert_eq!(material.pbr.perceptual_roughness, 0.25);
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
            editor: XrdsEditorMetadata::default(),
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
    assert_eq!(cube_payload.material.pbr.perceptual_roughness, 0.25);
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


