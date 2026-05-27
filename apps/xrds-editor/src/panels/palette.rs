use std::sync::Arc;
use xrds::editor::egui;
use xrds::scene_graph::{
    XrdsEditorMetadata, XrdsGltfAssetExportPolicy, XrdsSceneAmbientLight,
    XrdsSceneAssetDiagnostics, XrdsSceneAssetKind, XrdsSceneAssetRemovalPolicy, XrdsSceneAudioClip,
    XrdsSceneCamera, XrdsSceneCameraProjection, XrdsSceneCube, XrdsSceneCylinder,
    XrdsSceneDirectionalLight, XrdsSceneGltfPlacement, XrdsSceneHudText, XrdsSceneInteractionZone,
    XrdsSceneMaterial, XrdsSceneNode, XrdsSceneNodeId, XrdsSceneNodePayload, XrdsScenePlane3D,
    XrdsScenePlayerSpawn, XrdsScenePointLight, XrdsSceneSphere, XrdsSceneSpotLight,
    XrdsSceneTetrahedron, XrdsSceneText, XrdsSceneTransform,
};

use crate::icon::IconName;
use crate::io::import_asset;
use crate::state::{EditorSession, EditorState};

// ── Drag-and-drop payload ─────────────────────────────────────────────────────

/// Payload carried while dragging an item from the palette to the hierarchy.
#[derive(Clone, Debug)]
pub enum PaletteDragPayload {
    /// A built-in primitive identified by its label string.
    Primitive { label: &'static str },
    /// A catalog asset by id and kind.
    Asset {
        id: String,
        kind: XrdsSceneAssetKind,
    },
}

/// Reconstruct a default scene-node payload from a primitive label.
pub fn primitive_payload_from_label(label: &str) -> Option<XrdsSceneNodePayload> {
    match label {
        "Empty" => Some(XrdsSceneNodePayload::Empty),
        "Cube" => Some(XrdsSceneNodePayload::Cube(cube_default())),
        "Sphere" => Some(XrdsSceneNodePayload::Sphere(sphere_default())),
        "Cylinder" => Some(XrdsSceneNodePayload::Cylinder(cylinder_default())),
        "Plane" => Some(XrdsSceneNodePayload::Plane3D(plane_default())),
        "Tetrahedron" => Some(XrdsSceneNodePayload::Tetrahedron(tetra_default())),
        "Camera" => Some(XrdsSceneNodePayload::Camera(camera_default())),
        "Point Light" => Some(XrdsSceneNodePayload::PointLight(point_light_default())),
        "Spot Light" => Some(XrdsSceneNodePayload::SpotLight(spot_light_default())),
        "Dir. Light" => Some(XrdsSceneNodePayload::DirectionalLight(dir_light_default())),
        "Audio Clip" => Some(XrdsSceneNodePayload::AudioClip(audio_clip_default())),
        "Ambient Light" => Some(XrdsSceneNodePayload::AmbientLight(ambient_light_default())),
        "Interaction Zone" => Some(XrdsSceneNodePayload::InteractionZone(
            XrdsSceneInteractionZone::default(),
        )),
        "Player Spawn" => Some(XrdsSceneNodePayload::PlayerSpawn(
            XrdsScenePlayerSpawn::default(),
        )),
        "HUD Text" => Some(XrdsSceneNodePayload::HudText(XrdsSceneHudText::default())),
        "Text 3D" => Some(XrdsSceneNodePayload::Text(XrdsSceneText::default())),
        _ => None,
    }
}

/// Apply a drag-and-drop from the palette onto a hierarchy target.
/// `parent_id = None` places the node at scene root.
pub fn apply_palette_drop(
    session: &mut EditorSession,
    editor_state: &mut EditorState,
    parent_id: Option<XrdsSceneNodeId>,
    payload: Arc<PaletteDragPayload>,
) {
    match payload.as_ref() {
        PaletteDragPayload::Primitive { label } => {
            if let Some(node_payload) = primitive_payload_from_label(label) {
                add_node(session, editor_state, node_payload, label, parent_id);
            }
        }
        PaletteDragPayload::Asset { id, kind } => {
            place_asset(session, editor_state, id, *kind, parent_id);
        }
    }
}

// ── Panel ─────────────────────────────────────────────────────────────────────

pub fn palette_panel(
    ctx: &mut egui::Context,
    session: &mut EditorSession,
    editor_state: &mut EditorState,
) {
    egui::TopBottomPanel::bottom("palette")
        .resizable(true)
        .default_height(320.0)
        .min_height(160.0)
        .show(ctx, |ui| {
            // ── Tab bar ───────────────────────────────────────────────────────
            ui.horizontal(|ui| {
                ui.selectable_value(&mut editor_state.palette_tab, 0, "Primitives");
                ui.selectable_value(&mut editor_state.palette_tab, 1, "Project Assets");
            });
            ui.separator();

            match editor_state.palette_tab {
                0 => primitives_tab(ui, session, editor_state),
                _ => project_assets_tab(ui, session, editor_state),
            }
        });
}

// ── Primitives tab ────────────────────────────────────────────────────────────

fn primitives_tab(ui: &mut egui::Ui, session: &mut EditorSession, editor_state: &mut EditorState) {
    // Groups of related primitives — (icon, label, builder)
    let groups: &[&[(Option<IconName>, &str, NodeBuilder)]] = &[
        // Geometry
        &[
            (
                Some(IconName::Empty),
                "Empty",
                Box::new(|| XrdsSceneNodePayload::Empty),
            ),
            (
                Some(IconName::Cube),
                "Cube",
                Box::new(|| XrdsSceneNodePayload::Cube(cube_default())),
            ),
            (
                Some(IconName::Sphere),
                "Sphere",
                Box::new(|| XrdsSceneNodePayload::Sphere(sphere_default())),
            ),
            (
                Some(IconName::Cylinder),
                "Cylinder",
                Box::new(|| XrdsSceneNodePayload::Cylinder(cylinder_default())),
            ),
            (
                Some(IconName::Plane),
                "Plane",
                Box::new(|| XrdsSceneNodePayload::Plane3D(plane_default())),
            ),
            (
                None,
                "Tetrahedron",
                Box::new(|| XrdsSceneNodePayload::Tetrahedron(tetra_default())),
            ),
        ],
        // Scene objects
        &[
            (
                Some(IconName::Camera),
                "Camera",
                Box::new(|| XrdsSceneNodePayload::Camera(camera_default())),
            ),
            (
                Some(IconName::PointLight),
                "Point Light",
                Box::new(|| XrdsSceneNodePayload::PointLight(point_light_default())),
            ),
            (
                Some(IconName::SpotLight),
                "Spot Light",
                Box::new(|| XrdsSceneNodePayload::SpotLight(spot_light_default())),
            ),
            (
                Some(IconName::DirectionalLight),
                "Dir. Light",
                Box::new(|| XrdsSceneNodePayload::DirectionalLight(dir_light_default())),
            ),
            (
                None,
                "Audio Clip",
                Box::new(|| XrdsSceneNodePayload::AudioClip(audio_clip_default())),
            ),
            (
                Some(IconName::AmbientLight),
                "Ambient Light",
                Box::new(|| XrdsSceneNodePayload::AmbientLight(ambient_light_default())),
            ),
            (
                None,
                "Interaction Zone",
                Box::new(|| {
                    XrdsSceneNodePayload::InteractionZone(XrdsSceneInteractionZone::default())
                }),
            ),
            (
                None,
                "Player Spawn",
                Box::new(|| {
                    XrdsSceneNodePayload::PlayerSpawn(XrdsScenePlayerSpawn::default())
                }),
            ),
            (
                None,
                "HUD Text",
                Box::new(|| XrdsSceneNodePayload::HudText(XrdsSceneHudText::default())),
            ),
            (
                None,
                "Text 3D",
                Box::new(|| XrdsSceneNodePayload::Text(XrdsSceneText::default())),
            ),
        ],
    ];

    // Both axes: primitives scroll horizontally if the panel is narrow,
    // and vertically if the panel is shorter than a single button row.
    egui::ScrollArea::both()
        .id_salt("primitives_scroll")
        .show(ui, |ui| {
            ui.horizontal_wrapped(|ui| {
                for (gi, group) in groups.iter().enumerate() {
                    if gi > 0 {
                        ui.add(egui::Separator::default().vertical());
                    }
                    for (icon, label, ref builder) in *group {
                        let btn = if let Some(name) = icon {
                            let tex = editor_state.icon_cache.load_large(ui.ctx(), *name, 40);
                            let sized =
                                egui::load::SizedTexture::new(tex.id(), egui::vec2(40.0, 40.0));
                            let img = egui::Image::new(sized).max_size(egui::vec2(40.0, 40.0));
                            egui::Button::new(img).min_size(egui::vec2(40.0, 40.0))
                        } else {
                            // Emoji fallback for icons without SVG
                            egui::Button::new(format!("  {label}")).min_size(egui::vec2(40.0, 40.0))
                        };
                        let resp = ui.add(btn.sense(egui::Sense::click_and_drag()));

                        // Set drag payload for DnD to hierarchy.
                        resp.dnd_set_drag_payload(PaletteDragPayload::Primitive { label });

                        // Tooltip so users know what each icon means.
                        let resp = resp.on_hover_text(*label);

                        // Double-click places at scene root immediately.
                        if resp.double_clicked() {
                            add_node(session, editor_state, builder(), label, None);
                        }
                    }
                }
            });
        });
}

// ── Project assets tab ────────────────────────────────────────────────────────

fn project_assets_tab(
    ui: &mut egui::Ui,
    session: &mut EditorSession,
    editor_state: &mut EditorState,
) {
    // ── Search / import bar ───────────────────────────────────────────────────
    // Resolve import click before drawing (avoids simultaneous &mut borrows).
    let mut do_import = false;
    ui.horizontal(|ui| {
        ui.label("🔍");
        ui.add(
            egui::TextEdit::singleline(&mut editor_state.asset_search)
                .hint_text("filter…")
                .desired_width(140.0),
        );
        if !editor_state.asset_search.is_empty() && ui.small_button("✕").clicked() {
            editor_state.asset_search.clear();
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button("＋ Import").clicked() {
                do_import = true;
            }
        });
    });
    if do_import {
        import_asset(session, editor_state);
    }

