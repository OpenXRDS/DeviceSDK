// Audible check that an authored distance falloff is actually honoured.
//
// This exists because the four falloff fields on `XrdsSceneAudioClip`
// (`distance_model`, `min_distance`, `max_distance`, `rolloff_factor`) were
// serialized and inert until 2026-08-19 — an author could set them, save, reload,
// and hear no difference. See `docs/small-phases-plan.md` S1.
//
// ## Why two sources rather than one
//
// One source moving away gets quieter *whether or not* the authored curve is read,
// because rodio applies its own hardcoded inverse-square law regardless. Walking
// away from a single clip therefore proves nothing.
//
// So: two clips, same audio file, same volume, mirrored to left and right, always
// EQUIDISTANT from the listener — but with deliberately opposite curves.
//
//   NearField (LEFT,  red)  max_distance =  6  — dies quickly, silent past 6 m
//   FarField  (RIGHT, blue) max_distance = 60  — carries across the whole scene
//
// The camera pulls straight back along +Z, so both sources recede together at an
// identical rate. Any asymmetry you hear can only come from the authored fields.
//
// ## What you should hear
//
// Start: both audible, centred-ish, roughly equal.
// ~6 m:  the LEFT source drops out completely. The RIGHT source keeps playing.
// Later: the image is entirely right-hand, and slowly fades.
//
// Before S1 both channels stayed balanced the whole way out. If you hear balance,
// the falloff is not being applied.
//
// ## Reading it in the terminal
//
// Run with the falloff diagnostics on to see the numbers behind the sound:
//
//   RUST_LOG=xrds_runtime=debug cargo run --example spatial_audio_falloff_check
//
// One `[audio-falloff]` line per source per second: distance, model, the authored
// bounds, the resulting gain, and the volume actually pushed to the sink.

use xrds::scene_graph::{
    XrdsAudioDistanceModel, XrdsEditorMetadata, XrdsSceneAsset, XrdsSceneAssetKind,
    XrdsSceneAudioClip, XrdsSceneCamera, XrdsSceneCameraProjection, XrdsSceneDocument,
    XrdsSceneDocumentSession, XrdsSceneMetadata, XrdsSceneNode, XrdsSceneNodeId,
    XrdsSceneNodePayload, XrdsSceneTransform,
};
use xrds::sdk::{
    primitives::{XrdsCube, XrdsPlane3D, XrdsSphere},
    world::XrdsCamera,
    XrdsColor, XrdsId, XrdsLinearRgba, XrdsMaterialParams,
};
use xrds::{Handle, Runtime, RuntimeParameters, XrdsAPI, XrdsApp, XrdsUpdateContext};

const ROOT_ID: XrdsSceneNodeId = XrdsSceneNodeId(900);
const CAMERA_ID: XrdsSceneNodeId = XrdsSceneNodeId(901);
const NEAR_FIELD_ID: XrdsSceneNodeId = XrdsSceneNodeId(902);
const FAR_FIELD_ID: XrdsSceneNodeId = XrdsSceneNodeId(903);

const TONE_ASSET_ID: &str = "asset:audio-tone";

/// Both sources sit on this line; the camera starts here and reverses along +Z.
const SOURCE_Z: f32 = 0.0;
/// Mirrored either side of centre, so the two are always the same distance away.
const SOURCE_X: f32 = 1.2;
const EAR_HEIGHT: f32 = 1.6;

/// The whole point of the check: identical in every respect except these.
const NEAR_FIELD_MAX: f32 = 6.0;
const FAR_FIELD_MAX: f32 = 60.0;

/// Camera pull-back speed, metres per second. Slow enough to hear the left
/// channel drop out rather than have it flash past.
const PULLBACK_SPEED: f32 = 1.2;
const PULLBACK_LIMIT: f32 = 24.0;

