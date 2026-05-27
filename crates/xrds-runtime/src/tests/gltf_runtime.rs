use super::*;

#[test]
fn gltf_load_status_transitions_from_pending_to_loaded_for_valid_scene() {
    let mut app = test_app();
    let handle = spawn_scene_root_entity(&mut app, &format!("{VALID_GLTF_PATH}#Scene0"));

    let initial_status = gltf_load_status_in_world(app.world(), &handle);
    assert!(matches!(
        initial_status,
        Some(XrdsGltfLoadStatus::NotLoaded | XrdsGltfLoadStatus::Loading)
    ));

    // 5000 × 2ms = 10s — generous budget so parallel test runs don't race the asset loader.
    let final_status = drive_until_terminal_status(&mut app, &handle, 5000);
    assert_eq!(final_status, Some(XrdsGltfLoadStatus::Loaded));
}


#[test]
fn gltf_load_status_reports_root_scene_load_failures() {
    let mut app = test_app();
    let handle = spawn_scene_root_entity(&mut app, MISSING_ROOT_SCENE_PATH);

    let final_status = drive_until_terminal_status(&mut app, &handle, 300);
    match final_status {
        Some(XrdsGltfLoadStatus::Failed(message)) => {
            assert!(!message.trim().is_empty());
        }
        other => panic!("expected failed root-scene status, got {other:?}"),
    }
}


#[test]
fn gltf_load_status_reports_recursive_dependency_failures() {
    let mut app = test_app();
    let handle =
        spawn_scene_root_entity(&mut app, &format!("{BROKEN_DEPENDENCY_GLTF_PATH}#Scene0"));

    let final_status = drive_until_terminal_status(&mut app, &handle, 1000);
    match final_status {
        Some(XrdsGltfLoadStatus::Failed(message)) => {
            assert!(message.contains("missing.png") || !message.trim().is_empty());
        }
        other => panic!("expected failed dependency status, got {other:?}"),
    }

    let scene_id = app
        .world()
        .get::<SceneRoot>(handle.entity())
        .expect("test entity should keep SceneRoot")
        .id();
    let asset_server = app.world().resource::<AssetServer>();

    assert!(matches!(
        asset_server.load_state(scene_id),
        bevy::asset::LoadState::Loaded
    ));
    assert!(matches!(
        asset_server.recursive_dependency_load_state(scene_id),
        bevy::asset::RecursiveDependencyLoadState::Failed(_)
    ));
}


#[test]
fn xrds_gltf_animation_api_applies_index_playback_without_metadata_and_reports_morph_targets() {
    let mut app = xrds_test_app();

    let handle = spawn_stored_gltf_entity(&mut app, MORPH_STRESS_TEST_PATH);

    {
        let mut xrds = XrdsAPI::attach(&mut app);
        xrds.play_gltf_animation(
            &handle,
            XrdsGltfAnimationSelector::Index(2),
            XrdsGltfAnimationPlaybackOptions::default(),
        )
        .expect(
            "index-based glTF animation playback should apply when scene players already exist",
        );
    }

    assert!(!app
        .world()
        .resource::<PendingGltfAnimationRequests>()
        .requests
        .contains_key(&handle.entity()));

    let animation_state = {
        let xrds = XrdsAPI::attach(&mut app);
        xrds.gltf_animation_state(&handle)
            .expect("animation state queries should succeed for glTF handles")
            .expect(
                "index-based playback should populate XRDS glTF animation state without metadata",
            )
    };

    assert_eq!(animation_state.animation.index, 2);
    assert!(animation_state.animation.name.is_none());
    assert!(animation_state.playing);
    assert!(!animation_state.paused);

    seed_synthetic_gltf_asset(&mut app, MORPH_STRESS_TEST_PATH);

    let (animations, morph_targets) = {
        let xrds = XrdsAPI::attach(&mut app);
        (
            xrds.gltf_animations(&handle)
                .expect("synthetic glTF metadata should expose XRDS animation info"),
            xrds.gltf_morph_targets(&handle)
                .expect("synthetic realized mesh should expose XRDS morph target info"),
        )
    };

    assert!(animations.len() > 2);
    assert!(animations.iter().any(|animation| animation.index == 2));

    assert_eq!(animation_state.animation.index, 2);
    assert!(animation_state.playing);
    assert!(!animation_state.paused);
    assert!(matches!(
        animation_state.repeat,
        XrdsAnimationRepeatMode::Loop
    ));

    assert!(!morph_targets.is_empty());
    assert!(morph_targets.iter().any(|set| !set.targets.is_empty()));
}


