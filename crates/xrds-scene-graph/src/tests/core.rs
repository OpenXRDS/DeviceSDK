use super::*;

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
            grabbable: false,
            editor: XrdsEditorMetadata::default(),
            triggers: Vec::new(),
            watchers: Vec::new(),
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
        grabbable: false,
        editor: XrdsEditorMetadata::default(),
        triggers: Vec::new(),
        watchers: Vec::new(),
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
            roughness: 0.2,
            reflectance: 0.65,
            double_sided: true,
            alpha_mode: XrdsMaterialAlphaMode::Mask,
            alpha_cutoff: 0.35,
        },
        textures: XrdsMaterialTextureSlots::default(),
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
    assert_eq!(cube_payload.material.pbr.roughness, 0.2);
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
                grabbable: false,
                editor: XrdsEditorMetadata::default(),
                triggers: Vec::new(),
                watchers: Vec::new(),
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
                grabbable: false,
                editor: XrdsEditorMetadata::default(),
                triggers: Vec::new(),
                watchers: Vec::new(),
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
                    roughness: 0.3,
                    reflectance: 0.7,
                    double_sided: true,
                    alpha_mode: XrdsSceneMaterialAlphaMode::Blend,
                    alpha_cutoff: 0.5,
                },
                textures: XrdsSceneMaterialTextureSlots::default(),
            },
            ..Default::default()
        }),
        grabbable: false,
        editor: XrdsEditorMetadata::default(),
        triggers: Vec::new(),
        watchers: Vec::new(),
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
    assert_eq!(material.pbr.roughness, 0.3);
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
    let path = unique_temp_json_path("xrds-scene");

    document
        .save_json(&path)
        .expect("document should save to json file");
    let loaded = XrdsSceneDocument::load_json(&path).expect("document should load from json file");

    std::fs::remove_file(&path).expect("temporary json file should be removable");

    assert_eq!(loaded, document);
}

#[test]
fn ibl_environment_helpers_set_and_clear_scene_environment() {
    let mut document = XrdsSceneDocument {
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
        ],
        ..Default::default()
    };

    document
        .set_ibl_environment("asset:ibl-diffuse", "asset:ibl-specular", 1500.0)
        .expect("ibl environment should be accepted");

    let ibl = document
        .ibl_environment()
        .expect("ibl environment should be stored");
    assert_eq!(ibl.diffuse_asset_id, "asset:ibl-diffuse");
    assert_eq!(ibl.specular_asset_id, "asset:ibl-specular");
    assert_eq!(ibl.intensity, 1500.0);
    assert_eq!(
        document.ibl_environment_asset_ids(),
        Some(("asset:ibl-diffuse", "asset:ibl-specular"))
    );

    document.clear_ibl_environment();
    assert!(document.ibl_environment().is_none());
    assert!(document.environment().is_none());
}

#[test]
fn skybox_environment_helpers_set_and_clear_scene_environment() {
    let mut document = XrdsSceneDocument {
        assets: vec![XrdsSceneAsset {
            id: "asset:skybox".to_string(),
            uri: asset_fixture_path("environment_maps/specular.ktx2"),
            kind: XrdsSceneAssetKind::EnvironmentMap,
        }],
        ..Default::default()
    };

    document
        .set_skybox_environment("asset:skybox", 750.0, 90.0)
        .expect("skybox environment should be accepted");

    let skybox = document
        .skybox_environment()
        .expect("skybox environment should be stored");
    assert_eq!(skybox.texture_asset_id, "asset:skybox");
    assert_eq!(skybox.brightness, 750.0);
    assert_eq!(skybox.yaw_deg, 90.0);
    assert_eq!(document.skybox_environment_asset_id(), Some("asset:skybox"));

    document.clear_skybox_environment();
    assert!(document.skybox_environment().is_none());
    assert!(document.environment().is_none());
}

#[test]
fn exposure_environment_helpers_set_and_clear_scene_environment() {
    let mut document = XrdsSceneDocument::default();

    document
        .set_exposure_environment(6.5)
        .expect("exposure environment should be accepted");

    let exposure = document
        .exposure_environment()
        .expect("exposure environment should be stored");
    assert_eq!(exposure.ev100, 6.5);

    document.clear_exposure_environment();
    assert!(document.exposure_environment().is_none());
    assert!(document.environment().is_none());
}

#[test]
fn fog_environment_helpers_set_and_clear_scene_environment() {
    let mut document = XrdsSceneDocument::default();

    document
        .set_fog_environment(
            [0.35, 0.48, 0.66, 1.0],
            XrdsSceneFogFalloff::Linear { start: 5.0, end: 40.0 },
        )
        .expect("fog environment should be accepted");

    let fog = document
        .fog_environment()
        .expect("fog environment should be stored");
    assert_eq!(fog.color, [0.35, 0.48, 0.66, 1.0]);
    assert_eq!(fog.falloff, XrdsSceneFogFalloff::Linear { start: 5.0, end: 40.0 });

    document.clear_fog_environment();
    assert!(document.fog_environment().is_none());
    assert!(document.environment().is_none());
}

