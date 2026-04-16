use super::*;

#[test]
fn runtime_projection_resolves_gltf_asset_uri_from_catalog_reference() {
    let document = XrdsSceneDocument {
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
                asset_uri: "fallback/OldLamp.gltf".to_string(),
                scene_index: 0,
                export_policy: XrdsGltfAssetExportPolicy::KeepExternalReference,
            }),
            editor: XrdsEditorMetadata::default(),
        }],
        ..Default::default()
    };

    let runtime_nodes = document
        .to_runtime_nodes()
        .expect("catalog-backed gltf document should validate");

    let XrdsSceneRuntimeComponent::GltfAsset(asset) = &runtime_nodes[0].component else {
        panic!("expected gltf runtime component");
    };
    assert_eq!(asset.gltf_asset_path, "catalog/Lamp.gltf");
    assert_eq!(
        runtime_nodes[0]
            .editor
            .source
            .as_ref()
            .and_then(|source| source.asset_id.as_deref()),
        Some("asset:lamp")
    );
}

#[test]
fn runtime_projection_falls_back_to_embedded_gltf_uri_when_catalog_entry_is_missing() {
    let document = XrdsSceneDocument {
        nodes: vec![XrdsSceneNode {
            id: XrdsSceneNodeId(42),
            parent_id: None,
            name: "Lamp".to_string(),
            enabled: true,
            visible: true,
            transform: XrdsSceneTransform::default(),
            payload: XrdsSceneNodePayload::GltfAsset(XrdsSceneGltfAsset {
                asset_id: Some("asset:missing".to_string()),
                asset_uri: "fallback/OldLamp.gltf".to_string(),
                scene_index: 0,
                export_policy: XrdsGltfAssetExportPolicy::KeepExternalReference,
            }),
            editor: XrdsEditorMetadata::default(),
        }],
        ..Default::default()
    };

    let resolved = document.resolve_gltf_asset(match &document.nodes[0].payload {
        XrdsSceneNodePayload::GltfAsset(asset) => asset,
        _ => panic!("expected gltf asset payload"),
    });

    assert_eq!(resolved.asset_uri, "fallback/OldLamp.gltf");
    assert_eq!(
        resolved.source,
        XrdsSceneAssetResolutionSource::EmbeddedFallback
    );
}

#[test]
fn document_place_gltf_asset_uses_catalog_reference_and_allocates_node_id() {
    let mut document = XrdsSceneDocument {
        assets: vec![XrdsSceneAsset {
            id: "asset:lamp".to_string(),
            uri: "catalog/Lamp.gltf".to_string(),
            kind: XrdsSceneAssetKind::Gltf,
        }],
        nodes: vec![XrdsSceneNode {
            id: XrdsSceneNodeId(10),
            parent_id: None,
            name: "Root".to_string(),
            enabled: true,
            visible: true,
            transform: XrdsSceneTransform::default(),
            payload: XrdsSceneNodePayload::Empty,
            editor: XrdsEditorMetadata::default(),
        }],
        ..Default::default()
    };

    let node_id = document
        .place_gltf_asset(XrdsSceneGltfPlacement {
            asset_id: "asset:lamp".to_string(),
            node_id: None,
            parent_id: Some(XrdsSceneNodeId(10)),
            name: "Placed Lamp".to_string(),
            enabled: true,
            visible: true,
            transform: XrdsSceneTransform::default(),
            scene_index: 0,
            export_policy: XrdsGltfAssetExportPolicy::KeepExternalReference,
            editor: XrdsEditorMetadata::default(),
        })
        .expect("placing a catalog asset should succeed");

    assert_eq!(node_id, XrdsSceneNodeId(11));

    let node = document.node(node_id).expect("placed node should exist");
    let XrdsSceneNodePayload::GltfAsset(asset) = &node.payload else {
        panic!("expected gltf asset payload");
    };

    assert_eq!(asset.asset_id.as_deref(), Some("asset:lamp"));
    assert_eq!(asset.asset_uri, "catalog/Lamp.gltf");
    assert_eq!(node.parent_id, Some(XrdsSceneNodeId(10)));
    assert_eq!(
        node.editor
            .source
            .as_ref()
            .and_then(|source| source.asset_id.as_deref()),
        Some("asset:lamp")
    );
}

#[test]
fn register_gltf_asset_adds_catalog_entry_and_allows_duplicate_uri_aliases() {
    let mut document = XrdsSceneDocument::default();

    let created = document
        .register_gltf_asset("asset:lamp", "catalog/Lamp.gltf")
        .expect("registering a new gltf asset should succeed");
    assert_eq!(created.id, "asset:lamp");
    assert_eq!(created.uri, "catalog/Lamp.gltf");
    assert_eq!(created.kind, XrdsSceneAssetKind::Gltf);
    assert_eq!(document.assets, vec![created.clone()]);

    let duplicate_id = document
        .register_gltf_asset("asset:lamp", "catalog/Other.gltf")
        .expect_err("duplicate asset id should be rejected");
    assert_eq!(
        duplicate_id,
        XrdsSceneAssetWorkflowError::DuplicateAssetId("asset:lamp".to_string())
    );

    let alias = document
        .register_gltf_asset("asset:other", "catalog/Lamp.gltf")
        .expect("duplicate same-kind uri should be allowed for explicit aliases");
    assert_eq!(alias.id, "asset:other");
    assert_eq!(alias.uri, "catalog/Lamp.gltf");
    assert_eq!(document.assets.len(), 2);
}