#[test]
fn xrds_gltf_animation_api_animates_real_morph_asset_weights() {
    let mut app = xrds_real_asset_test_app();
    let handle = spawn_real_stored_gltf_entity(&mut app, MORPH_STRESS_TEST_EXAMPLE_PATH);

    {
        let mut xrds = XrdsAPI::attach(&mut app);
        xrds.play_gltf_animation(
            &handle,
            XrdsGltfAnimationSelector::Index(2),
            XrdsGltfAnimationPlaybackOptions::default(),
        )
        .expect("XRDS should accept a queued morph animation request before the real asset loads");
    }

    let mut first_snapshot: Option<Vec<f32>> = None;
    let mut animation_state = None;
    let mut weights_changed = false;

    for _ in 0..1200 {
        std::thread::sleep(Duration::from_millis(2));
        app.update();

        let xrds = XrdsAPI::attach(&mut app);
        let status = xrds.gltf_load_status(&handle);
        if status != Some(XrdsGltfLoadStatus::Loaded) {
            continue;
        }

        animation_state = xrds
            .gltf_animation_state(&handle)
            .expect("animation state queries should succeed for the real morph asset");

        let Ok(weight_sets) = xrds.gltf_morph_target_weights(&handle) else {
            continue;
        };
        if weight_sets.is_empty() {
            continue;
        }

        let flattened = weight_sets
            .iter()
            .flat_map(|set| set.weights.iter().map(|weight| weight.weight))
            .collect::<Vec<_>>();
        if flattened.is_empty() {
            continue;
        }

        match &first_snapshot {
            None => first_snapshot = Some(flattened),
            Some(previous) => {
                if previous.len() == flattened.len()
                    && previous
                        .iter()
                        .zip(&flattened)
                        .any(|(before, after)| (before - after).abs() > 1e-4)
                {
                    weights_changed = true;
                    break;
                }
            }
        }
    }

    assert_eq!(
        drive_until_terminal_status(&mut app, &handle, 1),
        Some(XrdsGltfLoadStatus::Loaded)
    );

    let animation_state = animation_state
        .expect("real morph asset should produce active XRDS animation state once loaded");
    assert_eq!(animation_state.animation.index, 2);
    assert!(animation_state.playing);
    assert!(!animation_state.paused);
    assert!(
        weights_changed,
        "real morph asset weights should change over time through the XRDS SDK playback path"
    );
}


#[test]
fn scene_ready_observer_applies_queued_gltf_animation_requests() {
    let mut app = xrds_scene_ready_observer_test_app();

    let scene_handle = {
        let mut scene_world = World::new();
        scene_world.spawn((Name::new("ObserverPlayer"), AnimationPlayer::default()));
        app.world_mut()
            .resource_mut::<Assets<Scene>>()
            .add(Scene::new(scene_world))
    };

    let root = app
        .world_mut()
        .spawn((
            Name::new("ObserverRoot"),
            SceneRoot(scene_handle),
            XrdsStored(XrdsGltfAsset::new(OBSERVER_TEST_GLTF_PATH).with_name("ObserverRoot")),
        ))
        .id();

    app.world_mut()
        .resource_mut::<PendingGltfAnimationRequests>()
        .requests
        .insert(
            root,
            PendingGltfAnimationRequest {
                selector: XrdsGltfAnimationSelector::Index(2),
                options: XrdsGltfAnimationPlaybackOptions::default(),
            },
        );

    for _ in 0..3 {
        app.update();
    }

    assert!(!app
        .world()
        .resource::<PendingGltfAnimationRequests>()
        .requests
        .contains_key(&root));

    let animation_state = app
        .world()
        .resource::<ActiveGltfAnimationStates>()
        .states
        .get(&root)
        .cloned()
        .expect("scene-ready observer should populate XRDS glTF animation state");

    assert_eq!(animation_state.animation.index, 2);
    assert!(animation_state.playing);
    assert!(!animation_state.paused);

    let player_entity = app
        .world()
        .get::<Children>(root)
        .expect("scene root should receive spawned scene children")
        .iter()
        .find(|child| app.world().get::<AnimationPlayer>(*child).is_some())
        .expect("spawned scene should contain an animation player descendant");

    assert!(app
        .world()
        .get::<AnimationGraphHandle>(player_entity)
        .is_some());
}


