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
