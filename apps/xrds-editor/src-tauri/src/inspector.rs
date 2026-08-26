use bevy::math::{EulerRot, Quat};
use bevy::log::error;
use xrds_scene_graph::{
    XrdsHudAnchor, XrdsSceneDocument, XrdsSceneNodeId, XrdsSceneNodePayload,
    XrdsSceneMaterial, XrdsSceneCameraProjection, XrdsSceneTextAlignment, XrdsSceneTextAnchor,
    XrdsScenePlayerSpawnZone, XrdsSceneEffect, XrdsSceneEffectBlend, XrdsSceneEffectKind,
    XrdsAudioDistanceModel, XrdsSceneAudioClip,
    XrdsSceneWorldLayout, XrdsSceneWorldWidget,
    XrdsSceneWorldLabel, XrdsSceneWorldButton, XrdsSceneWorldImage,
    XrdsSceneWorldSlider, XrdsSceneWorldToggle,
};
use crate::bridge::{
    EditorCommand, MaterialParamsDto, NodeInspectorDto, NodePayloadDto,
    WorldLayoutDto, WorldWidgetDto,
};
use crate::editor_state::{EditorSession, EditorState};
use crate::trigger_action::{
    build_node_trigger_diagnostics_dto, build_node_triggers_dto, build_node_watchers_dto,
};

// ---------------------------------------------------------------------------
// Snapshot serializer
// ---------------------------------------------------------------------------

pub fn build_node_inspector(
    doc: &XrdsSceneDocument,
    selection: &[XrdsSceneNodeId],
    gltf_clips: &std::collections::HashMap<XrdsSceneNodeId, Vec<(usize, String)>>,
) -> Option<NodeInspectorDto> {
    let id = *selection.last()?;
    let node = doc.node(id)?;

    let t = node.transform.translation;
    let q = node.transform.rotation_quat_xyzw;
    let s = node.transform.scale;

    let parent_kind = node.parent_id
        .and_then(|pid| doc.node(pid))
        .map(|p| crate::hierarchy::payload_kind_str(&p.payload).to_owned());

    Some(NodeInspectorDto {
        id: id.0,
        name: node.name.clone(),
        visible: node.visible,
        grabbable: node.grabbable,
        parent_id: node.parent_id.map(|p| p.0),
        translation: t,
        rotation_euler_degrees: quat_to_euler_degrees(q),
        scale: s,
        payload: build_payload_dto(doc, &node.payload, id, gltf_clips),
        parent_kind,
        triggers: build_node_triggers_dto(&node.triggers),
        watchers: build_node_watchers_dto(&node.watchers),
        trigger_diagnostics: build_node_trigger_diagnostics_dto(doc, id),
    })
}

fn build_payload_dto(
    doc: &XrdsSceneDocument,
    payload: &XrdsSceneNodePayload,
    id: XrdsSceneNodeId,
    gltf_clips: &std::collections::HashMap<XrdsSceneNodeId, Vec<(usize, String)>>,
) -> NodePayloadDto {
    match payload {
        XrdsSceneNodePayload::Cube(c)       => NodePayloadDto::Cube     { material: mat_dto(&c.material), physics_body: physics_body_str(c.physics_body), gravity_scale: c.gravity_scale, mass: c.mass },
        XrdsSceneNodePayload::Sphere(c)     => NodePayloadDto::Sphere   { material: mat_dto(&c.material), physics_body: physics_body_str(c.physics_body), gravity_scale: c.gravity_scale, mass: c.mass },
        XrdsSceneNodePayload::Cylinder(c)   => NodePayloadDto::Cylinder { material: mat_dto(&c.material), physics_body: physics_body_str(c.physics_body), gravity_scale: c.gravity_scale, mass: c.mass },
        XrdsSceneNodePayload::Effect(e)     => NodePayloadDto::Effect   { kind: effect_kind_str(e.kind).to_string(), auto_play: e.auto_play, burst_count: e.burst_count, spawn_rate: e.spawn_rate, lifetime_secs: e.lifetime_secs, size_min: e.size_min, size_max: e.size_max, color_start: e.color_start, color_end: e.color_end, speed_min: e.speed_min, speed_max: e.speed_max, omnidirectional: e.omnidirectional, spread_deg: e.spread_deg, gravity: e.gravity, emission_radius: e.emission_radius, blend: effect_blend_str(e.blend).to_string(), size_end: e.size_end, drag: e.drag, fade_edge: e.fade_edge, fade_scene: e.fade_scene },
        XrdsSceneNodePayload::Capsule(c)    => NodePayloadDto::Capsule  { material: mat_dto(&c.material), physics_body: physics_body_str(c.physics_body), gravity_scale: c.gravity_scale, mass: c.mass, radius: c.radius, length: c.length },
        XrdsSceneNodePayload::AudioClip(a)  => NodePayloadDto::AudioClip {
            asset_id: a.asset_id.clone(),
            volume: a.volume,
            looped: a.looped,
            spatial: a.spatial,
            autoplay: a.autoplay,
            distance_model: audio_distance_model_str(a.distance_model).to_string(),
            min_distance: a.min_distance,
            max_distance: a.max_distance,
            rolloff_factor: a.rolloff_factor,
        },
        XrdsSceneNodePayload::Plane3D(c)    => NodePayloadDto::Plane    { material: mat_dto(&c.material), physics_body: physics_body_str(c.physics_body), gravity_scale: c.gravity_scale, mass: c.mass },
        XrdsSceneNodePayload::Tetrahedron(c)=> NodePayloadDto::Cube     { material: mat_dto(&c.material), physics_body: "None".to_string(), gravity_scale: 1.0, mass: 1.0 }, // reuse Cube DTO for now

        XrdsSceneNodePayload::Camera(c) => {
            let (fov, near, far) = match c.projection {
                XrdsSceneCameraProjection::Perspective { fov_deg, near, far, .. } =>
                    (fov_deg, near, far.unwrap_or(1000.0)),
                XrdsSceneCameraProjection::Orthographic { near, far, .. } =>
                    (60.0, near, far),
            };
            NodePayloadDto::Camera { fov, near, far }
        }

        XrdsSceneNodePayload::DirectionalLight(l) =>
            NodePayloadDto::DirectionalLight { color: l.color, illuminance: l.illuminance },

        XrdsSceneNodePayload::PointLight(l) =>
            NodePayloadDto::PointLight { color: l.color, intensity: l.intensity, range: l.range },

        XrdsSceneNodePayload::SpotLight(l) =>
            NodePayloadDto::SpotLight {
                color: l.color, intensity: l.intensity, range: l.range,
                inner_angle: l.inner_angle, outer_angle: l.outer_angle,
            },

        XrdsSceneNodePayload::AmbientLight(l) =>
            NodePayloadDto::AmbientLight { color: l.color, brightness: l.brightness },

        XrdsSceneNodePayload::Text(t) =>
            NodePayloadDto::Text {
                text: t.text.clone(),
                font_size: t.font_size,
                color: t.color,
                alignment: format!("{:?}", t.alignment),
                anchor: anchor_kind_str(t.anchor),
                anchor_param: anchor_param_f32(t.anchor),
            },

        XrdsSceneNodePayload::ExtrudedText(t) =>
            NodePayloadDto::ExtrudedText {
                text: t.text.clone(),
                font_size: t.font_size,
                depth: t.depth,
                color: t.color,
                alignment: format!("{:?}", t.alignment),
            },

        XrdsSceneNodePayload::GltfAsset(_) => {
            let clips = gltf_clips.get(&id).map(|v| v.iter()
                .map(|(i, n)| crate::bridge::GltfClipDto { index: *i, name: n.clone() })
                .collect())
                .unwrap_or_default();
            NodePayloadDto::GltfAsset { clips }
        }

        XrdsSceneNodePayload::HudText(h) =>
            NodePayloadDto::HudText {
                text: h.text.clone(),
                font_size: h.font_size,
                color: h.color,
                anchor: format!("{:?}", h.anchor),
                offset: h.offset,
            },

        XrdsSceneNodePayload::Player(_) =>
            NodePayloadDto::Player,

        XrdsSceneNodePayload::PlayerAnchor(a) =>
            NodePayloadDto::PlayerAnchor {
                fov_deg: a.fov_deg,
                is_initial: a.is_initial,
                exposure: a.exposure,
            },

        XrdsSceneNodePayload::PlayerSpawnZone(z) =>
            NodePayloadDto::PlayerSpawnZone { size: z.size, player_node_id: z.player_node_id },

        // Only the id travels: the frontend already has `panel_library` in the
        // snapshot, so it resolves the name itself rather than carrying a second
        // copy that could go stale after a rename.
        XrdsSceneNodePayload::Panel(i) =>
            NodePayloadDto::Panel {
                template_id: i.template_id.0,
                elements: crate::panel_library::build_panel_instance_elements_dto(doc, i),
            },

        _ => NodePayloadDto::Other { kind: payload_kind_name(payload).to_owned() },
    }
}

