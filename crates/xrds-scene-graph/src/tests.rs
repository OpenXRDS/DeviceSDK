use super::*;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

fn asset_fixture_path(relative_path: &str) -> String {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../assets")
        .join(relative_path)
        .to_string_lossy()
        .into_owned()
}

fn persistence_test_document() -> XrdsSceneDocument {
    XrdsSceneDocument {
        metadata: XrdsSceneMetadata {
            name: "Persistence Test".to_string(),
            authored_by: Some("xrds-tests".to_string()),
            default_scene_label: Some("Main".to_string()),
            extras: [("theme".to_string(), "industrial".to_string())]
                .into_iter()
                .collect(),
        },
        assets: vec![XrdsSceneAsset {
            id: "asset:lamp".to_string(),
            uri: "models/StainedGlassLamp/StainedGlassLamp.gltf".to_string(),
            kind: XrdsSceneAssetKind::Gltf,
        }],
        nodes: vec![
            XrdsSceneNode {
                id: XrdsSceneNodeId(10),
                parent_id: None,
                name: "Root".to_string(),
                enabled: true,
                visible: true,
                transform: XrdsSceneTransform::default(),
                payload: XrdsSceneNodePayload::Empty,
                editor: XrdsEditorMetadata {
                    tags: vec!["folder".to_string(), "root".to_string()],
                    layer: Some("Scene".to_string()),
                    locked: true,
                    hidden_in_editor: false,
                    user_properties: [("expanded".to_string(), "true".to_string())]
                        .into_iter()
                        .collect(),
                    source: Some(XrdsSourceLink {
                        asset_id: Some("asset:lamp".to_string()),
                        source_node: Some("RootNode".to_string()),
                        import_revision: Some("rev-1".to_string()),
                    }),
                },
            },
            XrdsSceneNode {
                id: XrdsSceneNodeId(11),
                parent_id: Some(XrdsSceneNodeId(10)),
                name: "Lamp".to_string(),
                enabled: true,
                visible: true,
                transform: XrdsSceneTransform {
                    translation: [1.0, 2.0, 3.0],
                    rotation_quat_xyzw: [0.0, 0.0, 0.0, 1.0],
                    scale: [1.5, 1.5, 1.5],
                },
                payload: XrdsSceneNodePayload::GltfAsset(XrdsSceneGltfAsset {
                    asset_id: Some("asset:lamp".to_string()),
                    asset_uri: "models/StainedGlassLamp/StainedGlassLamp.gltf".to_string(),
                    scene_index: 0,
                    export_policy: XrdsGltfAssetExportPolicy::KeepExternalReference,
                }),
                editor: XrdsEditorMetadata {
                    tags: vec!["mesh".to_string()],
                    layer: Some("Gameplay".to_string()),
                    locked: false,
                    hidden_in_editor: true,
                    user_properties: [
                        ("selected".to_string(), "false".to_string()),
                        ("note".to_string(), "keep".to_string()),
                    ]
                    .into_iter()
                    .collect(),
                    source: Some(XrdsSourceLink {
                        asset_id: Some("asset:lamp".to_string()),
                        source_node: Some("LampNode".to_string()),
                        import_revision: Some("rev-2".to_string()),
                    }),
                },
            },
        ],
        ..Default::default()
    }
}

#[test]
fn document_validation_rejects_missing_parent() {
    let doc = XrdsSceneDocument {
        nodes: vec![XrdsSceneNode {
            id: XrdsSceneNodeId(1),
            parent_id: Some(XrdsSceneNodeId(99)),
            name: "Child".to_string(),
            enabled: true,
            visible: true,
            transform: XrdsSceneTransform::default(),
            payload: XrdsSceneNodePayload::Empty,
            editor: XrdsEditorMetadata::default(),
        }],
        ..Default::default()
    };

    assert_eq!(
        doc.validate(),
        Err(XrdsSceneValidationError::MissingParent {
            node_id: XrdsSceneNodeId(1),
            parent_id: XrdsSceneNodeId(99),
        })
    );
}

#[test]
fn primitives_are_marked_for_mesh_bake_on_gltf_export() {
    let node = XrdsSceneNode {
        id: XrdsSceneNodeId(1),
        parent_id: None,
        name: "Cube".to_string(),
        enabled: true,
        visible: true,
        transform: XrdsSceneTransform::default(),
        payload: XrdsSceneNodePayload::Cube(XrdsSceneCube::default()),
        editor: XrdsEditorMetadata::default(),
    };

    assert_eq!(
        node.gltf_export_class(),
        XrdsGltfExportClass::ProceduralMeshBake
    );
}

