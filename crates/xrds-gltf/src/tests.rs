use xrds_scene_graph::{
    XrdsEditorMetadata, XrdsSceneDocument, XrdsSceneMetadata, XrdsSceneNode, XrdsSceneNodeId,
    XrdsSceneNodePayload, XrdsSceneSphere, XrdsSceneMaterial, XrdsSceneMaterialPbrParams,
    XrdsSceneTransform,
};
use crate::{export_glb, tessellation};

fn sphere_doc() -> XrdsSceneDocument {
    XrdsSceneDocument {
        metadata: XrdsSceneMetadata { name: "TestScene".into(), ..Default::default() },
        nodes: vec![XrdsSceneNode {
            id: XrdsSceneNodeId(1),
            parent_id: None,
            name: "MySphere".into(),
            enabled: true,
            visible: true,
            transform: XrdsSceneTransform {
                translation: [1.0, 2.0, 3.0],
                rotation_quat_xyzw: [0.0, 0.0, 0.0, 1.0],
                scale: [1.0, 1.0, 1.0],
            },
            payload: XrdsSceneNodePayload::Sphere(XrdsSceneSphere {
                radius: 2.0,
                material: XrdsSceneMaterial {
                    base_color: [1.0, 0.0, 0.0, 1.0],
                    pbr: XrdsSceneMaterialPbrParams {
                        roughness: 0.4,
                        metallic: 0.0,
                        ..Default::default()
                    },
                    ..Default::default()
                },
                ..Default::default()
            }),
            grabbable: false,
            editor: XrdsEditorMetadata::default(),
        }],
        ..Default::default()
    }
}

// ── ST-10 tests ───────────────────────────────────────────────────────────────

#[test]
fn export_glb_produces_valid_glb_header() {
    let bytes = export_glb(&sphere_doc()).expect("export should succeed");
    // GLB magic = "glTF" = 0x46546C67 little-endian
    assert_eq!(&bytes[0..4], b"glTF", "GLB magic bytes");
    // Version = 2
    let version = u32::from_le_bytes(bytes[4..8].try_into().unwrap());
    assert_eq!(version, 2, "GLB version");
    // Total length matches
    let total = u32::from_le_bytes(bytes[8..12].try_into().unwrap());
    assert_eq!(total as usize, bytes.len(), "GLB total length matches file size");
}

#[test]
fn export_glb_json_is_parseable() {
    let bytes = export_glb(&sphere_doc()).expect("export should succeed");
    // JSON chunk starts at byte 12
    let json_chunk_len = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
    let json_bytes = &bytes[20..20 + json_chunk_len];
    let root: serde_json::Value =
        serde_json::from_slice(json_bytes).expect("JSON chunk should be valid JSON");
    assert_eq!(root["asset"]["version"], "2.0");
    assert_eq!(root["scene"], 0);
    assert!(!root["nodes"].as_array().unwrap().is_empty());
}

#[test]
fn export_glb_node_name_and_translation_preserved() {
    let bytes = export_glb(&sphere_doc()).expect("export should succeed");
    let json_chunk_len =
        u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
    let root: serde_json::Value =
        serde_json::from_slice(&bytes[20..20 + json_chunk_len]).unwrap();

    let node = &root["nodes"][0];
    assert_eq!(node["name"], "MySphere");
    let t = node["translation"].as_array().unwrap();
    assert!((t[0].as_f64().unwrap() - 1.0).abs() < 1e-5);
    assert!((t[1].as_f64().unwrap() - 2.0).abs() < 1e-5);
    assert!((t[2].as_f64().unwrap() - 3.0).abs() < 1e-5);
}

#[test]
fn export_glb_material_base_color_preserved() {
    let bytes = export_glb(&sphere_doc()).expect("export should succeed");
    let json_chunk_len =
        u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
    let root: serde_json::Value =
        serde_json::from_slice(&bytes[20..20 + json_chunk_len]).unwrap();

    let bcf = &root["materials"][0]["pbrMetallicRoughness"]["baseColorFactor"];
    assert!((bcf[0].as_f64().unwrap() - 1.0).abs() < 1e-5, "red=1");
    assert!((bcf[1].as_f64().unwrap() - 0.0).abs() < 1e-5, "green=0");
    assert!((bcf[2].as_f64().unwrap() - 0.0).abs() < 1e-5, "blue=0");
}

#[test]
fn sphere_tessellation_vertex_and_index_count() {
    let m = tessellation::sphere(1.0, 24, 16);
    // (lon+1)*(lat+1) vertices
    assert_eq!(m.vertex_count(), 25 * 17, "sphere vertex count");
    // lon*lat*2 triangles, 3 indices each
    assert_eq!(m.idx.len(), 24 * 16 * 6, "sphere index count");
}

#[test]
fn cuboid_tessellation_vertex_and_index_count() {
    let m = tessellation::cuboid(1.0, 1.0, 1.0);
    assert_eq!(m.vertex_count(), 24, "cube has 24 vertices (4 per face)");
    assert_eq!(m.idx.len(), 36, "cube has 36 indices (2 triangles × 3 × 6 faces)");
}

#[test]
fn hidden_nodes_excluded_from_export() {
    let mut doc = sphere_doc();
    doc.nodes[0].visible = false;
    let bytes = export_glb(&doc).expect("export should succeed");
    let json_chunk_len =
        u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
    let root: serde_json::Value =
        serde_json::from_slice(&bytes[20..20 + json_chunk_len]).unwrap();
    assert!(
        root["nodes"].as_array().unwrap().is_empty(),
        "invisible node should be excluded"
    );
}