    // Collect all asset data before the egui pass so we don't hold a
    // `session.document()` borrow inside closures that also need &mut session.
    let (asset_rows, is_empty) = {
        let doc = session.document();
        let diag = doc.asset_diagnostics();
        let filter = editor_state.asset_search.to_lowercase();

        let rows: Vec<(
            XrdsSceneAssetKind,
            String,
            String,
            usize,
            bool,
            &'static str,
            egui::Color32,
        )> = doc
            .assets
            .iter()
            .filter(|a| {
                filter.is_empty()
                    || a.id.to_lowercase().contains(&filter)
                    || a.uri.to_lowercase().contains(&filter)
            })
            .map(|a| {
                let usage = diag
                    .asset_usages
                    .iter()
                    .find(|u| u.asset.id == a.id)
                    .map(|u| u.referenced_node_ids.len())
                    .unwrap_or(0);
                let is_unused = diag.unused_asset_ids.contains(&a.id);
                let (health_icon, health_color) = asset_health(&diag, &a.id, a.kind);
                (
                    a.kind,
                    a.id.clone(),
                    a.uri.clone(),
                    usage,
                    is_unused,
                    health_icon,
                    health_color,
                )
            })
            .collect();
        let empty = doc.assets.is_empty();
        (rows, empty)
    };

