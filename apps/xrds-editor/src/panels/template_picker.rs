use xrds::editor::egui;

use crate::io::apply_template;
use crate::state::{EditorSession, EditorState};
use crate::templates::{build_template, ALL_TEMPLATES};

pub fn template_picker_panel(
    ctx: &mut egui::Context,
    session: &mut EditorSession,
    state: &mut EditorState,
) {
    if !state.show_template_picker {
        return;
    }

    if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
        state.show_template_picker = false;
        return;
    }

    let mut open = true;
    egui::Window::new("New Scene")
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .fixed_size(egui::vec2(420.0, 0.0))
        .open(&mut open)
        .show(ctx, |ui| {
            ui.add_space(4.0);
            ui.label(
                egui::RichText::new("Choose a starting template:")
                    .color(egui::Color32::from_rgb(180, 180, 200)),
            );
            ui.add_space(6.0);

            for template in ALL_TEMPLATES {
                let selected = state.template_picker_selection == template.id;
                let bg = if selected {
                    egui::Color32::from_rgba_premultiplied(50, 90, 160, 80)
                } else {
                    egui::Color32::TRANSPARENT
                };

                let frame = egui::Frame::none()
                    .fill(bg)
                    .inner_margin(egui::Margin::symmetric(8, 6))
                    .rounding(egui::Rounding::same(4));

                let resp = frame.show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    ui.label(
                        egui::RichText::new(template.name)
                            .strong()
                            .color(egui::Color32::from_rgb(220, 220, 240)),
                    );
                    ui.label(
                        egui::RichText::new(template.description)
                            .small()
                            .color(egui::Color32::from_rgb(160, 160, 170)),
                    );
                });

                if resp.response.interact(egui::Sense::click()).clicked() {
                    state.template_picker_selection = template.id;
                }

                ui.add_space(2.0);
            }

            ui.add_space(8.0);
            ui.separator();
            ui.add_space(4.0);

            ui.horizontal(|ui| {
                let create_clicked = ui
                    .add_sized(
                        egui::vec2(90.0, 26.0),
                        egui::Button::new(
                            egui::RichText::new("Create").color(egui::Color32::WHITE),
                        ),
                    )
                    .clicked();

                ui.add_space(4.0);

                let cancel_clicked = ui
                    .add_sized(egui::vec2(70.0, 26.0), egui::Button::new("Cancel"))
                    .clicked();

                if create_clicked {
                    let doc = build_template(state.template_picker_selection);
                    apply_template(session, state, doc);
                    state.show_template_picker = false;
                }

                if cancel_clicked {
                    state.show_template_picker = false;
                }
            });

            ui.add_space(4.0);
        });

    if !open {
        state.show_template_picker = false;
    }
}
