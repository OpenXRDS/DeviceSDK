//! Behavior tests for trigger-action sequencing (Phases 3-4). Unlike the
//! Phase 2 round-trip test, these exercise the *live* path: a trigger
//! message is written, and the authored action's effect on the world is
//! asserted.
use super::*;
use crate::xrds_api::trigger_action::{
    XrdsCustomTriggerEvent, XrdsHealth, XrdsSequenceAgent, XrdsTriggerValue,
};

/// Imports a single node carrying `bindings`, returns its live entity.
fn import_node_with_triggers(
    app: &mut App,
    node_id: u64,
    bindings: Vec<XrdsTriggerBinding>,
) -> Entity {
    let document = XrdsSceneDocument {
        nodes: vec![XrdsSceneNode {
            id: XrdsSceneNodeId(node_id),
            parent_id: None,
            name: format!("TriggerNode{node_id}"),
            enabled: true,
            visible: true,
            transform: XrdsSceneTransform::default(),
            payload: XrdsSceneNodePayload::Empty,
            grabbable: false,
            editor: XrdsEditorMetadata::default(),
            triggers: bindings,
        }],
        ..Default::default()
    };

    {
        let mut xrds = XrdsAPI::attach(app);
        xrds.import_scene_document(&document)
            .expect("import should succeed");
    }

    app.world()
        .resource::<XrdsIdIndex>()
        .entity_of(XrdsId(node_id))
        .expect("imported node should be indexed")
}

/// Drives enough frames for a trigger message to be consumed and its
/// sequence's first action to run.
fn pump(app: &mut App, frames: usize) {
    for _ in 0..frames {
        app.update();
    }
}

#[test]
fn zone_enter_trigger_runs_authored_teleport_action() {
    let mut app = xrds_test_app();

    let entity = import_node_with_triggers(
        &mut app,
        930,
        vec![XrdsTriggerBinding {
            trigger: XrdsTriggerKind::ZoneEnter,
            sequence: XrdsSequence {
                steps: vec![XrdsAction::Teleport { destination: [5.0, 6.0, 7.0] }],
            },
        }],
    );

    // Sanity: nothing has moved it yet.
    assert_eq!(
        app.world().get::<Transform>(entity).map(|t| t.translation),
        Some(Vec3::ZERO),
        "node should start at the origin"
    );

    app.world_mut()
        .write_message(xrds_components::XrZoneEnterEvent {
            zone_id: XrdsId(930),
            entity_id: XrdsId(930),
        });

    pump(&mut app, 3);

    assert_eq!(
        app.world().get::<Transform>(entity).map(|t| t.translation),
        Some(Vec3::new(5.0, 6.0, 7.0)),
        "ZoneEnter should have run the authored Teleport action"
    );
}

#[test]
fn zone_exit_binding_does_not_fire_on_enter() {
    let mut app = xrds_test_app();

    let entity = import_node_with_triggers(
        &mut app,
        931,
        vec![XrdsTriggerBinding {
            trigger: XrdsTriggerKind::ZoneExit,
            sequence: XrdsSequence {
                steps: vec![XrdsAction::Teleport { destination: [9.0, 9.0, 9.0] }],
            },
        }],
    );

    app.world_mut()
        .write_message(xrds_components::XrZoneEnterEvent {
            zone_id: XrdsId(931),
            entity_id: XrdsId(931),
        });

    pump(&mut app, 3);

    assert_eq!(
        app.world().get::<Transform>(entity).map(|t| t.translation),
        Some(Vec3::ZERO),
        "a ZoneExit binding must not fire on a ZoneEnter event"
    );
}

#[test]
fn modify_health_reads_value_from_trigger_source() {
    let mut app = xrds_test_app();

    let target = import_node_with_triggers(
        &mut app,
        932,
        vec![XrdsTriggerBinding {
            trigger: XrdsTriggerKind::ZoneEnter,
            sequence: XrdsSequence {
                steps: vec![XrdsAction::ModifyHealth {
                    target: XrdsActionTarget::SelfNode,
                    delta: XrdsActionValue::FromTriggerSource,
                }],
            },
        }],
    );

    app.world_mut().entity_mut(target).insert(XrdsHealth(100.0));

    // The "bullet": a separate entity registered in the id index, carrying
    // the damage amount in the generic XrdsTriggerValue slot the way
    // ordinary gameplay code would.
    let source = app.world_mut().spawn(XrdsTriggerValue(-30.0)).id();
    app.world_mut()
        .resource_mut::<XrdsIdIndex>()
        .register(XrdsId(9320), source);

    app.world_mut()
        .write_message(xrds_components::XrZoneEnterEvent {
            zone_id: XrdsId(932),
            entity_id: XrdsId(9320),
        });

    pump(&mut app, 3);

    let health = app.world().get::<XrdsHealth>(target).map(|h| h.0);
    assert_eq!(
        health,
        Some(70.0),
        "ModifyHealth should have applied the source's XrdsTriggerValue (-30)"
    );
}

