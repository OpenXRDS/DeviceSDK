use super::*;

#[test]
fn built_in_geometry_commit_helpers_update_runtime_and_exported_document() {
    let mut app = xrds_test_app();

    let (cube_id, cylinder_id, capsule_id, sphere_id, plane_id, tetrahedron_id) = {
        let mut xrds = XrdsAPI::attach(&mut app);
        let cube = xrds.spawn(&XrdsCube::new().with_name("Cube"));
        let cylinder = xrds.spawn(&XrdsCylinder::new().with_name("Cylinder"));
        let capsule = xrds.spawn(&XrdsCapsule::new().with_name("Capsule"));
        let sphere = xrds.spawn(&XrdsSphere::new().with_name("Sphere"));
        let plane = xrds.spawn(&XrdsPlane3D::new().with_name("Plane"));
        let tetrahedron = xrds.spawn(&XrdsTetrahedron::new().with_name("Tetrahedron"));

        xrds.set_cube_geometry(
            &cube,
            CubeGeometryParams {
                size: [2.0, 3.0, 4.0],
            },
        )
        .set_cylinder_geometry(
            &cylinder,
            CylinderGeometryParams {
                radius: 0.75,
                height: 5.0,
            },
        )
        .set_capsule_geometry(
            &capsule,
            CapsuleGeometryParams {
                radius: 0.6,
                length: 2.0,
            },
        )
        .set_sphere_geometry(&sphere, SphereGeometryParams { radius: 1.25 })
        .set_plane_geometry(&plane, Plane3DGeometryParams { size: [6.0, 8.0] })
        .set_tetrahedron_geometry(
            &tetrahedron,
            TetrahedronGeometryParams {
                vertices: [
                    [0.0, 0.0, 0.0],
                    [2.0, 0.0, 0.0],
                    [0.0, 3.0, 0.0],
                    [0.0, 0.0, 4.0],
                ],
            },
        );

        (
            xrds.id_of(&cube).expect("cube should have an id"),
            xrds.id_of(&cylinder).expect("cylinder should have an id"),
            xrds.id_of(&capsule).expect("capsule should have an id"),
            xrds.id_of(&sphere).expect("sphere should have an id"),
            xrds.id_of(&plane).expect("plane should have an id"),
            xrds.id_of(&tetrahedron)
                .expect("tetrahedron should have an id"),
        )
    };

    app.update();

    let exported = {
        let xrds = XrdsAPI::attach(&mut app);
        xrds.export_scene_document()
            .expect("scene document export should succeed")
    };

    let XrdsSceneNodePayload::Cube(cube) = &exported
        .node(XrdsSceneNodeId(cube_id.0))
        .expect("cube node should be exported")
        .payload
    else {
        panic!("expected cube payload");
    };
    assert_eq!(cube.size, [2.0, 3.0, 4.0]);

    let XrdsSceneNodePayload::Cylinder(cylinder) = &exported
        .node(XrdsSceneNodeId(cylinder_id.0))
        .expect("cylinder node should be exported")
        .payload
    else {
        panic!("expected cylinder payload");
    };
    assert_eq!(cylinder.radius, 0.75);
    assert_eq!(cylinder.height, 5.0);

    let XrdsSceneNodePayload::Capsule(capsule) = &exported
        .node(XrdsSceneNodeId(capsule_id.0))
        .expect("capsule node should be exported")
        .payload
    else {
        panic!("expected capsule payload");
    };
    assert_eq!(capsule.radius, 0.6);
    assert_eq!(capsule.length, 2.0);

    let XrdsSceneNodePayload::Sphere(sphere) = &exported
        .node(XrdsSceneNodeId(sphere_id.0))
        .expect("sphere node should be exported")
        .payload
    else {
        panic!("expected sphere payload");
    };
    assert_eq!(sphere.radius, 1.25);

    let XrdsSceneNodePayload::Plane3D(plane) = &exported
        .node(XrdsSceneNodeId(plane_id.0))
        .expect("plane node should be exported")
        .payload
    else {
        panic!("expected plane payload");
    };
    assert_eq!(plane.size, [6.0, 8.0]);

    let XrdsSceneNodePayload::Tetrahedron(tetrahedron) = &exported
        .node(XrdsSceneNodeId(tetrahedron_id.0))
        .expect("tetrahedron node should be exported")
        .payload
    else {
        panic!("expected tetrahedron payload");
    };
    assert_eq!(
        tetrahedron.vertices,
        [
            [0.0, 0.0, 0.0],
            [2.0, 0.0, 0.0],
            [0.0, 3.0, 0.0],
            [0.0, 0.0, 4.0],
        ]
    );
}


