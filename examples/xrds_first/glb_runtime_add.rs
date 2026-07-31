//! Runtime GLB placement test.
//!
//! Launches a blank 3D scene (camera + directional light).
//! A UI button (or press L) places `buster_drone.glb` at runtime using the
//! XRDS SDK — no direct Bevy scene management.
//!
//! All entities (camera, light, and the GLB) go through a single
//! `XrdsSceneDocumentSession` so IDs never clash.

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use bevy::prelude::{
    default, App, BackgroundColor, Button, Changed, Color, Commands, FlexDirection, Interaction,
    Node, PositionType, Query, Res, Resource, Startup, Text, UiRect, Update, Val, With,
};
use xrds::scene_graph::{
    XrdsEditorMetadata, XrdsGltfAssetExportPolicy, XrdsSceneCamera, XrdsSceneCameraProjection,
    XrdsSceneDocument, XrdsSceneDocumentSession, XrdsSceneGltfPlacement, XrdsSceneNode,
    XrdsSceneNodeId, XrdsSceneNodePayload, XrdsScenePointLight, XrdsSceneTransform,
};
use xrds::sdk::{world::XrdsGltfAsset, XrdsId};
use xrds::{
    Handle, Runtime, RuntimeParameters, XrdsAPI, XrdsAnimationRepeatMode, XrdsApp,
    XrdsGltfAnimationPlaybackOptions, XrdsGltfAnimationSelector, XrdsGltfLoadStatus, XrdsKey,
    XrdsUpdateContext,
};

// ── Static node IDs for the initial scene ────────────────────────────────────

const ID_CAMERA: XrdsSceneNodeId = XrdsSceneNodeId(1);
const ID_LIGHT: XrdsSceneNodeId = XrdsSceneNodeId(2);

// ── Shared flags between Bevy button systems and XrdsApp::update ─────────────

#[derive(Resource, Clone)]
struct LoadFlag(Arc<AtomicBool>);

#[derive(Resource, Clone)]
struct DespawnFlag(Arc<AtomicBool>);

// ── UI (added via configure — the SDK-approved escape hatch for plugins) ─────

fn spawn_ui(mut commands: Commands) {
    commands
        .spawn(Node {
            position_type: PositionType::Absolute,
            top: Val::Px(12.0),
            left: Val::Px(12.0),
            flex_direction: FlexDirection::Row,
            ..default()
        })
        .with_children(|parent| {
            parent
                .spawn((
                    Button,
                    Node {
                        padding: UiRect::axes(Val::Px(20.0), Val::Px(10.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.18, 0.40, 0.80)),
                ))
                .with_child(Text::new("Load GLB  (L)"));

            parent
                .spawn((
                    Button,
                    Node {
                        padding: UiRect::axes(Val::Px(20.0), Val::Px(10.0)),
                        margin: UiRect::left(Val::Px(8.0)),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.70, 0.15, 0.15)),
                ))
                .with_child(Text::new("Despawn All  (D)"));
        });
}

fn button_system(
    interaction_query: Query<
        (&Interaction, &BackgroundColor),
        (Changed<Interaction>, With<Button>),
    >,
    load_flag: Res<LoadFlag>,
    despawn_flag: Res<DespawnFlag>,
) {
    for (interaction, bg) in &interaction_query {
        if *interaction != Interaction::Pressed {
            continue;
        }
        // Distinguish buttons by background color hue: blue = load, red = despawn
        let c = bg.0.to_linear();
        if c.red < c.blue {
            load_flag.0.store(true, Ordering::Relaxed);
        } else {
            despawn_flag.0.store(true, Ordering::Relaxed);
        }
    }
}

// ── App ───────────────────────────────────────────────────────────────────────

struct GlbRuntimeAddApp {
    load_flag: Arc<AtomicBool>,
    despawn_flag: Arc<AtomicBool>,
    session: XrdsSceneDocumentSession,
    placed: bool,
    placed_id: Option<XrdsId>,
    gltf_handle: Option<Handle<XrdsGltfAsset>>,
    animation_started: bool,
}

impl GlbRuntimeAddApp {
    fn new() -> Self {
        let doc = XrdsSceneDocument {
            nodes: vec![
                XrdsSceneNode {
                    id: ID_CAMERA,
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
                            fov_deg: 50.0,
                            near: 0.1,
                            far: Some(200.0),
                            order: 0,
                        },
                        look_at: Some([0.0, 0.0, 0.0]),
                    }),
                    editor: XrdsEditorMetadata::default(),
                    grabbable: false,
                    triggers: Vec::new(),
                },
                XrdsSceneNode {
                    id: ID_LIGHT,
                    parent_id: None,
                    name: "Sun".to_string(),
                    enabled: true,
                    visible: true,
                    transform: XrdsSceneTransform {
                        rotation_quat_xyzw: [-0.383, 0.0, 0.0, 0.924],
                        ..Default::default()
                    },
                    payload: XrdsSceneNodePayload::PointLight(XrdsScenePointLight {
                        intensity: 1_000_000.0,
                        range: 30.0,
                        shadows: true,
                        ..Default::default()
                    }),
                    editor: XrdsEditorMetadata::default(),
                    grabbable: false,
                    triggers: Vec::new(),
                },
            ],
            ..Default::default()
        };

        let session =
            XrdsSceneDocumentSession::new(doc).expect("initial scene document should be valid");
        let load_flag = Arc::new(AtomicBool::new(false));
        let despawn_flag = Arc::new(AtomicBool::new(false));
        Self {
            load_flag,
            despawn_flag,
            session,
            placed: false,
            placed_id: None,
            gltf_handle: None,
            animation_started: false,
        }
    }
}

