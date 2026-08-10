//! **Deprecated: scene export to glTF/GLB is retired.** Nothing in the workspace
//! calls this crate any more, and the editor no longer offers the command.
//!
//! The reason is not that the exporter is broken — it does what it always did.
//! It is that glTF can no longer *represent an XRDS scene*. What a scene means
//! now lives largely in concepts glTF has no vocabulary for:
//!
//! - `XrdsPanelTemplate`s and their named elements
//! - element and node trigger bindings (`XrdsTriggerBinding`)
//! - the `XrdsTrack` registry — every piece of authored choreography
//! - `PlayerAnchor`s, spawn zones, interaction zones, locomotion modes
//! - threshold watchers, grabbables, physics bodies
//!
//! An export therefore produced a file that *looked* complete and was a mesh
//! dump: geometry, materials, cameras and lights, with every behaviour silently
//! dropped. A lossy export is defensible when the loss is visible; this one was
//! invisible, and reading it back could not reconstruct the scene. That makes it
//! a trap rather than a feature, which is why the entry point is gone rather
//! than merely labelled.
//!
//! **glTF/GLB *import* is unaffected and fully supported.** Loading `.glb`
//! assets never went through this crate — it runs through Bevy's glTF loader and
//! `XrdsSceneNode::from_xrds_gltf_asset`. Likewise the application and APK export
//! pipelines, which *copy* existing `.glb` assets and rewrite `asset_uri`; they
//! never generated glTF and do not depend on this crate.
//!
//! Kept compiling, with its tests, so the tessellators and GLB container writing
//! are not lost if a deliberately-labelled mesh-only export is ever wanted. Do
//! not wire it back into a "save"/"export scene" path.

mod buffers;
mod glb;
pub mod tessellation;

#[cfg(test)]
mod tests;

use buffers::{BufferBuilder, MeshAccessors};
use serde_json::{json, Value};
use xrds_scene_graph::{XrdsSceneDocument, XrdsSceneNodePayload, XrdsSceneCameraProjection};

// ── Public API ────────────────────────────────────────────────────────────────

#[derive(Debug)]
pub enum GlbExportError {
    Json(serde_json::Error),
}

impl std::fmt::Display for GlbExportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self { Self::Json(e) => write!(f, "{e}") }
    }
}
impl std::error::Error for GlbExportError {}