#[test]
fn built_in_light_commit_helpers_update_runtime_and_exported_document() {
    let mut app = xrds_test_app();

    let (point_id, directional_id, spot_id, ambient_id) = {
        let mut xrds = XrdsAPI::attach(&mut app);
        let point = xrds.spawn(&XrdsPointLight::new().with_name("Point"));
        let directional = xrds.spawn(&XrdsDirectionalLight::new().with_name("Directional"));
        let spot = xrds.spawn(&XrdsSpotLight::new().with_name("Spot"));
        let ambient = xrds.spawn(&XrdsAmbientLight::new().with_name("Ambient"));

        xrds.set_point_light_params(
            &point,
            PointLightParams {
                color: XrdsColor::srgb(1.0, 0.25, 0.1),
                intensity: 42_000.0,
                range: 18.0,
                radius: 0.4,
                shadows: true,
            },
        )
        .set_directional_light_params(
            &directional,
            DirectionalLightParams {
                color: XrdsColor::srgb(0.5, 0.6, 1.0),
                illuminance: 12_345.0,
                shadows: true,
            },
        )
        .set_spot_light_params(
            &spot,
            SpotLightParams {
                color: XrdsColor::srgb(0.9, 0.8, 0.5),
                intensity: 7_500.0,
                range: 14.0,
                inner_angle: 0.15,
                outer_angle: 0.6,
                shadows: true,
            },
        )
        .set_ambient_light_params(
            &ambient,
            AmbientLightParams {
                color: XrdsColor::srgb(0.2, 0.3, 0.4),
                brightness: 2.5,
                affects_baked_lighting: true,
            },
        );

        (
            xrds.id_of(&point).expect("point light should have an id"),
            xrds.id_of(&directional)
                .expect("directional light should have an id"),
            xrds.id_of(&spot).expect("spot light should have an id"),
            xrds.id_of(&ambient)
                .expect("ambient light should have an id"),
        )
    };

    app.update();

    let xrds = XrdsAPI::attach(&mut app);
    let point_handle = xrds
        .handle_of::<XrdsPointLight>(point_id)
        .expect("point light handle should resolve");
    let point_runtime = xrds
        .get_component::<PointLight, _>(&point_handle)
        .expect("point light component should exist");
    assert_eq!(point_runtime.intensity, 42_000.0);
    assert_eq!(point_runtime.range, 18.0);
    assert_eq!(point_runtime.radius, 0.4);
    assert!(point_runtime.shadows_enabled);

    let directional_handle = xrds
        .handle_of::<XrdsDirectionalLight>(directional_id)
        .expect("directional light handle should resolve");
    let directional_runtime = xrds
        .get_component::<DirectionalLight, _>(&directional_handle)
        .expect("directional light component should exist");
    assert_eq!(directional_runtime.illuminance, 12_345.0);
    assert!(directional_runtime.shadows_enabled);

    let spot_handle = xrds
        .handle_of::<XrdsSpotLight>(spot_id)
        .expect("spot light handle should resolve");
    let spot_runtime = xrds
        .get_component::<SpotLight, _>(&spot_handle)
        .expect("spot light component should exist");
    assert_eq!(spot_runtime.intensity, 7_500.0);
    assert_eq!(spot_runtime.range, 14.0);
    assert_eq!(spot_runtime.inner_angle, 0.15);
    assert_eq!(spot_runtime.outer_angle, 0.6);
    assert!(spot_runtime.shadows_enabled);

    let ambient_runtime = xrds
        .app
        .world()
        .get_resource::<AmbientLight>()
        .expect("ambient light resource should exist");
    assert_eq!(ambient_runtime.brightness, 2.5);
    assert!(ambient_runtime.affects_lightmapped_meshes); // Bevy field name

    let exported = xrds
        .export_scene_document()
        .expect("scene document export should succeed");

    let XrdsSceneNodePayload::PointLight(point) = &exported
        .node(XrdsSceneNodeId(point_id.0))
        .expect("point node should be exported")
        .payload
    else {
        panic!("expected point light payload");
    };
    assert_eq!(point.intensity, 42_000.0);
    assert_eq!(point.range, 18.0);
    assert_eq!(point.radius, 0.4);
    assert!(point.shadows);

    let XrdsSceneNodePayload::DirectionalLight(directional) = &exported
        .node(XrdsSceneNodeId(directional_id.0))
        .expect("directional node should be exported")
        .payload
    else {
        panic!("expected directional light payload");
    };
    assert_eq!(directional.illuminance, 12_345.0);
    assert!(directional.shadows);

    let XrdsSceneNodePayload::SpotLight(spot) = &exported
        .node(XrdsSceneNodeId(spot_id.0))
        .expect("spot node should be exported")
        .payload
    else {
        panic!("expected spot light payload");
    };
    assert_eq!(spot.intensity, 7_500.0);
    assert_eq!(spot.range, 14.0);
    assert_eq!(spot.inner_angle, 0.15);
    assert_eq!(spot.outer_angle, 0.6);
    assert!(spot.shadows);

    let XrdsSceneNodePayload::AmbientLight(ambient) = &exported
        .node(XrdsSceneNodeId(ambient_id.0))
        .expect("ambient node should be exported")
        .payload
    else {
        panic!("expected ambient light payload");
    };
    assert_eq!(ambient.brightness, 2.5);
    assert!(ambient.affects_baked_lighting);
}

