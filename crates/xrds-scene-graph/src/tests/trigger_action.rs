use super::*;

#[test]
fn trigger_binding_round_trips_every_v1_action_variant() {
    let binding = XrdsTriggerBinding {
        trigger: XrdsTriggerKind::ZoneEnter,
        sequence: XrdsSequence {
            steps: vec![
                XrdsAction::PlayGltfAnimation {
                    playback: XrdsSceneGltfPlayback {
                        selector: XrdsSceneGltfAnimationSelector::Name("Open".to_string()),
                        repeat: XrdsSceneAnimationRepeatMode::Once,
                        speed: 1.5,
                        start_paused: false,
                    },
                },
                XrdsAction::StopGltfAnimation,
                XrdsAction::SetVisible(false),
                XrdsAction::Teleport { destination: [1.0, 2.0, 3.0] },
                XrdsAction::ModifyHealth {
                    target: XrdsActionTarget::TriggerSource,
                    delta: XrdsActionValue::FromTriggerSource,
                },
                XrdsAction::Wait { seconds: 0.5 },
                XrdsAction::FireCustomEvent { name: "door_opened".to_string() },
            ],
        },
        disabled: false,
        hand: None,
    };

    let json = serde_json::to_string_pretty(&binding).expect("serialise");
    let restored: XrdsTriggerBinding = serde_json::from_str(&json).expect("deserialise");
    assert_eq!(binding, restored, "round-trip produced a different binding");
}

#[test]
fn trigger_binding_minimal_json_uses_defaults_and_deserializes_from_empty_object() {
    // An older/hand-authored document with no fields at all should still
    // deserialize, given every field here has a #[serde(default)] — this
    // is the additive-schema-evolution guarantee the implementation plan
    // calls for in Phase 1.
    let restored: XrdsTriggerBinding = serde_json::from_str("{}").expect("deserialise from {}");
    assert_eq!(restored.trigger, XrdsTriggerKind::ZoneEnter);
    assert_eq!(restored.sequence, XrdsSequence::default());
    assert!(restored.sequence.steps.is_empty());
}

#[test]
fn unknown_action_variant_does_not_destroy_the_whole_document() {
    // The failure this guards against: a scene authored by a newer editor
    // containing an action this build has never heard of. Without the
    // Unknown fallback, serde errors on the unknown variant and — because
    // the action is nested inside the document — the ENTIRE scene fails to
    // load, not just that step.
    //
    // Built by serializing a real document and renaming one action's tag,
    // rather than hand-writing document JSON that would break every time an
    // unrelated field is added to the schema.
    let doc = XrdsSceneDocument {
        metadata: XrdsSceneMetadata { name: "from-a-newer-editor".to_string(), ..Default::default() },
        nodes: vec![XrdsSceneNode {
            id: XrdsSceneNodeId(1),
            parent_id: None,
            name: "Door".to_string(),
            enabled: true,
            visible: true,
            grabbable: false,
            transform: XrdsSceneTransform::default(),
            payload: XrdsSceneNodePayload::Empty,
            editor: XrdsEditorMetadata::default(),
            triggers: vec![XrdsTriggerBinding {
                trigger: XrdsTriggerKind::ZoneEnter,
                sequence: XrdsSequence {
                    steps: vec![
                        // Stands in for the future action; renamed below.
                        XrdsAction::StopGltfAnimation,
                        XrdsAction::Teleport { destination: [1.0, 2.0, 3.0] },
                    ],
                },
                disabled: false,
                hand: None,
            }],
        }],
        ..Default::default()
    };

    let json = serde_json::to_string(&doc)
        .expect("serialise")
        .replace("\"StopGltfAnimation\"", "\"PlayAudio\"");
    assert!(json.contains("PlayAudio"), "test setup: the rename must have applied");

    let doc: XrdsSceneDocument =
        serde_json::from_str(&json).expect("document with an unknown action must still load");

    assert_eq!(doc.nodes.len(), 1, "the node must survive");
    assert_eq!(doc.nodes[0].name, "Door");

    let steps = &doc.nodes[0].triggers[0].sequence.steps;
    assert_eq!(steps.len(), 2, "both steps should be present");
    assert_eq!(
        steps[0],
        XrdsAction::Unknown,
        "the unrecognized action should degrade to Unknown"
    );
    assert_eq!(
        steps[1],
        XrdsAction::Teleport { destination: [1.0, 2.0, 3.0] },
        "the recognized action after it must be unaffected"
    );
}

