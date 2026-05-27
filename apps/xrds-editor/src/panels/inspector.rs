use xrds::editor::{egui, EulerRot, Quat};
use xrds::scene_graph::{
    XrdsAudioDistanceModel, XrdsGrabType, XrdsHudAnchor, XrdsInteractionZoneShape,
    XrdsPlayerLocomotionMode, XrdsSceneCameraProjection, XrdsSceneHudText,
    XrdsSceneInteractionZone, XrdsSceneMaterialAlphaMode, XrdsSceneMaterialTextureSlotKind,
    XrdsSceneNodePayload, XrdsScenePlayerSpawn, XrdsSceneText, XrdsSceneTextAlignment,
    XrdsSceneTextureRef, XrdsXrBlendMode,
};
use xrds::sdk::{XrdsMaterialAlphaMode as SdkAlphaMode, XrdsMaterialParams, XrdsMaterialPbrParams};
use xrds::XrdsAnimationRepeatMode;

use crate::io::{scene_relative_uri, spawn_file_dialog};
use crate::state::{EditorSession, EditorState, PendingFileOpKind};

pub fn inspector_panel(
    ctx: &mut egui::Context,
    session: &mut EditorSession,
    editor_state: &mut EditorState,
) {
    egui::SidePanel::right("inspector")
        .resizable(true)
        .default_width(260.0)
        .min_width(200.0)
        .show(ctx, |ui| {
            ui.label(egui::RichText::new("Inspector").strong());
            ui.separator();

            egui::ScrollArea::vertical().show(ui, |ui| {
                scene_metadata_section(ui, session, editor_state);
                scene_environment_section(ui, session);

                // Multi-select summary — show count and skip per-node sections.
                if editor_state.selection.count() > 1 {
                    ui.colored_label(
                        egui::Color32::from_rgb(180, 180, 100),
                        format!("{} nodes selected  (move with gizmo)", editor_state.selection.count()),
                    );
                    return;
                }

                let Some(selected_id) = editor_state.selection.primary() else {
                    ui.colored_label(egui::Color32::GRAY, "Nothing selected");
                    return;
                };

                let doc = session.document();
                let Some(node) = doc.node(selected_id) else {
                    ui.colored_label(egui::Color32::GRAY, "Node not found");
                    return;
                };
                // Clone what we need before dropping the borrow.
                let node_name = node.name.clone();
                let node_enabled = node.enabled;
                let node_visible = node.visible;
                let transform = node.transform;
                let payload = node.payload.clone();
                let _ = drop(doc);

                // ── Identity ─────────────────────────────────────────────────
                egui::CollapsingHeader::new("Node")
                    .default_open(true)
                    .show(ui, |ui| {
                        // Name — must use a persistent text buffer in EditorState.
                        // egui stores cursor/selection in Memory but NOT the text content.
                        // Re-cloning from node_name every frame would erase typed characters.
                        ui.horizontal(|ui| {
                            ui.label("Name");

                            // Initialise buffer when selection changes.
                            if editor_state
                                .editing_name
                                .as_ref()
                                .map_or(true, |(id, _)| *id != selected_id)
                            {
                                editor_state.editing_name = Some((selected_id, node_name.clone()));
                            }

                            let mut buf = editor_state
                                .editing_name
                                .as_ref()
                                .map(|(_, s)| s.clone())
                                .unwrap_or_else(|| node_name.clone());

                            let resp = ui.add(
                                egui::TextEdit::singleline(&mut buf)
                                    .id(egui::Id::new(("inspector_name", selected_id.0)))
                                    .desired_width(ui.available_width()),
                            );

                            // Write the (possibly modified) buffer back every frame.
                            editor_state.editing_name = Some((selected_id, buf.clone()));

                            let commit = resp.lost_focus()
                                || (resp.has_focus()
                                    && ui.ctx().input(|i| i.key_pressed(egui::Key::Enter)));

                            if commit && buf != node_name {
                                if let Err(e) = session.session.edit(|doc| {
                                    if let Some(n) = doc.node_mut(selected_id) {
                                        n.name = buf.clone();
                                    }
                                }) {
                                    eprintln!("[inspector] rename failed: {e:?}");
                                }
                            }
                        });

                        ui.horizontal(|ui| {
                            // Enabled toggle — document only (no direct runtime equivalent).
                            let mut enabled = node_enabled;
                            if ui.checkbox(&mut enabled, "Enabled").changed() {
                                if let Err(e) = session.session.edit(|doc| {
                                    if let Some(n) = doc.node_mut(selected_id) {
                                        n.enabled = enabled;
                                    }
                                }) {
                                    eprintln!("[inspector] enabled edit failed: {e:?}");
                                }
                            }

                            // Visible toggle — document + runtime entity (Visibility component).
                            let mut visible = node_visible;
                            if ui.checkbox(&mut visible, "Visible").changed() {
                                if let Err(e) = session.session.edit(|doc| {
                                    if let Some(n) = doc.node_mut(selected_id) {
                                        n.visible = visible;
                                    }
                                }) {
                                    eprintln!("[inspector] visible edit failed: {e:?}");
                                }
                                // Also update the live Bevy entity via pending state.
                                editor_state.pending_visible = Some((selected_id, visible));
                            }
                        });
                    });

                // ── Transform ─────────────────────────────────────────────────
                egui::CollapsingHeader::new("Transform")
                    .default_open(true)
                    .show(ui, |ui| {
                        // Live values: prefer pending over document.
                        let [mut tx, mut ty, mut tz] = editor_state
                            .pending_translation_for(selected_id)
                            .unwrap_or(transform.translation);

                        let current_quat = {
                            let [qx, qy, qz, qw] = editor_state
                                .pending_rotation_for(selected_id)
                                .unwrap_or(transform.rotation_quat_xyzw);
                            Quat::from_xyzw(qx, qy, qz, qw)
                        };
                        let (ex, ey, ez) = current_quat.to_euler(EulerRot::XYZ);
                        let [mut rx, mut ry, mut rz] =
                            [ex.to_degrees(), ey.to_degrees(), ez.to_degrees()];

                        let [mut sx, mut sy, mut sz] = editor_state
                            .pending_scale
                            .filter(|(id, _)| *id == selected_id)
                            .map(|(_, v)| v)
                            .unwrap_or(transform.scale);

                        egui::Grid::new("transform_grid")
                            .num_columns(4)
                            .spacing([4.0, 4.0])
                            .show(ui, |ui| {
                                // Translation
                                let mut tc = false;
                                ui.label("T");
                                tc |= ui
                                    .add(egui::DragValue::new(&mut tx).speed(0.05).prefix("X "))
                                    .changed();
                                tc |= ui
                                    .add(egui::DragValue::new(&mut ty).speed(0.05).prefix("Y "))
                                    .changed();
                                tc |= ui
                                    .add(egui::DragValue::new(&mut tz).speed(0.05).prefix("Z "))
                                    .changed();
                                ui.end_row();
                                if tc {
                                    editor_state.set_pending_translation(selected_id, [tx, ty, tz]);
                                }

                                // Rotation (Euler °)
                                let mut rc = false;
                                ui.label("R");
                                rc |= ui
                                    .add(
                                        egui::DragValue::new(&mut rx)
                                            .speed(0.5)
                                            .prefix("X ")
                                            .suffix("°"),
                                    )
                                    .changed();
                                rc |= ui
                                    .add(
                                        egui::DragValue::new(&mut ry)
                                            .speed(0.5)
                                            .prefix("Y ")
                                            .suffix("°"),
                                    )
                                    .changed();
                                rc |= ui
                                    .add(
                                        egui::DragValue::new(&mut rz)
                                            .speed(0.5)
                                            .prefix("Z ")
                                            .suffix("°"),
                                    )
                                    .changed();
                                ui.end_row();
                                if rc {
                                    let q = Quat::from_euler(
                                        EulerRot::XYZ,
                                        rx.to_radians(),
                                        ry.to_radians(),
                                        rz.to_radians(),
                                    );
                                    editor_state.set_pending_rotation(selected_id, [q.x, q.y, q.z, q.w]);
                                }

                                // Scale  (Shift+drag = uniform)
                                let shift = ui.ctx().input(|i| i.modifiers.shift);
                                let (old_sx, old_sy, old_sz) = (sx, sy, sz);
                                ui.label("S").on_hover_text("⇧ Shift+drag for uniform scale");
                                let sx_c = ui
                                    .add(egui::DragValue::new(&mut sx).speed(0.01).prefix("X "))
                                    .changed();
                                let sy_c = ui
                                    .add(egui::DragValue::new(&mut sy).speed(0.01).prefix("Y "))
                                    .changed();
                                let sz_c = ui
                                    .add(egui::DragValue::new(&mut sz).speed(0.01).prefix("Z "))
                                    .changed();
                                ui.end_row();
                                if sx_c && shift { let d = sx - old_sx; sy = old_sy + d; sz = old_sz + d; }
                                if sy_c && shift { let d = sy - old_sy; sx = old_sx + d; sz = old_sz + d; }
                                if sz_c && shift { let d = sz - old_sz; sx = old_sx + d; sy = old_sy + d; }
                                if sx_c || sy_c || sz_c {
                                    editor_state.pending_scale = Some((selected_id, [sx, sy, sz]));
                                }
                            });

                        // Commit on mouse release (session write).
                        if !ui
                            .ctx()
                            .input(|i| i.pointer.button_down(egui::PointerButton::Primary))
                        {
                            commit_transform(session, editor_state, selected_id, &transform);
                        }
                    });

                // ── Type-specific ─────────────────────────────────────────────
                match &payload {
                    XrdsSceneNodePayload::Sphere(sphere) => {
                        egui::CollapsingHeader::new("Sphere")
                            .default_open(true)
                            .show(ui, |ui| {
                                material_section(
                                    ui,
                                    &sphere.material,
                                    selected_id,
                                    editor_state,
                                    session,
                                );
                            });
                    }

                    XrdsSceneNodePayload::Cube(cube) => {
                        egui::CollapsingHeader::new("Cube")
                            .default_open(true)
                            .show(ui, |ui| {
                                material_section(
                                    ui,
                                    &cube.material,
                                    selected_id,
                                    editor_state,
                                    session,
                                );
                            });
                    }

                    XrdsSceneNodePayload::Cylinder(cyl) => {
                        egui::CollapsingHeader::new("Cylinder")
                            .default_open(true)
                            .show(ui, |ui| {
                                material_section(
                                    ui,
                                    &cyl.material,
                                    selected_id,
                                    editor_state,
                                    session,
                                );
                            });
                    }

                    XrdsSceneNodePayload::Plane3D(plane) => {
                        egui::CollapsingHeader::new("Plane")
                            .default_open(true)
                            .show(ui, |ui| {
                                material_section(
                                    ui,
                                    &plane.material,
                                    selected_id,
                                    editor_state,
                                    session,
                                );
                            });
                    }

                    XrdsSceneNodePayload::Camera(cam) => {
                        egui::CollapsingHeader::new("Camera")
                            .default_open(true)
                            .show(ui, |ui| match cam.projection {
                                XrdsSceneCameraProjection::Perspective {
                                    mut fov_deg,
                                    mut near,
                                    far,
                                    order,
                                } => {
                                    let mut f = fov_deg;
                                    let mut n = near;
                                    let mut finite = far.is_some();
                                    let mut far_val = far.unwrap_or(1000.0);
                                    let mut changed = false;
                                    changed |= ui
                                        .add(egui::Slider::new(&mut f, 10.0..=170.0).text("FOV °"))
                                        .changed();
                                    changed |= ui
                                        .add(
                                            egui::DragValue::new(&mut n)
                                                .speed(0.001)
                                                .prefix("Near ")
                                                .clamp_range(0.0001..=f32::MAX),
                                        )
                                        .changed();
                                    ui.horizontal(|ui| {
                                        if ui.checkbox(&mut finite, "Far").changed() {
                                            changed = true;
                                        }
                                        if finite {
                                            changed |= ui
                                                .add(
                                                    egui::DragValue::new(&mut far_val)
                                                        .speed(1.0)
                                                        .clamp_range(n..=f32::MAX),
                                                )
                                                .changed();
                                        } else {
                                            ui.weak("∞");
                                        }
                                    });
                                    if changed {
                                        fov_deg = f;
                                        near = n;
                                        let new_far = if finite { Some(far_val) } else { None };
                                        let _ = session.session.edit(|doc| {
                                            if let Some(nd) = doc.node_mut(selected_id) {
                                                if let XrdsSceneNodePayload::Camera(c) =
                                                    &mut nd.payload
                                                {
                                                    c.projection =
                                                        XrdsSceneCameraProjection::Perspective {
                                                            fov_deg,
                                                            near,
                                                            far: new_far,
                                                            order,
                                                        };
                                                }
                                            }
                                        });
                                    }
                                }
                                XrdsSceneCameraProjection::Orthographic {
                                    mut scale,
                                    near,
                                    far,
                                    order,
                                } => {
                                    if ui
                                        .add(
                                            egui::DragValue::new(&mut scale)
                                                .speed(0.01)
                                                .prefix("Scale "),
                                        )
                                        .changed()
                                    {
                                        let _ = session.session.edit(|doc| {
                                            if let Some(nd) = doc.node_mut(selected_id) {
                                                if let XrdsSceneNodePayload::Camera(c) =
                                                    &mut nd.payload
                                                {
                                                    c.projection =
                                                        XrdsSceneCameraProjection::Orthographic {
                                                            scale,
                                                            near,
                                                            far,
                                                            order,
                                                        };
                                                }
                                            }
                                        });
                                    }
                                }
                            });
                    }

                    XrdsSceneNodePayload::PointLight(light) => {
                        egui::CollapsingHeader::new("Point Light")
                            .default_open(true)
                            .show(ui, |ui| {
                                let pending = editor_state.pending_point_light
                                    .filter(|(id, ..)| *id == selected_id);
                                let mut color = pending.map(|(_, c, ..)| c).unwrap_or(light.color);
                                let mut intensity = pending.map(|(_, _, i, _)| i).unwrap_or(light.intensity);
                                let mut range = pending.map(|(_, _, _, r)| r).unwrap_or(light.range);
                                let mut changed = false;
                                changed |= color_row(ui, "Color", &mut color);
                                changed |= ui
                                    .add(
                                        egui::DragValue::new(&mut intensity)
                                            .speed(100.0)
                                            .prefix("Intensity ")
                                            .suffix(" cd")
                                            .clamp_range(0.0..=f32::MAX),
                                    )
                                    .changed();
                                changed |= ui
                                    .add(
                                        egui::DragValue::new(&mut range)
                                            .speed(0.1)
                                            .prefix("Range ")
                                            .suffix(" m")
                                            .clamp_range(0.0..=f32::MAX),
                                    )
                                    .changed();
                                if changed {
                                    editor_state.pending_point_light = Some((selected_id, color, intensity, range));
                                }
                                if !ui.ctx().input(|i| i.pointer.button_down(egui::PointerButton::Primary)) {
                                    if let Some((id, c, i, r)) = editor_state.pending_point_light.take() {
                                        if id == selected_id {
                                            let _ = session.session.edit(|doc| {
                                                if let Some(n) = doc.node_mut(id) {
                                                    if let XrdsSceneNodePayload::PointLight(l) = &mut n.payload {
                                                        l.color = c;
                                                        l.intensity = i;
                                                        l.range = r;
                                                    }
                                                }
                                            });
                                        }
                                    }
                                }
                            });
                    }

                    XrdsSceneNodePayload::DirectionalLight(light) => {
                        egui::CollapsingHeader::new("Directional Light")
                            .default_open(true)
                            .show(ui, |ui| {
                                let pending = editor_state.pending_directional_light
                                    .filter(|(id, ..)| *id == selected_id);
                                let mut color = pending.map(|(_, c, _)| c).unwrap_or(light.color);
                                let mut illuminance = pending.map(|(_, _, i)| i).unwrap_or(light.illuminance);
                                let mut changed = false;
                                changed |= color_row(ui, "Color", &mut color);
                                changed |= ui
                                    .add(
                                        egui::DragValue::new(&mut illuminance)
                                            .speed(200.0)
                                            .prefix("Illuminance ")
                                            .suffix(" lx")
                                            .clamp_range(0.0..=f32::MAX),
                                    )
                                    .changed();
                                if changed {
                                    editor_state.pending_directional_light = Some((selected_id, color, illuminance));
                                }
                                if !ui.ctx().input(|i| i.pointer.button_down(egui::PointerButton::Primary)) {
                                    if let Some((id, c, i)) = editor_state.pending_directional_light.take() {
                                        if id == selected_id {
                                            let _ = session.session.edit(|doc| {
                                                if let Some(n) = doc.node_mut(id) {
                                                    if let XrdsSceneNodePayload::DirectionalLight(l) = &mut n.payload {
                                                        l.color = c;
                                                        l.illuminance = i;
                                                    }
                                                }
                                            });
                                        }
                                    }
                                }
                            });
                    }

                    XrdsSceneNodePayload::AudioClip(clip) => {
                        egui::CollapsingHeader::new("Audio Clip")
                            .default_open(true)
                            .show(ui, |ui| {
                                ui.label(format!("Asset  {}", clip.asset_id));
                                let mut volume = clip.volume;
                                let mut looped = clip.looped;
                                let mut spatial = clip.spatial;
                                let mut autoplay = clip.autoplay;
                                let mut distance_model = clip.distance_model;
                                let mut min_distance = clip.min_distance;
                                let mut max_distance = clip.max_distance;
                                let mut rolloff_factor = clip.rolloff_factor;
                                let mut hrtf = clip.hrtf;
                                let mut changed = false;
                                changed |= ui
                                    .add(egui::Slider::new(&mut volume, 0.0..=1.0).text("Volume"))
                                    .changed();
                                ui.horizontal(|ui| {
                                    changed |= ui.checkbox(&mut looped, "Looped").changed();
                                    changed |= ui.checkbox(&mut spatial, "Spatial").changed();
                                    changed |= ui.checkbox(&mut autoplay, "Autoplay").changed();
                                });
                                if spatial {
                                    ui.separator();
                                    ui.label(egui::RichText::new("Spatial Parameters").small().weak());
                                    // Distance model selector
                                    ui.horizontal(|ui| {
                                        ui.label("Distance Model");
                                        let label = match distance_model {
                                            XrdsAudioDistanceModel::Linear => "Linear",
                                            XrdsAudioDistanceModel::Inverse => "Inverse",
                                            XrdsAudioDistanceModel::Exponential => "Exponential",
                                        };
                                        egui::ComboBox::from_id_salt("audio_dist_model")
                                            .selected_text(label)
                                            .show_ui(ui, |ui| {
                                                for model in [
                                                    XrdsAudioDistanceModel::Linear,
                                                    XrdsAudioDistanceModel::Inverse,
                                                    XrdsAudioDistanceModel::Exponential,
                                                ] {
                                                    let lbl = match model {
                                                        XrdsAudioDistanceModel::Linear => "Linear",
                                                        XrdsAudioDistanceModel::Inverse => "Inverse",
                                                        XrdsAudioDistanceModel::Exponential => "Exponential",
                                                    };
                                                    if ui.selectable_value(&mut distance_model, model, lbl).changed() {
                                                        changed = true;
                                                    }
                                                }
                                            });
                                    });
                                    changed |= ui
                                        .add(egui::Slider::new(&mut min_distance, 0.0..=max_distance).text("Min Distance"))
                                        .changed();
                                    changed |= ui
                                        .add(egui::Slider::new(&mut max_distance, min_distance..=500.0).text("Max Distance"))
                                        .changed();
                                    changed |= ui
                                        .add(egui::Slider::new(&mut rolloff_factor, 0.0..=10.0).text("Rolloff"))
                                        .changed();
                                    changed |= ui.checkbox(&mut hrtf, "HRTF").changed();
                                }
                                if changed {
                                    let _ = session.session.edit(|doc| {
                                        if let Some(n) = doc.node_mut(selected_id) {
                                            if let XrdsSceneNodePayload::AudioClip(c) =
                                                &mut n.payload
                                            {
                                                c.volume = volume;
                                                c.looped = looped;
                                                c.spatial = spatial;
                                                c.autoplay = autoplay;
                                                c.distance_model = distance_model;
                                                c.min_distance = min_distance;
                                                c.max_distance = max_distance;
                                                c.rolloff_factor = rolloff_factor;
                                                c.hrtf = hrtf;
                                            }
                                        }
                                    });
                                }
                            });
                    }

                    XrdsSceneNodePayload::InteractionZone(zone) => {
                        interaction_zone_section(ui, session, editor_state, selected_id, zone);
                    }

                    XrdsSceneNodePayload::PlayerSpawn(spawn) => {
                        player_spawn_section(ui, session, selected_id, spawn);
                    }

                    XrdsSceneNodePayload::HudText(hud) => {
                        hud_text_section(ui, session, selected_id, hud);
                    }

                    XrdsSceneNodePayload::Text(text) => {
                        text3d_section(ui, session, editor_state, selected_id, text);
                    }

                    XrdsSceneNodePayload::GltfAsset(asset) => {
                        egui::CollapsingHeader::new("glTF Asset")
                            .default_open(true)
                            .show(ui, |ui| {
                                if let Some(ref id) = asset.asset_id {
                                    ui.horizontal(|ui| {
                                        ui.label("Asset ID");
                                        ui.colored_label(
                                            ui.visuals().text_color(),
                                            egui::RichText::new(id).monospace(),
                                        );
                                    });
                                }
                                ui.label(format!("URI  {}", asset.asset_uri));
                                ui.label(format!("Scene  {}", asset.scene_index));
                            });
                        gltf_animation_section(ui, editor_state, selected_id);
                    }

                    XrdsSceneNodePayload::SpotLight(light) => {
                        egui::CollapsingHeader::new("Spot Light")
                            .default_open(true)
                            .show(ui, |ui| {
                                let pending = editor_state.pending_spot_light
                                    .filter(|(id, ..)| *id == selected_id);
                                let mut color = pending.map(|(_, c, ..)| c).unwrap_or(light.color);
                                let mut intensity = pending.map(|(_, _, i, ..)| i).unwrap_or(light.intensity);
                                let mut range = pending.map(|(_, _, _, r, ..)| r).unwrap_or(light.range);
                                let mut inner_deg = pending.map(|(_, _, _, _, ia, _)| ia.to_degrees())
                                    .unwrap_or_else(|| light.inner_angle.to_degrees());
                                let mut outer_deg = pending.map(|(_, _, _, _, _, oa)| oa.to_degrees())
                                    .unwrap_or_else(|| light.outer_angle.to_degrees());
                                let mut shadows = light.shadows;
                                let mut changed = false;
                                changed |= color_row(ui, "Color", &mut color);
                                changed |= ui
                                    .add(
                                        egui::DragValue::new(&mut intensity)
                                            .speed(500.0)
                                            .prefix("Intensity ")
                                            .suffix(" cd")
                                            .clamp_range(0.0..=f32::MAX),
                                    )
                                    .changed();
                                changed |= ui
                                    .add(
                                        egui::DragValue::new(&mut range)
                                            .speed(0.1)
                                            .prefix("Range ")
                                            .suffix(" m")
                                            .clamp_range(0.0..=f32::MAX),
                                    )
                                    .changed();
                                changed |= ui
                                    .add(
                                        egui::DragValue::new(&mut inner_deg)
                                            .speed(0.5)
                                            .prefix("Inner ")
                                            .suffix("°")
                                            .clamp_range(0.0..=outer_deg),
                                    )
                                    .changed();
                                changed |= ui
                                    .add(
                                        egui::DragValue::new(&mut outer_deg)
                                            .speed(0.5)
                                            .prefix("Outer ")
                                            .suffix("°")
                                            .clamp_range(inner_deg..=89.9),
                                    )
                                    .changed();
                                let shadows_changed = ui.checkbox(&mut shadows, "Shadows").changed();
                                if changed {
                                    editor_state.pending_spot_light = Some((selected_id, color, intensity, range, inner_deg.to_radians(), outer_deg.to_radians()));
                                }
                                if shadows_changed {
                                    let _ = session.session.edit(|doc| {
                                        if let Some(n) = doc.node_mut(selected_id) {
                                            if let XrdsSceneNodePayload::SpotLight(l) = &mut n.payload {
                                                l.shadows = shadows;
                                            }
                                        }
                                    });
                                }
                                if !ui.ctx().input(|i| i.pointer.button_down(egui::PointerButton::Primary)) {
                                    if let Some((id, c, i, r, ia, oa)) = editor_state.pending_spot_light.take() {
                                        if id == selected_id {
                                            let _ = session.session.edit(|doc| {
                                                if let Some(n) = doc.node_mut(id) {
                                                    if let XrdsSceneNodePayload::SpotLight(l) = &mut n.payload {
                                                        l.color = c;
                                                        l.intensity = i;
                                                        l.range = r;
                                                        l.inner_angle = ia;
                                                        l.outer_angle = oa;
                                                    }
                                                }
                                            });
                                        }
                                    }
                                }
                            });
                    }

                    XrdsSceneNodePayload::AmbientLight(light) => {
                        egui::CollapsingHeader::new("Ambient Light")
                            .default_open(true)
                            .show(ui, |ui| {
                                let pending = editor_state.pending_ambient_light
                                    .filter(|(id, ..)| *id == selected_id);
                                let mut color = pending.map(|(_, c, _)| c).unwrap_or(light.color);
                                let mut brightness = pending.map(|(_, _, b)| b).unwrap_or(light.brightness);
                                let mut changed = false;
                                changed |= color_row(ui, "Color", &mut color);
                                changed |= ui
                                    .add(
                                        egui::DragValue::new(&mut brightness)
                                            .speed(1.0)
                                            .prefix("Brightness ")
                                            .suffix(" cd/m²")
                                            .clamp_range(0.0..=f32::MAX),
                                    )
                                    .changed();
                                if changed {
                                    editor_state.pending_ambient_light = Some((selected_id, color, brightness));
                                }
                                if !ui.ctx().input(|i| i.pointer.button_down(egui::PointerButton::Primary)) {
                                    if let Some((id, c, b)) = editor_state.pending_ambient_light.take() {
                                        if id == selected_id {
                                            let _ = session.session.edit(|doc| {
                                                if let Some(n) = doc.node_mut(id) {
                                                    if let XrdsSceneNodePayload::AmbientLight(l) = &mut n.payload {
                                                        l.color = c;
                                                        l.brightness = b;
                                                    }
                                                }
                                            });
                                        }
                                    }
                                }
                            });
                    }

                    _ => {}
                }
            });
        });
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Inline colour edit row — shows a small swatch + hex label.
fn color_row(ui: &mut egui::Ui, label: &str, color: &mut [f32; 4]) -> bool {
    let mut rgba = egui::Rgba::from_rgba_premultiplied(color[0], color[1], color[2], color[3]);
    ui.horizontal(|ui| {
        ui.label(label);
        let changed = egui::color_picker::color_edit_button_rgba(
            ui,
            &mut rgba,
            egui::color_picker::Alpha::Opaque,
        )
        .changed();
        if changed {
            *color = [rgba.r(), rgba.g(), rgba.b(), rgba.a()];
        }
        changed
    })
    .inner
}

/// Editable material section shared by all mesh-type nodes.
fn material_section(
    ui: &mut egui::Ui,
    mat: &xrds::scene_graph::XrdsSceneMaterial,
    node_id: xrds::scene_graph::XrdsSceneNodeId,
    editor_state: &mut EditorState,
    session: &mut EditorSession,
) {
    egui::CollapsingHeader::new("Material")
        .default_open(true)
        .show(ui, |ui| {
            // Use pending material if available, otherwise clone from document.
            let pending_for_node = editor_state
                .pending_material
                .as_ref()
                .filter(|(id, _)| *id == node_id)
                .map(|(_, v)| v.clone());
            let mut m: XrdsMaterialParams =
                pending_for_node.unwrap_or_else(|| XrdsMaterialParams {
                    base_color: xrds::sdk::XrdsColor {
                        rgba: mat.base_color,
                    },
                    emissive: xrds::sdk::XrdsLinearRgba { rgba: mat.emissive },
                    opacity: mat.opacity,
                    unlit: mat.unlit,
                    pbr: XrdsMaterialPbrParams {
                        metallic: mat.pbr.metallic,
                        roughness: mat.pbr.roughness,
                        reflectance: mat.pbr.reflectance,
                        double_sided: mat.pbr.double_sided,
                        alpha_mode: scene_alpha_to_sdk(mat.pbr.alpha_mode),
                        alpha_cutoff: mat.pbr.alpha_cutoff,
                    },
                    textures: Default::default(),
                });

            let mut changed = false;

            // Base color
            let mut bc = m.base_color.rgba;
            if color_row(ui, "Base color", &mut bc) {
                m.base_color.rgba = bc;
                changed = true;
            }

            // Emissive color
            let mut em = m.emissive.rgba;
            if color_row(ui, "Emissive", &mut em) {
                m.emissive.rgba = em;
                changed = true;
            }

            // Sliders
            changed |= ui
                .add(egui::Slider::new(&mut m.opacity, 0.0..=1.0).text("Opacity"))
                .changed();
            changed |= ui
                .add(egui::Slider::new(&mut m.pbr.roughness, 0.0..=1.0).text("Roughness"))
                .changed();
            changed |= ui
                .add(egui::Slider::new(&mut m.pbr.metallic, 0.0..=1.0).text("Metallic"))
                .changed();
            changed |= ui.checkbox(&mut m.unlit, "Unlit").changed();
            changed |= ui.checkbox(&mut m.pbr.double_sided, "Double sided").changed();

            // Alpha mode dropdown + cutoff
            ui.horizontal(|ui| {
                ui.label("Alpha mode");
                egui::ComboBox::from_id_salt("alpha_mode")
                    .selected_text(alpha_mode_label(m.pbr.alpha_mode))
                    .show_ui(ui, |ui| {
                        for mode in [SdkAlphaMode::Auto, SdkAlphaMode::Opaque, SdkAlphaMode::Mask, SdkAlphaMode::Blend] {
                            if ui.selectable_value(&mut m.pbr.alpha_mode, mode, alpha_mode_label(mode)).changed() {
                                changed = true;
                            }
                        }
                    });
            });
            if m.pbr.alpha_mode == SdkAlphaMode::Mask {
                changed |= ui
                    .add(egui::Slider::new(&mut m.pbr.alpha_cutoff, 0.0..=1.0).text("Cutoff"))
                    .changed();
            }

            if changed {
                editor_state.pending_material = Some((node_id, m));
            }

            // Texture slots (file-pick per slot — commits immediately, triggers reimport).
            texture_slots_section(ui, mat, node_id, editor_state, session);

            // Commit on release.
            if !ui
                .ctx()
                .input(|i| i.pointer.button_down(egui::PointerButton::Primary))
            {
                if let Some((id, mat_params)) = editor_state.pending_material.take() {
                    if id == node_id {
                        let _ = session.session.set_node_material_base_color(
                            id,
                            xrds::sdk::XrdsColor {
                                rgba: mat_params.base_color.rgba,
                            },
                        );
                        let _ = session.session.set_node_material_emissive(
                            id,
                            xrds::sdk::XrdsLinearRgba { rgba: mat_params.emissive.rgba },
                        );
                        let _ = session
                            .session
                            .set_node_material_opacity(id, mat_params.opacity);
                        let _ = session.session.set_node_material_pbr(
                            id,
                            xrds::scene_graph::XrdsSceneMaterialPbrParams {
                                metallic: mat_params.pbr.metallic,
                                roughness: mat_params.pbr.roughness,
                                reflectance: mat_params.pbr.reflectance,
                                double_sided: mat_params.pbr.double_sided,
                                alpha_mode: sdk_alpha_to_scene(mat_params.pbr.alpha_mode),
                                alpha_cutoff: mat_params.pbr.alpha_cutoff,
                            },
                        );
                    }
                }
            }
        });
}

/// Commit pending transform fields to the session document on mouse release.
fn commit_transform(
    session: &mut EditorSession,
    editor_state: &mut EditorState,
    node_id: xrds::scene_graph::XrdsSceneNodeId,
    doc_transform: &xrds::scene_graph::XrdsSceneTransform,
) {
    // Only commit if at least one transform field has a pending value for this node.
    let translation = editor_state.pending_translation_for(node_id);
    let rotation = editor_state.pending_rotation_for(node_id);
    let scale    = editor_state.pending_scale.filter(|(id, _)| *id == node_id).map(|(_, v)| v);

    if translation.is_none() && rotation.is_none() && scale.is_none() {
        return;
    }

    session.session.set_node_transform(
        node_id,
        xrds::scene_graph::XrdsSceneTransform {
            translation: translation.unwrap_or(doc_transform.translation),
            rotation_quat_xyzw: rotation.unwrap_or(doc_transform.rotation_quat_xyzw),
            scale: scale.unwrap_or(doc_transform.scale),
        },
    );
    editor_state.pending_translations.retain(|(id, _)| *id != node_id);
    editor_state.pending_rotations.retain(|(id, _)| *id != node_id);
    editor_state.pending_scale = None;
}

// ── Interaction Zone ──────────────────────────────────────────────────────────

fn interaction_zone_section(
    ui: &mut egui::Ui,
    session: &mut EditorSession,
    _editor_state: &mut EditorState,
    selected_id: xrds::scene_graph::XrdsSceneNodeId,
    zone: &XrdsSceneInteractionZone,
) {
    egui::CollapsingHeader::new("Interaction Zone")
        .default_open(true)
        .show(ui, |ui| {
            let mut shape = zone.shape;
            let mut grab_type = zone.grab_type;
            let mut hoverable = zone.hoverable;
            let mut changed = false;

            // Shape selector
            ui.horizontal(|ui| {
                ui.label("Shape");
                let is_box = matches!(shape, XrdsInteractionZoneShape::Box { .. });
                let is_sphere = matches!(shape, XrdsInteractionZoneShape::Sphere { .. });
                if ui.selectable_label(is_box, "Box").clicked() && !is_box {
                    shape = XrdsInteractionZoneShape::Box { half_extents: [0.5, 0.5, 0.5] };
                    changed = true;
                }
                if ui.selectable_label(is_sphere, "Sphere").clicked() && !is_sphere {
                    shape = XrdsInteractionZoneShape::Sphere { radius: 0.5 };
                    changed = true;
                }
            });

            // Shape parameters
            match shape {
                XrdsInteractionZoneShape::Box { mut half_extents } => {
                    ui.horizontal(|ui| {
                        ui.label("Half extents");
                        if ui.add(egui::DragValue::new(&mut half_extents[0]).speed(0.01).prefix("X ").range(0.01..=f32::MAX)).changed() { changed = true; shape = XrdsInteractionZoneShape::Box { half_extents }; }
                        if ui.add(egui::DragValue::new(&mut half_extents[1]).speed(0.01).prefix("Y ").range(0.01..=f32::MAX)).changed() { changed = true; shape = XrdsInteractionZoneShape::Box { half_extents }; }
                        if ui.add(egui::DragValue::new(&mut half_extents[2]).speed(0.01).prefix("Z ").range(0.01..=f32::MAX)).changed() { changed = true; shape = XrdsInteractionZoneShape::Box { half_extents }; }
                    });
                }
                XrdsInteractionZoneShape::Sphere { mut radius } => {
                    if ui.add(egui::DragValue::new(&mut radius).speed(0.01).prefix("Radius ").range(0.01..=f32::MAX)).changed() {
                        changed = true;
                        shape = XrdsInteractionZoneShape::Sphere { radius };
                    }
                }
            }

            // Grab type
            ui.horizontal(|ui| {
                ui.label("Grab");
                changed |= ui.selectable_value(&mut grab_type, XrdsGrabType::None, "None").changed();
                changed |= ui.selectable_value(&mut grab_type, XrdsGrabType::Snap, "Snap").changed();
                changed |= ui.selectable_value(&mut grab_type, XrdsGrabType::Free, "Free").changed();
            });

            // Hoverable
            changed |= ui.checkbox(&mut hoverable, "Hoverable").changed();

            if changed {
                let new_zone = XrdsSceneInteractionZone { shape, grab_type, hoverable };
                let _ = session.session.edit(|doc| {
                    if let Some(n) = doc.node_mut(selected_id) {
                        if let XrdsSceneNodePayload::InteractionZone(z) = &mut n.payload {
                            *z = new_zone;
                        }
                    }
                });
            }
        });
}

// ── Player Spawn ──────────────────────────────────────────────────────────────

fn player_spawn_section(
    ui: &mut egui::Ui,
    session: &mut EditorSession,
    selected_id: xrds::scene_graph::XrdsSceneNodeId,
    spawn: &XrdsScenePlayerSpawn,
) {
    egui::CollapsingHeader::new("Player Spawn")
        .default_open(true)
        .show(ui, |ui| {
            let mut locomotion = spawn.locomotion_mode;
            let mut fov = spawn.fov_deg;
            let mut changed = false;

            ui.horizontal(|ui| {
                ui.label("Locomotion");
                if ui.selectable_label(locomotion == XrdsPlayerLocomotionMode::Teleport, "Teleport").clicked() {
                    locomotion = XrdsPlayerLocomotionMode::Teleport;
                    changed = true;
                }
                if ui.selectable_label(locomotion == XrdsPlayerLocomotionMode::Smooth, "Smooth").clicked() {
                    locomotion = XrdsPlayerLocomotionMode::Smooth;
                    changed = true;
                }
                if ui.selectable_label(locomotion == XrdsPlayerLocomotionMode::Flying, "Flying").clicked() {
                    locomotion = XrdsPlayerLocomotionMode::Flying;
                    changed = true;
                }
            });

            ui.horizontal(|ui| {
                ui.label("FOV (deg)");
                if ui.add(egui::DragValue::new(&mut fov).speed(0.5).range(30.0..=170.0)).changed() {
                    changed = true;
                }
            });

            if changed {
                let new_spawn = XrdsScenePlayerSpawn { locomotion_mode: locomotion, fov_deg: fov };
                let _ = session.session.edit(|doc| {
                    if let Some(n) = doc.node_mut(selected_id) {
                        if let XrdsSceneNodePayload::PlayerSpawn(s) = &mut n.payload {
                            *s = new_spawn;
                        }
                    }
                });
            }
        });
}

// ── HUD Text ──────────────────────────────────────────────────────────────────

fn hud_text_section(
    ui: &mut egui::Ui,
    session: &mut EditorSession,
    selected_id: xrds::scene_graph::XrdsSceneNodeId,
    hud: &XrdsSceneHudText,
) {
    egui::CollapsingHeader::new("HUD Text")
        .default_open(true)
        .show(ui, |ui| {
            let mut text = hud.text.clone();
            let mut font_size = hud.font_size;
            let mut color = hud.color;
            let mut anchor = hud.anchor;
            let mut offset = hud.offset;
            let mut changed = false;

            ui.horizontal(|ui| {
                ui.label("Text");
                if ui.text_edit_singleline(&mut text).changed() {
                    changed = true;
                }
            });

            ui.horizontal(|ui| {
                ui.label("Font Size");
                if ui
                    .add(egui::DragValue::new(&mut font_size).speed(0.5).range(4.0..=200.0))
                    .changed()
                {
                    changed = true;
                }
            });

            if color_row(ui, "Color", &mut color) {
                changed = true;
            }

            ui.horizontal(|ui| {
                ui.label("Anchor");
                for (label, variant) in &[
                    ("TL", XrdsHudAnchor::TopLeft),
                    ("TC", XrdsHudAnchor::TopCenter),
                    ("TR", XrdsHudAnchor::TopRight),
                    ("ML", XrdsHudAnchor::MiddleLeft),
                    ("C", XrdsHudAnchor::Center),
                    ("MR", XrdsHudAnchor::MiddleRight),
                    ("BL", XrdsHudAnchor::BottomLeft),
                    ("BC", XrdsHudAnchor::BottomCenter),
                    ("BR", XrdsHudAnchor::BottomRight),
                ] {
                    if ui.selectable_label(anchor == *variant, *label).clicked() {
                        anchor = *variant;
                        changed = true;
                    }
                }
            });

            ui.horizontal(|ui| {
                ui.label("Offset X");
                if ui.add(egui::DragValue::new(&mut offset[0]).speed(1.0)).changed() {
                    changed = true;
                }
                ui.label("Y");
                if ui.add(egui::DragValue::new(&mut offset[1]).speed(1.0)).changed() {
                    changed = true;
                }
            });

            if changed {
                let new_hud = XrdsSceneHudText { text, font_size, color, anchor, offset };
                let _ = session.session.edit(|doc| {
                    if let Some(n) = doc.node_mut(selected_id) {
                        if let XrdsSceneNodePayload::HudText(h) = &mut n.payload {
                            *h = new_hud;
                        }
                    }
                });
            }
        });
}

// ── Text 3D ───────────────────────────────────────────────────────────────────

fn text3d_section(
    ui: &mut egui::Ui,
    session: &mut EditorSession,
    editor_state: &mut EditorState,
    selected_id: xrds::scene_graph::XrdsSceneNodeId,
    text_node: &XrdsSceneText,
) {
    egui::CollapsingHeader::new("Text 3D")
        .default_open(true)
        .show(ui, |ui| {
            let mut text = text_node.text.clone();
            let mut font_size = text_node.font_size;
            let mut color = text_node.color;
            let mut alignment = text_node.alignment;
            let mut changed = false;

            ui.horizontal(|ui| {
                ui.label("Text");
                if ui.text_edit_singleline(&mut text).changed() {
                    changed = true;
                }
            });

            ui.horizontal(|ui| {
                ui.label("Font Size");
                if ui
                    .add(egui::DragValue::new(&mut font_size).speed(0.5).range(4.0..=200.0))
                    .changed()
                {
                    changed = true;
                }
            });

            if color_row(ui, "Color", &mut color) {
                changed = true;
            }

            ui.horizontal(|ui| {
                ui.label("Align");
                for (label, variant) in &[
                    ("Left", XrdsSceneTextAlignment::Left),
                    ("Center", XrdsSceneTextAlignment::Center),
                    ("Right", XrdsSceneTextAlignment::Right),
                ] {
                    if ui.selectable_label(alignment == *variant, *label).clicked() {
                        alignment = *variant;
                        changed = true;
                    }
                }
            });

            if changed {
                let new_text = XrdsSceneText { text, font_size, color, alignment };
                let _ = session.session.edit(|doc| {
                    if let Some(n) = doc.node_mut(selected_id) {
                        if let XrdsSceneNodePayload::Text(t) = &mut n.payload {
                            *t = new_text;
                        }
                    }
                });
                editor_state.needs_full_reimport = true;
            }
        });
}

// ── Texture slots ─────────────────────────────────────────────────────────────

fn texture_slots_section(
    ui: &mut egui::Ui,
    mat: &xrds::scene_graph::XrdsSceneMaterial,
    node_id: xrds::scene_graph::XrdsSceneNodeId,
    editor_state: &mut EditorState,
    session: &mut EditorSession,
) {
    ui.separator();
    ui.label(egui::RichText::new("Textures").small().color(egui::Color32::GRAY));

    let slots: &[(XrdsSceneMaterialTextureSlotKind, &str, &Option<XrdsSceneTextureRef>)] = &[
        (XrdsSceneMaterialTextureSlotKind::BaseColor,         "Base Color",   &mat.textures.base_color),
        (XrdsSceneMaterialTextureSlotKind::MetallicRoughness, "Metal/Rough",  &mat.textures.metallic_roughness),
        (XrdsSceneMaterialTextureSlotKind::Normal,            "Normal",       &mat.textures.normal),
        (XrdsSceneMaterialTextureSlotKind::Occlusion,         "Occlusion",    &mat.textures.occlusion),
        (XrdsSceneMaterialTextureSlotKind::Emissive,          "Emissive",     &mat.textures.emissive),
    ];

    for (slot_kind, slot_label, current_ref) in slots {
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(*slot_label).monospace().small());
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // Clear button — only shown when a texture is assigned.
                if current_ref.is_some() {
                    if ui.small_button("✕").on_hover_text("Clear texture").clicked() {
                        let _ = session.session.set_node_material_texture(node_id, *slot_kind, None);
                        editor_state.needs_full_reimport = true;
                    }
                }
                // Pick button — queues a background dialog to avoid dispatch_sync deadlock on macOS.
                if ui.small_button("…").on_hover_text("Pick texture file").clicked() {
                    let title = format!("Pick texture — {slot_label}");
                    spawn_file_dialog(editor_state, PendingFileOpKind::PickTexture {
                        node_id,
                        slot_kind: *slot_kind,
                    }, move || {
                        rfd::FileDialog::new()
                            .add_filter("Textures", &["png", "jpg", "jpeg", "ktx2", "dds"])
                            .set_title(title)
                            .pick_file()
                    });
                }
                // Current assignment label — filename from URI, or dash.
                let label = current_ref.as_ref()
                    .map(|r| {
                        // Show just the filename from the URI, or the asset ID if no path separator.
                        let id = &r.texture_asset_id;
                        std::path::Path::new(id)
                            .file_name()
                            .and_then(|f| f.to_str())
                            .unwrap_or(id.as_str())
                            .to_string()
                    })
                    .unwrap_or_else(|| "—".to_string());
                ui.label(egui::RichText::new(label).small().color(
                    if current_ref.is_some() {
                        egui::Color32::from_rgb(140, 210, 140)
                    } else {
                        egui::Color32::GRAY
                    },
                ));
            });
        });
    }
}

