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
fn action_target_and_value_defaults_are_self_node_and_fixed_zero() {
    assert_eq!(XrdsActionTarget::default(), XrdsActionTarget::SelfNode);
    assert_eq!(XrdsActionValue::default(), XrdsActionValue::Fixed(0.0));
}

#[test]
fn modify_health_target_defaults_when_omitted_from_json() {
    let json = r#"{ "ModifyHealth": { "delta": { "Fixed": -10.0 } } }"#;
    let action: XrdsAction = serde_json::from_str(json).expect("deserialise");
    assert_eq!(
        action,
        XrdsAction::ModifyHealth {
            target: XrdsActionTarget::SelfNode,
            delta: XrdsActionValue::Fixed(-10.0),
        }
    );
}
