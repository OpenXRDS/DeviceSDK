use super::*;

/// On-device verification fixture. Not a real test — it only runs when
/// XRDS_GEN_DEVICE_SCENE points somewhere, so `cargo test` is unaffected.
///
/// Kept rather than deleted, but treat the *contents* as scratch: edit it to build
/// whatever the current device check needs. Hand-written `scene.json` drifts from
/// the schema, whereas this goes through the real constructors and `save_json`, so
/// whatever it emits is a valid document by construction.
///
/// Driven by `docs/quest-device-test-recipe.md`, which carries the deploy sequence
/// and the traps.
///
/// # Current check: 360 video over passthrough
///
/// The device pass for `docs/video-360-plan.md`. Two scenes, chosen with
/// `XRDS_GEN_DEVICE_SCENE_KIND`, because putting both checks in one frame confounds
/// them — the sweep has to blend against the *room*, and a 360 sphere is a wall
/// between the two.
///
/// ## `KIND=sweep` (default) — is the compositor's alpha convention right?
///
/// Five unlit white quads at opacity 1.0, 0.8, 0.6, 0.4, 0.2, left to right, over
/// passthrough and nothing else.
///
/// This exists because the bug it looks for **is invisible at one opacity**.
/// `projection.rs` used to declare the layer `UNPREMULTIPLIED_ALPHA` while Bevy was
/// writing premultiplied alpha, so the runtime multiplied a second time and
/// composited `a²·C` where `a·C` was wanted. At the 0.8 a 360 video is authored at
/// that is 20% dark — and with no reference in the headset, 20% dark is simply what
/// the video looks like. A row makes the reference part of the picture:
///
/// - **Correct (premultiplied):** brightness falls **linearly** — 1.0, 0.8, 0.6,
///   0.4, 0.2. An even ramp, and the last quad is still clearly a pale wash over
///   the room.
/// - **Wrong (double-multiplied):** brightness falls as the **square** — 1.0, 0.64,
///   0.36, 0.16, 0.04. Bunched hard toward the dark end, and the last quad has all
///   but vanished.
///
/// Those are not subtle next to each other, which is the entire point of the row.
///
/// ## `KIND=dome` — does the feature work?
///
/// A `Video360` at opacity 0.8, a `PassthroughWindow`, and three markers placed to
/// make the window's contract visible rather than merely claimed:
///
/// - `BarBehindWindow` is a bar **wider than the window**, passing behind it. A
///   working window **interrupts it**: the ends stick out past the window's edges
///   and the middle is gone.
/// - `CubeInFrontOfWindow` sits in front, above the bar's line, and **must be
///   visible** — drawn over the clear room.
/// - `Liveness` sits off to the left, outside the window. If the whole view is black
///   this cube says whether the app is rendering at all — the distinction a headset
///   otherwise cannot make.
///
/// **The bar replaced a pair of small cubes after the first device run**, where the
/// check turned out to be unreadable: two identical black cubes 0.6 m apart at ~2 m
/// cannot be told from each other in a headset, so "is the one behind hidden?" had no
/// answer — an absent object and a misidentified one look the same. A single object
/// whose own shape carries the evidence needs no attribution.
///
/// ## Running it
///
/// Asset URIs are bare filenames because the asset-server root *is* `assets/`:
///
/// ```text
///   scene_dir/
///     scene.json
///     assets/
///       360_testrun.mp4  <- KIND=dome only; copied from assets/videos/
/// ```
///
/// The clip is **not in the repo** — `assets/videos/` is gitignored, because video is
/// the one asset kind here routinely too large to track. Copy it in, or name another
/// with `XRDS_GEN_DEVICE_SCENE_CLIP`.
///
/// ```bash
/// XRDS_GEN_DEVICE_SCENE=/path/to/scene_dir/scene.json \
///     cargo test -p xrds-scene-graph xxx_gen -- --nocapture
/// XRDS_GEN_DEVICE_SCENE_KIND=dome XRDS_GEN_DEVICE_SCENE=... cargo test ...
/// ```
///
/// Both scenes set `xr_blend_mode = AlphaBlend`. Without it the compositor submits
/// no passthrough layer, and *every* check here reads as a failure of something
/// else — a black quad row, a window that does nothing.
#[test]
fn xxx_gen_device_check_scene() {
    let Ok(out) = std::env::var("XRDS_GEN_DEVICE_SCENE") else {
        return;
    };
    let kind = std::env::var("XRDS_GEN_DEVICE_SCENE_KIND").unwrap_or_else(|_| "sweep".to_string());

    let mut doc = XrdsSceneDocument::default();
    let mut next = 1u64;
    let mut id = || {
        let v = XrdsSceneNodeId(next);
        next += 1;
        v
    };

    // The one setting every check here depends on. Without it there is no
    // passthrough layer beneath the scene and nothing below can be judged.
    doc.metadata.xr_blend_mode = XrdsXrBlendMode::AlphaBlend;

    // Content sits near the spawn at (0, 1.6, 8) looking toward -Z, not near the
    // origin — see trap 4 in the recipe. Eye height for everything, so the row is
    // straight ahead rather than underfoot.
    const EYE: f32 = 1.6;

    match kind.as_str() {
        "dome" => {
            // The clip that ships with this check, overridable for another one.
            // `assets/videos/` is gitignored — video is the one asset kind here
            // routinely too large to track — so this names a file the operator
            // copies in rather than one the repo carries.
            let clip = std::env::var("XRDS_GEN_DEVICE_SCENE_CLIP")
                .unwrap_or_else(|_| "360_testrun.mp4".to_string());
            doc.assets.push(XrdsSceneAsset {
                id: "clip".to_string(),
                uri: clip.clone(),
                kind: XrdsSceneAssetKind::Video,
            });

            // autoplay, because a Track cannot be fired by hand while wearing a
            // headset and a still first frame is indistinguishable from a video
            // that will not play.
            doc.nodes.push(video_360_node(
                id(),
                "Dome",
                XrdsSceneVideo360 {
                    video_asset_id: "clip".to_string(),
                    radius: 50.0,
                    opacity: 0.8,
                    autoplay: true,
                    // Explicit rather than defaulted: a device check that runs out
                    // of footage while the headset is being donned reports a frozen
                    // frame as a broken video, which is exactly what happened on
                    // 2026-09-02 and cost a diagnostic detour.
                    looped: true,
                },
            ));

            // 1.5 m × 1.0 m, two metres in front. The quad is built in local XY
            // facing +Z, and the viewer is on the +Z side, so it needs no rotation.
            doc.nodes.push(passthrough_window_node(
                id(),
                "Window",
                [0.0, EYE, 6.0],
                XrdsScenePassthroughWindow {
                    width: 1.5,
                    height: 1.0,
                },
            ));

            // **A bar, not a cube, and wider than the window.** The first version
            // put a small cube behind the window and another in front, and on
            // device the check was unreadable: two identical black cubes 0.6 m
            // apart at ~2 m cannot be told from each other in a headset, so
            // "is the one behind hidden?" had no answer — an absent object and a
            // misidentified one look the same.
            //
            // A bar spanning wider than the window answers it with no attribution
            // needed. It passes behind the hole, so a working window **interrupts
            // it**: the ends stick out past the window's edges and the middle is
            // simply gone. One object, and the evidence is its own shape.
            //
            // 3.0 m wide against a window cone that is ~1.9 m across at this
            // depth, so roughly half a metre of bar shows on each side.
            doc.nodes.push(box_node(
                id(),
                "BarBehindWindow",
                [0.0, EYE, 5.5],
                [3.0, 0.25, 0.05],
            ));
            // In front of the window plane and inside the hole, but placed *above*
            // the bar's line rather than beside it, so it cannot be mistaken for
            // one of the bar's ends. Must still draw, over the clear room.
            doc.nodes
                .push(cube_node(id(), "CubeInFrontOfWindow", [0.3, 1.9, 6.6], 0.15));
            // Outside the window entirely: the liveness marker.
            doc.nodes.push(cube_node(id(), "Liveness", [-2.0, EYE, 6.0], 0.3));
        }
        _ => {
            // The ramp. Left to right at a fixed distance, so the only difference
            // between neighbours is the authored opacity.
            for (i, opacity) in [1.0f32, 0.8, 0.6, 0.4, 0.2].iter().enumerate() {
                let x = -1.2 + i as f32 * 0.6;
                let mut quad = XrdsPlane3D::new().with_name(&format!("Op{opacity:.1}"));
                quad.size = [0.5, 0.5];
                quad.transform.translation = [x, EYE, 6.0];
                // +90° about X stands the ground-plane quad up facing +Z, toward a
                // viewer at z = 8. Unrotated it lies flat and is edge-on.
                quad.transform.rotation_quat_xyzw = [
                    std::f32::consts::FRAC_1_SQRT_2,
                    0.0,
                    0.0,
                    std::f32::consts::FRAC_1_SQRT_2,
                ];
                doc.nodes.push(XrdsSceneNode::from_xrds_plane3d(
                    id(),
                    None,
                    &quad,
                    Some(XrdsMaterialParams {
                        base_color: XrdsColor {
                            rgba: [1.0, 1.0, 1.0, 1.0],
                        },
                        // Unlit, or the ramp measures the lighting instead of the
                        // compositor — which is the only thing being read here.
                        unlit: true,
                        opacity: *opacity,
                        ..Default::default()
                    }),
                ));
            }
        }
    }

    doc.save_json(std::path::Path::new(&out))
        .expect("device-check scene should save");
    println!(
        "[gen] wrote {out}: kind={kind}, {} nodes, xr_blend_mode=AlphaBlend",
        doc.nodes.len()
    );
}