#[test]
fn two_distinct_sources_each_fire_their_own_sequence() {
    // The case that ruled out "ignore while a sequence is running": two
    // different sources firing the same trigger are independent, valid
    // events and must both run — matching Unity/Unreal/Godot semantics.
    let mut app = xrds_test_app();

    let target = import_node_with_triggers(
        &mut app,
        933,
        vec![XrdsTriggerBinding {
            trigger: XrdsTriggerKind::ZoneEnter,
            sequence: XrdsSequence {
                steps: vec![
                    // A Wait keeps the first sequence in flight while the
                    // second fires, so a suppress-while-running policy
                    // would visibly lose one of the two.
                    XrdsAction::Wait { seconds: 0.05 },
                    XrdsAction::ModifyHealth {
                        target: XrdsActionTarget::SelfNode,
                        delta: XrdsActionValue::Fixed(-10.0),
                    },
                ],
            },
        }],
    );
    app.world_mut().entity_mut(target).insert(XrdsHealth(100.0));

    for (index, source_id) in [9331_u64, 9332].into_iter().enumerate() {
        let source = app.world_mut().spawn_empty().id();
        app.world_mut()
            .resource_mut::<XrdsIdIndex>()
            .register(XrdsId(source_id), source);
        app.world_mut()
            .write_message(xrds_components::XrZoneEnterEvent {
                zone_id: XrdsId(933),
                entity_id: XrdsId(source_id),
            });
        // Advance one frame between the two so the first sequence is
        // genuinely mid-flight (inside its Wait) when the second arrives.
        if index == 0 {
            app.update();
        }
    }
    // Consume the second message too — consume_triggers only runs during an
    // update, so without this the second agent wouldn't exist yet.
    app.update();

    // Both agents should exist concurrently, each with its own queue.
    let live_agents = app
        .world_mut()
        .query::<&XrdsSequenceAgent>()
        .iter(app.world())
        .count();
    assert_eq!(
        live_agents, 2,
        "each firing should get its own ephemeral agent, not share one queue"
    );

    // Let both Waits elapse.
    for _ in 0..12 {
        app.update();
        std::thread::sleep(Duration::from_millis(10));
    }

    assert_eq!(
        app.world().get::<XrdsHealth>(target).map(|h| h.0),
        Some(80.0),
        "both firings should have applied their -10, not just one"
    );
}

#[test]
fn finished_sequence_agents_are_despawned() {
    let mut app = xrds_test_app();

    import_node_with_triggers(
        &mut app,
        934,
        vec![XrdsTriggerBinding {
            trigger: XrdsTriggerKind::ZoneEnter,
            sequence: XrdsSequence {
                steps: vec![XrdsAction::SetVisible(false)],
            },
        }],
    );

    app.world_mut()
        .write_message(xrds_components::XrZoneEnterEvent {
            zone_id: XrdsId(934),
            entity_id: XrdsId(934),
        });

    pump(&mut app, 5);

    let leftover = app
        .world_mut()
        .query::<&XrdsSequenceAgent>()
        .iter(app.world())
        .count();
    assert_eq!(
        leftover, 0,
        "ephemeral agents must be reaped once their queue drains, or every \
         trigger firing leaks an entity"
    );
}

#[test]
fn trigger_targeting_an_already_despawned_entity_is_ignored() {
    // consume_triggers resolves the target through XrdsIdIndex, which can
    // still hold a mapping for an entity that's already gone. That must
    // degrade to "skip this event", not panic.
    let mut app = xrds_test_app();

    let entity = import_node_with_triggers(
        &mut app,
        936,
        vec![XrdsTriggerBinding {
            trigger: XrdsTriggerKind::ZoneEnter,
            sequence: XrdsSequence {
                steps: vec![XrdsAction::Teleport { destination: [3.0, 3.0, 3.0] }],
            },
        }],
    );

    // Despawn the target, but leave the stale id→entity mapping in place —
    // exactly the state a mid-frame despawn leaves behind.
    app.world_mut().entity_mut(entity).despawn();

    app.world_mut()
        .write_message(xrds_components::XrZoneEnterEvent {
            zone_id: XrdsId(936),
            entity_id: XrdsId(936),
        });

    // The assertion is simply that pumping frames doesn't panic.
    pump(&mut app, 3);

    let agents = app
        .world_mut()
        .query::<&XrdsSequenceAgent>()
        .iter(app.world())
        .count();
    assert_eq!(
        agents, 0,
        "no agent should be spawned for a trigger whose target no longer exists"
    );
}