#[test]
fn xrds_gltf_morph_weight_api_reads_and_sets_weights() {
    let mut app = xrds_test_app();
    let handle = spawn_stored_gltf_entity(&mut app, MORPH_STRESS_TEST_PATH);
    seed_synthetic_gltf_asset(&mut app, MORPH_STRESS_TEST_PATH);

    let initial = {
        let xrds = XrdsAPI::attach(&mut app);
        xrds.gltf_morph_target_weights(&handle)
            .expect("synthetic realized mesh should expose XRDS morph target weights")
    };

    let first_set = initial
        .first()
        .expect("synthetic test mesh should expose at least one morph target set")
        .clone();
    assert_eq!(first_set.weights.len(), 2);
    assert!(first_set.weights.iter().all(|weight| weight.weight == 0.0));

    {
        let mut xrds = XrdsAPI::attach(&mut app);
        xrds.set_gltf_morph_target_weight(
            &handle,
            &first_set.node,
            first_set.mesh_name.as_deref(),
            XrdsGltfMorphTargetSelector::Name("Smile".to_string()),
            0.75,
        )
        .expect("setting XRDS glTF morph target weight should succeed");
    }

    let updated = {
        let xrds = XrdsAPI::attach(&mut app);
        xrds.gltf_morph_target_weights(&handle)
            .expect("updated XRDS morph target weights should be queryable")
    };

    let updated_set = updated
        .iter()
        .find(|set| set.node == first_set.node && set.mesh_name == first_set.mesh_name)
        .expect("updated weight set should still be addressable by node locator");
    let smile_weight = updated_set
        .weights
        .iter()
        .find(|weight| weight.target.name.as_deref() == Some("Smile"))
        .expect("Smile morph target should still exist");
    let blink_weight = updated_set
        .weights
        .iter()
        .find(|weight| weight.target.name.as_deref() == Some("Blink"))
        .expect("Blink morph target should still exist");

    assert_eq!(smile_weight.weight, 0.75);
    assert_eq!(blink_weight.weight, 0.0);
}


