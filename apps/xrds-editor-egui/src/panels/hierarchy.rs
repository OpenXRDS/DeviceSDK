use std::sync::Arc;
use xrds::editor::egui;
use xrds::scene_graph::{XrdsSceneDocument, XrdsSceneNodeId, XrdsSceneNodePayload};

use crate::panels::palette::{apply_palette_drop, PaletteDragPayload};
use crate::panels::toolbar::{delete_node, duplicate_node};
use crate::state::{EditorSession, EditorState};

enum HierarchyAction {
    Rename(XrdsSceneNodeId),
    Duplicate(XrdsSceneNodeId),
    Delete(XrdsSceneNodeId),
}

pub fn hierarchy_panel(
    ctx: &mut egui::Context,
    session: &mut EditorSession,
    editor_state: &mut EditorState,
) {
    let mut pending_drop: Option<(Option<XrdsSceneNodeId>, Arc<PaletteDragPayload>)> = None;
    let mut pending_rename: Option<(XrdsSceneNodeId, String)> = None;
    let mut pending_action: Option<HierarchyAction> = None;

    let panel_resp = egui::SidePanel::left("hierarchy")
        .resizable(true)
        .default_width(200.0)
        .min_width(140.0)
        .show(ctx, |ui| {
            ui.label(egui::RichText::new("Hierarchy").strong());
            ui.separator();

            egui::ScrollArea::vertical().show(ui, |ui| {
                let doc = session.document();

                let mut roots: Vec<XrdsSceneNodeId> = doc
                    .nodes
                    .iter()
                    .filter(|n| n.parent_id.is_none())
                    .map(|n| n.id)
                    .collect();
                roots.sort_by_key(|id| id.0);

                for root_id in roots {
                    show_node_recursive(
                        ui,
                        root_id,
                        doc,
                        editor_state,
                        &mut pending_drop,
                        &mut pending_rename,
                        &mut pending_action,
                    );
                }

                // ── Inline root drop zone (visible hint while dragging) ────────
                let is_dragging =
                    egui::DragAndDrop::payload::<PaletteDragPayload>(ui.ctx()).is_some();
                if is_dragging {
                    let resp = ui
                        .add(
                            egui::Button::new(
                                egui::RichText::new("Drop here to add at root level")
                                    .weak()
                                    .small(),
                            )
                            .frame(false)
                            .min_size(egui::vec2(ui.available_width(), 24.0)),
                        )
                        .highlight();

                    if resp.dnd_hover_payload::<PaletteDragPayload>().is_some() {
                        ui.painter().rect_stroke(
                            resp.rect,
                            4.0,
                            egui::Stroke::new(1.5, egui::Color32::from_rgb(100, 160, 240)),
                            egui::StrokeKind::Inside,
                        );
                    }
                    if let Some(payload) = resp.dnd_release_payload::<PaletteDragPayload>() {
                        pending_drop = Some((None, payload));
                    }
                }
            });
        });

    // Fallback: any drop on the panel background that missed all node rows → root.
    if pending_drop.is_none() {
        if let Some(payload) = panel_resp
            .response
            .dnd_release_payload::<PaletteDragPayload>()
        {
            pending_drop = Some((None, payload));
        }
    }

    if let Some((node_id, new_name)) = pending_rename {
        let _ = session.session.edit(|doc| {
            if let Some(n) = doc.node_mut(node_id) {
                n.name = new_name.clone();
            }
        });
    }

    match pending_action {
        Some(HierarchyAction::Rename(id)) => {
            let name = session
                .document()
                .node(id)
                .map(|n| n.name.clone())
                .unwrap_or_default();
            editor_state.selection.set_single(id);
            editor_state.renaming_id = Some(id);
            editor_state.editing_name = Some((id, name));
        }
        Some(HierarchyAction::Duplicate(id)) => duplicate_node(session, editor_state, id),
        Some(HierarchyAction::Delete(id)) => delete_node(session, editor_state, id),
        None => {}
    }

    if let Some((parent_id, payload)) = pending_drop {
        apply_palette_drop(session, editor_state, parent_id, payload);
    }
}