    if is_empty {
        ui.add_space(6.0);
        ui.colored_label(
            egui::Color32::GRAY,
            "No assets in catalog.  Use ＋ Import to add files.",
        );
        return;
    }

    ui.separator();

    let groups: &[(IconName, &str, XrdsSceneAssetKind)] = &[
        (IconName::GltfAsset, "glTF / GLB", XrdsSceneAssetKind::Gltf),
        (IconName::Texture, "Textures", XrdsSceneAssetKind::Texture),
        (
            IconName::EnvironmentMap,
            "Environment Maps",
            XrdsSceneAssetKind::EnvironmentMap,
        ),
        (IconName::AudioClip, "Audio", XrdsSceneAssetKind::Audio),
    ];

    // Action queued inside the scroll area, applied after (avoids double-borrow).
    let mut pending_place: Option<(String, XrdsSceneAssetKind)> = None;
    let mut pending_remove: Option<String> = None;

    egui::ScrollArea::vertical()
        .id_salt("assets_scroll")
        .show(ui, |ui| {
            for &(icon, group_label, kind) in groups {
                let assets: Vec<_> = asset_rows.iter().filter(|(k, ..)| *k == kind).collect();

                if assets.is_empty() {
                    continue;
                }

                egui::CollapsingHeader::new(egui::RichText::new(format!(
                    "{}  {group_label}  ({})",
                    icon.emoji_fallback(),
                    assets.len()
                )))
                .default_open(true)
                .show(ui, |ui| {
                    ui.horizontal_wrapped(|ui| {
                        for (
                            _,
                            asset_id,
                            asset_uri,
                            usage_count,
                            is_unused,
                            _health_icon,
                            health_color,
                        ) in &assets
                        {
                            // ── Asset card ────────────────────────────────────────
                            let card_w = 78.0_f32;
                            let card_h = 72.0_f32;
                            let (rect, resp) = ui.allocate_exact_size(
                                egui::vec2(card_w, card_h),
                                egui::Sense::click_and_drag(),
                            );

                            let hovered = resp.hovered();
                            let bg = kind_card_color(kind, hovered);
                            let painter = ui.painter_at(rect);

                            // Card background
                            painter.rect_filled(rect, 6.0, bg);
                            painter.rect_stroke(
                                rect,
                                6.0,
                                egui::Stroke::new(1.0, egui::Color32::from_white_alpha(30)),
                                egui::StrokeKind::Inside,
                            );

                            // Large kind icon, upper half — painted as SVG texture
                            let icon_center =
                                egui::pos2(rect.center().x, rect.top() + card_h * 0.38);
                            let icon = match kind {
                                XrdsSceneAssetKind::Gltf => IconName::GltfAsset,
                                XrdsSceneAssetKind::Texture => IconName::Texture,
                                XrdsSceneAssetKind::EnvironmentMap => IconName::EnvironmentMap,
                                XrdsSceneAssetKind::Audio => IconName::AudioClip,
                            };
                            editor_state.icon_cache.paint_large_icon(
                                ui.ctx(),
                                &painter,
                                icon,
                                icon_center,
                                card_h * 0.5,
                                egui::Color32::WHITE,
                            );

                            // Name, bottom of card — truncate to fit
                            let display_name =
                                asset_id.strip_prefix("asset:").unwrap_or(asset_id.as_str());
                            let truncated: String = if display_name.len() > 9 {
                                format!("{}…", &display_name[..8])
                            } else {
                                display_name.to_string()
                            };
                            let name_color = if *is_unused {
                                egui::Color32::from_rgba_unmultiplied(255, 255, 255, 110)
                            } else {
                                egui::Color32::WHITE
                            };
                            painter.text(
                                egui::pos2(rect.center().x, rect.bottom() - 7.0),
                                egui::Align2::CENTER_BOTTOM,
                                &truncated,
                                egui::FontId::proportional(10.0),
                                name_color,
                            );

                            // Health dot — top-right corner
                            painter.circle_filled(
                                egui::pos2(rect.right() - 7.0, rect.top() + 7.0),
                                4.0,
                                *health_color,
                            );

                            // Usage badge — top-left
                            if *usage_count > 0 {
                                painter.text(
                                    egui::pos2(rect.left() + 6.0, rect.top() + 5.0),
                                    egui::Align2::LEFT_TOP,
                                    format!("{usage_count}×"),
                                    egui::FontId::proportional(9.0),
                                    egui::Color32::from_white_alpha(180),
                                );
                            }

                            // Check interactions before consuming resp with hover/menu.
                            let double_clicked = resp.double_clicked();

                            // Tooltip: full id + uri
                            let resp = resp.on_hover_ui(|ui| {
                                ui.label(egui::RichText::new(asset_id).strong());
                                ui.colored_label(egui::Color32::GRAY, asset_uri);
                                ui.label(
                                    egui::RichText::new(
                                        "Double-click to place  •  Right-click for options",
                                    )
                                    .small()
                                    .italics(),
                                );
                            });

                            // Drag to hierarchy panel to place as a child node.
                            resp.dnd_set_drag_payload(PaletteDragPayload::Asset {
                                id: asset_id.clone(),
                                kind,
                            });

                            if double_clicked {
                                pending_place = Some((asset_id.clone(), kind));
                            }

                            // Right-click context menu
                            resp.context_menu(|ui| {
                                ui.label(egui::RichText::new(asset_id).strong().small());
                                ui.separator();
                                if ui.button("▶  Place in Scene").clicked() {
                                    pending_place = Some((asset_id.clone(), kind));
                                    ui.close_menu();
                                }
                                ui.separator();
                                if ui.button("🗑  Remove from Catalog").clicked() {
                                    pending_remove = Some(asset_id.clone());
                                    ui.close_menu();
                                }
                            });
                        }
                    });
                });
            }
        }); // end ScrollArea