#[test]
fn ensure_gltf_asset_reuses_existing_uri_and_generates_id_when_needed() {
    let mut document = XrdsSceneDocument {
        assets: vec![XrdsSceneAsset {
            id: "asset:lamp".to_string(),
            uri: "catalog/Lamp.gltf".to_string(),
            kind: XrdsSceneAssetKind::Gltf,
        }],
        ..Default::default()
    };

    let reused = document
        .ensure_gltf_asset(Some("asset:anything".to_string()), "catalog/Lamp.gltf")
        .expect("existing uri should be reused");
    assert!(!reused.created);
    assert_eq!(reused.asset.id, "asset:lamp");
    assert_eq!(document.assets.len(), 1);

    let created = document
        .ensure_gltf_asset(None::<String>, "catalog/Chair.gltf")
        .expect("missing uri should create a new catalog entry");
    assert!(created.created);
    assert_eq!(created.asset.uri, "catalog/Chair.gltf");
    assert_eq!(created.asset.kind, XrdsSceneAssetKind::Gltf);
    assert!(created.asset.id.starts_with("asset:gltf-chair"));
    assert!(document.asset(&created.asset.id).is_some());
}

#[test]
fn ensure_gltf_asset_reuses_existing_alias_instead_of_creating_another_entry() {
    let mut document = XrdsSceneDocument {
        assets: vec![
            XrdsSceneAsset {
                id: "asset:lamp-a".to_string(),
                uri: "catalog/Lamp.gltf".to_string(),
                kind: XrdsSceneAssetKind::Gltf,
            },
            XrdsSceneAsset {
                id: "asset:lamp-b".to_string(),
                uri: "catalog/Lamp.gltf".to_string(),
                kind: XrdsSceneAssetKind::Gltf,
            },
        ],
        ..Default::default()
    };

    let reused = document
        .ensure_gltf_asset(Some("asset:anything".to_string()), "catalog/Lamp.gltf")
        .expect("existing same-kind alias should still be reused by ensure");
    assert!(!reused.created);
    assert_eq!(reused.asset.id, "asset:lamp-a");
    assert_eq!(document.assets.len(), 2);
}

#[test]
fn ensure_texture_asset_reuses_existing_alias_instead_of_creating_another_entry() {
    let mut document = XrdsSceneDocument {
        assets: vec![
            XrdsSceneAsset {
                id: "asset:texture-a".to_string(),
                uri: "textures/bricks_basecolor.ktx2".to_string(),
                kind: XrdsSceneAssetKind::Texture,
            },
            XrdsSceneAsset {
                id: "asset:texture-b".to_string(),
                uri: "textures/bricks_basecolor.ktx2".to_string(),
                kind: XrdsSceneAssetKind::Texture,
            },
        ],
        ..Default::default()
    };

    let reused = document
        .ensure_texture_asset(None::<String>, "textures/bricks_basecolor.ktx2")
        .expect("existing same-kind alias should still be reused by ensure");
    assert!(!reused.created);
    assert_eq!(reused.asset.id, "asset:texture-a");
    assert_eq!(document.assets.len(), 2);
}

#[test]
fn rebind_asset_allows_duplicate_uri_aliases_within_same_kind() {
    let mut document = XrdsSceneDocument {
        assets: vec![
            XrdsSceneAsset {
                id: "asset:texture-a".to_string(),
                uri: "textures/a.ktx2".to_string(),
                kind: XrdsSceneAssetKind::Texture,
            },
            XrdsSceneAsset {
                id: "asset:texture-b".to_string(),
                uri: "textures/b.ktx2".to_string(),
                kind: XrdsSceneAssetKind::Texture,
            },
            XrdsSceneAsset {
                id: "asset:gltf-b".to_string(),
                uri: "models/b.glb".to_string(),
                kind: XrdsSceneAssetKind::Gltf,
            },
        ],
        ..Default::default()
    };

    let same_kind_alias = document
        .rebind_asset("asset:texture-b", "textures/a.ktx2")
        .expect("rebind should allow duplicate uri within the same kind for explicit aliases");
    assert_eq!(same_kind_alias.asset_id, "asset:texture-b");
    assert_eq!(same_kind_alias.previous_uri, "textures/b.ktx2");
    assert_eq!(same_kind_alias.new_uri, "textures/a.ktx2");
    assert!(same_kind_alias.rebound_node_ids.is_empty());

    let cross_kind = document
        .rebind_asset("asset:gltf-b", "models/a.glb")
        .expect("rebind should allow sharing a uri across different kinds");
    assert_eq!(cross_kind.asset_id, "asset:gltf-b");
    assert_eq!(cross_kind.previous_uri, "models/b.glb");
    assert_eq!(cross_kind.new_uri, "models/a.glb");
    assert!(cross_kind.rebound_node_ids.is_empty());
}

#[test]
fn remove_asset_rejects_when_policy_requires_no_references() {
    let mut document = XrdsSceneDocument {
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
            editor: XrdsEditorMetadata::default(),
        }],
        ..Default::default()
    };

    let error = document
        .remove_asset(
            "asset:lamp",
            XrdsSceneAssetRemovalPolicy::RejectIfReferenced,
        )
        .expect_err("referenced asset removal should be rejected");

    assert_eq!(
        error,
        XrdsSceneAssetWorkflowError::AssetInUse {
            asset_id: "asset:lamp".to_string(),
            node_ids: vec![XrdsSceneNodeId(42)],
        }
    );
}

