use super::*;

/// On-device verification fixture. Not a real test — it only runs when
/// XRDS_GEN_DEVICE_SCENE points somewhere, so `cargo test` is unaffected.
///
/// Kept rather than deleted, but treat the *contents* as scratch: edit it to build
/// whatever the current device check needs. Hand-written `scene.json` drifts from
/// the schema, whereas this goes through the real `from_xrds_*` constructors and
/// `save_json`, so whatever it emits is a valid document by construction.
///
/// Driven by `docs/quest-device-test-recipe.md`, which carries the deploy sequence
/// and the traps.
///
/// # Current check: passthrough (S4), and spatial audio still working under it
///
/// The scene carries `xr_blend_mode = AlphaBlend` and **no ground plane**.
/// Passthrough composites beneath the scene and shows only where the scene's alpha
/// is below 1.0, so an opaque floor would hide it. What should appear: the real
/// room, with two floating markers and the ping still audible from the rear one.
///
/// If the markers render but the background is black rather than the room, the
/// layer is not reaching the compositor or the camera clear is still opaque. If
/// nothing renders at all, suspect the layer order.
///
/// # Previous check, still exercised: does head tracking resolve front/back?
///
/// Previous contents built the zone-enter check for
/// `docs/done/player-body-collider-plan.md`; that landed and was verified, so the
/// scene has been repurposed. Recover it from git if it is needed again.
///
/// ## The question
///
/// Bevy's spatial audio is rodio's, which emits **one gain per ear** and nothing
/// else — no filtering, no inter-aural delay, no HRTF. A source 30° front-left and
/// one 30° back-left therefore produce identical ear gains. Statically, front and
/// back are indistinguishable, and that is not a bug we can fix from our side.
///
/// What may rescue it is **movement**. Turn your head and a front source drifts one
/// way in the stereo image while a rear source drifts the other; listeners use this
/// unconsciously. Bevy already feeds it: ear positions come from
/// `transform.transform_point(ear_offset)` — the full transform, rotation included
/// (`bevy_audio-0.17.2/src/audio_output.rs:73`) — and `update_listener_positions`
/// re-runs on `Changed<GlobalTransform>`. Every XRDS camera is the listener, so a
/// head turn on a Quest already repans every source.
///
/// None of that is worth anything until somebody hears it, and it cannot be heard
/// on desktop, where the check example drives a camera that never rotates. Hence a
/// device scene. The answer decides whether HRTF is worth costing out at all — see
/// `docs/spatial-audio-backend-spike.md`.
///
/// ## The scene, and why it is shaped this way
///
/// **Two identical markers, one in front and one behind, and audio from only one.**
/// The task is "which marker is making the sound?", which is a genuine forced choice
/// rather than an invitation to agree with the tester.
///
/// - Identical size, colour and distance, so nothing visual distinguishes them.
///   A single marker would let the listener infer the answer from the fact that
///   only one object exists.
/// - The sound is on the **rear** marker. Guessing "in front" is the default
///   assumption, so a correct answer is informative.
/// - `spatial_test_ping.wav`: mono, 1 s loop, four broadband bursts with sharp
///   onsets. Onsets are what the ear localises with. Every other clip in
///   `assets/sound/` is stereo and ~20 s, and rodio downmixes a spatial source to
///   mono — so a recording with its own baked-in stereo movement fights the cue
///   under test. `assets/sound` is staged into the APK automatically by
///   `build.ps1`, so no scene-dir assets are needed.
/// - `max_distance` **6 m**, not 30. The first device attempt used 30 so the source
///   would stay audible while turning, and that made the falloff impossible to
///   hear: `Linear` over 1..30 m gives gain 0.948 at 2.5 m and 0.862 at 5 m —
///   about 0.8 dB across the whole walkable area. Over 1..6 m the same walk spans
///   0.70 down to 0.20 and then silence, which a listener can actually judge. Set
///   the range to the space the tester can physically cover.
///
/// Positions are relative to xrds-app's default spawn `(0, 1.6, 8)` looking down
/// -Z — not the origin (recipe Trap 4). The markers sit 2.5 m ahead and 2.5 m
/// behind at ear height.
///
/// ## Wear headphones. The built-in speakers cannot show this.
///
/// Quest's speakers are open-ear and both of them reach both ears — acoustic
/// crosstalk. Amplitude panning works *entirely* by inter-aural level difference,
/// so crosstalk destroys precisely the cue under test. A first device run on the
/// built-in speakers produced "I can hear the ping but hard to recognize the volume
/// and direction changing", which says more about the speakers than about the code.
/// Pair Bluetooth headphones (Settings → Devices → Bluetooth); latency is
/// irrelevant here because the judgement is about a continuous sound, not about
/// sync with an action. If the headphones offer their own spatial-audio or
/// head-tracking mode, **turn it off** — a second spatializer on top of ours
/// invalidates the result.
///
/// This is worth knowing beyond the test: on Quest speakers, amplitude-panned
/// spatial audio is inherently weak for every XRDS app, not just this scene.
///
/// ## Running it
///
/// Put the headset on, stand still, and **turn slowly on the spot**. Decide which
/// marker the sound is coming from before looking. Then look.
///
/// Then walk toward and away from the sounding marker — over a 6 m range the level
/// change should be obvious. That is the S1 falloff on device.
///
/// - Correct, and the direction feels stable in the world as you turn → rung 1
///   works, and front/back may not need HRTF.
/// - It stays centred and you cannot tell → head tracking is not enough, and
///   binaural is the only route left.
#[test]
fn xxx_gen_device_check_scene() {
    let Ok(out) = std::env::var("XRDS_GEN_DEVICE_SCENE") else {
        return;
    };

    let mut doc = XrdsSceneDocument::default();
    let mut next = 1u64;
    let mut id = || {
        let v = XrdsSceneNodeId(next);
        next += 1;
        v
    };

    // Spawn is (0, 1.6, 8) looking down -Z, so -Z is "ahead" and +Z is "behind".
    const EAR_Y: f32 = 1.6;
    const OFFSET: f32 = 2.5;
    const FRONT_Z: f32 = 8.0 - OFFSET;
    const BACK_Z: f32 = 8.0 + OFFSET;

    const AUDIO_ASSET_ID: &str = "asset:spatial-ping";

    // Passthrough: reality is composited *beneath* the scene and shows only where
    // the scene's alpha is below 1.0. A ground plane therefore has to go — a
    // 24x24 opaque slab would hide the floor and most of the point.
    //
    // Two things are being checked at once, deliberately: that passthrough appears
    // at all, and that the audio work still behaves with it on. The markers stay
    // visible because they are opaque geometry over a transparent background.
    doc.metadata.xr_blend_mode = XrdsXrBlendMode::AlphaBlend;

    let mut sun = XrdsDirectionalLight::new().with_name("Sun");
    sun.transform.translation = [2.0, 5.0, 6.0];
    doc.nodes
        .push(XrdsSceneNode::from_xrds_directional_light(id(), None, &sun));

    // --- The two markers ----------------------------------------------------
    // Deliberately indistinguishable. Emissive so they read clearly against the
    // ground without depending on where the sun happens to fall.
    let mut marker_material = xrds_components::XrdsMaterialParams::default();
    marker_material.base_color = XrdsColor {
        rgba: [0.85, 0.85, 0.9, 1.0],
    };
    marker_material.unlit = true;

    for (name, z) in [("MARKER_FRONT", FRONT_Z), ("MARKER_BACK", BACK_Z)] {
        let mut marker = XrdsSphere::new().with_name(name);
        marker.radius = 0.22;
        marker.transform.translation = [0.0, EAR_Y, z];
        doc.nodes.push(XrdsSceneNode::from_xrds_sphere(
            id(),
            None,
            &marker,
            Some(marker_material.clone()),
        ));
    }

    // --- The sound, on the REAR marker only ---------------------------------
    doc.assets.push(XrdsSceneAsset {
        id: AUDIO_ASSET_ID.to_string(),
        uri: "sound/wav/spatial_test_ping.wav".to_string(),
        kind: XrdsSceneAssetKind::Audio,
    });

    let mut ping = xrds_components::XrdsAudioClip::new(AUDIO_ASSET_ID).with_name("REAR_SOURCE");
    ping.transform.translation = [0.0, EAR_Y, BACK_Z];
    ping.volume = 1.0;
    ping.looped = true;
    ping.spatial = true;
    ping.autoplay = true;
    // Inverse, not Linear: linear-in-amplitude is brutal in loudness terms, since
    // the final stretch runs -14 dB to silence over one metre. Inverse is how sound
    // actually behaves and holds a usable level across the room.
    ping.distance_model = xrds_components::XrdsAudioDistanceModel::Inverse;
    // `min_distance` is the reference radius, not just a near clamp: raising it
    // flattens the near field so the whole curve is gentler. At 2.0 the far marker
    // (5 m away) sits at 0.4 — clearly audible — where the previous
    // `min 1 / max 6 / Linear` setup put it at 0.18, which read as nothing.
    ping.min_distance = 2.0;
    ping.max_distance = 15.0;
    ping.rolloff_factor = 1.0;
    doc.nodes
        .push(XrdsSceneNode::from_xrds_audio_clip(id(), None, &ping));

    doc.save_json(std::path::Path::new(&out))
        .expect("device-check scene should save");
    println!("[gen] wrote {out} with {} nodes", doc.nodes.len());
    println!("[gen] sound is on MARKER_BACK at z={BACK_Z}; MARKER_FRONT at z={FRONT_Z} is silent");
}