fn payload_kind_name(p: &XrdsSceneNodePayload) -> &'static str {
    match p {
        XrdsSceneNodePayload::Empty           => "Empty",
        XrdsSceneNodePayload::Cube(_)         => "Cube",
        XrdsSceneNodePayload::Sphere(_)       => "Sphere",
        XrdsSceneNodePayload::Cylinder(_)     => "Cylinder",
        XrdsSceneNodePayload::Capsule(_)      => "Capsule",
        // Kind string only. Full editor authoring (palette entry, DTO,
        // Inspector section) is Phase 3 of docs/done/vfx-particle-effects-plan.md;
        // this arm exists because the match is exhaustive.
        XrdsSceneNodePayload::Effect(_)       => "Effect",
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

/// Apply an inspector-related EditorCommand. Returns true if a full reimport is needed.
pub fn apply_inspector_command(
    cmd: &EditorCommand,
    session: &mut EditorSession,
    state: &mut EditorState,
) -> bool {
    match cmd {
        // ── Transform live preview ───────────────────────────────────────────
        EditorCommand::SetTranslation { id, value } => {
            state.set_pending_translation(XrdsSceneNodeId(*id), *value);
            false
        }
        EditorCommand::SetRotationEuler { id, degrees } => {
            let quat = euler_degrees_to_quat(*degrees);
            state.set_pending_rotation(XrdsSceneNodeId(*id), quat);
            false
        }
        EditorCommand::SetScale { id, value } => {
            state.pending_scale = Some((XrdsSceneNodeId(*id), *value));
            false
        }
        EditorCommand::CommitTransform { id, translation, rotation_euler_degrees, scale } => {
            let node_id = XrdsSceneNodeId(*id);
            let quat = euler_degrees_to_quat(*rotation_euler_degrees);
            let t = *translation;
            let s = *scale;
            match session.0.edit(|doc| {
                if let Some(node) = doc.node_mut(node_id) {
                    node.transform.translation = t;
                    node.transform.rotation_quat_xyzw = quat;
                    node.transform.scale = s;
                }
            }) {
                Ok(_) => {}
                Err(e) => error!("[inspector] CommitTransform failed: {:?}", e),
            }
            // Clear pending for this node so gizmo + selection read fresh document value.
            state.pending_translations.retain(|(i, _)| *i != node_id);
            state.pending_rotations.retain(|(i, _)| *i != node_id);
            false
        }

        // ── Visibility ───────────────────────────────────────────────────────
        EditorCommand::SetVisible { id, visible } => {
            state.pending_visible = Some((XrdsSceneNodeId(*id), *visible));
            let id = XrdsSceneNodeId(*id);
            let v = *visible;
            let _ = session.0.edit(|doc| {
                if let Some(node) = doc.node_mut(id) { node.visible = v; }
            });
            false
        }

        // ── Grabbable ────────────────────────────────────────────────────────
        EditorCommand::SetGrabbable { id, grabbable } => {
            state.pending_grabbable = Some((XrdsSceneNodeId(*id), *grabbable));
            let id = XrdsSceneNodeId(*id);
            let g = *grabbable;
            let _ = session.0.edit(|doc| {
                if let Some(node) = doc.node_mut(id) { node.grabbable = g; }
            });
            false
        }

        // ── Material ─────────────────────────────────────────────────────────
        EditorCommand::SetMaterial { id, params } => {
            state.pending_material = Some((XrdsSceneNodeId(*id), params.clone()));
            false
        }

        // ── Material ─────────────────────────────────────────────────────────
        EditorCommand::CommitMaterial { id, params } => {
            let id = XrdsSceneNodeId(*id);
            let dto = params.clone();
            match session.0.edit(|doc| {
                // Merge, do not replace — see `merge_material_dto`.
                let existing = doc.node_material(id).ok().cloned();
                let mat = merge_material_dto(existing.as_ref(), &dto);
                if let Some(node) = doc.node_mut(id) {
                    set_node_material(node, mat);
                }
            }) {
                Ok(_) => {}
                Err(e) => error!("[inspector] CommitMaterial failed: {:?}", e),
            }
            false
        }

        EditorCommand::SetNodeMaterialTexture { id, slot, texture_asset_id } => {
            let id = XrdsSceneNodeId(*id);
            let slot = crate::trigger_action::texture_slot_from_dto(slot);
            // `None` clears the slot — a real authoring action, not a no-op.
            let texture = texture_asset_id.as_ref().map(|asset_id| {
                xrds_scene_graph::XrdsSceneTextureRef {
                    texture_asset_id: asset_id.clone(),
                    uv: Default::default(),
                    sampler: Default::default(),
                }
            });
            match session.0.edit(|doc| {
                if let Err(e) = doc.set_node_material_texture(id, slot, texture.clone()) {
                    error!("[inspector] SetNodeMaterialTexture rejected: {:?}", e);
                }
            }) {
                Ok(_) => {}
                Err(e) => error!("[inspector] SetNodeMaterialTexture failed: {:?}", e),
            }
            // Structural enough to need the scene rebuilt so the new image
            // actually loads onto the mesh.
            state.needs_full_reimport = true;
            false
        }

        // ── Lights ───────────────────────────────────────────────────────────
        EditorCommand::SetPointLight { id, color, intensity, range } => {
            state.pending_point_light = Some((XrdsSceneNodeId(*id), *color, *intensity, *range));
            false
        }
        EditorCommand::CommitLight { id } => {
            // pending_*_light is NOT cleared by update(), so it still holds the latest value here.
            let id = XrdsSceneNodeId(*id);
            if let Some((_, color, intensity, range)) = state.pending_point_light {
                let _ = session.0.edit(|doc| {
                    if let Some(node) = doc.node_mut(id) {
                        if let XrdsSceneNodePayload::PointLight(l) = &mut node.payload {
                            l.color = color; l.intensity = intensity; l.range = range;
                        }
                    }
                });
                state.pending_point_light = None;
            } else if let Some((_, color, illuminance)) = state.pending_directional_light {
                let _ = session.0.edit(|doc| {
                    if let Some(node) = doc.node_mut(id) {
                        if let XrdsSceneNodePayload::DirectionalLight(l) = &mut node.payload {
                            l.color = color; l.illuminance = illuminance;
                        }
                    }
                });
                state.pending_directional_light = None;
            } else if let Some((_, color, intensity, range, inner, outer)) = state.pending_spot_light {
                let _ = session.0.edit(|doc| {
                    if let Some(node) = doc.node_mut(id) {
                        if let XrdsSceneNodePayload::SpotLight(l) = &mut node.payload {
                            l.color = color; l.intensity = intensity; l.range = range;
                            l.inner_angle = inner; l.outer_angle = outer;
                        }
                    }
                });
                state.pending_spot_light = None;
            } else if let Some((color, brightness)) = state.pending_ambient_light {
                // Ambient has no node id — write to whichever AmbientLight node matches
                let _ = session.0.edit(|doc| {
                    if let Some(node) = doc.node_mut(id) {
                        if let XrdsSceneNodePayload::AmbientLight(l) = &mut node.payload {
                            l.color = color; l.brightness = brightness;
                        }
                    }
                });
                state.pending_ambient_light = None;
            }
            false
        }
        EditorCommand::SetDirectionalLight { id, color, illuminance } => {
            state.pending_directional_light = Some((XrdsSceneNodeId(*id), *color, *illuminance));
            false
        }
        EditorCommand::SetSpotLight { id, color, intensity, range, inner_angle, outer_angle } => {
            state.pending_spot_light = Some((
                XrdsSceneNodeId(*id), *color, *intensity, *range, *inner_angle, *outer_angle,
            ));
            false
        }
        EditorCommand::SetAmbientLight { id: _, color, brightness } => {
            state.pending_ambient_light = Some((*color, *brightness));
            false
        }

        EditorCommand::SetHudText { id, text, font_size, color, anchor, offset } => {
            let id = XrdsSceneNodeId(*id);
            let text = text.clone();
            let font_size = *font_size;
            let color = *color;
            let offset = *offset;
            let anc = parse_hud_anchor(anchor);
            match session.0.edit(|doc| {
                if let Some(node) = doc.node_mut(id) {
                    if let XrdsSceneNodePayload::HudText(ref mut h) = node.payload {
                        h.text = text; h.font_size = font_size; h.color = color;
                        h.anchor = anc; h.offset = offset;
                    }
                }
            }) {
                Ok(_) => {}
                Err(e) => error!("[inspector] SetHudText failed: {:?}", e),
            }
            true // HudText is screen-space UI — needs reimport to update
        }

        EditorCommand::SetExtrudedTextColor { id, color } => {
            state.pending_extruded_color = Some((XrdsSceneNodeId(*id), *color));
            false
        }

        // ── Camera selector ──────────────────────────────────────────────────
        EditorCommand::SetActiveCamera { id } => {
            state.active_camera_id = id.map(|i| XrdsSceneNodeId(i));
            false
        }

        // ── Camera FOV live preview ───────────────────────────────────────────
        EditorCommand::SetCameraParams { id, fov, .. } => {
            state.pending_camera = Some((XrdsSceneNodeId(*id), *fov));
            false
        }
        EditorCommand::CommitCameraParams { id, fov, near, far } => {
            let id = XrdsSceneNodeId(*id);
            let fov = *fov; let near = *near; let far = *far;
            use xrds_scene_graph::{XrdsSceneCameraProjection};
            match session.0.edit(|doc| {
                if let Some(node) = doc.node_mut(id) {
                    if let xrds_scene_graph::XrdsSceneNodePayload::Camera(ref mut cam) = node.payload {
                        cam.projection = match cam.projection {
                            XrdsSceneCameraProjection::Perspective { order, .. } =>
                                XrdsSceneCameraProjection::Perspective { fov_deg: fov, near, far: Some(far), order },
                            XrdsSceneCameraProjection::Orthographic { order, .. } =>
                                XrdsSceneCameraProjection::Perspective { fov_deg: fov, near, far: Some(far), order },
                        };
                    }
                }
            }) {
                Ok(_) => {}
                Err(e) => error!("[inspector] CommitCameraParams failed: {:?}", e),
            }
            state.pending_camera = None;
            false
        }

        // ── Player / PlayerAnchor ────────────────────────────────────────────
        EditorCommand::SetSpawnZoneSize { id, size } => {
            let id = XrdsSceneNodeId(*id);
            let sz = *size;
            match session.0.edit(|doc| {
                if let Some(node) = doc.node_mut(id) {
                    if let XrdsSceneNodePayload::PlayerSpawnZone(ref mut z) = node.payload {
                        z.size = sz;
                    }
                }
            }) {
                Ok(_) => {}
                Err(e) => error!("[inspector] SetSpawnZoneSize failed: {:?}", e),
            }
            true // size change needs reimport to update ECS component
        }

        EditorCommand::SetSpawnZonePlayer { id, player_node_id } => {
            let id = XrdsSceneNodeId(*id);
            let pid = *player_node_id;
            match session.0.edit(|doc| {
                if let Some(node) = doc.node_mut(id) {
                    if let XrdsSceneNodePayload::PlayerSpawnZone(ref mut z) = node.payload {
                        z.player_node_id = pid;
                    }
                }
            }) {
                Ok(_) => {}
                Err(e) => error!("[inspector] SetSpawnZonePlayer failed: {:?}", e),
            }
            true // ECS component must be updated
        }

        // ── Physics body ────────────────────────────────────────────────────
        EditorCommand::SetPhysicsBody { id, physics_body } => {
            let id = XrdsSceneNodeId(*id);
            let pb = physics_body_from_str(physics_body.as_str());
            match session.0.edit(|doc| {
                if let Some(node) = doc.node_mut(id) {
                    match &mut node.payload {
                        XrdsSceneNodePayload::Cube(ref mut c)     => c.physics_body = pb,
                        XrdsSceneNodePayload::Sphere(ref mut c)   => c.physics_body = pb,
                        XrdsSceneNodePayload::Cylinder(ref mut c) => c.physics_body = pb,
                        XrdsSceneNodePayload::Capsule(ref mut c)  => c.physics_body = pb,
                        XrdsSceneNodePayload::Plane3D(ref mut c)  => c.physics_body = pb,
                        _ => {}
                    }
                }
            }) {
                Ok(_) => {}
                Err(e) => error!("[inspector] SetPhysicsBody failed: {:?}", e),
            }
            true // ECS physics components must be updated via reimport
        }

        EditorCommand::SetGravityScale { id, value } => {
            let id = XrdsSceneNodeId(*id);
            let v = *value;
            match session.0.edit(|doc| {
                if let Some(node) = doc.node_mut(id) {
                    match &mut node.payload {
                        XrdsSceneNodePayload::Cube(ref mut c)     => c.gravity_scale = v,
                        XrdsSceneNodePayload::Sphere(ref mut c)   => c.gravity_scale = v,
                        XrdsSceneNodePayload::Cylinder(ref mut c) => c.gravity_scale = v,
                        XrdsSceneNodePayload::Capsule(ref mut c)  => c.gravity_scale = v,
                        XrdsSceneNodePayload::Plane3D(ref mut c)  => c.gravity_scale = v,
                        _ => {}
                    }
                }
            }) {
                Ok(_) => {}
                Err(e) => error!("[inspector] SetGravityScale failed: {:?}", e),
            }
            state.pending_gravity_scale = Some((id, v));
            false
        }

        EditorCommand::SetMass { id, value } => {
            let id = XrdsSceneNodeId(*id);
            let v = *value;
            match session.0.edit(|doc| {
                if let Some(node) = doc.node_mut(id) {
                    match &mut node.payload {
                        XrdsSceneNodePayload::Cube(ref mut c)     => c.mass = v,
                        XrdsSceneNodePayload::Sphere(ref mut c)   => c.mass = v,
                        XrdsSceneNodePayload::Cylinder(ref mut c) => c.mass = v,
                        XrdsSceneNodePayload::Capsule(ref mut c)  => c.mass = v,
                        XrdsSceneNodePayload::Plane3D(ref mut c)  => c.mass = v,
                        _ => {}
                    }
                }
            }) {
                Ok(_) => {}
                Err(e) => error!("[inspector] SetMass failed: {:?}", e),
            }
            state.pending_mass = Some((id, v));
            false
        }

        EditorCommand::SetCapsuleGeometry { id, radius, length } => {
            let id = XrdsSceneNodeId(*id);
            let (r, l) = (*radius, *length);
            match session.0.edit(|doc| {
                if let Some(node) = doc.node_mut(id) {
                    if let XrdsSceneNodePayload::Capsule(ref mut c) = node.payload {
                        c.radius = r;
                        c.length = l;
                    }
                }
            }) {
                Ok(_) => {}
                Err(e) => error!("[inspector] SetCapsuleGeometry failed: {:?}", e),
            }
            state.pending_capsule_geometry = Some((id, r, l));
            false
        }

        EditorCommand::PreviewAudioClip { id, playing } => {
            state.pending_audio_preview = Some((XrdsSceneNodeId(*id), *playing));
            // No reimport: the point of auditioning is to hear the clip that is
            // already there. Rebuilding the entity would restart every other sound
            // in the scene and lose the playhead.
            false
        }
        EditorCommand::PreviewVideo { asset_id, playing } => {
            state.pending_video_preview = Some((asset_id.clone(), *playing));
            // No reimport, for the same reason auditioning audio does not: the point
            // is to see the clip already bound, and rebuilding the entity would drop
            // the texture the surface is showing.
            false
        }
        EditorCommand::SetAudioClipParams {
            id,
            asset_id,
            volume,
            looped,
            spatial,
            autoplay,
            distance_model,
            min_distance,
            max_distance,
            rolloff_factor,
        } => {
            let id = XrdsSceneNodeId(*id);
            let Some(model) = audio_distance_model_from_str(distance_model) else {
                error!("[inspector] SetAudioClipParams: unknown distance model {distance_model:?}, ignoring");
                return false;
            };

            // `max` is held above `min`: the two are dragged independently in the
            // UI, and an inverted pair would mean "silent everywhere", which reads
            // as a broken clip rather than as a bad value.
            let min_distance = min_distance.max(0.01);
            let max_distance = max_distance.max(min_distance + 0.01);

            let clip = XrdsSceneAudioClip {
                asset_id: asset_id.clone(),
                volume: volume.clamp(0.0, 1.0),
                looped: *looped,
                spatial: *spatial,
                autoplay: *autoplay,
                distance_model: model,
                min_distance,
                max_distance,
                rolloff_factor: rolloff_factor.max(0.0),
            };

            // Which fields changed decides whether the entity must be rebuilt.
            // Reimport respawns it, which restarts every sound in the scene and
            // cuts off the clip the author is auditioning — so it is reserved for
            // the three fields that genuinely require a new sink.
            let needs_respawn = session
                .0
                .document()
                .node(id)
                .and_then(|n| match &n.payload {
                    XrdsSceneNodePayload::AudioClip(a) => Some(a),
                    _ => None,
                })
                .map(|a| {
                    // asset_id: a different file entirely.
                    // looped:   PlaybackMode is fixed when the sink is built.
                    // spatial:  decides AudioSink vs SpatialAudioSink — a different
                    //           component, so a different sink.
                    a.asset_id != clip.asset_id
                        || a.looped != clip.looped
                        || a.spatial != clip.spatial
                })
                .unwrap_or(true);

            match session.0.edit(|doc| {
                if let Some(node) = doc.node_mut(id) {
                    if let XrdsSceneNodePayload::AudioClip(ref mut a) = node.payload {
                        *a = clip.clone();
                    }
                }
            }) {
                Ok(_) => {}
                Err(e) => error!("[inspector] SetAudioClipParams failed: {:?}", e),
            }

            if !needs_respawn {
                // Volume and the whole falloff curve are read fresh every frame, so
                // they can be applied live — a slider drag is audible immediately
                // on a clip that is already playing.
                state.pending_audio_falloff = Some((id, clip));
            }
            needs_respawn
        }
        EditorCommand::SetEffectParams {
            id,
            kind,
            auto_play,
            burst_count,
            spawn_rate,
            lifetime_secs,
            size_min,
            size_max,
            color_start,
            color_end,
            speed_min,
            speed_max,
            omnidirectional,
            spread_deg,
            gravity,
            emission_radius,
            blend,
            size_end,
            drag,
            fade_edge,
            fade_scene,
        } => {
            let id = XrdsSceneNodeId(*id);
            // Unknown kind strings are ignored rather than defaulted: silently
            // turning a Trail into a Burst because of a frontend typo would be a
            // confusing edit to debug.
            let Some(parsed_kind) = effect_kind_from_str(kind) else {
                error!("[inspector] SetEffectParams: unknown kind {kind:?}, ignoring");
                return false;
            };
            let Some(parsed_blend) = effect_blend_from_str(blend) else {
                error!("[inspector] SetEffectParams: unknown blend {blend:?}, ignoring");
                return false;
            };
            let params = XrdsSceneEffect {
                kind: parsed_kind,
                auto_play: *auto_play,
                burst_count: *burst_count,
                spawn_rate: *spawn_rate,
                lifetime_secs: *lifetime_secs,
                size_min: *size_min,
                size_max: *size_max,
                color_start: *color_start,
                color_end: *color_end,
                speed_min: *speed_min,
                speed_max: *speed_max,
                omnidirectional: *omnidirectional,
                spread_deg: *spread_deg,
                gravity: *gravity,
                emission_radius: *emission_radius,
                blend: parsed_blend,
                size_end: *size_end,
                drag: *drag,
                fade_edge: *fade_edge,
                fade_scene: *fade_scene,
            };
            let stored = params.clone();
            match session.0.edit(|doc| {
                if let Some(node) = doc.node_mut(id) {
                    if let XrdsSceneNodePayload::Effect(ref mut e) = node.payload {
                        *e = stored.clone();
                    }
                }
            }) {
                Ok(_) => {}
                Err(e) => error!("[inspector] SetEffectParams failed: {:?}", e),
            }
            state.pending_effect_params = Some((id, params));
            false
        }

        EditorCommand::SetPlayerAnchorFov { id, fov_deg } => {
            let id = XrdsSceneNodeId(*id);
            let fov = *fov_deg;
            match session.0.edit(|doc| {
                if let Some(node) = doc.node_mut(id) {
                    if let XrdsSceneNodePayload::PlayerAnchor(ref mut a) = node.payload {
                        a.fov_deg = fov;
                    }
                }
            }) {
                Ok(_) => {}
                Err(e) => error!("[inspector] SetPlayerAnchorFov failed: {:?}", e),
            }
            state.pending_anchor_fov = Some((id, fov));
            false
        }

        EditorCommand::SetPlayerAnchorInitial { id, is_initial } => {
            let target = XrdsSceneNodeId(*id);
            let val = *is_initial;
            match session.0.edit(|doc| {
                // Enforce at most one initial anchor: clear all others first.
                if val {
                    for node in doc.nodes.iter_mut() {
                        if node.id != target {
                            if let XrdsSceneNodePayload::PlayerAnchor(ref mut a) = node.payload {
                                a.is_initial = false;
                            }
                        }
                    }
                }
                if let Some(node) = doc.node_mut(target) {
                    if let XrdsSceneNodePayload::PlayerAnchor(ref mut a) = node.payload {
                        a.is_initial = val;
                    }
                }
            }) {
                Ok(_) => {}
                Err(e) => error!("[inspector] SetPlayerAnchorInitial failed: {:?}", e),
            }
            true
        }

        EditorCommand::SetPlayerAnchorExposure { id, ev100 } => {
            let id = XrdsSceneNodeId(*id);
            let ev = *ev100;
            match session.0.edit(|doc| {
                if let Some(node) = doc.node_mut(id) {
                    if let XrdsSceneNodePayload::PlayerAnchor(ref mut a) = node.payload {
                        a.exposure = ev;
                    }
                }
            }) {
                Ok(_) => {}
                Err(e) => error!("[inspector] SetPlayerAnchorExposure failed: {:?}", e),
            }
            true // ECS XrdsAnchorExposure component must be updated
        }

        // The 7 World Panel commands lived here (SetWorldPanelParams,
        // Add/Remove/MoveWorldPanelWidget, SetWorldPanelWidget(s),
        // SetWorldPanelLayout). Retired with `XrdsSceneWorldPanel`: its widgets
        // carried no triggers, so every button on one was permanently dead.
        // A panel template's own commands (`SetPanelTemplateParams`,
        // `SetPanelElementWidget`, …) cover the live replacement.

        // ── GLTF animation ───────────────────────────────────────────────────
        EditorCommand::PlayGltfAnimation { id, clip_index, speed, .. } => {
            state.pending_gltf_play = Some((XrdsSceneNodeId(*id), *clip_index, *speed));
            false
        }
        EditorCommand::StopGltfAnimation { id } => {
            state.pending_gltf_stop = Some(XrdsSceneNodeId(*id));
            false
        }

        // ── Text — write directly to document (requires reimport) ────────────
        EditorCommand::SetTextContent { id, text, font_size, color, alignment, anchor, anchor_param } => {
            let id = XrdsSceneNodeId(*id);
            let text = text.clone();
            let font_size = *font_size;
            let color = *color;
            let align = parse_text_alignment(alignment);
            let anc = parse_text_anchor(anchor, *anchor_param);
            match session.0.edit(|doc| {
                if let Some(node) = doc.node_mut(id) {
                    if let XrdsSceneNodePayload::Text(ref mut t) = node.payload {
                        t.text = text;
                        t.font_size = font_size;
                        t.color = color;
                        t.alignment = align;
                        t.anchor = anc;
                    }
                }
            }) {
                Ok(_) => {}
                Err(e) => error!("[inspector] SetTextContent failed: {:?}", e),
            }
            true
        }

        EditorCommand::SetExtrudedText { id, text, font_size, depth, color, alignment } => {
            let id = XrdsSceneNodeId(*id);
            let text = text.clone();
            let font_size = *font_size;
            let depth = *depth;
            let color = *color;
            let align = parse_text_alignment(alignment);
            match session.0.edit(|doc| {
                if let Some(node) = doc.node_mut(id) {
                    if let XrdsSceneNodePayload::ExtrudedText(ref mut t) = node.payload {
                        t.text = text;
                        t.font_size = font_size;
                        t.depth = depth;
                        t.color = color;
                        t.alignment = align;
                    }
                }
            }) {
                Ok(_) => {}
                Err(e) => error!("[inspector] SetExtrudedText failed: {:?}", e),
            }
            true
        }


        _ => false,
    }
}

// ---------------------------------------------------------------------------
// Helpers — rotation conversion
// ---------------------------------------------------------------------------

pub fn quat_to_euler_degrees(q: [f32; 4]) -> [f32; 3] {
    let quat = Quat::from_xyzw(q[0], q[1], q[2], q[3]);
    let (x, y, z) = quat.to_euler(EulerRot::XYZ);
    [x.to_degrees(), y.to_degrees(), z.to_degrees()]
}

pub fn euler_degrees_to_quat(degrees: [f32; 3]) -> [f32; 4] {
    let q = Quat::from_euler(
        EulerRot::XYZ,
        degrees[0].to_radians(),
        degrees[1].to_radians(),
        degrees[2].to_radians(),
    );
    [q.x, q.y, q.z, q.w]
}

// ---------------------------------------------------------------------------
// ---------------------------------------------------------------------------
// Helpers — physics body string ↔ XrdsPhysicsBody
// ---------------------------------------------------------------------------

fn audio_distance_model_str(model: XrdsAudioDistanceModel) -> &'static str {
    match model {
        XrdsAudioDistanceModel::Linear => "Linear",
        XrdsAudioDistanceModel::Inverse => "Inverse",
        XrdsAudioDistanceModel::Exponential => "Exponential",
    }
}

