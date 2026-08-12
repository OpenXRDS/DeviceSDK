use std::collections::{HashMap, HashSet};
use xrds_scene_graph::{XrdsSceneDocument, XrdsSceneNodeId, XrdsSceneNodePayload, XrdsSceneCameraProjection};
use crate::bridge::HierarchyNode;
use crate::editor_state::{EditorSession, EditorState};
use crate::bridge::EditorCommand;

// ---------------------------------------------------------------------------
// Hierarchy serializer
// ---------------------------------------------------------------------------

pub fn build_hierarchy(doc: &XrdsSceneDocument) -> Vec<HierarchyNode> {
    build_children(doc, None)
}

fn build_children(doc: &XrdsSceneDocument, parent_id: Option<XrdsSceneNodeId>) -> Vec<HierarchyNode> {
    let mut children: Vec<_> = doc.nodes.iter()
        .filter(|n| n.parent_id == parent_id)
        .collect();
    children.sort_by_key(|n| n.id);

    children.iter().map(|n| HierarchyNode {
        id: n.id.0,
        name: n.name.clone(),
        kind: payload_kind(&n.payload).to_owned(),
        visible: n.visible,
        children: build_children(doc, Some(n.id)),
    }).collect()
}

pub fn payload_kind_str(payload: &XrdsSceneNodePayload) -> &'static str {
    payload_kind(payload)
}

fn payload_kind(payload: &XrdsSceneNodePayload) -> &'static str {
    match payload {
        XrdsSceneNodePayload::Empty           => "Empty",
        XrdsSceneNodePayload::Cube(_)         => "Cube",
        XrdsSceneNodePayload::Sphere(_)       => "Sphere",
        XrdsSceneNodePayload::Cylinder(_)     => "Cylinder",
        XrdsSceneNodePayload::Capsule(_)      => "Capsule",
        // Split by emission kind, unlike every other arm here, which maps one
        // payload variant to one string. Burst and Trail share a payload (kind is
        // a field, not a variant), but a hierarchy full of identically-iconed
        // "Effect" rows tells the author nothing about which is which. This field
        // is display-only -- the icon in Hierarchy.tsx and the kind badge beside
        // the name -- so distinguishing them here is free. Note the Inspector's
        // own payload_kind_name still reports "Effect", since that one describes
        // the payload type.
        XrdsSceneNodePayload::Effect(e) => match e.kind {
            xrds_scene_graph::XrdsSceneEffectKind::Burst => "EffectBurst",
            xrds_scene_graph::XrdsSceneEffectKind::Trail => "EffectTrail",
        },
        XrdsSceneNodePayload::Plane3D(_)      => "Plane",
        XrdsSceneNodePayload::Tetrahedron(_)  => "Tetrahedron",
        XrdsSceneNodePayload::Camera(_)       => "Camera",
        XrdsSceneNodePayload::DirectionalLight(_) => "DirectionalLight",
        XrdsSceneNodePayload::PointLight(_)   => "PointLight",
        XrdsSceneNodePayload::SpotLight(_)    => "SpotLight",
        XrdsSceneNodePayload::AmbientLight(_) => "AmbientLight",
        XrdsSceneNodePayload::GltfAsset(_)    => "GltfAsset",
        XrdsSceneNodePayload::Text(_)         => "Text",
        XrdsSceneNodePayload::ExtrudedText(_) => "ExtrudedText",
        XrdsSceneNodePayload::HudText(_)      => "HudText",
        XrdsSceneNodePayload::AudioClip(_)    => "AudioClip",
        XrdsSceneNodePayload::InteractionZone(_) => "InteractionZone",
        XrdsSceneNodePayload::PlayerSpawn(_)     => "PlayerSpawn",
        XrdsSceneNodePayload::PlayerSpawnZone(_) => "PlayerSpawnZone",
        XrdsSceneNodePayload::Player(_)          => "Player",
        XrdsSceneNodePayload::PlayerAnchor(_)    => "PlayerAnchor",
        XrdsSceneNodePayload::Panel(_)           => "Panel",
    }
}

// ---------------------------------------------------------------------------
// Command handlers
// ---------------------------------------------------------------------------