#[test]
fn export_scene_document_captures_runtime_gltf_morph_weights_in_authoring() {
    let fixture_uri = asset_fixture_path(MORPH_STRESS_TEST_PATH);
    let document = XrdsSceneDocument {
        assets: vec![XrdsSceneAsset {
            id: "asset:morph".to_string(),
            uri: fixture_uri.clone(),
            kind: XrdsSceneAssetKind::Gltf,
        }],
        nodes: vec![XrdsSceneNode {
            id: XrdsSceneNodeId(630),
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
            630,
            XrdsSceneGltfNodeAuthoring {
                default_playback: Some(XrdsSceneGltfPlayback {
                    selector: XrdsSceneGltfAnimationSelector::Name("Run".to_string()),
                    repeat: XrdsSceneAnimationRepeatMode::Loop,
                    speed: 1.0,
                    start_paused: false,
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
            .expect("scene import should succeed before runtime morph edits");
    }

    let handle = {
        let xrds = XrdsAPI::attach(&mut app);
        xrds.handle_of::<XrdsGltfAsset>(XrdsId(630))
            .expect("imported gltf node should be indexed by id")
    };

    attach_synthetic_morph_mesh_to_root(&mut app, handle.entity(), "MorphMeshNode");
    seed_synthetic_gltf_asset(&mut app, &fixture_uri);

    let first_set = {
        let xrds = XrdsAPI::attach(&mut app);
        xrds.gltf_morph_target_weights(&handle)
            .expect("runtime morph weights should be queryable before export")
            .into_iter()
            .next()
            .expect("synthetic morph mesh should exist")
    };

    {
        let mut xrds = XrdsAPI::attach(&mut app);
        xrds.set_gltf_morph_target_weight(
            &handle,
            &first_set.node,
            first_set.mesh_name.as_deref(),
            XrdsGltfMorphTargetSelector::Name("Blink".to_string()),
            0.15,
        )
        .expect("Blink weight update should succeed");
        xrds.set_gltf_morph_target_weight(
            &handle,
            &first_set.node,
            first_set.mesh_name.as_deref(),
            XrdsGltfMorphTargetSelector::Name("Smile".to_string()),
            0.9,
        )
        .expect("Smile weight update should succeed");
    }

    let exported = {
        let xrds = XrdsAPI::attach(&mut app);
        xrds.export_scene_document()
            .expect("scene export should capture current runtime morph weights")
    };

    let exported_authoring = exported
        .gltf_node_authoring
        .get(&630)
        .expect("exported scene should keep gltf authoring for the runtime-edited node");
    assert!(exported_authoring.default_playback.is_some());
    assert_eq!(exported_authoring.morph_target_overrides.len(), 1);

    let override_entry = &exported_authoring.morph_target_overrides[0];
    assert_eq!(
        override_entry.node.node_name.as_deref(),
        Some("MorphMeshNode")
    );
    assert_eq!(override_entry.node.node_index_path, vec![0]);
    assert_eq!(override_entry.weights.len(), 2);

    let blink = override_entry
        .weights
        .iter()
        .find(|weight| {
            matches!(
                &weight.selector,
                XrdsSceneGltfMorphTargetSelector::Name(name) if name == "Blink"
            )
        })
        .expect("Blink override should be exported");
    let smile = override_entry
        .weights
        .iter()
        .find(|weight| {
            matches!(
                &weight.selector,
                XrdsSceneGltfMorphTargetSelector::Name(name) if name == "Smile"
            )
        })
        .expect("Smile override should be exported");

    assert_eq!(blink.weight, 0.15);
    assert_eq!(smile.weight, 0.9);
}


#[test]
fn export_scene_document_captures_runtime_gltf_playback_in_authoring() {
    let mut app = xrds_test_app();
    let fixture_uri = asset_fixture_path(MORPH_STRESS_TEST_PATH);
    let document = XrdsSceneDocument {
        assets: vec![XrdsSceneAsset {
            id: "asset:morph".to_string(),
            uri: fixture_uri.clone(),
            kind: XrdsSceneAssetKind::Gltf,
        }],
        nodes: vec![XrdsSceneNode {
            id: XrdsSceneNodeId(631),
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
        ..Default::default()
    };

    {
        let mut xrds = XrdsAPI::attach(&mut app);
        xrds.import_scene_document(&document)
            .expect("scene import should succeed before runtime playback edits");
    }

    let handle = {
        let xrds = XrdsAPI::attach(&mut app);
        xrds.handle_of::<XrdsGltfAsset>(XrdsId(631))
            .expect("imported gltf node should be indexed by id")
    };

    seed_synthetic_gltf_asset(&mut app, &fixture_uri);

    {
        let mut xrds = XrdsAPI::attach(&mut app);
        xrds.play_gltf_animation(
            &handle,
            XrdsGltfAnimationSelector::Name("MorphStressLoop".to_string()),
            XrdsGltfAnimationPlaybackOptions {
                repeat: XrdsAnimationRepeatMode::Once,
                speed: 1.4,
                start_paused: true,
            },
        )
        .expect("runtime gltf playback should succeed before export");
    }

    let exported = {
        let xrds = XrdsAPI::attach(&mut app);
        xrds.export_scene_document()
            .expect("scene export should capture current runtime playback state")
    };

    let authoring = exported
        .gltf_node_authoring
        .get(&631)
        .expect("exported scene should include gltf authoring for runtime playback");
    let playback = authoring
        .default_playback
        .as_ref()
        .expect("current runtime playback should export into default_playback");

    assert!(matches!(
        &playback.selector,
        XrdsSceneGltfAnimationSelector::Name(name) if name == "MorphStressLoop"
    ));
    assert!(matches!(
        playback.repeat,
        XrdsSceneAnimationRepeatMode::Once
    ));
    assert_eq!(playback.speed, 1.4);
    assert!(playback.start_paused);
}