// ── Alpha mode helpers ────────────────────────────────────────────────────────

fn alpha_mode_label(mode: SdkAlphaMode) -> &'static str {
    match mode {
        SdkAlphaMode::Auto   => "Auto",
        SdkAlphaMode::Opaque => "Opaque",
        SdkAlphaMode::Mask   => "Mask",
        SdkAlphaMode::Blend  => "Blend",
    }
}

fn scene_alpha_to_sdk(mode: XrdsSceneMaterialAlphaMode) -> SdkAlphaMode {
    match mode {
        XrdsSceneMaterialAlphaMode::Auto   => SdkAlphaMode::Auto,
        XrdsSceneMaterialAlphaMode::Opaque => SdkAlphaMode::Opaque,
        XrdsSceneMaterialAlphaMode::Mask   => SdkAlphaMode::Mask,
        XrdsSceneMaterialAlphaMode::Blend  => SdkAlphaMode::Blend,
    }
}

fn sdk_alpha_to_scene(mode: SdkAlphaMode) -> XrdsSceneMaterialAlphaMode {
    match mode {
        SdkAlphaMode::Auto   => XrdsSceneMaterialAlphaMode::Auto,
        SdkAlphaMode::Opaque => XrdsSceneMaterialAlphaMode::Opaque,
        SdkAlphaMode::Mask   => XrdsSceneMaterialAlphaMode::Mask,
        SdkAlphaMode::Blend  => XrdsSceneMaterialAlphaMode::Blend,
    }
}