#[test]
fn remove_asset_can_detach_references_and_keep_fallback_uri() {
    let mut document = XrdsSceneDocument {
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
                asset_uri: "stale/OldLamp.gltf".to_string(),
                scene_index: 0,
                export_policy: XrdsGltfAssetExportPolicy::KeepExternalReference,
            }),
            editor: XrdsEditorMetadata {
                source: Some(XrdsSourceLink {
                    asset_id: Some("asset:lamp".to_string()),
                    source_node: Some("LampNode".to_string()),
                    import_revision: Some("rev-1".to_string()),
                }),
                ..Default::default()
            },
        }],
        ..Default::default()
    };

    let result = document
        .remove_asset(
            "asset:lamp",
            XrdsSceneAssetRemovalPolicy::DetachReferencingNodes,
        )
        .expect("detaching references should allow asset removal");

    assert_eq!(result.removed_asset.id, "asset:lamp");
    assert_eq!(result.detached_node_ids, vec![XrdsSceneNodeId(42)]);
    assert!(document.asset("asset:lamp").is_none());

    let node = document
        .node(XrdsSceneNodeId(42))
        .expect("node should remain");
    let XrdsSceneNodePayload::GltfAsset(asset) = &node.payload else {
        panic!("expected gltf asset payload");
    };
    assert_eq!(asset.asset_id, None);
    assert_eq!(asset.asset_uri, "catalog/Lamp.gltf");
    assert_eq!(
        node.editor
            .source
            .as_ref()
            .and_then(|source| source.asset_id.as_deref()),
        None
    );
    assert_eq!(
        node.editor
            .source
            .as_ref()
            .and_then(|source| source.source_node.as_deref()),
        Some("LampNode")
    );
}

#[test]
fn rebind_gltf_asset_updates_catalog_and_referencing_node_fallbacks() {
    let mut document = XrdsSceneDocument {
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
            editor: XrdsEditorMetadata::default(),
        }],
        ..Default::default()
    };

    let result = document
        .rebind_gltf_asset("asset:lamp", "catalog/Lamp_v2.gltf")
        .expect("rebinding asset should succeed");

    assert_eq!(result.asset_id, "asset:lamp");
    assert_eq!(result.previous_uri, "catalog/Lamp.gltf");
    assert_eq!(result.new_uri, "catalog/Lamp_v2.gltf");
    assert_eq!(result.rebound_node_ids, vec![XrdsSceneNodeId(42)]);
    assert_eq!(
        document
            .asset("asset:lamp")
            .expect("asset should remain in catalog")
            .uri,
        "catalog/Lamp_v2.gltf"
    );

    let node = document
        .node(XrdsSceneNodeId(42))
        .expect("node should remain");
    let XrdsSceneNodePayload::GltfAsset(asset) = &node.payload else {
        panic!("expected gltf asset payload");
    };
    assert_eq!(asset.asset_id.as_deref(), Some("asset:lamp"));
    assert_eq!(asset.asset_uri, "catalog/Lamp_v2.gltf");

    let runtime_nodes = document
        .to_runtime_nodes()
        .expect("rebound document should still convert to runtime nodes");
    let XrdsSceneRuntimeComponent::GltfAsset(asset) = &runtime_nodes[0].component else {
        panic!("expected runtime gltf component");
    };
    assert_eq!(asset.gltf_asset_path, "catalog/Lamp_v2.gltf");
}

#[test]
fn rename_asset_id_rewrites_catalog_references_and_editor_metadata() {
    let mut document = XrdsSceneDocument {
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
            editor: XrdsEditorMetadata {
                source: Some(XrdsSourceLink {
                    asset_id: Some("asset:lamp".to_string()),
                    source_node: Some("LampNode".to_string()),
                    import_revision: Some("rev-1".to_string()),
                }),
                ..Default::default()
            },
        }],
        ..Default::default()
    };

    let result = document
        .rename_asset_id("asset:lamp", "asset:lamp-renamed")
        .expect("asset id rename should succeed");

    assert_eq!(result.previous_asset_id, "asset:lamp");
    assert_eq!(result.new_asset_id, "asset:lamp-renamed");
    assert_eq!(result.rewritten_node_ids, vec![XrdsSceneNodeId(42)]);
    assert!(document.asset("asset:lamp").is_none());
    assert!(document.asset("asset:lamp-renamed").is_some());

    let node = document
        .node(XrdsSceneNodeId(42))
        .expect("node should remain");
    let XrdsSceneNodePayload::GltfAsset(asset) = &node.payload else {
        panic!("expected gltf asset payload");
    };
    assert_eq!(asset.asset_id.as_deref(), Some("asset:lamp-renamed"));
    assert_eq!(
        node.editor
            .source
            .as_ref()
            .and_then(|source| source.asset_id.as_deref()),
        Some("asset:lamp-renamed")
    );
}

#[test]
fn rename_asset_id_rejects_duplicate_target_id() {
    let mut document = XrdsSceneDocument {
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
        ..Default::default()
    };

    let error = document
        .rename_asset_id("asset:lamp", "asset:triangle")
        .expect_err("duplicate target id should be rejected");

    assert_eq!(
        error,
        XrdsSceneAssetWorkflowError::DuplicateAssetId("asset:triangle".to_string())
    );
}

