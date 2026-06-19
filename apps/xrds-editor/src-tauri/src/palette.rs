use xrds_scene_graph::{
    XrdsEditorMetadata, XrdsSceneAmbientLight, XrdsSceneAssetKind, XrdsSceneAudioClip,
    XrdsSceneCamera, XrdsSceneCameraProjection, XrdsSceneCube, XrdsSceneCylinder,
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
    })
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
        XrdsSceneNodePayload::Cube(_)
        | XrdsSceneNodePayload::Sphere(_)
        | XrdsSceneNodePayload::Cylinder(_)
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
        _ => [0.0, 0.0, 0.0],
    };

    // X offset to prevent standalone spawns from overlapping existing geometry.
    // Not applied to child nodes — their position is already local to the parent,
    // so [0, height, 0] correctly means "centered above parent".
    let offset = if parent_id.is_none() {
        let geometry_count = doc.nodes.iter()
            .filter(|n| matches!(n.payload,
                XrdsSceneNodePayload::Cube(_)     | XrdsSceneNodePayload::Sphere(_)   |
                XrdsSceneNodePayload::Cylinder(_) | XrdsSceneNodePayload::Plane3D(_)  |
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