#[test]
fn camera_projection_and_look_at_are_readable_after_spawn_and_update() {
    let mut app = xrds_test_app();

    let camera_id = {
        let mut xrds = XrdsAPI::attach(&mut app);
        let camera = xrds.spawn(&XrdsCamera::new().with_name("Cam"));
        xrds.set_camera_perspective(
            &camera,
            PerspectiveCameraParams {
                fov_deg: 75.0,
                near: 0.05,
                far: Some(500.0),
                order: 0,
            },
        )
        .set_camera_look_at(&camera, Some([1.0, 2.0, 3.0]));
        xrds.id_of(&camera).expect("camera should have an id")
    };

    app.update();

    let xrds = XrdsAPI::attach(&mut app);
    let handle = xrds
        .handle_of::<XrdsCamera>(camera_id)
        .expect("camera should be indexed");

    let projection = xrds
        .camera_projection(&handle)
        .expect("camera_projection should return Some for a live camera");
    let CameraProjectionParams::Perspective(persp) = projection else {
        panic!("expected perspective projection");
    };
    assert_eq!(persp.fov_deg, 75.0);
    assert_eq!(persp.near, 0.05);
    assert_eq!(persp.far, Some(500.0));

    let look_at = xrds
        .camera_look_at(&handle)
        .expect("camera_look_at outer None means entity missing");
    assert_eq!(look_at, Some([1.0, 2.0, 3.0]));
}

#[test]
fn camera_look_at_returns_some_none_when_not_set() {
    let mut app = xrds_test_app();

    let camera_id = {
        let mut xrds = XrdsAPI::attach(&mut app);
        let camera = xrds.spawn(&XrdsCamera::new());
        xrds.id_of(&camera).expect("camera should have an id")
    };

    app.update();

    let xrds = XrdsAPI::attach(&mut app);
    let handle = xrds
        .handle_of::<XrdsCamera>(camera_id)
        .expect("camera should be indexed");

    assert_eq!(
        xrds.camera_look_at(&handle),
        Some(None),
        "camera with no look-at should return Some(None)"
    );
}