fn audio_distance_model_from_str(model: &str) -> Option<XrdsAudioDistanceModel> {
    match model {
        "Linear" => Some(XrdsAudioDistanceModel::Linear),
        "Inverse" => Some(XrdsAudioDistanceModel::Inverse),
        "Exponential" => Some(XrdsAudioDistanceModel::Exponential),
        _ => None,
    }
}

fn effect_kind_str(kind: XrdsSceneEffectKind) -> &'static str {
    match kind {
        XrdsSceneEffectKind::Burst => "Burst",
        XrdsSceneEffectKind::Trail => "Trail",
    }
}

fn effect_kind_from_str(kind: &str) -> Option<XrdsSceneEffectKind> {
    match kind {
        "Burst" => Some(XrdsSceneEffectKind::Burst),
        "Trail" => Some(XrdsSceneEffectKind::Trail),
        _ => None,
    }
}

fn effect_blend_str(blend: XrdsSceneEffectBlend) -> &'static str {
    match blend {
        XrdsSceneEffectBlend::Blend => "Blend",
        XrdsSceneEffectBlend::Add => "Add",
        XrdsSceneEffectBlend::Multiply => "Multiply",
    }
}

fn effect_blend_from_str(blend: &str) -> Option<XrdsSceneEffectBlend> {
    match blend {
        "Blend" => Some(XrdsSceneEffectBlend::Blend),
        "Add" => Some(XrdsSceneEffectBlend::Add),
        "Multiply" => Some(XrdsSceneEffectBlend::Multiply),
        _ => None,
    }
}