#[cfg(test)]
fn device_check_node(
    id: XrdsSceneNodeId,
    name: &str,
    translation: [f32; 3],
    payload: XrdsSceneNodePayload,
) -> XrdsSceneNode {
    let mut transform = XrdsSceneTransform::default();
    transform.translation = translation;
    XrdsSceneNode {
        id,
        parent_id: None,
        name: name.to_string(),
        enabled: true,
        visible: true,
        transform,
        payload,
        grabbable: false,
        editor: XrdsEditorMetadata::default(),
        triggers: Vec::new(),
        watchers: Vec::new(),
    }
}

#[cfg(test)]
fn video_360_node(id: XrdsSceneNodeId, name: &str, video: XrdsSceneVideo360) -> XrdsSceneNode {
    device_check_node(
        id,
        name,
        [0.0, 0.0, 0.0],
        XrdsSceneNodePayload::Video360(video),
    )
}

#[cfg(test)]
fn passthrough_window_node(
    id: XrdsSceneNodeId,
    name: &str,
    translation: [f32; 3],
    window: XrdsScenePassthroughWindow,
) -> XrdsSceneNode {
    device_check_node(
        id,
        name,
        translation,
        XrdsSceneNodePayload::PassthroughWindow(window),
    )
}

#[cfg(test)]
fn cube_node(id: XrdsSceneNodeId, name: &str, translation: [f32; 3], size: f32) -> XrdsSceneNode {
    box_node(id, name, translation, [size, size, size])
}

#[cfg(test)]
fn box_node(
    id: XrdsSceneNodeId,
    name: &str,
    translation: [f32; 3],
    size: [f32; 3],
) -> XrdsSceneNode {
    let mut cube = XrdsCube::new().with_name(name);
    cube.size = size;
    cube.transform.translation = translation;
    XrdsSceneNode::from_xrds_cube(id, None, &cube, None)
}