#[test]
fn cube_descriptor_conversion_preserves_material_override() {
    let mut cube = XrdsCube::new().with_name("Cube");
    cube.transform.translation = [1.0, 2.0, 3.0];

    let material = XrdsMaterialParams {
        base_color: XrdsColor::srgba(0.2, 0.4, 0.6, 1.0),
        emissive: XrdsLinearRgba::rgb(0.1, 0.2, 0.3),
        opacity: 0.75,
        unlit: true,
        pbr: XrdsMaterialPbrParams {
            metallic: 0.8,
            perceptual_roughness: 0.2,
            reflectance: 0.65,
            double_sided: true,
            alpha_mode: XrdsMaterialAlphaMode::Mask,
            alpha_cutoff: 0.35,
        },
    };

    let node = XrdsSceneNode::from_xrds_cube(
        XrdsSceneNodeId(7),
        Some(XrdsSceneNodeId(3)),
        &cube,
        Some(material),
    );

    assert_eq!(node.id, XrdsSceneNodeId(7));
    assert_eq!(node.parent_id, Some(XrdsSceneNodeId(3)));
    assert_eq!(node.transform.translation, [1.0, 2.0, 3.0]);

    let XrdsSceneNodePayload::Cube(cube_payload) = node.payload else {
        panic!("expected cube payload");
    };

    assert_eq!(cube_payload.material.opacity, 0.75);
    assert!(cube_payload.material.unlit);
    assert_eq!(cube_payload.material.base_color, [0.2, 0.4, 0.6, 1.0]);
    assert_eq!(cube_payload.material.pbr.metallic, 0.8);
    assert_eq!(cube_payload.material.pbr.perceptual_roughness, 0.2);
    assert_eq!(cube_payload.material.pbr.reflectance, 0.65);
    assert!(cube_payload.material.pbr.double_sided);
    assert_eq!(
        cube_payload.material.pbr.alpha_mode,
        XrdsSceneMaterialAlphaMode::Mask
    );
    assert_eq!(cube_payload.material.pbr.alpha_cutoff, 0.35);
}

#[test]
fn runtime_projection_preserves_ids_parents_and_gltf_references() {
    let doc = XrdsSceneDocument {
        nodes: vec![
            XrdsSceneNode {
                id: XrdsSceneNodeId(10),
                parent_id: None,
                name: "Root".to_string(),
                enabled: true,
                visible: true,
                transform: XrdsSceneTransform::default(),
                payload: XrdsSceneNodePayload::Empty,
                editor: XrdsEditorMetadata::default(),
            },
            XrdsSceneNode {
                id: XrdsSceneNodeId(11),
                parent_id: Some(XrdsSceneNodeId(10)),
                name: "Lamp".to_string(),
                enabled: true,
                visible: true,
                transform: XrdsSceneTransform::default(),
                payload: XrdsSceneNodePayload::GltfAsset(XrdsSceneGltfAsset {
                    asset_id: None,
                    asset_uri: "models/StainedGlassLamp/StainedGlassLamp.gltf".to_string(),
                    scene_index: 0,
                    export_policy: XrdsGltfAssetExportPolicy::KeepExternalReference,
                }),
                editor: XrdsEditorMetadata::default(),
            },
        ],
        ..Default::default()
    };

    let runtime_nodes = doc.to_runtime_nodes().expect("document should validate");
    assert_eq!(runtime_nodes.len(), 2);
    assert_eq!(runtime_nodes[0].id, XrdsId(10));
    assert_eq!(runtime_nodes[1].parent_id, Some(XrdsId(10)));

    let XrdsSceneRuntimeComponent::GltfAsset(asset) = &runtime_nodes[1].component else {
        panic!("expected gltf runtime component");
    };
    assert_eq!(
        asset.gltf_asset_path,
        "models/StainedGlassLamp/StainedGlassLamp.gltf"
    );
    assert_eq!(asset.scene_index, 0);
}

