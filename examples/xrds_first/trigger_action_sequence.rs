// Trigger-action sequencing, watched in a real 3D scene.
//
// See docs/xrds-scenegraph-trigger-action-sequencing.md for the design and
// docs/done/xrds-trigger-action-v1.md Phase 5 for where this
// sits. This is the counterpart to the headless behavior tests in
// crates/xrds-runtime/src/tests/trigger_action.rs — those assert the
// mechanism is correct, this one lets you *see* it.
//
// Deliberately written against the default XrdsApp/XrdsAPI layer, not
// RuntimeHandler: authoring a trigger-action sequence should never require
// dropping to the expert layer. The only Bevy-level code here is the small
// system that synthesizes the zone events (see FIRING, below).
//
// What you should see, on a ~3s loop:
//   1. The red cube blinks off, then on   (SetVisible false → Wait → true)
//   2. It jumps to the right              (Teleport)
//   3. On the next tick it jumps back     (a ZoneExit binding, separate
//                                          sequence, separate agent)
//
// The staged blink is the point: those are four ordered steps of ONE
// authored sequence, running off a single trigger firing.
//
// FIRING: this example writes XrZoneEnterEvent/XrZoneExitEvent directly
// from a timer system rather than having a physics body walk into a sensor
// volume. That keeps what you're watching deterministic and free of
// physics setup — the events, the consume_triggers path, the sequencer and
// the actions are all the real production ones. The physics→event half
// (zone_collision_system) is separately covered and unchanged by this
// work.
use bevy::prelude::*;
use xrds::scene_graph::{
    XrdsAction, XrdsEditorMetadata, XrdsSceneCamera, XrdsSceneCameraProjection, XrdsSceneCube,
    XrdsSceneDocument, XrdsSceneMaterial, XrdsSceneMetadata, XrdsSceneNode, XrdsSceneNodePayload,
    XrdsScenePointLight, XrdsSceneTransform, XrdsSequence, XrdsSceneNodeId, XrdsTriggerBinding,
    XrdsTriggerKind,
};
use xrds::sdk::{XrZoneEnterEvent, XrZoneExitEvent, XrdsId};
use xrds::{Runtime, RuntimeParameters, XrdsAPI, XrdsApp, XrdsUpdateContext};

const CUBE_ID: u64 = 201;
const LIGHT_ID: u64 = 202;
const CAMERA_ID: u64 = 203;

const HOME: [f32; 3] = [-1.5, 0.5, 0.0];
const AWAY: [f32; 3] = [1.5, 0.5, 0.0];

struct TriggerActionDemo;

fn main() {
    Runtime::new(RuntimeParameters {
        app_name: "TriggerActionSequence".to_owned(),
        ..Default::default()
    })
    .run_xrds(TriggerActionDemo)
    .expect("Could not run application");
}

impl XrdsApp for TriggerActionDemo {
    fn setup(&mut self, api: &mut XrdsAPI<'_>) {
        api.import_scene_document(&demo_document())
            .expect("importing the trigger-action demo document should succeed");
        api.add_update_system(fire_zone_events_on_a_timer);
    }

