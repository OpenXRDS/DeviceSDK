use super::*;

#[test]
fn session_save_as_and_load_round_trip_preserve_document_and_state() {
    let document = persistence_test_document();
    let path = unique_temp_json_path("xrds-scene-session");

    let mut session = XrdsSceneDocumentSession::new(document.clone())
        .expect("session should accept valid document");
    assert!(session.save_path().is_none());
    assert!(!session.is_dirty());

    session
        .save_as(&path)
        .expect("session should save document to requested path");

    assert_eq!(session.save_path(), Some(path.as_path()));
    assert!(!session.is_dirty());

    let loaded =
        XrdsSceneDocumentSession::load_json(&path).expect("session should load from saved file");
    std::fs::remove_file(&path).expect("temporary json file should be removable");

    assert_eq!(loaded.document(), &document);
    assert_eq!(loaded.save_path(), Some(path.as_path()));
    assert!(!loaded.is_dirty());
    assert!(!loaded.can_undo());
    assert!(!loaded.can_redo());
}

#[test]
fn session_tracks_dirty_state_and_supports_undo_redo() {
    let mut session = XrdsSceneDocumentSession::new(persistence_test_document())
        .expect("session should accept valid document");

    session
        .edit(|document| {
            document.metadata.name = "Edited Name".to_string();
        })
        .expect("valid edit should succeed");

    assert_eq!(session.document().metadata.name, "Edited Name");
    assert!(session.is_dirty());
    assert!(session.can_undo());
    assert!(!session.can_redo());

    assert!(session.undo());
    assert_eq!(session.document().metadata.name, "Persistence Test");
    assert!(!session.is_dirty());
    assert!(session.can_redo());

    assert!(session.redo());
    assert_eq!(session.document().metadata.name, "Edited Name");
    assert!(session.is_dirty());
    assert!(session.can_undo());
}

#[test]
fn session_rejects_invalid_edit_and_restores_previous_document() {
    let mut session = XrdsSceneDocumentSession::new(persistence_test_document())
        .expect("session should accept valid document");
    let before = session.document().clone();

    let error = session
        .edit(|document| {
            document.nodes[0].parent_id = Some(document.nodes[0].id);
        })
        .expect_err("invalid edit should be rejected");

    assert_eq!(
        error,
        XrdsSceneDocumentEditError::Validation(XrdsSceneValidationError::SelfParent(
            XrdsSceneNodeId(10)
        ))
    );
    assert_eq!(session.document(), &before);
    assert!(!session.is_dirty());
    assert!(!session.can_undo());
}

#[test]
fn session_save_requires_known_path() {
    let mut session = XrdsSceneDocumentSession::new(persistence_test_document())
        .expect("session should accept valid document");

    let error = session
        .save()
        .expect_err("save without prior path should be rejected");

    match error {
        XrdsSceneDocumentSessionError::MissingSavePath => {}
        other => panic!("expected missing save path error, got {other:?}"),
    }
}

#[test]
fn session_history_limit_bounds_undo_depth() {
    let mut session = XrdsSceneDocumentSession::with_history_limit(persistence_test_document(), 2)
        .expect("session should accept valid document");

    for index in 0..3 {
        session
            .edit(|document| {
                document.metadata.name = format!("Edit {index}");
            })
            .expect("valid edit should succeed");
    }

    assert_eq!(session.document().metadata.name, "Edit 2");
    assert!(session.undo());
    assert_eq!(session.document().metadata.name, "Edit 1");
    assert!(session.undo());
    assert_eq!(session.document().metadata.name, "Edit 0");
    assert!(!session.undo());
}