/// Lateral pass for the direction phase. Close in, because rodio's panning is
/// strong near the listener and negligible far from it.
const STRAFE_LIMIT: f32 = 5.0;
const STRAFE_Z: f32 = 2.0;

fn main() {
    Runtime::new(RuntimeParameters {
        app_name: "SpatialAudioFalloffCheck".to_owned(),
        ..Default::default()
    })
    .run_xrds(SpatialAudioFalloffCheckApp::default())
    .expect("failed to run spatial_audio_falloff_check example");
}

#[derive(Default)]
struct SpatialAudioFalloffCheckApp {
    camera: Option<Handle<XrdsCamera>>,
    announced_near_cutoff: bool,
    phase_name: &'static str,
}

impl XrdsApp for SpatialAudioFalloffCheckApp {
    fn setup(&mut self, api: &mut XrdsAPI<'_>) {
        let session = XrdsSceneDocumentSession::new(authored_scene_document())
            .expect("authored document should be valid");
        api.import_scene_document(session.document())
            .expect("falloff check document import should succeed");

        // Floor, so pulling back reads as motion rather than a fade.
        let floor = api.spawn(&{
            let mut p = XrdsPlane3D::new().with_name("Floor");
            p.size = [80.0, 80.0];
            p.transform.rotation_quat_xyzw = [-0.70710677, 0.0, 0.0, 0.70710677];
            p
        });
        api.set_material_params(
            &floor,
            XrdsMaterialParams {
                base_color: XrdsColor::srgb(0.07, 0.08, 0.09),
                ..Default::default()
            },
        );

        // Distance ticks every 2 m along the pull-back path, so the moment the
        // left channel cuts out can be read off the floor against NEAR_FIELD_MAX.
        for i in 1..=12i32 {
            let z = i as f32 * 2.0;
            let plank = api.spawn(&{
                let mut c = XrdsCube::new().with_name("Tick");
                c.size = [6.0, 0.02, 0.05];
                c.transform.translation = [0.0, 0.01, z];
                c
            });
            api.set_material_params(
                &plank,
                XrdsMaterialParams {
                    // Every 6 m — i.e. NEAR_FIELD_MAX — gets a brighter tick.
                    base_color: if (z as i32) % 6 == 0 {
                        XrdsColor::srgb(0.75, 0.55, 0.2)
                    } else {
                        XrdsColor::srgb(0.22, 0.24, 0.27)
                    },
                    ..Default::default()
                },
            );
        }

        spawn_marker(
            api,
            "NearFieldMarker",
            [-SOURCE_X, EAR_HEIGHT, SOURCE_Z],
            XrdsColor::srgb(0.95, 0.15, 0.1),
            XrdsLinearRgba::rgb(0.6, 0.0, 0.0),
        );
        spawn_marker(
            api,
            "FarFieldMarker",
            [SOURCE_X, EAR_HEIGHT, SOURCE_Z],
            XrdsColor::srgb(0.1, 0.35, 0.95),
            XrdsLinearRgba::rgb(0.0, 0.0, 0.6),
        );

        self.camera = api.handle_of::<XrdsCamera>(XrdsId::from(CAMERA_ID));

        println!();
        println!("═══════════════════════════════════════════════════════════");
        println!(" Spatial audio falloff check");
        println!("───────────────────────────────────────────────────────────");
        println!(" Two clips. Same file, same volume, mirrored L/R, ALWAYS");
        println!(" equidistant from you. Only their authored curves differ:");
        println!();
        println!("   LEFT  (red)  NearField  max_distance = {NEAR_FIELD_MAX:.0} m");
        println!("   RIGHT (blue) FarField   max_distance = {FAR_FIELD_MAX:.0} m");
        println!();
        println!(" The camera reverses at {PULLBACK_SPEED} m/s, out to");
        println!(" {PULLBACK_LIMIT:.0} m and back, repeating. Listen for the LEFT");
        println!(" channel to drop out entirely at ~{NEAR_FIELD_MAX:.0} m (the");
        println!(" bright floor tick) while the RIGHT keeps playing — then");
        println!(" return on the way back in.");
        println!();
        println!(" Balanced stereo the whole way out = falloff NOT applied.");
        println!();
        println!(" Numbers behind the sound:");
        println!("   RUST_LOG=xrds_runtime=debug cargo run \\");
        println!("       --example spatial_audio_falloff_check");
        println!("═══════════════════════════════════════════════════════════");
        println!();
    }

