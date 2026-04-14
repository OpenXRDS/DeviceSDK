use super::*;

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


