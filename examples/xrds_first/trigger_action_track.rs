// Tracks — one piece of choreography driving several assets, watched in a
// real 3D scene. See `docs/done/xrds-track-model-plan.md`.
//
// Replaces the former `trigger_action_sequence` and `trigger_action_timeline`
// examples. Those two existed to contrast an ordered *sequence* against an
// absolute-time *timeline*, and to show `XrdsAction::Run` bridging them. That
// distinction turned out to be illusory — every action that blocked a
// sequence blocked for a duration known at author time, so any sequence could
// be rewritten as absolute times — so there is now one execution model, and
// nothing for a second example to contrast against.
//
// What you should see:
//
//   1. At ~1.5s, ONE trigger fires ONE Track, and *both* cubes start moving.
//      That is the point of the model: a Track has one row per asset, so a
//      single trigger choreographs several nodes. Under the old timeline a
//      firing had exactly one implicit target, and this was impossible.
//
//   2. The blue cube slides out and back; the green cube does the same, a
//      beat later. Each cube's timing lives on its own row.
//
//   3. Both cubes blink off and on *together*, on the same tick — two rows
//      with keys at the same timestamp. Concurrency an ordered queue cannot
//      express.
//
//   4. It repeats forever without being re-triggered. That is the Track's own
//      `looping`, not a repeated trigger — nothing fires it again.
//
//   5. At ~10s a SECOND Track ("steal-blue") is fired, which wants the blue
//      cube. **Nothing visibly happens**, and the console explains why: the
//      looping Track already holds that asset, so the newcomer is refused
//      whole rather than half-applied. Watch for the
//      "was not started" warning. `track_diagnostics` reports this pairing at
//      author time too — as an error, because a *looping* holder never
//      releases, so the sharer can never run at all.
use bevy::prelude::*;
use xrds::scene_graph::{
    XrdsAction, XrdsActionTarget, XrdsEaseCurve, XrdsEditorMetadata, XrdsNamedTrack,
    XrdsSceneCamera, XrdsSceneCameraProjection, XrdsSceneCube, XrdsSceneDocument, XrdsSceneMaterial,
    XrdsSceneMetadata, XrdsSceneNode, XrdsSceneNodeId, XrdsSceneNodePayload, XrdsScenePointLight,
    XrdsSceneTransform, XrdsTrack, XrdsTrackAsset, XrdsTrackKey, XrdsTriggerBinding,
    XrdsTriggerKind,
};
use xrds::sdk::{XrZoneEnterEvent, XrdsId};
use xrds::{Runtime, RuntimeParameters, XrdsAPI, XrdsApp, XrdsUpdateContext};

const BLUE_CUBE_ID: u64 = 301;
const GREEN_CUBE_ID: u64 = 302;
const LIGHT_ID: u64 = 303;
const CAMERA_ID: u64 = 304;
/// Bound to nothing visible; exists only so its ZoneEnter can fire the second
/// Track and demonstrate the conflict refusal.
const TRIGGER_ID: u64 = 305;

const BLUE_HOME: [f32; 3] = [-1.5, 0.5, 0.0];
const GREEN_HOME: [f32; 3] = [1.5, 0.5, 0.0];

/// The Track both cubes belong to — one row each.
const DANCE: &str = "dance";
/// Wants the blue cube, and will be refused while `DANCE` is running.
const STEAL_BLUE: &str = "steal-blue";

struct TrackDemo;

fn main() {
    Runtime::new(RuntimeParameters {
        app_name: "TriggerActionTrack".to_owned(),
        ..Default::default()
    })
    .run_xrds(TrackDemo)
    .expect("Could not run application");
}

impl XrdsApp for TrackDemo {
    fn setup(&mut self, api: &mut XrdsAPI<'_>) {
        let document = demo_document();

        // Print what the authoring-time checks say before anything runs. The
        // shared-asset pairing below is diagnosable statically — you do not
        // have to wait for the runtime refusal to find out.
        for d in document.track_diagnostics() {
            println!("[diagnostic] {:?}: {} — {}", d.severity, d.title, d.detail);
        }

        api.import_scene_document(&document)
            .expect("importing the track demo document should succeed");
        api.add_update_system(fire_once_then_try_to_steal);
    }