#[test]
fn session_metadata_operations_participate_in_undo_redo() {
    let mut session = XrdsSceneDocumentSession::new(XrdsSceneDocument {
        nodes: vec![XrdsSceneNode {
            id: XrdsSceneNodeId(10),
            parent_id: None,
            name: "Node".to_string(),
            enabled: true,
            visible: true,
            transform: XrdsSceneTransform::default(),
            payload: XrdsSceneNodePayload::Empty,
            grabbable: false,
            editor: XrdsEditorMetadata::default(),
            triggers: Vec::new(),
        }],
        ..Default::default()
    })
    .expect("session should accept valid document");

    session
        .set_node_tags(
            XrdsSceneNodeId(10),
            vec![" gameplay ".to_string(), "gameplay".to_string()],
        )
        .expect("setting tags should succeed");
    session
        .set_node_layer(XrdsSceneNodeId(10), Some(" Actors ".to_string()))
        .expect("setting layer should succeed");
    session
        .set_node_locked(XrdsSceneNodeId(10), true)
        .expect("setting lock should succeed");
    session
        .set_node_hidden_in_editor(XrdsSceneNodeId(10), true)
        .expect("setting hidden should succeed");
    session
        .set_node_user_property(XrdsSceneNodeId(10), "selected", "true")
        .expect("setting user property should succeed");
    session
        .set_node_source_link(
            XrdsSceneNodeId(10),
            Some(XrdsSourceLink {
                asset_id: Some("asset:lamp".to_string()),
                source_node: Some("RootNode".to_string()),
                import_revision: None,
            }),
        )
        .expect("setting source link should succeed");

    let metadata = &session
        .document()
        .node(XrdsSceneNodeId(10))
        .expect("node should exist")
        .editor;
    assert_eq!(metadata.tags, vec!["gameplay".to_string()]);
    assert_eq!(metadata.layer.as_deref(), Some("Actors"));
    assert!(metadata.locked);
    assert!(metadata.hidden_in_editor);
    assert_eq!(
        metadata.user_properties.get("selected"),
        Some(&"true".to_string())
    );
    assert_eq!(
        metadata.source,
        Some(XrdsSourceLink {
            asset_id: Some("asset:lamp".to_string()),
            source_node: Some("RootNode".to_string()),
            import_revision: None,
        })
    );

    assert!(session.undo());
    assert!(session
        .document()
        .node(XrdsSceneNodeId(10))
        .expect("node should exist after undo")
        .editor
        .source
        .is_none());

    assert!(session.redo());
    assert_eq!(
        session
            .document()
            .node(XrdsSceneNodeId(10))
            .expect("node should exist after redo")
            .editor
            .source,
        Some(XrdsSourceLink {
            asset_id: Some("asset:lamp".to_string()),
            source_node: Some("RootNode".to_string()),
            import_revision: None,
        })
    );
}

#[test]
fn session_material_operations_participate_in_undo_redo() {
    let mut session = XrdsSceneDocumentSession::new(XrdsSceneDocument {
        nodes: vec![XrdsSceneNode {
            id: XrdsSceneNodeId(12),
            parent_id: None,
            name: "Sphere".to_string(),
            enabled: true,
            visible: true,
            transform: XrdsSceneTransform::default(),
            payload: XrdsSceneNodePayload::Sphere(XrdsSceneSphere::default()),
            grabbable: false,
            editor: XrdsEditorMetadata::default(),
            triggers: Vec::new(),
        }],
        ..Default::default()
    })
    .expect("session should accept valid document");

    session
        .set_node_material_base_color(XrdsSceneNodeId(12), XrdsColor::srgb(0.3, 0.7, 0.95))
        .expect("setting base color should succeed");
    session
        .set_node_material_pbr(
            XrdsSceneNodeId(12),
            XrdsSceneMaterialPbrParams {
                metallic: 0.9,
                roughness: 0.15,
                reflectance: 0.8,
                double_sided: true,
                alpha_mode: XrdsSceneMaterialAlphaMode::Blend,
                alpha_cutoff: 0.25,
            },
        )
        .expect("setting pbr should succeed");
    session
        .set_node_material_opacity(XrdsSceneNodeId(12), 0.45)
        .expect("setting opacity should succeed");

    let material = session
        .document()
        .node_material(XrdsSceneNodeId(12))
        .expect("material should exist");
    assert_eq!(material.base_color, [0.3, 0.7, 0.95, 1.0]);
    assert_eq!(material.opacity, 0.45);
    assert_eq!(material.pbr.metallic, 0.9);
    assert_eq!(material.pbr.alpha_mode, XrdsSceneMaterialAlphaMode::Blend);
    assert!(session.can_undo());

    assert!(session.undo());
    let material_after_undo = session
        .document()
        .node_material(XrdsSceneNodeId(12))
        .expect("material should still exist after undo");
    assert_eq!(material_after_undo.opacity, 1.0);
    assert_eq!(material_after_undo.pbr.metallic, 0.9);

    assert!(session.undo());
    let material_after_second_undo = session
        .document()
        .node_material(XrdsSceneNodeId(12))
        .expect("material should still exist after second undo");
    assert_eq!(material_after_second_undo.base_color, [0.3, 0.7, 0.95, 1.0]);
    assert_eq!(
        material_after_second_undo.pbr,
        XrdsSceneMaterialPbrParams::default()
    );

    assert!(session.redo());
    assert!(session.redo());
    let material_after_redo = session
        .document()
        .node_material(XrdsSceneNodeId(12))
        .expect("material should exist after redo");
    assert_eq!(material_after_redo.opacity, 0.45);
    assert_eq!(material_after_redo.pbr.metallic, 0.9);
    assert_eq!(
        material_after_redo.pbr.alpha_mode,
        XrdsSceneMaterialAlphaMode::Blend
    );
}

