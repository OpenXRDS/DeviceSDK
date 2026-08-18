/// Tests GLB files in assets/models/animated/ directly through Bevy's
/// GltfLoader pipeline and reports their load state.  Valid models are displayed
/// in a 3-D window.
///
/// Run with:
///   cargo run --example gltf_samples_check
///
/// Expected output:
///   buster_drone.glb              → Loaded (complex skeletal animation)
///   phoenix_bird.glb              → Loaded (single animated mesh)
use bevy::asset::LoadState;
use bevy::gltf::Gltf;
use bevy::prelude::*;
use bevy::scene::SceneInstanceReady;

const SAMPLES_DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/models/animated");

const FILES: &[&str] = &[
    "buster_drone.glb",
    "phoenix_bird.glb",
];

fn main() {
    let samples_dir = SAMPLES_DIR.replace('\\', "/");
    println!("=== XRDS GLB sample availability check (Bevy pipeline) ===");
    println!("Asset root : {samples_dir}");
    println!();

    App::new()
        .add_plugins(
            DefaultPlugins
                .set(AssetPlugin {
                    file_path: samples_dir,
                    ..default()
                })
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "gltf_samples_check".to_string(),
                        ..default()
                    }),
                    ..default()
                }),
        )
        .add_systems(Startup, (setup_camera, load_samples))
        .add_systems(Update, (report_load_status, spawn_loaded_scenes))
        .run();
}

// ---------------------------------------------------------------------------
// Resources / components
// ---------------------------------------------------------------------------

#[derive(Resource)]
struct SampleHandles {
    entries: Vec<SampleEntry>,
    reported: Vec<&'static str>,
}

struct SampleEntry {
    filename: &'static str,
    gltf_handle: Handle<Gltf>,
    scene_handle: Handle<Scene>,
    /// First animation clip in this GLB (used to build the AnimationGraph).
    clip_handle: Handle<AnimationClip>,
    spawned: Option<Entity>,
}

/// Attached to a `SceneRoot` entity so the `SceneInstanceReady` observer
/// knows which animation graph + node index to start.
#[derive(Component)]
struct AnimationToPlay {
    graph_handle: Handle<AnimationGraph>,
    node_index: AnimationNodeIndex,
}

// ---------------------------------------------------------------------------
// Systems
// ---------------------------------------------------------------------------

fn setup_camera(mut commands: Commands) {
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 1.5, 5.0).looking_at(Vec3::new(0.0, 0.5, 0.0), Vec3::Y),
    ));
    commands.spawn((
        DirectionalLight {
            illuminance: 10_000.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.8, 0.5, 0.0)),
    ));
}

fn load_samples(mut commands: Commands, asset_server: Res<AssetServer>) {
    let entries = FILES
        .iter()
        .enumerate()
        .map(|(_i, &filename)| {
            let gltf_handle: Handle<Gltf> = asset_server.load(filename.to_string());
            let scene_handle: Handle<Scene> =
                asset_server.load(GltfAssetLabel::Scene(0).from_asset(filename));
            let clip_handle: Handle<AnimationClip> =
                asset_server.load(GltfAssetLabel::Animation(0).from_asset(filename));

            println!("[queued] {filename}");
            SampleEntry {
                filename,
                gltf_handle,
                scene_handle,
                clip_handle,
                spawned: None,
            }
        })
        .collect();

    commands.insert_resource(SampleHandles {
        entries,
        reported: Vec::new(),
    });
}

fn report_load_status(asset_server: Res<AssetServer>, mut samples: ResMut<SampleHandles>) {
    let mut newly_reported: Vec<&'static str> = Vec::new();

    for entry in &samples.entries {
        let ls = asset_server.load_state(entry.gltf_handle.id());
        let is_terminal = matches!(ls, LoadState::Loaded | LoadState::Failed(_));

        if is_terminal && !samples.reported.contains(&entry.filename) {
            match ls {
                LoadState::Loaded => {
                    let rdls = asset_server.recursive_dependency_load_state(entry.gltf_handle.id());
                    println!("[loaded  ✓] {}  (deps: {rdls:?})", entry.filename);
                }
                LoadState::Failed(ref err) => {
                    println!("[failed  ✗] {}  error: {err}", entry.filename);
                }
                _ => {}
            }
            newly_reported.push(entry.filename);
        }
    }

    samples.reported.extend(newly_reported);
}

fn spawn_loaded_scenes(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
    mut samples: ResMut<SampleHandles>,
) {
    for (i, entry) in samples.entries.iter_mut().enumerate() {
        if entry.spawned.is_some() {
            continue;
        }

        if !matches!(
            asset_server.load_state(entry.scene_handle.id()),
            LoadState::Loaded
        ) {
            continue;
        }

        // Build an AnimationGraph for the first clip so we can play it once
        // the SceneInstanceReady observer fires.
        let (graph, node_index) = AnimationGraph::from_clip(entry.clip_handle.clone());
        let graph_handle = graphs.add(graph);

        let x = i as f32 * 3.5;
        let entity = commands
            .spawn((
                SceneRoot(entry.scene_handle.clone()),
                Transform::from_xyz(x, 0.0, 0.0),
                Visibility::default(),
                InheritedVisibility::default(),
                ViewVisibility::default(),
                AnimationToPlay {
                    graph_handle,
                    node_index,
                },
            ))
            .observe(play_animation_when_ready)
            .id();

        entry.spawned = Some(entity);
        println!("[spawned] {} at x={x}", entry.filename);
    }
}

/// Observer: fires on the scene root entity once `SceneInstanceReady` is triggered.
///
/// Walks all descendants, finds every `AnimationPlayer`, and starts looping
/// the animation stored in `AnimationToPlay`.
fn play_animation_when_ready(
    trigger: On<SceneInstanceReady>,
    mut commands: Commands,
    children: Query<&Children>,
    to_play: Query<&AnimationToPlay>,
    mut players: Query<&mut AnimationPlayer>,
) {
    let root = trigger.entity;
    let Ok(anim) = to_play.get(root) else {
        return;
    };

    for descendant in children.iter_descendants(root) {
        let Ok(mut player) = players.get_mut(descendant) else {
            continue;
        };
        player.play(anim.node_index).repeat();
        commands
            .entity(descendant)
            .insert(AnimationGraphHandle(anim.graph_handle.clone()));
        println!("[playing] animation started on entity {descendant:?}");
    }
}
