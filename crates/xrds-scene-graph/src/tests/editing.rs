use super::*;
use crate::XrdsSceneTextureUvTransformMode;

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
            grabbable: false,
            editor: XrdsEditorMetadata::default(),
            triggers: Vec::new(),
            watchers: Vec::new(),
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
                grabbable: false,
                editor: XrdsEditorMetadata::default(),
                triggers: Vec::new(),
                watchers: Vec::new(),
            },
            XrdsSceneNode {
                id: XrdsSceneNodeId(9),
                parent_id: None,
                name: "Empty".to_string(),
                enabled: true,
                visible: true,
                transform: XrdsSceneTransform::default(),
                payload: XrdsSceneNodePayload::Empty,
                grabbable: false,
                editor: XrdsEditorMetadata::default(),
                triggers: Vec::new(),
                watchers: Vec::new(),
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
                    roughness: -0.2,
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
        .set_node_material_roughness(XrdsSceneNodeId(8), 0.18)
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
    assert_eq!(material.pbr.roughness, 0.18);
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
            grabbable: false,
            editor: XrdsEditorMetadata::default(),
            triggers: Vec::new(),
            watchers: Vec::new(),
        }],
        ..Default::default()
    };

    document
        .set_node_material_texture(
            XrdsSceneNodeId(80),
            XrdsSceneMaterialTextureSlotKind::BaseColor,
            Some(XrdsSceneTextureRef {
                texture_asset_id: " asset:texture-bricks ".to_string(),
                uv: XrdsSceneTextureUvParams::default(),
                sampler: XrdsSceneTextureSamplerParams::default(),
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
    assert_eq!(
        textures
            .base_color
            .as_ref()
            .map(|texture| texture.uv.transform_mode),
        Some(XrdsSceneTextureUvTransformMode::Centered)
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
                            uv: XrdsSceneTextureUvParams::default(),
                            sampler: XrdsSceneTextureSamplerParams::default(),
                        }),
                        ..Default::default()
                    },
                    ..Default::default()
                },
                ..Default::default()
            }),
            grabbable: false,
            editor: XrdsEditorMetadata::default(),
            triggers: Vec::new(),
            watchers: Vec::new(),
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
        uri: "models/box.gltf".to_string(),
        kind: XrdsSceneAssetKind::Gltf,
    });

    assert_eq!(
        missing_document.validate(),
        Err(XrdsSceneValidationError::MaterialTextureAssetKindMismatch {
            node_id: XrdsSceneNodeId(81),
            slot: XrdsSceneMaterialTextureSlotKind::BaseColor,
            asset_id: "asset:texture-missing".to_string(),
            found: XrdsSceneAssetKind::Gltf,
        })
    );
}