/// Apply a hierarchy-related EditorCommand. Returns true if a full reimport
/// of the scene document is needed after this command.
pub fn apply_hierarchy_command(
    cmd: &EditorCommand,
    session: &mut EditorSession,
    state: &mut EditorState,
) -> bool {
    match cmd {
        EditorCommand::SelectNode { id } => {
            state.selection.set_single(XrdsSceneNodeId(*id));
            false
        }

        EditorCommand::MultiSelectNode { id, extend } => {
            if *extend { state.selection.toggle(XrdsSceneNodeId(*id)); }
            else       { state.selection.set_single(XrdsSceneNodeId(*id)); }
            false
        }

        EditorCommand::DeselectAll => {
            state.selection.clear();
            false
        }

        EditorCommand::RenameNode { id, name } => {
            let id = XrdsSceneNodeId(*id);
            let name = name.clone();
            let _ = session.0.edit(|doc| {
                if let Some(node) = doc.node_mut(id) {
                    node.name = name;
                }
            });
            false // name change doesn't need reimport (runtime uses XrdsIdIndex, not name)
        }

        EditorCommand::DeleteNode { id } => {
            let id = XrdsSceneNodeId(*id);
            delete_node(session, id);
            if state.selection.contains(id) { state.selection.toggle(id); }
            true
        }

        EditorCommand::DeleteSelection => {
            let ids: Vec<XrdsSceneNodeId> = state.selection.ids().to_vec();
            if ids.is_empty() { return false; }
            // Delete in one edit so validation runs once on the final state.
            match session.0.edit(|doc| {
                // Skip nodes whose ancestor is also selected (parent deletion covers them).
                let selected_set: std::collections::HashSet<XrdsSceneNodeId> =
                    ids.iter().cloned().collect();
                let roots: Vec<XrdsSceneNodeId> = ids.iter().cloned()
                    .filter(|&id| {
                        // Keep if no ancestor is also in the selection set.
                        let mut cur = doc.node(id).and_then(|n| n.parent_id);
                        while let Some(p) = cur {
                            if selected_set.contains(&p) { return false; }
                            cur = doc.node(p).and_then(|n| n.parent_id);
                        }
                        true
                    })
                    .collect();
                let mut to_remove = Vec::new();
                for root_id in roots {
                    to_remove.extend(collect_subtree(doc, root_id));
                }
                doc.nodes.retain(|n| !to_remove.contains(&n.id));
                for removed_id in &to_remove {
                    doc.gltf_node_authoring.remove(&removed_id.0);
                }
            }) {
                Ok(_) => {}
                Err(e) => bevy::log::error!("[hierarchy] DeleteSelection failed: {:?}", e),
            }
            state.selection.clear();
            state.clear_pending_translations();
            true
        }

        EditorCommand::DuplicateNode { id } => {
            let id = XrdsSceneNodeId(*id);
            let _ = session.0.edit(|doc| {
                let subtree = collect_subtree(doc, id);
                let max_id = doc.nodes.iter().map(|n| n.id.0).max().unwrap_or(0);
                let id_map: HashMap<XrdsSceneNodeId, XrdsSceneNodeId> = subtree
                    .iter()
                    .enumerate()
                    .map(|(i, &old)| (old, XrdsSceneNodeId(max_id + 1 + i as u64)))
                    .collect();

                // Find max camera order already in the document so the duplicate
                // gets a distinct order and won't trigger Bevy's render-ambiguity warning.
                let max_camera_order: isize = doc.nodes.iter()
                    .filter_map(|n| match &n.payload {
                        XrdsSceneNodePayload::Camera(c) => Some(match c.projection {
                            XrdsSceneCameraProjection::Perspective { order, .. } => order,
                            XrdsSceneCameraProjection::Orthographic { order, .. } => order,
                        }),
                        _ => None,
                    })
                    .max()
                    .unwrap_or(0);

                let new_nodes: Vec<_> = subtree.iter().filter_map(|&old_id| {
                    doc.node(old_id).map(|n| {
                        let mut new_node = n.clone();
                        new_node.id = id_map[&old_id];
                        new_node.parent_id = n.parent_id.map(|p| {
                            id_map.get(&p).copied().unwrap_or(p)
                        });
                        // Give duplicate cameras a unique order.
                        if old_id == id {
                            if let XrdsSceneNodePayload::Camera(ref mut cam) = new_node.payload {
                                match &mut cam.projection {
                                    XrdsSceneCameraProjection::Perspective { order, .. } => {
                                        *order = max_camera_order + 1;
                                    }
                                    XrdsSceneCameraProjection::Orthographic { order, .. } => {
                                        *order = max_camera_order + 1;
                                    }
                                }
                            }
                        }
                        new_node
                    })
                }).collect();

                doc.nodes.extend(new_nodes);
                // Duplicate: don't copy gltf_node_authoring — the new node is a fresh instance.
                // (Authoring entries are per-node and should start clean for the duplicate.)
            });
            true
        }

        EditorCommand::ReparentNode { id, new_parent_id, .. } => {
            let id = XrdsSceneNodeId(*id);
            let new_parent = new_parent_id.map(XrdsSceneNodeId);
            let _ = session.0.edit(|doc| {
                if let Some(node) = doc.node_mut(id) {
                    node.parent_id = new_parent;
                }
            });
            true
        }

        // ── Clipboard ────────────────────────────────────────────────────────
        EditorCommand::CopySelection => {
            let ids: Vec<XrdsSceneNodeId> = state.selection.ids().to_vec();
            if ids.is_empty() { return false; }
            let doc = session.0.document();
            let id_set: HashSet<XrdsSceneNodeId> = ids.iter().cloned().collect();

            // Only collect roots (skip nodes whose ancestor is already selected)
            let roots: Vec<_> = ids.iter().cloned()
                .filter(|&id| {
                    let mut cur = doc.node(id).and_then(|n| n.parent_id);
                    while let Some(p) = cur {
                        if id_set.contains(&p) { return false; }
                        cur = doc.node(p).and_then(|n| n.parent_id);
                    }
                    true
                })
                .collect();

            let mut nodes = Vec::new();
            for root in roots {
                for id in collect_subtree(doc, root) {
                    if let Some(n) = doc.node(id) { nodes.push(n.clone()); }
                }
            }
            state.clipboard = Some(nodes);
            false
        }

        EditorCommand::CutSelection => {
            // Copy first, then delete
            let ids: Vec<XrdsSceneNodeId> = state.selection.ids().to_vec();
            if ids.is_empty() { return false; }
            let doc = session.0.document();
            let id_set: HashSet<XrdsSceneNodeId> = ids.iter().cloned().collect();
            let roots: Vec<_> = ids.iter().cloned()
                .filter(|&id| {
                    let mut cur = doc.node(id).and_then(|n| n.parent_id);
                    while let Some(p) = cur { if id_set.contains(&p) { return false; } cur = doc.node(p).and_then(|n| n.parent_id); }
                    true
                })
                .collect();
            let mut nodes = Vec::new();
            for root in &roots {
                for id in collect_subtree(doc, *root) {
                    if let Some(n) = doc.node(id) { nodes.push(n.clone()); }
                }
            }
            state.clipboard = Some(nodes);
            // Delete the original nodes
            for root in roots { delete_node(session, root); }
            state.selection.clear();
            state.clear_pending_translations();
            true
        }

        EditorCommand::PasteClipboard => {
            let Some(clipboard) = state.clipboard.clone() else { return false; };
            if clipboard.is_empty() { return false; }

            let target_parent = state.selection.primary();
            let clip_id_set: HashSet<XrdsSceneNodeId> = clipboard.iter().map(|n| n.id).collect();

            // Nodes whose parent is not in the clipboard = roots of pasted subtrees
            let root_ids: HashSet<XrdsSceneNodeId> = clipboard.iter()
                .filter(|n| n.parent_id.map(|p| !clip_id_set.contains(&p)).unwrap_or(true))
                .map(|n| n.id)
                .collect();

            match session.0.edit(|doc| {
                let max_id = doc.nodes.iter().map(|n| n.id.0).max().unwrap_or(0);
                let id_map: HashMap<XrdsSceneNodeId, XrdsSceneNodeId> = clipboard.iter()
                    .enumerate()
                    .map(|(i, n)| (n.id, XrdsSceneNodeId(max_id + 1 + i as u64)))
                    .collect();

                for node in &clipboard {
                    let mut new_node = node.clone();
                    new_node.id = id_map[&node.id];
                    new_node.parent_id = if root_ids.contains(&node.id) {
                        target_parent // attach roots to selected node (or scene root)
                    } else {
                        node.parent_id.and_then(|p| id_map.get(&p).copied())
                    };
                    doc.nodes.push(new_node);
                }
            }) {
                Ok(_) => {}
                Err(e) => bevy::log::error!("[hierarchy] PasteClipboard failed: {:?}", e),
            }
            true
        }

        _ => false,
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn delete_node(session: &mut crate::editor_state::EditorSession, id: XrdsSceneNodeId) {
    match session.0.edit(|doc| {
        let to_remove = collect_subtree(doc, id);
        doc.nodes.retain(|n| !to_remove.contains(&n.id));
        for removed_id in &to_remove {
            doc.gltf_node_authoring.remove(&removed_id.0);
        }
    }) {
        Ok(_) => {}
        Err(e) => bevy::log::error!("[hierarchy] DeleteNode {:?} failed: {:?}", id, e),
    }
}

/// Collect all node IDs in the subtree rooted at `root_id` (BFS).
fn collect_subtree(doc: &XrdsSceneDocument, root_id: XrdsSceneNodeId) -> Vec<XrdsSceneNodeId> {
    let mut result = vec![root_id];
    let mut i = 0;
    while i < result.len() {
        let cur = result[i];
        for n in doc.nodes.iter().filter(|n| n.parent_id == Some(cur)) {
            result.push(n.id);
        }
        i += 1;
    }
    result
}