// ── Scene metadata section ────────────────────────────────────────────────────

fn scene_metadata_section(ui: &mut egui::Ui, session: &mut EditorSession, state: &mut EditorState) {
    egui::CollapsingHeader::new("Scene")
        .default_open(false)
        .show(ui, |ui| {
            let doc = session.document();
            let doc_name   = doc.metadata.name.clone();
            let doc_author = doc.metadata.authored_by.clone().unwrap_or_default();
            drop(doc);

            // ── Scene name ────────────────────────────────────────────────────
            if state.editing_scene_name.as_deref() != Some(&doc_name)
                && !ui.ctx().memory(|m| {
                    m.has_focus(egui::Id::new("scene_meta_name"))
                })
            {
                state.editing_scene_name = Some(doc_name.clone());
            }
            ui.horizontal(|ui| {
                ui.label("Name");
                let mut buf = state.editing_scene_name.clone().unwrap_or_else(|| doc_name.clone());
                let resp = ui.add(
                    egui::TextEdit::singleline(&mut buf)
                        .id(egui::Id::new("scene_meta_name"))
                        .desired_width(ui.available_width()),
                );
                state.editing_scene_name = Some(buf.clone());
                if resp.lost_focus() || (resp.has_focus() && ui.ctx().input(|i| i.key_pressed(egui::Key::Enter))) {
                    if buf != doc_name {
                        session.session.edit(|doc| doc.metadata.name = buf.clone());
                    }
                }
            });

            // ── Author ────────────────────────────────────────────────────────
            if state.editing_scene_author.as_deref() != Some(&doc_author)
                && !ui.ctx().memory(|m| {
                    m.has_focus(egui::Id::new("scene_meta_author"))
                })
            {
                state.editing_scene_author = Some(doc_author.clone());
            }
            ui.horizontal(|ui| {
                ui.label("Author");
                let mut buf = state.editing_scene_author.clone().unwrap_or_else(|| doc_author.clone());
                let resp = ui.add(
                    egui::TextEdit::singleline(&mut buf)
                        .id(egui::Id::new("scene_meta_author"))
                        .desired_width(ui.available_width()),
                );
                state.editing_scene_author = Some(buf.clone());
                if resp.lost_focus() || (resp.has_focus() && ui.ctx().input(|i| i.key_pressed(egui::Key::Enter))) {
                    let new_val = if buf.is_empty() { None } else { Some(buf.clone()) };
                    if new_val.as_deref() != session.document().metadata.authored_by.as_deref() {
                        session.session.edit(|doc| doc.metadata.authored_by = new_val);
                    }
                }
            });

            // ── XR blend mode (VR / AR passthrough) ──────────────────────────
            let blend_mode = session.document().metadata.xr_blend_mode;
            let mut ar = blend_mode == XrdsXrBlendMode::AlphaBlend;
            ui.horizontal(|ui| {
                ui.label("AR Passthrough");
                if ui.checkbox(&mut ar, "").changed() {
                    let new_mode = if ar { XrdsXrBlendMode::AlphaBlend } else { XrdsXrBlendMode::Opaque };
                    session.session.edit(|doc| doc.metadata.xr_blend_mode = new_mode);
                }
            });
        });
}