#[test]
fn target_despawned_mid_sequence_does_not_panic() {
    // The harder case: the sequence is already in flight (inside a Wait)
    // when its target dies, so a later action runs against a dead entity.
    let mut app = xrds_test_app();

    let entity = import_node_with_triggers(
        &mut app,
        937,
        vec![XrdsTriggerBinding {
            trigger: XrdsTriggerKind::ZoneEnter,
            sequence: XrdsSequence {
                steps: vec![
                    XrdsAction::Wait { seconds: 0.05 },
                    // Every remaining action type that touches the target,
                    // so a missing entity is exercised on each path.
                    XrdsAction::SetVisible(false),
                    XrdsAction::Teleport { destination: [1.0, 1.0, 1.0] },
                    XrdsAction::ModifyHealth {
                        target: XrdsActionTarget::SelfNode,
                        delta: XrdsActionValue::Fixed(-5.0),
                    },
                    XrdsAction::StopGltfAnimation,
                ],
            },
        }],
    );

    app.world_mut()
        .write_message(xrds_components::XrZoneEnterEvent {
            zone_id: XrdsId(937),
            entity_id: XrdsId(937),
        });

    // One frame: the agent spawns and enters its Wait.
    app.update();
    assert_eq!(
        app.world_mut()
            .query::<&XrdsSequenceAgent>()
            .iter(app.world())
            .count(),
        1,
        "sequence should be in flight before the despawn"
    );

    // Kill the target out from under the running sequence.
    app.world_mut().entity_mut(entity).despawn();

    // Drain the Wait and let every subsequent action run against the
    // now-dead target. Not panicking IS the assertion here.
    for _ in 0..12 {
        app.update();
        std::thread::sleep(Duration::from_millis(10));
    }

    // And the agent must still be reaped, or a despawned target would
    // leak its agent forever.
    assert_eq!(
        app.world_mut()
            .query::<&XrdsSequenceAgent>()
            .iter(app.world())
            .count(),
        0,
        "agent should still be reaped after its target was despawned mid-sequence"
    );
}

#[test]
fn grab_event_fires_its_authored_sequence() {
    // XrGrabEvent carries an XrdsId, so this exercises the
    // XrdsTriggerRef::Id resolution path.
    let mut app = xrds_test_app();

    let entity = import_node_with_triggers(
        &mut app,
        950,
        vec![XrdsTriggerBinding {
            trigger: XrdsTriggerKind::Grabbed,
            sequence: XrdsSequence {
                steps: vec![XrdsAction::Teleport { destination: [2.0, 0.0, 0.0] }],
            },
        }],
    );

    app.world_mut().write_message(xrds_components::XrGrabEvent {
        id: XrdsId(950),
        hand: xrds_components::XrGrabHand::Right,
    });

    pump(&mut app, 3);

    assert_eq!(
        app.world().get::<Transform>(entity).map(|t| t.translation),
        Some(Vec3::new(2.0, 0.0, 0.0)),
        "Grabbed should have fired its authored sequence"
    );
}

#[test]
fn button_press_fires_via_the_entity_ref_path() {
    // XrWorldButtonPressEvent carries a raw Entity rather than an XrdsId —
    // the other half of XrdsTriggerRef, and the reason that enum exists.
    let mut app = xrds_test_app();

    let entity = import_node_with_triggers(
        &mut app,
        951,
        vec![XrdsTriggerBinding {
            trigger: XrdsTriggerKind::ButtonPress,
            sequence: XrdsSequence {
                steps: vec![XrdsAction::Teleport { destination: [0.0, 7.0, 0.0] }],
            },
        }],
    );

    app.world_mut()
        .write_message(xrds_components::XrWorldButtonPressEvent {
            button_entity: entity,
            hand: xrds_components::XrGrabHand::Left,
        });

    pump(&mut app, 3);

    assert_eq!(
        app.world().get::<Transform>(entity).map(|t| t.translation),
        Some(Vec3::new(0.0, 7.0, 0.0)),
        "ButtonPress should resolve through XrdsTriggerRef::Entity and fire"
    );
}

