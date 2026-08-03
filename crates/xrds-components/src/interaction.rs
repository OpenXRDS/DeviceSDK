use bevy::math::{Quat, Vec3};
use bevy::prelude::Component;
use crate::XrdsId;

/// A single raycast hit returned by [`XrdsUpdateContext::raycast`].
#[derive(Debug, Clone)]
pub struct XrRayhit {
    /// XRDS id of the entity that was hit (the closest `XrdsId` ancestor of the Bevy mesh entity).
    pub id: XrdsId,
    /// Distance along the ray from the origin to the hit point, in metres.
    pub distance: f32,
    /// World-space point of intersection.
    pub point: Vec3,
}

/// Mark any XRDS entity as pick-up-able by the XR grab system.
///
/// Add via [`XrdsAPI::make_grabbable`] or [`XrdsUpdateContext::make_grabbable`].
/// The SDK grab system scans for entities with this marker when the player presses
/// the trigger within grab range.
#[derive(Component, Debug, Clone)]
pub struct XrGrabbable;

/// Inserted by the SDK while an entity is being held; removed on drop.
#[derive(Component, Debug, Clone)]
pub struct XrGrabbed {
    /// Which controller is holding this entity.
    pub hand: XrGrabHand,
    /// Entity position offset from the aim origin at grab time, in aim-orientation space.
    /// (`aim_rot.inverse() * (world_pos - aim_origin)`)
    pub offset: Vec3,
    /// Entity rotation delta at grab time: `aim_rot.inverse() * world_rot`.
    pub rotation_offset: Quat,
}

/// Which controller initiated or holds a grab.
///
/// `Serialize`/`Deserialize` so it can be stored as authored document data —
/// needed for the optional `hand` filter on `XrdsTriggerBinding`
/// (`xrds-scene-graph`), which lets an author require a specific hand for a
/// binding to fire.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum XrGrabHand {
    Left,
    Right,
}

/// Fired once when an entity transitions from free → held.
#[derive(Debug, Clone, bevy::prelude::Message)]
pub struct XrGrabEvent {
    pub id:   XrdsId,
    pub hand: XrGrabHand,
}

/// Fired once when an entity transitions from held → free.
#[derive(Debug, Clone, bevy::prelude::Message)]
pub struct XrDropEvent {
    pub id:   XrdsId,
    pub hand: XrGrabHand,
}

/// Physics simulation mode for XRDS primitive entities.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
pub enum XrdsPhysicsBody {
    /// No physics — purely kinematic, driven by authored Transform.
    #[default]
    None,
    /// Fixed in place; other bodies collide with it but it never moves.
    Static,
    /// Fully simulated: gravity, collisions, forces apply.
    Dynamic,
}

impl XrdsPhysicsBody {
    pub fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }
}

/// Shape of a trigger/interaction volume.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum XrdsInteractionZoneShape {
    Sphere { radius: f32 },
    Box { half_extents: [f32; 3] },
}

impl Default for XrdsInteractionZoneShape {
    fn default() -> Self { Self::Box { half_extents: [0.5, 0.5, 0.5] } }
}

/// What happens when a grabbed object is released inside an interaction zone.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, Default)]
pub enum XrdsGrabType {
    #[default]
    None,
    /// Released object snaps to the zone's origin.
    Snap,
    /// Released object keeps its position inside the zone.
    Free,
}

/// Runtime marker placed on every interaction-zone entity.
/// Carries the shape/behaviour data from the authored document.
///
/// `shape` alone already makes this a valid trigger-detection volume —
/// `zone_collision_system` fires `XrZoneEnterEvent`/`XrZoneExitEvent` off
/// `shape` independently of `grab_type`/`hoverable`. So `grab_type: None`,
/// `hoverable: false` is the normal, expected shape for a zone that's
/// meant only to be walked through (e.g. a teleport pad or damage zone),
/// not a sign something's misconfigured — this type covers both "can be
/// grabbed/hovered" and "fires enter/exit for trigger-action sequencing"
/// (see `docs/xrds-scenegraph-trigger-action-sequencing.md`), and most
/// zones only need one of the two.
#[derive(bevy::prelude::Component, Debug, Clone, Copy)]
pub struct XrdsInteractionZone {
    pub shape:     XrdsInteractionZoneShape,
    pub grab_type: XrdsGrabType,
    pub hoverable: bool,
}

/// Fired when any entity enters an interaction zone's sensor volume.
#[derive(Debug, Clone, bevy::prelude::Message)]
pub struct XrZoneEnterEvent {
    /// XRDS id of the zone that was entered.
    pub zone_id:   XrdsId,
    /// XRDS id of the entity that entered the zone.
    pub entity_id: XrdsId,
}

/// Fired when an entity exits an interaction zone's sensor volume.
#[derive(Debug, Clone, bevy::prelude::Message)]
pub struct XrZoneExitEvent {
    pub zone_id:   XrdsId,
    pub entity_id: XrdsId,
}

/// Marks an XRDS entity as a player spawn zone volume.
///
/// Inserted by the runtime importer for every `PlayerSpawnZone` document node.
/// `size` is the full box dimensions in metres [width, height, depth].
/// Use `XrdsAPI::random_spawn_zone_position()` to pick a spawn point.
#[derive(Component, Debug, Clone)]
pub struct XrdsPlayerSpawnZone {
    pub size: Vec3,
    /// Mirrors `XrdsScenePlayerSpawnZone::player_node_id`.
    /// `None` = shared zone; `Some(id)` = reserved for the Player node with that scene ID.
    pub player_node_id: Option<u64>,
}
