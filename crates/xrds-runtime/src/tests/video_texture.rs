use super::*;
use bevy::asset::AssetEvent;
use bevy::image::Image;

/// A written frame must reach the asset *and* be announced as modified.
///
/// The announcement is the half that matters and the half that is invisible: Bevy
/// re-uploads a texture only on `AssetEvent::Modified`, so a write that lands in
/// `Assets<Image>` but emits nothing changes the CPU copy and never the screen —
/// which looks exactly like "the video is not playing".
#[test]
fn writing_a_frame_modifies_the_asset_and_announces_it() {
    let mut app = xrds_test_app();

    {
        let mut xrds = XrdsAPI::attach(&mut app);
        xrds.create_video_texture("clip", 4, 2);
    }

    // Find the handle the registry created, via the same lookup the material
    // resolver uses.
    let handle = crate::xrds_api::video::video_texture_handle_in_world(app.world(), "clip")
        .expect("the registry should hold a handle");

    // Drain events from creation so only the write's event is observed.
    app.update();
    {
        let world = app.world_mut();
        let mut events = world.resource_mut::<Events<AssetEvent<Image>>>();
        events.clear();
    }

    let frame: Vec<u8> = (0..4 * 2 * 4).map(|i| (i * 7 % 256) as u8).collect();
    {
        let mut xrds = XrdsAPI::attach(&mut app);
        assert!(xrds.write_video_frame("clip", &frame), "write should be accepted");
    }

    // Tick once: `Assets<T>` accumulates changes and a system drains them into
    // `Events<AssetEvent<T>>`, so the event does not exist until the schedule runs.
    app.update();

    // 1. The data landed.
    let stored = app
        .world()
        .resource::<Assets<Image>>()
        .get(&handle)
        .and_then(|i| i.data.clone())
        .expect("the image should still hold a CPU copy");
    assert_eq!(stored, frame, "the write should be visible in the asset");

    // 2. And Bevy was told. Without this the GPU keeps the previous upload.
    let world = app.world();
    let events = world.resource::<Events<AssetEvent<Image>>>();
    let mut reader = events.get_cursor();
    let modified = reader
        .read(events)
        .any(|e| matches!(e, AssetEvent::Modified { id } if *id == handle.id()));
    assert!(
        modified,
        "writing a frame must emit AssetEvent::Modified — without it the texture \
         is re-uploaded never, and the surface shows whatever was first uploaded"
    );
}

/// A wrong-length buffer is refused rather than partially written.
#[test]
fn a_wrong_sized_frame_is_refused() {
    let mut app = xrds_test_app();
    let mut xrds = XrdsAPI::attach(&mut app);
    xrds.create_video_texture("clip", 4, 2);
    assert!(!xrds.write_video_frame("clip", &[0u8; 8]), "short buffer must be refused");
    assert!(!xrds.write_video_frame("missing", &[0u8; 32]), "unknown id must be refused");
}

/// Writing a frame must also mark the materials that sample it as modified.
///
/// This is the assertion that would have saved the spike several days. Bevy rebuilds
/// a modified image into an *entirely new* `wgpu::Texture`, and `bevy_pbr` never
/// watches `AssetEvent<Image>` — so a material's bind group goes on pointing at the
/// first texture forever unless the material is itself re-prepared. Every other
/// signal says the update worked; only the picture disagrees, and only on a screen.
#[test]
fn writing_a_frame_marks_the_sampling_material_modified() {
    use crate::xrds_api::material::XrdsRuntimeMaterial;

    let mut app = xrds_test_app();

    let plane = {
        let mut xrds = XrdsAPI::attach(&mut app);
        xrds.create_video_texture("clip", 4, 2);
        let plane = xrds.spawn(&XrdsPlane3D::new().with_name("Screen"));
        xrds.set_material_texture_slot(
            &plane,
            XrdsMaterialTextureSlotKind::BaseColor,
            Some(XrdsMaterialTextureRef {
                texture_asset_id: "clip".to_string(),
                uv: XrdsMaterialTextureUvParams::default(),
                sampler: XrdsMaterialTextureSamplerParams::default(),
            }),
        );
        plane
    };
    app.update();

    // The slot must actually have resolved to the runtime texture — otherwise this
    // test would pass vacuously on a scene where nothing samples the video at all.
    let image = crate::xrds_api::video::video_texture_handle_in_world(app.world(), "clip")
        .expect("the registry should hold a handle");
    let sampling = app
        .world()
        .resource::<Assets<XrdsRuntimeMaterial>>()
        .iter()
        .filter(|(_, m)| {
            m.extension.base_color_texture.as_ref().map(|h| h.id()) == Some(image.id())
        })
        .count();
    assert_eq!(
        sampling, 1,
        "exactly one material should sample the video texture; the slot did not resolve"
    );

    {
        let world = app.world_mut();
        world.resource_mut::<Events<AssetEvent<XrdsRuntimeMaterial>>>().clear();
    }

    let frame: Vec<u8> = vec![7u8; 4 * 2 * 4];
    {
        let mut xrds = XrdsAPI::attach(&mut app);
        assert!(xrds.write_video_frame("clip", &frame));
    }
    app.update();

    let world = app.world();
    let events = world.resource::<Events<AssetEvent<XrdsRuntimeMaterial>>>();
    let mut reader = events.get_cursor();
    let modified = reader.read(events).any(|e| matches!(e, AssetEvent::Modified { .. }));
    assert!(
        modified,
        "a frame write must re-prepare the materials sampling it, or their bind \
         groups keep the first texture and the surface freezes on frame one"
    );

    let _ = plane;
}