/// An app-defined trigger source: exactly what third-party gameplay code
/// would write to fire a sequence from its own conditions (including a
/// threshold crossed by some continuous value).
#[derive(bevy::prelude::Message, Debug, Clone)]
struct ValveOpenedEvent {
    node_id: XrdsId,
}

impl crate::xrds_api::trigger_action::XrdsTriggerEvent for ValveOpenedEvent {
    fn target(&self) -> crate::xrds_api::trigger_action::XrdsTriggerRef {
        crate::xrds_api::trigger_action::XrdsTriggerRef::Id(self.node_id)
    }
    fn kind(&self) -> XrdsTriggerKind {
        XrdsTriggerKind::Custom("valve_opened".to_string())
    }
}

#[test]
fn app_defined_custom_trigger_fires_without_any_sdk_change() {
    // Proves the escape hatch works end to end: a message type defined
    // outside the SDK's vocabulary drives an authored sequence purely by
    // implementing the trait and registering the generic consumer.
    let mut app = xrds_test_app();
    app.add_message::<ValveOpenedEvent>();
    app.add_systems(
        Update,
        crate::xrds_api::trigger_action::consume_triggers::<ValveOpenedEvent>,
    );

    let entity = import_node_with_triggers(
        &mut app,
        952,
        vec![XrdsTriggerBinding {
            trigger: XrdsTriggerKind::Custom("valve_opened".to_string()),
            sequence: XrdsSequence {
                steps: vec![XrdsAction::Teleport { destination: [8.0, 0.0, 0.0] }],
            },
        }],
    );

    app.world_mut()
        .write_message(ValveOpenedEvent { node_id: XrdsId(952) });

    pump(&mut app, 3);

    assert_eq!(
        app.world().get::<Transform>(entity).map(|t| t.translation),
        Some(Vec3::new(8.0, 0.0, 0.0)),
        "an app-defined Custom trigger should fire its authored sequence"
    );
}

#[test]
fn custom_trigger_with_a_different_name_does_not_fire() {
    // Custom is matched by name, so a non-matching name must be inert —
    // the flip side of the string-matching trade-off.
    let mut app = xrds_test_app();
    app.add_message::<ValveOpenedEvent>();
    app.add_systems(
        Update,
        crate::xrds_api::trigger_action::consume_triggers::<ValveOpenedEvent>,
    );

    let entity = import_node_with_triggers(
        &mut app,
        953,
        vec![XrdsTriggerBinding {
            trigger: XrdsTriggerKind::Custom("some_other_name".to_string()),
            sequence: XrdsSequence {
                steps: vec![XrdsAction::Teleport { destination: [8.0, 0.0, 0.0] }],
            },
        }],
    );

    app.world_mut()
        .write_message(ValveOpenedEvent { node_id: XrdsId(953) });

    pump(&mut app, 3);

    assert_eq!(
        app.world().get::<Transform>(entity).map(|t| t.translation),
        Some(Vec3::ZERO),
        "a Custom binding with a different name must not fire"
    );
}

/// Builds a node with an `AnimationPlayer` on a child (matching real glTF
/// structure — `animation_player_entities_for_root_in_world` walks
/// `Children` and does not consider the root itself), playing a short clip
/// once, and registers it in XRDS's animation-state cache as if
/// `play_gltf_animation` had started it.
fn spawn_node_with_finishing_animation(
    app: &mut App,
    node_id: u64,
    bindings: Vec<XrdsTriggerBinding>,
    repeat: XrdsAnimationRepeatMode,
) -> Entity {
    let root = import_node_with_triggers(app, node_id, bindings);

    let mut clip = AnimationClip::default();
    clip.set_duration(0.05);
    let clip_handle = app
        .world_mut()
        .resource_mut::<Assets<AnimationClip>>()
        .add(clip);
    let (graph, node_index) = AnimationGraph::from_clip(clip_handle);
    let graph_handle = app
        .world_mut()
        .resource_mut::<Assets<AnimationGraph>>()
        .add(graph);

    let mut player = AnimationPlayer::default();
    {
        let active = player.play(node_index);
        if matches!(repeat, XrdsAnimationRepeatMode::Loop) {
            active.repeat();
        }
    }
    let child = app
        .world_mut()
        .spawn((
            player,
            bevy::animation::graph::AnimationGraphHandle(graph_handle),
        ))
        .id();
    app.world_mut().entity_mut(root).add_child(child);

    // Stand in for what apply_gltf_animation_request_for_entity_in_world
    // records when real playback starts.
    app.world_mut()
        .resource_mut::<ActiveGltfAnimationStates>()
        .states
        .insert(
            root,
            XrdsGltfAnimationState {
                animation: XrdsGltfAnimationInfo {
                    index: 0,
                    name: Some("test-clip".to_string()),
                    duration_secs: Some(0.05),
                },
                playing: true,
                paused: false,
                repeat,
                speed: 1.0,
            },
        );

    root
}