#[test]
fn document_validation_rejects_missing_scene_ibl_asset() {
    let document = XrdsSceneDocument {
        metadata: XrdsSceneMetadata {
            environment: Some(XrdsSceneEnvironment {
                ibl: Some(XrdsSceneIblEnvironment {
                    diffuse_asset_id: "asset:ibl-diffuse".to_string(),
                    specular_asset_id: "asset:ibl-specular".to_string(),
                    intensity: 1200.0,
                }),
                ..Default::default()
            }),
            ..Default::default()
        },
        assets: vec![XrdsSceneAsset {
            id: "asset:ibl-diffuse".to_string(),
            uri: asset_fixture_path("environment_maps/diffuse.ktx2"),
            kind: XrdsSceneAssetKind::EnvironmentMap,
        }],
        ..Default::default()
    };

    assert_eq!(
        document.validate(),
        Err(XrdsSceneValidationError::MissingSceneIblAsset {
            slot: XrdsSceneIblAssetSlot::Specular,
            asset_id: "asset:ibl-specular".to_string(),
        })
    );
}

#[test]
fn document_validation_rejects_missing_scene_skybox_asset() {
    let document = XrdsSceneDocument {
        metadata: XrdsSceneMetadata {
            environment: Some(XrdsSceneEnvironment {
                skybox: Some(XrdsSceneSkyboxEnvironment {
                    texture_asset_id: "asset:skybox".to_string(),
                    brightness: 1200.0,
                    yaw_deg: 0.0,
                }),
                ..Default::default()
            }),
            ..Default::default()
        },
        ..Default::default()
    };

    assert_eq!(
        document.validate(),
        Err(XrdsSceneValidationError::MissingSceneSkyboxAsset {
            slot: XrdsSceneSkyboxAssetSlot::Texture,
            asset_id: "asset:skybox".to_string(),
        })
    );
}

#[test]
fn document_validation_rejects_invalid_scene_exposure() {
    let document = XrdsSceneDocument {
        metadata: XrdsSceneMetadata {
            environment: Some(XrdsSceneEnvironment {
                exposure: Some(XrdsSceneExposureEnvironment { ev100: f32::NAN }),
                ..Default::default()
            }),
            ..Default::default()
        },
        ..Default::default()
    };

    assert_eq!(
        document.validate(),
        Err(XrdsSceneValidationError::InvalidSceneExposureEv100)
    );
}

#[test]
fn document_validation_rejects_invalid_scene_fog_range() {
    let document = XrdsSceneDocument {
        metadata: XrdsSceneMetadata {
            environment: Some(XrdsSceneEnvironment {
                fog: Some(XrdsSceneFogEnvironment {
                    color: [0.35, 0.48, 0.66, 1.0],
                    falloff: XrdsSceneFogFalloff::Linear { start: 20.0, end: 10.0 },
                }),
                ..Default::default()
            }),
            ..Default::default()
        },
        ..Default::default()
    };

    assert_eq!(
        document.validate(),
        Err(XrdsSceneValidationError::InvalidSceneFogRange)
    );
}

#[test]
fn json_round_trip_preserves_scene_ibl_environment() {
    let mut document = persistence_test_document();
    document.assets.extend([
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
    ]);
    document.metadata.environment = Some(XrdsSceneEnvironment {
        ibl: Some(XrdsSceneIblEnvironment {
            diffuse_asset_id: "asset:ibl-diffuse".to_string(),
            specular_asset_id: "asset:ibl-specular".to_string(),
            intensity: 900.0,
        }),
        ..Default::default()
    });

    let json = document
        .to_json_string_pretty()
        .expect("document with scene ibl should serialize");
    let decoded = XrdsSceneDocument::from_json_str(&json)
        .expect("document with scene ibl should deserialize");

    assert_eq!(decoded.metadata.environment, document.metadata.environment);
}

#[test]
fn json_round_trip_preserves_scene_skybox_environment() {
    let mut document = persistence_test_document();
    document.assets.push(XrdsSceneAsset {
        id: "asset:skybox".to_string(),
        uri: asset_fixture_path("environment_maps/specular.ktx2"),
        kind: XrdsSceneAssetKind::EnvironmentMap,
    });
    document.metadata.environment = Some(XrdsSceneEnvironment {
        skybox: Some(XrdsSceneSkyboxEnvironment {
            texture_asset_id: "asset:skybox".to_string(),
            brightness: 640.0,
            yaw_deg: 0.0,
        }),
        ..Default::default()
    });

    let json = document
        .to_json_string_pretty()
        .expect("document with scene skybox should serialize");
    let decoded = XrdsSceneDocument::from_json_str(&json)
        .expect("document with scene skybox should deserialize");

    assert_eq!(decoded.metadata.environment, document.metadata.environment);
}