#[test]
fn unknown_trigger_kind_loads_and_is_inert() {
    let json = r#"{ "trigger": { "kind": "SomeFutureTrigger" },
                    "sequence": { "steps": [] } }"#; // adjacently tagged
    let binding: XrdsTriggerBinding =
        serde_json::from_str(json).expect("unknown trigger kind must still load");
    assert_eq!(binding.trigger, XrdsTriggerKind::Unknown);
    // Nothing emits Unknown, so this binding can never fire — inert, not
    // misfiring.
}

#[test]
fn action_target_and_value_defaults_are_self_node_and_fixed_zero() {
    assert_eq!(XrdsActionTarget::default(), XrdsActionTarget::SelfNode);
    assert_eq!(XrdsActionValue::default(), XrdsActionValue::Fixed(0.0));
}

#[test]
fn modify_health_target_defaults_when_omitted_from_json() {
    // Adjacently tagged since the Unknown fallback was added; XrdsActionValue
    // itself is still externally tagged, hence the bare `{"Fixed": ...}`.
    let json = r#"{ "kind": "ModifyHealth", "data": { "delta": { "Fixed": -10.0 } } }"#;
    let action: XrdsAction = serde_json::from_str(json).expect("deserialise");
    assert_eq!(
        action,
        XrdsAction::ModifyHealth {
            target: XrdsActionTarget::SelfNode,
            delta: XrdsActionValue::Fixed(-10.0),
        }
    );
}

#[test]
fn trigger_diagnostics_catch_the_silent_failure_modes() {
    use XrdsSceneTriggerDiagnosticSeverity as Severity;

    let doc = XrdsSceneDocument {
        metadata: XrdsSceneMetadata { name: "diag".to_string(), ..Default::default() },
        nodes: vec![XrdsSceneNode {
            id: XrdsSceneNodeId(1),
            parent_id: None,
            name: "Thing".to_string(),
            enabled: true,
            visible: true,
            grabbable: false,
            transform: XrdsSceneTransform::default(),
            // Not a glTF payload — so the animation action below is bogus.
            payload: XrdsSceneNodePayload::Empty,
            editor: XrdsEditorMetadata::default(),
            triggers: vec![
                XrdsTriggerBinding {
                    trigger: XrdsTriggerKind::ZoneEnter,
                    sequence: XrdsSequence {
                        steps: vec![
                            XrdsAction::StopGltfAnimation,
                            XrdsAction::ModifyHealth {
                                target: XrdsActionTarget::Node(XrdsSceneNodeId(999)),
                                delta: XrdsActionValue::Fixed(-1.0),
                            },
                            XrdsAction::FireCustomEvent { name: "nobody_listens".to_string() },
                        ],
                    },
                    disabled: false,
                    hand: None,
                },
                // Listens for a name nothing fires.
                XrdsTriggerBinding {
                    trigger: XrdsTriggerKind::Custom("never_fired".to_string()),
                    sequence: XrdsSequence { steps: vec![] },
                    disabled: false,
                    hand: None,
                },
            ],
        }],
        ..Default::default()
    };

    let diags = doc.trigger_diagnostics();
    let titles: Vec<&str> = diags.iter().map(|d| d.title.as_str()).collect();

    assert!(
        titles.iter().any(|t| t.contains("non-glTF node")),
        "should flag a glTF animation action on a non-glTF node, got {titles:?}"
    );
    assert!(
        diags.iter().any(|d| d.severity == Severity::Error
            && d.title.contains("node that does not exist")),
        "a dangling node target is unworkable and must be an Error, got {diags:?}"
    );
    assert!(
        titles.iter().any(|t| t.contains("no listener")),
        "should flag a fired custom event nothing listens for, got {titles:?}"
    );
    assert!(
        titles.iter().any(|t| t.contains("no emitter")),
        "should flag a custom trigger nothing emits, got {titles:?}"
    );
    assert!(
        titles.iter().any(|t| t.contains("Empty sequence")),
        "should flag the empty sequence, got {titles:?}"
    );
}

