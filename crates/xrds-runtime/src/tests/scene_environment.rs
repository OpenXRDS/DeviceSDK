use super::*;

#[test]
fn import_scene_document_applies_authored_exposure_to_imported_cameras() {
    let mut app = xrds_test_app();
    let document = XrdsSceneDocument {
        metadata: XrdsSceneMetadata {
            environment: Some(XrdsSceneEnvironment {
                exposure: Some(XrdsSceneExposureEnvironment { ev100: 6.0 }),
                ..Default::default()
            }),
            ..Default::default()
        },
        nodes: vec![XrdsSceneNode {
            id: XrdsSceneNodeId(502),
            parent_id: None,
            name: "Camera".to_string(),
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

    {
        let mut xrds = XrdsAPI::attach(&mut app);
        xrds.import_scene_document(&document)
            .expect("document should import");
    }

    let camera_entity = app
        .world()
        .resource::<XrdsIdIndex>()
        .entity_of(XrdsId(502))
        .expect("camera entity should exist");
    let exposure = app
        .world()
        .get::<Exposure>(camera_entity)
        .expect("imported camera should receive exposure policy");
    assert_eq!(exposure.ev100, 6.0);
}

#[test]
fn import_scene_document_applies_authored_fog_to_imported_cameras() {
    let mut app = xrds_test_app();
    let document = XrdsSceneDocument {
        metadata: XrdsSceneMetadata {
            environment: Some(XrdsSceneEnvironment {
                fog: Some(XrdsSceneFogEnvironment {
                    color: [0.35, 0.48, 0.66, 1.0],
                    start: 5.0,
                    end: 40.0,
                }),
                ..Default::default()
            }),
            ..Default::default()
        },
        nodes: vec![XrdsSceneNode {
            id: XrdsSceneNodeId(503),
            parent_id: None,
            name: "Camera".to_string(),
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

    {
        let mut xrds = XrdsAPI::attach(&mut app);
        xrds.import_scene_document(&document)
            .expect("document should import");
    }

    let camera_entity = app
        .world()
        .resource::<XrdsIdIndex>()
        .entity_of(XrdsId(503))
        .expect("camera entity should exist");
    let fog = app
        .world()
        .get::<DistanceFog>(camera_entity)
        .expect("imported camera should receive fog policy");
    assert_eq!(fog.color.to_srgba().to_f32_array(), [0.35, 0.48, 0.66, 1.0]);
    match &fog.falloff {
        FogFalloff::Linear { start, end } => {
            assert_eq!((*start, *end), (5.0, 40.0));
        }
        other => panic!("expected linear fog falloff, got {other:?}"),
    }
}

#[test]
fn import_scene_document_applies_authored_ibl_environment_to_imported_cameras() {
    let mut app = xrds_test_app();
    let document = XrdsSceneDocument {
        metadata: XrdsSceneMetadata {
            environment: Some(XrdsSceneEnvironment {
                ibl: Some(XrdsSceneIblEnvironment {
                    diffuse_asset_id: "asset:ibl-diffuse".to_string(),
                    specular_asset_id: "asset:ibl-specular".to_string(),
                    intensity: 1234.0,
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
            id: XrdsSceneNodeId(500),
            parent_id: None,
            name: "Camera".to_string(),
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

    let imported_ids = {
        let mut xrds = XrdsAPI::attach(&mut app);
        xrds.import_scene_document(&document)
            .expect("document should import")
    };

    assert_eq!(imported_ids, vec![XrdsId(500)]);

    let camera_entity = app
        .world()
        .resource::<XrdsIdIndex>()
        .entity_of(XrdsId(500))
        .expect("camera entity should exist");
    let environment = app
        .world()
        .get::<EnvironmentMapLight>(camera_entity)
        .expect("imported camera should receive environment map light");
    assert_eq!(environment.intensity, 1234.0);

    let asset_server = app.world().resource::<AssetServer>();
    assert_eq!(
        environment.diffuse_map,
        asset_server.load::<Image>("environment_maps/diffuse.ktx2")
    );
    assert_eq!(
        environment.specular_map,
        asset_server.load::<Image>("environment_maps/specular.ktx2")
    );
}


/// An authored yaw must reach the `Skybox` component, and reach it as a rotation
/// about the vertical axis.
///
/// `Skybox::rotation` is applied in view space — it rotates the sampling direction
/// rather than the sky — so the runtime negates the authored yaw to make a positive
/// value turn the sky the way an author expects. That sign is invisible in review
/// and obvious the moment someone drags the slider, so it is pinned here.
#[test]
fn an_authored_skybox_yaw_reaches_the_component() {
    let mut app = xrds_test_app();
    app.insert_resource(xrds_openxr::OpenXrPassthroughEnabled(false));

    let mut document = skybox_document();
    if let Some(env) = document.metadata.environment.as_mut() {
        if let Some(sky) = env.skybox.as_mut() {
            sky.yaw_deg = 90.0;
        }
    }

    {
        let mut xrds = XrdsAPI::attach(&mut app);
        xrds.import_scene_document(&document)
            .expect("document should import");
    }

    let camera_entity = app
        .world()
        .resource::<XrdsIdIndex>()
        .entity_of(XrdsId(501))
        .expect("camera entity should exist");
    let skybox = app
        .world()
        .get::<Skybox>(camera_entity)
        .expect("camera should receive the skybox");

    let expected = Quat::from_rotation_y(-90f32.to_radians());
    // Compared by dot product: `q` and `-q` are the same rotation, so a
    // component-wise assert would fail for a correct value.
    let dot = skybox.rotation.dot(expected).abs();
    assert!(
        (dot - 1.0).abs() < 1e-5,
        "expected a -90 degree yaw about Y, got {:?}",
        skybox.rotation,
    );
}

/// A procedural sky is applied when authored, and suppressed under passthrough for
/// the same reason a skybox is: it paints over the transparent clear the
/// passthrough layer needs, so the author would tick Passthrough and get a computed
/// sky instead of the room.
#[test]
fn atmosphere_applies_when_authored_and_yields_to_passthrough() {
    use bevy::pbr::Atmosphere;

    fn scene_with_atmosphere() -> XrdsSceneDocument {
        let mut document = skybox_document();
        if let Some(env) = document.metadata.environment.as_mut() {
            env.skybox = None; // atmosphere alone, so the skybox cannot mask the result
            env.atmosphere = Some(Default::default());
        }
        document
    }

    for (passthrough, expected) in [(false, true), (true, false)] {
        let mut app = xrds_test_app();
        app.insert_resource(xrds_openxr::OpenXrPassthroughEnabled(passthrough));
        {
            let mut xrds = XrdsAPI::attach(&mut app);
            xrds.import_scene_document(&scene_with_atmosphere())
                .expect("document should import");
        }

        let camera_entity = app
            .world()
            .resource::<XrdsIdIndex>()
            .entity_of(XrdsId(501))
            .expect("camera entity should exist");
        assert_eq!(
            app.world().get::<Atmosphere>(camera_entity).is_some(),
            expected,
            "passthrough={passthrough} should give atmosphere present={expected}",
        );
    }
}

/// The sky and the lighting must turn *together*.
///
/// They did not, at first: Bevy documents `Skybox::rotation` as view space and
/// `EnvironmentMapLight::rotation` as world space, so this code used opposite signs
/// — and the reflection on a metal sphere turned the opposite way to the sky, which
/// is worse than either being individually inverted. The wording differs; the
/// behaviour does not.
///
/// Asserted as *equality between the two*, not against a literal quaternion,
/// because that is the property that matters and it survives someone later
/// discovering the absolute direction is backwards.
#[test]
fn skybox_and_environment_light_rotate_together() {
    let mut app = xrds_test_app();
    app.insert_resource(xrds_openxr::OpenXrPassthroughEnabled(false));

    let mut document = skybox_document();
    if let Some(env) = document.metadata.environment.as_mut() {
        if let Some(sky) = env.skybox.as_mut() {
            sky.yaw_deg = 55.0;
        }
        env.ibl = Some(XrdsSceneIblEnvironment {
            diffuse_asset_id: "asset:skybox".to_string(),
            specular_asset_id: "asset:skybox".to_string(),
            intensity: 1000.0,
        });
    }

    {
        let mut xrds = XrdsAPI::attach(&mut app);
        xrds.import_scene_document(&document)
            .expect("document should import");
    }

    let camera_entity = app
        .world()
        .resource::<XrdsIdIndex>()
        .entity_of(XrdsId(501))
        .expect("camera entity should exist");

    let sky_rotation = app
        .world()
        .get::<Skybox>(camera_entity)
        .expect("camera should receive the skybox")
        .rotation;
    let ibl_rotation = app
        .world()
        .get::<EnvironmentMapLight>(camera_entity)
        .expect("camera should receive the environment light")
        .rotation;

    let dot = sky_rotation.dot(ibl_rotation).abs();
    assert!(
        (dot - 1.0).abs() < 1e-5,
        "sky and lighting must share a rotation, got {sky_rotation:?} vs {ibl_rotation:?}",
    );
}

/// A skybox and passthrough are mutually exclusive, and the conflict is silent
/// without this: the skybox is inserted on every `Camera3d` — the XR eye cameras
/// included — and draws into the main pass, painting over the transparent clear
/// passthrough needs. The projection layer ends up opaque everywhere, the
/// passthrough layer beneath is never revealed, and the author sees their skybox
/// with no indication of why the real world never appears.
#[test]
fn passthrough_suppresses_an_authored_skybox() {
    let mut app = xrds_test_app();
    app.insert_resource(xrds_openxr::OpenXrPassthroughEnabled(true));

    let document = skybox_document();
    {
        let mut xrds = XrdsAPI::attach(&mut app);
        xrds.import_scene_document(&document)
            .expect("document should import");
    }

    let camera_entity = app
        .world()
        .resource::<XrdsIdIndex>()
        .entity_of(XrdsId(501))
        .expect("camera entity should exist");
    assert!(
        app.world().get::<Skybox>(camera_entity).is_none(),
        "an authored skybox must not be applied while passthrough is on — it would \
         hide the real world with no way for the author to tell why",
    );
}

/// The same scene without passthrough still gets its skybox, so the suppression
/// above is conditional rather than a regression in skybox support.
#[test]
fn a_skybox_still_applies_when_passthrough_is_off() {
    let mut app = xrds_test_app();
    app.insert_resource(xrds_openxr::OpenXrPassthroughEnabled(false));

    let document = skybox_document();
    {
        let mut xrds = XrdsAPI::attach(&mut app);
        xrds.import_scene_document(&document)
            .expect("document should import");
    }

    let camera_entity = app
        .world()
        .resource::<XrdsIdIndex>()
        .entity_of(XrdsId(501))
        .expect("camera entity should exist");
    assert!(
        app.world().get::<Skybox>(camera_entity).is_some(),
        "skybox should apply normally when passthrough is off",
    );
}

/// One camera, one authored skybox — the shared fixture for the two tests above.
fn skybox_document() -> XrdsSceneDocument {
    XrdsSceneDocument {
        metadata: XrdsSceneMetadata {
            environment: Some(XrdsSceneEnvironment {
                skybox: Some(XrdsSceneSkyboxEnvironment {
                    texture_asset_id: "asset:skybox".to_string(),
                    brightness: 321.0,
                    yaw_deg: 0.0,
                }),
                ..Default::default()
            }),
            ..Default::default()
        },
        assets: vec![XrdsSceneAsset {
            id: "asset:skybox".to_string(),
            uri: "environment_maps/specular.ktx2".to_string(),
            kind: XrdsSceneAssetKind::EnvironmentMap,
        }],
        nodes: vec![XrdsSceneNode {
            id: XrdsSceneNodeId(501),
            parent_id: None,
            name: "Camera".to_string(),
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
    }
}

#[test]
fn import_scene_document_applies_authored_skybox_to_imported_cameras() {
    let mut app = xrds_test_app();
    let document = XrdsSceneDocument {
        metadata: XrdsSceneMetadata {
            environment: Some(XrdsSceneEnvironment {
                skybox: Some(XrdsSceneSkyboxEnvironment {
                    texture_asset_id: "asset:skybox".to_string(),
                    brightness: 321.0,
                    yaw_deg: 0.0,
                }),
                ..Default::default()
            }),
            ..Default::default()
        },
        assets: vec![XrdsSceneAsset {
            id: "asset:skybox".to_string(),
            uri: "environment_maps/specular.ktx2".to_string(),
            kind: XrdsSceneAssetKind::EnvironmentMap,
        }],
        nodes: vec![XrdsSceneNode {
            id: XrdsSceneNodeId(501),
            parent_id: None,
            name: "Camera".to_string(),
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

    {
        let mut xrds = XrdsAPI::attach(&mut app);
        xrds.import_scene_document(&document)
            .expect("document should import");
    }

    let camera_entity = app
        .world()
        .resource::<XrdsIdIndex>()
        .entity_of(XrdsId(501))
        .expect("camera entity should exist");
    let skybox = app
        .world()
        .get::<Skybox>(camera_entity)
        .expect("imported camera should receive skybox policy");
    assert_eq!(skybox.brightness, 321.0);

    let asset_server = app.world().resource::<AssetServer>();
    assert_eq!(
        skybox.image,
        asset_server.load::<Image>("environment_maps/specular.ktx2")
    );
}


#[test]
fn scene_environment_policy_applies_to_non_imported_cameras() {
    let mut app = xrds_test_app();
    let document = XrdsSceneDocument {
        metadata: XrdsSceneMetadata {
            environment: Some(XrdsSceneEnvironment {
                ibl: Some(XrdsSceneIblEnvironment {
                    diffuse_asset_id: "asset:ibl-diffuse".to_string(),
                    specular_asset_id: "asset:ibl-specular".to_string(),
                    intensity: 700.0,
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
            id: XrdsSceneNodeId(505),
            parent_id: None,
            name: "ImportedCamera".to_string(),
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

    {
        let mut xrds = XrdsAPI::attach(&mut app);
        xrds.import_scene_document(&document)
            .expect("document should import");
    }

    let camera_entity = app.world_mut().spawn(Camera3d::default()).id();
    app.update();

    let environment = app
        .world()
        .get::<EnvironmentMapLight>(camera_entity)
        .expect("non-imported camera should inherit the active scene environment policy");
    assert_eq!(environment.intensity, 700.0);
}


#[test]
fn scene_environment_policy_preserves_explicit_camera_environment() {
    let mut app = xrds_test_app();
    let document = XrdsSceneDocument {
        metadata: XrdsSceneMetadata {
            environment: Some(XrdsSceneEnvironment {
                ibl: Some(XrdsSceneIblEnvironment {
                    diffuse_asset_id: "asset:ibl-diffuse".to_string(),
                    specular_asset_id: "asset:ibl-specular".to_string(),
                    intensity: 500.0,
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
        ..Default::default()
    };

    {
        let mut xrds = XrdsAPI::attach(&mut app);
        xrds.import_scene_document(&document)
            .expect("document should import");
    }

    let explicit_diffuse = {
        let asset_server = app.world().resource::<AssetServer>();
        asset_server.load::<Image>("environment_maps/specular.ktx2")
    };
    let explicit_specular = {
        let asset_server = app.world().resource::<AssetServer>();
        asset_server.load::<Image>("environment_maps/diffuse.ktx2")
    };

    let camera_entity = app
        .world_mut()
        .spawn((
            Camera3d::default(),
            EnvironmentMapLight {
                diffuse_map: explicit_diffuse.clone(),
                specular_map: explicit_specular.clone(),
                intensity: 42.0,
                ..default()
            },
        ))
        .id();
    app.update();

    let environment = app
        .world()
        .get::<EnvironmentMapLight>(camera_entity)
        .expect("explicit camera environment should remain attached");
    assert_eq!(environment.intensity, 42.0);
    assert_eq!(environment.diffuse_map, explicit_diffuse);
    assert_eq!(environment.specular_map, explicit_specular);
}


#[test]
fn scene_environment_policy_preserves_explicit_camera_skybox() {
    let mut app = xrds_test_app();
    let document = XrdsSceneDocument {
        metadata: XrdsSceneMetadata {
            environment: Some(XrdsSceneEnvironment {
                skybox: Some(XrdsSceneSkyboxEnvironment {
                    texture_asset_id: "asset:skybox".to_string(),
                    brightness: 500.0,
                    yaw_deg: 0.0,
                }),
                ..Default::default()
            }),
            ..Default::default()
        },
        assets: vec![XrdsSceneAsset {
            id: "asset:skybox".to_string(),
            uri: "environment_maps/specular.ktx2".to_string(),
            kind: XrdsSceneAssetKind::EnvironmentMap,
        }],
        ..Default::default()
    };

    {
        let mut xrds = XrdsAPI::attach(&mut app);
        xrds.import_scene_document(&document)
            .expect("document should import");
    }

    let explicit_image = {
        let asset_server = app.world().resource::<AssetServer>();
        asset_server.load::<Image>("environment_maps/diffuse.ktx2")
    };

    let camera_entity = app
        .world_mut()
        .spawn((
            Camera3d::default(),
            Skybox {
                image: explicit_image.clone(),
                brightness: 42.0,
                ..default()
            },
        ))
        .id();
    app.update();

    let skybox = app
        .world()
        .get::<Skybox>(camera_entity)
        .expect("explicit camera skybox should remain attached");
    assert_eq!(skybox.brightness, 42.0);
    assert_eq!(skybox.image, explicit_image);
}

#[test]
fn scene_environment_policy_preserves_explicit_camera_exposure() {
    let mut app = xrds_test_app();
    let document = XrdsSceneDocument {
        metadata: XrdsSceneMetadata {
            environment: Some(XrdsSceneEnvironment {
                exposure: Some(XrdsSceneExposureEnvironment { ev100: 7.0 }),
                ..Default::default()
            }),
            ..Default::default()
        },
        ..Default::default()
    };

    {
        let mut xrds = XrdsAPI::attach(&mut app);
        xrds.import_scene_document(&document)
            .expect("document should import");
    }

    let camera_entity = app
        .world_mut()
        .spawn((Camera3d::default(), Exposure { ev100: 3.25 }))
        .id();
    app.update();

    let exposure = app
        .world()
        .get::<Exposure>(camera_entity)
        .expect("explicit camera exposure should remain attached");
    assert_eq!(exposure.ev100, 3.25);
}

#[test]
fn scene_environment_policy_preserves_explicit_camera_fog() {
    let mut app = xrds_test_app();
    let document = XrdsSceneDocument {
        metadata: XrdsSceneMetadata {
            environment: Some(XrdsSceneEnvironment {
                fog: Some(XrdsSceneFogEnvironment {
                    color: [0.35, 0.48, 0.66, 1.0],
                    start: 5.0,
                    end: 40.0,
                }),
                ..Default::default()
            }),
            ..Default::default()
        },
        ..Default::default()
    };

    {
        let mut xrds = XrdsAPI::attach(&mut app);
        xrds.import_scene_document(&document)
            .expect("document should import");
    }

    let camera_entity = app
        .world_mut()
        .spawn((
            Camera3d::default(),
            DistanceFog {
                color: Color::srgba(0.1, 0.2, 0.3, 1.0),
                falloff: FogFalloff::Linear { start: 1.0, end: 8.0 },
                ..default()
            },
        ))
        .id();
    app.update();

    let fog = app
        .world()
        .get::<DistanceFog>(camera_entity)
        .expect("explicit camera fog should remain attached");
    assert_eq!(fog.color.to_srgba().to_f32_array(), [0.1, 0.2, 0.3, 1.0]);
    match &fog.falloff {
        FogFalloff::Linear { start, end } => {
            assert_eq!((*start, *end), (1.0, 8.0));
        }
        other => panic!("expected linear fog falloff, got {other:?}"),
    }
}


#[test]
fn scene_environment_policy_clears_managed_camera_environment_when_removed() {
    let mut app = xrds_test_app();
    let document = XrdsSceneDocument {
        metadata: XrdsSceneMetadata {
            environment: Some(XrdsSceneEnvironment {
                ibl: Some(XrdsSceneIblEnvironment {
                    diffuse_asset_id: "asset:ibl-diffuse".to_string(),
                    specular_asset_id: "asset:ibl-specular".to_string(),
                    intensity: 250.0,
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
        ..Default::default()
    };

    {
        let mut xrds = XrdsAPI::attach(&mut app);
        xrds.import_scene_document(&document)
            .expect("document should import");
    }

    let camera_entity = app.world_mut().spawn(Camera3d::default()).id();
    app.update();
    assert!(app
        .world()
        .get::<EnvironmentMapLight>(camera_entity)
        .is_some());

    store_imported_scene_environment_in_world(app.world_mut(), None);
    app.update();

    assert!(
        app.world()
            .get::<EnvironmentMapLight>(camera_entity)
            .is_none(),
        "managed camera environment should be removed when the scene policy is cleared"
    );
}


#[test]
fn scene_environment_policy_clears_managed_camera_skybox_when_removed() {
    let mut app = xrds_test_app();
    let document = XrdsSceneDocument {
        metadata: XrdsSceneMetadata {
            environment: Some(XrdsSceneEnvironment {
                skybox: Some(XrdsSceneSkyboxEnvironment {
                    texture_asset_id: "asset:skybox".to_string(),
                    brightness: 250.0,
                    yaw_deg: 0.0,
                }),
                ..Default::default()
            }),
            ..Default::default()
        },
        assets: vec![XrdsSceneAsset {
            id: "asset:skybox".to_string(),
            uri: "environment_maps/specular.ktx2".to_string(),
            kind: XrdsSceneAssetKind::EnvironmentMap,
        }],
        ..Default::default()
    };

    {
        let mut xrds = XrdsAPI::attach(&mut app);
        xrds.import_scene_document(&document)
            .expect("document should import");
    }

    let camera_entity = app.world_mut().spawn(Camera3d::default()).id();
    app.update();
    assert!(app.world().get::<Skybox>(camera_entity).is_some());

    store_imported_scene_environment_in_world(app.world_mut(), None);
    app.update();

    assert!(
        app.world().get::<Skybox>(camera_entity).is_none(),
        "managed camera skybox should be removed when the scene policy is cleared"
    );
}

#[test]
fn scene_environment_policy_clears_managed_camera_exposure_when_removed() {
    let mut app = xrds_test_app();
    let document = XrdsSceneDocument {
        metadata: XrdsSceneMetadata {
            environment: Some(XrdsSceneEnvironment {
                exposure: Some(XrdsSceneExposureEnvironment { ev100: 5.5 }),
                ..Default::default()
            }),
            ..Default::default()
        },
        ..Default::default()
    };

    {
        let mut xrds = XrdsAPI::attach(&mut app);
        xrds.import_scene_document(&document)
            .expect("document should import");
    }

    let camera_entity = app.world_mut().spawn(Camera3d::default()).id();
    app.update();
    assert!(app.world().get::<Exposure>(camera_entity).is_some());

    store_imported_scene_environment_in_world(app.world_mut(), None);
    app.update();

    assert!(
        app.world().get::<Exposure>(camera_entity).is_none(),
        "managed camera exposure should be removed when the scene policy is cleared"
    );
}

#[test]
fn scene_environment_policy_clears_managed_camera_fog_when_removed() {
    let mut app = xrds_test_app();
    let document = XrdsSceneDocument {
        metadata: XrdsSceneMetadata {
            environment: Some(XrdsSceneEnvironment {
                fog: Some(XrdsSceneFogEnvironment {
                    color: [0.35, 0.48, 0.66, 1.0],
                    start: 5.0,
                    end: 40.0,
                }),
                ..Default::default()
            }),
            ..Default::default()
        },
        ..Default::default()
    };

    {
        let mut xrds = XrdsAPI::attach(&mut app);
        xrds.import_scene_document(&document)
            .expect("document should import");
    }

    let camera_entity = app.world_mut().spawn(Camera3d::default()).id();
    app.update();
    assert!(app.world().get::<DistanceFog>(camera_entity).is_some());

    store_imported_scene_environment_in_world(app.world_mut(), None);
    app.update();

    assert!(
        app.world().get::<DistanceFog>(camera_entity).is_none(),
        "managed camera fog should be removed when the scene policy is cleared"
    );
}


#[test]
fn export_scene_document_preserves_imported_ibl_environment_by_default() {
    let mut app = xrds_test_app();
    let document = XrdsSceneDocument {
        metadata: XrdsSceneMetadata {
            environment: Some(XrdsSceneEnvironment {
                ibl: Some(XrdsSceneIblEnvironment {
                    diffuse_asset_id: "asset:ibl-diffuse".to_string(),
                    specular_asset_id: "asset:ibl-specular".to_string(),
                    intensity: 600.0,
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
            id: XrdsSceneNodeId(510),
            parent_id: None,
            name: "Camera".to_string(),
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

    let exported = {
        let mut xrds = XrdsAPI::attach(&mut app);
        xrds.import_scene_document(&document)
            .expect("document should import");
        xrds.export_scene_document()
            .expect("document should export")
    };

    assert_eq!(exported.metadata.environment, document.metadata.environment);
}


#[test]
fn export_scene_document_preserves_imported_skybox_environment_by_default() {
    let mut app = xrds_test_app();
    let document = XrdsSceneDocument {
        metadata: XrdsSceneMetadata {
            environment: Some(XrdsSceneEnvironment {
                skybox: Some(XrdsSceneSkyboxEnvironment {
                    texture_asset_id: "asset:skybox".to_string(),
                    brightness: 600.0,
                    yaw_deg: 0.0,
                }),
                ..Default::default()
            }),
            ..Default::default()
        },
        assets: vec![XrdsSceneAsset {
            id: "asset:skybox".to_string(),
            uri: "environment_maps/specular.ktx2".to_string(),
            kind: XrdsSceneAssetKind::EnvironmentMap,
        }],
        nodes: vec![XrdsSceneNode {
            id: XrdsSceneNodeId(511),
            parent_id: None,
            name: "Camera".to_string(),
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

    let exported = {
        let mut xrds = XrdsAPI::attach(&mut app);
        xrds.import_scene_document(&document)
            .expect("document should import");
        xrds.export_scene_document()
            .expect("document should export")
    };

    assert_eq!(exported.metadata.environment, document.metadata.environment);
}

#[test]
fn export_scene_document_preserves_imported_exposure_by_default() {
    let mut app = xrds_test_app();
    let document = XrdsSceneDocument {
        metadata: XrdsSceneMetadata {
            environment: Some(XrdsSceneEnvironment {
                exposure: Some(XrdsSceneExposureEnvironment { ev100: 8.0 }),
                ..Default::default()
            }),
            ..Default::default()
        },
        nodes: vec![XrdsSceneNode {
            id: XrdsSceneNodeId(512),
            parent_id: None,
            name: "Camera".to_string(),
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

    let exported = {
        let mut xrds = XrdsAPI::attach(&mut app);
        xrds.import_scene_document(&document)
            .expect("document should import");
        xrds.export_scene_document()
            .expect("document should export")
    };

    assert_eq!(exported.metadata.environment, document.metadata.environment);
}

#[test]
fn export_scene_document_preserves_imported_fog_by_default() {
    let mut app = xrds_test_app();
    let document = XrdsSceneDocument {
        metadata: XrdsSceneMetadata {
            environment: Some(XrdsSceneEnvironment {
                fog: Some(XrdsSceneFogEnvironment {
                    color: [0.35, 0.48, 0.66, 1.0],
                    start: 5.0,
                    end: 40.0,
                }),
                ..Default::default()
            }),
            ..Default::default()
        },
        nodes: vec![XrdsSceneNode {
            id: XrdsSceneNodeId(513),
            parent_id: None,
            name: "Camera".to_string(),
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

    let exported = {
        let mut xrds = XrdsAPI::attach(&mut app);
        xrds.import_scene_document(&document)
            .expect("document should import");
        xrds.export_scene_document()
            .expect("document should export")
    };

    assert_eq!(exported.metadata.environment, document.metadata.environment);
}


#[test]
fn set_scene_environment_applies_runtime_policy_to_existing_camera() {
    let mut app = xrds_test_app();
    let camera_entity = app.world_mut().spawn(Camera3d::default()).id();

    {
        let mut xrds = XrdsAPI::attach(&mut app);
        xrds.merge_scene_assets(&[
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
        ])
        .set_scene_environment(XrdsSceneEnvironment {
            ibl: Some(XrdsSceneIblEnvironment {
                diffuse_asset_id: "asset:ibl-diffuse".to_string(),
                specular_asset_id: "asset:ibl-specular".to_string(),
                intensity: 880.0,
            }),
            ..Default::default()
        });
    }

    let environment = app
        .world()
        .get::<EnvironmentMapLight>(camera_entity)
        .expect("runtime scene environment should apply to existing cameras");
    assert_eq!(environment.intensity, 880.0);
}

#[test]
fn set_scene_environment_supports_distinct_asset_ids_for_same_texture_uri() {
    let mut app = xrds_test_app();
    let camera_entity = app.world_mut().spawn(Camera3d::default()).id();

    {
        let mut xrds = XrdsAPI::attach(&mut app);
        xrds.merge_scene_assets(&[
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
        ])
        .set_scene_environment(XrdsSceneEnvironment {
            ibl: Some(XrdsSceneIblEnvironment {
                diffuse_asset_id: "asset:ibl-diffuse".to_string(),
                specular_asset_id: "asset:ibl-specular".to_string(),
                intensity: 880.0,
            }),
            skybox: Some(XrdsSceneSkyboxEnvironment {
                texture_asset_id: "asset:skybox".to_string(),
                brightness: 777.0,
                yaw_deg: 0.0,
            }),
            ..Default::default()
        });
    }

    let world = app.world();
    let environment = world
        .get::<EnvironmentMapLight>(camera_entity)
        .expect("runtime scene environment should apply to existing cameras");
    let skybox = world
        .get::<Skybox>(camera_entity)
        .expect("runtime scene skybox should apply to existing cameras");
    let asset_server = world.resource::<AssetServer>();

    assert_eq!(environment.intensity, 880.0);
    assert_eq!(skybox.brightness, 777.0);
    assert_eq!(
        environment.specular_map,
        asset_server.load::<Image>("environment_maps/specular.ktx2")
    );
    assert_eq!(
        skybox.image,
        asset_server.load::<Image>("environment_maps/specular.ktx2")
    );
}


#[test]
fn clear_scene_environment_removes_runtime_policy_from_managed_cameras() {
    let mut app = xrds_test_app();
    let camera_entity = app.world_mut().spawn(Camera3d::default()).id();

    {
        let mut xrds = XrdsAPI::attach(&mut app);
        xrds.merge_scene_assets(&[
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
        ])
        .set_scene_environment(XrdsSceneEnvironment {
            ibl: Some(XrdsSceneIblEnvironment {
                diffuse_asset_id: "asset:ibl-diffuse".to_string(),
                specular_asset_id: "asset:ibl-specular".to_string(),
                intensity: 510.0,
            }),
            ..Default::default()
        });
        xrds.clear_scene_environment();
    }

    assert!(
        app.world()
            .get::<EnvironmentMapLight>(camera_entity)
            .is_none(),
        "clearing the runtime scene environment should remove managed camera environment maps"
    );
}


#[test]
fn export_scene_document_preserves_runtime_set_scene_environment() {
    let mut app = xrds_test_app();

    let exported = {
        let mut xrds = XrdsAPI::attach(&mut app);
        let environment = XrdsSceneEnvironment {
            ibl: Some(XrdsSceneIblEnvironment {
                diffuse_asset_id: "asset:ibl-diffuse".to_string(),
                specular_asset_id: "asset:ibl-specular".to_string(),
                intensity: 444.0,
            }),
            ..Default::default()
        };
        xrds.merge_scene_assets(&[
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
        ])
        .set_scene_environment(environment.clone());
        assert_eq!(xrds.scene_environment(), Some(environment.clone()));
        xrds.export_scene_document()
            .expect("document should export with runtime scene environment")
    };

    assert_eq!(
        exported.metadata.environment,
        Some(XrdsSceneEnvironment {
            ibl: Some(XrdsSceneIblEnvironment {
                diffuse_asset_id: "asset:ibl-diffuse".to_string(),
                specular_asset_id: "asset:ibl-specular".to_string(),
                intensity: 444.0,
            }),
            ..Default::default()
        })
    );
}

#[test]
fn set_scene_environment_applies_runtime_exposure_policy_to_existing_camera() {
    let mut app = xrds_test_app();
    let camera_entity = app.world_mut().spawn(Camera3d::default()).id();

    {
        let mut xrds = XrdsAPI::attach(&mut app);
        xrds.set_scene_environment(XrdsSceneEnvironment {
            exposure: Some(XrdsSceneExposureEnvironment { ev100: 4.5 }),
            ..Default::default()
        });
    }

    let exposure = app
        .world()
        .get::<Exposure>(camera_entity)
        .expect("runtime scene exposure should apply to existing cameras");
    assert_eq!(exposure.ev100, 4.5);
}

#[test]
fn set_scene_environment_applies_runtime_fog_policy_to_existing_camera() {
    let mut app = xrds_test_app();
    let camera_entity = app.world_mut().spawn(Camera3d::default()).id();

    {
        let mut xrds = XrdsAPI::attach(&mut app);
        xrds.set_scene_environment(XrdsSceneEnvironment {
            fog: Some(XrdsSceneFogEnvironment {
                color: [0.35, 0.48, 0.66, 1.0],
                start: 5.0,
                end: 40.0,
            }),
            ..Default::default()
        });
    }

    let fog = app
        .world()
        .get::<DistanceFog>(camera_entity)
        .expect("runtime scene fog should apply to existing cameras");
    assert_eq!(fog.color.to_srgba().to_f32_array(), [0.35, 0.48, 0.66, 1.0]);
    match &fog.falloff {
        FogFalloff::Linear { start, end } => {
            assert_eq!((*start, *end), (5.0, 40.0));
        }
        other => panic!("expected linear fog falloff, got {other:?}"),
    }
}

#[test]
fn export_scene_document_preserves_runtime_set_scene_exposure_environment() {
    let mut app = xrds_test_app();

    let exported = {
        let mut xrds = XrdsAPI::attach(&mut app);
        let environment = XrdsSceneEnvironment {
            exposure: Some(XrdsSceneExposureEnvironment { ev100: 5.75 }),
            ..Default::default()
        };
        xrds.set_scene_environment(environment.clone());
        assert_eq!(xrds.scene_environment(), Some(environment.clone()));
        xrds.export_scene_document()
            .expect("document should export with runtime scene exposure environment")
    };

    assert_eq!(
        exported.metadata.environment,
        Some(XrdsSceneEnvironment {
            exposure: Some(XrdsSceneExposureEnvironment { ev100: 5.75 }),
            ..Default::default()
        })
    );
}

#[test]
fn export_scene_document_preserves_runtime_set_scene_fog_environment() {
    let mut app = xrds_test_app();

    let exported = {
        let mut xrds = XrdsAPI::attach(&mut app);
        let environment = XrdsSceneEnvironment {
            fog: Some(XrdsSceneFogEnvironment {
                color: [0.35, 0.48, 0.66, 1.0],
                start: 5.0,
                end: 40.0,
            }),
            ..Default::default()
        };
        xrds.set_scene_environment(environment.clone());
        assert_eq!(xrds.scene_environment(), Some(environment.clone()));
        xrds.export_scene_document()
            .expect("document should export with runtime scene fog environment")
    };

    assert_eq!(
        exported.metadata.environment,
        Some(XrdsSceneEnvironment {
            fog: Some(XrdsSceneFogEnvironment {
                color: [0.35, 0.48, 0.66, 1.0],
                start: 5.0,
                end: 40.0,
            }),
            ..Default::default()
        })
    );
}


#[test]
fn set_scene_environment_applies_runtime_skybox_policy_to_existing_camera() {
    let mut app = xrds_test_app();
    let camera_entity = app.world_mut().spawn(Camera3d::default()).id();

    {
        let mut xrds = XrdsAPI::attach(&mut app);
        xrds.merge_scene_assets(&[XrdsSceneAsset {
            id: "asset:skybox".to_string(),
            uri: "environment_maps/specular.ktx2".to_string(),
            kind: XrdsSceneAssetKind::EnvironmentMap,
        }])
        .set_scene_environment(XrdsSceneEnvironment {
            skybox: Some(XrdsSceneSkyboxEnvironment {
                texture_asset_id: "asset:skybox".to_string(),
                brightness: 777.0,
                yaw_deg: 0.0,
            }),
            ..Default::default()
        });
    }

    let skybox = app
        .world()
        .get::<Skybox>(camera_entity)
        .expect("runtime scene skybox should apply to existing cameras");
    assert_eq!(skybox.brightness, 777.0);
}


#[test]
fn export_scene_document_preserves_runtime_set_scene_skybox_environment() {
    let mut app = xrds_test_app();

    let exported = {
        let mut xrds = XrdsAPI::attach(&mut app);
        let environment = XrdsSceneEnvironment {
            skybox: Some(XrdsSceneSkyboxEnvironment {
                texture_asset_id: "asset:skybox".to_string(),
                brightness: 444.0,
                yaw_deg: 0.0,
            }),
            ..Default::default()
        };
        xrds.merge_scene_assets(&[XrdsSceneAsset {
            id: "asset:skybox".to_string(),
            uri: "environment_maps/specular.ktx2".to_string(),
            kind: XrdsSceneAssetKind::EnvironmentMap,
        }])
        .set_scene_environment(environment.clone());
        assert_eq!(xrds.scene_environment(), Some(environment.clone()));
        xrds.export_scene_document()
            .expect("document should export with runtime scene skybox environment")
    };

    assert_eq!(
        exported.metadata.environment,
        Some(XrdsSceneEnvironment {
            skybox: Some(XrdsSceneSkyboxEnvironment {
                texture_asset_id: "asset:skybox".to_string(),
                brightness: 444.0,
                yaw_deg: 0.0,
            }),
            ..Default::default()
        })
    );
}

#[test]
fn export_scene_document_preserves_runtime_asset_aliases_for_environment_textures() {
    let mut app = xrds_test_app();

    let exported = {
        let mut xrds = XrdsAPI::attach(&mut app);
        let environment = XrdsSceneEnvironment {
            ibl: Some(XrdsSceneIblEnvironment {
                diffuse_asset_id: "asset:ibl-diffuse".to_string(),
                specular_asset_id: "asset:ibl-specular".to_string(),
                intensity: 444.0,
            }),
            skybox: Some(XrdsSceneSkyboxEnvironment {
                texture_asset_id: "asset:skybox".to_string(),
                brightness: 333.0,
                yaw_deg: 0.0,
            }),
            ..Default::default()
        };
        xrds.merge_scene_assets(&[
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
        ])
        .set_scene_environment(environment.clone());
        xrds.export_scene_document()
            .expect("document should export with aliased runtime environment assets")
    };

    assert_eq!(
        exported.metadata.environment,
        Some(XrdsSceneEnvironment {
            ibl: Some(XrdsSceneIblEnvironment {
                diffuse_asset_id: "asset:ibl-diffuse".to_string(),
                specular_asset_id: "asset:ibl-specular".to_string(),
                intensity: 444.0,
            }),
            skybox: Some(XrdsSceneSkyboxEnvironment {
                texture_asset_id: "asset:skybox".to_string(),
                brightness: 333.0,
                yaw_deg: 0.0,
            }),
            ..Default::default()
        })
    );
    assert!(exported.assets.iter().any(|asset| {
        asset.id == "asset:ibl-specular"
            && asset.kind == XrdsSceneAssetKind::EnvironmentMap
            && asset.uri == "environment_maps/specular.ktx2"
    }));
    assert!(exported.assets.iter().any(|asset| {
        asset.id == "asset:skybox"
            && asset.kind == XrdsSceneAssetKind::EnvironmentMap
            && asset.uri == "environment_maps/specular.ktx2"
    }));
}