fn physics_body_str(pb: xrds_scene_graph::XrdsPhysicsBody) -> String {
    match pb {
        xrds_scene_graph::XrdsPhysicsBody::None    => "None".to_string(),
        xrds_scene_graph::XrdsPhysicsBody::Static  => "Static".to_string(),
        xrds_scene_graph::XrdsPhysicsBody::Dynamic => "Dynamic".to_string(),
    }
}

fn physics_body_from_str(s: &str) -> xrds_scene_graph::XrdsPhysicsBody {
    match s {
        "Static"  => xrds_scene_graph::XrdsPhysicsBody::Static,
        "Dynamic" => xrds_scene_graph::XrdsPhysicsBody::Dynamic,
        _         => xrds_scene_graph::XrdsPhysicsBody::None,
    }
}

// ---------------------------------------------------------------------------
// Helpers — material DTO ↔ scene material
// ---------------------------------------------------------------------------

pub fn mat_dto(mat: &XrdsSceneMaterial) -> MaterialParamsDto {
    let id_of = |t: &Option<xrds_scene_graph::XrdsSceneTextureRef>| {
        t.as_ref().map(|r| r.texture_asset_id.clone())
    };
    MaterialParamsDto {
        base_color: mat.base_color,
        metallic: mat.pbr.metallic,
        roughness: mat.pbr.roughness,
        emissive: [mat.emissive[0], mat.emissive[1], mat.emissive[2]],
        textures: crate::bridge::MaterialTexturesDto {
            base_color: id_of(&mat.textures.base_color),
            metallic_roughness: id_of(&mat.textures.metallic_roughness),
            normal: id_of(&mat.textures.normal),
            occlusion: id_of(&mat.textures.occlusion),
            emissive: id_of(&mat.textures.emissive),
        },
    }
}