#[test]
fn session_remove_asset_and_rebind_asset_participate_in_undo_redo() {
    let mut session = XrdsSceneDocumentSession::new(XrdsSceneDocument {
        assets: vec![XrdsSceneAsset {
            id: "asset:lamp".to_string(),
            uri: "catalog/Lamp.gltf".to_string(),
            kind: XrdsSceneAssetKind::Gltf,
        }],
        nodes: vec![XrdsSceneNode {
            id: XrdsSceneNodeId(42),
            parent_id: None,
            name: "Lamp".to_string(),
            enabled: true,
            visible: true,
            transform: XrdsSceneTransform::default(),
            payload: XrdsSceneNodePayload::GltfAsset(XrdsSceneGltfAsset {
                asset_id: Some("asset:lamp".to_string()),
                asset_uri: "catalog/Lamp.gltf".to_string(),
                scene_index: 0,
                export_policy: XrdsGltfAssetExportPolicy::KeepExternalReference,
            }),
            grabbable: false,
            editor: XrdsEditorMetadata::default(),
            triggers: Vec::new(),
        }],
        ..Default::default()
    })
    .expect("session should accept valid document");

    session
        .rebind_gltf_asset("asset:lamp", "catalog/Lamp_v2.gltf")
        .expect("rebind should succeed");
    assert_eq!(
        session
            .document()
            .asset("asset:lamp")
            .expect("asset should exist")
            .uri,
        "catalog/Lamp_v2.gltf"
    );

    let removal = session
        .remove_asset(
            "asset:lamp",
            XrdsSceneAssetRemovalPolicy::DetachReferencingNodes,
        )
        .expect("detaching removal should succeed");
    assert_eq!(removal.detached_node_ids, vec![XrdsSceneNodeId(42)]);
    assert!(session.document().assets.is_empty());

    assert!(session.undo());
    assert_eq!(
        session
            .document()
            .asset("asset:lamp")
            .expect("asset should be restored after undo")
            .uri,
        "catalog/Lamp_v2.gltf"
    );

    assert!(session.undo());
    assert_eq!(
        session
            .document()
            .asset("asset:lamp")
            .expect("asset should return to original uri after second undo")
            .uri,
        "catalog/Lamp.gltf"
    );

    assert!(session.redo());
    assert_eq!(
        session
            .document()
            .asset("asset:lamp")
            .expect("asset should be rebound again after redo")
            .uri,
        "catalog/Lamp_v2.gltf"
    );

    assert!(session.redo());
    assert!(session.document().asset("asset:lamp").is_none());
    let node = session
        .document()
        .node(XrdsSceneNodeId(42))
        .expect("node should still exist after detach removal");
    let XrdsSceneNodePayload::GltfAsset(asset) = &node.payload else {
        panic!("expected gltf asset payload");
    };
    assert_eq!(asset.asset_id, None);
    assert_eq!(asset.asset_uri, "catalog/Lamp_v2.gltf");
}

#[test]
fn session_rename_asset_id_participates_in_undo_redo() {
    let mut session = XrdsSceneDocumentSession::new(XrdsSceneDocument {
        assets: vec![XrdsSceneAsset {
            id: "asset:lamp".to_string(),
            uri: "catalog/Lamp.gltf".to_string(),
            kind: XrdsSceneAssetKind::Gltf,
        }],
        nodes: vec![XrdsSceneNode {
            id: XrdsSceneNodeId(42),
            parent_id: None,
            name: "Lamp".to_string(),
            enabled: true,
            visible: true,
            transform: XrdsSceneTransform::default(),
            payload: XrdsSceneNodePayload::GltfAsset(XrdsSceneGltfAsset {
                asset_id: Some("asset:lamp".to_string()),
                asset_uri: "catalog/Lamp.gltf".to_string(),
                scene_index: 0,
                export_policy: XrdsGltfAssetExportPolicy::KeepExternalReference,
            }),
            grabbable: false,
            editor: XrdsEditorMetadata::default(),
            triggers: Vec::new(),
        }],
        ..Default::default()
    })
    .expect("session should accept valid document");

    session
        .rename_asset_id("asset:lamp", "asset:lamp-renamed")
        .expect("asset id rename should succeed");

    assert!(session.document().asset("asset:lamp").is_none());
    assert!(session.document().asset("asset:lamp-renamed").is_some());

    let node = session
        .document()
        .node(XrdsSceneNodeId(42))
        .expect("node should remain after rename");
    let XrdsSceneNodePayload::GltfAsset(asset) = &node.payload else {
        panic!("expected gltf asset payload");
    };
    assert_eq!(asset.asset_id.as_deref(), Some("asset:lamp-renamed"));

    assert!(session.undo());
    assert!(session.document().asset("asset:lamp").is_some());
    assert!(session.document().asset("asset:lamp-renamed").is_none());

    let node = session
        .document()
        .node(XrdsSceneNodeId(42))
        .expect("node should remain after undo");
    let XrdsSceneNodePayload::GltfAsset(asset) = &node.payload else {
        panic!("expected gltf asset payload");
    };
    assert_eq!(asset.asset_id.as_deref(), Some("asset:lamp"));

    assert!(session.redo());
    assert!(session.document().asset("asset:lamp").is_none());
    assert!(session.document().asset("asset:lamp-renamed").is_some());
}