#[test]
fn light_params_are_readable_after_spawn_and_update() {
    let mut app = xrds_test_app();

    let (point_id, dir_id, spot_id, ambient_id) = {
        let mut xrds = XrdsAPI::attach(&mut app);
        let point = xrds.spawn(&XrdsPointLight::new());
        let dir = xrds.spawn(&XrdsDirectionalLight::new());
        let spot = xrds.spawn(&XrdsSpotLight::new());
        let ambient = xrds.spawn(&XrdsAmbientLight::new());

        xrds.set_point_light_params(
            &point,
            PointLightParams {
                color: XrdsColor::srgb(1.0, 0.5, 0.0),
                intensity: 3_000.0,
                range: 8.0,
                radius: 0.1,
                shadows: true,
            },
        )
        .set_directional_light_params(
            &dir,
            DirectionalLightParams {
                color: XrdsColor::srgb(0.9, 0.9, 1.0),
                illuminance: 5_000.0,
                shadows: false,
            },
        )
        .set_spot_light_params(
            &spot,
            SpotLightParams {
                color: XrdsColor::srgb(0.2, 0.8, 0.4),
                intensity: 1_200.0,
                range: 12.0,
                inner_angle: 0.1,
                outer_angle: 0.4,
                shadows: true,
            },
        )
        .set_ambient_light_params(
            &ambient,
            AmbientLightParams {
                color: XrdsColor::srgb(0.3, 0.3, 0.3),
                brightness: 0.8,
                affects_baked_lighting: true,
            },
        );

        (
            xrds.id_of(&point).unwrap(),
            xrds.id_of(&dir).unwrap(),
            xrds.id_of(&spot).unwrap(),
            xrds.id_of(&ambient).unwrap(),
        )
    };

    app.update();

    let xrds = XrdsAPI::attach(&mut app);
    let point_h = xrds.handle_of::<XrdsPointLight>(point_id).unwrap();
    let dir_h = xrds.handle_of::<XrdsDirectionalLight>(dir_id).unwrap();
    let spot_h = xrds.handle_of::<XrdsSpotLight>(spot_id).unwrap();
    let ambient_h = xrds.handle_of::<XrdsAmbientLight>(ambient_id).unwrap();

    let point = xrds.point_light_params(&point_h).expect("point light params should be readable");
    assert_eq!(point.intensity, 3_000.0);
    assert_eq!(point.range, 8.0);
    assert_eq!(point.radius, 0.1);
    assert!(point.shadows);

    let dir = xrds.directional_light_params(&dir_h).expect("directional light params should be readable");
    assert_eq!(dir.illuminance, 5_000.0);
    assert!(!dir.shadows);

    let spot = xrds.spot_light_params(&spot_h).expect("spot light params should be readable");
    assert_eq!(spot.intensity, 1_200.0);
    assert_eq!(spot.inner_angle, 0.1);
    assert_eq!(spot.outer_angle, 0.4);
    assert!(spot.shadows);

    let ambient = xrds.ambient_light_params(&ambient_h).expect("ambient light params should be readable");
    assert_eq!(ambient.brightness, 0.8);
    assert!(ambient.affects_baked_lighting);
}

#[test]
fn gltf_source_is_readable_after_import() {
    let fixture_uri = asset_fixture_path(VALID_GLTF_PATH);

    let document = XrdsSceneDocument {
        assets: vec![XrdsSceneAsset {
            id: "asset:lamp".to_string(),
            uri: fixture_uri.clone(),
            kind: XrdsSceneAssetKind::Gltf,
        }],
        nodes: vec![XrdsSceneNode {
            id: XrdsSceneNodeId(900),
            parent_id: None,
            name: "Lamp".to_string(),
            enabled: true,
            visible: true,
            transform: XrdsSceneTransform::default(),
            payload: XrdsSceneNodePayload::GltfAsset(XrdsSceneGltfAsset {
                asset_id: Some("asset:lamp".to_string()),
                asset_uri: fixture_uri.clone(),
                scene_index: 0,
                export_policy: XrdsGltfAssetExportPolicy::KeepExternalReference,
            }),
            grabbable: false,
            editor: XrdsEditorMetadata::default(),
            triggers: Vec::new(),
            watchers: Vec::new(),
        }],
        ..Default::default()
    };

    let mut app = xrds_test_app();

    let gltf_id = {
        let mut xrds = XrdsAPI::attach(&mut app);
        let ids = xrds
            .import_scene_document(&document)
            .expect("document import should succeed");
        ids[0]
    };

    app.update();

    let xrds = XrdsAPI::attach(&mut app);
    let handle = xrds
        .handle_of::<XrdsGltfAsset>(gltf_id)
        .expect("gltf should be indexed");
    let source = xrds
        .gltf_source(&handle)
        .expect("gltf_source should return Some after import");
    assert_eq!(source.gltf_asset_path, fixture_uri);
    assert_eq!(source.scene_index, 0);
}

