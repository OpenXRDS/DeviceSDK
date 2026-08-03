// Timeline-based composition and the runnable registry, watched in a real
// 3D scene (docs/done/xrds-trigger-action-v1.md, Phases 9 and 9a).
//
// This is the Phase 9/9a counterpart to trigger_action_sequence.rs (Phase
// 5): that example shows an ordered *sequence*; this one shows an
// absolute-time *timeline*, the document-level named-runnable registry, and
// `XrdsAction::Run` — the mechanism that lets a sequence start a timeline
// and a timeline start a sequence. See the terminology section in the
// Phase 9 write-up if "sequence" vs "timeline" isn't obvious: they are
// genuinely different execution models, not two names for the same thing.
//
// What you should see:
//   - Both cubes fire ONCE, a couple seconds in, then keep dancing on their
//     own forever. Nothing re-fires them — the repetition you see afterward
//     is the timeline's own `looping: true`, not a repeated trigger. That's
//     the actual point of a timeline over a sequence: a sequence's repeats
//     would need an external re-fire (compare trigger_action_sequence.rs,
//     which does exactly that on a 3s timer).
//   - Blue cube (binding -> timeline, direct): jumps away, blinks midway
//     through its trip (a `Run` into the shared "blink" *sequence*, fired
//     from inside the timeline — timeline-starts-sequence interop), then
//     jumps home and blinks off/on again — those last two things happen on
//     the SAME tick, two timeline keys sharing a timestamp, which a queue
//     cannot express but a timeline can.
//   - Green cube (binding -> inline sequence -> Run): waits half a second,
//     then plays the exact same "blink" sequence entry the blue cube's
//     timeline also uses (registry reuse — one authored rule, two
//     independent call sites), blocking on it (`wait: true`) before then
//     kicking off its own independent copy of the SAME "loop-dance" timeline
//     the blue cube runs (`wait: false` — fire-and-forget, since a green
//     cube waiting on a timeline that loops forever would never continue).
//     Both cubes then run the identical choreography, entirely
//     independently, just started from two different authoring paths.
//
// Not shown here: the chain-depth-capped runaway-loop escape hatch
// (`XrdsTriggerKind::RunawayDetected`). It resolves within a handful of
// frames, which isn't something a human watching the viewport can actually
// perceive — that guarantee is verified by
// `crates/xrds-runtime/src/tests/trigger_action.rs`
// (`run_action_runaway_chain_is_capped_and_fires_runaway_detected`), not by
// this visual example.
use bevy::prelude::*;
use xrds::scene_graph::{
    XrdsAction, XrdsEditorMetadata, XrdsNamedRunnable, XrdsRunnable, XrdsSceneCamera,
    XrdsSceneCameraProjection, XrdsSceneCube, XrdsSceneDocument, XrdsSceneMaterial,
    XrdsSceneMetadata, XrdsSceneNode, XrdsSceneNodeId, XrdsSceneNodePayload, XrdsScenePointLight,
    XrdsSceneTransform, XrdsSequence, XrdsTimeline, XrdsTimelineKey, XrdsTriggerBinding,
    XrdsTriggerKind,
};
use xrds::sdk::{XrZoneEnterEvent, XrdsId};
use xrds::{Runtime, RuntimeParameters, XrdsAPI, XrdsApp, XrdsUpdateContext};

const BLUE_CUBE_ID: u64 = 301;
const GREEN_CUBE_ID: u64 = 302;
const LIGHT_ID: u64 = 303;
const CAMERA_ID: u64 = 304;

const BLUE_HOME: [f32; 3] = [-1.5, 0.5, 0.0];
const GREEN_HOME: [f32; 3] = [1.5, 0.5, 0.0];

/// Shared by both cubes, from two different call sites — the reuse Phase
/// 9a's registry design was built for. Purely visibility, no baked-in
/// position, so it is safe to run on any node regardless of where that
/// node actually is.
const BLINK: &str = "blink";
/// Absolute-time choreography, looping. Uses `XrdsActionTarget::SelfNode`
/// implicitly (`SetVisible`/`Teleport` always apply to whichever node
/// started the chain), so the SAME registry entry produces the SAME
/// relative motion for either cube even though their homes differ.
const LOOP_DANCE: &str = "loop-dance";