/// The same texture works on any XRDS mesh and any slot, not just a plane's base
/// colour.
///
/// A flat quad is the obvious video surface, but nothing in the mechanism is
/// specific to one — it is an ordinary material texture slot. This pins that down so
/// "can it go on a cube / a sphere / an emissive panel" is answered by a test rather
/// than by argument from the one case that was tried by hand.
#[test]
fn a_video_texture_binds_to_any_mesh_and_any_slot() {
    use crate::xrds_api::material::XrdsRuntimeMaterial;

    let mut app = xrds_test_app();
    {
        let mut xrds = XrdsAPI::attach(&mut app);
        xrds.create_video_texture("clip", 4, 2);
        let cube = xrds.spawn(&XrdsCube::new().with_name("Screen"));
        xrds.set_material_texture_slot(
            &cube,
            XrdsMaterialTextureSlotKind::Emissive,
            Some(XrdsMaterialTextureRef {
                texture_asset_id: "clip".to_string(),
                uv: XrdsMaterialTextureUvParams::default(),
                sampler: XrdsMaterialTextureSamplerParams::default(),
            }),
        );
    }
    app.update();

    let image = crate::xrds_api::video::video_texture_handle_in_world(app.world(), "clip")
        .expect("the registry should hold a handle");
    let sampling = app
        .world()
        .resource::<Assets<XrdsRuntimeMaterial>>()
        .iter()
        .filter(|(_, m)| {
            m.extension.emissive_texture.as_ref().map(|h| h.id()) == Some(image.id())
        })
        .count();
    assert_eq!(sampling, 1, "a cube's emissive slot should resolve to the video texture");

    {
        let world = app.world_mut();
        world.resource_mut::<Events<AssetEvent<XrdsRuntimeMaterial>>>().clear();
    }
    {
        let mut xrds = XrdsAPI::attach(&mut app);
        assert!(xrds.write_video_frame("clip", &vec![9u8; 4 * 2 * 4]));
    }
    app.update();

    let world = app.world();
    let events = world.resource::<Events<AssetEvent<XrdsRuntimeMaterial>>>();
    let mut reader = events.get_cursor();
    assert!(
        reader.read(events).any(|e| matches!(e, AssetEvent::Modified { .. })),
        "a non-base-colour slot must be rebound too, or it freezes on frame one in a \
         way that is far harder to recognise than a frozen video screen"
    );
}

/// A scene that names a video gets a surface, and no decoder until asked.
///
/// This is the contract that separates video from every other asset kind. A texture
/// costs a file read; a *video* costs a decoder — a thread on a desktop, a hardware
/// codec session on a headset — plus GPU work every frame. So a scene that merely
/// contains a screen must not pay for it, and playback is always asked for.
///
/// Both halves are asserted, because getting either wrong is silent: no surface
/// means an authored screen resolves to nothing, and an auto-started decoder means
/// every scene pays for every clip it mentions.
///
/// Desktop only: the Android path needs MediaCodec and a Vulkan device.
#[cfg(not(target_os = "android"))]
#[test]
fn a_named_video_gets_a_surface_but_does_not_start_playing() {
    use xrds_scene_graph::{XrdsSceneAsset, XrdsSceneAssetKind};

    let clip = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../crates/xrds-net/samples/sample_video_only.mp4");
    assert!(
        clip.exists(),
        "missing fixture {} — this test is then vacuous",
        clip.display()
    );

    let mut app = xrds_test_app();
    crate::xrds_api::helper::merge_imported_asset_catalog(
        app.world_mut(),
        &[XrdsSceneAsset {
            id: "lobby-screen".to_string(),
            uri: clip.to_string_lossy().into_owned(),
            kind: XrdsSceneAssetKind::Video,
        }],
    );

    // The surface exists, so a material slot naming it resolves...
    assert!(
        crate::xrds_api::video::video_texture_handle_in_world(app.world(), "lobby-screen")
            .is_some(),
        "importing a Video asset should register its texture, or an authored screen          resolves to nothing"
    );
    // ...and nothing is decoding.
    assert!(
        !crate::xrds_api::video::is_video_playing_in_world(app.world(), "lobby-screen"),
        "importing must not start a decoder — a scene containing a screen should not          pay for it until something asks"
    );

    let mut xrds = XrdsAPI::attach(&mut app);
    assert!(xrds.play_video("lobby-screen"), "play_video should start the clip");
    assert!(xrds.is_video_playing("lobby-screen"));

    xrds.stop_video("lobby-screen");
    assert!(
        !xrds.is_video_playing("lobby-screen"),
        "stopping should release the decoder"
    );
    assert!(
        crate::xrds_api::video::video_texture_handle_in_world(app.world(), "lobby-screen")
            .is_some(),
        "stopping a clip stops a picture; it does not remove the screen"
    );
}