impl XrdsApp for GlbRuntimeAddApp {
    fn configure(&mut self, app: &mut App) {
        app.insert_resource(LoadFlag(Arc::clone(&self.load_flag)))
            .insert_resource(DespawnFlag(Arc::clone(&self.despawn_flag)))
            .add_systems(Startup, spawn_ui)
            .add_systems(Update, button_system);
    }

    fn setup(&mut self, api: &mut XrdsAPI<'_>) {
        api.import_scene_document(self.session.document())
            .expect("initial scene import should succeed");
    }

    fn update(&mut self, ctx: &mut XrdsUpdateContext<'_>) {
        let l_key = ctx.key_just_pressed(XrdsKey::KeyL);
        let button = self.load_flag.swap(false, Ordering::Relaxed);

        if (l_key || button) && !self.placed {
            self.placed = true;

            // Register the GLB asset in the session catalog
            if let Err(e) = self
                .session
                .register_gltf_asset("phoenix_bird", "models/animated/buster_drone.glb")
            {
                println!("[glb_runtime_add] register failed: {e:?}");
                self.placed = false;
                return;
            }

            // Place a new GLB node in the document — ID is allocated above camera/light
            let node_id = match self.session.place_gltf_asset(XrdsSceneGltfPlacement {
                asset_id: "phoenix_bird".to_string(),
                node_id: None,
                parent_id: None,
                name: "PhoenixBird".to_string(),
                enabled: true,
                visible: true,
                transform: XrdsSceneTransform::default(),
                scene_index: 0,
                export_policy: XrdsGltfAssetExportPolicy::KeepExternalReference,
                editor: XrdsEditorMetadata::default(),
            }) {
                Ok(id) => id,
                Err(e) => {
                    println!("[glb_runtime_add] place failed: {e:?}");
                    self.placed = false;
                    return;
                }
            };

            println!("[glb_runtime_add] document node placed: {node_id:?}");

            // Incrementally spawn only the new node — no full reimport
            let xrds_id: XrdsId = node_id.into();
            match ctx.spawn_document_node(xrds_id, self.session.document()) {
                Ok(id) => {
                    println!("[glb_runtime_add] spawned into world: {id:?}");
                    self.placed_id = Some(xrds_id);
                    // Grab a typed handle so we can poll load state + start animation
                    self.gltf_handle = ctx.handle_of::<XrdsGltfAsset>(xrds_id);
                    self.animation_started = false;
                }
                Err(e) => {
                    println!("[glb_runtime_add] world spawn failed: {e:?}");
                    self.placed = false;
                }
            }
        }

        // ── Start animation once the GLB is fully loaded ──────────────────────
        if !self.animation_started {
            if let Some(handle) = &self.gltf_handle {
                let status = ctx.gltf_load_status(handle);
                if !matches!(status, Some(XrdsGltfLoadStatus::Loaded)) {
                    println!("[glb_runtime_add] load status: {status:?}");
                }
                if matches!(status, Some(XrdsGltfLoadStatus::Loaded)) {
                    match ctx.gltf_animations(handle) {
                        Ok(anims) if !anims.is_empty() => {
                            let selector = match &anims[0].name {
                                Some(name) => XrdsGltfAnimationSelector::Name(name.clone()),
                                None => XrdsGltfAnimationSelector::Index(0),
                            };
                            match ctx.play_gltf_animation(
                                handle,
                                selector,
                                XrdsGltfAnimationPlaybackOptions {
                                    repeat: XrdsAnimationRepeatMode::Loop,
                                    ..Default::default()
                                },
                            ) {
                                Ok(()) => {
                                    println!("[glb_runtime_add] animation started (looping).")
                                }
                                Err(e) => {
                                    println!("[glb_runtime_add] animation start failed: {e:?}")
                                }
                            }
                            self.animation_started = true;
                        }
                        Ok(_) => {
                            println!("[glb_runtime_add] no animations found.");
                            self.animation_started = true;
                        }
                        Err(e) => println!("[glb_runtime_add] gltf_animations error: {e:?}"),
                    }
                }
            }
        }

        // ── Despawn (animation still running) ─────────────────────────────────
        let d_key = ctx.key_just_pressed(XrdsKey::KeyR); // R = Remove
        let despawn = self.despawn_flag.swap(false, Ordering::Relaxed);

        if (d_key || despawn) && self.placed {
            let placed_scene_id = self.placed_id.map(xrds::scene_graph::XrdsSceneNodeId::from);
            self.session
                .edit(|doc| {
                    if let Some(scene_id) = placed_scene_id {
                        doc.nodes.retain(|n| n.id != scene_id);
                    }
                    doc.assets.retain(|a| a.id != "phoenix_bird");
                })
                .ok();

            match ctx.reimport_scene(self.session.document()) {
                Ok(_) => println!("[glb_runtime_add] despawned while animation was running."),
                Err(e) => println!("[glb_runtime_add] reimport failed: {e:?}"),
            }

            self.placed = false;
            self.placed_id = None;
            self.gltf_handle = None;
            self.animation_started = false;
        }
    }
}

fn main() {
    Runtime::new(RuntimeParameters {
        app_name: "GlbRuntimeAdd".to_owned(),
        asset_path: Some(
            std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("assets")
                .to_string_lossy()
                .into_owned(),
        ),
        ..Default::default()
    })
    .run_xrds(GlbRuntimeAddApp::new())
    .expect("failed to run glb_runtime_add");
}
