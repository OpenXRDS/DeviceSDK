//! Behavior tests for trigger-action sequencing (Phases 3-4). Unlike the
//! Phase 2 round-trip test, these exercise the *live* path: a trigger
//! message is written, and the authored action's effect on the world is
//! asserted.
use super::*;
use crate::xrds_api::trigger_action::{
    XrdsCustomTriggerEvent, XrdsHealth, XrdsSequenceAgent, XrdsTrackAgent,
    XrdsTrackAssetLocks, XrdsTransformTween, XrdsTriggerValue,
};
use xrds_scene_graph::{XrdsNamedTrack, XrdsTrack, XrdsTrackAsset, XrdsTrackKey};

/// Test-local stand-in for what used to be a binding with an inline
/// sequence.
///
/// A binding now names a Track, so every test that used to say "this trigger
/// runs these actions" has to author a Track too. Rather than repeat that
/// two-part construction ~40 times, `Bound` describes the intent and
/// [`import_bound`] generates the binding *and* its Track.
///
/// `steps` all land at t=0 on a single `SelfNode` asset row, which reproduces
/// the old inline-sequence semantics for the common single-action case. A test
/// that genuinely needs events at *different* times builds its Track
/// directly — the old file leaned on `Wait` for that, which no longer exists.
#[derive(Default)]
struct Bound {
    trigger: XrdsTriggerKind,
    /// Actions at t=0, concurrently.
    steps: Vec<XrdsAction>,
    disabled: bool,
    hand: Option<xrds_components::XrGrabHand>,
}

impl Bound {
    /// Splits into the binding and the Track it names.
    fn split(self, track_name: String) -> (XrdsTriggerBinding, XrdsNamedTrack) {
        let keys: Vec<XrdsTrackKey> = self
            .steps
            .into_iter()
            .map(|action| XrdsTrackKey { at_secs: 0.0, action })
            .collect();

        let binding = XrdsTriggerBinding {
            trigger: self.trigger,
            track: Some(track_name.clone()),
            disabled: self.disabled,
            hand: self.hand,
        };
        let track = XrdsNamedTrack {
            name: track_name,
            track: XrdsTrack {
                assets: vec![XrdsTrackAsset {
                    target: XrdsActionTarget::SelfNode,
                    keys,
                }],
                ..XrdsTrack::default()
            },
        };
        (binding, track)
    }
}

fn scene_node(node_id: u64, name: &str) -> XrdsSceneNode {
    XrdsSceneNode {
        id: XrdsSceneNodeId(node_id),
        parent_id: None,
        name: format!("{name}{node_id}"),
        enabled: true,
        visible: true,
        transform: XrdsSceneTransform::default(),
        payload: XrdsSceneNodePayload::Empty,
        grabbable: false,
        editor: XrdsEditorMetadata::default(),
        triggers: Vec::new(),
        watchers: Vec::new(),
    }
}

fn import_document(app: &mut App, document: &XrdsSceneDocument, node_id: u64) -> Entity {
    {
        let mut xrds = XrdsAPI::attach(app);
        xrds.import_scene_document(document).expect("import should succeed");
    }
    app.world()
        .resource::<XrdsIdIndex>()
        .entity_of(XrdsId(node_id))
        .expect("imported node should be indexed")
}

/// Imports one node whose bindings each get an auto-generated Track.
fn import_bound(app: &mut App, node_id: u64, bound: Vec<Bound>) -> Entity {
    let mut bindings = Vec::new();
    let mut tracks = Vec::new();
    for (i, b) in bound.into_iter().enumerate() {
        let (binding, track) = b.split(format!("track_{node_id}_{i}"));
        bindings.push(binding);
        tracks.push(track);
    }
    let document = XrdsSceneDocument {
        nodes: vec![XrdsSceneNode { triggers: bindings, ..scene_node(node_id, "TriggerNode") }],
        tracks,
        ..Default::default()
    };
    import_document(app, &document, node_id)
}

/// Imports several nodes, each with its own auto-generated Tracks, in a
/// single document.
///
/// Tracks live in a document-level registry that import replaces wholesale,
/// so importing node A and then node B would drop A's Tracks. Multi-node
/// tests must build one document.
fn import_many_bound(app: &mut App, entries: Vec<(u64, Vec<Bound>)>) -> Vec<Entity> {
    let mut nodes = Vec::new();
    let mut tracks = Vec::new();
    let ids: Vec<u64> = entries.iter().map(|(id, _)| *id).collect();
    for (node_id, bound) in entries {
        let mut bindings = Vec::new();
        for (i, b) in bound.into_iter().enumerate() {
            let (binding, track) = b.split(format!("track_{node_id}_{i}"));
            bindings.push(binding);
            tracks.push(track);
        }
        nodes.push(XrdsSceneNode { triggers: bindings, ..scene_node(node_id, "Node") });
    }
    let document = XrdsSceneDocument { nodes, tracks, ..Default::default() };
    {
        let mut xrds = XrdsAPI::attach(app);
        xrds.import_scene_document(&document).expect("import should succeed");
    }
    let index = app.world().resource::<XrdsIdIndex>();
    ids.iter()
        .map(|id| index.entity_of(XrdsId(*id)).expect("node should be indexed"))
        .collect()
}

/// Imports one node whose binding fires a Track with explicitly-timed events.
/// For tests about the Track's own clock, where `Bound`'s everything-at-t=0
/// shape is not enough.
fn import_timed_track(
    app: &mut App,
    node_id: u64,
    trigger: XrdsTriggerKind,
    keys: Vec<(f32, XrdsAction)>,
) -> Entity {
    let name = format!("timed_{node_id}");
    let document = XrdsSceneDocument {
        nodes: vec![XrdsSceneNode {
            triggers: vec![XrdsTriggerBinding {
                trigger,
                track: Some(name.clone()),
                disabled: false,
                hand: None,
            }],
            ..scene_node(node_id, "TimedNode")
        }],
        tracks: vec![XrdsNamedTrack {
            name,
            track: XrdsTrack {
                assets: vec![XrdsTrackAsset {
                    target: XrdsActionTarget::SelfNode,
                    keys: keys
                        .into_iter()
                        .map(|(at_secs, action)| XrdsTrackKey { at_secs, action })
                        .collect(),
                }],
                ..XrdsTrack::default()
            },
        }],
        ..Default::default()
    };
    import_document(app, &document, node_id)
}