    fn update(&mut self, _ctx: &mut XrdsUpdateContext<'_>) {}
}

/// Alternates ZoneEnter / ZoneExit every 3 seconds so both authored
/// bindings get exercised. Stands in for a physics body entering and
/// leaving a sensor volume — see FIRING in the module docs.
fn fire_zone_events_on_a_timer(
    time: Res<Time>,
    mut next_fire_secs: Local<f32>,
    mut inside: Local<bool>,
    mut enter: MessageWriter<XrZoneEnterEvent>,
    mut exit: MessageWriter<XrZoneExitEvent>,
) {
    // Give the imported scene a beat to finish spawning before the first
    // firing, so the very first sequence has something to act on.
    if *next_fire_secs == 0.0 {
        *next_fire_secs = 1.5;
    }
    if time.elapsed_secs() < *next_fire_secs {
        return;
    }
    *next_fire_secs = time.elapsed_secs() + 3.0;

    // target == source here: nothing external caused this, the node is
    // standing in as its own trigger source.
    let ids = (XrdsId(CUBE_ID), XrdsId(CUBE_ID));
    if *inside {
        println!("[t={:.1}s] firing ZoneExit  -> cube should return", time.elapsed_secs());
        exit.write(XrZoneExitEvent { zone_id: ids.0, entity_id: ids.1 });
    } else {
        println!("[t={:.1}s] firing ZoneEnter -> cube should blink, then move", time.elapsed_secs());
        enter.write(XrZoneEnterEvent { zone_id: ids.0, entity_id: ids.1 });
    }
    *inside = !*inside;
}

fn demo_document() -> XrdsSceneDocument {
    XrdsSceneDocument {
        metadata: XrdsSceneMetadata {
            name: "Trigger-Action Sequence Demo".to_string(),
            ..Default::default()
        },
        nodes: vec![
            XrdsSceneNode {
                id: XrdsSceneNodeId(CAMERA_ID),
                parent_id: None,
                name: "Camera".to_string(),
                enabled: true,
                visible: true,
                transform: XrdsSceneTransform {
                    translation: [0.0, 2.0, 6.0],
                    ..Default::default()
                },
                payload: XrdsSceneNodePayload::Camera(XrdsSceneCamera {
                    projection: XrdsSceneCameraProjection::Perspective {
                        fov_deg: 60.0,
                        near: 0.1,
                        far: Some(500.0),
                        order: 0,
                    },
                    look_at: Some([0.0, 0.5, 0.0]),
                }),
                grabbable: false,
                editor: XrdsEditorMetadata::default(),
                triggers: Vec::new(),
            },
            XrdsSceneNode {
                id: XrdsSceneNodeId(LIGHT_ID),
                parent_id: None,
                name: "Light".to_string(),
                enabled: true,
                visible: true,
                transform: XrdsSceneTransform {
                    translation: [2.0, 5.0, 3.0],
                    ..Default::default()
                },
                payload: XrdsSceneNodePayload::PointLight(XrdsScenePointLight {
                    intensity: 2_000_000.0,
                    ..Default::default()
                }),
                grabbable: false,
                editor: XrdsEditorMetadata::default(),
                triggers: Vec::new(),
            },
            // The star of the show: one node, two authored bindings.
            XrdsSceneNode {
                id: XrdsSceneNodeId(CUBE_ID),
                parent_id: None,
                name: "TriggeredCube".to_string(),
                enabled: true,
                visible: true,
                transform: XrdsSceneTransform {
                    translation: HOME,
                    ..Default::default()
                },
                payload: XrdsSceneNodePayload::Cube(XrdsSceneCube {
                    size: [1.0, 1.0, 1.0],
                    material: XrdsSceneMaterial {
                        base_color: [0.9, 0.2, 0.2, 1.0],
                        ..Default::default()
                    },
                    ..Default::default()
                }),
                grabbable: false,
                editor: XrdsEditorMetadata::default(),
                triggers: vec![
                    // Four ordered steps, one firing. The blink makes the
                    // sequencing visible rather than instantaneous.
                    XrdsTriggerBinding {
                        trigger: XrdsTriggerKind::ZoneEnter,
                        sequence: XrdsSequence {
                            steps: vec![
                                XrdsAction::SetVisible(false),
                                XrdsAction::Wait { seconds: 0.35 },
                                XrdsAction::SetVisible(true),
                                XrdsAction::Teleport { destination: AWAY },
                            ],
                        },
                        disabled: false,
                        hand: None,
                    },
                    // A separate binding — fires its own agent, and does
                    // NOT cancel or interfere with the one above.
                    XrdsTriggerBinding {
                        trigger: XrdsTriggerKind::ZoneExit,
                        sequence: XrdsSequence {
                            steps: vec![XrdsAction::Teleport { destination: HOME }],
                        },
                        disabled: false,
                        hand: None,
                    },
                ],
            },
        ],
        ..Default::default()
    }
}