#[test]
fn gltf_node_health_reports_catalog_resolved_missing_and_detached_states() {
    let document = XrdsSceneDocument {
        assets: vec![XrdsSceneAsset {
            id: "asset:lamp".to_string(),
            uri: "catalog/Lamp.gltf".to_string(),
            kind: XrdsSceneAssetKind::Gltf,
        }],
        nodes: vec![
            XrdsSceneNode {
                id: XrdsSceneNodeId(1),
                parent_id: None,
                name: "Catalog Lamp".to_string(),
                enabled: true,
                visible: true,
                transform: XrdsSceneTransform::default(),
                payload: XrdsSceneNodePayload::GltfAsset(XrdsSceneGltfAsset {
                    asset_id: Some("asset:lamp".to_string()),
                    asset_uri: "fallback/Lamp.gltf".to_string(),
                    scene_index: 0,
                    export_policy: XrdsGltfAssetExportPolicy::KeepExternalReference,
                }),
                editor: XrdsEditorMetadata::default(),
            },
            XrdsSceneNode {
                id: XrdsSceneNodeId(2),
                parent_id: None,
                name: "Missing Lamp".to_string(),
                enabled: true,
                visible: true,
                transform: XrdsSceneTransform::default(),
                payload: XrdsSceneNodePayload::GltfAsset(XrdsSceneGltfAsset {
                    asset_id: Some("asset:missing".to_string()),
                    asset_uri: "fallback/Missing.gltf".to_string(),
                    scene_index: 1,
                    export_policy: XrdsGltfAssetExportPolicy::KeepExternalReference,
                }),
                editor: XrdsEditorMetadata::default(),
            },
            XrdsSceneNode {
                id: XrdsSceneNodeId(3),
                parent_id: None,
                name: "Detached Lamp".to_string(),
                enabled: true,
                visible: true,
                transform: XrdsSceneTransform::default(),
                payload: XrdsSceneNodePayload::GltfAsset(XrdsSceneGltfAsset {
                    asset_id: None,
                    asset_uri: "fallback/Detached.gltf".to_string(),
                    scene_index: 2,
                    export_policy: XrdsGltfAssetExportPolicy::KeepExternalReference,
                }),
                editor: XrdsEditorMetadata::default(),
            },
        ],
        ..Default::default()
    };

    let healths = document.gltf_node_healths();
    assert_eq!(healths.len(), 3);

    let catalog_health = document
        .gltf_node_health(XrdsSceneNodeId(1))
        .expect("catalog node should produce health report");
    assert_eq!(
        catalog_health.status,
        XrdsSceneGltfNodeHealthStatus::CatalogResolved
    );
    assert_eq!(catalog_health.resolved_asset_uri, "catalog/Lamp.gltf");
    assert_eq!(catalog_health.stored_asset_uri, "fallback/Lamp.gltf");

    let missing_health = document
        .gltf_node_health(XrdsSceneNodeId(2))
        .expect("missing node should produce health report");
    assert_eq!(
        missing_health.status,
        XrdsSceneGltfNodeHealthStatus::MissingCatalogAsset
    );
    assert_eq!(missing_health.resolved_asset_uri, "fallback/Missing.gltf");

    let detached_health = document
        .gltf_node_health(XrdsSceneNodeId(3))
        .expect("detached node should produce health report");
    assert_eq!(
        detached_health.status,
        XrdsSceneGltfNodeHealthStatus::DetachedFallback
    );
    assert_eq!(detached_health.resolved_asset_uri, "fallback/Detached.gltf");
}

#[test]
fn gltf_source_diagnostics_report_valid_missing_invalid_extension_and_bad_scene_index() {
    let valid_gltf = asset_fixture_path("models/TestStatus/EmbeddedTriangle.gltf");
    let invalid_extension_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../README.md")
        .to_string_lossy()
        .into_owned();

    let document = XrdsSceneDocument {
        nodes: vec![
            XrdsSceneNode {
                id: XrdsSceneNodeId(1),
                parent_id: None,
                name: "Valid".to_string(),
                enabled: true,
                visible: true,
                transform: XrdsSceneTransform::default(),
                payload: XrdsSceneNodePayload::GltfAsset(XrdsSceneGltfAsset {
                    asset_id: None,
                    asset_uri: valid_gltf.clone(),
                    scene_index: 0,
                    export_policy: XrdsGltfAssetExportPolicy::KeepExternalReference,
                }),
                editor: XrdsEditorMetadata::default(),
            },
            XrdsSceneNode {
                id: XrdsSceneNodeId(2),
                parent_id: None,
                name: "Missing".to_string(),
                enabled: true,
                visible: true,
                transform: XrdsSceneTransform::default(),
                payload: XrdsSceneNodePayload::GltfAsset(XrdsSceneGltfAsset {
                    asset_id: None,
                    asset_uri: asset_fixture_path("models/DoesNotExist/MissingScene.gltf"),
                    scene_index: 0,
                    export_policy: XrdsGltfAssetExportPolicy::KeepExternalReference,
                }),
                editor: XrdsEditorMetadata::default(),
            },
            XrdsSceneNode {
                id: XrdsSceneNodeId(3),
                parent_id: None,
                name: "InvalidExtension".to_string(),
                enabled: true,
                visible: true,
                transform: XrdsSceneTransform::default(),
                payload: XrdsSceneNodePayload::GltfAsset(XrdsSceneGltfAsset {
                    asset_id: None,
                    asset_uri: invalid_extension_path,
                    scene_index: 0,
                    export_policy: XrdsGltfAssetExportPolicy::KeepExternalReference,
                }),
                editor: XrdsEditorMetadata::default(),
            },
            XrdsSceneNode {
                id: XrdsSceneNodeId(4),
                parent_id: None,
                name: "BadSceneIndex".to_string(),
                enabled: true,
                visible: true,
                transform: XrdsSceneTransform::default(),
                payload: XrdsSceneNodePayload::GltfAsset(XrdsSceneGltfAsset {
                    asset_id: None,
                    asset_uri: valid_gltf,
                    scene_index: 5,
                    export_policy: XrdsGltfAssetExportPolicy::KeepExternalReference,
                }),
                editor: XrdsEditorMetadata::default(),
            },
        ],
        ..Default::default()
    };

    let diagnostics = document.gltf_source_diagnostics();
    assert_eq!(diagnostics.len(), 4);

    let valid = document
        .gltf_source_diagnostic(XrdsSceneNodeId(1))
        .expect("valid node should produce source diagnostics");
    assert_eq!(valid.status, XrdsSceneGltfSourceDiagnosticStatus::Valid);
    assert!(valid.resolved_path.is_some());
    assert_eq!(valid.message, None);

    let missing = document
        .gltf_source_diagnostic(XrdsSceneNodeId(2))
        .expect("missing node should produce source diagnostics");
    assert_eq!(
        missing.status,
        XrdsSceneGltfSourceDiagnosticStatus::MissingFile
    );
    assert!(missing.resolved_path.is_none());
    assert!(missing
        .message
        .expect("missing file should produce a message")
        .contains("was not found"));

    let invalid_extension = document
        .gltf_source_diagnostic(XrdsSceneNodeId(3))
        .expect("invalid extension node should produce source diagnostics");
    assert_eq!(
        invalid_extension.status,
        XrdsSceneGltfSourceDiagnosticStatus::InvalidExtension
    );
    assert!(invalid_extension.resolved_path.is_some());
    assert!(invalid_extension
        .message
        .expect("invalid extension should produce a message")
        .contains("must end in .gltf or .glb"));

    let bad_scene = document
        .gltf_source_diagnostic(XrdsSceneNodeId(4))
        .expect("bad scene index node should produce source diagnostics");
    assert_eq!(
        bad_scene.status,
        XrdsSceneGltfSourceDiagnosticStatus::MissingSceneIndex
    );
    assert!(bad_scene.resolved_path.is_some());
    assert!(bad_scene
        .message
        .expect("bad scene index should produce a message")
        .contains("does not contain scene index"));
}