/// Merges the four fields the Material panel edits over an existing material,
/// leaving everything else untouched.
///
/// `MaterialParamsDto` is a *partial* view — base colour, emissive, metallic,
/// roughness — so rebuilding an `XrdsSceneMaterial` from it destroys texture
/// slots, `opacity`, `unlit` and the extra PBR fields (reflectance /
/// double_sided / alpha_mode / alpha_cutoff). That was happening on every
/// colour drag; it stayed invisible only while nothing in the editor could
/// author those fields, and became real data loss the moment texture slots
/// were exposed.
///
/// Extracted from the command handler so the preservation is directly
/// assertable — the handler needs a whole session to exercise.
pub(crate) fn merge_material_dto(
    existing: Option<&XrdsSceneMaterial>,
    dto: &MaterialParamsDto,
) -> XrdsSceneMaterial {
    let mut mat = match existing {
        Some(m) => m.clone(),
        // No material to merge into (a payload kind that has none) — build one.
        None => return scene_material_from_dto(dto),
    };
    mat.base_color = dto.base_color;
    mat.emissive = [dto.emissive[0], dto.emissive[1], dto.emissive[2], 1.0];
    mat.pbr.metallic = dto.metallic;
    mat.pbr.roughness = dto.roughness;
    mat
}

fn scene_material_from_dto(dto: &MaterialParamsDto) -> XrdsSceneMaterial {
    use xrds_scene_graph::XrdsSceneMaterialPbrParams;
    XrdsSceneMaterial {
        base_color: dto.base_color,
        emissive: [dto.emissive[0], dto.emissive[1], dto.emissive[2], 1.0],
        pbr: XrdsSceneMaterialPbrParams {
            metallic: dto.metallic,
            roughness: dto.roughness,
            ..Default::default()
        },
        ..Default::default()
    }
}

