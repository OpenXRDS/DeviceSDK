use super::*;

#[test]
fn world_panel_round_trip() {
    let doc = make_world_panel_document();
    let json = serde_json::to_string_pretty(&doc).expect("serialise");
    let restored: XrdsSceneDocument = serde_json::from_str(&json).expect("deserialise");
    assert_eq!(doc, restored, "round-trip produced different document");
}

#[test]
fn world_panel_minimal_json_compact() {
    let doc = make_minimal_world_panel_document();
    let json = serde_json::to_string(&doc).expect("serialise");

    assert!(!json.contains("corner_radius"), "corner_radius should be skipped when 0.0");
    assert!(!json.contains("\"opacity\""), "opacity should be skipped when 1.0");
    assert!(!json.contains("\"layout\""), "layout should be skipped when None");
    assert!(!json.contains("\"widgets\""), "widgets should be skipped when empty");

    let restored: XrdsSceneDocument = serde_json::from_str(&json).expect("deserialise");
    assert_eq!(doc, restored);
}

#[test]
fn world_panel_to_runtime_node() {
    use XrdsSceneRuntimeComponent::WorldPanel;

    let doc = make_world_panel_document();
    let panel_node = doc.nodes.iter()
        .find(|n| matches!(n.payload, XrdsSceneNodePayload::WorldPanel(_)))
        .expect("no WorldPanel node in test document");

    let runtime = panel_node.to_runtime_node();
    let WorldPanel(panel_desc, widgets, layout) = runtime.component else {
        panic!("expected WorldPanel runtime component");
    };

    assert_eq!(panel_desc.size, [0.5, 0.35]);
    assert_eq!(widgets.len(), 2, "expected 2 widgets");
    assert!(matches!(widgets[0], XrdsSceneWorldWidget::Label(_)));
    assert!(matches!(widgets[1], XrdsSceneWorldWidget::Button(_)));
    assert!(matches!(layout, XrdsSceneWorldLayout::VStack { gap } if (gap - 0.01).abs() < 1e-6));
}

fn make_world_panel_document() -> XrdsSceneDocument {
    XrdsSceneDocument {
        version: 1,
        metadata: XrdsSceneMetadata { name: "test".to_string(), ..Default::default() },
        assets: vec![],
        nodes: vec![
            XrdsSceneNode {
                id: XrdsSceneNodeId(1),
                parent_id: None,
                name: "Panel".to_string(),
                enabled: true,
                visible: true,
                grabbable: false,
                transform: XrdsSceneTransform::default(),
                payload: XrdsSceneNodePayload::WorldPanel(XrdsSceneWorldPanel {
                    size: [0.5, 0.35],
                    color: [0.1, 0.1, 0.1, 0.9],
                    corner_radius: 0.015,
                    opacity: 0.95,
                    layout: XrdsSceneWorldLayout::VStack { gap: 0.01 },
                    widgets: vec![
                        XrdsSceneWorldWidget::Label(XrdsSceneWorldLabel {
                            text: "Hello, XRDS!".to_string(),
                            font_size: 0.05,
                            color: [1.0, 1.0, 1.0, 1.0],
                            local_position: [0.0, 0.1],
                            layout_size: [0.4, 0.06],
                        }),
                        XrdsSceneWorldWidget::Button(XrdsSceneWorldButton {
                            label: "OK".to_string(),
                            font_size: 0.04,
                            label_color: [1.0, 1.0, 1.0, 1.0],
                            size: [0.14, 0.06],
                            local_position: [0.0, 0.0],
                            normal_color:  [0.2, 0.2, 0.5, 1.0],
                            hover_color:   [0.3, 0.3, 0.7, 1.0],
                            pressed_color: [0.1, 0.1, 0.3, 1.0],
                        }),
                    ],
                }),
                editor: XrdsEditorMetadata::default(),
                triggers: Vec::new(),
                watchers: Vec::new(),
            },
        ],
        ..Default::default()
    }
}

fn make_minimal_world_panel_document() -> XrdsSceneDocument {
    XrdsSceneDocument {
        version: 1,
        metadata: XrdsSceneMetadata { name: "minimal".to_string(), ..Default::default() },
        assets: vec![],
        nodes: vec![
            XrdsSceneNode {
                id: XrdsSceneNodeId(1),
                parent_id: None,
                name: "Panel".to_string(),
                enabled: true,
                visible: true,
                grabbable: false,
                transform: XrdsSceneTransform::default(),
                payload: XrdsSceneNodePayload::WorldPanel(XrdsSceneWorldPanel {
                    size: [0.4, 0.3],
                    color: [0.08, 0.08, 0.08, 0.92],
                    corner_radius: 0.0,
                    opacity: 1.0,
                    layout: XrdsSceneWorldLayout::None,
                    widgets: vec![],
                }),
                editor: XrdsEditorMetadata::default(),
                triggers: Vec::new(),
                watchers: Vec::new(),
            },
        ],
        ..Default::default()
    }
}