// ── Scene environment section ─────────────────────────────────────────────────

fn scene_environment_section(ui: &mut egui::Ui, session: &mut EditorSession) {
    egui::CollapsingHeader::new("Scene Environment")
        .default_open(false)
        .show(ui, |ui| {
            let doc = session.document();
            let fog   = doc.fog_environment().cloned();
            let exp   = doc.exposure_environment().cloned();
            drop(doc);

            // ── Exposure ─────────────────────────────────────────────────────
            let mut exp_enabled = exp.is_some();
            let mut ev100 = exp.as_ref().map(|e| e.ev100).unwrap_or(9.7); // ~sunny
            ui.horizontal(|ui| {
                if ui.checkbox(&mut exp_enabled, "Exposure").changed() {
                    if exp_enabled {
                        session.session.edit(|doc| {
                            let _ = doc.set_exposure_environment(ev100);
                        });
                    } else {
                        session.session.edit(|doc| {
                            doc.clear_exposure_environment();
                        });
                    }
                }
            });
            if exp_enabled {
                if ui
                    .add(egui::DragValue::new(&mut ev100).speed(0.1).range(-10.0..=20.0).suffix(" EV100"))
                    .changed()
                {
                    session.session.edit(|doc| { let _ = doc.set_exposure_environment(ev100); });
                }
            }

            ui.separator();

            // ── Fog ───────────────────────────────────────────────────────────
            let mut fog_enabled = fog.is_some();
            let mut fog_color = fog.as_ref().map(|f| f.color).unwrap_or([0.7, 0.8, 0.9, 1.0]);
            let mut fog_start = fog.as_ref().map(|f| f.start).unwrap_or(10.0);
            let mut fog_end   = fog.as_ref().map(|f| f.end).unwrap_or(50.0);

            ui.horizontal(|ui| {
                if ui.checkbox(&mut fog_enabled, "Fog").changed() {
                    if fog_enabled {
                        session.session.edit(|doc| {
                            let _ = doc.set_fog_environment(fog_color, fog_start, fog_end);
                        });
                    } else {
                        session.session.edit(|doc| { doc.clear_fog_environment(); });
                    }
                }
            });
            if fog_enabled {
                let mut changed = false;
                changed |= color_row(ui, "Fog color", &mut fog_color);
                changed |= ui
                    .add(egui::DragValue::new(&mut fog_start).speed(0.5).range(0.0..=fog_end).suffix(" m start"))
                    .changed();
                changed |= ui
                    .add(egui::DragValue::new(&mut fog_end).speed(0.5).range(fog_start..=10_000.0).suffix(" m end"))
                    .changed();
                if changed {
                    session.session.edit(|doc| {
                        let _ = doc.set_fog_environment(fog_color, fog_start, fog_end);
                    });
                }
            }
        });
}