/// Imports one node carrying `watchers` (and any bindings a watcher's
/// `Custom` firing should drive).
fn import_node_with_watchers(
    app: &mut App,
    node_id: u64,
    watchers: Vec<XrdsThresholdWatcher>,
    bound: Vec<Bound>,
) -> Entity {
    let mut bindings = Vec::new();
    let mut tracks = Vec::new();
    for (i, b) in bound.into_iter().enumerate() {
        let (binding, track) = b.split(format!("track_{node_id}_{i}"));
        bindings.push(binding);
        tracks.push(track);
    }
    let document = XrdsSceneDocument {
        nodes: vec![XrdsSceneNode {
            triggers: bindings,
            watchers,
            ..scene_node(node_id, "WatcherNode")
        }],
        tracks,
        ..Default::default()
    };
    import_document(app, &document, node_id)
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

    let entity = import_bound(
        &mut app,
        930,
        vec![Bound {
            trigger: XrdsTriggerKind::ZoneEnter,
            steps: vec![XrdsAction::SetTransform {
                            position: Some([5.0, 6.0, 7.0]),
                            rotation: None,
                            scale: None,
                            duration_secs: 0.0,
                            ease: XrdsEaseCurve::Linear,
                        }],
            disabled: false,
            hand: None,
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

    let entity = import_bound(
        &mut app,
        931,
        vec![Bound {
            trigger: XrdsTriggerKind::ZoneExit,
            steps: vec![XrdsAction::SetTransform {
                            position: Some([9.0, 9.0, 9.0]),
                            rotation: None,
                            scale: None,
                            duration_secs: 0.0,
                            ease: XrdsEaseCurve::Linear,
                        }],
            disabled: false,
            hand: None,
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

    let target = import_bound(
        &mut app,
        932,
        vec![Bound {
            trigger: XrdsTriggerKind::ZoneEnter,
            steps: vec![XrdsAction::ModifyHealth {
                    delta: XrdsActionValue::FromTriggerSource,
                }],
            disabled: false,
            hand: None,
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
fn each_firing_resolves_from_trigger_source_against_its_own_source() {
    // What this originally asserted was that `FromTriggerSource` reads the
    // entity that actually fired, per firing — verified by firing the *same*
    // binding twice from two sources and watching two concurrent agents.
    //
    // Two firings of one Track no longer run concurrently: a re-fire of a
    // Track that is already running restarts it (plan doc §4), so the old
    // shape can't express this. Two targets, each with its own Track, tests
    // the same property without depending on concurrency that is now
    // deliberately impossible.
    let mut app = xrds_test_app();

    let bound = || Bound {
        trigger: XrdsTriggerKind::ZoneEnter,
        steps: vec![XrdsAction::ModifyHealth {
            delta: XrdsActionValue::FromTriggerSource,
        }],
        ..Default::default()
    };
    let targets = import_many_bound(&mut app, vec![(933, vec![bound()]), (934, vec![bound()])]);

    for entity in &targets {
        app.world_mut().entity_mut(*entity).insert(XrdsHealth(100.0));
    }

    // Two sources carrying different values.
    for (i, (source_id, value)) in [(9331_u64, -10.0_f32), (9332, -25.0)].into_iter().enumerate() {
        let source = app.world_mut().spawn(XrdsTriggerValue(value)).id();
        app.world_mut().resource_mut::<XrdsIdIndex>().register(XrdsId(source_id), source);
        app.world_mut().write_message(xrds_components::XrZoneEnterEvent {
            zone_id: XrdsId(if i == 0 { 933 } else { 934 }),
            entity_id: XrdsId(source_id),
        });
    }

    pump(&mut app, 4);

    // Each target should have taken its own source's delta, not the other's
    // and not one applied twice.
    assert_eq!(
        app.world().get::<XrdsHealth>(targets[0]).map(|h| h.0),
        Some(90.0),
        "node 933 should have read -10 from its own source"
    );
    assert_eq!(
        app.world().get::<XrdsHealth>(targets[1]).map(|h| h.0),
        Some(75.0),
        "node 934 should have read -25 from its own source"
    );
}

#[test]
fn finished_sequence_agents_are_despawned() {
    let mut app = xrds_test_app();

    import_bound(
        &mut app,
        934,
        vec![Bound {
            trigger: XrdsTriggerKind::ZoneEnter,
            steps: vec![XrdsAction::SetVisible(false)],
            disabled: false,
            hand: None,
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

    let entity = import_bound(
        &mut app,
        936,
        vec![Bound {
            trigger: XrdsTriggerKind::ZoneEnter,
            steps: vec![XrdsAction::SetTransform {
                            position: Some([3.0, 3.0, 3.0]),
                            rotation: None,
                            scale: None,
                            duration_secs: 0.0,
                            ease: XrdsEaseCurve::Linear,
                        }],
            disabled: false,
            hand: None,
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

    let entity = import_bound(
        &mut app,
        937,
        vec![Bound {
            trigger: XrdsTriggerKind::ZoneEnter,
            steps: vec![
                    // Keeps the agent in flight so the despawn lands mid-run.
                    XrdsAction::SetTransform {
                        position: None,
                        rotation: None,
                        scale: Some([1.0, 1.0, 1.0]),
                        duration_secs: 0.05,
                        ease: XrdsEaseCurve::Linear,
                    },
                    // Every remaining action type that touches the target,
                    // so a missing entity is exercised on each path.
                    XrdsAction::SetVisible(false),
                    XrdsAction::SetTransform {
                            position: Some([1.0, 1.0, 1.0]),
                            rotation: None,
                            scale: None,
                            duration_secs: 0.0,
                            ease: XrdsEaseCurve::Linear,
                        },
                    XrdsAction::ModifyHealth {
                        delta: XrdsActionValue::Fixed(-5.0),
                    },
                    XrdsAction::StopGltfAnimation,
                ],
            disabled: false,
            hand: None,
        }],
    );

    app.world_mut()
        .write_message(xrds_components::XrZoneEnterEvent {
            zone_id: XrdsId(937),
            entity_id: XrdsId(937),
        });

    // Two frames, not one: `consume_triggers` queues the Track spawn as a
    // command, so the Track agent exists on frame 2 and `advance_tracks` fires
    // its t=0 keys then. The interpolating key's own one-step agent is what
    // stays in flight.
    app.update();
    app.update();
    assert!(
        app.world_mut()
            .query::<&XrdsSequenceAgent>()
            .iter(app.world())
            .count()
            > 0,
        "an action agent should be in flight before the despawn"
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

    let entity = import_bound(
        &mut app,
        950,
        vec![Bound {
            trigger: XrdsTriggerKind::Grabbed,
            steps: vec![XrdsAction::SetTransform {
                            position: Some([2.0, 0.0, 0.0]),
                            rotation: None,
                            scale: None,
                            duration_secs: 0.0,
                            ease: XrdsEaseCurve::Linear,
                        }],
            disabled: false,
            hand: None,
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

    let entity = import_bound(
        &mut app,
        951,
        vec![Bound {
            trigger: XrdsTriggerKind::ButtonPress,
            steps: vec![XrdsAction::SetTransform {
                            position: Some([0.0, 7.0, 0.0]),
                            rotation: None,
                            scale: None,
                            duration_secs: 0.0,
                            ease: XrdsEaseCurve::Linear,
                        }],
            disabled: false,
            hand: None,
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

    let entity = import_bound(
        &mut app,
        952,
        vec![Bound {
            trigger: XrdsTriggerKind::Custom("valve_opened".to_string()),
            steps: vec![XrdsAction::SetTransform {
                            position: Some([8.0, 0.0, 0.0]),
                            rotation: None,
                            scale: None,
                            duration_secs: 0.0,
                            ease: XrdsEaseCurve::Linear,
                        }],
            disabled: false,
            hand: None,
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

    let entity = import_bound(
        &mut app,
        953,
        vec![Bound {
            trigger: XrdsTriggerKind::Custom("some_other_name".to_string()),
            steps: vec![XrdsAction::SetTransform {
                            position: Some([8.0, 0.0, 0.0]),
                            rotation: None,
                            scale: None,
                            duration_secs: 0.0,
                            ease: XrdsEaseCurve::Linear,
                        }],
            disabled: false,
            hand: None,
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
    bindings: Vec<Bound>,
    repeat: XrdsAnimationRepeatMode,
) -> Entity {
    let root = import_bound(app, node_id, bindings);

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
        vec![Bound {
            trigger: XrdsTriggerKind::AnimationComplete,
            steps: vec![XrdsAction::SetTransform {
                            position: Some([4.0, 0.0, 4.0]),
                            rotation: None,
                            scale: None,
                            duration_secs: 0.0,
                            ease: XrdsEaseCurve::Linear,
                        }],
            disabled: false,
            hand: None,
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
fn fire_trigger_runs_bindings_without_a_real_event() {
    // The editor "preview this sequence" path, and how app tests should
    // stage a sequence rather than faking a zone collision.
    let mut app = xrds_test_app();

    let entity = import_bound(
        &mut app,
        960,
        vec![Bound {
            trigger: XrdsTriggerKind::ButtonPress,
            steps: vec![XrdsAction::SetTransform {
                            position: Some([3.0, 0.0, 0.0]),
                            rotation: None,
                            scale: None,
                            duration_secs: 0.0,
                            ease: XrdsEaseCurve::Linear,
                        }],
            disabled: false,
            hand: None,
        }],
    );

    let started = crate::xrds_api::trigger_action::fire_trigger_in_world(
        app.world_mut(),
        XrdsId(960),
        &XrdsTriggerKind::ButtonPress,
        None,
    );
    assert_eq!(started, 1, "one binding matched, so one sequence should start");

    pump(&mut app, 3);
    assert_eq!(
        app.world().get::<Transform>(entity).map(|t| t.translation),
        Some(Vec3::new(3.0, 0.0, 0.0)),
    );
}

#[test]
fn fire_trigger_reports_zero_when_nothing_is_bound() {
    let mut app = xrds_test_app();
    import_bound(
        &mut app,
        961,
        vec![Bound {
            trigger: XrdsTriggerKind::ZoneEnter,
            steps: vec![XrdsAction::SetVisible(false)],
            disabled: false,
            hand: None,
        }],
    );

    // A kind with no binding on this node.
    let started = crate::xrds_api::trigger_action::fire_trigger_in_world(
        app.world_mut(),
        XrdsId(961),
        &XrdsTriggerKind::Grabbed,
        None,
    );
    assert_eq!(started, 0, "caller must be able to tell 'nothing bound' from 'ran'");
}

#[test]
fn stop_sequences_on_cancels_in_flight_work() {
    let mut app = xrds_test_app();

    let entity = import_bound(
        &mut app,
        962,
        vec![Bound {
            trigger: XrdsTriggerKind::ZoneEnter,
            steps: vec![
                    // Long enough that the cancel below lands mid-flight.
                    XrdsAction::SetTransform {
                        position: None,
                        rotation: None,
                        scale: Some([2.0, 2.0, 2.0]),
                        duration_secs: 5.0,
                        ease: XrdsEaseCurve::Linear,
                    },
                ],
            disabled: false,
            hand: None,
        }],
    );

    app.world_mut()
        .write_message(xrds_components::XrZoneEnterEvent {
            zone_id: XrdsId(962),
            entity_id: XrdsId(962),
        });
    app.update();

    let stopped =
        crate::xrds_api::trigger_action::stop_sequences_on_in_world(app.world_mut(), XrdsId(962));
    assert_eq!(stopped, 1, "the in-flight sequence should have been cancelled");

    pump(&mut app, 5);
    assert_eq!(
        app.world().get::<Transform>(entity).map(|t| t.translation),
        Some(Vec3::ZERO),
        "the step after the Wait must not run once cancelled"
    );
    assert_eq!(
        app.world_mut()
            .query::<&XrdsSequenceAgent>()
            .iter(app.world())
            .count(),
        0,
        "cancelling must not leave the agent behind"
    );
}

#[test]
fn a_tracks_own_clock_respects_paused_virtual_time() {
    // `advance_tracks` reads `Res<Time>`, which is `Time<Virtual>` in Bevy, so
    // pausing the app SHOULD freeze a Track mid-play. Previously this was
    // asserted about `Wait`; with the Track model the same property matters
    // more, because it now governs the whole Track's schedule rather than one
    // action. Exactly the kind of thing that is silently wrong until someone
    // pauses during a cutscene.
    let mut app = xrds_test_app();

    let entity = import_timed_track(
        &mut app,
        963,
        XrdsTriggerKind::ZoneEnter,
        vec![(0.08, XrdsAction::SetTransform {
                            position: Some([4.0, 0.0, 0.0]),
                            rotation: None,
                            scale: None,
                            duration_secs: 0.0,
                            ease: XrdsEaseCurve::Linear,
                        })],
    );

    app.world_mut().write_message(xrds_components::XrZoneEnterEvent {
        zone_id: XrdsId(963),
        entity_id: XrdsId(963),
    });
    app.update();

    app.world_mut().resource_mut::<Time<Virtual>>().pause();

    // Well past 0.08s in wall-clock terms.
    for _ in 0..20 {
        app.update();
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(
        app.world().get::<Transform>(entity).map(|t| t.translation),
        Some(Vec3::ZERO),
        "a Track must not advance while virtual time is paused"
    );

    app.world_mut().resource_mut::<Time<Virtual>>().unpause();
    for _ in 0..20 {
        app.update();
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(
        app.world().get::<Transform>(entity).map(|t| t.translation),
        Some(Vec3::new(4.0, 0.0, 0.0)),
        "the event should fire once virtual time resumes"
    );
}

#[test]
fn disabled_binding_does_not_fire() {
    let mut app = xrds_test_app();

    let entity = import_bound(
        &mut app,
        970,
        vec![Bound {
            trigger: XrdsTriggerKind::ZoneEnter,
            steps: vec![XrdsAction::SetTransform {
                            position: Some([6.0, 0.0, 0.0]),
                            rotation: None,
                            scale: None,
                            duration_secs: 0.0,
                            ease: XrdsEaseCurve::Linear,
                        }],
            disabled: true,
            hand: None,
        }],
    );

    app.world_mut()
        .write_message(xrds_components::XrZoneEnterEvent {
            zone_id: XrdsId(970),
            entity_id: XrdsId(970),
        });
    pump(&mut app, 3);

    assert_eq!(
        app.world().get::<Transform>(entity).map(|t| t.translation),
        Some(Vec3::ZERO),
        "a parked binding must stay inert"
    );
}

#[test]
fn disabling_one_binding_leaves_its_siblings_running() {
    // The actual point of the flag: silence one rule without touching the
    // others on the same node and trigger kind.
    let mut app = xrds_test_app();

    let target = import_bound(
        &mut app,
        971,
        vec![
            Bound {
                trigger: XrdsTriggerKind::ZoneEnter,
                steps: vec![XrdsAction::ModifyHealth {
                        delta: XrdsActionValue::Fixed(-1.0),
                    }],
                disabled: true,
                hand: None,
            },
            Bound {
                trigger: XrdsTriggerKind::ZoneEnter,
                steps: vec![XrdsAction::ModifyHealth {
                        delta: XrdsActionValue::Fixed(-10.0),
                    }],
                disabled: false,
                hand: None,
            },
        ],
    );
    app.world_mut().entity_mut(target).insert(XrdsHealth(100.0));

    app.world_mut()
        .write_message(xrds_components::XrZoneEnterEvent {
            zone_id: XrdsId(971),
            entity_id: XrdsId(971),
        });
    pump(&mut app, 3);

    assert_eq!(
        app.world().get::<XrdsHealth>(target).map(|h| h.0),
        Some(90.0),
        "only the enabled binding should have applied (-10, not -11)"
    );
}

#[test]
fn fire_trigger_skips_disabled_bindings() {
    // An editor preview that ran parked rules would misrepresent runtime.
    let mut app = xrds_test_app();

    import_bound(
        &mut app,
        972,
        vec![Bound {
            trigger: XrdsTriggerKind::ButtonPress,
            steps: vec![XrdsAction::SetVisible(false)],
            disabled: true,
            hand: None,
        }],
    );

    let started = crate::xrds_api::trigger_action::fire_trigger_in_world(
        app.world_mut(),
        XrdsId(972),
        &XrdsTriggerKind::ButtonPress,
        None,
    );
    assert_eq!(started, 0, "preview must skip disabled bindings too");
}

#[test]
fn hand_filter_matches_the_specified_hand_only() {
    let mut app = xrds_test_app();

    let entity = import_bound(
        &mut app,
        980,
        vec![Bound {
            trigger: XrdsTriggerKind::Grabbed,
            steps: vec![XrdsAction::SetTransform {
                            position: Some([5.0, 0.0, 0.0]),
                            rotation: None,
                            scale: None,
                            duration_secs: 0.0,
                            ease: XrdsEaseCurve::Linear,
                        }],
            disabled: false,
            hand: Some(xrds_components::XrGrabHand::Left),
        }],
    );

    // Wrong hand: must not fire.
    app.world_mut().write_message(xrds_components::XrGrabEvent {
        id: XrdsId(980),
        hand: xrds_components::XrGrabHand::Right,
    });
    pump(&mut app, 3);
    assert_eq!(
        app.world().get::<Transform>(entity).map(|t| t.translation),
        Some(Vec3::ZERO),
        "a Left-only binding must not fire for a Right-hand grab"
    );

    // Correct hand: must fire.
    app.world_mut().write_message(xrds_components::XrGrabEvent {
        id: XrdsId(980),
        hand: xrds_components::XrGrabHand::Left,
    });
    pump(&mut app, 3);
    assert_eq!(
        app.world().get::<Transform>(entity).map(|t| t.translation),
        Some(Vec3::new(5.0, 0.0, 0.0)),
        "a Left-only binding must fire for a Left-hand grab"
    );
}

#[test]
fn no_hand_filter_matches_either_hand() {
    // The backward-compatible default: existing bindings with hand: None
    // keep working exactly as before this feature existed.
    let mut app = xrds_test_app();

    let entity = import_bound(
        &mut app,
        981,
        vec![Bound {
            trigger: XrdsTriggerKind::Grabbed,
            steps: vec![XrdsAction::SetTransform {
                            position: Some([6.0, 0.0, 0.0]),
                            rotation: None,
                            scale: None,
                            duration_secs: 0.0,
                            ease: XrdsEaseCurve::Linear,
                        }],
            disabled: false,
            hand: None,
        }],
    );

    app.world_mut().write_message(xrds_components::XrGrabEvent {
        id: XrdsId(981),
        hand: xrds_components::XrGrabHand::Right,
    });
    pump(&mut app, 3);

    assert_eq!(
        app.world().get::<Transform>(entity).map(|t| t.translation),
        Some(Vec3::new(6.0, 0.0, 0.0)),
        "no hand filter should match any hand"
    );
}

#[test]
fn fire_trigger_honors_the_hand_argument() {
    let mut app = xrds_test_app();

    import_bound(
        &mut app,
        982,
        vec![Bound {
            trigger: XrdsTriggerKind::ButtonPress,
            steps: vec![XrdsAction::SetVisible(false)],
            disabled: false,
            hand: Some(xrds_components::XrGrabHand::Right),
        }],
    );

    let wrong = crate::xrds_api::trigger_action::fire_trigger_in_world(
        app.world_mut(),
        XrdsId(982),
        &XrdsTriggerKind::ButtonPress,
        Some(xrds_components::XrGrabHand::Left),
    );
    assert_eq!(wrong, 0, "preview must respect the hand filter too");

    let right = crate::xrds_api::trigger_action::fire_trigger_in_world(
        app.world_mut(),
        XrdsId(982),
        &XrdsTriggerKind::ButtonPress,
        Some(xrds_components::XrGrabHand::Right),
    );
    assert_eq!(right, 1, "the matching hand should start the sequence");
}

#[test]
fn height_watcher_fires_custom_trigger_on_crossing_above() {
    let mut app = xrds_test_app();

    let entity = import_node_with_watchers(
        &mut app,
        990,
        vec![XrdsThresholdWatcher {
            observable: XrdsObservable::Height,
            crossing: XrdsCrossing::Above,
            value: 5.0,
            hysteresis: 0.0,
            fires: "risen".to_string(),
            disabled: false,
        }],
        vec![Bound {
            trigger: XrdsTriggerKind::Custom("risen".to_string()),
            steps: vec![XrdsAction::SetTransform {
                            position: Some([7.0, 7.0, 7.0]),
                            rotation: None,
                            scale: None,
                            duration_secs: 0.0,
                            ease: XrdsEaseCurve::Linear,
                        }],
            disabled: false,
            hand: None,
        }],
    );

    // The first evaluation only primes the watcher's initial state; it
    // must not fire even though the node has not moved.
    app.update();
    assert_eq!(
        app.world().get::<Transform>(entity).map(|t| t.translation),
        Some(Vec3::ZERO),
        "priming must not fire"
    );

    // Cross upward through the threshold.
    app.world_mut().entity_mut(entity).insert(Transform::from_xyz(0.0, 6.0, 0.0));
    pump(&mut app, 3);

    assert_eq!(
        app.world().get::<Transform>(entity).map(|t| t.translation),
        Some(Vec3::new(7.0, 7.0, 7.0)),
        "crossing above 5.0 should have fired the Custom(risen) binding"
    );
}

#[test]
fn crossing_above_only_does_not_fire_on_the_way_back_down() {
    let mut app = xrds_test_app();

    let entity = import_node_with_watchers(
        &mut app,
        991,
        vec![XrdsThresholdWatcher {
            observable: XrdsObservable::Height,
            crossing: XrdsCrossing::Above,
            value: 5.0,
            hysteresis: 0.0,
            fires: "risen".to_string(),
            disabled: false,
        }],
        vec![Bound {
            trigger: XrdsTriggerKind::Custom("risen".to_string()),
            steps: vec![XrdsAction::ModifyHealth {
                    delta: XrdsActionValue::Fixed(-1.0),
                }],
            disabled: false,
            hand: None,
        }],
    );
    app.world_mut().entity_mut(entity).insert(XrdsHealth(100.0));
    app.update(); // prime

    app.world_mut().entity_mut(entity).insert(Transform::from_xyz(0.0, 6.0, 0.0));
    pump(&mut app, 3); // crosses above -> fires once, health 99

    app.world_mut().entity_mut(entity).insert(Transform::from_xyz(0.0, 0.0, 0.0));
    pump(&mut app, 3); // crosses back below -> Above-only watcher stays silent

    assert_eq!(
        app.world().get::<XrdsHealth>(entity).map(|h| h.0),
        Some(99.0),
        "an Above-only watcher must not fire again on the downward crossing"
    );
}

#[test]
fn hysteresis_suppresses_chatter_at_the_boundary() {
    let mut app = xrds_test_app();

    let entity = import_node_with_watchers(
        &mut app,
        992,
        vec![XrdsThresholdWatcher {
            observable: XrdsObservable::Height,
            crossing: XrdsCrossing::Either,
            value: 5.0,
            hysteresis: 1.0,
            fires: "wobble".to_string(),
            disabled: false,
        }],
        vec![Bound {
            trigger: XrdsTriggerKind::Custom("wobble".to_string()),
            steps: vec![XrdsAction::ModifyHealth {
                    delta: XrdsActionValue::Fixed(-1.0),
                }],
            disabled: false,
            hand: None,
        }],
    );
    app.world_mut().entity_mut(entity).insert(XrdsHealth(100.0));
    app.update(); // primes below (0.0 < 5.0)

    // Wobble around the raw threshold, but stay inside the [4.0, 6.0]
    // hysteresis band the whole time -- without hysteresis this would fire
    // on every single one of these.
    for y in [5.2, 4.9, 5.3, 4.8, 5.1] {
        app.world_mut().entity_mut(entity).insert(Transform::from_xyz(0.0, y, 0.0));
        app.update();
    }

    assert_eq!(
        app.world().get::<XrdsHealth>(entity).map(|h| h.0),
        Some(100.0),
        "wobbling inside the hysteresis band must not fire at all"
    );

    // Now actually clear the band on the high side.
    app.world_mut().entity_mut(entity).insert(Transform::from_xyz(0.0, 6.5, 0.0));
    pump(&mut app, 3);

    assert_eq!(
        app.world().get::<XrdsHealth>(entity).map(|h| h.0),
        Some(99.0),
        "clearing the hysteresis band should fire exactly once"
    );
}

#[test]
fn either_crossing_re_arms_and_fires_both_directions() {
    let mut app = xrds_test_app();

    let entity = import_node_with_watchers(
        &mut app,
        993,
        vec![XrdsThresholdWatcher {
            observable: XrdsObservable::Height,
            crossing: XrdsCrossing::Either,
            value: 5.0,
            hysteresis: 0.0,
            fires: "crossed".to_string(),
            disabled: false,
        }],
        vec![Bound {
            trigger: XrdsTriggerKind::Custom("crossed".to_string()),
            steps: vec![XrdsAction::ModifyHealth {
                    delta: XrdsActionValue::Fixed(-1.0),
                }],
            disabled: false,
            hand: None,
        }],
    );
    app.world_mut().entity_mut(entity).insert(XrdsHealth(100.0));
    app.update(); // primes below

    app.world_mut().entity_mut(entity).insert(Transform::from_xyz(0.0, 6.0, 0.0));
    pump(&mut app, 3); // up-crossing: fires, 99

    app.world_mut().entity_mut(entity).insert(Transform::from_xyz(0.0, 0.0, 0.0));
    pump(&mut app, 3); // down-crossing: Either re-arms and fires again, 98

    assert_eq!(
        app.world().get::<XrdsHealth>(entity).map(|h| h.0),
        Some(98.0),
        "Either should fire on both the up- and the down-crossing"
    );
}

#[test]
fn distance_to_watcher_uses_world_space_positions() {
    let mut app = xrds_test_app();

    let watched = import_node_with_watchers(
        &mut app,
        994,
        vec![XrdsThresholdWatcher {
            observable: XrdsObservable::DistanceTo { node: XrdsSceneNodeId(995) },
            crossing: XrdsCrossing::Below,
            value: 2.0,
            hysteresis: 0.0,
            fires: "close".to_string(),
            disabled: false,
        }],
        vec![Bound {
            trigger: XrdsTriggerKind::Custom("close".to_string()),
            steps: vec![XrdsAction::SetVisible(false)],
            disabled: false,
            hand: None,
        }],
    );
    // 995 must be in the *same* document as 994: import replaces the Track
    // registry wholesale, so a second import would drop 994's Track and its
    // binding would resolve to nothing.
    let other = app.world_mut().spawn_empty().id();
    app.world_mut().resource_mut::<XrdsIdIndex>().register(XrdsId(995), other);
    app.world_mut().entity_mut(other).insert((
        Transform::default(),
        GlobalTransform::default(),
    ));
    app.world_mut().entity_mut(other).insert(Transform::from_xyz(10.0, 0.0, 0.0));
    app.update(); // primes far (distance 10 > 2, so "not below" == Above)

    app.world_mut().entity_mut(watched).insert(Transform::from_xyz(9.0, 0.0, 0.0));
    pump(&mut app, 3); // distance now 1.0 < 2.0 -> crosses Below

    assert_eq!(
        app.world().get::<Visibility>(watched),
        Some(&Visibility::Hidden),
        "DistanceTo should have crossed Below once the nodes were close enough"
    );
}

#[test]
fn disabled_watcher_never_evaluates() {
    let mut app = xrds_test_app();

    let entity = import_node_with_watchers(
        &mut app,
        996,
        vec![XrdsThresholdWatcher {
            observable: XrdsObservable::Height,
            crossing: XrdsCrossing::Above,
            value: 5.0,
            hysteresis: 0.0,
            fires: "risen".to_string(),
            disabled: true,
        }],
        vec![Bound {
            trigger: XrdsTriggerKind::Custom("risen".to_string()),
            steps: vec![XrdsAction::SetTransform {
                            position: Some([1.0, 1.0, 1.0]),
                            rotation: None,
                            scale: None,
                            duration_secs: 0.0,
                            ease: XrdsEaseCurve::Linear,
                        }],
            disabled: false,
            hand: None,
        }],
    );
    app.update();
    app.world_mut().entity_mut(entity).insert(Transform::from_xyz(0.0, 50.0, 0.0));
    pump(&mut app, 3);

    assert_eq!(
        app.world().get::<Transform>(entity).map(|t| t.translation.y),
        Some(50.0),
        "a disabled watcher must never fire regardless of how far the value moves"
    );
}

// ---------------------------------------------------------------------------
// AnimateTransform / SetMaterial (sequencer redesign)
// ---------------------------------------------------------------------------

#[test]
fn set_transform_with_zero_duration_applies_instantly_and_leaves_unset_fields_alone() {
    // duration_secs <= 0.0 is the "instant" path (no XrdsTransformTween
    // component ever inserted) — deterministic, so this test doesn't need
    // to depend on real elapsed time between frames like the >0 case below.
    let mut app = xrds_test_app();

    let entity = import_bound(
        &mut app,
        940,
        vec![Bound {
            trigger: XrdsTriggerKind::ZoneEnter,
            steps: vec![XrdsAction::SetTransform {
                    position: None,
                    rotation: Some([0.0, 90.0, 0.0]),
                    scale: None,
                    duration_secs: 0.0,
                    ease: XrdsEaseCurve::Linear,
                }],
            disabled: false,
            hand: None,
        }],
    );

    app.world_mut().write_message(xrds_components::XrZoneEnterEvent {
        zone_id: XrdsId(940),
        entity_id: XrdsId(940),
    });
    pump(&mut app, 3);

    let transform = app.world().get::<Transform>(entity).copied().expect("has Transform");
    assert_eq!(
        transform.translation,
        Vec3::ZERO,
        "position was unset (None), so translation must be untouched"
    );
    assert_eq!(
        transform.scale,
        Vec3::ONE,
        "scale was unset (None), so it must be untouched"
    );
    let (_, y, _) = transform.rotation.to_euler(EulerRot::XYZ);
    assert!(
        (y.to_degrees() - 90.0).abs() < 0.01,
        "rotation was Some([0, 90, 0]), so Y rotation should be ~90 degrees, got {}",
        y.to_degrees()
    );
    assert_eq!(
        app.world_mut().query::<&XrdsTransformTween>().iter(app.world()).count(),
        0,
        "duration_secs <= 0.0 must never insert a tween component"
    );
}

#[test]
fn set_transform_reaches_target_and_blocks_the_sequence_queue() {
    let mut app = xrds_test_app();

    let entity = import_bound(
        &mut app,
        941,
        vec![Bound {
            trigger: XrdsTriggerKind::ZoneEnter,
            steps: vec![
                    XrdsAction::SetTransform {
                        position: Some([5.0, 0.0, 0.0]),
                        rotation: None,
                        scale: None,
                        duration_secs: 0.05,
                        ease: XrdsEaseCurve::Cubic,
                    },
                    // Only runs once the tween above finishes — proves
                    // SetTransform with a duration blocks its own one-step agent.
                    XrdsAction::ModifyHealth {
                        delta: XrdsActionValue::Fixed(-10.0),
                    },
                ],
            disabled: false,
            hand: None,
        }],
    );
    app.world_mut().entity_mut(entity).insert(XrdsHealth(100.0));

    app.world_mut().write_message(xrds_components::XrZoneEnterEvent {
        zone_id: XrdsId(941),
        entity_id: XrdsId(941),
    });

    // One frame in: the tween has started but 0.05s hasn't elapsed yet
    // (real time, same assumption the existing Wait tests already make),
    // so the queue must still be blocked on it.
    pump(&mut app, 1);
    assert_eq!(
        app.world().get::<XrdsHealth>(entity).map(|h| h.0),
        Some(100.0),
        "ModifyHealth must not run until the SetTransform ahead of it finishes"
    );

    // Plenty of frames to guarantee 0.05s of real time has passed.
    pump(&mut app, 60);

    assert_eq!(
        app.world().get::<Transform>(entity).map(|t| t.translation),
        Some(Vec3::new(5.0, 0.0, 0.0)),
        "SetTransform should have reached its target position exactly"
    );
    assert_eq!(
        app.world().get::<XrdsHealth>(entity).map(|h| h.0),
        Some(90.0),
        "the queue should have advanced to ModifyHealth once the tween finished"
    );
    assert_eq!(
        app.world_mut().query::<&XrdsTransformTween>().iter(app.world()).count(),
        0,
        "the tween component should have been removed on completion"
    );
}

#[test]
fn set_material_applies_only_the_provided_fields() {
    let mut app = xrds_test_app();

    // A single document import (Cube payload + its own trigger binding),
    // same as every other test in this file — mixing `xrds.spawn()` and a
    // later `import_scene_document` for the *same* node id isn't a
    // supported "reimport" path (import_scene_document is for a fresh
    // document, confirmed empirically: it errors with DuplicateRuntimeId).
    let cube_id = XrdsId(942);
    let document = XrdsSceneDocument {
        nodes: vec![XrdsSceneNode {
            id: XrdsSceneNodeId(cube_id.0),
            parent_id: None,
            name: "MaterialCube".to_string(),
            enabled: true,
            visible: true,
            transform: XrdsSceneTransform::default(),
            payload: XrdsSceneNodePayload::Cube(XrdsSceneCube::default()),
            grabbable: false,
            editor: XrdsEditorMetadata::default(),
            triggers: vec![XrdsTriggerBinding {
                trigger: XrdsTriggerKind::ZoneEnter,
                track: Some("recolour".to_string()),
                disabled: false,
                hand: None,
            }],
            watchers: Vec::new(),
        }],
        tracks: vec![XrdsNamedTrack {
            name: "recolour".to_string(),
            track: XrdsTrack {
                assets: vec![XrdsTrackAsset {
                    target: XrdsActionTarget::SelfNode,
                    keys: vec![XrdsTrackKey {
                        at_secs: 0.0,
                        action: XrdsAction::SetMaterial {
                            base_color: Some([1.0, 0.0, 0.0, 1.0]),
                            metallic: None,
                            roughness: Some(0.9),
                            texture: None,
                        },
                    }],
                }],
                ..XrdsTrack::default()
            },
        }],
        ..Default::default()
    };
    {
        let mut xrds = XrdsAPI::attach(&mut app);
        xrds.import_scene_document(&document).expect("import should succeed");
    }

    app.world_mut().write_message(xrds_components::XrZoneEnterEvent {
        zone_id: cube_id,
        entity_id: cube_id,
    });
    pump(&mut app, 3);

    let params = {
        let xrds = XrdsAPI::attach(&mut app);
        let cube = xrds.handle_of::<XrdsCube>(cube_id).unwrap();
        xrds.material_params(&cube).expect("cube should have a material")
    };
    assert_eq!(
        params.base_color.rgba,
        [1.0, 0.0, 0.0, 1.0],
        "base_color was Some, so it should have been overridden"
    );
    assert_eq!(
        params.pbr.roughness, 0.9,
        "roughness was Some, so it should have been overridden"
    );
    assert_eq!(
        params.pbr.metallic, 0.0,
        "metallic was None, so it should still be the default"
    );
}

// ---------------------------------------------------------------------------
// Cross-Track asset conflicts (reject-the-newcomer) and lock lifecycle
// ---------------------------------------------------------------------------

/// One asset row driving `node_id`.
fn node_row(node_id: u64, keys: Vec<(f32, XrdsAction)>) -> XrdsTrackAsset {
    XrdsTrackAsset {
        target: XrdsActionTarget::Node(XrdsSceneNodeId(node_id)),
        keys: keys
            .into_iter()
            .map(|(at_secs, action)| XrdsTrackKey { at_secs, action })
            .collect(),
    }
}

/// A long interpolation, so a Track stays in flight (and keeps its locks)
/// across the frames a test needs.
fn long_tween() -> XrdsAction {
    XrdsAction::SetTransform {
        position: Some([5.0, 0.0, 0.0]),
        rotation: None,
        scale: None,
        duration_secs: 10.0,
        ease: XrdsEaseCurve::Linear,
    }
}

/// Imports `node_ids` as plain nodes plus a Track registry, with no bindings.
/// Tracks are started directly so a test controls the exact firing order.
fn import_bare_nodes_and_tracks(
    app: &mut App,
    node_ids: &[u64],
    tracks: Vec<XrdsNamedTrack>,
) -> Vec<Entity> {
    let document = XrdsSceneDocument {
        nodes: node_ids.iter().map(|id| scene_node(*id, "Asset")).collect(),
        tracks,
        ..Default::default()
    };
    {
        let mut xrds = XrdsAPI::attach(app);
        xrds.import_scene_document(&document).expect("import should succeed");
    }
    let index = app.world().resource::<XrdsIdIndex>();
    node_ids
        .iter()
        .map(|id| index.entity_of(XrdsId(*id)).expect("indexed"))
        .collect()
}

fn start(app: &mut App, name: &str, on: Entity) -> Option<Entity> {
    let track = app
        .world()
        .resource::<crate::xrds_api::trigger_action::XrdsTrackRegistry>()
        .0
        .get(name)
        .cloned()
        .expect("track should be registered");
    crate::xrds_api::trigger_action::spawn_track_agent_in_world(
        app.world_mut(),
        on,
        None,
        name,
        &track,
        0,
        false,
    )
}

fn live_tracks(app: &mut App) -> usize {
    app.world_mut().query::<&XrdsTrackAgent>().iter(app.world()).count()
}

#[test]
fn a_track_sharing_an_asset_with_a_running_one_is_refused() {
    let mut app = xrds_test_app();
    let entities = import_bare_nodes_and_tracks(
        &mut app,
        &[601, 602],
        vec![
            XrdsNamedTrack {
                name: "A".to_string(),
                track: XrdsTrack {
                    assets: vec![node_row(601, vec![(0.0, long_tween())])],
                    ..XrdsTrack::default()
                },
            },
            // Shares 601 with A, and also drives 602.
            XrdsNamedTrack {
                name: "B".to_string(),
                track: XrdsTrack {
                    assets: vec![
                        node_row(601, vec![(0.0, long_tween())]),
                        node_row(602, vec![(0.0, long_tween())]),
                    ],
                    ..XrdsTrack::default()
                },
            },
        ],
    );

    assert!(start(&mut app, "A", entities[0]).is_some(), "A should start");
    assert!(
        start(&mut app, "B", entities[0]).is_none(),
        "B shares an asset with the running A, so it must be refused"
    );
    assert_eq!(live_tracks(&mut app), 1, "only A should be running");

    // Atomic refusal: B must not have partially started on 602 either.
    let locks = app.world().resource::<XrdsTrackAssetLocks>();
    assert!(
        locks.holder_of(entities[1]).is_none(),
        "a refused Track must not hold any asset - refusal is all-or-nothing"
    );
    let conflict = locks.last_conflict.as_ref().expect("the refusal should be reported");
    assert_eq!(conflict.blocked_track, "B");
    assert_eq!(conflict.contended, vec![entities[0]]);
}

#[test]
fn tracks_with_disjoint_assets_run_concurrently() {
    // The point of the whole rule: disjoint Tracks are free to overlap.
    let mut app = xrds_test_app();
    let entities = import_bare_nodes_and_tracks(
        &mut app,
        &[611, 612],
        vec![
            XrdsNamedTrack {
                name: "A".to_string(),
                track: XrdsTrack {
                    assets: vec![node_row(611, vec![(0.0, long_tween())])],
                    ..XrdsTrack::default()
                },
            },
            XrdsNamedTrack {
                name: "B".to_string(),
                track: XrdsTrack {
                    assets: vec![node_row(612, vec![(0.0, long_tween())])],
                    ..XrdsTrack::default()
                },
            },
        ],
    );

    assert!(start(&mut app, "A", entities[0]).is_some());
    assert!(start(&mut app, "B", entities[1]).is_some(), "disjoint assets must not conflict");
    assert_eq!(live_tracks(&mut app), 2);
}

#[test]
fn re_firing_a_running_track_is_refused_so_the_first_run_keeps_priority() {
    // Replaces an earlier test that asserted the opposite. A same-name
    // exemption used to despawn the running instance and restart it, which is
    // why "three buttons pressed together" meant "only the last one did
    // anything". The policy is now uniform: a running Track is never
    // preempted except by an explicit stop.
    let mut app = xrds_test_app();
    let entities = import_bare_nodes_and_tracks(
        &mut app,
        &[621],
        vec![XrdsNamedTrack {
            name: "A".to_string(),
            track: XrdsTrack {
                assets: vec![node_row(621, vec![(0.0, long_tween())])],
                ..XrdsTrack::default()
            },
        }],
    );

    let first = start(&mut app, "A", entities[0]).expect("first start");
    app.update();

    assert!(
        start(&mut app, "A", entities[0]).is_none(),
        "a second firing must be refused while the first still holds the asset"
    );
    assert_eq!(live_tracks(&mut app), 1, "no second agent should exist");
    assert_eq!(
        app.world().resource::<XrdsTrackAssetLocks>().holder_of(entities[0]),
        Some(first),
        "the *first* agent must keep the asset"
    );
}

#[test]
fn one_track_fired_from_several_sources_runs_concurrently_on_disjoint_assets() {
    // The case panel templates make routine: N instances of one template, each
    // firing the same Track, each driving its own asset through a
    // `TriggerSource` row. Nothing is contended, so nothing should be refused.
    //
    // The old name-keyed restart made this impossible — the second firing
    // despawned the first regardless of what it was touching.
    let mut app = xrds_test_app();
    let entities = import_bare_nodes_and_tracks(
        &mut app,
        &[631, 632, 633],
        vec![XrdsNamedTrack {
            name: "PerSource".to_string(),
            // A TriggerSource row resolves to whoever fired it, so each firing
            // touches a different entity.
            track: XrdsTrack {
                assets: vec![XrdsTrackAsset {
                    target: XrdsActionTarget::TriggerSource,
                    keys: vec![XrdsTrackKey { at_secs: 0.0, action: long_tween() }],
                }],
                ..XrdsTrack::default()
            },
        }],
    );

    // `start` passes the target as both target and source-less, so drive the
    // source explicitly through the world spawner.
    let mut agents = Vec::new();
    for e in &entities {
        let track = app
            .world()
            .resource::<crate::xrds_api::trigger_action::XrdsTrackRegistry>()
            .0
            .get("PerSource")
            .cloned()
            .expect("registered");
        let agent = crate::xrds_api::trigger_action::spawn_track_agent_in_world(
            app.world_mut(), *e, Some(*e), "PerSource", &track, 0, false,
        );
        agents.push(agent);
    }

    assert!(
        agents.iter().all(Option::is_some),
        "three firings onto three different assets must all start: {agents:?}"
    );
    assert_eq!(live_tracks(&mut app), 3, "all three should be running at once");
}

#[test]
fn one_track_fired_from_several_sources_onto_the_same_asset_lets_the_first_win() {
    // Same shape as above, but every firing lands on one shared asset — so the
    // existing reject-the-newcomer policy applies instead of a restart.
    let mut app = xrds_test_app();
    let entities = import_bare_nodes_and_tracks(
        &mut app,
        &[641],
        vec![XrdsNamedTrack {
            name: "Shared".to_string(),
            track: XrdsTrack {
                assets: vec![node_row(641, vec![(0.0, long_tween())])],
                ..XrdsTrack::default()
            },
        }],
    );

    let first = start(&mut app, "Shared", entities[0]).expect("first start");
    app.update();
    assert!(start(&mut app, "Shared", entities[0]).is_none(), "second refused");
    assert!(start(&mut app, "Shared", entities[0]).is_none(), "third refused");
    assert_eq!(
        app.world().resource::<XrdsTrackAssetLocks>().holder_of(entities[0]),
        Some(first),
        "the first firing keeps the asset"
    );
}

#[test]
fn previewing_the_same_track_twice_restarts_it_rather_than_being_refused() {
    // The editor's ⏮ restart button re-sends PreviewPlayTrack for the Track
    // that is *already* previewing. With first-run priority that only works
    // because `preview_play_track_in_world` stops the current preview first,
    // synchronously, so the locks are free before the new claim.
    //
    // Directly regression-guards removing the same-name restart: if that stop
    // ever became deferred, ⏮ would silently refuse instead of restarting, and
    // nothing else in the suite would notice — there were no preview tests.
    use crate::xrds_api::trigger_action::{preview_play_track_in_world, track_preview_state_in_world};

    let mut app = xrds_test_app();
    let _ = import_bare_nodes_and_tracks(
        &mut app,
        &[661],
        vec![XrdsNamedTrack {
            name: "P".to_string(),
            track: XrdsTrack {
                assets: vec![node_row(661, vec![(0.0, long_tween())])],
                duration_secs: Some(30.0),
                ..XrdsTrack::default()
            },
        }],
    );

    let first = preview_play_track_in_world(app.world_mut(), "P").expect("first preview");
    app.update();
    let second = preview_play_track_in_world(app.world_mut(), "P")
        .expect("re-previewing must restart, not be refused");

    assert_ne!(first, second, "restart should be a fresh agent");
    assert_eq!(live_tracks(&mut app), 1, "exactly one preview agent, not two");
    assert_eq!(
        track_preview_state_in_world(app.world_mut()).map(|(n, ..)| n),
        Some("P".to_string()),
        "the preview should still be reported as live"
    );
}

#[test]
fn an_explicit_stop_is_what_lets_a_track_be_restarted() {
    // First-run priority is not a dead end: stopping releases the locks, and
    // the next firing then starts normally. This is the path the editor's ⏮
    // restart button takes (`preview_stop_track_in_world` before spawning).
    let mut app = xrds_test_app();
    let entities = import_bare_nodes_and_tracks(
        &mut app,
        &[651],
        vec![XrdsNamedTrack {
            name: "A".to_string(),
            track: XrdsTrack {
                assets: vec![node_row(651, vec![(0.0, long_tween())])],
                ..XrdsTrack::default()
            },
        }],
    );

    let first = start(&mut app, "A", entities[0]).expect("first start");
    app.update();
    assert!(start(&mut app, "A", entities[0]).is_none(), "refused while running");

    crate::xrds_api::trigger_action::stop_all_sequences_in_world(app.world_mut());
    app.update();

    let second = start(&mut app, "A", entities[0]).expect("must start after an explicit stop");
    assert_ne!(first, second, "a fresh agent");
}

#[test]
fn locks_are_released_when_a_track_finishes_so_another_can_then_run() {
    // The leak this guards against is permanent and silent: a lock that
    // outlives its agent blocks every Track sharing that asset forever, and
    // presents as "the trigger just stopped working".
    let mut app = xrds_test_app();
    let entities = import_bare_nodes_and_tracks(
        &mut app,
        &[631],
        vec![
            XrdsNamedTrack {
                name: "Short".to_string(),
                track: XrdsTrack {
                    assets: vec![node_row(631, vec![(0.0, XrdsAction::SetVisible(false))])],
                    duration_secs: Some(0.01),
                    ..XrdsTrack::default()
                },
            },
            XrdsNamedTrack {
                name: "After".to_string(),
                track: XrdsTrack {
                    assets: vec![node_row(631, vec![(0.0, XrdsAction::SetVisible(true))])],
                    ..XrdsTrack::default()
                },
            },
        ],
    );

    assert!(start(&mut app, "Short", entities[0]).is_some());
    assert!(
        app.world().resource::<XrdsTrackAssetLocks>().holder_of(entities[0]).is_some(),
        "a running Track should hold its asset"
    );

    // Let it run to completion.
    for _ in 0..6 {
        app.update();
        std::thread::sleep(Duration::from_millis(10));
    }

    assert_eq!(live_tracks(&mut app), 0, "the Track should have finished");
    assert!(
        app.world().resource::<XrdsTrackAssetLocks>().is_empty(),
        "finishing must release every lock, or nothing can drive this asset again"
    );
    assert!(
        start(&mut app, "After", entities[0]).is_some(),
        "a later Track should be able to claim the freed asset"
    );
}

#[test]
fn stopping_a_track_releases_its_locks() {
    let mut app = xrds_test_app();
    let entities = import_bare_nodes_and_tracks(
        &mut app,
        &[641],
        vec![XrdsNamedTrack {
            name: "A".to_string(),
            track: XrdsTrack {
                assets: vec![node_row(641, vec![(0.0, long_tween())])],
                ..XrdsTrack::default()
            },
        }],
    );

    assert!(start(&mut app, "A", entities[0]).is_some());
    let stopped = crate::xrds_api::trigger_action::stop_all_sequences_in_world(app.world_mut());
    assert!(stopped > 0, "the running Track should have been stopped");
    assert!(
        app.world().resource::<XrdsTrackAssetLocks>().is_empty(),
        "an explicit stop must release locks too, not just natural completion"
    );
}

#[test]
fn a_looping_track_keeps_its_locks_forever_which_is_why_it_is_diagnosed() {
    // Runtime counterpart to the `track_diagnostics` error: a looping Track
    // never releases, so a sharer can never start. Asserted here so the
    // diagnostic and the runtime cannot drift apart on the claim.
    let mut app = xrds_test_app();
    let entities = import_bare_nodes_and_tracks(
        &mut app,
        &[651],
        vec![
            XrdsNamedTrack {
                name: "Ambient".to_string(),
                track: XrdsTrack {
                    assets: vec![node_row(651, vec![(0.0, XrdsAction::SetVisible(false))])],
                    duration_secs: Some(0.01),
                    looping: true,
                },
            },
            XrdsNamedTrack {
                name: "Blocked".to_string(),
                track: XrdsTrack {
                    assets: vec![node_row(651, vec![(0.0, XrdsAction::SetVisible(true))])],
                    ..XrdsTrack::default()
                },
            },
        ],
    );

    assert!(start(&mut app, "Ambient", entities[0]).is_some());
    for _ in 0..6 {
        app.update();
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(live_tracks(&mut app), 1, "a looping Track should still be running");
    assert!(
        start(&mut app, "Blocked", entities[0]).is_none(),
        "nothing sharing an asset with a looping Track can ever start"
    );
}

#[test]
fn one_track_drives_several_assets_from_their_own_rows() {
    // The core of the Track model: per-row targets, so one Track moves several
    // nodes. Under the old single-implicit-target timeline this was impossible.
    let mut app = xrds_test_app();
    let entities = import_bare_nodes_and_tracks(
        &mut app,
        &[661, 662],
        vec![XrdsNamedTrack {
            name: "Both".to_string(),
            track: XrdsTrack {
                assets: vec![
                    node_row(
                        661,
                        vec![(0.0, XrdsAction::SetTransform {
                            position: Some([1.0, 0.0, 0.0]),
                            rotation: None,
                            scale: None,
                            duration_secs: 0.0,
                            ease: XrdsEaseCurve::Linear,
                        })],
                    ),
                    node_row(
                        662,
                        vec![(0.0, XrdsAction::SetTransform {
                            position: Some([0.0, 2.0, 0.0]),
                            rotation: None,
                            scale: None,
                            duration_secs: 0.0,
                            ease: XrdsEaseCurve::Linear,
                        })],
                    ),
                ],
                ..XrdsTrack::default()
            },
        }],
    );

    // Fired at 661, but 662's row must still drive 662 - not the firing target.
    assert!(start(&mut app, "Both", entities[0]).is_some());
    pump(&mut app, 3);

    assert_eq!(
        app.world().get::<Transform>(entities[0]).map(|t| t.translation),
        Some(Vec3::new(1.0, 0.0, 0.0)),
    );
    assert_eq!(
        app.world().get::<Transform>(entities[1]).map(|t| t.translation),
        Some(Vec3::new(0.0, 2.0, 0.0)),
        "each row must apply to its own asset, not to the firing target"
    );
}

#[test]
fn a_paused_track_does_not_advance_but_keeps_its_assets() {
    let mut app = xrds_test_app();
    let entities = import_bare_nodes_and_tracks(
        &mut app,
        &[671],
        vec![
            XrdsNamedTrack {
                name: "Preview".to_string(),
                track: XrdsTrack {
                    assets: vec![node_row(
                        671,
                        vec![(0.05, XrdsAction::SetTransform {
                            position: Some([7.0, 0.0, 0.0]),
                            rotation: None,
                            scale: None,
                            duration_secs: 0.0,
                            ease: XrdsEaseCurve::Linear,
                        })],
                    )],
                    ..XrdsTrack::default()
                },
            },
            XrdsNamedTrack {
                name: "Other".to_string(),
                track: XrdsTrack {
                    assets: vec![node_row(671, vec![(0.0, XrdsAction::SetVisible(false))])],
                    ..XrdsTrack::default()
                },
            },
        ],
    );

    let agent = start(&mut app, "Preview", entities[0]).expect("start");
    app.world_mut().get_mut::<XrdsTrackAgent>(agent).unwrap().paused = true;

    for _ in 0..8 {
        app.update();
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(
        app.world().get::<Transform>(entities[0]).map(|t| t.translation),
        Some(Vec3::ZERO),
        "a paused Track must not fire its events"
    );
    assert_eq!(
        app.world().get::<XrdsTrackAgent>(agent).map(|a| a.elapsed_secs()),
        Some(0.0),
        "a paused Track clock must not advance"
    );
    // Pausing is not releasing: the preview still owns the asset.
    assert!(
        start(&mut app, "Other", entities[0]).is_none(),
        "a paused Track still holds its assets"
    );
}

// ---------------------------------------------------------------------------
// Looping restores its assets' initial state
//
// "A loop is a repeated Track. It doesn't rewind the world — it just puts the
// assets it owns back to the state they started in." Captured at spawn, not
// read from the document, so a Track fired while its assets sit somewhere the
// document never described still repeats from where *it* began.
// ---------------------------------------------------------------------------

// ---------------------------------------------------------------------------
// Panel element triggers
//
// The premise of the whole panel-template plan: an element carries its own
// triggers, they land on the entity the widget event targets, and
// `consume_triggers` fires the named Track — with no change to dispatch.
//
// Before this, `spawn_world_widget_from_scene` discarded the entity it created,
// so there was nothing to attach `XrdsTriggerBindings` to and every
// ButtonPress/SliderChange/ToggleChange on an authored widget was dropped.
// ---------------------------------------------------------------------------

/// Imports Tracks only, then hands back a bare entity to hang elements off —
/// standing in for the panel a real attachment would spawn.
fn import_tracks_and_a_panel(app: &mut App, tracks: Vec<XrdsNamedTrack>) -> Entity {
    let document = XrdsSceneDocument { tracks, ..Default::default() };
    {
        let mut xrds = XrdsAPI::attach(app);
        xrds.import_scene_document(&document).expect("import should succeed");
    }
    app.world_mut().spawn(Transform::default()).id()
}

fn button_element(name: &str, triggers: Vec<XrdsTriggerBinding>) -> xrds_scene_graph::XrdsPanelElement {
    xrds_scene_graph::XrdsPanelElement {
        name: name.to_string(),
        kind: xrds_scene_graph::XrdsSceneWorldWidget::Button(
            xrds_scene_graph::XrdsSceneWorldButton::default(),
        ),
        triggers,
    }
}

fn element_binding(kind: XrdsTriggerKind, track: &str) -> XrdsTriggerBinding {
    XrdsTriggerBinding {
        trigger: kind,
        track: Some(track.to_string()),
        disabled: false,
        hand: None,
    }
}

/// A Track that parks a marker node somewhere observable, so "did it run" is a
/// transform read rather than a log scrape.
fn move_track(name: &str, target: u64) -> XrdsNamedTrack {
    XrdsNamedTrack {
        name: name.to_string(),
        track: XrdsTrack {
            assets: vec![node_row(
                target,
                vec![(0.0, XrdsAction::SetTransform {
                    position: Some([42.0, 0.0, 0.0]),
                    rotation: None,
                    scale: None,
                    duration_secs: 0.0,
                    ease: XrdsEaseCurve::Linear,
                })],
            )],
            ..XrdsTrack::default()
        },
    }
}

#[test]
fn pressing_a_panel_element_fires_the_track_its_binding_names() {
    let mut app = xrds_test_app();
    // The Track drives node 810, which must exist for its row to resolve.
    let entities = import_bare_nodes_and_tracks(&mut app, &[810], vec![move_track("Open", 810)]);
    let panel = app.world_mut().spawn(Transform::default()).id();

    let element = button_element("start", vec![element_binding(XrdsTriggerKind::ButtonPress, "Open")]);
    let element_entity = crate::xrds_api::trigger_action::spawn_panel_element_in_world(
        app.world_mut(),
        panel,
        &element,
    );

    // The bindings must be on the element itself — that is the entity the event
    // targets, and the reason this could never work before.
    assert!(
        app.world()
            .get::<crate::xrds_api::trigger_action::XrdsTriggerBindings>(element_entity)
            .is_some(),
        "the element entity must carry its authored bindings"
    );

    app.world_mut().write_message(xrds_components::XrWorldButtonPressEvent {
        button_entity: element_entity,
        hand: XrGrabHand::Right,
    });

    assert!(
        spin_until(&mut app, 20, 5, |app| {
            app.world().get::<Transform>(entities[0]).map(|t| t.translation.x) == Some(42.0)
        }),
        "pressing the element should have fired its Track"
    );
}

#[test]
fn an_element_with_no_triggers_carries_no_bindings_component() {
    // Not just an empty list: an empty component would still match the query
    // `consume_triggers` runs, so remove-when-empty keeps "unbound" meaning
    // unbound.
    let mut app = xrds_test_app();
    let panel = import_tracks_and_a_panel(&mut app, vec![]);
    let entity = crate::xrds_api::trigger_action::spawn_panel_element_in_world(
        app.world_mut(),
        panel,
        &button_element("quiet", vec![]),
    );
    assert!(
        app.world()
            .get::<crate::xrds_api::trigger_action::XrdsTriggerBindings>(entity)
            .is_none(),
        "no triggers should mean no component at all"
    );
}

#[test]
fn clearing_an_elements_triggers_detaches_the_component() {
    // The remove-when-empty half, on the re-authoring path rather than spawn.
    let mut app = xrds_test_app();
    let panel = import_tracks_and_a_panel(&mut app, vec![]);
    let element = button_element("start", vec![element_binding(XrdsTriggerKind::ButtonPress, "Open")]);
    let entity = crate::xrds_api::trigger_action::spawn_panel_element_in_world(
        app.world_mut(),
        panel,
        &element,
    );
    assert!(app
        .world()
        .get::<crate::xrds_api::trigger_action::XrdsTriggerBindings>(entity)
        .is_some());

    crate::xrds_api::trigger_action::set_element_trigger_bindings(app.world_mut(), entity, &[]);
    assert!(
        app.world()
            .get::<crate::xrds_api::trigger_action::XrdsTriggerBindings>(entity)
            .is_none(),
        "clearing the last binding must detach, not leave an empty list"
    );
}

#[test]
fn a_disabled_element_binding_does_not_fire() {
    let mut app = xrds_test_app();
    let entities = import_bare_nodes_and_tracks(&mut app, &[811], vec![move_track("Open", 811)]);
    let panel = app.world_mut().spawn(Transform::default()).id();

    let mut binding = element_binding(XrdsTriggerKind::ButtonPress, "Open");
    binding.disabled = true;
    let entity = crate::xrds_api::trigger_action::spawn_panel_element_in_world(
        app.world_mut(),
        panel,
        &button_element("start", vec![binding]),
    );

    app.world_mut().write_message(xrds_components::XrWorldButtonPressEvent {
        button_entity: entity,
        hand: XrGrabHand::Right,
    });
    for _ in 0..8 {
        app.update();
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(
        app.world().get::<Transform>(entities[0]).map(|t| t.translation.x),
        Some(0.0),
        "a disabled binding must stay inert"
    );
}

#[test]
fn a_press_on_one_element_does_not_fire_another_elements_binding() {
    // Bindings are per-entity, so two elements on one panel stay independent —
    // which is what makes several buttons on a template usable at all.
    let mut app = xrds_test_app();
    let entities = import_bare_nodes_and_tracks(
        &mut app,
        &[812, 813],
        vec![move_track("A", 812), move_track("B", 813)],
    );
    let panel = app.world_mut().spawn(Transform::default()).id();

    let a = crate::xrds_api::trigger_action::spawn_panel_element_in_world(
        app.world_mut(),
        panel,
        &button_element("a", vec![element_binding(XrdsTriggerKind::ButtonPress, "A")]),
    );
    let _b = crate::xrds_api::trigger_action::spawn_panel_element_in_world(
        app.world_mut(),
        panel,
        &button_element("b", vec![element_binding(XrdsTriggerKind::ButtonPress, "B")]),
    );

    app.world_mut()
        .write_message(xrds_components::XrWorldButtonPressEvent { button_entity: a, hand: XrGrabHand::Right });

    assert!(
        spin_until(&mut app, 20, 5, |app| {
            app.world().get::<Transform>(entities[0]).map(|t| t.translation.x) == Some(42.0)
        }),
        "element A should have fired Track A"
    );
    assert_eq!(
        app.world().get::<Transform>(entities[1]).map(|t| t.translation.x),
        Some(0.0),
        "element B must not have fired"
    );
}

// ---------------------------------------------------------------------------
// Panel instances — the scene attachment
// ---------------------------------------------------------------------------

/// A document with one panel template and `count` nodes instancing it.
fn import_panel_instances(
    app: &mut App,
    count: usize,
    elements: Vec<xrds_scene_graph::XrdsPanelElement>,
    extra_nodes: &[u64],
    tracks: Vec<XrdsNamedTrack>,
) -> Vec<Entity> {
    use xrds_scene_graph::{XrdsPanelTemplate, XrdsPanelTemplateId, XrdsScenePanelInstance};

    let template = XrdsPanelTemplate {
        id: XrdsPanelTemplateId(1),
        name: "Menu".to_string(),
        elements,
        ..XrdsPanelTemplate::default()
    };

    let mut nodes: Vec<XrdsSceneNode> = (0..count)
        .map(|i| XrdsSceneNode {
            payload: XrdsSceneNodePayload::Panel(XrdsScenePanelInstance {
                template_id: XrdsPanelTemplateId(1),
            }),
            ..scene_node(900 + i as u64, "PanelInstance")
        })
        .collect();
    nodes.extend(extra_nodes.iter().map(|id| scene_node(*id, "Asset")));

    let document = XrdsSceneDocument {
        nodes,
        panels: vec![template],
        tracks,
        ..Default::default()
    };
    {
        let mut xrds = XrdsAPI::attach(app);
        xrds.import_scene_document(&document).expect("import should succeed");
    }
    let index = app.world().resource::<XrdsIdIndex>();
    (0..count)
        .map(|i| index.entity_of(XrdsId(900 + i as u64)).expect("panel indexed"))
        .collect()
}

/// Every entity carrying trigger bindings, i.e. every tagged element.
fn tagged_element_count(app: &mut App) -> usize {
    app.world_mut()
        .query_filtered::<Entity, bevy::prelude::With<crate::xrds_api::trigger_action::XrdsTriggerBindings>>()
        .iter(app.world())
        .count()
}

#[test]
fn a_panel_instance_spawns_its_templates_elements_with_bindings() {
    let mut app = xrds_test_app();
    import_panel_instances(
        &mut app,
        1,
        vec![button_element("start", vec![element_binding(XrdsTriggerKind::ButtonPress, "Open")])],
        &[820],
        vec![move_track("Open", 820)],
    );
    assert_eq!(
        tagged_element_count(&mut app),
        1,
        "the instance should have spawned one tagged element from its template"
    );
}

#[test]
fn a_template_instanced_twice_yields_two_independent_element_sets() {
    // The point of the template/instance split, and why elements are spawned per
    // instance rather than once per template. If these shared entities, two
    // panels could never behave independently.
    let mut app = xrds_test_app();
    let panels = import_panel_instances(
        &mut app,
        2,
        vec![button_element("start", vec![element_binding(XrdsTriggerKind::ButtonPress, "Open")])],
        &[821],
        vec![move_track("Open", 821)],
    );
    assert_eq!(panels.len(), 2);
    assert_ne!(panels[0], panels[1], "two instances are two entities");
    assert_eq!(
        tagged_element_count(&mut app),
        2,
        "each instance needs its own element entity, not a shared one"
    );
}

#[test]
fn an_element_on_an_instance_fires_its_track_end_to_end() {
    // The full authored path: document -> template -> instance -> element ->
    // binding -> Track. No hand-spawned entities anywhere.
    let mut app = xrds_test_app();
    import_panel_instances(
        &mut app,
        1,
        vec![button_element("start", vec![element_binding(XrdsTriggerKind::ButtonPress, "Open")])],
        &[822],
        vec![move_track("Open", 822)],
    );
    let target = app.world().resource::<XrdsIdIndex>().entity_of(XrdsId(822)).expect("indexed");

    // Find the element the import spawned, rather than assuming an entity id.
    let element = app
        .world_mut()
        .query_filtered::<Entity, bevy::prelude::With<crate::xrds_api::trigger_action::XrdsTriggerBindings>>()
        .iter(app.world())
        .next()
        .expect("the import should have tagged an element");

    app.world_mut().write_message(xrds_components::XrWorldButtonPressEvent {
        button_entity: element,
        hand: XrGrabHand::Right,
    });

    assert!(
        spin_until(&mut app, 20, 5, |app| {
            app.world().get::<Transform>(target).map(|t| t.translation.x) == Some(42.0)
        }),
        "an authored element on an authored instance should fire its Track"
    );
}

#[test]
fn a_panel_instance_naming_a_missing_template_loads_as_an_empty_node() {
    // Refusing to load the whole scene over one dangling reference would be
    // worse than an empty panel; the reference is diagnosed at author time.
    use xrds_scene_graph::{XrdsPanelTemplateId, XrdsScenePanelInstance};
    let mut app = xrds_test_app();
    let document = XrdsSceneDocument {
        nodes: vec![XrdsSceneNode {
            payload: XrdsSceneNodePayload::Panel(XrdsScenePanelInstance {
                template_id: XrdsPanelTemplateId(404),
            }),
            ..scene_node(830, "Dangling")
        }],
        ..Default::default()
    };
    {
        let mut xrds = XrdsAPI::attach(&mut app);
        xrds.import_scene_document(&document).expect("import must still succeed");
    }
    assert!(
        app.world().resource::<XrdsIdIndex>().entity_of(XrdsId(830)).is_some(),
        "the node itself should still exist"
    );
    assert_eq!(tagged_element_count(&mut app), 0, "but it has no elements");
}

// ---------------------------------------------------------------------------
// Live edits reach already-running agents
//
// `XrdsTrackAgent` is a snapshot taken at spawn, so without an explicit
// re-sync a running Track ignores every authored change. Reported symptom:
// editing a looping Track's duration and watching it keep lapping at the old
// one.
// ---------------------------------------------------------------------------

/// Rewrites a registered Track in place, the way an editor command does.
fn edit_registered_track(app: &mut App, name: &str, edit: impl FnOnce(&mut XrdsTrack)) {
    let mut registry = app
        .world_mut()
        .resource_mut::<crate::xrds_api::trigger_action::XrdsTrackRegistry>();
    let track = registry.0.get_mut(name).expect("track should be registered");
    edit(track);
}

#[test]
fn changing_a_running_looping_tracks_duration_takes_effect_immediately() {
    let mut app = xrds_test_app();
    let entities = import_bare_nodes_and_tracks(
        &mut app,
        &[700],
        vec![XrdsNamedTrack {
            name: "Loop".to_string(),
            track: XrdsTrack {
                assets: vec![node_row(700, vec![(0.0, XrdsAction::SetVisible(false))])],
                duration_secs: Some(60.0),
                looping: true,
            },
        }],
    );
    let agent = start(&mut app, "Loop", entities[0]).expect("start");
    app.update();
    assert_eq!(
        app.world().get::<XrdsTrackAgent>(agent).unwrap().duration_secs(),
        60.0
    );

    edit_registered_track(&mut app, "Loop", |t| t.duration_secs = Some(0.05));
    app.update();

    assert_eq!(
        app.world().get::<XrdsTrackAgent>(agent).unwrap().duration_secs(),
        0.05,
        "a running agent must adopt the Track's new duration, not keep lapping at the old one"
    );
}

#[test]
fn shortening_the_duration_below_the_clock_wraps_instead_of_stalling() {
    // Otherwise elapsed sits past the end and the Track waits out one more full
    // lap at the *old* length before the new duration is felt.
    let mut app = xrds_test_app();
    let entities = import_bare_nodes_and_tracks(
        &mut app,
        &[701],
        vec![XrdsNamedTrack {
            name: "Loop".to_string(),
            track: XrdsTrack {
                assets: vec![node_row(701, vec![(0.0, XrdsAction::SetVisible(false))])],
                duration_secs: Some(60.0),
                looping: true,
            },
        }],
    );
    let agent = start(&mut app, "Loop", entities[0]).expect("start");
    for _ in 0..3 {
        app.update();
        std::thread::sleep(Duration::from_millis(10));
    }
    let before = app.world().get::<XrdsTrackAgent>(agent).unwrap().elapsed_secs();
    assert!(before > 0.0, "clock should have advanced");

    // Shrink the duration to well under the elapsed time.
    edit_registered_track(&mut app, "Loop", |t| t.duration_secs = Some(0.005));
    app.update();

    let after = app.world().get::<XrdsTrackAgent>(agent).unwrap().elapsed_secs();
    assert!(
        after < 0.005,
        "elapsed ({after}) must be wrapped into the new duration, was {before}"
    );
}

#[test]
fn toggling_looping_off_while_running_lets_the_track_finish() {
    let mut app = xrds_test_app();
    let entities = import_bare_nodes_and_tracks(
        &mut app,
        &[702],
        vec![XrdsNamedTrack {
            name: "Loop".to_string(),
            track: XrdsTrack {
                assets: vec![node_row(702, vec![(0.0, XrdsAction::SetVisible(false))])],
                duration_secs: Some(0.03),
                looping: true,
            },
        }],
    );
    let agent = start(&mut app, "Loop", entities[0]).expect("start");
    app.update();
    assert!(app.world().get::<XrdsTrackAgent>(agent).unwrap().looping());

    edit_registered_track(&mut app, "Loop", |t| t.looping = false);

    // Now it must reach its end and despawn instead of lapping forever.
    assert!(
        spin_until(&mut app, 40, 5, |app| app.world().get::<XrdsTrackAgent>(agent).is_none()),
        "un-checking Loop on a running Track must let it finish"
    );
}

#[test]
fn a_structural_edit_is_not_adopted_mid_flight() {
    // Adopting a changed asset set would mean rewriting the lock table while
    // the Track runs; getting that wrong leaks a lock and blocks the asset for
    // the rest of the session. The Track keeps its original schedule and the
    // author re-fires to pick the change up.
    let mut app = xrds_test_app();
    let entities = import_bare_nodes_and_tracks(
        &mut app,
        &[703, 704],
        vec![XrdsNamedTrack {
            name: "Loop".to_string(),
            track: XrdsTrack {
                assets: vec![node_row(703, vec![(0.0, XrdsAction::SetVisible(false))])],
                duration_secs: Some(60.0),
                looping: true,
            },
        }],
    );
    let agent = start(&mut app, "Loop", entities[0]).expect("start");
    app.update();

    // Add a second asset row *and* change the duration in one edit.
    edit_registered_track(&mut app, "Loop", |t| {
        t.assets.push(node_row(704, vec![(0.0, XrdsAction::SetVisible(false))]));
        t.duration_secs = Some(0.05);
    });
    app.update();

    assert_eq!(
        app.world().get::<XrdsTrackAgent>(agent).unwrap().duration_secs(),
        60.0,
        "a structural edit must be skipped whole, not half-adopted"
    );
    let _ = entities[1];
}

/// Pumps the app until `pred` holds, or gives up after `tries`.
///
/// Both phases of the looping tests below need this rather than "update N
/// times, then assert": a key's effect is deferred (fire → command → spawn a
/// one-step agent → `SequentialActions` runs `on_start`), and these tests
/// advance real wall-clock time, which jitters when the suite runs in
/// parallel. Asserting after a fixed count made them pass alone and fail in
/// the full run.
fn spin_until(app: &mut App, tries: u32, ms: u64, mut pred: impl FnMut(&App) -> bool) -> bool {
    for _ in 0..tries {
        app.update();
        std::thread::sleep(Duration::from_millis(ms));
        if pred(app) {
            return true;
        }
    }
    false
}

#[test]
fn a_looping_track_puts_its_asset_back_before_each_new_lap() {
    let mut app = xrds_test_app();
    let entities = import_bare_nodes_and_tracks(
        &mut app,
        &[690],
        vec![XrdsNamedTrack {
            name: "Loop".to_string(),
            track: XrdsTrack {
                // Moves away and never moves back — only the loop restore can
                // return it, so this cannot pass by the Track undoing itself.
                assets: vec![node_row(
                    690,
                    vec![(0.02, XrdsAction::SetTransform {
                        position: Some([9.0, 0.0, 0.0]),
                        rotation: None,
                        scale: None,
                        duration_secs: 0.0,
                        ease: XrdsEaseCurve::Linear,
                    })],
                )],
                duration_secs: Some(0.08),
                looping: true,
            },
        }],
    );

    // Park it somewhere the document does *not* say, to prove the restore
    // target is the spawn-time state rather than the authored transform.
    app.world_mut().get_mut::<Transform>(entities[0]).unwrap().translation = Vec3::new(-4.0, 1.0, 2.0);

    start(&mut app, "Loop", entities[0]).expect("start");

    // Phase 1: the event moves it away.
    assert!(
        spin_until(&mut app, 20, 5, |app| {
            app.world().get::<Transform>(entities[0]).map(|t| t.translation.x) == Some(9.0)
        }),
        "the event should have moved the asset before any lap boundary"
    );

    // Phase 2: cross a lap boundary and catch the window *after* the restore
    // but *before* the 0.02s event re-fires.
    assert!(
        spin_until(&mut app, 60, 4, |app| {
            let x = app.world().get::<Transform>(entities[0]).unwrap().translation.x;
            (x - (-4.0)).abs() < 0.001
        }),
        "a looping Track must put its asset back to its spawn-time state at the top of a lap"
    );
}

#[test]
fn a_looping_track_restores_visibility_too_not_just_transform() {
    let mut app = xrds_test_app();
    let entities = import_bare_nodes_and_tracks(
        &mut app,
        &[691],
        vec![XrdsNamedTrack {
            name: "Blink".to_string(),
            track: XrdsTrack {
                assets: vec![node_row(691, vec![(0.02, XrdsAction::SetVisible(false))])],
                duration_secs: Some(0.08),
                looping: true,
            },
        }],
    );

    start(&mut app, "Blink", entities[0]).expect("start");
    assert!(
        spin_until(&mut app, 20, 5, |app| matches!(
            app.world().get::<Visibility>(entities[0]),
            Some(Visibility::Hidden)
        )),
        "the event should have hidden it first"
    );
    assert!(
        spin_until(&mut app, 60, 4, |app| !matches!(
            app.world().get::<Visibility>(entities[0]),
            Some(Visibility::Hidden)
        )),
        "looping restore must cover visibility, not only transform"
    );
}

#[test]
fn a_non_looping_track_captures_no_initial_state() {
    // The capture is not free (it reads material params per asset), so a
    // one-shot Track — which can never lap — must not pay for it.
    let mut app = xrds_test_app();
    let entities = import_bare_nodes_and_tracks(
        &mut app,
        &[692],
        vec![XrdsNamedTrack {
            name: "Once".to_string(),
            track: XrdsTrack {
                assets: vec![node_row(692, vec![(0.0, XrdsAction::SetVisible(false))])],
                duration_secs: Some(1.0),
                looping: false,
            },
        }],
    );
    let agent = start(&mut app, "Once", entities[0]).expect("start");
    assert!(
        !app.world().get::<XrdsTrackAgent>(agent).unwrap().has_initial_state(),
        "a non-looping Track should not capture initial state"
    );
}

#[test]
fn a_looping_track_does_capture_initial_state() {
    let mut app = xrds_test_app();
    let entities = import_bare_nodes_and_tracks(
        &mut app,
        &[693],
        vec![XrdsNamedTrack {
            name: "Round".to_string(),
            track: XrdsTrack {
                assets: vec![node_row(693, vec![(0.0, XrdsAction::SetVisible(false))])],
                duration_secs: Some(1.0),
                looping: true,
            },
        }],
    );
    let agent = start(&mut app, "Round", entities[0]).expect("start");
    assert!(
        app.world().get::<XrdsTrackAgent>(agent).unwrap().has_initial_state(),
        "a looping Track must capture what to put back"
    );
}

/// Regression test: pausing a Track only stopped `advance_tracks` from
/// firing *new* keys. A `SetTransform` already mid-flight lives on the
/// target as an `XrdsTransformTween`, with no link back to the agent that
/// started it, and `advance_transform_tweens` advanced every tween in the
/// world unconditionally — so a pause landing mid-glide looked like it did
/// nothing at all, since the cube kept sliding to its destination anyway.
#[test]
fn pausing_a_track_also_freezes_its_own_in_flight_interpolation() {
    let mut app = xrds_test_app();
    let entities = import_bare_nodes_and_tracks(
        &mut app,
        &[672],
        vec![XrdsNamedTrack {
            name: "Glide".to_string(),
            track: XrdsTrack {
                assets: vec![node_row(
                    672,
                    vec![(0.0, XrdsAction::SetTransform {
                        position: Some([10.0, 0.0, 0.0]),
                        rotation: None,
                        scale: None,
                        duration_secs: 1.0,
                        ease: XrdsEaseCurve::Linear,
                    })],
                )],
                ..XrdsTrack::default()
            },
        }],
    );

    let agent = start(&mut app, "Glide", entities[0]).expect("start");

    // Run partway into the tween, so it is genuinely in flight, not merely
    // scheduled.
    for _ in 0..4 {
        app.update();
        std::thread::sleep(Duration::from_millis(10));
    }
    let mid_x = app
        .world()
        .get::<Transform>(entities[0])
        .expect("cube has a transform")
        .translation
        .x;
    assert!(mid_x > 0.0 && mid_x < 10.0, "must be genuinely mid-glide, got x={mid_x}");

    app.world_mut().get_mut::<XrdsTrackAgent>(agent).unwrap().paused = true;

    for _ in 0..8 {
        app.update();
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(
        app.world().get::<Transform>(entities[0]).map(|t| t.translation.x),
        Some(mid_x),
        "a paused Track's in-flight interpolation must not keep advancing"
    );
}