#[test]
fn runtime_projection_preserves_material_for_mesh_nodes() {
    let node = XrdsSceneNode {
        id: XrdsSceneNodeId(21),
        parent_id: Some(XrdsSceneNodeId(20)),
        name: "Sphere".to_string(),
        enabled: true,
        visible: false,
        transform: XrdsSceneTransform {
            translation: [2.0, 3.0, 4.0],
            rotation_quat_xyzw: [0.0, 0.0, 0.0, 1.0],
            scale: [1.0, 2.0, 1.0],
        },
        payload: XrdsSceneNodePayload::Sphere(XrdsSceneSphere {
            radius: 1.5,
            material: XrdsSceneMaterial {
                base_color: [0.1, 0.2, 0.3, 1.0],
                emissive: [0.4, 0.5, 0.6, 1.0],
                opacity: 0.7,
                unlit: true,
                pbr: XrdsSceneMaterialPbrParams {
                    metallic: 0.6,
                    perceptual_roughness: 0.3,
                    reflectance: 0.7,
                    double_sided: true,
                    alpha_mode: XrdsSceneMaterialAlphaMode::Blend,
                    alpha_cutoff: 0.5,
                },
                textures: XrdsSceneMaterialTextureSlots::default(),
            },
        }),
        editor: XrdsEditorMetadata::default(),
    };

    let runtime = node.to_runtime_node();
    assert_eq!(runtime.id, XrdsId(21));
    assert_eq!(runtime.parent_id, Some(XrdsId(20)));

    let XrdsSceneRuntimeComponent::Sphere(sphere) = runtime.component else {
        panic!("expected sphere runtime component");
    };
    assert_eq!(sphere.radius, 1.5);
    assert_eq!(sphere.transform.translation, [2.0, 3.0, 4.0]);
    assert!(!sphere.visible);

    let material = runtime.material.expect("mesh nodes should carry material");
    assert_eq!(material.base_color.rgba, [0.1, 0.2, 0.3, 1.0]);
    assert_eq!(material.emissive.rgba, [0.4, 0.5, 0.6, 1.0]);
    assert_eq!(material.opacity, 0.7);
    assert!(material.unlit);
    assert_eq!(material.pbr.metallic, 0.6);
    assert_eq!(material.pbr.perceptual_roughness, 0.3);
    assert_eq!(material.pbr.reflectance, 0.7);
    assert!(material.pbr.double_sided);
    assert_eq!(material.pbr.alpha_mode, XrdsMaterialAlphaMode::Blend);
}