    fn update(&mut self, ctx: &mut XrdsUpdateContext<'_>) {
        let t = ctx.elapsed_secs();

        // Two motions, alternating, because one cannot test both things.
        //
        // Pulling straight back tests DISTANCE but destroys any chance of hearing
        // direction: the sources' angular separation shrinks towards zero as you
        // retreat, and rodio's panning independently collapses with distance
        // (~22 dB between the ears at 3 m, ~1 dB at 10 m). Judging the stereo
        // image from the retreat leg is judging it where it cannot exist.
        //
        // So DIRECTION gets its own motion: a lateral pass at close range, where
        // rodio pans strongly, swinging both sources from one side of the head to
        // the other.
        let leg = PULLBACK_LIMIT / PULLBACK_SPEED;
        let cycle = t % (leg * 4.0);
        let in_distance_phase = cycle < leg * 2.0;

        let (position, phase_name) = if in_distance_phase {
            let p = (cycle / leg) % 2.0;
            let z = if p < 1.0 {
                p * PULLBACK_LIMIT
            } else {
                (2.0 - p) * PULLBACK_LIMIT
            };
            ([0.0, EAR_HEIGHT, z], "DISTANCE")
        } else {
            let p = ((cycle - leg * 2.0) / leg) % 2.0;
            // -STRAFE_LIMIT .. +STRAFE_LIMIT and back.
            let x = if p < 1.0 {
                -STRAFE_LIMIT + p * 2.0 * STRAFE_LIMIT
            } else {
                STRAFE_LIMIT - (p - 1.0) * 2.0 * STRAFE_LIMIT
            };
            ([x, EAR_HEIGHT, STRAFE_Z], "DIRECTION")
        };

        if let Some(ref cam) = self.camera {
            ctx.set_translation(cam, position);
        }

        if self.phase_name != phase_name {
            self.phase_name = phase_name;
            if in_distance_phase {
                println!(
                    "\n── DISTANCE ── pulling back to {PULLBACK_LIMIT:.0} m and returning. \
                     Listen for the LEFT source to cut out at {NEAR_FIELD_MAX:.0} m \
                     while the RIGHT keeps playing, then return."
                );
            } else {
                println!(
                    "\n── DIRECTION ── passing left-to-right at {STRAFE_Z:.0} m. Both \
                     sources should swing from one side of your head to the other. \
                     Panning is amplitude-only, so expect a stereo image, not \
                     placement in space."
                );
            }
        }

        if in_distance_phase && !self.announced_near_cutoff {
            let z = position[2];
            let distance = (z * z + SOURCE_X * SOURCE_X).sqrt();
            if distance >= NEAR_FIELD_MAX {
                self.announced_near_cutoff = true;
                println!(
                    "[t={t:.1}s] distance={distance:.2} m — past NearField's max_distance \
                     of {NEAR_FIELD_MAX:.0} m. The LEFT channel should now be SILENT; \
                     the RIGHT should still be playing."
                );
            }
        }
    }
}

fn spawn_marker(
    api: &mut XrdsAPI<'_>,
    name: &str,
    translation: [f32; 3],
    base: XrdsColor,
    emissive: XrdsLinearRgba,
) {
    let sphere = api.spawn(&{
        let mut s = XrdsSphere::new().with_name(name);
        s.radius = 0.25;
        s.transform.translation = translation;
        s
    });
    api.set_material_params(
        &sphere,
        XrdsMaterialParams {
            base_color: base,
            emissive,
            ..Default::default()
        },
    );
}