#[test]
fn trigger_diagnostics_are_quiet_on_a_healthy_document() {
    let doc = XrdsSceneDocument {
        metadata: XrdsSceneMetadata { name: "healthy".to_string(), ..Default::default() },
        nodes: vec![XrdsSceneNode {
            id: XrdsSceneNodeId(1),
            parent_id: None,
            name: "Pad".to_string(),
            enabled: true,
            visible: true,
            grabbable: false,
            transform: XrdsSceneTransform::default(),
            payload: XrdsSceneNodePayload::Empty,
            editor: XrdsEditorMetadata::default(),
            triggers: vec![
                XrdsTriggerBinding {
                    trigger: XrdsTriggerKind::ZoneEnter,
                    sequence: XrdsSequence {
                        steps: vec![
                            XrdsAction::Teleport { destination: [1.0, 0.0, 0.0] },
                            XrdsAction::FireCustomEvent { name: "arrived".to_string() },
                        ],
                    },
                    disabled: false,
                    hand: None,
                },
                XrdsTriggerBinding {
                    trigger: XrdsTriggerKind::Custom("arrived".to_string()),
                    sequence: XrdsSequence {
                        steps: vec![XrdsAction::SetVisible(false)],
                    },
                    disabled: false,
                    hand: None,
                },
            ],
        }],
        ..Default::default()
    };

    assert_eq!(
        doc.trigger_diagnostics(),
        vec![],
        "a document whose custom event and listener match should produce no diagnostics"
    );
}

#[test]
fn disabled_flag_defaults_to_false_so_existing_documents_stay_active() {
    // The trap this guards against: if the field were named `enabled`,
    // serde's bool default of `false` would silently switch off every
    // binding in every existing document on load. The negative name makes
    // the default correct.
    let restored: XrdsTriggerBinding = serde_json::from_str("{}").expect("deserialise");
    assert!(!restored.disabled, "a binding with no flag present must be active");

    // And it stays out of serialized output when unset.
    let json = serde_json::to_string(&XrdsTriggerBinding::default()).expect("serialise");
    assert!(
        !json.contains("disabled"),
        "an unset flag should not appear in output, got {json}"
    );
}

#[test]
fn diagnostics_stay_quiet_about_disabled_bindings() {
    // A parked binding is deliberately inert, so nagging about its contents
    // is noise. Anything genuinely wrong resurfaces when it is re-enabled.
    let broken_but_parked = XrdsTriggerBinding {
        trigger: XrdsTriggerKind::Custom("never_fired".to_string()),
        sequence: XrdsSequence { steps: vec![] }, // empty AND unlistened-for
        disabled: true,
        hand: None,
    };

    let doc = XrdsSceneDocument {
        metadata: XrdsSceneMetadata { name: "parked".to_string(), ..Default::default() },
        nodes: vec![XrdsSceneNode {
            id: XrdsSceneNodeId(1),
            parent_id: None,
            name: "Thing".to_string(),
            enabled: true,
            visible: true,
            grabbable: false,
            transform: XrdsSceneTransform::default(),
            payload: XrdsSceneNodePayload::Empty,
            editor: XrdsEditorMetadata::default(),
            triggers: vec![broken_but_parked.clone()],
        }],
        ..Default::default()
    };
    assert_eq!(
        doc.trigger_diagnostics(),
        vec![],
        "a disabled binding should produce no diagnostics"
    );

    // Re-enabling it surfaces the problems again.
    let mut doc = doc;
    doc.nodes[0].triggers[0].disabled = false;
    assert!(
        !doc.trigger_diagnostics().is_empty(),
        "re-enabling must bring the diagnostics back"
    );
}