fn show_node_recursive(
    ui: &mut egui::Ui,
    node_id: XrdsSceneNodeId,
    doc: &XrdsSceneDocument,
    editor_state: &mut EditorState,
    pending_drop: &mut Option<(Option<XrdsSceneNodeId>, Arc<PaletteDragPayload>)>,
    pending_rename: &mut Option<(XrdsSceneNodeId, String)>,
    pending_action: &mut Option<HierarchyAction>,
) {
    let Some(node) = doc.node(node_id) else {
        return;
    };

    let icon = node_icon(&node.payload);
    let label = format!("{icon}  {}", node.name);
    let node_name = node.name.clone();

    let children: Vec<XrdsSceneNodeId> = doc
        .nodes
        .iter()
        .filter(|n| n.parent_id == Some(node_id))
        .map(|n| n.id)
        .collect();

    let selected = editor_state.selection.contains(node_id);
    let renaming = editor_state.renaming_id == Some(node_id);

    // Shared post-render logic: drop zone + hover highlight.
    let handle_row =
        |resp: &egui::Response,
         pending_drop: &mut Option<(Option<XrdsSceneNodeId>, Arc<PaletteDragPayload>)>| {
            if resp.dnd_hover_payload::<PaletteDragPayload>().is_some() {
                resp.ctx.request_repaint();
                let painter = resp.ctx.layer_painter(egui::LayerId::new(
                    egui::Order::Foreground,
                    egui::Id::new("hier_drop_highlight"),
                ));
                painter.rect_stroke(
                    resp.rect,
                    2.0,
                    egui::Stroke::new(1.5, egui::Color32::from_rgb(100, 160, 240)),
                    egui::StrokeKind::Inside,
                );
            }
            if let Some(payload) = resp.dnd_release_payload::<PaletteDragPayload>() {
                *pending_drop = Some((Some(node_id), payload));
            }
        };

    if children.is_empty() {
        if renaming {
            show_rename_field(ui, node_id, &node_name, editor_state, pending_rename);
        } else {
            let response = ui.selectable_label(selected, &label);
            if response.clicked() {
                apply_hierarchy_click(ui, editor_state, node_id);
            }
            if response.double_clicked() {
                editor_state.selection.set_single(node_id);
                editor_state.renaming_id = Some(node_id);
                editor_state.editing_name = Some((node_id, node_name.clone()));
            }
            if response.hovered() {
                editor_state.hovered_id = Some(node_id);
            }
            handle_row(&response, pending_drop);
            node_context_menu(&response, node_id, pending_action);
        }
    } else {
        let id = ui.make_persistent_id(node_id.0);
        egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, true)
            .show_header(ui, |ui| {
                if renaming {
                    show_rename_field(ui, node_id, &node_name, editor_state, pending_rename);
                } else {
                    let response = ui.selectable_label(selected, &label);
                    if response.clicked() {
                        apply_hierarchy_click(ui, editor_state, node_id);
                    }
                    if response.double_clicked() {
                        editor_state.selection.set_single(node_id);
                        editor_state.renaming_id = Some(node_id);
                        editor_state.editing_name = Some((node_id, node_name.clone()));
                    }
                    if response.hovered() {
                        editor_state.hovered_id = Some(node_id);
                    }
                    handle_row(&response, pending_drop);
                    node_context_menu(&response, node_id, pending_action);
                }
            })
            .body(|ui| {
                ui.indent(node_id.0, |ui| {
                    let mut sorted = children;
                    sorted.sort_by_key(|id| id.0);
                    for child_id in sorted {
                        show_node_recursive(
                            ui,
                            child_id,
                            doc,
                            editor_state,
                            pending_drop,
                            pending_rename,
                            pending_action,
                        );
                    }
                });
            });
    }
}