fn authored_scene_document() -> XrdsSceneDocument {
    // Identical but for `max_distance`. Built through one helper so the two
    // cannot drift apart in some other field and quietly invalidate the check.
    let clip = |max_distance: f32| XrdsSceneAudioClip {
        asset_id: TONE_ASSET_ID.to_string(),
        volume: 1.0,
        looped: true,
        spatial: true,
        autoplay: true,
        distance_model: XrdsAudioDistanceModel::Linear,
        min_distance: 1.0,
        max_distance,
        rolloff_factor: 1.0,
    };

    XrdsSceneDocument {
        metadata: XrdsSceneMetadata {
            name: "SpatialAudioFalloffCheck".to_string(),
            ..Default::default()
        },
        assets: vec![XrdsSceneAsset {
            id: TONE_ASSET_ID.to_string(),
            // Purpose-built for this check: 1 s loop, MONO, four short broadband
            // noise bursts with sharp onsets.
            //
            // The other clips in assets/sound/ are unusable here and it is worth
            // saying why, because reaching for one is the obvious move: they are
            // all STEREO and ~20 s long. rodio downmixes a spatial source to mono,
            // so a recording's own stereo image and movement — `transportation_1`
            // is a train passing, which pans by itself — fight the cue you are
            // trying to judge. A steady sound also gives the ear no onset to
            // localise. Broadband bursts fix both.
            uri: "sound/wav/spatial_test_ping.wav".to_string(),
            kind: XrdsSceneAssetKind::Audio,
        }],
        nodes: vec![
            XrdsSceneNode {
                id: ROOT_ID,
                parent_id: None,
                name: "FalloffCheckRoot".to_string(),
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
                id: CAMERA_ID,
                parent_id: Some(ROOT_ID),
                name: "MainCamera".to_string(),
                enabled: true,
                visible: true,
                transform: XrdsSceneTransform {
                    translation: [0.0, EAR_HEIGHT, 0.0],
                    ..Default::default()
                },
                payload: XrdsSceneNodePayload::Camera(XrdsSceneCamera {
                    projection: XrdsSceneCameraProjection::Perspective {
                        fov_deg: 60.0,
                        near: 0.1,
                        far: Some(200.0),
                        order: 0,
                    },
                    look_at: Some([0.0, EAR_HEIGHT, SOURCE_Z]),
                }),
                editor: XrdsEditorMetadata::default(),
                grabbable: false,
                triggers: Vec::new(),
                watchers: Vec::new(),
            },
            XrdsSceneNode {
                id: NEAR_FIELD_ID,
                parent_id: Some(ROOT_ID),
                name: "NearField".to_string(),
                enabled: true,
                visible: true,
                transform: XrdsSceneTransform {
                    translation: [-SOURCE_X, EAR_HEIGHT, SOURCE_Z],
                    ..Default::default()
                },
                payload: XrdsSceneNodePayload::AudioClip(clip(NEAR_FIELD_MAX)),
                editor: XrdsEditorMetadata::default(),
                grabbable: false,
                triggers: Vec::new(),
                watchers: Vec::new(),
            },
            XrdsSceneNode {
                id: FAR_FIELD_ID,
                parent_id: Some(ROOT_ID),
                name: "FarField".to_string(),
                enabled: true,
                visible: true,
                transform: XrdsSceneTransform {
                    translation: [SOURCE_X, EAR_HEIGHT, SOURCE_Z],
                    ..Default::default()
                },
                payload: XrdsSceneNodePayload::AudioClip(clip(FAR_FIELD_MAX)),
                editor: XrdsEditorMetadata::default(),
                grabbable: false,
                triggers: Vec::new(),
                watchers: Vec::new(),
            },
        ],
        ..Default::default()
    }
}
