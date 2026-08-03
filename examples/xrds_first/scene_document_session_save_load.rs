use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

use xrds::scene_graph::{
    XrdsEditorMetadata, XrdsSceneCube, XrdsSceneDocument, XrdsSceneDocumentSession,
    XrdsSceneMaterial, XrdsSceneMetadata, XrdsSceneNode, XrdsSceneNodeId, XrdsSceneNodePayload,
    XrdsSceneTransform,
};

fn main() {
    let unique_suffix = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time should be after unix epoch")
        .as_nanos();
    let path =
        std::env::temp_dir().join(format!("xrds-scene-session-save-load-{unique_suffix}.json"));

    let mut session =
        XrdsSceneDocumentSession::new(authored_scene_document()).expect("document is valid");

    println!("Initial dirty state: {}", session.is_dirty());

    session
        .save_as(&path)
        .expect("session should save to a new path");
    println!("Saved draft scene to: {}", path.display());

    session
        .set_node_tags(
            XrdsSceneNodeId(2),
            vec![
                " gameplay ".to_string(),
                "hero".to_string(),
                "hero".to_string(),
            ],
        )
        .expect("setting tags should succeed");
    session
        .set_node_locked(XrdsSceneNodeId(2), true)
        .expect("setting lock state should succeed");
    session
        .edit(|document| {
            document.metadata.name = "Session Save Load Example".to_string();
        })
        .expect("metadata edit should succeed");

    println!(
        "After edits: dirty={}, can_undo={}, can_redo={}",
        session.is_dirty(),
        session.can_undo(),
        session.can_redo()
    );

    session.save().expect("session should save in place");
    println!("Saved edited scene document.");

    let loaded = XrdsSceneDocumentSession::load_json(&path).expect("session should load from disk");

    println!(
        "Reloaded scene: name='{}', dirty={}, save_path={} ",
        loaded.document().metadata.name,
        loaded.is_dirty(),
        loaded
            .save_path()
            .expect("loaded session should keep save path")
            .display()
    );

    let node = loaded
        .document()
        .node(XrdsSceneNodeId(2))
        .expect("saved cube node should exist");
    println!(
        "Reloaded node metadata: tags={:?}, locked={}",
        node.editor.tags, node.editor.locked
    );

    println!(
        "Reloaded JSON:\n{}",
        loaded
            .document()
            .to_json_string_pretty()
            .expect("loaded document should serialize")
    );

    fs::remove_file(&path).expect("temporary scene document should be removable");
}

fn authored_scene_document() -> XrdsSceneDocument {
    XrdsSceneDocument {
        metadata: XrdsSceneMetadata {
            name: "Draft Scene".to_string(),
            authored_by: Some("xrds example".to_string()),
            ..Default::default()
        },
        nodes: vec![
            XrdsSceneNode {
                id: XrdsSceneNodeId(1),
                parent_id: None,
                name: "Root".to_string(),
                enabled: true,
                visible: true,
                transform: XrdsSceneTransform::default(),
                payload: XrdsSceneNodePayload::Empty,
                editor: XrdsEditorMetadata::default(),
                grabbable: false,
                triggers: Vec::new(),
                watchers: Vec::new(),
            },
            XrdsSceneNode {
                id: XrdsSceneNodeId(2),
                parent_id: Some(XrdsSceneNodeId(1)),
                name: "Cube".to_string(),
                enabled: true,
                visible: true,
                transform: XrdsSceneTransform {
                    translation: [0.0, 1.0, 0.0],
                    ..Default::default()
                },
                payload: XrdsSceneNodePayload::Cube(XrdsSceneCube {
                    size: [1.5, 1.5, 1.5],
                    material: XrdsSceneMaterial {
                        base_color: [0.24, 0.64, 0.96, 1.0],
                        emissive: [0.02, 0.04, 0.08, 1.0],
                        opacity: 1.0,
                        unlit: false,
                        pbr: Default::default(),
                        textures: Default::default(),
                    },
                    ..Default::default()
                }),
                editor: XrdsEditorMetadata::default(),
                grabbable: false,
                triggers: Vec::new(),
                watchers: Vec::new(),
            },
        ],
        ..Default::default()
    }
}
