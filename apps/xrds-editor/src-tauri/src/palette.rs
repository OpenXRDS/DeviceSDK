use xrds_scene_graph::{
    XrdsEditorMetadata, XrdsSceneAmbientLight, XrdsSceneAssetKind, XrdsSceneAudioClip,
    XrdsSceneCamera, XrdsSceneCameraProjection, XrdsSceneCapsule, XrdsSceneCube, XrdsSceneCylinder,
    XrdsSceneEffect, XrdsSceneEffectKind,
    XrdsSceneDirectionalLight, XrdsSceneDocument, XrdsSceneExtrudedText, XrdsSceneGltfAsset,
    XrdsSceneHudText, XrdsSceneInteractionZone, XrdsSceneNode, XrdsSceneNodeId,
    XrdsSceneNodePayload, XrdsScenePlane3D, XrdsScenePlayer, XrdsScenePlayerAnchor,
    XrdsScenePlayerSpawn, XrdsScenePlayerSpawnZone, XrdsScenePointLight, XrdsSceneSpotLight,
    XrdsSceneSphere, XrdsSceneTetrahedron, XrdsSceneText, XrdsSceneTransform,
};
use bevy::log::error;
use crate::bridge::{AssetCatalogEntry, EditorCommand};
use crate::editor_state::{EditorSession, EditorState};

const IDENTITY_ROT: [f32; 4] = [0.0, 0.0, 0.0, 1.0];

/// Apply a palette (spawn) EditorCommand. Returns true if a reimport is needed.
pub fn apply_palette_command(
    cmd: &EditorCommand,
    session: &mut EditorSession,
    state: &mut EditorState,
) -> bool {
    match cmd {
        EditorCommand::SpawnPrimitive { kind, parent_id } => {
            let parent = parent_id.map(XrdsSceneNodeId);

            // Checked before the edit so the author gets a sentence they can act
            // on. Without this the spawn silently produces no node — the palette
            // click just does nothing, which is indistinguishable from a broken
            // button.
            if kind == "AudioClip"
                && !session
                    .0
                    .document()
                    .assets
                    .iter()
                    .any(|a| a.kind == XrdsSceneAssetKind::Audio)
            {
                let message = "Import a sound first (Ctrl+I) — an audio clip needs a file to play";
                bevy::log::info!("[palette] {message}");
                state.pending_status = Some(message.to_string());
                return false;
            }
            // Peek at next ID before the edit so we can queue the spawn.
            let new_id = {
                let doc = session.0.document();
                next_id(doc)
            };
            match session.0.edit(|doc| {
                // A `Panel` node is *nothing but* a template reference, so
                // placing one needs a template to point at. Create a starter
                // template when the library is empty rather than refusing the
                // spawn: the palette entry would otherwise look broken to anyone
                // who has not visited the Panels workspace yet.
                if kind == "Panel" && doc.panels.is_empty() {
                    let id = doc.next_available_panel_template_id();
                    doc.panels.push(xrds_scene_graph::XrdsPanelTemplate {
                        id,
                        name: "Panel".to_string(),
                        ..Default::default()
                    });
                }
                match build_primitive_node(doc, kind, parent) {
                    Some(node) => doc.nodes.push(node),
                    // Never silent again. A `None` here means the palette offered a
                    // kind it then declined to build — a click that does nothing,
                    // with no error, no node and no clue. That is exactly how the
                    // Video360 entry shipped broken, so the drop now says so.
                    None => error!(
                        "[palette] SpawnPrimitive '{kind}': no node was built. The kind is                          offered by the palette but `build_primitive_node` returned None —                          usually a required asset that does not exist."
                    ),
                }
            }) {
                Ok(_) => {}
                Err(e) => {
                    error!("[palette] SpawnPrimitive '{}' rejected by document validation: {:?}", kind, e);
                    return false;
                }
            }
            true // trigger full reimport
        }

        EditorCommand::SpawnAsset { asset_id, parent_id } => {
            let parent = parent_id.map(XrdsSceneNodeId);
            let asset_id = asset_id.clone();
            let new_id = {
                let doc = session.0.document();
                next_id(doc)
            };
            match session.0.edit(|doc| {
                let asset = doc.assets.iter().find(|a| a.id == asset_id).cloned();
                if let Some(asset) = asset {
                    let payload = match asset.kind {
                        XrdsSceneAssetKind::Gltf => XrdsSceneNodePayload::GltfAsset(
                            XrdsSceneGltfAsset {
                                asset_id: Some(asset_id.clone()),
                                asset_uri: asset.uri.clone(),
                                scene_index: 0,
                                export_policy: xrds_scene_graph::XrdsGltfAssetExportPolicy::KeepExternalReference,
                            },
                        ),
                        XrdsSceneAssetKind::Audio => XrdsSceneNodePayload::AudioClip(
                            XrdsSceneAudioClip {
                                asset_id: asset_id.clone(),
                                ..Default::default()
                            },
                        ),
                        _ => return, // textures/environment maps are not scene nodes
                    };
                    let transform = default_transform_for_payload(&payload, doc, parent);
                    doc.nodes.push(XrdsSceneNode {
                        id: new_id,
                        parent_id: parent,
                        name: asset.id.clone(),
                        enabled: true,
                        visible: true,
                        grabbable: false,
                        transform,
                        payload,
                        editor: XrdsEditorMetadata::default(),
                        triggers: Vec::new(),
                        watchers: Vec::new(),
                    });
                }
            }) {
                Ok(_) => {}
                Err(e) => {
                    error!("[palette] SpawnAsset '{}' rejected by document validation: {:?}", asset_id, e);
                    return false;
                }
            }
            true
        }

        _ => false,
    }
}