fn parse_hud_anchor(s: &str) -> XrdsHudAnchor {
    match s {
        "TopCenter"    => XrdsHudAnchor::TopCenter,
        "TopRight"     => XrdsHudAnchor::TopRight,
        "MiddleLeft"   => XrdsHudAnchor::MiddleLeft,
        "Center"       => XrdsHudAnchor::Center,
        "MiddleRight"  => XrdsHudAnchor::MiddleRight,
        "BottomLeft"   => XrdsHudAnchor::BottomLeft,
        "BottomCenter" => XrdsHudAnchor::BottomCenter,
        "BottomRight"  => XrdsHudAnchor::BottomRight,
        _              => XrdsHudAnchor::TopLeft,
    }
}

fn parse_text_alignment(s: &str) -> XrdsSceneTextAlignment {
    match s {
        "Right" => XrdsSceneTextAlignment::Right,
        "Left"  => XrdsSceneTextAlignment::Left,
        _       => XrdsSceneTextAlignment::Center,
    }
}

fn parse_text_anchor(s: &str, param: f32) -> XrdsSceneTextAnchor {
    match s {
        "Billboard"    => XrdsSceneTextAnchor::Billboard,
        "HeadLocked"   => XrdsSceneTextAnchor::HeadLocked,
        "BodyLocked"   => XrdsSceneTextAnchor::BodyLocked,
        "ComfortPinned"=> XrdsSceneTextAnchor::ComfortPinned { depth_m: param },
        "Cylindrical"  => XrdsSceneTextAnchor::Cylindrical   { radius_m: param },
        _              => XrdsSceneTextAnchor::World,
    }
}