fn apply_hierarchy_click(ui: &egui::Ui, state: &mut EditorState, node_id: XrdsSceneNodeId) {
    let shift = ui.ctx().input(|i| i.modifiers.shift);
    let ctrl = ui.ctx().input(|i| i.modifiers.command); // Ctrl on Win/Linux
    if ctrl {
        state.selection.toggle(node_id);
    } else if shift {
        state.selection.add(node_id);
    } else {
        state.selection.set_single(node_id);
    }
}

fn node_context_menu(
    response: &egui::Response,
    node_id: XrdsSceneNodeId,
    pending_action: &mut Option<HierarchyAction>,
) {
    response.context_menu(|ui| {
        if ui.button("Rename").clicked() {
            *pending_action = Some(HierarchyAction::Rename(node_id));
            ui.close();
        }
        if ui.button("Duplicate").clicked() {
            *pending_action = Some(HierarchyAction::Duplicate(node_id));
            ui.close();
        }
        ui.separator();
        if ui.button("Delete").clicked() {
            *pending_action = Some(HierarchyAction::Delete(node_id));
            ui.close();
        }
    });
}

/// Render the in-place TextEdit for renaming a node in the hierarchy.
fn show_rename_field(
    ui: &mut egui::Ui,
    node_id: XrdsSceneNodeId,
    current_name: &str,
    editor_state: &mut EditorState,
    pending_rename: &mut Option<(XrdsSceneNodeId, String)>,
) {
    // Initialise buffer if we just entered rename mode.
    if editor_state
        .editing_name
        .as_ref()
        .map_or(true, |(id, _)| *id != node_id)
    {
        editor_state.editing_name = Some((node_id, current_name.to_string()));
    }

    let mut buf = editor_state
        .editing_name
        .as_ref()
        .map(|(_, s)| s.clone())
        .unwrap_or_else(|| current_name.to_string());

    let te = ui.add(
        egui::TextEdit::singleline(&mut buf)
            .id(egui::Id::new(("hier_rename", node_id.0)))
            .desired_width(ui.available_width()),
    );
    te.request_focus();

    editor_state.editing_name = Some((node_id, buf.clone()));

    let escape = ui.ctx().input(|i| i.key_pressed(egui::Key::Escape));
    let commit = te.lost_focus() || ui.ctx().input(|i| i.key_pressed(egui::Key::Enter)) || escape;

    if commit {
        if !escape && buf != current_name {
            *pending_rename = Some((node_id, buf));
        }
        editor_state.renaming_id = None;
    }
}

fn node_icon(payload: &XrdsSceneNodePayload) -> &'static str {
    match payload {
        XrdsSceneNodePayload::Empty => "📁",
        XrdsSceneNodePayload::Camera(_) => "📷",
        XrdsSceneNodePayload::GltfAsset(_) => "🗂",
        XrdsSceneNodePayload::Cube(_) => "⬜",
        XrdsSceneNodePayload::Sphere(_) => "⚪",
        XrdsSceneNodePayload::Cylinder(_) => "🥫",
        XrdsSceneNodePayload::Plane3D(_) => "▭",
        XrdsSceneNodePayload::Tetrahedron(_) => "🔺",
        XrdsSceneNodePayload::AmbientLight(_) => "☀",
        XrdsSceneNodePayload::DirectionalLight(_) => "🌞",
        XrdsSceneNodePayload::PointLight(_) => "💡",
        XrdsSceneNodePayload::SpotLight(_) => "🔦",
        XrdsSceneNodePayload::AudioClip(_) => "🔊",
        XrdsSceneNodePayload::InteractionZone(_) => "⬡",
        XrdsSceneNodePayload::PlayerSpawn(_) => "🧍",
        XrdsSceneNodePayload::HudText(_) => "T",
        XrdsSceneNodePayload::Text(_) => "T³",
        XrdsSceneNodePayload::ExtrudedText(_) => "E³",
    }
}