// ---------------------------------------------------------------------------
// Asset catalog serializer
// ---------------------------------------------------------------------------

pub fn build_asset_catalog(doc: &XrdsSceneDocument) -> Vec<AssetCatalogEntry> {
    doc.assets
        .iter()
        .map(|a| AssetCatalogEntry {
            id: a.id.clone(),
            name: a.id.clone(),
            kind: format!("{:?}", a.kind),
            bound_to_node: if a.kind == xrds_scene_graph::XrdsSceneAssetKind::Video {
                video_owner(doc, &a.id)
            } else {
                None
            },
        })
        .collect()
}

/// The node whose material shows `asset_id`, if any.
///
/// Mirrors the document's own one-video-one-mesh rule so the picker and the
/// validator agree; a picker that offers what the validator refuses is the
/// arrangement that produced `MaterialTextureAssetKindMismatch` on commit.
fn video_owner(doc: &XrdsSceneDocument, asset_id: &str) -> Option<u64> {
    // Through the document's own accessor, which knows both places a video can
    // live — a material texture slot, or a `Video360` payload. This walked only
    // material slots once, and the result was that a clip already bound to a 360
    // sphere was offered as free and then refused by the validator.
    doc.nodes
        .iter()
        .find(|node| {
            doc.video_asset_ids_on_node(node.id)
                .iter()
                .any(|id| id == asset_id)
        })
        .map(|node| node.id.0)
}

fn next_id(doc: &XrdsSceneDocument) -> XrdsSceneNodeId {
    XrdsSceneNodeId(doc.nodes.iter().map(|n| n.id.0).max().unwrap_or(0) + 1)
}