/// Export an `XrdsSceneDocument` to GLB bytes (binary glTF 2.0).
///
/// Produces a self-contained `.glb` file containing **only**:
/// - Scene hierarchy with transforms
/// - PBR materials
/// - Tessellated primitive meshes (Sphere, Cube, Cylinder, Plane, Tetrahedron)
/// - Cameras (perspective + orthographic)
/// - Lights via `KHR_lights_punctual`
/// - GltfAsset source URI + AudioClip asset_id in node `extras`
///
/// Everything else in the document — panels, triggers, Tracks, anchors, zones,
/// watchers, physics — is **silently dropped**. See the crate docs: that is why
/// this is deprecated rather than merely documented as lossy.
#[deprecated(
    since = "0.1.0",
    note = "glTF cannot represent an XRDS scene (panels, triggers, Tracks, anchors are all \
            dropped silently), so scene export is retired. glTF *import* is unaffected. Do not \
            wire this into a save or export-scene path; see the crate docs."
)]
pub fn export_glb(doc: &XrdsSceneDocument) -> Result<Vec<u8>, GlbExportError> {
    let mut buf = BufferBuilder::default();

    let mut materials: Vec<Value> = Vec::new();
    let mut meshes: Vec<Value> = Vec::new();
    let mut cameras: Vec<Value> = Vec::new();
    let mut lights: Vec<Value> = Vec::new(); // KHR_lights_punctual

    // Maps from document-node index to glTF object index
    let mut node_mat: std::collections::HashMap<usize, usize> = Default::default();
    let mut node_mesh: std::collections::HashMap<usize, usize> = Default::default();
    let mut node_cam: std::collections::HashMap<usize, usize> = Default::default();
    let mut node_light: std::collections::HashMap<usize, usize> = Default::default();

    let mut extensions_used: Vec<&str> = Vec::new();

    // Only export visible nodes
    let visible: Vec<usize> = doc.nodes.iter().enumerate()
        .filter(|(_, n)| n.visible)
        .map(|(i, _)| i)
        .collect();
    let visible_set: std::collections::HashSet<usize> = visible.iter().copied().collect();

    // ── Pass 1: build materials / meshes / cameras / lights ──────────────────
    for &i in &visible {
        let node = &doc.nodes[i];
        match &node.payload {
            XrdsSceneNodePayload::Sphere(s) => {
                let mi = push_material(&mut materials, &s.material, &mut extensions_used);
                node_mat.insert(i, mi);
                let mesh_acc = buf.push_mesh(tessellation::sphere(s.radius, 24, 16));
                node_mesh.insert(i, push_mesh(&mut meshes, mesh_acc, mi));
            }
            XrdsSceneNodePayload::Cube(c) => {
                let mi = push_material(&mut materials, &c.material, &mut extensions_used);
                node_mat.insert(i, mi);
                let mesh_acc = buf.push_mesh(tessellation::cuboid(c.size[0], c.size[1], c.size[2]));
                node_mesh.insert(i, push_mesh(&mut meshes, mesh_acc, mi));
            }
            XrdsSceneNodePayload::Cylinder(c) => {
                let mi = push_material(&mut materials, &c.material, &mut extensions_used);
                node_mat.insert(i, mi);
                let mesh_acc = buf.push_mesh(tessellation::cylinder(c.radius, c.height, 24));
                node_mesh.insert(i, push_mesh(&mut meshes, mesh_acc, mi));
            }
            XrdsSceneNodePayload::Plane3D(p) => {
                let mi = push_material(&mut materials, &p.material, &mut extensions_used);
                node_mat.insert(i, mi);
                let mesh_acc = buf.push_mesh(tessellation::plane(p.size[0], p.size[1]));
                node_mesh.insert(i, push_mesh(&mut meshes, mesh_acc, mi));
            }
            XrdsSceneNodePayload::Tetrahedron(t) => {
                let mi = push_material(&mut materials, &t.material, &mut extensions_used);
                node_mat.insert(i, mi);
                // Use the centroid distance as a proxy radius for tessellation.
                let r = t.vertices.iter()
                    .map(|v| (v[0]*v[0]+v[1]*v[1]+v[2]*v[2]).sqrt())
                    .fold(0.0_f32, f32::max)
                    .max(0.5);
                let mesh_acc = buf.push_mesh(tessellation::tetrahedron(r));
                node_mesh.insert(i, push_mesh(&mut meshes, mesh_acc, mi));
            }
            XrdsSceneNodePayload::Camera(cam) => {
                let ci = cameras.len();
                cameras.push(build_camera(cam));
                node_cam.insert(i, ci);
            }
            XrdsSceneNodePayload::PointLight(l) => {
                if !extensions_used.contains(&"KHR_lights_punctual") {
                    extensions_used.push("KHR_lights_punctual");
                }
                let li = lights.len();
                lights.push(json!({
                    "type": "point",
                    "color": [l.color[0], l.color[1], l.color[2]],
                    "intensity": l.intensity,
                    "range": l.range
                }));
                node_light.insert(i, li);
            }
            XrdsSceneNodePayload::DirectionalLight(l) => {
                if !extensions_used.contains(&"KHR_lights_punctual") {
                    extensions_used.push("KHR_lights_punctual");
                }
                let li = lights.len();
                lights.push(json!({
                    "type": "directional",
                    "color": [l.color[0], l.color[1], l.color[2]],
                    "intensity": l.illuminance
                }));
                node_light.insert(i, li);
            }
            XrdsSceneNodePayload::SpotLight(l) => {
                if !extensions_used.contains(&"KHR_lights_punctual") {
                    extensions_used.push("KHR_lights_punctual");
                }
                let li = lights.len();
                lights.push(json!({
                    "type": "spot",
                    "color": [l.color[0], l.color[1], l.color[2]],
                    "intensity": l.intensity,
                    "range": l.range,
                    "spot": {
                        "innerConeAngle": l.inner_angle,
                        "outerConeAngle": l.outer_angle
                    }
                }));
                node_light.insert(i, li);
            }
            _ => {}
        }
    }

    // ── Pass 2: build glTF nodes ──────────────────────────────────────────────
    // Map document index → glTF node index (only visible)
    let doc_to_gltf: std::collections::HashMap<usize, usize> = visible.iter()
        .enumerate().map(|(gi, &di)| (di, gi)).collect();

    let mut gltf_nodes: Vec<Value> = Vec::new();
    for &i in &visible {
        let n = &doc.nodes[i];
        let [tx, ty, tz] = n.transform.translation;
        let [qx, qy, qz, qw] = n.transform.rotation_quat_xyzw;
        let [sx, sy, sz] = n.transform.scale;

        let children: Vec<usize> = doc.nodes.iter().enumerate()
            .filter(|(ci, cn)| cn.parent_id == Some(n.id) && visible_set.contains(ci))
            .filter_map(|(ci, _)| doc_to_gltf.get(&ci).copied())
            .collect();

        let mut obj = json!({
            "name": n.name,
            "translation": [tx, ty, tz],
            "rotation": [qx, qy, qz, qw],
            "scale": [sx, sy, sz],
        });

        if !children.is_empty() {
            obj["children"] = json!(children);
        }
        if let Some(&mi) = node_mesh.get(&i) { obj["mesh"] = json!(mi); }
        if let Some(&ci) = node_cam.get(&i)  { obj["camera"] = json!(ci); }

        if let Some(&li) = node_light.get(&i) {
            obj["extensions"] = json!({
                "KHR_lights_punctual": { "light": li }
            });
        }

        // Non-renderable nodes: write useful metadata into extras
        let extras = match &n.payload {
            XrdsSceneNodePayload::GltfAsset(a) =>
                Some(json!({ "xrds_gltf_source_uri": a.asset_uri })),
            XrdsSceneNodePayload::AudioClip(a) =>
                Some(json!({ "xrds_audio_asset_id": a.asset_id })),
            _ => None,
        };
        if let Some(ex) = extras { obj["extras"] = ex; }

        gltf_nodes.push(obj);
    }

    // Root nodes (no parent)
    let root_nodes: Vec<usize> = visible.iter().enumerate()
        .filter(|(_, &di)| doc.nodes[di].parent_id.is_none())
        .map(|(gi, _)| gi)
        .collect();

    // ── Assemble glTF root ────────────────────────────────────────────────────
    let mut root = json!({
        "asset": { "version": "2.0", "generator": "xrds-gltf" },
        "scene": 0,
        "scenes": [{ "name": doc.metadata.name, "nodes": root_nodes }],
        "nodes": gltf_nodes,
    });

    if !materials.is_empty() { root["materials"] = json!(materials); }
    if !meshes.is_empty()    { root["meshes"]    = json!(meshes); }
    if !cameras.is_empty()   { root["cameras"]   = json!(cameras); }

    if !lights.is_empty() {
        root["extensions"] = json!({ "KHR_lights_punctual": { "lights": lights } });
    }

    if !extensions_used.is_empty() {
        root["extensionsUsed"] = json!(extensions_used);
    }

    if !buf.bytes.is_empty() {
        root["buffers"] = json!([{ "byteLength": buf.bytes.len() }]);
        root["bufferViews"] = json!(buf.buffer_views);
        root["accessors"] = json!(buf.accessors);
    }

    let json_bytes = serde_json::to_vec(&root).map_err(GlbExportError::Json)?;
    Ok(glb::write_glb(&json_bytes, &buf.bytes))
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn push_material(
    materials: &mut Vec<Value>,
    mat: &xrds_scene_graph::XrdsSceneMaterial,
    extensions_used: &mut Vec<&'static str>,
) -> usize {
    let [r, g, b, a] = mat.base_color;
    let [er, eg, eb, _] = mat.emissive;
    let alpha_mode = if mat.opacity < 1.0 || a < 1.0 { "BLEND" } else { "OPAQUE" };

    let mut m = json!({
        "pbrMetallicRoughness": {
            "baseColorFactor": [r, g, b, a],
            "metallicFactor": mat.pbr.metallic,
            "roughnessFactor": mat.pbr.roughness,
        },
        "emissiveFactor": [er, eg, eb],
        "alphaMode": alpha_mode,
        "alphaCutoff": mat.pbr.alpha_cutoff,
        "doubleSided": mat.pbr.double_sided,
    });

    if mat.unlit {
        if !extensions_used.contains(&"KHR_materials_unlit") {
            extensions_used.push("KHR_materials_unlit");
        }
        m["extensions"] = json!({ "KHR_materials_unlit": {} });
    }

    let idx = materials.len();
    materials.push(m);
    idx
}

fn push_mesh(meshes: &mut Vec<Value>, acc: MeshAccessors, mat_idx: usize) -> usize {
    let idx = meshes.len();
    meshes.push(json!({
        "primitives": [{
            "attributes": {
                "POSITION": acc.position,
                "NORMAL":   acc.normal,
                "TEXCOORD_0": acc.texcoord,
            },
            "indices": acc.indices,
            "material": mat_idx,
            "mode": 4  // TRIANGLES
        }]
    }));
    idx
}

fn build_camera(cam: &xrds_scene_graph::XrdsSceneCamera) -> Value {
    match cam.projection {
        XrdsSceneCameraProjection::Perspective { fov_deg, near, far, .. } => {
            let mut p = json!({ "yfov": fov_deg.to_radians(), "znear": near });
            if let Some(f) = far { p["zfar"] = json!(f); }
            json!({ "type": "perspective", "perspective": p })
        }
        XrdsSceneCameraProjection::Orthographic { scale, near, far, .. } => {
            json!({
                "type": "orthographic",
                "orthographic": { "xmag": scale, "ymag": scale, "znear": near, "zfar": far }
            })
        }
    }
}