#[test]
fn hand_filter_round_trips_and_defaults_to_none() {
    let with_hand = XrdsTriggerBinding {
        trigger: XrdsTriggerKind::Grabbed,
        sequence: XrdsSequence { steps: vec![] },
        disabled: false,
        hand: Some(xrds_components::XrGrabHand::Left),
    };
    let json = serde_json::to_string(&with_hand).expect("serialise");
    let restored: XrdsTriggerBinding = serde_json::from_str(&json).expect("deserialise");
    assert_eq!(with_hand, restored);

    // Absent from JSON entirely (older document) -> None, not an error.
    let restored: XrdsTriggerBinding = serde_json::from_str("{}").expect("deserialise from {}");
    assert_eq!(restored.hand, None);

    // And an unset filter stays out of serialized output.
    let no_hand = XrdsTriggerBinding::default();
    let json = serde_json::to_string(&no_hand).expect("serialise");
    assert!(!json.contains("hand"), "an unset hand filter should not appear in output, got {json}");
}

#[test]
fn diagnostics_flag_a_hand_filter_on_a_handless_trigger_kind() {
    let doc = XrdsSceneDocument {
        metadata: XrdsSceneMetadata { name: "bad-hand".to_string(), ..Default::default() },
        nodes: vec![XrdsSceneNode {
            id: XrdsSceneNodeId(1),
            parent_id: None,
            name: "Zone".to_string(),
            enabled: true,
            visible: true,
            grabbable: false,
            transform: XrdsSceneTransform::default(),
            payload: XrdsSceneNodePayload::Empty,
            editor: XrdsEditorMetadata::default(),
            triggers: vec![XrdsTriggerBinding {
                // ZoneEnter never reports a hand, so this can never fire.
                trigger: XrdsTriggerKind::ZoneEnter,
                sequence: XrdsSequence {
                    steps: vec![XrdsAction::Teleport { destination: [1.0, 0.0, 0.0] }],
                },
                disabled: false,
                hand: Some(xrds_components::XrGrabHand::Left),
            }],
        }],
        ..Default::default()
    };

    let diags = doc.trigger_diagnostics();
    assert!(
        diags.iter().any(|d| {
            d.severity == XrdsSceneTriggerDiagnosticSeverity::Error
                && d.title.contains("Hand filter")
        }),
        "a hand filter on ZoneEnter must be flagged as an Error (unfireable), got {diags:?}"
    );
}

#[test]
fn diagnostics_allow_a_hand_filter_on_a_grab_binding() {
    let doc = XrdsSceneDocument {
        metadata: XrdsSceneMetadata { name: "good-hand".to_string(), ..Default::default() },
        nodes: vec![XrdsSceneNode {
            id: XrdsSceneNodeId(1),
            parent_id: None,
            name: "Item".to_string(),
            enabled: true,
            visible: true,
            grabbable: true,
            transform: XrdsSceneTransform::default(),
            payload: XrdsSceneNodePayload::Empty,
            editor: XrdsEditorMetadata::default(),
            triggers: vec![XrdsTriggerBinding {
                trigger: XrdsTriggerKind::Grabbed,
                sequence: XrdsSequence {
                    steps: vec![XrdsAction::Teleport { destination: [1.0, 0.0, 0.0] }],
                },
                disabled: false,
                hand: Some(xrds_components::XrGrabHand::Right),
            }],
        }],
        ..Default::default()
    };

    assert_eq!(
        doc.trigger_diagnostics(),
        vec![],
        "a hand filter on a Grabbed binding is legitimate and should not be flagged"
    );
}