// ── GLB animation section ─────────────────────────────────────────────────────

fn gltf_animation_section(
    ui: &mut egui::Ui,
    state: &mut EditorState,
    node_id: xrds::scene_graph::XrdsSceneNodeId,
) {
    let clips = state.gltf_clips.get(&node_id).cloned().unwrap_or_default();
    let anim_state = state.gltf_anim_state.get(&node_id).and_then(|s| s.clone());

    egui::CollapsingHeader::new("Animation")
        .default_open(true)
        .show(ui, |ui| {
            if clips.is_empty() {
                ui.colored_label(
                    egui::Color32::GRAY,
                    if state.gltf_clips.contains_key(&node_id) {
                        "No animations"
                    } else {
                        "Loading…"
                    },
                );
                return;
            }

            // ── Clip list ─────────────────────────────────────────────────────
            let playing_index = anim_state.as_ref().map(|s| s.animation.index);
            for clip in &clips {
                let label = clip.name.clone().unwrap_or_else(|| format!("Animation {}", clip.index));
                let duration = String::new();
                let is_current = playing_index == Some(clip.index);

                ui.horizontal(|ui| {
                    let btn = egui::Button::new("▶").small().selected(is_current);
                    if ui.add(btn).on_hover_text("Play this clip").clicked() {
                        let speed = anim_state.as_ref().map(|s| s.speed).unwrap_or(1.0);
                        let repeat = anim_state.as_ref().map(|s| s.repeat).unwrap_or(XrdsAnimationRepeatMode::Loop);
                        state.pending_gltf_play = Some((node_id, clip.index, speed, repeat));
                    }
                    let color = if is_current {
                        egui::Color32::from_rgb(100, 210, 120)
                    } else {
                        ui.visuals().text_color()
                    };
                    ui.colored_label(color, format!("{label}{duration}"));
                });
            }

            // ── Playback controls ─────────────────────────────────────────────
            ui.separator();
            if let Some(ref st) = anim_state {
                let clip_name = st.animation.name.clone()
                    .unwrap_or_else(|| format!("Animation {}", st.animation.index));

                // Speed drag (restart required — shown always)
                ui.horizontal(|ui| {
                    ui.label("Speed");
                    let mut speed = st.speed;
                    if ui.add(
                        egui::DragValue::new(&mut speed)
                            .speed(0.01)
                            .range(0.01..=10.0)
                            .fixed_decimals(2),
                    ).changed() && (speed - st.speed).abs() > 0.001 {
                        state.pending_gltf_play = Some((
                            node_id,
                            st.animation.index,
                            speed,
                            st.repeat,
                        ));
                    }

                    // Loop toggle
                    let mut looping = st.repeat == XrdsAnimationRepeatMode::Loop;
                    if ui.checkbox(&mut looping, "Loop").changed() {
                        let new_repeat = if looping {
                            XrdsAnimationRepeatMode::Loop
                        } else {
                            XrdsAnimationRepeatMode::Once
                        };
                        state.pending_gltf_play = Some((node_id, st.animation.index, st.speed, new_repeat));
                    }
                });

                ui.horizontal(|ui| {
                    if st.paused {
                        if ui.button("▶ Resume").on_hover_text(&clip_name).clicked() {
                            state.pending_gltf_resume = Some(node_id);
                        }
                    } else {
                        if ui.button("⏸ Pause").on_hover_text(&clip_name).clicked() {
                            state.pending_gltf_pause = Some(node_id);
                        }
                    }
                    if ui.button("⏹ Stop").clicked() {
                        state.pending_gltf_stop = Some(node_id);
                    }
                    ui.colored_label(
                        egui::Color32::GRAY,
                        egui::RichText::new(&clip_name).small(),
                    );
                });
            } else {
                ui.colored_label(egui::Color32::GRAY, "Stopped");
            }
        });
}
