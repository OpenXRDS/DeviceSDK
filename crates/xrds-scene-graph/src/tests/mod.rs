pub(super) use super::*;

use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

mod assets;
mod core;
mod editing;
mod naming;
mod panel;
mod session;
mod trigger_action;

pub(super) fn asset_fixture_path(relative_path: &str) -> String {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../assets")
        .join(relative_path)
        .to_string_lossy()
        .into_owned()
}

pub(super) fn unique_temp_json_path(prefix: &str) -> PathBuf {
    let unique_suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_nanos();
    std::env::temp_dir().join(format!("{prefix}-{unique_suffix}.json"))
}

pub(super) fn persistence_test_document() -> XrdsSceneDocument {
    XrdsSceneDocument {
        metadata: XrdsSceneMetadata {
            name: "Persistence Test".to_string(),
            authored_by: Some("xrds-tests".to_string()),
            default_scene_label: Some("Main".to_string()),
            environment: None,
            extras: [("theme".to_string(), "industrial".to_string())]
                .into_iter()
                .collect(),
            ..Default::default()
        },
        assets: vec![XrdsSceneAsset {
            id: "asset:lamp".to_string(),
            uri: "models/StainedGlassLamp/StainedGlassLamp.gltf".to_string(),
            kind: XrdsSceneAssetKind::Gltf,
        }],
        nodes: vec![
            XrdsSceneNode {
                id: XrdsSceneNodeId(10),
                parent_id: None,
                name: "Root".to_string(),
                enabled: true,
                visible: true,
                transform: XrdsSceneTransform::default(),
                payload: XrdsSceneNodePayload::Empty,
                grabbable: false,
                editor: XrdsEditorMetadata {
                    tags: vec!["folder".to_string(), "root".to_string()],
                    layer: Some("Scene".to_string()),
                    locked: true,
                    hidden_in_editor: false,
                    user_properties: [("expanded".to_string(), "true".to_string())]
                        .into_iter()
                        .collect(),
                    source: Some(XrdsSourceLink {
                        asset_id: Some("asset:lamp".to_string()),
                        source_node: Some("RootNode".to_string()),
                        import_revision: Some("rev-1".to_string()),
                    }),
                },
                triggers: Vec::new(),
                watchers: Vec::new(),
            },
            XrdsSceneNode {
                id: XrdsSceneNodeId(11),
                parent_id: Some(XrdsSceneNodeId(10)),
                name: "Lamp".to_string(),
                enabled: true,
                visible: true,
                transform: XrdsSceneTransform {
                    translation: [1.0, 2.0, 3.0],
                    rotation_quat_xyzw: [0.0, 0.0, 0.0, 1.0],
                    scale: [1.5, 1.5, 1.5],
                },
                payload: XrdsSceneNodePayload::GltfAsset(XrdsSceneGltfAsset {
                    asset_id: Some("asset:lamp".to_string()),
                    asset_uri: "models/StainedGlassLamp/StainedGlassLamp.gltf".to_string(),
                    scene_index: 0,
                    export_policy: XrdsGltfAssetExportPolicy::KeepExternalReference,
                }),
                grabbable: false,
                editor: XrdsEditorMetadata {
                    tags: vec!["mesh".to_string()],
                    layer: Some("Gameplay".to_string()),
                    locked: false,
                    hidden_in_editor: true,
                    user_properties: [
                        ("selected".to_string(), "false".to_string()),
                        ("note".to_string(), "keep".to_string()),
                    ]
                    .into_iter()
                    .collect(),
                    source: Some(XrdsSourceLink {
                        asset_id: Some("asset:lamp".to_string()),
                        source_node: Some("LampNode".to_string()),
                        import_revision: Some("rev-2".to_string()),
                    }),
                },
                triggers: Vec::new(),
                watchers: Vec::new(),
            },
        ],
        ..Default::default()
    }
}