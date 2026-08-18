//! The player's physics body.
//!
//! Exists because an `XrdsInteractionZone` could not detect the player at all:
//! `zone_collision_system` translates avian3d collision events, and a collision needs
//! colliders on *both* bodies. Zones get one; nothing gave the player one, so walking
//! into a zone did nothing. Confirmed on a Quest 3 — a correctly placed, visibly marked
//! zone produced zero events.
//!
//! See `docs/done/player-body-collider-plan.md` for the decisions behind the shape below.

use super::anchor::XrdsPlayerCamera;
use super::state::XrdsIdIndex;
use avian3d::prelude::{Collider, CollisionEventsEnabled, RigidBody};
use bevy::prelude::*;
use xrds_components::XrdsId;

/// The XRDS id reserved for the player's body.
///
/// `XrdsIdAllocator` starts at 1, so 0 can never collide with an authored node. This
/// also gives triggers a stable way to say "the player did this":
/// `XrZoneEnterEvent::entity_id` previously only ever held authored node ids, so
/// anything resolving that field to a document node must tolerate this one resolving to
/// nothing.
pub const XRDS_PLAYER_ID: XrdsId = XrdsId(0);

/// Shape of the player's physics body. See [`XrdsPlayerBodyConfig`] to enable it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct XrdsPlayerBody {
    /// Total standing height in metres, tip to toe.
    pub height: f32,
    /// Capsule radius in metres — roughly half a shoulder width.
    pub radius: f32,
}

impl Default for XrdsPlayerBody {
    fn default() -> Self {
        Self {
            height: 1.7,
            radius: 0.25,
        }
    }
}

/// Whether the player has a physics body, and its shape.
///
/// `None` is the observer/spectator mode — a camera that moves through the world
/// without touching it. This mirrors how other engines model it: Unreal ships
/// `ACharacter` (capsule) and `ASpectatorPawn` (collision off) as separate classes, and
/// Unity's `CharacterController` is a component you add rather than something a camera
/// always carries.
///
/// Defaults to `Some`, because an author who places an `InteractionZone` expects walking
/// into it to fire, and silence is the worst possible outcome. Set from
/// `RuntimeParameters::player_body`; that insert happens before `install_xrds`, so it
/// takes precedence over this default.
#[derive(Resource, Debug, Clone, Copy, PartialEq)]
pub struct XrdsPlayerBodyConfig(pub Option<XrdsPlayerBody>);

impl Default for XrdsPlayerBodyConfig {
    fn default() -> Self {
        Self(Some(XrdsPlayerBody::default()))
    }
}

/// Marks the child entity holding the player's collider, and names the camera it
/// belongs to so the detach pass can find it without a hierarchy walk.
///
/// The collider lives on a **child** rather than on the camera itself because the
/// camera transform sits at *eye* height while the capsule must stand on the floor —
/// a child carries that offset natively, and avian3d attributes a child collider to the
/// parent rigid body via `ColliderOf`.
#[derive(Component, Debug, Clone, Copy)]
pub struct XrdsPlayerBodyCollider {
    pub camera: Entity,
}

/// Give any newly-marked `XrdsPlayerCamera` a body.
///
/// Keyed on the marker rather than done at spawn time because **the SDK does not own
/// the player entity** — `xrds-app` spawns its own (`spawn_app_camera`) and the editor
/// spawns another (`viewport_camera.rs`). The marker is the only thing they share.
pub(super) fn attach_player_body_system(
    mut commands: Commands,
    config: Res<XrdsPlayerBodyConfig>,
    cameras: Query<(Entity, &Transform), Added<XrdsPlayerCamera>>,
    existing: Query<&XrdsPlayerBodyCollider>,
    mut index: ResMut<XrdsIdIndex>,
) {
    let Some(body) = config.0 else {
        return;
    };
    if cameras.is_empty() {
        return;
    }

    // A second body would double every zone event, so refuse to build one while any
    // still exists.
    //
    // Load-bearing, not defensive: the editor's `sync_stereo_cameras` re-inserts
    // `XrdsPlayerCamera` on the *same* entity every frame while stereo preview is on, and
    // `Added` can also fire for a new camera in the same frame an old one is torn down.
    // Without this guard either shape stacks bodies.
    if !existing.is_empty() {
        return;
    }

    let height = body.height.max(0.01);
    // Clamp the radius so the cylinder section can never go negative; a radius at or
    // above half the height degenerates the capsule into a sphere.
    let radius = body.radius.clamp(0.01, height * 0.5);
    let length = (height - radius * 2.0).max(0.0);

    for (camera, camera_tf) in cameras.iter() {
        // The camera sits at eye height, so drop the capsule until its base reaches the
        // floor. Derived from the actual transform rather than assuming 1.6 m, so a
        // different eye height still lands correctly.
        let eye_height = camera_tf.translation.y;
        let offset_y = height * 0.5 - eye_height;

        // Kinematic, deliberately: locomotion writes the transform directly, so the
        // player pushes dynamic props but is *not* stopped by static geometry. Dynamic
        // would fight those writes and jitter. This is not wall collision — blocking
        // movement needs a locomotion shapecast, which is a separate feature.
        commands.entity(camera).insert(RigidBody::Kinematic);

        let collider = commands
            .spawn((
                Name::new("XrdsPlayerBody"),
                Collider::capsule(radius, length),
                CollisionEventsEnabled,
                Transform::from_xyz(0.0, offset_y, 0.0),
                XrdsPlayerBodyCollider { camera },
            ))
            .id();
        commands.entity(camera).add_child(collider);

        // Without this the collider is useless: `zone_collision_system` needs an
        // `XrdsId` for *both* entities and drops the event otherwise.
        index.register(XRDS_PLAYER_ID, collider);
    }
}

/// Tear the body down when its camera stops being the player camera.
///
/// Load-bearing for the editor: `sync_stereo_cameras` *removes* `XrdsPlayerCamera`
/// when stereo preview turns off (`viewport_camera.rs:315`) and inserts it back on the
/// same entity when it turns on. Without this the body outlives its marker and keeps
/// firing zone events from wherever that camera sits — phantom triggers rather than an
/// obvious leak.
pub(super) fn detach_player_body_system(
    mut commands: Commands,
    mut removed: RemovedComponents<XrdsPlayerCamera>,
    colliders: Query<(Entity, &XrdsPlayerBodyCollider)>,
    mut index: ResMut<XrdsIdIndex>,
) {
    for camera in removed.read() {
        for (collider, owner) in colliders.iter() {
            if owner.camera != camera {
                continue;
            }
            index.unregister(collider);
            commands.entity(collider).despawn();
            // Gate on the entity still *existing*, not on it still carrying the marker —
            // it just lost the marker, which is why we are here. `RemovedComponents` also
            // fires when the whole entity is despawned, and touching a dead entity warns.
            if let Ok(mut camera_cmds) = commands.get_entity(camera) {
                camera_cmds.remove::<RigidBody>();
            }
        }
    }
}