    fn update(&mut self, _ctx: &mut XrdsUpdateContext<'_>) {}
}

/// Fires the shared Track once, then later fires a competing Track that wants
/// an asset the first one still holds. Neither is ever fired again.
fn fire_once_then_try_to_steal(
    time: Res<Time>,
    mut started: Local<bool>,
    mut stole: Local<bool>,
    mut enter: MessageWriter<XrZoneEnterEvent>,
) {
    if !*started && time.elapsed_secs() >= 1.5 {
        println!(
            "[t={:.1}s] firing ZoneEnter -> Track {DANCE:?}: one trigger, two cubes",
            time.elapsed_secs()
        );
        enter.write(XrZoneEnterEvent {
            zone_id: XrdsId(BLUE_CUBE_ID),
            entity_id: XrdsId(BLUE_CUBE_ID),
        });
        *started = true;
    }

    if !*stole && time.elapsed_secs() >= 10.0 {
        println!(
            "[t={:.1}s] firing ZoneEnter -> Track {STEAL_BLUE:?}, which wants the blue cube. \
             Expect a refusal, not a fight.",
            time.elapsed_secs()
        );
        enter.write(XrZoneEnterEvent {
            zone_id: XrdsId(TRIGGER_ID),
            entity_id: XrdsId(TRIGGER_ID),
        });
        *stole = true;
    }
}

/// A smooth move to `to` over `secs`.
fn glide(to: [f32; 3], secs: f32) -> XrdsAction {
    XrdsAction::SetTransform {
        position: Some(to),
        rotation: None,
        scale: None,
        duration_secs: secs,
        ease: XrdsEaseCurve::Cubic,
    }
}

fn key(at_secs: f32, action: XrdsAction) -> XrdsTrackKey {
    XrdsTrackKey { at_secs, action }
}

/// One asset row: every event for `node_id` inside this Track.
fn row(node_id: u64, keys: Vec<XrdsTrackKey>) -> XrdsTrackAsset {
    XrdsTrackAsset {
        target: XrdsActionTarget::Node(XrdsSceneNodeId(node_id)),
        keys,
    }
}

fn cube(node_id: u64, name: &str, home: [f32; 3], colour: [f32; 4]) -> XrdsSceneNode {
    XrdsSceneNode {
        id: XrdsSceneNodeId(node_id),
        parent_id: None,
        name: name.to_string(),
        enabled: true,
        visible: true,
        transform: XrdsSceneTransform { translation: home, ..Default::default() },
        payload: XrdsSceneNodePayload::Cube(XrdsSceneCube {
            size: [1.0, 1.0, 1.0],
            material: XrdsSceneMaterial { base_color: colour, ..Default::default() },
            ..Default::default()
        }),
        grabbable: false,
        editor: XrdsEditorMetadata::default(),
        triggers: Vec::new(),
        watchers: Vec::new(),
    }
}

fn demo_document() -> XrdsSceneDocument {
    // Timings are stretched well past what the mechanism needs, purely so a
    // human watching the window can follow each beat instead of it reading as
    // a blur.
    let dance = XrdsTrack {
        assets: vec![
            row(
                BLUE_CUBE_ID,
                vec![
                    key(0.0, glide([-1.5, 0.5, -3.0], 1.2)),
                    key(2.0, glide(BLUE_HOME, 1.2)),
                    // Same timestamp as the green row's blink below.
                    key(4.5, XrdsAction::SetVisible(false)),
                    key(4.9, XrdsAction::SetVisible(true)),
                ],
            ),
            row(
                GREEN_CUBE_ID,
                vec![
                    // A beat later than blue: per-row timing.
                    key(1.0, glide([1.5, 0.5, -3.0], 1.2)),
                    key(3.0, glide(GREEN_HOME, 1.2)),
                    // Shares 4.5s with the blue row — the two cubes blink
                    // together, which an ordered queue could not express.
                    key(4.5, XrdsAction::SetVisible(false)),
                    key(4.9, XrdsAction::SetVisible(true)),
                ],
            ),
        ],
        // A beat of stillness before it laps, so "home" is visible rather
        // than the loop restarting instantly.
        duration_secs: Some(6.0),
        looping: true,
    };

    // Wants the blue cube. Because `dance` loops, it holds blue forever, so
    // this can never start — reported statically as an error, and refused at
    // runtime with a log line.
    let steal_blue = XrdsTrack {
        assets: vec![row(
            BLUE_CUBE_ID,
            vec![key(0.0, glide([0.0, 3.0, 0.0], 0.8))],
        )],
        ..XrdsTrack::default()
    };

    XrdsSceneDocument {
        metadata: XrdsSceneMetadata { name: "Track Demo".to_string(), ..Default::default() },
        tracks: vec![
            XrdsNamedTrack { name: DANCE.to_string(), track: dance },
            XrdsNamedTrack { name: STEAL_BLUE.to_string(), track: steal_blue },
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
            // The blue cube carries the binding, but the Track it names drives
            // *both* cubes. A binding says "when this happens, run that
            // choreography" — it does not scope what the choreography touches.
            XrdsSceneNode {
                triggers: vec![XrdsTriggerBinding {
                    trigger: XrdsTriggerKind::ZoneEnter,
                    track: Some(DANCE.to_string()),
                    disabled: false,
                    hand: None,
                }],
                ..cube(BLUE_CUBE_ID, "BlueCube", BLUE_HOME, [0.2, 0.3, 0.9, 1.0])
            },
            cube(GREEN_CUBE_ID, "GreenCube", GREEN_HOME, [0.2, 0.8, 0.3, 1.0]),
            // Invisible node whose only job is to own the second binding, so
            // the refusal can be triggered independently of the cubes.
            XrdsSceneNode {
                id: XrdsSceneNodeId(TRIGGER_ID),
                parent_id: None,
                name: "StealTrigger".to_string(),
                enabled: true,
                visible: false,
                transform: XrdsSceneTransform::default(),
                payload: XrdsSceneNodePayload::Empty,
                grabbable: false,
                editor: XrdsEditorMetadata::default(),
                triggers: vec![XrdsTriggerBinding {
                    trigger: XrdsTriggerKind::ZoneEnter,
                    track: Some(STEAL_BLUE.to_string()),
                    disabled: false,
                    hand: None,
                }],
                watchers: Vec::new(),
            },
        ],
        ..Default::default()
    }
}