#[test]
fn session_place_and_retarget_gltf_asset_support_undo_and_redo() {
    let mut session = XrdsSceneDocumentSession::new(XrdsSceneDocument {
        assets: vec![
            XrdsSceneAsset {
                id: "asset:lamp".to_string(),
                uri: "catalog/Lamp.gltf".to_string(),
                kind: XrdsSceneAssetKind::Gltf,
            },
            XrdsSceneAsset {
                id: "asset:triangle".to_string(),
                uri: "catalog/Triangle.gltf".to_string(),
                kind: XrdsSceneAssetKind::Gltf,
            },
        ],
        nodes: vec![XrdsSceneNode {
            id: XrdsSceneNodeId(10),
            parent_id: None,
            name: "Root".to_string(),
            enabled: true,
            visible: true,
            transform: XrdsSceneTransform::default(),
            payload: XrdsSceneNodePayload::Empty,
            grabbable: false,
            editor: XrdsEditorMetadata::default(),
            triggers: Vec::new(),
        }],
        ..Default::default()
    })
    .expect("session should accept valid document");

    let placed_id = session
        .place_gltf_asset(XrdsSceneGltfPlacement {
            asset_id: "asset:lamp".to_string(),
            node_id: None,
            parent_id: Some(XrdsSceneNodeId(10)),
            name: "Lamp".to_string(),
            enabled: true,
            visible: true,
            transform: XrdsSceneTransform::default(),
            scene_index: 0,
            export_policy: XrdsGltfAssetExportPolicy::KeepExternalReference,
            editor: XrdsEditorMetadata::default(),
        })
        .expect("placing gltf asset should succeed");

    session
        .retarget_gltf_asset(placed_id, "asset:triangle", 1)
        .expect("retargeting gltf asset should succeed");

    let placed_node = session
        .document()
        .node(placed_id)
        .expect("placed node should exist");
    let XrdsSceneNodePayload::GltfAsset(asset) = &placed_node.payload else {
        panic!("expected gltf asset payload");
    };
    assert_eq!(asset.asset_id.as_deref(), Some("asset:triangle"));
    assert_eq!(asset.asset_uri, "catalog/Triangle.gltf");
    assert_eq!(asset.scene_index, 1);

    assert!(session.undo());
    let placed_node = session
        .document()
        .node(placed_id)
        .expect("placed node should still exist after undo");
    let XrdsSceneNodePayload::GltfAsset(asset) = &placed_node.payload else {
        panic!("expected gltf asset payload");
    };
    assert_eq!(asset.asset_id.as_deref(), Some("asset:lamp"));
    assert_eq!(asset.asset_uri, "catalog/Lamp.gltf");
    assert_eq!(asset.scene_index, 0);

    assert!(session.redo());
    let placed_node = session
        .document()
        .node(placed_id)
        .expect("placed node should exist after redo");
    let XrdsSceneNodePayload::GltfAsset(asset) = &placed_node.payload else {
        panic!("expected gltf asset payload");
    };
    assert_eq!(asset.asset_id.as_deref(), Some("asset:triangle"));
    assert_eq!(asset.asset_uri, "catalog/Triangle.gltf");
    assert_eq!(asset.scene_index, 1);
}