    // Apply queued actions after the egui pass (avoids simultaneous borrows).
    if let Some((id, kind)) = pending_place {
        place_asset(session, editor_state, &id, kind, None);
        println!("Placing asset '{id}' of kind {kind:?} at scene root");
    }
    if let Some(id) = pending_remove {
        match session
            .session
            .remove_asset(&id, XrdsSceneAssetRemovalPolicy::DetachReferencingNodes)
        {
            Ok(r) => {
                let d = r.detached_node_ids.len();
                editor_state.status_message = Some(if d > 0 {
                    format!("Removed '{id}' ({d} node(s) detached)")
                } else {
                    format!("Removed '{id}'")
                });
            }
            Err(e) => editor_state.status_message = Some(format!("Remove failed: {e:?}")),
        }
    }
}

/// Background fill colour for an asset card, slightly lighter on hover.
fn kind_card_color(kind: XrdsSceneAssetKind, hovered: bool) -> egui::Color32 {
    let base = match kind {
        XrdsSceneAssetKind::Gltf => egui::Color32::from_rgb(40, 80, 110),
        XrdsSceneAssetKind::Texture => egui::Color32::from_rgb(80, 50, 110),
        XrdsSceneAssetKind::EnvironmentMap => egui::Color32::from_rgb(100, 70, 30),
        XrdsSceneAssetKind::Audio => egui::Color32::from_rgb(30, 90, 60),
    };
    if hovered {
        egui::Color32::from_rgb(
            (base.r() as u16 + 30).min(255) as u8,
            (base.g() as u16 + 30).min(255) as u8,
            (base.b() as u16 + 30).min(255) as u8,
        )
    } else {
        base
    }
}