fn build_primitive_node(
    doc: &XrdsSceneDocument,
    kind: &str,
    parent_id: Option<XrdsSceneNodeId>,
) -> Option<XrdsSceneNode> {
    let new_id = next_id(doc);

    let payload: XrdsSceneNodePayload = match kind {
        "Empty"           => XrdsSceneNodePayload::Empty,
        "Cube"            => XrdsSceneNodePayload::Cube(XrdsSceneCube::default()),
        "Sphere"          => XrdsSceneNodePayload::Sphere(XrdsSceneSphere::default()),
        "Cylinder"        => XrdsSceneNodePayload::Cylinder(XrdsSceneCylinder::default()),
        "Capsule"         => XrdsSceneNodePayload::Capsule(XrdsSceneCapsule::default()),
        // auto_play=false, deliberately differing from the Rust default: a burst
        // placed by hand almost always wants a trigger to fire it, and an
        // auto-playing one would go off once at load and never again. Trail
        // keeps auto_play=true so it visibly runs as soon as it is placed.
        "EffectBurst"     => XrdsSceneNodePayload::Effect(XrdsSceneEffect {
            kind: XrdsSceneEffectKind::Burst,
            auto_play: false,
            ..Default::default()
        }),
        "EffectTrail"     => XrdsSceneNodePayload::Effect(XrdsSceneEffect {
            kind: XrdsSceneEffectKind::Trail,
            auto_play: true,
            omnidirectional: false,
            spread_deg: 20.0,
            gravity: [0.0, 0.5, 0.0],
            ..Default::default()
        }),
        "Plane"           => XrdsSceneNodePayload::Plane3D(XrdsScenePlane3D::default()),
        "Tetrahedron"     => XrdsSceneNodePayload::Tetrahedron(XrdsSceneTetrahedron::default()),
        "PointLight"      => XrdsSceneNodePayload::PointLight(XrdsScenePointLight::default()),
        "SpotLight"       => XrdsSceneNodePayload::SpotLight(XrdsSceneSpotLight::default()),
        "DirectionalLight"=> XrdsSceneNodePayload::DirectionalLight(XrdsSceneDirectionalLight::default()),
        "AmbientLight"    => XrdsSceneNodePayload::AmbientLight(XrdsSceneAmbientLight::default()),
        "Camera"          => {
            // Assign a unique order so it doesn't conflict with existing cameras.
            let max_order: isize = doc.nodes.iter()
                .filter_map(|n| match &n.payload {
                    XrdsSceneNodePayload::Camera(c) => Some(match c.projection {
                        XrdsSceneCameraProjection::Perspective { order, .. } => order,
                        XrdsSceneCameraProjection::Orthographic { order, .. } => order,
                    }),
                    _ => None,
                })
                .max()
                .unwrap_or(0);
            let mut cam = XrdsSceneCamera::default();
            cam.projection = XrdsSceneCameraProjection::Perspective {
                fov_deg: 60.0,
                near: 0.1,
                far: Some(1000.0),
                order: max_order + 1,
            };
            XrdsSceneNodePayload::Camera(cam)
        }
        "Text"            => XrdsSceneNodePayload::Text(XrdsSceneText::default()),
        "Billboard"       => XrdsSceneNodePayload::Text(XrdsSceneText {
            text: "Label".to_string(),
            anchor: xrds_scene_graph::XrdsSceneTextAnchor::Billboard,
            ..XrdsSceneText::default()
        }),
        "ExtrudedText"    => XrdsSceneNodePayload::ExtrudedText(XrdsSceneExtrudedText::default()),
        "HudText"         => XrdsSceneNodePayload::HudText(XrdsSceneHudText::default()),
        // Takes the first imported Audio asset. `XrdsSceneAudioClip::default()`
        // has an empty `asset_id`, and the document validator rejects that
        // (`EmptyAudioClipAssetId`) — correctly, since a clip that names no file
        // can never make a sound. So the palette entry could never place a node:
        // it failed validation every time, logging to the terminal where nobody
        // authoring a scene would see it. Reported 2026-08-19 as "I couldn't find
        // the audio section", which was true — no audio node could exist to select.
        //
        // `?` when there is no audio asset, matching the `Panel` arm's reasoning:
        // no node beats a node that cannot work. The caller turns that into a
        // status message telling the author to import a sound first.
        // Takes the first *unbound* video — one clip is one surface, so handing out
        // a clip another node already shows would place a node the validator
        // immediately refuses.
        //
        // **Falls back to no clip rather than refusing to place the node.** The
        // first version used `?` here, copying the AudioClip arm, and the result
        // was that clicking "360 Video" with no video imported did nothing at all,
        // silently — `build_primitive_node`'s `None` is dropped without a word by
        // the caller. It also contradicted this document's own validator, which
        // deliberately allows an empty clip id because placing the sphere before
        // importing the footage is normal authoring. The inspector's clip picker
        // is where it gets filled in.
        "Video360"        => XrdsSceneNodePayload::Video360(xrds_scene_graph::XrdsSceneVideo360 {
            video_asset_id: doc
                .assets
                .iter()
                .find(|a| {
                    a.kind == XrdsSceneAssetKind::Video && video_owner(doc, &a.id).is_none()
                })
                .map(|a| a.id.clone())
                .unwrap_or_default(),
            ..Default::default()
        }),
        // No asset and nothing to look up: a window is geometry and a rule, not
        // content. Placed at the payload default of 1 m × 1 m.
        "PassthroughWindow" => XrdsSceneNodePayload::PassthroughWindow(Default::default()),
        "AudioClip"       => XrdsSceneNodePayload::AudioClip(XrdsSceneAudioClip {
            asset_id: doc
                .assets
                .iter()
                .find(|a| a.kind == XrdsSceneAssetKind::Audio)
                .map(|a| a.id.clone())?,
            // Left at the payload default of `false`.
            //
            // This was briefly `true`, as a stopgap while `autoplay` was the only
            // way a clip could ever sound — a placed clip was otherwise silent for
            // the whole session and looked broken. Both reasons are gone: the
            // Inspector has a preview button, and `XrdsAction::PlayAudio` lets a
            // zone, button or Track start it. A sound that begins the instant it is
            // placed is intrusive, and looping ambience placed near the camera is
            // the usual first thing an author does.
            ..XrdsSceneAudioClip::default()
        }),
        "InteractionZone" => XrdsSceneNodePayload::InteractionZone(XrdsSceneInteractionZone::default()),
        "PlayerSpawn"     => XrdsSceneNodePayload::PlayerSpawn(XrdsScenePlayerSpawn::default()),
        "PlayerSpawnZone" => XrdsSceneNodePayload::PlayerSpawnZone(XrdsScenePlayerSpawnZone::default()),
        "Player"          => XrdsSceneNodePayload::Player(XrdsScenePlayer::default()),
        "PlayerAnchor"    => XrdsSceneNodePayload::PlayerAnchor(XrdsScenePlayerAnchor::default()),
        // "WorldPanel" was here. Retired: its widgets carried no triggers, so
        // every button on one was permanently dead, and no tracked scene used it.
        // Scene-placed half of "attachment is the only difference" — the same
        // template a PlayerAnchor head-locks. Takes the first template in the
        // library; the Inspector picks which one afterwards. `?` rather than a
        // fallback id: a Panel pointing at a template that does not exist spawns
        // nothing at all, so no node is better than an invisible one.
        "Panel"           => XrdsSceneNodePayload::Panel(xrds_scene_graph::XrdsScenePanelInstance {
            template_id: doc.panels.first().map(|t| t.id)?,
            ..Default::default()
        }),
        _ => return None,
    };

    Some(XrdsSceneNode {
        id: new_id,
        parent_id,
        name: kind.to_string(),
        enabled: true,
        visible: true,
        grabbable: false,
        transform: default_transform_for_payload(&payload, doc, parent_id),
        payload,
        editor: XrdsEditorMetadata::default(),
        triggers: Vec::new(),
        watchers: Vec::new(),
    })
}