#[test]
fn json_load_defaults_new_material_pbr_fields_for_legacy_documents() {
    let json = r#"
        {
            "version": 1,
            "metadata": {
                "name": "Legacy",
                "authored_by": null,
                "default_scene_label": null,
                "extras": {}
            },
            "assets": [],
            "nodes": [
                {
                    "id": 1,
                    "parent_id": null,
                    "name": "Sphere",
                    "enabled": true,
                    "visible": true,
                    "transform": {
                        "translation": [0.0, 0.0, 0.0],
                        "rotation_quat_xyzw": [0.0, 0.0, 0.0, 1.0],
                        "scale": [1.0, 1.0, 1.0]
                    },
                    "payload": {
                        "Sphere": {
                            "radius": 1.0,
                            "material": {
                                "base_color": [1.0, 0.0, 0.0, 1.0],
                                "emissive": [0.0, 0.0, 0.0, 1.0],
                                "opacity": 1.0,
                                "unlit": false
                            }
                        }
                    },
                    "editor": {
                        "tags": [],
                        "layer": null,
                        "locked": false,
                        "hidden_in_editor": false,
                        "user_properties": {},
                        "source": null
                    }
                }
            ]
        }
        "#;

    let document = XrdsSceneDocument::from_json_str(json).expect("legacy document should load");
    let XrdsSceneNodePayload::Sphere(sphere) = &document.nodes[0].payload else {
        panic!("expected sphere payload");
    };

    assert_eq!(sphere.material.pbr, XrdsSceneMaterialPbrParams::default());
}

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
fn register_gltf_asset_adds_catalog_entry_and_rejects_duplicate_id_or_uri() {
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

    let duplicate_uri = document
        .register_gltf_asset("asset:other", "catalog/Lamp.gltf")
        .expect_err("duplicate asset uri should be rejected");
    assert_eq!(
        duplicate_uri,
        XrdsSceneAssetWorkflowError::DuplicateAssetUri {
            uri: "catalog/Lamp.gltf".to_string(),
            asset_id: "asset:lamp".to_string(),
        }
    );
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
fn register_image_and_texture_assets_allow_shared_source_uri_across_kinds() {
    let mut document = XrdsSceneDocument::default();

    let image = document
        .register_image_asset("asset:image-bricks", "textures/bricks_basecolor.ktx2")
        .expect("registering an image asset should succeed");
    let texture = document
        .register_texture_asset("asset:texture-bricks", "textures/bricks_basecolor.ktx2")
        .expect("registering a texture asset on the same source file should succeed");

    assert_eq!(image.kind, XrdsSceneAssetKind::Image);
    assert_eq!(texture.kind, XrdsSceneAssetKind::Texture);
    assert_eq!(document.assets.len(), 2);

    let duplicate_image = document
        .register_image_asset("asset:image-bricks-2", "textures/bricks_basecolor.ktx2")
        .expect_err("duplicate image uri should still be rejected within the same kind");
    assert_eq!(
        duplicate_image,
        XrdsSceneAssetWorkflowError::DuplicateAssetUri {
            uri: "textures/bricks_basecolor.ktx2".to_string(),
            asset_id: "asset:image-bricks".to_string(),
        }
    );
}

#[test]
fn ensure_image_and_texture_assets_are_scoped_by_kind() {
    let mut document = XrdsSceneDocument {
        assets: vec![XrdsSceneAsset {
            id: "asset:image-bricks".to_string(),
            uri: "textures/bricks_basecolor.ktx2".to_string(),
            kind: XrdsSceneAssetKind::Image,
        }],
        ..Default::default()
    };

    let reused_image = document
        .ensure_image_asset(None::<String>, "textures/bricks_basecolor.ktx2")
        .expect("image ensure should reuse same-kind asset");
    assert!(!reused_image.created);
    assert_eq!(reused_image.asset.id, "asset:image-bricks");

    let created_texture = document
        .ensure_texture_asset(None::<String>, "textures/bricks_basecolor.ktx2")
        .expect("texture ensure should create a new asset even when image uri matches");
    assert!(created_texture.created);
    assert_eq!(created_texture.asset.kind, XrdsSceneAssetKind::Texture);
    assert!(created_texture
        .asset
        .id
        .starts_with("asset:texture-bricks-basecolor"));
    assert_eq!(document.assets.len(), 2);
}

#[test]
fn rebind_asset_enforces_duplicate_uri_per_kind_only() {
    let mut document = XrdsSceneDocument {
        assets: vec![
            XrdsSceneAsset {
                id: "asset:image-a".to_string(),
                uri: "textures/a.ktx2".to_string(),
                kind: XrdsSceneAssetKind::Image,
            },
            XrdsSceneAsset {
                id: "asset:image-b".to_string(),
                uri: "textures/b.ktx2".to_string(),
                kind: XrdsSceneAssetKind::Image,
            },
            XrdsSceneAsset {
                id: "asset:texture-b".to_string(),
                uri: "textures/b.ktx2".to_string(),
                kind: XrdsSceneAssetKind::Texture,
            },
        ],
        ..Default::default()
    };

    let duplicate_same_kind = document
        .rebind_asset("asset:image-b", "textures/a.ktx2")
        .expect_err("rebind should reject duplicate uri within the same kind");
    assert_eq!(
        duplicate_same_kind,
        XrdsSceneAssetWorkflowError::DuplicateAssetUri {
            uri: "textures/a.ktx2".to_string(),
            asset_id: "asset:image-a".to_string(),
        }
    );

    let cross_kind = document
        .rebind_asset("asset:texture-b", "textures/a.ktx2")
        .expect("rebind should allow sharing a uri across different kinds");
    assert_eq!(cross_kind.asset_id, "asset:texture-b");
    assert_eq!(cross_kind.previous_uri, "textures/b.ktx2");
    assert_eq!(cross_kind.new_uri, "textures/a.ktx2");
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
                payload: XrdsSceneNodePayload::GltfAsset(XrdsSceneGltfAsset {
                    asset_id: None,
                    asset_uri: valid_gltf,
                    scene_index: 0,
                    export_policy: XrdsGltfAssetExportPolicy::KeepExternalReference,
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

    assert_eq!(diagnostics.node_healths.len(), 3);
    assert_eq!(diagnostics.source_diagnostics.len(), 3);
    assert_eq!(diagnostics.asset_usages.len(), 2);
    assert_eq!(
        diagnostics.catalog_resolved_node_ids,
        vec![XrdsSceneNodeId(1)]
    );
    assert_eq!(
        diagnostics.detached_fallback_node_ids,
        vec![XrdsSceneNodeId(2)]
    );
    assert_eq!(
        diagnostics.missing_catalog_node_ids,
        vec![XrdsSceneNodeId(3)]
    );
    assert_eq!(
        diagnostics.valid_source_node_ids,
        vec![XrdsSceneNodeId(1), XrdsSceneNodeId(2)]
    );
    assert_eq!(
        diagnostics.invalid_source_node_ids,
        vec![XrdsSceneNodeId(3)]
    );
    assert_eq!(
        diagnostics.unused_asset_ids,
        vec!["asset:unused".to_string()]
    );
}

#[test]
fn document_metadata_workflow_normalizes_editor_fields() {
    let mut document = XrdsSceneDocument {
        nodes: vec![XrdsSceneNode {
            id: XrdsSceneNodeId(7),
            parent_id: None,
            name: "Node".to_string(),
            enabled: true,
            visible: true,
            transform: XrdsSceneTransform::default(),
            payload: XrdsSceneNodePayload::Empty,
            editor: XrdsEditorMetadata::default(),
        }],
        ..Default::default()
    };

    document
        .set_node_tags(
            XrdsSceneNodeId(7),
            vec![
                " hero ".to_string(),
                "".to_string(),
                "setpiece".to_string(),
                "hero".to_string(),
            ],
        )
        .expect("setting tags should succeed");
    document
        .set_node_layer(XrdsSceneNodeId(7), Some(" Gameplay ".to_string()))
        .expect("setting layer should succeed");
    document
        .set_node_locked(XrdsSceneNodeId(7), true)
        .expect("setting lock state should succeed");
    document
        .set_node_hidden_in_editor(XrdsSceneNodeId(7), true)
        .expect("setting hidden state should succeed");
    let previous = document
        .set_node_user_property(XrdsSceneNodeId(7), " note ", "keep")
        .expect("setting user property should succeed");
    assert_eq!(previous, None);
    document
        .set_node_source_link(
            XrdsSceneNodeId(7),
            Some(XrdsSourceLink {
                asset_id: Some(" asset:lamp ".to_string()),
                source_node: Some(" LampNode ".to_string()),
                import_revision: Some(" rev-2 ".to_string()),
            }),
        )
        .expect("setting source link should succeed");

    let metadata = document
        .editor_metadata(XrdsSceneNodeId(7))
        .expect("metadata should exist for node");
    assert_eq!(
        metadata.tags,
        vec!["hero".to_string(), "setpiece".to_string()]
    );
    assert_eq!(metadata.layer.as_deref(), Some("Gameplay"));
    assert!(metadata.locked);
    assert!(metadata.hidden_in_editor);
    assert_eq!(
        metadata.user_properties.get("note"),
        Some(&"keep".to_string())
    );
    assert_eq!(
        metadata.source,
        Some(XrdsSourceLink {
            asset_id: Some("asset:lamp".to_string()),
            source_node: Some("LampNode".to_string()),
            import_revision: Some("rev-2".to_string()),
        })
    );

    let removed = document
        .remove_node_user_property(XrdsSceneNodeId(7), " note ")
        .expect("removing property should succeed");
    assert_eq!(removed.as_deref(), Some("keep"));

    document
        .set_node_source_link(
            XrdsSceneNodeId(7),
            Some(XrdsSourceLink {
                asset_id: Some(" ".to_string()),
                source_node: None,
                import_revision: Some(" ".to_string()),
            }),
        )
        .expect("empty source link fields should clear the source link");
    assert_eq!(
        document
            .editor_metadata(XrdsSceneNodeId(7))
            .expect("metadata should still exist")
            .source,
        None
    );
}

#[test]
fn document_material_workflow_updates_mesh_nodes_and_rejects_materialless_nodes() {
    let mut document = XrdsSceneDocument {
        nodes: vec![
            XrdsSceneNode {
                id: XrdsSceneNodeId(8),
                parent_id: None,
                name: "Cube".to_string(),
                enabled: true,
                visible: true,
                transform: XrdsSceneTransform::default(),
                payload: XrdsSceneNodePayload::Cube(XrdsSceneCube::default()),
                editor: XrdsEditorMetadata::default(),
            },
            XrdsSceneNode {
                id: XrdsSceneNodeId(9),
                parent_id: None,
                name: "Empty".to_string(),
                enabled: true,
                visible: true,
                transform: XrdsSceneTransform::default(),
                payload: XrdsSceneNodePayload::Empty,
                editor: XrdsEditorMetadata::default(),
            },
        ],
        ..Default::default()
    };

    document
        .set_node_material(
            XrdsSceneNodeId(8),
            XrdsSceneMaterial {
                base_color: [0.2, 0.3, 0.4, 1.0],
                emissive: [0.0, 0.0, 0.0, 1.0],
                opacity: 1.4,
                unlit: false,
                pbr: XrdsSceneMaterialPbrParams {
                    metallic: 1.4,
                    perceptual_roughness: -0.2,
                    reflectance: 1.3,
                    double_sided: false,
                    alpha_mode: XrdsSceneMaterialAlphaMode::Opaque,
                    alpha_cutoff: 1.5,
                },
                textures: XrdsSceneMaterialTextureSlots::default(),
            },
        )
        .expect("setting full material should succeed");
    document
        .set_node_material_base_color(XrdsSceneNodeId(8), XrdsColor::srgb(0.9, 0.2, 0.3))
        .expect("setting base color should succeed");
    document
        .set_node_material_emissive(XrdsSceneNodeId(8), XrdsLinearRgba::rgb(0.6, 0.1, 0.2))
        .expect("setting emissive should succeed");
    document
        .set_node_material_opacity(XrdsSceneNodeId(8), -0.5)
        .expect("setting opacity should succeed");
    document
        .set_node_material_unlit(XrdsSceneNodeId(8), true)
        .expect("setting unlit should succeed");
    document
        .set_node_material_metallic(XrdsSceneNodeId(8), 0.85)
        .expect("setting metallic should succeed");
    document
        .set_node_material_perceptual_roughness(XrdsSceneNodeId(8), 0.18)
        .expect("setting roughness should succeed");
    document
        .set_node_material_reflectance(XrdsSceneNodeId(8), 0.72)
        .expect("setting reflectance should succeed");
    document
        .set_node_material_double_sided(XrdsSceneNodeId(8), true)
        .expect("setting double-sided should succeed");
    document
        .set_node_material_alpha_mode(XrdsSceneNodeId(8), XrdsSceneMaterialAlphaMode::Mask)
        .expect("setting alpha mode should succeed");
    document
        .set_node_material_alpha_cutoff(XrdsSceneNodeId(8), -3.0)
        .expect("setting alpha cutoff should succeed");

    let material = document
        .node_material(XrdsSceneNodeId(8))
        .expect("mesh node should expose material");
    assert_eq!(material.base_color, [0.9, 0.2, 0.3, 1.0]);
    assert_eq!(material.emissive, [0.6, 0.1, 0.2, 1.0]);
    assert_eq!(material.opacity, 0.0);
    assert!(material.unlit);
    assert_eq!(material.pbr.metallic, 0.85);
    assert_eq!(material.pbr.perceptual_roughness, 0.18);
    assert_eq!(material.pbr.reflectance, 0.72);
    assert!(material.pbr.double_sided);
    assert_eq!(material.pbr.alpha_mode, XrdsSceneMaterialAlphaMode::Mask);
    assert_eq!(material.pbr.alpha_cutoff, 0.0);
    assert_eq!(
        document.node_material_pbr(XrdsSceneNodeId(8)).unwrap(),
        &material.pbr
    );
    assert!(document
        .node_material_textures(XrdsSceneNodeId(8))
        .unwrap()
        .is_empty());

    assert_eq!(
        document.node_material(XrdsSceneNodeId(9)),
        Err(XrdsSceneMaterialWorkflowError::NodeHasNoMaterial(
            XrdsSceneNodeId(9)
        ))
    );
}

#[test]
fn document_material_texture_workflow_updates_slots_and_normalizes_ids() {
    let mut document = XrdsSceneDocument {
        assets: vec![XrdsSceneAsset {
            id: "asset:texture-bricks".to_string(),
            uri: "textures/bricks_basecolor.ktx2".to_string(),
            kind: XrdsSceneAssetKind::Texture,
        }],
        nodes: vec![XrdsSceneNode {
            id: XrdsSceneNodeId(80),
            parent_id: None,
            name: "Cube".to_string(),
            enabled: true,
            visible: true,
            transform: XrdsSceneTransform::default(),
            payload: XrdsSceneNodePayload::Cube(XrdsSceneCube::default()),
            editor: XrdsEditorMetadata::default(),
        }],
        ..Default::default()
    };

    document
        .set_node_material_texture(
            XrdsSceneNodeId(80),
            XrdsSceneMaterialTextureSlotKind::BaseColor,
            Some(XrdsSceneTextureRef {
                texture_asset_id: " asset:texture-bricks ".to_string(),
            }),
        )
        .expect("setting a material texture slot should succeed");

    let textures = document
        .node_material_textures(XrdsSceneNodeId(80))
        .expect("mesh node should expose texture slots");
    assert_eq!(
        textures
            .base_color
            .as_ref()
            .map(|texture| texture.texture_asset_id.as_str()),
        Some("asset:texture-bricks")
    );

    document
        .set_node_material_texture(
            XrdsSceneNodeId(80),
            XrdsSceneMaterialTextureSlotKind::BaseColor,
            None,
        )
        .expect("clearing a material texture slot should succeed");
    assert!(document
        .node_material_textures(XrdsSceneNodeId(80))
        .unwrap()
        .base_color
        .is_none());
}

#[test]
fn document_validation_rejects_material_texture_reference_to_missing_or_wrong_kind_asset() {
    let mut missing_document = XrdsSceneDocument {
        nodes: vec![XrdsSceneNode {
            id: XrdsSceneNodeId(81),
            parent_id: None,
            name: "Sphere".to_string(),
            enabled: true,
            visible: true,
            transform: XrdsSceneTransform::default(),
            payload: XrdsSceneNodePayload::Sphere(XrdsSceneSphere {
                radius: 1.0,
                material: XrdsSceneMaterial {
                    textures: XrdsSceneMaterialTextureSlots {
                        base_color: Some(XrdsSceneTextureRef {
                            texture_asset_id: "asset:texture-missing".to_string(),
                        }),
                        ..Default::default()
                    },
                    ..Default::default()
                },
            }),
            editor: XrdsEditorMetadata::default(),
        }],
        ..Default::default()
    };

    assert_eq!(
        missing_document.validate(),
        Err(XrdsSceneValidationError::MissingMaterialTextureAsset {
            node_id: XrdsSceneNodeId(81),
            slot: XrdsSceneMaterialTextureSlotKind::BaseColor,
            asset_id: "asset:texture-missing".to_string(),
        })
    );

    missing_document.assets.push(XrdsSceneAsset {
        id: "asset:texture-missing".to_string(),
        uri: "textures/bricks_basecolor.ktx2".to_string(),
        kind: XrdsSceneAssetKind::Image,
    });

    assert_eq!(
        missing_document.validate(),
        Err(XrdsSceneValidationError::MaterialTextureAssetKindMismatch {
            node_id: XrdsSceneNodeId(81),
            slot: XrdsSceneMaterialTextureSlotKind::BaseColor,
            asset_id: "asset:texture-missing".to_string(),
            found: XrdsSceneAssetKind::Image,
        })
    );
}

#[test]
fn image_and_texture_source_diagnostics_report_valid_missing_and_invalid_extension() {
    let valid_image = asset_fixture_path("environment_maps/diffuse.ktx2");
    let invalid_extension_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../README.md")
        .to_string_lossy()
        .into_owned();

    let document = XrdsSceneDocument {
        assets: vec![
            XrdsSceneAsset {
                id: "asset:image-valid".to_string(),
                uri: valid_image.clone(),
                kind: XrdsSceneAssetKind::Image,
            },
            XrdsSceneAsset {
                id: "asset:image-missing".to_string(),
                uri: asset_fixture_path("textures/does-not-exist.ktx2"),
                kind: XrdsSceneAssetKind::Image,
            },
            XrdsSceneAsset {
                id: "asset:texture-invalid-extension".to_string(),
                uri: invalid_extension_path,
                kind: XrdsSceneAssetKind::Texture,
            },
        ],
        ..Default::default()
    };

    let valid_image_diag = document
        .image_source_diagnostic("asset:image-valid")
        .expect("valid image asset should produce diagnostics");
    assert_eq!(
        valid_image_diag.status,
        XrdsSceneAssetSourceDiagnosticStatus::Valid
    );
    assert!(valid_image_diag.resolved_path.is_some());

    let missing_image_diag = document
        .image_source_diagnostic("asset:image-missing")
        .expect("missing image asset should produce diagnostics");
    assert_eq!(
        missing_image_diag.status,
        XrdsSceneAssetSourceDiagnosticStatus::MissingFile
    );
    assert!(missing_image_diag.resolved_path.is_none());

    let invalid_texture_diag = document
        .texture_source_diagnostic("asset:texture-invalid-extension")
        .expect("invalid texture asset should produce diagnostics");
    assert_eq!(
        invalid_texture_diag.status,
        XrdsSceneAssetSourceDiagnosticStatus::InvalidExtension
    );
    assert!(invalid_texture_diag.resolved_path.is_some());
}

#[test]
fn json_round_trip_preserves_document_fidelity() {
    let document = persistence_test_document();

    let json = document
        .to_json_string_pretty()
        .expect("document should serialize to json");
    let decoded =
        XrdsSceneDocument::from_json_str(&json).expect("document should deserialize from json");

    assert_eq!(decoded, document);
}

#[test]
fn save_and_load_json_preserves_document_fidelity() {
    let document = persistence_test_document();
    let unique_suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_nanos();
    let path = std::env::temp_dir().join(format!("xrds-scene-{unique_suffix}.json"));

    document
        .save_json(&path)
        .expect("document should save to json file");
    let loaded = XrdsSceneDocument::load_json(&path).expect("document should load from json file");

    fs::remove_file(&path).expect("temporary json file should be removable");

    assert_eq!(loaded, document);
}

#[test]
fn loading_rejects_unsupported_document_version() {
    let document = persistence_test_document();
    let json = document
        .to_json_string()
        .expect("document should serialize for version test");
    let invalid_json = json.replace(
        &format!("\"version\":{}", XRDS_SCENE_DOCUMENT_VERSION),
        "\"version\":999",
    );

    let error = XrdsSceneDocument::from_json_str(&invalid_json)
        .expect_err("unsupported version should be rejected");

    match error {
        XrdsSceneDocumentPersistenceError::UnsupportedVersion { found, expected } => {
            assert_eq!(found, 999);
            assert_eq!(expected, XRDS_SCENE_DOCUMENT_VERSION);
        }
        other => panic!("expected unsupported version error, got {other:?}"),
    }
}

#[test]
fn session_save_as_and_load_round_trip_preserve_document_and_state() {
    let document = persistence_test_document();
    let unique_suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_nanos();
    let path = std::env::temp_dir().join(format!("xrds-scene-session-{unique_suffix}.json"));

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
    fs::remove_file(&path).expect("temporary json file should be removable");

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
            editor: XrdsEditorMetadata::default(),
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
            editor: XrdsEditorMetadata::default(),
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
                perceptual_roughness: 0.15,
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
            editor: XrdsEditorMetadata::default(),
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
            editor: XrdsEditorMetadata::default(),
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
            editor: XrdsEditorMetadata::default(),
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
fn session_register_and_ensure_image_and_texture_assets_participate_in_undo_redo() {
    let mut session = XrdsSceneDocumentSession::new(XrdsSceneDocument::default())
        .expect("session should accept empty valid document");

    let image = session
        .register_image_asset("asset:image-bricks", "textures/bricks_basecolor.ktx2")
        .expect("explicit image registration should succeed");
    assert_eq!(image.kind, XrdsSceneAssetKind::Image);

    let texture = session
        .ensure_texture_asset(None::<String>, "textures/bricks_basecolor.ktx2")
        .expect("ensure should create a texture asset independently from image assets");
    assert!(texture.created);
    assert_eq!(session.document().assets.len(), 2);

    assert!(session.undo());
    assert_eq!(session.document().assets.len(), 1);
    assert!(session.document().asset("asset:image-bricks").is_some());

    assert!(session.undo());
    assert!(session.document().assets.is_empty());

    assert!(session.redo());
    assert!(session.document().asset("asset:image-bricks").is_some());

    assert!(session.redo());
    assert_eq!(session.document().assets.len(), 2);
    assert!(session.document().asset(&texture.asset.id).is_some());
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
