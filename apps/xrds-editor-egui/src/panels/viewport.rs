//! Transparent overlay drawn on top of the 3D viewport.
//!
//! Draws the orientation indicator (world-axis cube) in the bottom-right corner.
//! The panel itself is invisible so the Bevy 3D scene renders through it unchanged.
//!
//! IMPORTANT: does NOT create a `CentralPanel`.  A `CentralPanel` would claim the
//! entire remaining area as an egui surface, making `ctx.wants_pointer_input()`
//! return `true` on every click in the viewport and blocking Bevy's ray-cast picking.
//! The indicator uses a non-interactable `Area` that only covers its own pixels.

use xrds::editor::{egui, EulerRot, Quat, Vec3};

use crate::camera::EditorCameraState;
use crate::state::EditorState;

const INDICATOR_RADIUS: f32 = 38.0;
const INDICATOR_MARGIN: f32 = 16.0;
const AXIS_LENGTH: f32 = 28.0;
const TIP_RADIUS: f32 = 5.0;
const LABEL_OFFSET: egui::Vec2 = egui::Vec2 { x: 6.0, y: -4.0 };

/// Draw the viewport overlay.
/// Edit mode: orientation indicator (world-axis cube) in the bottom-right corner.
/// Play mode: nothing — the Bevy UI HUD spawned in `spawn_player_pawn_system` handles it.
pub fn viewport_panel(
    ctx: &mut egui::Context,
    cam_state: &mut EditorCameraState,
    editor_state: &mut EditorState,
) {
    if editor_state.is_playing {
        return;
    }

    let viewport = ctx.available_rect();

    let widget_center = egui::pos2(
        viewport.right() - INDICATOR_RADIUS - INDICATOR_MARGIN,
        viewport.bottom() - INDICATOR_RADIUS - INDICATOR_MARGIN,
    );

    egui::Area::new(egui::Id::new("orientation_indicator"))
        .fixed_pos(egui::pos2(
            widget_center.x - INDICATOR_RADIUS,
            widget_center.y - INDICATOR_RADIUS,
        ))
        .interactable(false)
        .order(egui::Order::Foreground)
        .show(ctx, |ui| {
            let size = INDICATOR_RADIUS * 2.0;
            ui.set_min_size(egui::vec2(size, size));
            draw_orientation_indicator(ui.painter(), widget_center, cam_state);
        });
}

fn draw_orientation_indicator(
    painter: &egui::Painter,
    center: egui::Pos2,
    cam_state: &EditorCameraState,
) {
    painter.circle_filled(center, INDICATOR_RADIUS, egui::Color32::from_black_alpha(80));
    painter.circle_stroke(
        center,
        INDICATOR_RADIUS,
        egui::Stroke::new(1.0, egui::Color32::from_white_alpha(40)),
    );

    let cam_rot = Quat::from_euler(EulerRot::YXZ, cam_state.yaw, -cam_state.pitch, 0.0);

    let axes: [(&str, Vec3, [u8; 3]); 3] = [
        ("X", Vec3::X,     [220, 50,  50]),
        ("Y", Vec3::Y,     [50,  200, 50]),
        ("Z", Vec3::NEG_Z, [50,  80, 220]),
    ];

    let mut sorted: Vec<(f32, &str, Vec3, [u8; 3])> = axes
        .iter()
        .map(|(label, world_dir, color)| {
            let cam_space = cam_rot.inverse() * *world_dir;
            (cam_space.z, *label, *world_dir, *color)
        })
        .collect();
    sorted.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap());

    for (cam_z, label, world_dir, [r, g, b]) in &sorted {
        let cam_space = cam_rot.inverse() * *world_dir;
        let screen = egui::vec2(cam_space.x, -cam_space.y) * AXIS_LENGTH;
        let tip = egui::pos2(center.x + screen.x, center.y + screen.y);

        let alpha: u8 = if *cam_z < 0.0 { 255 } else { 100 };
        let color = egui::Color32::from_rgba_unmultiplied(*r, *g, *b, alpha);

        painter.line_segment([center, tip], egui::Stroke::new(2.5, color));
        painter.circle_filled(tip, TIP_RADIUS, color);

        if *cam_z < 0.0 {
            painter.text(
                tip + LABEL_OFFSET,
                egui::Align2::LEFT_CENTER,
                *label,
                egui::FontId::proportional(11.0),
                color,
            );
        }
    }
}