#[test]
fn asset_usage_reports_reference_counts_for_catalog_assets() {
    let document = XrdsSceneDocument {
        assets: vec![
            XrdsSceneAsset {
                id: "asset:lamp".to_string(),
                uri: "catalog/Lamp.gltf".to_string(),
                kind: XrdsSceneAssetKind::Gltf,
            },
            XrdsSceneAsset {
                id: "asset:unused".to_string(),
                uri: "catalog/Unused.gltf".to_string(),
                kind: XrdsSceneAssetKind::Gltf,
            },
        ],
        nodes: vec![
            XrdsSceneNode {
                id: XrdsSceneNodeId(1),
                parent_id: None,
                name: "Lamp A".to_string(),
                enabled: true,
                visible: true,
                transform: XrdsSceneTransform::default(),
                payload: XrdsSceneNodePayload::GltfAsset(XrdsSceneGltfAsset {
                    asset_id: Some("asset:lamp".to_string()),
                    asset_uri: "catalog/Lamp.gltf".to_string(),
                    scene_index: 0,
                    export_policy: XrdsGltfAssetExportPolicy::KeepExternalReference,
                }),
                editor: XrdsEditorMetadata::default(),
            },
            XrdsSceneNode {
                id: XrdsSceneNodeId(2),
                parent_id: None,
                name: "Lamp B".to_string(),
                enabled: true,
                visible: true,
                transform: XrdsSceneTransform::default(),
                payload: XrdsSceneNodePayload::GltfAsset(XrdsSceneGltfAsset {
                    asset_id: Some("asset:lamp".to_string()),
                    asset_uri: "catalog/Lamp.gltf".to_string(),
                    scene_index: 1,
                    export_policy: XrdsGltfAssetExportPolicy::KeepExternalReference,
                }),
                editor: XrdsEditorMetadata::default(),
            },
        ],
        ..Default::default()
    };

    let usages = document.asset_usages();
    assert_eq!(usages.len(), 2);

    let lamp_usage = document
        .asset_usage("asset:lamp")
        .expect("lamp usage should be reported");
    assert_eq!(
        lamp_usage.referenced_node_ids,
        vec![XrdsSceneNodeId(1), XrdsSceneNodeId(2)]
    );

    let unused_usage = document
        .asset_usage("asset:unused")
        .expect("unused asset usage should be reported");
    assert!(unused_usage.referenced_node_ids.is_empty());
}