#[test]
fn session_register_and_ensure_gltf_asset_participate_in_undo_redo() {
    let mut session = XrdsSceneDocumentSession::new(XrdsSceneDocument::default())
        .expect("session should accept empty valid document");

    let registered = session
        .register_gltf_asset("asset:lamp", "catalog/Lamp.gltf")
        .expect("explicit catalog registration should succeed");
    assert_eq!(registered.id, "asset:lamp");
    assert_eq!(session.document().assets.len(), 1);

    let ensured = session
        .ensure_gltf_asset(None::<String>, "catalog/Chair.gltf")
        .expect("ensure should create missing catalog asset");
    assert!(ensured.created);
    assert_eq!(session.document().assets.len(), 2);

    let reused = session
        .ensure_gltf_asset(None::<String>, "catalog/Chair.gltf")
        .expect("ensure should reuse existing asset by uri");
    assert!(!reused.created);
    assert_eq!(session.document().assets.len(), 2);

    assert!(session.undo());
    assert_eq!(session.document().assets.len(), 1);
    assert!(session.document().asset("asset:lamp").is_some());

    assert!(session.undo());
    assert!(session.document().assets.is_empty());

    assert!(session.redo());
    assert!(session.document().asset("asset:lamp").is_some());

    assert!(session.redo());
    assert_eq!(session.document().assets.len(), 2);
    assert!(session.document().asset(&ensured.asset.id).is_some());
}

#[test]
fn session_register_and_ensure_texture_assets_participate_in_undo_redo() {
    let mut session = XrdsSceneDocumentSession::new(XrdsSceneDocument::default())
        .expect("session should accept empty valid document");

    let first = session
        .register_texture_asset("asset:texture-bricks", "textures/bricks_basecolor.ktx2")
        .expect("explicit texture registration should succeed");
    assert_eq!(first.kind, XrdsSceneAssetKind::Texture);

    let second = session
        .ensure_texture_asset(None::<String>, "textures/wall_normal.ktx2")
        .expect("ensure should create a second texture asset");
    assert!(second.created);
    assert_eq!(session.document().assets.len(), 2);

    assert!(session.undo());
    assert_eq!(session.document().assets.len(), 1);
    assert!(session.document().asset("asset:texture-bricks").is_some());

    assert!(session.undo());
    assert!(session.document().assets.is_empty());

    assert!(session.redo());
    assert!(session.document().asset("asset:texture-bricks").is_some());

    assert!(session.redo());
    assert_eq!(session.document().assets.len(), 2);
    assert!(session.document().asset(&second.asset.id).is_some());
}

#[test]
fn session_gltf_authoring_operations_participate_in_undo_redo() {
    let mut session = XrdsSceneDocumentSession::new(persistence_test_document())
        .expect("session should accept valid document");

    session
        .set_gltf_default_playback(
            XrdsSceneNodeId(11),
            Some(XrdsSceneGltfPlayback {
                selector: XrdsSceneGltfAnimationSelector::Name("Wave".to_string()),
                repeat: XrdsSceneAnimationRepeatMode::Once,
                speed: 0.75,
                start_paused: false,
            }),
        )
        .expect("gltf playback edit should succeed");
    session
        .set_gltf_morph_target_weight(
            XrdsSceneNodeId(11),
            XrdsSceneGltfNodeLocator {
                node_index_path: vec![2],
                node_name: Some("Face".to_string()),
            },
            Some("FaceMesh".to_string()),
            XrdsSceneGltfMorphTargetSelector::Index(1),
            0.4,
        )
        .expect("gltf morph edit should succeed");

    let authoring = session
        .document()
        .gltf_node_authoring(XrdsSceneNodeId(11))
        .expect("node should exist")
        .expect("authoring should exist after edits");
    assert!(authoring.default_playback.is_some());
    assert_eq!(authoring.morph_target_overrides.len(), 1);

    assert!(session.undo());
    let authoring_after_undo = session
        .document()
        .gltf_node_authoring(XrdsSceneNodeId(11))
        .expect("node should still exist after undo")
        .expect("playback authoring should remain after first undo");
    assert!(authoring_after_undo.default_playback.is_some());
    assert!(authoring_after_undo.morph_target_overrides.is_empty());

    assert!(session.undo());
    let authoring_after_second_undo = session
        .document()
        .gltf_node_authoring(XrdsSceneNodeId(11))
        .expect("node should still exist after second undo");
    assert!(authoring_after_second_undo.is_none());

    assert!(session.redo());
    assert!(session.redo());
    let authoring_after_redo = session
        .document()
        .gltf_node_authoring(XrdsSceneNodeId(11))
        .expect("node should still exist after redo")
        .expect("authoring should exist after redo");
    assert_eq!(authoring_after_redo.morph_target_overrides.len(), 1);
    assert!(matches!(
        authoring_after_redo.default_playback.as_ref().map(|playback| &playback.selector),
        Some(XrdsSceneGltfAnimationSelector::Name(name)) if name == "Wave"
    ));
}