/// Whether `parent_id`'s ancestor chain reaches a `PlayerAnchor` — i.e. whether a
/// node placed here will be head-locked.
///
/// Mirrors the runtime's `head_locked_anchor_of`, and is bounded the same way so a
/// `parent_id` cycle in a hand-edited document cannot hang the editor.
fn is_under_player_anchor(doc: &XrdsSceneDocument, parent_id: Option<XrdsSceneNodeId>) -> bool {
    let mut current = parent_id;
    for _ in 0..doc.nodes.len() {
        let Some(id) = current else { return false };
        let Some(node) = doc.nodes.iter().find(|n| n.id == id) else { return false };
        if matches!(node.payload, XrdsSceneNodePayload::PlayerAnchor(_)) {
            return true;
        }
        current = node.parent_id;
    }
    false
}

pub fn default_transform_for_payload(
    payload: &XrdsSceneNodePayload,
    doc: &XrdsSceneDocument,
    parent_id: Option<XrdsSceneNodeId>,
) -> XrdsSceneTransform {
    let base = match payload {
        // Billboard: float at nameplate height in parent-local space.
        // When attached to a character (parent_id set): [0, 2.0, 0] places the label
        // 2 m above the parent's local origin — typical eye/head level for humanoids.
        // When standalone: same offset reads as world-space, also sensible.
        XrdsSceneNodePayload::Text(t)
            if t.anchor == xrds_scene_graph::XrdsSceneTextAnchor::Billboard =>
        {
            let height = if parent_id.is_some() { 2.0 } else { 1.5 };
            [0.0_f32, height, 0.0]
        }
        // Eye-ish height rather than the 0.5 the meshes use. An effect has no
        // silhouette, so one sitting on the floor reads as "effects don't work"
        // rather than "look down" -- exactly the confusion hit during on-device
        // verification, where effects authored far from the viewer looked absent.
        XrdsSceneNodePayload::Effect(_) => [0.0_f32, 1.4, 0.0],
        XrdsSceneNodePayload::Cube(_)
        | XrdsSceneNodePayload::Sphere(_)
        | XrdsSceneNodePayload::Cylinder(_)
        | XrdsSceneNodePayload::Capsule(_)
        | XrdsSceneNodePayload::Tetrahedron(_)
        | XrdsSceneNodePayload::Text(_)
        | XrdsSceneNodePayload::ExtrudedText(_)
        | XrdsSceneNodePayload::GltfAsset(_) => [0.0_f32, 0.5, 0.0],
        XrdsSceneNodePayload::PointLight(_)
        | XrdsSceneNodePayload::SpotLight(_)
        | XrdsSceneNodePayload::DirectionalLight(_) => [0.0, 3.0, 0.0],
        XrdsSceneNodePayload::Camera(_) => [0.0, 3.0, 8.0],
        // PlayerAnchor defaults to eye height (1.6 m) — correct whether it is a child
        // of a ground-level Player node or placed standalone in world space.
        XrdsSceneNodePayload::PlayerAnchor(_) => [0.0, 1.6, 0.0],
        // A Panel node under a PlayerAnchor is head-locked, and its transform is
        // read as **camera-local** — so the world-space default below would put it
        // 1.5 m above the viewer's eye.
        //
        // 1.5 m ahead, not the 0.5 m the retired anchor-link path once used. That
        // value was inherited from the retired `XrdsHudTemplate`, whose only
        // element kind was text and which drew no backdrop: a floating label half
        // a metre out is a HUD, and nothing behind it was hidden. A panel has an
        // opaque backdrop now (`apply_panel_backdrop_in_world`), and the same 0.5 m
        // turns the default 0.6 × 0.4 m template into a blindfold — at the default 60° vertical
        // FOV the visible height at 0.5 m is only 0.577 m, so a 0.4 m panel covers
        // ~69% of it, dead centre. At 1.5 m it covers ~23%, which is what a HUD
        // should look like, and matches where Quest puts its own windows.
        XrdsSceneNodePayload::Panel(_) if is_under_player_anchor(doc, parent_id) => {
            [0.0, 0.0, -1.5]
        }
        // Otherwise panels default to eye height, slightly in front.
        XrdsSceneNodePayload::Panel(_) => [0.0, 1.5, -1.0],
        _ => [0.0, 0.0, 0.0],
    };

    // X offset to prevent standalone spawns from overlapping existing geometry.
    // Not applied to child nodes — their position is already local to the parent,
    // so [0, height, 0] correctly means "centered above parent".
    let offset = if parent_id.is_none() {
        let geometry_count = doc.nodes.iter()
            .filter(|n| matches!(n.payload,
                XrdsSceneNodePayload::Cube(_)     | XrdsSceneNodePayload::Sphere(_)   |
                XrdsSceneNodePayload::Cylinder(_) | XrdsSceneNodePayload::Capsule(_)  |
                XrdsSceneNodePayload::Plane3D(_)  | XrdsSceneNodePayload::Effect(_)   |
                XrdsSceneNodePayload::Tetrahedron(_) | XrdsSceneNodePayload::GltfAsset(_) |
                XrdsSceneNodePayload::Text(_)     | XrdsSceneNodePayload::ExtrudedText(_)
            ))
            .count() as f32;
        geometry_count * 1.5
    } else {
        0.0
    };

    let translation = [base[0] + offset, base[1], base[2]];

    XrdsSceneTransform {
        translation,
        rotation_quat_xyzw: IDENTITY_ROT,
        scale: [1.0, 1.0, 1.0],
    }
}