#[test]
fn asset_diagnostics_summarize_node_health_and_unused_assets() {
    let valid_gltf = asset_fixture_path("models/TestStatus/EmbeddedTriangle.gltf");
    let document = XrdsSceneDocument {
        assets: vec![
            XrdsSceneAsset {
                id: "asset:lamp".to_string(),
                uri: valid_gltf.clone(),
                kind: XrdsSceneAssetKind::Gltf,
            },
            XrdsSceneAsset {
                id: "asset:unused".to_string(),
                uri: valid_gltf.clone(),
                kind: XrdsSceneAssetKind::Gltf,
            },
            XrdsSceneAsset {
                id: "asset:texture-used".to_string(),
                uri: asset_fixture_path("environment_maps/diffuse.ktx2"),
                kind: XrdsSceneAssetKind::Texture,
            },
        ],
        nodes: vec![
            XrdsSceneNode {
                id: XrdsSceneNodeId(1),
                parent_id: None,
                name: "Catalog Lamp".to_string(),
                enabled: true,
                visible: true,
                transform: XrdsSceneTransform::default(),
                payload: XrdsSceneNodePayload::GltfAsset(XrdsSceneGltfAsset {
                    asset_id: Some("asset:lamp".to_string()),
                    asset_uri: "fallback/Lamp.gltf".to_string(),
                    scene_index: 0,
                    export_policy: XrdsGltfAssetExportPolicy::KeepExternalReference,
                }),
                editor: XrdsEditorMetadata::default(),
            },
            XrdsSceneNode {
                id: XrdsSceneNodeId(2),
                parent_id: None,
                name: "Detached Lamp".to_string(),
                enabled: true,
                visible: true,
                transform: XrdsSceneTransform::default(),
                payload: XrdsSceneNodePayload::Cube(XrdsSceneCube {
                    size: [1.0, 1.0, 1.0],
                    material: XrdsSceneMaterial {
                        textures: XrdsSceneMaterialTextureSlots {
                            base_color: Some(XrdsSceneTextureRef {
                                texture_asset_id: "asset:texture-used".to_string(),
                                uv: XrdsSceneTextureUvParams::default(),
                                sampler: XrdsSceneTextureSamplerParams::default(),
                            }),
                            ..Default::default()
                        },
                        ..Default::default()
                    },
                }),
                editor: XrdsEditorMetadata::default(),
            },
            XrdsSceneNode {
                id: XrdsSceneNodeId(3),
                parent_id: None,
                name: "Missing Lamp".to_string(),
                enabled: true,
                visible: true,
                transform: XrdsSceneTransform::default(),
                payload: XrdsSceneNodePayload::GltfAsset(XrdsSceneGltfAsset {
                    asset_id: Some("asset:missing".to_string()),
                    asset_uri: "fallback/Missing.gltf".to_string(),
                    scene_index: 2,
                    export_policy: XrdsGltfAssetExportPolicy::KeepExternalReference,
                }),
                editor: XrdsEditorMetadata::default(),
            },
        ],
        ..Default::default()
    };

    let diagnostics = document.asset_diagnostics();

    assert_eq!(diagnostics.node_healths.len(), 2);
    assert_eq!(diagnostics.source_diagnostics.len(), 2);
    assert_eq!(diagnostics.asset_usages.len(), 3);
    assert_eq!(
        diagnostics.catalog_resolved_node_ids,
        vec![XrdsSceneNodeId(1)]
    );
    assert_eq!(
        diagnostics.detached_fallback_node_ids,
        Vec::<XrdsSceneNodeId>::new()
    );
    assert_eq!(
        diagnostics.missing_catalog_node_ids,
        vec![XrdsSceneNodeId(3)]
    );
    assert_eq!(diagnostics.valid_source_node_ids, vec![XrdsSceneNodeId(1)]);
    assert_eq!(
        diagnostics.invalid_source_node_ids,
        vec![XrdsSceneNodeId(3)]
    );
    assert_eq!(
        diagnostics.unused_asset_ids,
        vec!["asset:unused".to_string()]
    );
    let texture_usage = diagnostics
        .asset_usages
        .iter()
        .find(|usage| usage.asset.id == "asset:texture-used")
        .expect("texture usage should be included in asset diagnostics");
    assert_eq!(texture_usage.referenced_node_ids, vec![XrdsSceneNodeId(2)]);
}

#[test]
fn asset_diagnostic_entries_surface_texture_source_issues_for_ui() {
    let document = XrdsSceneDocument {
        assets: vec![XrdsSceneAsset {
            id: "asset:texture-unused".to_string(),
            uri: asset_fixture_path("environment_maps/diffuse.ktx2"),
            kind: XrdsSceneAssetKind::Texture,
        }],
        ..Default::default()
    };

    let entries = document.asset_diagnostic_entries();
    assert!(entries.iter().any(|entry| {
        entry.subject
            == XrdsSceneAssetDiagnosticSubject::Asset {
                asset_id: "asset:texture-unused".to_string(),
                kind: XrdsSceneAssetKind::Texture,
            }
            && entry.severity == XrdsSceneAssetDiagnosticSeverity::Info
            && entry.title == "Unused asset"
    }));
}

#[test]
fn texture_source_diagnostics_report_valid_missing_and_invalid_extension() {
    let valid_texture = asset_fixture_path("environment_maps/diffuse.ktx2");
    let invalid_extension_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../README.md")
        .to_string_lossy()
        .into_owned();

    let document = XrdsSceneDocument {
        assets: vec![
            XrdsSceneAsset {
                id: "asset:texture-valid".to_string(),
                uri: valid_texture.clone(),
                kind: XrdsSceneAssetKind::Texture,
            },
            XrdsSceneAsset {
                id: "asset:texture-missing".to_string(),
                uri: asset_fixture_path("textures/does-not-exist.ktx2"),
                kind: XrdsSceneAssetKind::Texture,
            },
            XrdsSceneAsset {
                id: "asset:texture-invalid-extension".to_string(),
                uri: invalid_extension_path,
                kind: XrdsSceneAssetKind::Texture,
            },
        ],
        ..Default::default()
    };

    let valid_diag = document
        .texture_source_diagnostic("asset:texture-valid")
        .expect("valid texture asset should produce diagnostics");
    assert_eq!(valid_diag.status, XrdsSceneAssetSourceDiagnosticStatus::Valid);
    assert!(valid_diag.resolved_path.is_some());

    let missing_diag = document
        .texture_source_diagnostic("asset:texture-missing")
        .expect("missing texture asset should produce diagnostics");
    assert_eq!(
        missing_diag.status,
        XrdsSceneAssetSourceDiagnosticStatus::MissingFile
    );
    assert!(missing_diag.resolved_path.is_none());

    let invalid_diag = document
        .texture_source_diagnostic("asset:texture-invalid-extension")
        .expect("invalid texture asset should produce diagnostics");
    assert_eq!(
        invalid_diag.status,
        XrdsSceneAssetSourceDiagnosticStatus::InvalidExtension
    );
    assert!(invalid_diag.resolved_path.is_some());
}

