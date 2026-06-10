use xrds::editor::egui;

use crate::icon::IconName;
use crate::io::{export_app, export_glb, export_glb_selection, load, new_scene, save, save_as};
use crate::state::{EditorSession, EditorState, GizmoMode};

pub fn menubar_panel(
    ctx: &mut egui::Context,
    session: &mut EditorSession,
    state: &mut EditorState,
) {
    egui::TopBottomPanel::top("menubar")
        .exact_height(20.0)
        .show(ctx, |ui| {
            egui::menu::bar(ui, |ui| {
                // ── File ──────────────────────────────────────────────────────
                ui.menu_button("File", |ui| {
                    if menu_item_with_icon(ui, state, "New", "Ctrl+N", IconName::NewScene) { new_scene(session, state); ui.close(); }
                    if menu_item_with_icon(ui, state, "Open…", "Ctrl+O", IconName::OpenFile) { load(session, state); ui.close(); }
                    ui.separator();
                    if menu_item_with_icon(ui, state, "Save", "Ctrl+S", IconName::SaveFile) { save(session, state); ui.close(); }
                    if menu_item_with_icon(ui, state, "Save As…", "Ctrl+Shift+S", IconName::SaveAs) { save_as(session, state); ui.close(); }
                    ui.separator();
                    if menu_item_with_icon(ui, state, "Export Scene…", "Ctrl+Shift+E", IconName::Export) { export_glb(session, state); ui.close(); }
                    if ui.add(menu_item("Export as Application…", "")).clicked() { export_app(session, state); ui.close(); }
                    ui.separator();
                    let has_selection = !state.selection.is_empty();
                    if ui
                        .add_enabled(has_selection, menu_item("Export Selected…", ""))
                        .clicked()
                    {
                        export_glb_selection(session, state);
                        ui.close();
                    }
                });

                // ── Edit ──────────────────────────────────────────────────────
                ui.menu_button("Edit", |ui| {
                    let can_undo = session.session.can_undo();
                    let can_redo = session.session.can_redo();
                    if ui
                        .add_enabled(can_undo, menu_item("Undo", "Ctrl+Z"))
                        .clicked()
                    {
                        if session.session.undo() {
                            undo_redo_cleanup(state);
                        }
                        ui.close_menu();
                    }
                    if ui
                        .add_enabled(can_redo, menu_item("Redo", "Ctrl+Y"))
                        .clicked()
                    {
                        if session.session.redo() {
                            undo_redo_cleanup(state);
                        }
                        ui.close_menu();
                    }
                });

                // ── Window ────────────────────────────────────────────────────
                ui.menu_button("Window", |ui| {
                    ui.checkbox(&mut state.show_hierarchy, "Hierarchy");
                    ui.checkbox(&mut state.show_inspector, "Inspector");
                    ui.horizontal(|ui| {
                        ui.checkbox(&mut state.show_palette, "Palette");
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            ui.label(egui::RichText::new("P").small().color(egui::Color32::GRAY));
                        });
                    });
                });

                // ── Help ──────────────────────────────────────────────────────
                ui.menu_button("Help", |ui| {
                    if ui.add(menu_item("Keyboard Shortcuts", "?")).clicked() {
                        state.show_help = true;
                        ui.close_menu();
                    }
                });
            });
        });

    // ── Keyboard shortcuts window ─────────────────────────────────────────────
    if state.show_help {
        help_window(ctx, state);
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// A menu item button with the label left-aligned and the shortcut right-aligned.
fn menu_item(label: &str, shortcut: &str) -> egui::Button<'static> {
    egui::Button::new(egui::RichText::new(format!("{label:<28}{shortcut}")))
        .min_size(egui::vec2(220.0, 0.0))
}

/// Paint a menu item with a small icon to the left of the label.
/// Returns `true` if the item was clicked.
fn menu_item_with_icon(
    ui: &mut egui::Ui,
    state: &mut EditorState,
    label: &str,
    shortcut: &str,
    icon: IconName,
) -> bool {
    let tex = state.icon_cache.load_tinted_at(ui.ctx(), icon, 14, 8, egui::Color32::WHITE, 128);
    let sized = egui::load::SizedTexture::new(tex.id(), egui::vec2(14.0, 14.0));
    let img = egui::Image::new(sized).max_size(egui::vec2(14.0, 14.0));
    let btn = egui::Button::new(img).min_size(egui::vec2(220.0, 20.0));
    let resp = ui.add(btn);
    resp.clicked()
}

