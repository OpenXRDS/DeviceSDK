// Spike: evaluate `bevy-sequential-actions` 0.14 (matches this workspace's
// `bevy = "0.17.2"`) as the execution substrate for xrds-scene-graph's
// planned trigger-action sequencing — see
// docs/xrds-scenegraph-trigger-action-sequencing.md for the design context
// and the two open questions this spike answers with running code instead
// of docs alone:
//
//   1. Does `SequentialActionsPlugin` + the `Action` trait coexist cleanly
//      with XRDS's `Runtime`/`RuntimeHandler` expert-layer setup?
//   2. What does tuple `.add((a, b))` actually do — run both actions
//      concurrently, or queue them to run one after another?
//
// Question 2 is answered both visually and via stdout timestamps: the
// left (red) cube only spins while its action is active, the right
// (blue) cube only spins while its is. If sequential, you'll see red spin
// alone, stop, then blue spin alone. If genuinely concurrent, both would
// spin at once.
//
// Not exercised here: routing a *real* xrds-runtime action (e.g.
// `play_gltf_animation_in_world`) through this queue. That free function
// already takes `&mut World`/`&World` directly (see
// crates/xrds-runtime/src/xrds_api/api.rs), which is exactly the
// signature `Action::on_start`/`is_finished` receive below — so wiring it
// in later is a drop-in swap, not a new adapter layer.
//
// The app exits itself once the queue drains (see `PrintAndExit` below).
use bevy::prelude::*;
use bevy_sequential_actions::*;
use xrds::*;

struct Handler;

pub fn main() {
    let runtime = Runtime::new(RuntimeParameters {
        app_name: "SequentialActionsSpike".to_owned(),
        ..Default::default()
    });
    runtime.run(Handler).expect("Could not run application");
}

impl RuntimeHandler for Handler {
    fn on_construct(&mut self, mut on_construct: OnConstruct) {
        on_construct.app_mut().add_plugins(SequentialActionsPlugin);
        on_construct.app_mut().add_systems(Update, spin_active_cubes);
        on_construct.add_systems(setup);
    }
}

/// Marker: entities with this component spin; added on `on_start`, removed
/// on `on_stop` — the visible proxy for "this action is currently active."
#[derive(Component)]
struct Spinning;

fn spin_active_cubes(time: Res<Time>, mut cubes: Query<&mut Transform, With<Spinning>>) {
    for mut transform in &mut cubes {
        transform.rotate_y(time.delta_secs() * 3.0);
    }
}

fn setup(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    commands.spawn((
        Camera3d::default(),
        Transform::from_xyz(0.0, 2.5, 7.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    commands.spawn((
        PointLight {
            intensity: 2_000_000.0,
            shadows_enabled: true,
            ..default()
        },
        Transform::from_xyz(0.0, 5.0, 4.0),
    ));
    commands.insert_resource(AmbientLight {
        brightness: 300.0,
        ..default()
    });

    let cube_mesh = meshes.add(Mesh::from(Cuboid::new(1.5, 1.5, 1.5)));

    let cube_a = commands
        .spawn((
            Mesh3d(cube_mesh.clone()),
            MeshMaterial3d(materials.add(Color::srgb(0.9, 0.2, 0.2))),
            Transform::from_xyz(-2.0, 0.75, 0.0),
        ))
        .id();
    let cube_b = commands
        .spawn((
            Mesh3d(cube_mesh),
            MeshMaterial3d(materials.add(Color::srgb(0.2, 0.4, 0.9))),
            Transform::from_xyz(2.0, 0.75, 0.0),
        ))
        .id();

    let agent = commands.spawn(SequentialActions).id();
    commands
        .actions(agent)
        .add(PrintAction::new("queue started"))
        .add((
            TimedAction::new("A (red cube spins)", 2.0, Some(cube_a)),
            TimedAction::new("B (blue cube spins)", 2.0, Some(cube_b)),
        ))
        .add(PrintAndExit::new(
            "done — if red and blue never spun at the same time, tuple add is sequential",
        ));
}

/// Finishes on the same frame it starts — just logs a timestamped message.
struct PrintAction {
    message: &'static str,
}

impl PrintAction {
    fn new(message: &'static str) -> Self {
        Self { message }
    }
}

impl Action for PrintAction {
    fn is_finished(&self, _agent: Entity, _world: &World) -> bool {
        true
    }

    fn on_start(&mut self, _agent: Entity, world: &mut World) -> bool {
        let now = world.resource::<Time>().elapsed_secs();
        println!("[t={now:.2}s] {}", self.message);
        true
    }

    fn on_stop(&mut self, _agent: Option<Entity>, _world: &mut World, _reason: StopReason) {}
}

/// Same as `PrintAction`, but also requests app exit — used as the final
/// step so this example terminates on its own once the queue drains.
struct PrintAndExit {
    message: &'static str,
}

impl PrintAndExit {
    fn new(message: &'static str) -> Self {
        Self { message }
    }
}

impl Action for PrintAndExit {
    fn is_finished(&self, _agent: Entity, _world: &World) -> bool {
        true
    }

    fn on_start(&mut self, _agent: Entity, world: &mut World) -> bool {
        let now = world.resource::<Time>().elapsed_secs();
        println!("[t={now:.2}s] {}", self.message);
        world.write_message(AppExit::Success);
        true
    }

    fn on_stop(&mut self, _agent: Option<Entity>, _world: &mut World, _reason: StopReason) {}
}

/// Records a deadline on `on_start` (computed from the live `Time`
/// resource) and polls it in `is_finished` — `is_finished` only gets
/// `&self`, so it can't tick a `Timer` itself; the deadline pattern is
/// the workaround. While active, spins `spin_entity` (if given) via the
/// `Spinning` marker — the visible proxy for "this action is running now."
struct TimedAction {
    label: &'static str,
    duration_secs: f32,
    deadline_secs: f32,
    spin_entity: Option<Entity>,
}

impl TimedAction {
    fn new(label: &'static str, duration_secs: f32, spin_entity: Option<Entity>) -> Self {
        Self {
            label,
            duration_secs,
            deadline_secs: 0.0,
            spin_entity,
        }
    }
}

impl Action for TimedAction {
    fn is_finished(&self, _agent: Entity, world: &World) -> bool {
        world.resource::<Time>().elapsed_secs() >= self.deadline_secs
    }

    fn on_start(&mut self, _agent: Entity, world: &mut World) -> bool {
        let now = world.resource::<Time>().elapsed_secs();
        self.deadline_secs = now + self.duration_secs;
        println!(
            "[t={now:.2}s] TimedAction({}) started, {}s duration",
            self.label, self.duration_secs
        );
        if let Some(entity) = self.spin_entity {
            if let Ok(mut entity_mut) = world.get_entity_mut(entity) {
                entity_mut.insert(Spinning);
            }
        }
        self.duration_secs <= 0.0
    }

    fn on_stop(&mut self, _agent: Option<Entity>, world: &mut World, _reason: StopReason) {
        if let Some(entity) = self.spin_entity {
            if let Ok(mut entity_mut) = world.get_entity_mut(entity) {
                entity_mut.remove::<Spinning>();
            }
        }
    }
}
