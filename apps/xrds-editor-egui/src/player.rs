use xrds::editor::bevy_ecs;
use xrds::scene_graph::XrdsPlayerLocomotionMode;

/// Marks the player pawn entity spawned during play mode.
/// The pawn entity also carries `Camera3d` — it IS the player camera.
/// Used to exclude the pawn from editor-only decoration systems
/// (camera icon attach, deactivate_scene_cameras, etc.).
#[derive(bevy_ecs::component::Component)]
pub struct PlayerPawnMarker;

/// Locomotion mode carried on the pawn so the locomotion system can branch
/// without re-querying the scene document every frame.
#[derive(bevy_ecs::component::Component)]
pub struct PawnLocomotionMode(pub XrdsPlayerLocomotionMode);

/// Marks UI nodes that belong to the play-mode HUD so they can be despawned on stop.
#[derive(bevy_ecs::component::Component)]
pub struct PlayHudMarker;

/// Kinematic vertical state — used for grounded (Smooth / Teleport) locomotion.
/// Gravity and jump are computed against `ground_y`, which is the pawn's y
/// coordinate when standing on the spawn-point ground level.
#[derive(bevy_ecs::component::Component)]
pub struct PawnVerticalState {
    /// Vertical velocity in m/s (positive = upward).
    pub velocity: f32,
    pub is_grounded: bool,
    /// Pawn y when standing flat on the ground (spawn y, i.e. eye height above ground plane).
    pub ground_y: f32,
}