struct TimelineInteropDemo;

fn main() {
    Runtime::new(RuntimeParameters {
        app_name: "TriggerActionTimeline".to_owned(),
        ..Default::default()
    })
    .run_xrds(TimelineInteropDemo)
    .expect("Could not run application");
}

impl XrdsApp for TimelineInteropDemo {
    fn setup(&mut self, api: &mut XrdsAPI<'_>) {
        api.import_scene_document(&demo_document())
            .expect("importing the timeline-interop demo document should succeed");
        api.add_update_system(fire_each_cube_once);
    }

    fn update(&mut self, _ctx: &mut XrdsUpdateContext<'_>) {}
}

/// Fires each cube's ZoneEnter exactly once, staggered, then never again —
/// unlike trigger_action_sequence.rs's repeating timer. Everything you see
/// after these two firings is the looping timeline sustaining itself.
fn fire_each_cube_once(
    time: Res<Time>,
    mut blue_fired: Local<bool>,
    mut green_fired: Local<bool>,
    mut enter: MessageWriter<XrZoneEnterEvent>,
) {
    if !*blue_fired && time.elapsed_secs() >= 1.5 {
        println!("[t={:.1}s] firing ZoneEnter on the blue cube (binding -> timeline)", time.elapsed_secs());
        enter.write(XrZoneEnterEvent { zone_id: XrdsId(BLUE_CUBE_ID), entity_id: XrdsId(BLUE_CUBE_ID) });
        *blue_fired = true;
    }
    if !*green_fired && time.elapsed_secs() >= 4.0 {
        println!(
            "[t={:.1}s] firing ZoneEnter on the green cube (binding -> sequence -> Run)",
            time.elapsed_secs()
        );
        enter.write(XrZoneEnterEvent { zone_id: XrdsId(GREEN_CUBE_ID), entity_id: XrdsId(GREEN_CUBE_ID) });
        *green_fired = true;
    }
}

fn demo_document() -> XrdsSceneDocument {
    XrdsSceneDocument {
        metadata: XrdsSceneMetadata {
            name: "Timeline Interop Demo".to_string(),
            ..Default::default()
        },
        // The document-level registry (XrdsSceneDocument::runnables) —
        // named templates, referenced by name from bindings and from
        // XrdsAction::Run. See docs/done/xrds-trigger-action-v1.md Phase 9a.
        runnables: vec![
            XrdsNamedRunnable {
                name: BLINK.to_string(),
                runnable: XrdsRunnable::Sequence(XrdsSequence {
                    steps: vec![
                        XrdsAction::SetVisible(false),
                        XrdsAction::Wait { seconds: 0.6 },
                        XrdsAction::SetVisible(true),
                    ],
                }),
            },
            // Stretched well past what the mechanism itself needs, purely
            // so a human watching the window can actually follow each step
            // instead of it reading as a blur.
            XrdsNamedRunnable {
                name: LOOP_DANCE.to_string(),
                runnable: XrdsRunnable::Timeline(XrdsTimeline {
                    keys: vec![
                        // 0.0s: jump away from home.
                        XrdsTimelineKey {
                            at_secs: 0.0,
                            action: XrdsAction::Teleport { destination: [0.0, 0.5, -3.0] },
                        },
                        // 1.8s: timeline -> sequence interop. Fire-and-forget
                        // (wait is ignored on a timeline key anyway) so the
                        // timeline's own clock never pauses for it.
                        XrdsTimelineKey {
                            at_secs: 1.8,
                            action: XrdsAction::Run { runnable: BLINK.to_string(), wait: false },
                        },
                        // 3.6s: two keys sharing a timestamp — the
                        // concurrency an ordered queue cannot express.
                        XrdsTimelineKey {
                            at_secs: 3.6,
                            action: XrdsAction::Teleport { destination: [0.0, 0.5, 0.0] },
                        },
                        XrdsTimelineKey {
                            at_secs: 3.6,
                            action: XrdsAction::SetVisible(false),
                        },
                        XrdsTimelineKey {
                            at_secs: 4.1,
                            action: XrdsAction::SetVisible(true),
                        },
                    ],
                    // A pause before it laps, so the "home" pose is visible
                    // for a beat rather than looping instantly.
                    duration_secs: Some(6.0),
                    looping: true,
                }),
            },
        ],
        nodes: vec![
            XrdsSceneNode {
                id: XrdsSceneNodeId(CAMERA_ID),
                parent_id: None,
                name: "Camera".to_string(),
                enabled: true,
                visible: true,
                transform: XrdsSceneTransform {
                    translation: [0.0, 2.5, 5.0],
                    ..Default::default()
                },
                payload: XrdsSceneNodePayload::Camera(XrdsSceneCamera {
                    projection: XrdsSceneCameraProjection::Perspective {
                        fov_deg: 65.0,
                        near: 0.1,
                        far: Some(500.0),
                        order: 0,
                    },
                    look_at: Some([0.0, 0.5, -1.0]),
                }),
                grabbable: false,
                editor: XrdsEditorMetadata::default(),
                triggers: Vec::new(),
                watchers: Vec::new(),
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
                watchers: Vec::new(),
            },
            // Binding -> timeline, directly through the registry. No Run
            // action needed: `runnable` can name either a sequence or a
            // timeline, and this binding leaves the inline `sequence` at
            // its default (empty), since `runnable` takes priority.
            XrdsSceneNode {
                id: XrdsSceneNodeId(BLUE_CUBE_ID),
                parent_id: None,
                name: "BlueCube".to_string(),
                enabled: true,
                visible: true,
                transform: XrdsSceneTransform {
                    translation: BLUE_HOME,
                    ..Default::default()
                },
                payload: XrdsSceneNodePayload::Cube(XrdsSceneCube {
                    size: [1.0, 1.0, 1.0],
                    material: XrdsSceneMaterial {
                        base_color: [0.2, 0.3, 0.9, 1.0],
                        ..Default::default()
                    },
                    ..Default::default()
                }),
                grabbable: false,
                editor: XrdsEditorMetadata::default(),
                triggers: vec![XrdsTriggerBinding {
                    trigger: XrdsTriggerKind::ZoneEnter,
                    sequence: XrdsSequence::default(),
                    disabled: false,
                    hand: None,
                    runnable: Some(LOOP_DANCE.to_string()),
                }],
                watchers: Vec::new(),
            },
            // Binding -> inline sequence -> Run. This is the other way to
            // reach the same two registry entries: an authored sequence
            // step that names them explicitly, rather than the binding
            // pointing straight at one.
            XrdsSceneNode {
                id: XrdsSceneNodeId(GREEN_CUBE_ID),
                parent_id: None,
                name: "GreenCube".to_string(),
                enabled: true,
                visible: true,
                transform: XrdsSceneTransform {
                    translation: GREEN_HOME,
                    ..Default::default()
                },
                payload: XrdsSceneNodePayload::Cube(XrdsSceneCube {
                    size: [1.0, 1.0, 1.0],
                    material: XrdsSceneMaterial {
                        base_color: [0.2, 0.8, 0.3, 1.0],
                        ..Default::default()
                    },
                    ..Default::default()
                }),
                grabbable: false,
                editor: XrdsEditorMetadata::default(),
                triggers: vec![XrdsTriggerBinding {
                    trigger: XrdsTriggerKind::ZoneEnter,
                    sequence: XrdsSequence {
                        steps: vec![
                            XrdsAction::Wait { seconds: 1.0 },
                            // Blocks until "blink" finishes, since it is
                            // still inside a sequence's queue here.
                            XrdsAction::Run { runnable: BLINK.to_string(), wait: true },
                            // Then kicks off its own independent
                            // "loop-dance" — fire-and-forget, since it
                            // loops forever and a sequence waiting on it
                            // would never move on.
                            XrdsAction::Run { runnable: LOOP_DANCE.to_string(), wait: false },
                        ],
                    },
                    disabled: false,
                    hand: None,
                    runnable: None,
                }],
                watchers: Vec::new(),
            },
        ],
        ..Default::default()
    }
}