#[test]
fn gltf_node_authoring_round_trips_through_json() {
    let mut document = persistence_test_document();

    document
        .set_gltf_default_playback(
            XrdsSceneNodeId(11),
            Some(XrdsSceneGltfPlayback {
                selector: XrdsSceneGltfAnimationSelector::Name("Idle".to_string()),
                repeat: XrdsSceneAnimationRepeatMode::Loop,
                speed: 1.1,
                start_paused: true,
            }),
        )
        .expect("gltf playback should be assignable");
    document
        .set_gltf_morph_target_weight(
            XrdsSceneNodeId(11),
            XrdsSceneGltfNodeLocator {
                node_index_path: vec![0, 1],
                node_name: Some("Head".to_string()),
            },
            Some("HeadMesh".to_string()),
            XrdsSceneGltfMorphTargetSelector::Name("BlinkLeft".to_string()),
            0.65,
        )
        .expect("morph override should be assignable");

    let json = document
        .to_json_string_pretty()
        .expect("document should serialize");
    let restored = XrdsSceneDocument::from_json_str(&json).expect("document should deserialize");

    assert_eq!(
        restored.gltf_node_authoring.get(&11),
        document.gltf_node_authoring.get(&11)
    );
}

#[test]
fn document_validation_rejects_gltf_authoring_on_non_gltf_node() {
    let document = XrdsSceneDocument {
        nodes: vec![XrdsSceneNode {
            id: XrdsSceneNodeId(40),
            parent_id: None,
            name: "Root".to_string(),
            enabled: true,
            visible: true,
            transform: XrdsSceneTransform::default(),
            payload: XrdsSceneNodePayload::Empty,
            editor: XrdsEditorMetadata::default(),
        }],
        gltf_node_authoring: [(
            40,
            XrdsSceneGltfNodeAuthoring {
                default_playback: Some(XrdsSceneGltfPlayback::default()),
                morph_target_overrides: Vec::new(),
            },
        )]
        .into_iter()
        .collect(),
        ..Default::default()
    };

    assert_eq!(
        document.validate(),
        Err(XrdsSceneValidationError::GltfAuthoringTargetIsNotGltf(
            XrdsSceneNodeId(40)
        ))
    );
}

#[test]
fn environment_map_assets_referenced_by_environment_policy_are_not_flagged_unused() {
    let document = XrdsSceneDocument {
        metadata: XrdsSceneMetadata {
            environment: Some(XrdsSceneEnvironment {
                ibl: Some(XrdsSceneIblEnvironment {
                    diffuse_asset_id: "asset:ibl-diffuse".to_string(),
                    specular_asset_id: "asset:ibl-specular".to_string(),
                    intensity: 500.0,
                }),
                skybox: Some(XrdsSceneSkyboxEnvironment {
                    texture_asset_id: "asset:skybox".to_string(),
                    brightness: 1.0,
                }),
                ..Default::default()
            }),
            ..Default::default()
        },
        assets: vec![
            XrdsSceneAsset {
                id: "asset:ibl-diffuse".to_string(),
                uri: asset_fixture_path("environment_maps/diffuse.ktx2"),
                kind: XrdsSceneAssetKind::EnvironmentMap,
            },
            XrdsSceneAsset {
                id: "asset:ibl-specular".to_string(),
                uri: asset_fixture_path("environment_maps/specular.ktx2"),
                kind: XrdsSceneAssetKind::EnvironmentMap,
            },
            XrdsSceneAsset {
                id: "asset:skybox".to_string(),
                uri: asset_fixture_path("environment_maps/specular.ktx2"),
                kind: XrdsSceneAssetKind::EnvironmentMap,
            },
            XrdsSceneAsset {
                id: "asset:unreferenced-envmap".to_string(),
                uri: asset_fixture_path("environment_maps/diffuse.ktx2"),
                kind: XrdsSceneAssetKind::EnvironmentMap,
            },
        ],
        ..Default::default()
    };

    let diagnostics = document.asset_diagnostics();

    assert!(
        !diagnostics.unused_asset_ids.contains(&"asset:ibl-diffuse".to_string()),
        "IBL diffuse asset should not be flagged as unused"
    );
    assert!(
        !diagnostics.unused_asset_ids.contains(&"asset:ibl-specular".to_string()),
        "IBL specular asset should not be flagged as unused"
    );
    assert!(
        !diagnostics.unused_asset_ids.contains(&"asset:skybox".to_string()),
        "skybox asset should not be flagged as unused"
    );
    assert!(
        diagnostics.unused_asset_ids.contains(&"asset:unreferenced-envmap".to_string()),
        "environment map with no policy reference should still be flagged unused"
    );
}

#[test]
fn register_and_validate_audio_assets() {
    let mut document = XrdsSceneDocument::default();

    let clip = document
        .register_audio_asset("asset:audio-music", "audio/background_music.ogg")
        .expect("registering an audio asset should succeed");
    assert_eq!(clip.kind, XrdsSceneAssetKind::Audio);
    assert_eq!(clip.id, "asset:audio-music");
    assert_eq!(document.assets.len(), 1);

    let ensured = document
        .ensure_audio_asset(None::<String>, "audio/footstep.wav")
        .expect("ensuring a new audio asset should succeed");
    assert!(ensured.created);
    assert_eq!(ensured.asset.kind, XrdsSceneAssetKind::Audio);
    assert_eq!(document.assets.len(), 2);

    let reused = document
        .ensure_audio_asset(None::<String>, "audio/background_music.ogg")
        .expect("ensure should reuse an existing same-kind asset");
    assert!(!reused.created);
    assert_eq!(reused.asset.id, "asset:audio-music");
    assert_eq!(document.assets.len(), 2);
}