fn anchor_kind_str(a: XrdsSceneTextAnchor) -> String {
    match a {
        XrdsSceneTextAnchor::World              => "World".to_string(),
        XrdsSceneTextAnchor::Billboard          => "Billboard".to_string(),
        XrdsSceneTextAnchor::HeadLocked         => "HeadLocked".to_string(),
        XrdsSceneTextAnchor::BodyLocked         => "BodyLocked".to_string(),
        XrdsSceneTextAnchor::ComfortPinned { .. }=> "ComfortPinned".to_string(),
        XrdsSceneTextAnchor::Cylindrical { .. } => "Cylindrical".to_string(),
    }
}

fn anchor_param_f32(a: XrdsSceneTextAnchor) -> f32 {
    match a {
        XrdsSceneTextAnchor::ComfortPinned { depth_m }  => depth_m,
        XrdsSceneTextAnchor::Cylindrical   { radius_m } => radius_m,
        _                                               => 0.0,
    }
}

fn set_node_material(node: &mut xrds_scene_graph::XrdsSceneNode, mat: XrdsSceneMaterial) {
    match &mut node.payload {
        XrdsSceneNodePayload::Cube(c)      => c.material = mat,
        XrdsSceneNodePayload::Sphere(c)    => c.material = mat,
        XrdsSceneNodePayload::Cylinder(c)  => c.material = mat,
        XrdsSceneNodePayload::Capsule(c)   => c.material = mat,
        XrdsSceneNodePayload::Plane3D(c)   => c.material = mat,
        XrdsSceneNodePayload::Tetrahedron(c) => c.material = mat,
        _ => {}
    }
}

// ---------------------------------------------------------------------------
// World Panel widget/layout DTO conversion
// ---------------------------------------------------------------------------

pub(crate) fn world_layout_dto(l: &XrdsSceneWorldLayout) -> WorldLayoutDto {
    match l {
        XrdsSceneWorldLayout::None            => WorldLayoutDto::None,
        XrdsSceneWorldLayout::VStack { gap }  => WorldLayoutDto::VStack { gap: *gap },
        XrdsSceneWorldLayout::HStack { gap }  => WorldLayoutDto::HStack { gap: *gap },
        XrdsSceneWorldLayout::Grid { cols, gap } => WorldLayoutDto::Grid { cols: *cols, gap: *gap },
    }
}

fn world_layout_from_dto(l: &WorldLayoutDto) -> XrdsSceneWorldLayout {
    match l {
        WorldLayoutDto::None            => XrdsSceneWorldLayout::None,
        WorldLayoutDto::VStack { gap }  => XrdsSceneWorldLayout::VStack { gap: *gap },
        WorldLayoutDto::HStack { gap }  => XrdsSceneWorldLayout::HStack { gap: *gap },
        WorldLayoutDto::Grid { cols, gap } => XrdsSceneWorldLayout::Grid { cols: *cols, gap: *gap },
    }
}

pub(crate) fn world_widget_dto(w: &XrdsSceneWorldWidget) -> WorldWidgetDto {
    match w {
        XrdsSceneWorldWidget::Label(l) => WorldWidgetDto::Label {
            text: l.text.clone(), font_size: l.font_size, color: l.color,
            local_position: l.local_position, layout_size: l.layout_size,
        },
        XrdsSceneWorldWidget::Button(b) => WorldWidgetDto::Button {
            label: b.label.clone(), font_size: b.font_size, label_color: b.label_color,
            size: b.size, local_position: b.local_position,
            normal_color: b.normal_color, hover_color: b.hover_color, pressed_color: b.pressed_color,
        },
        XrdsSceneWorldWidget::Image(i) => WorldWidgetDto::Image {
            asset_path: i.asset_path.clone(), size: i.size,
            local_position: i.local_position, tint: i.tint,
        },
        XrdsSceneWorldWidget::Slider(s) => WorldWidgetDto::Slider {
            min: s.min, max: s.max, value: s.value,
            size: s.size, local_position: s.local_position,
            track_color: s.track_color, fill_color: s.fill_color,
            thumb_color: s.thumb_color, thumb_size: s.thumb_size,
        },
        XrdsSceneWorldWidget::Toggle(t) => WorldWidgetDto::Toggle {
            checked: t.checked, size: t.size, local_position: t.local_position,
            track_off_color: t.track_off_color, track_on_color: t.track_on_color,
            thumb_color: t.thumb_color,
        },
    }
}