fn drain_frames(app: &mut App, frames: usize) {
    for _ in 0..frames {
        app.update();
        std::thread::sleep(Duration::from_millis(8));
    }
}

#[test]
fn completed_animation_clears_the_playing_flag() {
    // Regression test for a real pre-existing bug: every other writer of
    // ActiveGltfAnimationStates was an imperative API call, so `playing`
    // stayed true forever after a Once clip reached its end, and
    // gltf_animation_state() reported a finished animation as still playing.
    let mut app = xrds_test_app();
    let root = spawn_node_with_finishing_animation(
        &mut app,
        940,
        Vec::new(),
        XrdsAnimationRepeatMode::Once,
    );

    assert_eq!(
        app.world()
            .resource::<ActiveGltfAnimationStates>()
            .states
            .get(&root)
            .map(|s| s.playing),
        Some(true),
        "should start out marked as playing"
    );

    drain_frames(&mut app, 15);

    assert_eq!(
        app.world()
            .resource::<ActiveGltfAnimationStates>()
            .states
            .get(&root)
            .map(|s| s.playing),
        Some(false),
        "playing must be cleared once the clip actually finishes"
    );
}

#[test]
fn looping_animation_never_reports_completion() {
    // RepeatAnimation::Forever makes ActiveAnimation::is_finished() always
    // false, so a Loop clip must never flip `playing` or fire the trigger —
    // this is the case that would otherwise hang a wait-for-completion.
    let mut app = xrds_test_app();
    let root = spawn_node_with_finishing_animation(
        &mut app,
        941,
        Vec::new(),
        XrdsAnimationRepeatMode::Loop,
    );

    drain_frames(&mut app, 15);

    assert_eq!(
        app.world()
            .resource::<ActiveGltfAnimationStates>()
            .states
            .get(&root)
            .map(|s| s.playing),
        Some(true),
        "a looping clip has no completion — it must stay marked as playing"
    );
}

#[test]
fn animation_complete_fires_as_an_authored_trigger() {
    // The user-facing payoff: "play an animation, then do something else"
    // expressed as a second binding rather than a blocking step.
    let mut app = xrds_test_app();
    let root = spawn_node_with_finishing_animation(
        &mut app,
        942,
        vec![XrdsTriggerBinding {
            trigger: XrdsTriggerKind::AnimationComplete,
            sequence: XrdsSequence {
                steps: vec![XrdsAction::Teleport { destination: [4.0, 0.0, 4.0] }],
            },
        }],
        XrdsAnimationRepeatMode::Once,
    );

    assert_eq!(
        app.world().get::<Transform>(root).map(|t| t.translation),
        Some(Vec3::ZERO),
        "should not have moved before the animation completes"
    );

    drain_frames(&mut app, 20);

    assert_eq!(
        app.world().get::<Transform>(root).map(|t| t.translation),
        Some(Vec3::new(4.0, 0.0, 4.0)),
        "AnimationComplete should have fired its authored sequence"
    );
}

#[test]
fn fire_custom_event_emits_message_with_target_and_source() {
    let mut app = xrds_test_app();

    let target = import_node_with_triggers(
        &mut app,
        935,
        vec![XrdsTriggerBinding {
            trigger: XrdsTriggerKind::ZoneEnter,
            sequence: XrdsSequence {
                steps: vec![XrdsAction::FireCustomEvent { name: "door_opened".to_string() }],
            },
        }],
    );

    let source = app.world_mut().spawn_empty().id();
    app.world_mut()
        .resource_mut::<XrdsIdIndex>()
        .register(XrdsId(9350), source);

    app.world_mut()
        .write_message(xrds_components::XrZoneEnterEvent {
            zone_id: XrdsId(935),
            entity_id: XrdsId(9350),
        });

    pump(&mut app, 3);

    let messages = app.world().resource::<Messages<XrdsCustomTriggerEvent>>();
    let mut cursor = messages.get_cursor();
    let emitted: Vec<_> = cursor.read(messages).collect();

    assert_eq!(emitted.len(), 1, "expected exactly one custom trigger event");
    assert_eq!(emitted[0].name, "door_opened");
    assert_eq!(emitted[0].target, target);
    assert_eq!(emitted[0].source, Some(source));
}