#[test]
fn audio_asset_validation_rejects_wrong_extensions() {
    let err = XrdsSceneDocument {
        assets: vec![XrdsSceneAsset {
            id: "asset:audio-bad".to_string(),
            uri: "audio/clip.jpg".to_string(),
            kind: XrdsSceneAssetKind::Audio,
        }],
        ..Default::default()
    }
    .validate()
    .expect_err("audio asset with unsupported extension should fail validation");

    assert!(
        matches!(err, XrdsSceneValidationError::InvalidAssetExtension { .. }),
        "expected InvalidAssetExtension error, got {err:?}"
    );
}

#[test]
fn audio_asset_validates_all_supported_extensions() {
    for ext in &["mp3", "ogg", "wav", "flac"] {
        let result = XrdsSceneDocument {
            assets: vec![XrdsSceneAsset {
                id: format!("asset:audio-{ext}"),
                uri: format!("audio/clip.{ext}"),
                kind: XrdsSceneAssetKind::Audio,
            }],
            ..Default::default()
        }
        .validate();
        assert!(result.is_ok(), "audio asset with .{ext} extension should pass validation");
    }
}

#[test]
fn audio_assets_appear_in_unused_ids_when_unreferenced() {
    let document = XrdsSceneDocument {
        assets: vec![
            XrdsSceneAsset {
                id: "asset:audio-ambient".to_string(),
                uri: "audio/ambient.ogg".to_string(),
                kind: XrdsSceneAssetKind::Audio,
            },
            XrdsSceneAsset {
                id: "asset:texture-floor".to_string(),
                uri: "textures/floor.png".to_string(),
                kind: XrdsSceneAssetKind::Texture,
            },
        ],
        ..Default::default()
    };

    let diagnostics = document.asset_diagnostics();
    assert!(
        diagnostics.unused_asset_ids.contains(&"asset:audio-ambient".to_string()),
        "unreferenced audio asset should appear in unused_asset_ids"
    );
}

#[test]
fn audio_clip_node_references_audio_catalog_asset_and_drives_usage_tracking() {
    let document = XrdsSceneDocument {
        assets: vec![
            XrdsSceneAsset {
                id: "asset:audio-footstep".to_string(),
                uri: "audio/footstep.wav".to_string(),
                kind: XrdsSceneAssetKind::Audio,
            },
            XrdsSceneAsset {
                id: "asset:audio-unused".to_string(),
                uri: "audio/ambient.ogg".to_string(),
                kind: XrdsSceneAssetKind::Audio,
            },
        ],
        nodes: vec![
            XrdsSceneNode {
                id: XrdsSceneNodeId(50),
                parent_id: None,
                name: "FootstepSource".to_string(),
                enabled: true,
                visible: true,
                transform: XrdsSceneTransform::default(),
                payload: XrdsSceneNodePayload::AudioClip(XrdsSceneAudioClip {
                    asset_id: "asset:audio-footstep".to_string(),
                    volume: 0.8,
                    looped: false,
                    spatial: true,
                    autoplay: false,
                }),
                editor: XrdsEditorMetadata::default(),
            },
        ],
        ..Default::default()
    };

    document.validate().expect("document with valid audio clip node should pass validation");

    let diagnostics = document.asset_diagnostics();

    let footstep_usage = diagnostics
        .asset_usages
        .iter()
        .find(|u| u.asset.id == "asset:audio-footstep")
        .expect("footstep asset usage should be tracked");
    assert_eq!(
        footstep_usage.referenced_node_ids,
        vec![XrdsSceneNodeId(50)],
        "footstep asset should be referenced by the audio clip node"
    );

    assert!(
        !diagnostics.unused_asset_ids.contains(&"asset:audio-footstep".to_string()),
        "referenced audio asset should not be in unused_asset_ids"
    );
    assert!(
        diagnostics.unused_asset_ids.contains(&"asset:audio-unused".to_string()),
        "unreferenced audio asset should be in unused_asset_ids"
    );
}

#[test]
fn audio_clip_node_validation_rejects_missing_and_wrong_kind_assets() {
    // Missing asset
    let err = XrdsSceneDocument {
        assets: vec![],
        nodes: vec![XrdsSceneNode {
            id: XrdsSceneNodeId(60),
            parent_id: None,
            name: "Source".to_string(),
            enabled: true,
            visible: true,
            transform: XrdsSceneTransform::default(),
            payload: XrdsSceneNodePayload::AudioClip(XrdsSceneAudioClip {
                asset_id: "asset:missing".to_string(),
                ..Default::default()
            }),
            editor: XrdsEditorMetadata::default(),
        }],
        ..Default::default()
    }
    .validate()
    .expect_err("audio clip referencing missing asset should fail validation");

    assert!(
        matches!(
            err,
            XrdsSceneValidationError::MissingAudioClipAsset { .. }
        ),
        "expected MissingAudioClipAsset, got {err:?}"
    );

    // Wrong kind
    let err = XrdsSceneDocument {
        assets: vec![XrdsSceneAsset {
            id: "asset:texture".to_string(),
            uri: "textures/floor.png".to_string(),
            kind: XrdsSceneAssetKind::Texture,
        }],
        nodes: vec![XrdsSceneNode {
            id: XrdsSceneNodeId(61),
            parent_id: None,
            name: "Source".to_string(),
            enabled: true,
            visible: true,
            transform: XrdsSceneTransform::default(),
            payload: XrdsSceneNodePayload::AudioClip(XrdsSceneAudioClip {
                asset_id: "asset:texture".to_string(),
                ..Default::default()
            }),
            editor: XrdsEditorMetadata::default(),
        }],
        ..Default::default()
    }
    .validate()
    .expect_err("audio clip pointing to non-audio asset should fail validation");

    assert!(
        matches!(
            err,
            XrdsSceneValidationError::AudioClipAssetKindMismatch { .. }
        ),
        "expected AudioClipAssetKindMismatch, got {err:?}"
    );
}
