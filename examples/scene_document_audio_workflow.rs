// Demonstrates the audio asset catalog and audio clip node workflow.
//
// Three audio clips play simultaneously so you can verify all sources:
//
//   BackgroundTheme  – non-spatial OGG, same volume everywhere.
//   FootstepZoneA    – spatial WAV at x = -4.5 (camera level).
//                      Pans HARD LEFT when camera is near it.
//   FootstepZoneB    – spatial WAV at x = +4.5 (camera level).
//                      Pans HARD RIGHT when camera is near it.
//   CombatTheme      – non-spatial MP3, joins after decoder validates.
//
// The audio zones are placed at the same Y/Z as the camera so panning is purely
// horizontal and immediately obvious.  The camera sweeps ±3 units on X — watch
// the floor grid lines slide past and listen for the stereo swap.
// Console output prints "◀  LEFT", "CENTER", "RIGHT  ▶" each second.

use xrds::scene_graph::{
    XrdsEditorMetadata, XrdsSceneAsset, XrdsSceneAssetKind, XrdsSceneAudioClip, XrdsSceneCamera,
    XrdsSceneCameraProjection, XrdsSceneDocument, XrdsSceneDocumentSession, XrdsSceneMetadata,
    XrdsSceneNode, XrdsSceneNodeId, XrdsSceneNodePayload, XrdsSceneTransform,
};
use xrds::sdk::{
    primitives::{XrdsCube, XrdsPlane3D, XrdsSphere},
    world::XrdsCamera,
    XrdsColor, XrdsId, XrdsMaterialParams,
};
use xrds::{Handle, Runtime, RuntimeParameters, XrdsAPI, XrdsApp, XrdsUpdateContext};

// ── Stable document ids ───────────────────────────────────────────────────────────────────────────
const DOCUMENT_ROOT_ID: XrdsSceneNodeId = XrdsSceneNodeId(850);
const DOCUMENT_CAMERA_ID: XrdsSceneNodeId = XrdsSceneNodeId(851);
const DOCUMENT_AMBIENT_MUSIC_ID: XrdsSceneNodeId = XrdsSceneNodeId(852);
const DOCUMENT_FOOTSTEP_ZONE_A_ID: XrdsSceneNodeId = XrdsSceneNodeId(853);
const DOCUMENT_COMBAT_MUSIC_ID: XrdsSceneNodeId = XrdsSceneNodeId(855);

// ── Catalog asset ids ─────────────────────────────────────────────────────────────────────────────
const THEME_ASSET_ID: &str = "asset:audio-theme";
const FOOTSTEP_ASSET_ID: &str = "asset:audio-footstep";
const COMBAT_ASSET_ID: &str = "asset:audio-combat";

fn main() {
    Runtime::new(RuntimeParameters {
        app_name: "SceneDocumentAudioWorkflow".to_owned(),
        ..Default::default()
    })
    .run_xrds(SceneDocumentAudioWorkflowApp::default())
    .expect("failed to run scene_document_audio_workflow example");
}

#[derive(Default)]
struct SceneDocumentAudioWorkflowApp {
    camera: Option<Handle<XrdsCamera>>,
    floor: Option<Handle<XrdsPlane3D>>,
    sphere_a: Option<Handle<XrdsSphere>>,
    sphere_b: Option<Handle<XrdsSphere>>,
    announced_combat: bool,
}