/// Place a catalog asset as a new scene node.
/// glTF → GltfAsset node via `place_gltf_asset`.
/// Audio → AudioClip node.
/// Texture / EnvironmentMap → status message (no placeable scene node type).
fn place_asset(
    session: &mut EditorSession,
    editor_state: &mut EditorState,
    asset_id: &str,
    kind: XrdsSceneAssetKind,
    parent_id: Option<XrdsSceneNodeId>,
) {
    // Allocate a unique node id.
    let new_id = {
        let doc = session.document();
        let max = doc.nodes.iter().map(|n| n.id.0).max().unwrap_or(0);
        XrdsSceneNodeId(max + 1)
    };

    // Human-readable name: strip the "asset:" prefix if present.
    let name = asset_id
        .strip_prefix("asset:")
        .unwrap_or(asset_id)
        .to_string();

    match kind {
        XrdsSceneAssetKind::Gltf => {
            let placement = XrdsSceneGltfPlacement {
                asset_id: asset_id.to_string(),
                node_id: Some(new_id),
                parent_id,
                name: name.clone(),
                enabled: true,
                visible: true,
                transform: XrdsSceneTransform::default(),
                scene_index: 0,
                export_policy: XrdsGltfAssetExportPolicy::KeepExternalReference,
                editor: XrdsEditorMetadata::default(),
            };
            match session.session.place_gltf_asset(placement) {
                Ok(placed_id) => {
                    editor_state.selection.set_single(placed_id);
                    // Incremental spawn: add only this new node to the Bevy world
                    // without despawning and re-importing the entire scene.
                    editor_state.pending_node_spawns.push(placed_id);
                    editor_state.status_message = Some(format!("Placed '{name}' in scene"));
                }
                Err(e) => {
                    editor_state.status_message = Some(format!("Place failed: {e:?}"));
                }
            }
        }
        XrdsSceneAssetKind::Audio => {
            let node = XrdsSceneNode {
                id: new_id,
                parent_id,
                name: name.clone(),
                enabled: true,
                visible: true,
                transform: XrdsSceneTransform::default(),
                payload: XrdsSceneNodePayload::AudioClip(XrdsSceneAudioClip {
                    asset_id: asset_id.to_string(),
                    ..Default::default()
                }),
                editor: XrdsEditorMetadata::default(),
            };
            let _ = session.session.edit(|doc| {
                doc.nodes.push(node);
            });
            editor_state.selection.set_single(new_id);
            editor_state.needs_full_reimport = true;
            editor_state.status_message = Some(format!("Placed audio '{name}'"));
        }
        XrdsSceneAssetKind::Texture | XrdsSceneAssetKind::EnvironmentMap => {
            editor_state.status_message = Some(
                "Textures and environment maps are applied via the material inspector, not placed as nodes."
                    .to_string(),
            );
        }
    }
}

