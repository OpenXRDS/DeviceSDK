/// Status bar — scene name, dirty flag, undo/redo controls, node count,
/// status messages.  File operations live in the menu bar; this bar handles
/// global keyboard shortcuts and always-visible runtime state.
use xrds::editor::egui;
use xrds::scene_graph::XrdsSceneNodeId;

use xrds::scene_graph::{XrdsSceneDocumentSession, XrdsSceneNode};

use crate::icon::IconName;
use crate::io::{export_glb, load, new_scene, save, save_as};
use crate::camera::EditorCameraState;
use crate::state::{CameraMode, EditorSession, EditorState, GizmoMode};

pub fn toolbar_panel(
    ctx: &mut egui::Context,
    session: &mut EditorSession,
    state: &mut EditorState,
    cam: &mut EditorCameraState,
) {
    let doc = session.document();
    let node_count = doc.nodes.len();
    let is_dirty = session.session.is_dirty();
    let can_undo = session.session.can_undo();
    let can_redo = session.session.can_redo();
    let undo_count = session.session.undo_count();
    let redo_count = session.session.redo_count();
    let history_limit = session.session.history_limit();
    let scene_name = {
        let n = &doc.metadata.name;
        if n.is_empty() {
            "Untitled".to_string()
        } else {
            n.clone()
        }
    };
    let save_path_label = session
        .session
        .save_path()
        .and_then(|p| p.file_name())
        .map(|f| f.to_string_lossy().into_owned());
    let _ = drop(doc);

    // ── Global keyboard shortcuts ─────────────────────────────────────────────
    if !ctx.wants_keyboard_input() {
        let ctrl = |key: egui::Key| ctx.input(|i| i.key_pressed(key) && i.modifiers.ctrl);
        let ctrl_shift = |key: egui::Key| {
            ctx.input(|i| i.key_pressed(key) && i.modifiers.ctrl && i.modifiers.shift)
        };

        if ctrl(egui::Key::N) {
            new_scene(session, state);
        } else if ctrl(egui::Key::O) {
            load(session, state);
        } else if ctrl_shift(egui::Key::S) {
            save_as(session, state);
        } else if ctrl(egui::Key::S) {
            save(session, state);
        } else if ctrl_shift(egui::Key::E) {
            export_glb(session, state);
        } else if ctrl(egui::Key::Z) && !ctx.input(|i| i.modifiers.shift) && can_undo {
            do_undo(session, state);
        } else if (ctrl(egui::Key::Y) || ctrl_shift(egui::Key::Z)) && can_redo {
            do_redo(session, state);
        } else if ctrl(egui::Key::C) {
            if !state.selection.is_empty() {
                let subtrees: Vec<_> = state.selection.ids().iter()
                    .map(|&id| collect_subtree(session, id))
                    .collect();
                let count = subtrees.len();
                state.clipboard = Some(subtrees);
                state.status_message = Some(format!("Copied {count} node(s)."));
            }
        } else if ctrl(egui::Key::V) {
            paste_clipboard(session, state);
        }

        // Delete selected nodes
        if ctx.input(|i| i.key_pressed(egui::Key::Delete)) {
            if !state.selection.is_empty() {
                delete_selection(session, state);
            }
        }

        // Duplicate (Ctrl+D)
        if ctrl(egui::Key::D) {
            let roots: Vec<_> = state.selection.ids().to_vec();
            for root_id in roots {
                duplicate_node(session, state, root_id);
            }
        }

        // Gizmo mode switch (T = Translate, R = Rotate, S = Scale — no modifier, no Ctrl)
        if ctx.input(|i| i.key_pressed(egui::Key::T) && !i.modifiers.ctrl) {
            state.gizmo_mode = GizmoMode::Translate;
            state.gizmo_drag = None;
        } else if ctx.input(|i| i.key_pressed(egui::Key::R) && !i.modifiers.ctrl) {
            state.gizmo_mode = GizmoMode::Rotate;
            state.gizmo_drag = None;
        } else if ctx.input(|i| i.key_pressed(egui::Key::Y) && !i.modifiers.ctrl) {
            state.gizmo_mode = GizmoMode::Scale;
            state.gizmo_drag = None;
        }

        // Play / Stop
        if ctx.input(|i| i.key_pressed(egui::Key::Space)) && !state.is_playing {
            start_play(session, state);
        } else if ctx.input(|i| i.key_pressed(egui::Key::Escape)) && state.is_playing {
            stop_play(session, state);
        }

        // Frame Selected (F key) — center orbit camera on primary node.
        if ctx.input(|i| i.key_pressed(egui::Key::F)) {
            if let Some(sel_id) = state.selection.primary() {
                let pos = session.document().node(sel_id).map(|n| n.transform.translation);
                if let Some(pos) = pos {
                    state.frame_selected_target = Some(pos);
                }
            }
        }

        // Arrow-key nudge in Translate mode — applies to all selected nodes.
        if state.gizmo_mode == GizmoMode::Translate && !state.selection.is_empty() {
            let (dx, dy, dz) = ctx.input(|i| {
                let step = if i.modifiers.shift { 1.0_f32 } else { 0.1_f32 };
                let mut dx = 0.0_f32;
                let mut dy = 0.0_f32;
                let mut dz = 0.0_f32;
                if i.key_pressed(egui::Key::ArrowRight) { dx += step; }
                if i.key_pressed(egui::Key::ArrowLeft)  { dx -= step; }
                if i.key_pressed(egui::Key::ArrowUp)    { dy += step; }
                if i.key_pressed(egui::Key::ArrowDown)  { dy -= step; }
                if i.key_pressed(egui::Key::PageUp)     { dz -= step; }
                if i.key_pressed(egui::Key::PageDown)   { dz += step; }
                (dx, dy, dz)
            });
            if dx != 0.0 || dy != 0.0 || dz != 0.0 {
                let ids: Vec<_> = state.selection.ids().to_vec();
                let _ = session.session.edit(|doc| {
                    for id in &ids {
                        if let Some(n) = doc.node_mut(*id) {
                            let [tx, ty, tz] = n.transform.translation;
                            n.transform.translation = [tx + dx, ty + dy, tz + dz];
                        }
                    }
                });
                state.needs_runtime_sync = true;
            }
        }

        // Panel visibility toggles (no Ctrl modifier, only when not typing)
        if ctx.input(|i| i.key_pressed(egui::Key::P)) {
            state.show_palette = !state.show_palette;
        }

        // Camera mode toggle [V]: Orbit ↔ Fly
        if ctx.input(|i| i.key_pressed(egui::Key::V) && !i.modifiers.ctrl) {
            toggle_camera_mode(state, cam);
        }
    }

    // ── Status bar ────────────────────────────────────────────────────────────
    egui::TopBottomPanel::top("statusbar")
        .exact_height(24.0)
        .show(ctx, |ui| {
            ui.horizontal_centered(|ui| {
                // Scene identity
                let file = save_path_label.as_deref().unwrap_or("unsaved");
                ui.strong(format!("{scene_name}"));
                ui.colored_label(
                    egui::Color32::GRAY,
                    format!("[{file}]{}", if is_dirty { " •" } else { "" }),
                );

                ui.separator();

                // Undo / redo
                if ui
                    .add_enabled(can_undo, egui::Button::new(format!("Undo ({})", undo_count)))
                    .on_hover_text(format!("Undo  Ctrl+Z  ({undo_count}/{history_limit})"))
                    .on_disabled_hover_text("Nothing to undo")
                    .clicked()
                {
                    do_undo(session, state);
                }
                if ui
                    .add_enabled(can_redo, egui::Button::new(format!("Redo ({})", redo_count)))
                    .on_hover_text("Redo  Ctrl+Y  /  Ctrl+Shift+Z")
                    .on_disabled_hover_text("Nothing to redo")
                    .clicked()
                {
                    do_redo(session, state);
                }

                ui.separator();

                // Gizmo mode toggle buttons
                let is_translate = state.gizmo_mode == GizmoMode::Translate;
                let is_rotate    = state.gizmo_mode == GizmoMode::Rotate;
                let is_scale     = state.gizmo_mode == GizmoMode::Scale;
                let gizmo_icons = [
                    (is_translate, IconName::Undo, "Move [T]", "Translate gizmo  [T]\nArrow keys: nudge 0.1 m  (Shift = 1 m)"),
                    (is_rotate, IconName::Redo, "Rotate [R]", "Rotate gizmo  [R]\nDrag a ring to rotate around that axis"),
                    (is_scale, IconName::Build, "Scale [Y]", "Scale gizmo  [Y]\nDrag an axis handle to scale on that axis"),
                ];
                for (selected, icon, text, hover) in gizmo_icons {
                    let mode = match icon {
                        IconName::Undo => GizmoMode::Translate,
                        IconName::Redo => GizmoMode::Rotate,
                        IconName::Build => GizmoMode::Scale,
                        _ => state.gizmo_mode,
                    };
                    let tex = state.icon_cache.load_tinted_at(ui.ctx(), icon, 18, 8, egui::Color32::WHITE, 128);
                    let sized = egui::load::SizedTexture::new(tex.id(), egui::vec2(18.0, 18.0));
                    let img = egui::Image::new(sized).max_size(egui::vec2(18.0, 18.0));
                    let btn = egui::Button::new(img).min_size(egui::vec2(0.0, 0.0));
                    if ui
                        .add(btn.selected(selected))
                        .on_hover_text(hover)
                        .clicked()
                    {
                        state.gizmo_mode = mode;
                        state.gizmo_drag = None;
                    }
                }

                ui.separator();

                // Camera mode: Orbit / Fly [V]
                let is_orbit = state.camera_mode == CameraMode::Orbit;
                let is_fly   = state.camera_mode == CameraMode::Fly;
                let camera_icons = [
                    (is_orbit, IconName::Perspective, "Orbit", "Orbit mode — MMB drag to orbit, Shift+MMB to pan, scroll to zoom  [V]"),
                    (is_fly, IconName::Wireframe, "Fly [V]", "Fly mode — RMB + mouse to look, WASD/Q/E to move, Shift for fast  [V]"),
                ];
                for (selected, icon, text, hover) in camera_icons {
                    let tex = state.icon_cache.load_tinted_at(ui.ctx(), icon, 18, 8, egui::Color32::WHITE, 128);
                    let sized = egui::load::SizedTexture::new(tex.id(), egui::vec2(18.0, 18.0));
                    let img = egui::Image::new(sized).max_size(egui::vec2(18.0, 18.0));
                    let btn = egui::Button::new(img).min_size(egui::vec2(0.0, 0.0));
                    if ui
                        .add(btn.selected(selected))
                        .on_hover_text(hover)
                        .clicked()
                    {
                        toggle_camera_mode(state, cam);
                    }
                }

                ui.separator();

                // Play / Stop
                if state.is_playing {
                    let tex = state.icon_cache.load_tinted_at(ui.ctx(), IconName::Stop, 18, 8, egui::Color32::WHITE, 128);
                    let sized = egui::load::SizedTexture::new(tex.id(), egui::vec2(18.0, 18.0));
                    let img = egui::Image::new(sized).max_size(egui::vec2(18.0, 18.0));
                    let btn = egui::Button::new(img).min_size(egui::vec2(0.0, 0.0));
                    if ui
                        .add(btn.fill(egui::Color32::from_rgb(180, 60, 60)))
                        .on_hover_text("Stop and restore scene  [Esc]")
                        .clicked()
                    {
                        stop_play(session, state);
                    }
                } else {
                    let tex = state.icon_cache.load_tinted_at(ui.ctx(), IconName::Play, 18, 8, egui::Color32::WHITE, 128);
                    let sized = egui::load::SizedTexture::new(tex.id(), egui::vec2(18.0, 18.0));
                    let img = egui::Image::new(sized).max_size(egui::vec2(18.0, 18.0));
                    let btn = egui::Button::new(img).min_size(egui::vec2(0.0, 0.0));
                    if ui
                        .add(btn)
                        .on_hover_text("Snapshot scene and preview  [Space]")
                        .clicked()
                    {
                        start_play(session, state);
                    }
                }

                ui.separator();

                // Snap step cycle (Ctrl+drag snaps to this step)
                let snap_label = format!("Snap {:.2}m", state.snap_step);
                if ui
                    .button(snap_label)
                    .on_hover_text("Ctrl+drag to snap translate gizmo.\nClick to cycle step: 0.1 / 0.25 / 1.0 m")
                    .clicked()
                {
                    state.snap_step = if state.snap_step < 0.15 { 0.25 }
                        else if state.snap_step < 0.5 { 1.0 }
                        else { 0.1 };
                }

                ui.separator();

                if ui
                    .add(egui::Button::new("Grid").selected(state.show_grid))
                    .on_hover_text("Toggle floor grid (XZ plane, ±10 m)")
                    .clicked()
                {
                    state.show_grid = !state.show_grid;
                }
                if ui
                    .add(egui::Button::new("Rays: All").selected(state.light_rays_all))
                    .on_hover_text("Debug: show light shapes for all light nodes\nPoint: 6 rays + range sphere  •  Spot: cone outline  •  Dir: parallel arrows")
                    .clicked()
                {
                    state.light_rays_all = !state.light_rays_all;
                }
                if ui
                    .add(egui::Button::new("Rays: Sel").selected(state.light_rays_selected))
                    .on_hover_text("Debug: show light shape for the selected light node only")
                    .clicked()
                {
                    state.light_rays_selected = !state.light_rays_selected;
                }
                if ui
                    .add(egui::Button::new("Stats").selected(state.show_perf_stats))
                    .on_hover_text("Toggle performance stats overlay (FPS, vertices, texture memory)")
                    .clicked()
                {
                    state.show_perf_stats = !state.show_perf_stats;
                }

                ui.separator();

                ui.label(format!("{node_count} node(s)"));

                // Transient status message (e.g. "Saved", "Exported 42 KB")
                if let Some(ref msg) = state.status_message {
                    ui.separator();
                    ui.colored_label(egui::Color32::from_rgb(100, 210, 120), msg.as_str());
                }
            });
        });
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn do_undo(session: &mut EditorSession, state: &mut EditorState) {
    if session.session.undo() {
        on_history_change(state);
    }
}

fn do_redo(session: &mut EditorSession, state: &mut EditorState) {
    if session.session.redo() {
        on_history_change(state);
    }
}

fn on_history_change(state: &mut EditorState) {
    state.clear_pending_translations();
    state.clear_pending_rotations();
    state.pending_scale = None;
    state.pending_material = None;
    state.pending_visible = None;
    state.pending_point_light = None;
    state.pending_directional_light = None;
    state.pending_spot_light = None;
    state.pending_ambient_light = None;
    state.editing_name = None;
    state.renaming_id = None;
    state.editing_scene_name = None;
    state.editing_scene_author = None;
    state.needs_runtime_sync = true;
}

/// Delete a single node by ID (used by hierarchy context menu).
pub(crate) fn delete_node(session: &mut EditorSession, state: &mut EditorState, root_id: XrdsSceneNodeId) {
    let _ = session.session.edit(|doc| {
        let mut to_remove = vec![root_id];
        let mut i = 0;
        while i < to_remove.len() {
            let parent = to_remove[i];
            for n in &doc.nodes {
                if n.parent_id == Some(parent) { to_remove.push(n.id); }
            }
            i += 1;
        }
        doc.nodes.retain(|n| !to_remove.contains(&n.id));
    });
    state.selection.clear();
    state.editing_name = None;
    state.renaming_id = None;
    state.needs_full_reimport = true;
}

/// Delete all currently selected nodes and their descendants.
pub(crate) fn delete_selection(session: &mut EditorSession, state: &mut EditorState) {
    let roots: Vec<_> = state.selection.ids().to_vec();
    let _ = session.session.edit(|doc| {
        let mut to_remove = roots.clone();
        let mut i = 0;
        while i < to_remove.len() {
            let parent = to_remove[i];
            for n in &doc.nodes {
                if n.parent_id == Some(parent) && !to_remove.contains(&n.id) {
                    to_remove.push(n.id);
                }
            }
            i += 1;
        }
        doc.nodes.retain(|n| !to_remove.contains(&n.id));
    });
    state.selection.clear();
    state.editing_name = None;
    state.renaming_id = None;
    state.needs_full_reimport = true;
}

pub(crate) fn duplicate_node(session: &mut EditorSession, state: &mut EditorState, root_id: XrdsSceneNodeId) {
    let mut new_root_id = None;
    let _ = session.session.edit(|doc| {
        // BFS-collect the subtree.
        let mut subtree_ids = vec![root_id];
        let mut i = 0;
        while i < subtree_ids.len() {
            let parent = subtree_ids[i];
            for n in &doc.nodes {
                if n.parent_id == Some(parent) {
                    subtree_ids.push(n.id);
                }
            }
            i += 1;
        }

        // Clone the nodes.
        let subtree: Vec<_> = doc.nodes.iter()
            .filter(|n| subtree_ids.contains(&n.id))
            .cloned()
            .collect();

        // Allocate fresh IDs.
        let base = doc.nodes.iter().map(|n| n.id.0).max().unwrap_or(0) + 1;
        let id_map: std::collections::HashMap<XrdsSceneNodeId, XrdsSceneNodeId> = subtree.iter()
            .enumerate()
            .map(|(i, n)| (n.id, XrdsSceneNodeId(base + i as u64)))
            .collect();

        for mut node in subtree {
            let old_id = node.id;
            node.id = id_map[&old_id];
            if old_id == root_id {
                // Root node: keep the same parent so clone is a sibling.
                // parent_id is already correct (original root's parent).
            } else {
                node.parent_id = node.parent_id.map(|p| id_map.get(&p).copied().unwrap_or(p));
            }
            if old_id == root_id {
                new_root_id = Some(node.id);
            }
            doc.nodes.push(node);
        }
    });

    if let Some(new_id) = new_root_id {
        state.selection.set_single(new_id);
        state.editing_name = None;
        state.needs_full_reimport = true;
    }
}

// ── Copy / paste helpers ──────────────────────────────────────────────────────

/// Collect a node and its full subtree (BFS order, root first).
pub(crate) fn collect_subtree(session: &EditorSession, root_id: XrdsSceneNodeId) -> Vec<XrdsSceneNode> {
    let doc = session.document();
    let mut ids = vec![root_id];
    let mut i = 0;
    while i < ids.len() {
        let parent = ids[i];
        for n in &doc.nodes { if n.parent_id == Some(parent) { ids.push(n.id); } }
        i += 1;
    }
    doc.nodes.iter().filter(|n| ids.contains(&n.id)).cloned().collect()
}

/// Paste all clipboard subtrees into the document as siblings of their originals.
fn paste_clipboard(session: &mut EditorSession, state: &mut EditorState) {
    let Some(forest) = state.clipboard.clone() else { return; };
    if forest.is_empty() { return; }

    let mut new_root_ids: Vec<XrdsSceneNodeId> = vec![];
    let _ = session.session.edit(|doc| {
        let mut base = doc.nodes.iter().map(|n| n.id.0).max().unwrap_or(0) + 1;
        for subtree in &forest {
            let Some(root) = subtree.first() else { continue; };
            let root_id = root.id;
            let root_parent = root.parent_id;
            let id_map: std::collections::HashMap<XrdsSceneNodeId, XrdsSceneNodeId> = subtree
                .iter().enumerate()
                .map(|(i, n)| (n.id, XrdsSceneNodeId(base + i as u64)))
                .collect();
            base += subtree.len() as u64;
            for node in subtree {
                let mut new_node = node.clone();
                new_node.id = id_map[&node.id];
                if node.id == root_id {
                    new_node.parent_id = root_parent;
                    new_root_ids.push(new_node.id);
                } else {
                    new_node.parent_id = node.parent_id.map(|p| id_map.get(&p).copied().unwrap_or(p));
                }
                doc.nodes.push(new_node);
            }
        }
    });

    if !new_root_ids.is_empty() {
        state.selection.clear();
        for id in &new_root_ids { state.selection.add(*id); }
        state.editing_name = None;
        state.needs_full_reimport = true;
        state.status_message = Some(format!("Pasted {} node(s).", new_root_ids.len()));
    }
}

// ── Camera mode helpers ───────────────────────────────────────────────────────

pub(crate) fn toggle_camera_mode(state: &mut EditorState, cam: &mut EditorCameraState) {
    if state.camera_mode == CameraMode::Fly {
        state.camera_mode = CameraMode::Orbit;
        cam.distance = cam.fly_saved_distance.max(1.0);
    } else {
        cam.fly_saved_distance = cam.distance;
        cam.distance = 0.01;
        state.camera_mode = CameraMode::Fly;
    }
}

// ── Play mode helpers ─────────────────────────────────────────────────────────

pub(crate) fn start_play(session: &EditorSession, state: &mut EditorState) {
    state.play_snapshot = Some(session.document().clone());
    state.is_playing = true;
    state.play_started = true;
    state.status_message = Some("Playing — Esc to stop".into());
}

pub(crate) fn stop_play(session: &mut EditorSession, state: &mut EditorState) {
    let Some(snapshot) = state.play_snapshot.take() else {
        state.is_playing = false;
        return;
    };
    match XrdsSceneDocumentSession::new(snapshot) {
        Ok(new_session) => {
            session.session = new_session;
            state.selection.clear();
            state.editing_name = None;
            state.clear_pending_translations();
            state.clear_pending_rotations();
            state.pending_scale = None;
            state.pending_material = None;
            state.pending_visible = None;
            state.gizmo_drag = None;
            state.editing_scene_name = None;
            state.editing_scene_author = None;
            state.needs_full_reimport = true;
            state.status_message = Some("Stopped.".into());
        }
        Err(e) => {
            state.status_message = Some(format!("Stop failed: {e:?}"));
        }
    }
    state.is_playing = false;
}
