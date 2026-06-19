use bevy::log::error;
use xrds_scene_graph::{
    HudItemDefId, HudTemplateId, XrdsHudItemDef, XrdsHudTemplate, XrdsSceneDocument,
    XrdsSceneNodeId, XrdsSceneNodePayload,
};
use crate::bridge::{EditorCommand, HudItemDefDto, HudTemplateDto};
use crate::editor_state::{EditorSession, EditorState};

// ---------------------------------------------------------------------------
// Snapshot serializer
// ---------------------------------------------------------------------------

pub fn build_hud_library_dto(doc: &XrdsSceneDocument) -> Vec<HudTemplateDto> {
    doc.hud_library.iter().map(|t| HudTemplateDto {
        id: t.id.0,
        name: t.name.clone(),
        depth: t.depth,
        items: t.items.iter().map(|item| HudItemDefDto {
            id: item.id.0,
            name: item.name.clone(),
            position: item.position,
            text: item.text.clone(),
            font_size: item.font_size,
            color: item.color,
        }).collect(),
    }).collect()
}

// ---------------------------------------------------------------------------
// Command handlers
// ---------------------------------------------------------------------------

/// Returns true if a full scene reimport is needed after the command.
pub fn apply_hud_library_command(
    cmd: &EditorCommand,
    session: &mut EditorSession,
    _state: &mut EditorState,
) -> bool {
    match cmd {
        EditorCommand::CreateHudTemplate { name } => {
            let name = name.clone();
            match session.0.edit(|doc| {
                let id = doc.next_available_template_id();
                doc.hud_library.push(XrdsHudTemplate {
                    id,
                    name,
                    depth: 0.5,
                    items: Vec::new(),
                });
            }) {
                Ok(_) => {}
                Err(e) => error!("[hud_library] CreateHudTemplate failed: {:?}", e),
            }
            false
        }

        EditorCommand::DeleteHudTemplate { id } => {
            let tid = HudTemplateId(*id);
            match session.0.edit(|doc| {
                doc.hud_library.retain(|t| t.id != tid);
                // Unlink any PlayerAnchor that referenced this template.
                for node in doc.nodes.iter_mut() {
                    if let XrdsSceneNodePayload::PlayerAnchor(ref mut a) = node.payload {
                        if a.hud_template_id == Some(tid) {
                            a.hud_template_id = None;
                        }
                    }
                }
            }) {
                Ok(_) => {}
                Err(e) => error!("[hud_library] DeleteHudTemplate failed: {:?}", e),
            }
            true // PlayerAnchor link may have changed — reimport
        }

        EditorCommand::RenameHudTemplate { id, name } => {
            let tid = HudTemplateId(*id);
            let name = name.clone();
            match session.0.edit(|doc| {
                if let Some(t) = doc.hud_template_mut(tid) {
                    t.name = name;
                }
            }) {
                Ok(_) => {}
                Err(e) => error!("[hud_library] RenameHudTemplate failed: {:?}", e),
            }
            false
        }

        EditorCommand::SetHudTemplateDepth { id, depth } => {
            let tid = HudTemplateId(*id);
            let depth = *depth;
            match session.0.edit(|doc| {
                if let Some(t) = doc.hud_template_mut(tid) {
                    t.depth = depth;
                }
            }) {
                Ok(_) => {}
                Err(e) => error!("[hud_library] SetHudTemplateDepth failed: {:?}", e),
            }
            true // depth change affects head-lock offset — reimport
        }

        EditorCommand::AddHudItem { template_id } => {
            let tid = HudTemplateId(*template_id);
            match session.0.edit(|doc| {
                if let Some(t) = doc.hud_template_mut(tid) {
                    let next_id = HudItemDefId(
                        t.items.iter().map(|i| i.id.0).max().unwrap_or(0).saturating_add(1),
                    );
                    t.items.push(XrdsHudItemDef {
                        id: next_id,
                        name: format!("item{}", next_id.0),
                        ..Default::default()
                    });
                }
            }) {
                Ok(_) => {}
                Err(e) => error!("[hud_library] AddHudItem failed: {:?}", e),
            }
            true
        }

        EditorCommand::RemoveHudItem { template_id, item_id } => {
            let tid = HudTemplateId(*template_id);
            let iid = HudItemDefId(*item_id);
            match session.0.edit(|doc| {
                if let Some(t) = doc.hud_template_mut(tid) {
                    t.items.retain(|i| i.id != iid);
                }
            }) {
                Ok(_) => {}
                Err(e) => error!("[hud_library] RemoveHudItem failed: {:?}", e),
            }
            true
        }

        EditorCommand::RenameHudItem { template_id, item_id, name } => {
            let tid = HudTemplateId(*template_id);
            let iid = HudItemDefId(*item_id);
            let name = name.clone();
            match session.0.edit(|doc| {
                if let Some(t) = doc.hud_template_mut(tid) {
                    if let Some(item) = t.items.iter_mut().find(|i| i.id == iid) {
                        item.name = name;
                    }
                }
            }) {
                Ok(_) => {}
                Err(e) => error!("[hud_library] RenameHudItem failed: {:?}", e),
            }
            false
        }

        EditorCommand::SetHudItemPosition { template_id, item_id, position } => {
            let tid = HudTemplateId(*template_id);
            let iid = HudItemDefId(*item_id);
            let position = *position;
            match session.0.edit(|doc| {
                if let Some(t) = doc.hud_template_mut(tid) {
                    if let Some(item) = t.items.iter_mut().find(|i| i.id == iid) {
                        item.position = position;
                    }
                }
            }) {
                Ok(_) => {}
                Err(e) => error!("[hud_library] SetHudItemPosition failed: {:?}", e),
            }
            true
        }

        EditorCommand::SetHudItemText { template_id, item_id, text } => {
            let tid = HudTemplateId(*template_id);
            let iid = HudItemDefId(*item_id);
            let text = text.clone();
            match session.0.edit(|doc| {
                if let Some(t) = doc.hud_template_mut(tid) {
                    if let Some(item) = t.items.iter_mut().find(|i| i.id == iid) {
                        item.text = text;
                    }
                }
            }) {
                Ok(_) => {}
                Err(e) => error!("[hud_library] SetHudItemText failed: {:?}", e),
            }
            true
        }

        EditorCommand::SetHudItemFontSize { template_id, item_id, font_size } => {
            let tid = HudTemplateId(*template_id);
            let iid = HudItemDefId(*item_id);
            let font_size = *font_size;
            match session.0.edit(|doc| {
                if let Some(t) = doc.hud_template_mut(tid) {
                    if let Some(item) = t.items.iter_mut().find(|i| i.id == iid) {
                        item.font_size = font_size;
                    }
                }
            }) {
                Ok(_) => {}
                Err(e) => error!("[hud_library] SetHudItemFontSize failed: {:?}", e),
            }
            true
        }

        EditorCommand::SetHudItemColor { template_id, item_id, color } => {
            let tid = HudTemplateId(*template_id);
            let iid = HudItemDefId(*item_id);
            let color = *color;
            match session.0.edit(|doc| {
                if let Some(t) = doc.hud_template_mut(tid) {
                    if let Some(item) = t.items.iter_mut().find(|i| i.id == iid) {
                        item.color = color;
                    }
                }
            }) {
                Ok(_) => {}
                Err(e) => error!("[hud_library] SetHudItemColor failed: {:?}", e),
            }
            true
        }

        EditorCommand::LinkHudTemplate { anchor_id, template_id } => {
            let anchor_id = XrdsSceneNodeId(*anchor_id);
            let tid = template_id.map(HudTemplateId);
            match session.0.edit(|doc| {
                if let Some(node) = doc.node_mut(anchor_id) {
                    if let XrdsSceneNodePayload::PlayerAnchor(ref mut a) = node.payload {
                        a.hud_template_id = tid;
                    }
                }
            }) {
                Ok(_) => {}
                Err(e) => error!("[hud_library] LinkHudTemplate failed: {:?}", e),
            }
            true
        }

        _ => false,
    }
}