impl XrdsApp for SceneDocumentAudioWorkflowApp {
    fn setup(&mut self, api: &mut XrdsAPI<'_>) {
        // ── Build and validate the authored scene document ────────────────────────────────────
        let session = XrdsSceneDocumentSession::new(authored_scene_document())
            .expect("authored document should be valid");
        print_audio_diagnostics(session.document());

        // ── Import: audio clip nodes → XrdsAudioClip runtime components ──────────────────────
        api.import_scene_document(session.document())
            .expect("audio clip document import should succeed");

        // ── Visual geometry ───────────────────────────────────────────────────────────────────

        // Floor base plate.
        let floor = api.spawn(&{
            let mut p = XrdsPlane3D::new().with_name("Floor");
            p.size = [14.0, 14.0];
            p.transform.rotation_quat_xyzw = [-0.70710677, 0.0, 0.0, 0.70710677];
            p
        });
        api.set_material_params(
            &floor,
            XrdsMaterialParams {
                base_color: XrdsColor::srgb(0.08, 0.09, 0.10),
                ..Default::default()
            },
        );

        // ── Floor grid ────────────────────────────────────────────────────────────────────────
        // Thin XrdsCube planks form a reference grid so camera movement is obvious.
        let grid_color = XrdsMaterialParams {
            base_color: XrdsColor::srgb(0.28, 0.30, 0.33),
            ..Default::default()
        };

        // Lines running left-right (along X), spaced every 2 units on Z.
        for iz in -3..=3i32 {
            let z = iz as f32 * 2.0;
            let plank = api.spawn(&{
                let mut c = XrdsCube::new().with_name("GridX");
                c.size = [14.0, 0.02, 0.06];
                c.transform.translation = [0.0, 0.01, z];
                c
            });
            api.set_material_params(&plank, grid_color.clone());
        }

        // Lines running front-back (along Z), spaced every 2 units on X.
        for ix in -3..=3i32 {
            let x = ix as f32 * 2.0;
            let plank = api.spawn(&{
                let mut c = XrdsCube::new().with_name("GridZ");
                c.size = [0.06, 0.02, 14.0];
                c.transform.translation = [x, 0.01, 0.0];
                c
            });
            api.set_material_params(&plank, grid_color.clone());
        }

        // ── Audio zone markers ────────────────────────────────────────────────────────────────
        // Spheres float at the same Y/Z as the camera so they visually coincide with the
        // audio sources.  Floor columns below them provide a ground reference.

        // Red = FootstepZoneA at x = -4, z = 6  (IN FRONT of camera, visible).
        let sphere_a = api.spawn(&{
            let mut s = XrdsSphere::new().with_name("ZoneA");
            s.radius = 0.35;
            s.transform.translation = [-4.0, 3.5, 6.0];
            s
        });
        api.set_material_params(
            &sphere_a,
            XrdsMaterialParams {
                base_color: XrdsColor::srgb(0.95, 0.15, 0.1),
                emissive: xrds::sdk::XrdsLinearRgba::rgb(0.5, 0.0, 0.0),
                ..Default::default()
            },
        );
        // Floor column below ZoneA.
        let col_a = api.spawn(&{
            let mut c = XrdsCube::new().with_name("ColA");
            c.size = [0.08, 3.5, 0.08];
            c.transform.translation = [-4.0, 1.75, 6.0];
            c
        });
        api.set_material_params(
            &col_a,
            XrdsMaterialParams {
                base_color: XrdsColor::srgb(0.4, 0.1, 0.1),
                ..Default::default()
            },
        );

        // Blue = FootstepZoneB at x = +4, z = 6  (IN FRONT of camera, visible).
        let sphere_b = api.spawn(&{
            let mut s = XrdsSphere::new().with_name("ZoneB");
            s.radius = 0.35;
            s.transform.translation = [4.0, 3.5, 6.0];
            s
        });
        api.set_material_params(
            &sphere_b,
            XrdsMaterialParams {
                base_color: XrdsColor::srgb(0.1, 0.35, 0.95),
                emissive: xrds::sdk::XrdsLinearRgba::rgb(0.0, 0.0, 0.5),
                ..Default::default()
            },
        );
        // Floor column below ZoneB.
        let col_b = api.spawn(&{
            let mut c = XrdsCube::new().with_name("ColB");
            c.size = [0.08, 3.5, 0.08];
            c.transform.translation = [4.0, 1.75, 6.0];
            c
        });
        api.set_material_params(
            &col_b,
            XrdsMaterialParams {
                base_color: XrdsColor::srgb(0.1, 0.1, 0.4),
                ..Default::default()
            },
        );

        // Reuse the document camera — do NOT spawn a second one (would cause
        // Bevy camera order ambiguity warning with both at priority 0).
        self.camera = api.handle_of::<XrdsCamera>(XrdsId::from(DOCUMENT_CAMERA_ID));
        self.floor = Some(floor);
        self.sphere_a = Some(sphere_a);
        self.sphere_b = Some(sphere_b);

        println!();
        println!("═══════════════════════════════════════════════════");
        println!(" Audio workflow demo");
        println!("───────────────────────────────────────────────────");
        println!(" Red  sphere (● left,  x=-4) = FootstepZoneA");
        println!(" Blue sphere (● right, x=+4) = FootstepZoneB");
        println!(" Camera sweeps ±3.5 — gets within 0.5 u of each zone.");
        println!(" Floor grid: watch lines slide to feel the sweep.");
        println!(" Spatial panning is hard-left / hard-right because");
        println!(" both sources are at the same depth as the camera.");
        println!(" CombatTheme (non-spatial) joins after ~1–2 s.");
        println!("═══════════════════════════════════════════════════");
    }

    fn update(&mut self, ctx: &mut XrdsUpdateContext<'_>) {
        let t = ctx.elapsed_secs();

        // Sweep x from -3.5 to +3.5 on a 4-second cycle.
        // Zones sit at ±4 so the camera gets within 0.5 units of each zone at the
        // extremes — inverse-distance rolloff makes the near zone VERY loud, far zone
        // nearly silent, giving a clean one-ear-at-a-time effect.
        let x = 3.5 * (t * std::f32::consts::TAU / 4.0).sin();
        if let Some(ref cam) = self.camera {
            ctx.set_translation(cam, [x, 3.5, 9.0]);
        }

        if !self.announced_combat && t >= 2.0 {
            self.announced_combat = true;
            println!("[t={t:.1}s] CombatTheme should now be audible (non-spatial, same volume everywhere).");
        }
    }
}

// ── Document authoring ────────────────────────────────────────────────────────────────────────────