fn help_window(ctx: &mut egui::Context, state: &mut EditorState) {
    let mode_label = match state.gizmo_mode {
        GizmoMode::Translate => "Translate",
        GizmoMode::Rotate => "Rotate",
        GizmoMode::Scale => "Scale",
    };

    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        state.show_help = false;
        return;
    }

    let mut open = state.show_help;
    egui::Window::new("Keyboard Shortcuts")
        .collapsible(false)
        .resizable(true)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .default_size(egui::vec2(480.0, 520.0))
        .min_size(egui::vec2(320.0, 200.0))
        .open(&mut open)
        .show(ctx, |ui| {
            ui.label(
                egui::RichText::new(format!("Active gizmo mode: {mode_label}"))
                    .color(egui::Color32::from_rgb(100, 210, 120)),
            );
            ui.separator();

            egui::ScrollArea::vertical()
                .auto_shrink([false, false])
                .show(ui, |ui| {
                    shortcut_table(
                        ui,
                        &[
                            ("", "[File]"),
                            ("Ctrl+N", "New scene"),
                            ("Ctrl+O", "Open scene"),
                            ("Ctrl+S", "Save"),
                            ("Ctrl+Shift+S", "Save As"),
                            ("Ctrl+Shift+E", "Export scene as GLB"),
                            ("", "[Edit]"),
                            ("Ctrl+Z", "Undo"),
                            ("Ctrl+Y  /  Ctrl+Shift+Z", "Redo"),
                            ("Ctrl+C", "Copy selected subtree"),
                            ("Ctrl+V", "Paste copied subtree"),
                            ("Ctrl+D", "Duplicate selected node"),
                            ("Del", "Delete selected node"),
                            ("", "[Play]"),
                            ("Space", "Start play preview"),
                            ("Esc", "Stop play and restore scene"),
                            ("", "[Gizmo]"),
                            ("T", "Translate gizmo mode"),
                            ("R", "Rotate gizmo mode"),
                            ("Y", "Scale gizmo mode"),
                            ("F", "Frame selected node"),
                            (
                                "Ctrl + drag arrow",
                                "Snap translate to step (toolbar button cycles step)",
                            ),
                            ("Drag axis arrow", "Move node  (Translate mode)"),
                            ("Drag color ring", "Rotate node  (Rotate mode)"),
                            ("Drag axis handle", "Scale node on that axis  (Scale mode)"),
                            (
                                "Shift + drag scale axis",
                                "Uniform scale on all axes  (Inspector or Scale mode)",
                            ),
                            ("", "[Nudge — Translate mode only]"),
                            ("Left / Right", "Move along X axis  +/-0.1 m"),
                            ("Up / Down", "Move along Y axis  +/-0.1 m"),
                            ("PageUp / PageDown", "Move along Z axis  +/-0.1 m"),
                            ("Shift + arrow", "Move in 1 m steps"),
                            ("", "[Camera]"),
                            ("Middle drag", "Orbit"),
                            ("Shift + Middle drag", "Pan"),
                            ("Scroll wheel", "Zoom"),
                            ("W / S", "Fly pivot forward / back"),
                            ("A / D", "Fly pivot left / right"),
                            ("Q / E", "Fly pivot down / up"),
                            ("Shift + WASD", "Fast fly"),
                            ("", "[Panels]"),
                            ("P", "Toggle Palette panel"),
                        ],
                    );
                });

            ui.separator();
            ui.vertical_centered(|ui| {
                if ui.button("Close").clicked() {
                    state.show_help = false;
                }
            });
        });
    // Sync close from the title-bar X button.
    if !open {
        state.show_help = false;
    }
}

fn shortcut_table(ui: &mut egui::Ui, rows: &[(&str, &str)]) {
    egui::Grid::new("shortcuts_grid")
        .num_columns(2)
        .spacing([16.0, 3.0])
        .striped(true)
        .show(ui, |ui| {
            for (key, action) in rows {
                if key.is_empty() {
                    ui.end_row();
                    let heading_size = ui.text_style_height(&egui::TextStyle::Body) * 1.5;
                    ui.label(
                        egui::RichText::new(*action)
                            .color(egui::Color32::from_rgb(150, 150, 210))
                            .size(heading_size),
                    );
                    ui.end_row();
                } else {
                    ui.label(
                        egui::RichText::new(*key)
                            .monospace()
                            .color(egui::Color32::from_rgb(220, 200, 100)),
                    );
                    ui.label(*action);
                    ui.end_row();
                }
            }
        });
}

fn undo_redo_cleanup(state: &mut EditorState) {
    state.clear_pending_translations();
    state.clear_pending_rotations();
    state.pending_scale = None;
    state.pending_material = None;
    state.pending_visible = None;
    state.editing_name = None;
    state.needs_runtime_sync = true;
}