pub(crate) fn world_widget_from_dto(w: &WorldWidgetDto) -> XrdsSceneWorldWidget {
    match w {
        WorldWidgetDto::Label { text, font_size, color, local_position, layout_size } =>
            XrdsSceneWorldWidget::Label(XrdsSceneWorldLabel {
                text: text.clone(), font_size: *font_size, color: *color,
                local_position: *local_position, layout_size: *layout_size,
            }),
        WorldWidgetDto::Button { label, font_size, label_color, size, local_position,
                                 normal_color, hover_color, pressed_color } =>
            XrdsSceneWorldWidget::Button(XrdsSceneWorldButton {
                label: label.clone(), font_size: *font_size, label_color: *label_color,
                size: *size, local_position: *local_position,
                normal_color: *normal_color, hover_color: *hover_color, pressed_color: *pressed_color,
            }),
        WorldWidgetDto::Image { asset_path, size, local_position, tint } =>
            XrdsSceneWorldWidget::Image(XrdsSceneWorldImage {
                asset_path: asset_path.clone(), size: *size,
                local_position: *local_position, tint: *tint,
            }),
        WorldWidgetDto::Slider { min, max, value, size, local_position,
                                 track_color, fill_color, thumb_color, thumb_size } =>
            XrdsSceneWorldWidget::Slider(XrdsSceneWorldSlider {
                min: *min, max: *max, value: *value,
                size: *size, local_position: *local_position,
                track_color: *track_color, fill_color: *fill_color,
                thumb_color: *thumb_color, thumb_size: *thumb_size,
            }),
        WorldWidgetDto::Toggle { checked, size, local_position,
                                 track_off_color, track_on_color, thumb_color } =>
            XrdsSceneWorldWidget::Toggle(XrdsSceneWorldToggle {
                checked: *checked, size: *size, local_position: *local_position,
                track_off_color: *track_off_color, track_on_color: *track_on_color,
                thumb_color: *thumb_color,
            }),
    }
}

fn default_widget_for_kind(kind: &str) -> Option<XrdsSceneWorldWidget> {
    Some(match kind {
        "Label"  => XrdsSceneWorldWidget::Label(XrdsSceneWorldLabel {
            text: "Label".to_string(), ..Default::default()
        }),
        "Button" => XrdsSceneWorldWidget::Button(XrdsSceneWorldButton {
            label: "Button".to_string(), ..Default::default()
        }),
        "Image"  => XrdsSceneWorldWidget::Image(XrdsSceneWorldImage::default()),
        "Slider" => XrdsSceneWorldWidget::Slider(XrdsSceneWorldSlider::default()),
        "Toggle" => XrdsSceneWorldWidget::Toggle(XrdsSceneWorldToggle::default()),
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use xrds_scene_graph::{
        XrdsSceneMaterialTextureSlotKind, XrdsSceneMaterialTextureSlots, XrdsSceneTextureRef,
    };

    fn textured_material() -> XrdsSceneMaterial {
        let mut textures = XrdsSceneMaterialTextureSlots::default();
        textures.set(
            XrdsSceneMaterialTextureSlotKind::Normal,
            Some(XrdsSceneTextureRef {
                texture_asset_id: "asset:normal-map".to_string(),
                uv: Default::default(),
                sampler: Default::default(),
            }),
        );
        XrdsSceneMaterial {
            base_color: [0.1, 0.2, 0.3, 1.0],
            emissive: [0.0, 0.0, 0.0, 1.0],
            opacity: 0.25,
            unlit: true,
            pbr: xrds_scene_graph::XrdsSceneMaterialPbrParams {
                metallic: 0.1,
                roughness: 0.9,
                double_sided: true,
                ..Default::default()
            },
            textures,
        }
    }

    /// Regression: editing colour/metallic/roughness must not destroy the rest
    /// of the material. This was live for as long as the Material panel has
    /// existed — invisible only because nothing could author the fields it
    /// wiped, and it became real data loss when texture slots were exposed.
    #[test]
    fn committing_a_material_edit_preserves_textures_opacity_unlit_and_pbr_extras() {
        let existing = textured_material();
        let dto = MaterialParamsDto {
            base_color: [1.0, 0.0, 0.0, 1.0],
            metallic: 0.75,
            roughness: 0.25,
            emissive: [0.0, 0.0, 0.0],
            textures: Default::default(), // read-only field; must be ignored on write
        };

        let merged = merge_material_dto(Some(&existing), &dto);

        // The four edited fields land.
        assert_eq!(merged.base_color, [1.0, 0.0, 0.0, 1.0]);
        assert_eq!(merged.pbr.metallic, 0.75);
        assert_eq!(merged.pbr.roughness, 0.25);

        // Everything else survives.
        assert_eq!(
            merged
                .textures
                .get(XrdsSceneMaterialTextureSlotKind::Normal)
                .map(|t| t.texture_asset_id.as_str()),
            Some("asset:normal-map"),
            "a colour edit must not drop an authored texture slot"
        );
        assert_eq!(merged.opacity, 0.25, "opacity must survive");
        assert!(merged.unlit, "unlit must survive");
        assert!(merged.pbr.double_sided, "extra PBR fields must survive");
    }

    /// The DTO's `textures` is read-only — writes go through
    /// `SetNodeMaterialTexture`. An empty one arriving from a drag must not be
    /// mistaken for "clear every slot".
    #[test]
    fn the_dtos_read_only_textures_field_never_writes() {
        let existing = textured_material();
        let dto = MaterialParamsDto { textures: Default::default(), ..Default::default() };
        let merged = merge_material_dto(Some(&existing), &dto);
        assert!(
            !merged.textures.is_empty(),
            "an empty DTO texture set must be ignored, not applied"
        );
    }

    /// A payload kind with no material at all still has to produce one.
    #[test]
    fn merging_onto_nothing_builds_a_material_from_the_dto() {
        let dto = MaterialParamsDto {
            base_color: [0.0, 1.0, 0.0, 1.0],
            metallic: 0.5,
            ..Default::default()
        };
        let built = merge_material_dto(None, &dto);
        assert_eq!(built.base_color, [0.0, 1.0, 0.0, 1.0]);
        assert_eq!(built.pbr.metallic, 0.5);
    }
}