#[test]
fn material_texture_slot_read_write_and_clear_work_on_mesh_entity() {
    let mut app = xrds_test_app();

    let cube_id = {
        let mut xrds = XrdsAPI::attach(&mut app);
        let cube = xrds.spawn(&XrdsCube::new().with_name("TexturedCube"));
        xrds.id_of(&cube).expect("cube should have an id")
    };

    app.update();

    // Initially all texture slots are empty.
    {
        let xrds = XrdsAPI::attach(&mut app);
        let cube = xrds.handle_of::<XrdsCube>(cube_id).unwrap();
        let slots = xrds
            .material_textures(&cube)
            .expect("material_textures should return Some for a spawned cube");
        assert!(slots.is_empty(), "all texture slots should start empty");
    }

    // Set the base-color slot.
    {
        let mut xrds = XrdsAPI::attach(&mut app);
        let cube = xrds.handle_of::<XrdsCube>(cube_id).unwrap();
        xrds.set_material_texture_slot(
            &cube,
            XrdsMaterialTextureSlotKind::BaseColor,
            Some(XrdsMaterialTextureRef {
                texture_asset_id: "asset:texture-floor".to_string(),
                uv: XrdsMaterialTextureUvParams::default(),
                sampler: XrdsMaterialTextureSamplerParams::default(),
            }),
        );
    }

    app.update();

    // Read it back — slot should be set, others still empty.
    {
        let xrds = XrdsAPI::attach(&mut app);
        let cube = xrds.handle_of::<XrdsCube>(cube_id).unwrap();
        let slots = xrds.material_textures(&cube).unwrap();
        let base = slots
            .base_color
            .as_ref()
            .expect("base-color slot should now be set");
        assert_eq!(base.texture_asset_id, "asset:texture-floor");
        assert!(slots.normal.is_none());
        assert!(slots.metallic_roughness.is_none());
    }

    // Clear the slot.
    {
        let mut xrds = XrdsAPI::attach(&mut app);
        let cube = xrds.handle_of::<XrdsCube>(cube_id).unwrap();
        xrds.set_material_texture_slot(
            &cube,
            XrdsMaterialTextureSlotKind::BaseColor,
            None,
        );
    }

    app.update();

    {
        let xrds = XrdsAPI::attach(&mut app);
        let cube = xrds.handle_of::<XrdsCube>(cube_id).unwrap();
        let slots = xrds.material_textures(&cube).unwrap();
        assert!(
            slots.is_empty(),
            "all texture slots should be empty after clearing"
        );
    }
}

#[test]
fn set_material_textures_replaces_all_slots_at_once() {
    let mut app = xrds_test_app();

    let sphere_id = {
        let mut xrds = XrdsAPI::attach(&mut app);
        let sphere = xrds.spawn(&XrdsSphere::new().with_name("TexturedSphere"));
        xrds.id_of(&sphere).unwrap()
    };

    app.update();

    let full_slots = XrdsMaterialTextureSlots {
        base_color: Some(XrdsMaterialTextureRef {
            texture_asset_id: "asset:albedo".to_string(),
            uv: XrdsMaterialTextureUvParams::default(),
            sampler: XrdsMaterialTextureSamplerParams::default(),
        }),
        normal: Some(XrdsMaterialTextureRef {
            texture_asset_id: "asset:normal".to_string(),
            uv: XrdsMaterialTextureUvParams::default(),
            sampler: XrdsMaterialTextureSamplerParams::default(),
        }),
        ..Default::default()
    };

    {
        let mut xrds = XrdsAPI::attach(&mut app);
        let sphere = xrds.handle_of::<XrdsSphere>(sphere_id).unwrap();
        xrds.set_material_textures(&sphere, full_slots.clone());
    }

    app.update();

    {
        let xrds = XrdsAPI::attach(&mut app);
        let sphere = xrds.handle_of::<XrdsSphere>(sphere_id).unwrap();
        let slots = xrds.material_textures(&sphere).unwrap();
        assert_eq!(
            slots.base_color.as_ref().map(|t| t.texture_asset_id.as_str()),
            Some("asset:albedo")
        );
        assert_eq!(
            slots.normal.as_ref().map(|t| t.texture_asset_id.as_str()),
            Some("asset:normal")
        );
        assert!(slots.metallic_roughness.is_none());
        assert!(slots.emissive.is_none());
    }
}