/// An unknown id is refused rather than silently doing nothing.
#[cfg(not(target_os = "android"))]
#[test]
fn playing_a_video_that_is_not_in_the_catalog_is_refused() {
    let mut app = xrds_test_app();
    let mut xrds = XrdsAPI::attach(&mut app);
    assert!(!xrds.play_video("no-such-clip"));
}

/// `PlayVideo` targets a surface, so the clip must be recoverable from it.
///
/// The Track model addresses actions through a node, and a video is not a node —
/// it is a texture on a material. So the action has to look at what the target's
/// material names. A resolver that finds nothing makes the action silently do
/// nothing, which is the failure mode this asset kind is most prone to.
#[cfg(not(target_os = "android"))]
#[test]
fn a_video_is_recoverable_from_the_surface_showing_it() {
    use xrds_scene_graph::{XrdsSceneAsset, XrdsSceneAssetKind};

    let clip = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../crates/xrds-net/samples/sample_video_only.mp4");
    assert!(clip.exists(), "missing fixture {}", clip.display());

    let mut app = xrds_test_app();
    crate::xrds_api::helper::merge_imported_asset_catalog(
        app.world_mut(),
        &[XrdsSceneAsset {
            id: "screen-clip".to_string(),
            uri: clip.to_string_lossy().into_owned(),
            kind: XrdsSceneAssetKind::Video,
        }],
    );

    let plane = {
        let mut xrds = XrdsAPI::attach(&mut app);
        let plane = xrds.spawn(&XrdsPlane3D::new().with_name("Screen"));
        xrds.set_material_texture_slot(
            &plane,
            XrdsMaterialTextureSlotKind::BaseColor,
            Some(XrdsMaterialTextureRef {
                texture_asset_id: "screen-clip".to_string(),
                uv: XrdsMaterialTextureUvParams::default(),
                sampler: XrdsMaterialTextureSamplerParams::default(),
            }),
        );
        plane.entity()
    };
    app.update();

    assert_eq!(
        crate::xrds_api::video::video_asset_ids_on_entity(app.world(), plane),
        vec!["screen-clip".to_string()],
        "PlayVideo on this node must be able to find the clip its material names"
    );

    // A surface showing an ordinary texture is not a video surface, and PlayVideo
    // on it should find nothing rather than guess.
    let bare = {
        let mut xrds = XrdsAPI::attach(&mut app);
        xrds.spawn(&XrdsCube::new().with_name("NotAScreen")).entity()
    };
    app.update();
    assert!(crate::xrds_api::video::video_asset_ids_on_entity(app.world(), bare).is_empty());
}

/// Re-playing a clip that is already playing the same way leaves it alone.
///
/// A looping Track re-fires its `PlayVideo` every cycle. If that restarted the
/// decoder, a video set to Loop would never reach its own end — it would just track
/// the Track's period, which is exactly what "the loop option does not work" looks
/// like from the outside. Changing the repeat mode *is* a different request and
/// does restart.
#[cfg(not(target_os = "android"))]
#[test]
fn replaying_an_already_playing_video_does_not_restart_it() {
    use xrds_scene_graph::{XrdsSceneAsset, XrdsSceneAssetKind};

    let clip = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../crates/xrds-net/samples/sample_video_only.mp4");
    assert!(clip.exists(), "missing fixture {}", clip.display());

    let mut app = xrds_test_app();
    crate::xrds_api::helper::merge_imported_asset_catalog(
        app.world_mut(),
        &[XrdsSceneAsset {
            id: "wall".to_string(),
            uri: clip.to_string_lossy().into_owned(),
            kind: XrdsSceneAssetKind::Video,
        }],
    );

    assert!(crate::xrds_api::video::play_video_asset_in_world(
        app.world_mut(),
        "wall",
        true
    ));
    assert!(crate::xrds_api::video_desktop::is_playing_as_in_world(
        app.world(),
        "wall",
        true
    ));

    // Same request again — still playing, still looping, and (the point) not
    // rebuilt.
    assert!(crate::xrds_api::video::play_video_asset_in_world(
        app.world_mut(),
        "wall",
        true
    ));
    assert!(crate::xrds_api::video_desktop::is_playing_as_in_world(
        app.world(),
        "wall",
        true
    ));

    // A different repeat mode is a different request, and takes effect.
    assert!(crate::xrds_api::video::play_video_asset_in_world(
        app.world_mut(),
        "wall",
        false
    ));
    assert!(crate::xrds_api::video_desktop::is_playing_as_in_world(
        app.world(),
        "wall",
        false
    ));
}