#[test]
fn json_round_trip_preserves_scene_exposure_environment() {
    let mut document = persistence_test_document();
    document.metadata.environment = Some(XrdsSceneEnvironment {
        exposure: Some(XrdsSceneExposureEnvironment { ev100: 7.25 }),
        ..Default::default()
    });

    let json = document
        .to_json_string_pretty()
        .expect("document with scene exposure should serialize");
    let decoded = XrdsSceneDocument::from_json_str(&json)
        .expect("document with scene exposure should deserialize");

    assert_eq!(decoded.metadata.environment, document.metadata.environment);
}

#[test]
fn json_round_trip_preserves_scene_fog_environment() {
    let mut document = persistence_test_document();
    document.metadata.environment = Some(XrdsSceneEnvironment {
        fog: Some(XrdsSceneFogEnvironment {
            color: [0.35, 0.48, 0.66, 1.0],
            falloff: XrdsSceneFogFalloff::Linear { start: 5.0, end: 40.0 },
        }),
        ..Default::default()
    });

    let json = document
        .to_json_string_pretty()
        .expect("document with scene fog should serialize");
    let decoded = XrdsSceneDocument::from_json_str(&json)
        .expect("document with scene fog should deserialize");

    assert_eq!(decoded.metadata.environment, document.metadata.environment);
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

// ── Fog falloff modes ────────────────────────────────────────────────────

/// Scenes saved before falloff modes existed carry `start`/`end` on the fog object
/// itself. They must still load, as linear fog.
///
/// This is why `XrdsSceneFogEnvironment` has a hand-written `Deserialize`. A
/// derived one would ignore the unknown fields and silently reset every existing
/// scene's fog to the default — data loss that no other test would see, because
/// nothing else reads a file written by an older build.
#[test]
fn a_scene_saved_before_falloff_modes_still_loads_as_linear_fog() {
    let legacy = r#"{ "color": [0.35, 0.48, 0.66, 1.0], "start": 5.0, "end": 40.0 }"#;
    let fog: XrdsSceneFogEnvironment =
        serde_json::from_str(legacy).expect("a pre-falloff fog object must still deserialize");

    assert_eq!(fog.color, [0.35, 0.48, 0.66, 1.0]);
    assert_eq!(
        fog.falloff,
        XrdsSceneFogFalloff::Linear { start: 5.0, end: 40.0 },
        "the authored distances must survive, not be replaced by defaults"
    );
}

#[test]
fn every_falloff_mode_round_trips_through_json() {
    for falloff in [
        XrdsSceneFogFalloff::Linear { start: 5.0, end: 40.0 },
        XrdsSceneFogFalloff::Exponential { visibility: 80.0 },
        XrdsSceneFogFalloff::ExponentialSquared { visibility: 120.0 },
    ] {
        let fog = XrdsSceneFogEnvironment { color: [0.3, 0.4, 0.5, 1.0], falloff };
        let json = serde_json::to_string(&fog).expect("serialize");
        let back: XrdsSceneFogEnvironment = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(back, fog, "round trip changed {falloff:?}");
    }
}

/// Both failure modes render as artefacts rather than errors, so they are refused
/// where an author can still be told: an inverted ramp, and a visibility of zero
/// that divides by zero inside Koschmieder's equation.
#[test]
fn fog_rejects_an_inverted_ramp_and_a_non_positive_visibility() {
    let mut doc = XrdsSceneDocument::default();
    let color = [0.35, 0.48, 0.66, 1.0];

    assert!(doc
        .set_fog_environment(color, XrdsSceneFogFalloff::Linear { start: 20.0, end: 10.0 })
        .is_err());
    assert!(doc
        .set_fog_environment(color, XrdsSceneFogFalloff::Exponential { visibility: 0.0 })
        .is_err());
    assert!(doc
        .set_fog_environment(color, XrdsSceneFogFalloff::ExponentialSquared { visibility: -5.0 })
        .is_err());
    assert!(doc
        .set_fog_environment(color, XrdsSceneFogFalloff::Exponential { visibility: f32::NAN })
        .is_err());

    // And the valid ones are accepted, so the guard is not simply refusing
    // everything.
    assert!(doc
        .set_fog_environment(color, XrdsSceneFogFalloff::Exponential { visibility: 80.0 })
        .is_ok());
    assert_eq!(
        doc.fog_environment().unwrap().falloff,
        XrdsSceneFogFalloff::Exponential { visibility: 80.0 }
    );
}

#[test]
fn sanitizing_repairs_rather_than_rejects_for_live_editing() {
    // A slider dragged past its partner should keep rendering, not blank out — so
    // the editor path repairs instead of refusing.
    match (XrdsSceneFogFalloff::Linear { start: 30.0, end: 10.0 }).sanitized() {
        XrdsSceneFogFalloff::Linear { start, end } => assert!(end > start),
        other => panic!("mode changed: {other:?}"),
    }
    match (XrdsSceneFogFalloff::Exponential { visibility: 0.0 }).sanitized() {
        XrdsSceneFogFalloff::Exponential { visibility } => assert!(visibility > 0.0),
        other => panic!("mode changed: {other:?}"),
    }
    // A already-valid value is left exactly alone.
    let good = XrdsSceneFogFalloff::Exponential { visibility: 80.0 };
    assert_eq!(good.sanitized(), good);
}
