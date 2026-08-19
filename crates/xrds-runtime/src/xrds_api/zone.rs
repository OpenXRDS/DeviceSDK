use super::*;
use avian3d::prelude::{CollisionEnd, CollisionStart};
use bevy::prelude::*;
use xrds_components::{XrdsInteractionZone, XrZoneEnterEvent, XrZoneExitEvent};

/// Which side of a collision pair is the zone, if either.
///
/// Returns `(zone_entity, other_entity)`. Extracted because the enter and exit
/// loops resolved this inline in four near-identical blocks, and a difference
/// between two of them would have been invisible.
fn zone_side(
    zones: &Query<(), With<XrdsInteractionZone>>,
    e1: Entity,
    e2: Entity,
) -> Option<(Entity, Entity)> {
    if zones.contains(e1) {
        Some((e1, e2))
    } else if zones.contains(e2) {
        Some((e2, e1))
    } else {
        None
    }
}

/// Reports a collision that reached a zone but could not be turned into an event.
///
/// **Deliberately not `warn!` for the recurring case.** An unregistered entity
/// overlapping a zone is legitimate — scenery, debris, anything spawned outside the
/// authored-node path — so warning on every one would train people to ignore it.
///
/// But the *first* occurrence is logged at `info!`, once, because this is exactly
/// the failure that cost two device sessions: the collision happened, the id was
/// missing, and nothing anywhere said so. `debug!` alone would not have helped —
/// the deployed runtime runs at `Level::INFO` (`runtime.rs:178`), so a `debug!`
/// line is invisible on a headset, which is precisely where this needs to be read.
/// One `info!` breadcrumb costs nothing and turns "nothing happened" into "the id
/// was missing"; everything after it stays at `debug!` so a busy scene cannot spam.
fn report_unresolved(
    kind: &str,
    zone_entity: Entity,
    other_entity: Entity,
    zone_id: Option<XrdsId>,
    entity_id: Option<XrdsId>,
    already_reported: &mut bool,
) {
    let missing = match (zone_id, entity_id) {
        (None, None) => "both the zone and the other entity are unregistered",
        (None, Some(_)) => "the ZONE is unregistered",
        (Some(_), None) => "the other entity is unregistered",
        (Some(_), Some(_)) => return, // resolved; nothing to report
    };

    if !*already_reported {
        *already_reported = true;
        info!(
            "[zone] {kind} dropped: {missing}. zone={zone_entity} other={other_entity}. \
             The collision DID occur — this is an id-registration gap, not a mis-placed \
             volume. `XrdsIdIndex::register` runs on the authored-node spawn path; an \
             entity spawned by host-app code needs registering too. Further drops log at \
             debug only."
        );
    } else {
        debug!("[zone] {kind} dropped: {missing}. zone={zone_entity} other={other_entity}");
    }
}

/// Translates avian3d sensor collision events into XRDS zone enter/exit messages.
///
/// Reads `CollisionStart`/`CollisionEnd` for any entity that carries `XrdsInteractionZone`
/// and emits `XrZoneEnterEvent`/`XrZoneExitEvent` with stable XRDS ids.
///
/// Both the emitted events and the dropped ones are logged. Zone enter/exit are
/// discrete and infrequent — a player walking into a volume — so `info!` is
/// affordable and matches the existing `GRABBED` precedent, and it means a device
/// pass can tell "no collision occurred" from "collision occurred, id unresolved"
/// from the log alone, without a rebuild. See `docs/quest-device-test-recipe.md`.
pub(super) fn zone_collision_system(
    mut starts: MessageReader<CollisionStart>,
    mut ends:   MessageReader<CollisionEnd>,
    zones:      Query<(), With<XrdsInteractionZone>>,
    id_index:   Res<XrdsIdIndex>,
    mut enter:  MessageWriter<XrZoneEnterEvent>,
    mut exit:   MessageWriter<XrZoneExitEvent>,
    mut reported_unresolved: Local<bool>,
) {
    for event in starts.read() {
        let Some((zone_entity, other_entity)) =
            zone_side(&zones, event.collider1, event.collider2)
        else {
            continue;
        };

        match (id_index.id_of(zone_entity), id_index.id_of(other_entity)) {
            (Some(zone_id), Some(entity_id)) => {
                info!("[zone] ENTER zone={zone_id:?} entity={entity_id:?}");
                enter.write(XrZoneEnterEvent { zone_id, entity_id });
            }
            (zone_id, entity_id) => report_unresolved(
                "ENTER",
                zone_entity,
                other_entity,
                zone_id,
                entity_id,
                &mut reported_unresolved,
            ),
        }
    }

    for event in ends.read() {
        let Some((zone_entity, other_entity)) =
            zone_side(&zones, event.collider1, event.collider2)
        else {
            continue;
        };

        match (id_index.id_of(zone_entity), id_index.id_of(other_entity)) {
            (Some(zone_id), Some(entity_id)) => {
                info!("[zone] EXIT  zone={zone_id:?} entity={entity_id:?}");
                exit.write(XrZoneExitEvent { zone_id, entity_id });
            }
            (zone_id, entity_id) => report_unresolved(
                "EXIT",
                zone_entity,
                other_entity,
                zone_id,
                entity_id,
                &mut reported_unresolved,
            ),
        }
    }
}