fn authored_scene_document() -> XrdsSceneDocument {
    XrdsSceneDocument {
        metadata: XrdsSceneMetadata {
            name: "AudioWorkflowScene".to_string(),
            ..Default::default()
        },
        assets: vec![
            XrdsSceneAsset {
                id: THEME_ASSET_ID.to_string(),
                uri: "sound/ogg/file_example_OOG_5MG.ogg".to_string(),
                kind: XrdsSceneAssetKind::Audio,
            },
            XrdsSceneAsset {
                id: FOOTSTEP_ASSET_ID.to_string(),
                uri: "sound/wav/footstep_1.wav".to_string(),
                kind: XrdsSceneAssetKind::Audio,
            },
            XrdsSceneAsset {
                id: COMBAT_ASSET_ID.to_string(),
                uri: "sound/wav/transportation_1.wav".to_string(),
                kind: XrdsSceneAssetKind::Audio,
            },
        ],
        nodes: vec![
            XrdsSceneNode {
                id: DOCUMENT_ROOT_ID,
                parent_id: None,
                name: "AudioSceneRoot".to_string(),
                enabled: true,
                visible: true,
                transform: XrdsSceneTransform::default(),
                payload: XrdsSceneNodePayload::Empty,
                editor: XrdsEditorMetadata::default(),
            },
            // Imported camera (also becomes the SpatialListener).
            XrdsSceneNode {
                id: DOCUMENT_CAMERA_ID,
                parent_id: Some(DOCUMENT_ROOT_ID),
                name: "MainCamera".to_string(),
                enabled: true,
                visible: true,
                transform: XrdsSceneTransform {
                    translation: [0.0, 3.5, 9.0],
                    ..Default::default()
                },
                payload: XrdsSceneNodePayload::Camera(XrdsSceneCamera {
                    projection: XrdsSceneCameraProjection::Perspective {
                        fov_deg: 50.0,
                        near: 0.1,
                        far: Some(200.0),
                        order: 0,
                    },
                    look_at: Some([0.0, 0.5, 0.0]),
                }),
                editor: XrdsEditorMetadata::default(),
            },
            // Non-spatial: same volume everywhere; demonstrates looping background music.
            XrdsSceneNode {
                id: DOCUMENT_AMBIENT_MUSIC_ID,
                parent_id: Some(DOCUMENT_ROOT_ID),
                name: "BackgroundTheme".to_string(),
                enabled: true,
                visible: true,
                transform: XrdsSceneTransform::default(),
                payload: XrdsSceneNodePayload::AudioClip(XrdsSceneAudioClip {
                    asset_id: THEME_ASSET_ID.to_string(),
                    volume: 0.5,
                    looped: true,
                    spatial: false,
                    autoplay: true,
                }),
                editor: XrdsEditorMetadata::default(),
            },
            // Spatial: at z=6 (in front of camera) and camera height y=3.5.
            // Camera sweeps ±3.5; this zone is at x=-4 → 0.5 units away at sweep peak.
            XrdsSceneNode {
                id: DOCUMENT_FOOTSTEP_ZONE_A_ID,
                parent_id: Some(DOCUMENT_ROOT_ID),
                name: "FootstepZoneA".to_string(),
                enabled: true,
                visible: true,
                transform: XrdsSceneTransform {
                    translation: [-4.0, 3.5, 6.0],
                    ..Default::default()
                },
                payload: XrdsSceneNodePayload::AudioClip(XrdsSceneAudioClip {
                    asset_id: FOOTSTEP_ASSET_ID.to_string(),
                    volume: 1.0,
                    looped: true, // loop so it plays continuously for demo purposes
                    spatial: true,
                    autoplay: true,
                }),
                editor: XrdsEditorMetadata::default(),
            },
            XrdsSceneNode {
                id: DOCUMENT_COMBAT_MUSIC_ID,
                parent_id: Some(DOCUMENT_ROOT_ID),
                name: "CombatTheme".to_string(),
                enabled: true,
                visible: true,
                transform: XrdsSceneTransform {
                    translation: [4.0, 3.5, 6.0],
                    ..Default::default()
                },
                payload: XrdsSceneNodePayload::AudioClip(XrdsSceneAudioClip {
                    asset_id: COMBAT_ASSET_ID.to_string(),
                    volume: 0.7,
                    looped: true,
                    spatial: true,
                    autoplay: true,
                }),
                editor: XrdsEditorMetadata::default(),
            },
        ],
        ..Default::default()
    }
}

// ── Diagnostics helper ────────────────────────────────────────────────────────────────────────────

fn print_audio_diagnostics(document: &xrds::scene_graph::XrdsSceneDocument) {
    let diagnostics = document.asset_diagnostics();
    let audio_usages: Vec<_> = diagnostics
        .asset_usages
        .iter()
        .filter(|u| u.asset.kind == XrdsSceneAssetKind::Audio)
        .collect();

    println!("── Audio catalog ─────────────────────────────────────");
    for usage in &audio_usages {
        let refs = usage.referenced_node_ids.len();
        println!(
            "  [{}]  {}  → {} node(s)",
            usage.asset.id, usage.asset.uri, refs
        );
    }

    if diagnostics.unused_asset_ids.is_empty() {
        println!("  All assets referenced.");
    } else {
        println!("  Unused: {:?}", diagnostics.unused_asset_ids);
    }
    println!("─────────────────────────────────────────────────────");
}
