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
                if let Some(node) = build_primitive_node(doc, kind, parent) {
                    doc.nodes.push(node);
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
    doc.assets.iter().map(|a| AssetCatalogEntry {
        id: a.id.clone(),
        name: a.id.clone(),
        kind: format!("{:?}", a.kind),
    }).collect()
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

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
        "AudioClip"       => XrdsSceneNodePayload::AudioClip(XrdsSceneAudioClip::default()),
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