#[cfg(test)]
mod video_360_palette_tests {
    use super::*;

    fn fresh() -> (EditorSession, EditorState) {
        let session = EditorSession(
            xrds_scene_graph::XrdsSceneDocumentSession::new(XrdsSceneDocument::default())
                .expect("an empty document is valid"),
        );
        (session, EditorState::default())
    }

    fn video_360_count(session: &EditorSession) -> usize {
        session
            .0
            .document()
            .nodes
            .iter()
            .filter(|n| matches!(n.payload, XrdsSceneNodePayload::Video360(_)))
            .count()
    }

    /// A 360 Video node is placed even when no video has been imported.
    ///
    /// **This shipped broken.** The arm used `?` on the asset lookup, so with no
    /// video in the catalog it returned `None` — and `build_primitive_node`'s
    /// `None` is dropped by the caller without a node, an error or a log. Clicking
    /// the palette entry did nothing at all, which is indistinguishable from a
    /// dead button.
    ///
    /// It also contradicted the document's own validator, which allows an empty
    /// clip id on purpose: placing the sphere before importing the footage is
    /// normal authoring, and the inspector's picker is where the clip goes.
    #[test]
    fn a_360_video_node_can_be_placed_before_any_video_is_imported() {
        let (mut session, mut state) = fresh();
        let placed = apply_palette_command(
            &EditorCommand::SpawnPrimitive {
                kind: "Video360".to_string(),
                parent_id: None,
            },
            &mut session,
            &mut state,
        );
        assert!(placed, "the command should report success");
        assert_eq!(
            video_360_count(&session),
            1,
            "a 360 Video node must be placed even with an empty asset catalog"
        );
    }