/// Returns (icon, color) for a health badge given the diagnostics and asset id.
fn asset_health(
    diag: &XrdsSceneAssetDiagnostics,
    id: &str,
    kind: XrdsSceneAssetKind,
) -> (&'static str, egui::Color32) {
    let ok = egui::Color32::from_rgb(80, 180, 80);
    let warn = egui::Color32::from_rgb(220, 170, 50);
    let error = egui::Color32::from_rgb(200, 70, 70);

    let (valid_ids, invalid_ids): (&[String], &[String]) = match kind {
        XrdsSceneAssetKind::Gltf => (&[], &[]), // glTF is node-level, not id-level
        XrdsSceneAssetKind::Texture => (
            &diag.valid_texture_asset_ids,
            &diag.invalid_texture_asset_ids,
        ),
        XrdsSceneAssetKind::EnvironmentMap => (
            &diag.valid_environment_map_asset_ids,
            &diag.invalid_environment_map_asset_ids,
        ),
        XrdsSceneAssetKind::Audio => (&diag.valid_audio_asset_ids, &diag.invalid_audio_asset_ids),
    };

    if invalid_ids.iter().any(|i| i == id) {
        return ("⚠", warn);
    }
    if valid_ids.iter().any(|i| i == id) || kind == XrdsSceneAssetKind::Gltf {
        return ("✓", ok);
    }
    // Not mentioned in either list — unknown / unchecked
    ("·", error)
}

// ── Node insertion ────────────────────────────────────────────────────────────

type NodeBuilder = Box<dyn Fn() -> XrdsSceneNodePayload>;

fn add_node(
    session: &mut EditorSession,
    editor_state: &mut EditorState,
    payload: XrdsSceneNodePayload,
    label: &str,
    parent_id: Option<XrdsSceneNodeId>,
) {
    // Allocate a unique ID: max existing + 1
    let new_id = {
        let doc = session.document();
        let max = doc.nodes.iter().map(|n| n.id.0).max().unwrap_or(0);
        XrdsSceneNodeId(max + 1)
    };

    let new_node = XrdsSceneNode {
        id: new_id,
        parent_id,
        name: label.to_string(),
        enabled: true,
        visible: true,
        transform: XrdsSceneTransform::default(), // place at origin
        payload,
        editor: XrdsEditorMetadata::default(),
    };

    let result = session.session.edit(|doc| {
        doc.nodes.push(new_node);
    });

    if result.is_ok() {
        editor_state.selection.set_single(new_id);
        editor_state.editing_name = None;
        // Trigger full reimport so the new node appears in the 3D viewport.
        editor_state.needs_full_reimport = true;
        editor_state.status_message = Some(format!("Added '{label}'"));
    } else {
        editor_state.status_message = Some(format!("Failed to add '{label}'"));
    }
}