    /// With a video available it is bound, so the common case needs no second step.
    #[test]
    fn a_360_video_node_picks_up_a_free_clip() {
        let (mut session, mut state) = fresh();
        session
            .0
            .edit(|doc| {
                doc.assets.push(xrds_scene_graph::XrdsSceneAsset {
                    id: "clip".to_string(),
                    uri: "videos/clip.mp4".to_string(),
                    kind: XrdsSceneAssetKind::Video,
                });
            })
            .expect("adding an asset should be valid");

        apply_palette_command(
            &EditorCommand::SpawnPrimitive {
                kind: "Video360".to_string(),
                parent_id: None,
            },
            &mut session,
            &mut state,
        );

        let doc = session.0.document();
        let node = doc
            .nodes
            .iter()
            .find_map(|n| match &n.payload {
                XrdsSceneNodePayload::Video360(v) => Some(v),
                _ => None,
            })
            .expect("a Video360 node should exist");
        assert_eq!(node.video_asset_id, "clip");
    }

    /// A second node does not steal the first one's clip.
    ///
    /// One clip is one surface, and the document refuses a double binding — so
    /// placing a second sphere while the only clip is taken must leave it unbound
    /// rather than produce a node the validator then rejects.
    #[test]
    fn a_second_360_video_node_does_not_steal_the_bound_clip() {
        let (mut session, mut state) = fresh();
        session
            .0
            .edit(|doc| {
                doc.assets.push(xrds_scene_graph::XrdsSceneAsset {
                    id: "clip".to_string(),
                    uri: "videos/clip.mp4".to_string(),
                    kind: XrdsSceneAssetKind::Video,
                });
            })
            .expect("adding an asset should be valid");

        for _ in 0..2 {
            apply_palette_command(
                &EditorCommand::SpawnPrimitive {
                    kind: "Video360".to_string(),
                    parent_id: None,
                },
                &mut session,
                &mut state,
            );
        }

        let doc = session.0.document();
        assert_eq!(video_360_count(&session), 2, "both nodes should be placed");
        let bound: Vec<&str> = doc
            .nodes
            .iter()
            .filter_map(|n| match &n.payload {
                XrdsSceneNodePayload::Video360(v) => Some(v.video_asset_id.as_str()),
                _ => None,
            })
            .collect();
        assert!(bound.contains(&"clip"), "the first should have taken the clip");
        assert!(
            bound.contains(&""),
            "the second must be left unbound, not given the same clip"
        );
    }
}

#[cfg(test)]
mod passthrough_window_palette_tests {
    use super::*;

    /// A Passthrough Window is placed with nothing imported and nothing selected.
    ///
    /// Worth a test despite being a two-line arm, because the payload-default route
    /// is exactly what the `Video360` arm got wrong: that one looked up an asset and
    /// returned `None` when it found none, and `build_primitive_node`'s `None` is
    /// dropped by the caller without a node, an error or a log — a palette entry
    /// that did nothing at all. A window has no asset to look up, so the failure
    /// cannot recur *here*; this pins that it never grows one.
    #[test]
    fn a_passthrough_window_is_placed_from_payload_defaults() {
        let session = EditorSession(
            xrds_scene_graph::XrdsSceneDocumentSession::new(XrdsSceneDocument::default())
                .expect("an empty document is valid"),
        );
        let (mut session, mut state) = (session, EditorState::default());

        let placed = apply_palette_command(
            &EditorCommand::SpawnPrimitive {
                kind: "PassthroughWindow".to_string(),
                parent_id: None,
            },
            &mut session,
            &mut state,
        );
        assert!(placed, "the command should report success");

        let doc = session.0.document();
        let window = doc
            .nodes
            .iter()
            .find_map(|n| match &n.payload {
                XrdsSceneNodePayload::PassthroughWindow(w) => Some(w),
                _ => None,
            })
            .expect("a PassthroughWindow node should exist");
        assert!(
            window.width > 0.0 && window.height > 0.0,
            "a zero-area window is a node that draws nothing and cannot be told \
             apart from one facing away; got {}x{}",
            window.width,
            window.height
        );
    }
}