// ── Default descriptors ───────────────────────────────────────────────────────

fn cube_default() -> XrdsSceneCube {
    XrdsSceneCube {
        size: [1.0, 1.0, 1.0],
        material: XrdsSceneMaterial::default(),
    }
}
fn sphere_default() -> XrdsSceneSphere {
    XrdsSceneSphere {
        radius: 0.5,
        material: XrdsSceneMaterial::default(),
    }
}
fn cylinder_default() -> XrdsSceneCylinder {
    XrdsSceneCylinder {
        radius: 0.5,
        height: 1.0,
        material: XrdsSceneMaterial::default(),
    }
}
fn plane_default() -> XrdsScenePlane3D {
    XrdsScenePlane3D {
        size: [2.0, 2.0],
        material: XrdsSceneMaterial::default(),
    }
}
fn tetra_default() -> XrdsSceneTetrahedron {
    // Regular tetrahedron inscribed in unit sphere
    use std::f32::consts::PI;
    let r = 1.0_f32;
    let v0 = [0.0, r, 0.0];
    let a = 2.0 * (2.0_f32 / 3.0).sqrt() * r;
    let h = -r / 3.0;
    let v1 = [a, h, 0.0];
    let v2 = [
        -a * 60_f32.to_radians().cos(),
        h,
        a * 60_f32.to_radians().sin(),
    ];
    let v3 = [
        -a * 60_f32.to_radians().cos(),
        h,
        -a * 60_f32.to_radians().sin(),
    ];
    XrdsSceneTetrahedron {
        vertices: [v0, v1, v2, v3],
        material: XrdsSceneMaterial::default(),
    }
}
fn camera_default() -> XrdsSceneCamera {
    XrdsSceneCamera {
        projection: XrdsSceneCameraProjection::Perspective {
            fov_deg: 60.0,
            near: 0.1,
            far: Some(1000.0),
            order: 1,
        },
        look_at: None,
    }
}
fn point_light_default() -> XrdsScenePointLight {
    // 10 000 cd — clearly visible alongside the default 10 000-lux directional sun.
    XrdsScenePointLight {
        color: [1.0, 1.0, 1.0, 1.0],
        intensity: 10_000.0,
        range: 20.0,
        radius: 0.0,
        shadows: false,
    }
}
fn spot_light_default() -> XrdsSceneSpotLight {
    // 50 000 cd with a tight 25° cone — the focused beam is immediately obvious
    // even against bright ambient/directional light.
    XrdsSceneSpotLight {
        color: [1.0, 1.0, 1.0, 1.0],
        intensity: 50_000.0,
        range: 20.0,
        inner_angle: 10.0_f32.to_radians(),
        outer_angle: 25.0_f32.to_radians(),
        shadows: false,
    }
}
fn dir_light_default() -> XrdsSceneDirectionalLight {
    // 10 000 lux = bright overcast-to-sunlight illuminance.
    XrdsSceneDirectionalLight {
        color: [1.0, 1.0, 1.0, 1.0],
        illuminance: 10_000.0,
        shadows: false,
    }
}
fn audio_clip_default() -> XrdsSceneAudioClip {
    XrdsSceneAudioClip::default()
}
fn ambient_light_default() -> XrdsSceneAmbientLight {
    // Match Bevy's AmbientLight::default() brightness of 80.0 cd/m².
    XrdsSceneAmbientLight {
        color: [1.0, 1.0, 1.0, 1.0],
        brightness: 80.0,
        affects_baked_lighting: true,
    }
}
