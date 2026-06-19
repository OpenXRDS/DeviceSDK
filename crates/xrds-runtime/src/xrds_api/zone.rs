use super::*;
use avian3d::prelude::{CollisionEnd, CollisionStart};
use bevy::prelude::*;
use xrds_components::{XrdsInteractionZone, XrZoneEnterEvent, XrZoneExitEvent};

/// Translates avian3d sensor collision events into XRDS zone enter/exit messages.
///
/// Reads `CollisionStart`/`CollisionEnd` for any entity that carries `XrdsInteractionZone`
/// and emits `XrZoneEnterEvent`/`XrZoneExitEvent` with stable XRDS ids.
pub(super) fn zone_collision_system(
    mut starts: MessageReader<CollisionStart>,
    mut ends:   MessageReader<CollisionEnd>,
    zones:      Query<(), With<XrdsInteractionZone>>,
    id_index:   Res<XrdsIdIndex>,
    mut enter:  MessageWriter<XrZoneEnterEvent>,
    mut exit:   MessageWriter<XrZoneExitEvent>,
) {
    for event in starts.read() {
        let (e1, e2) = (event.collider1, event.collider2);
        if zones.contains(e1) {
            if let (Some(zone_id), Some(entity_id)) =
                (id_index.id_of(e1), id_index.id_of(e2))
            {
                enter.write(XrZoneEnterEvent { zone_id, entity_id });
            }
        } else if zones.contains(e2) {
            if let (Some(zone_id), Some(entity_id)) =
                (id_index.id_of(e2), id_index.id_of(e1))
            {
                enter.write(XrZoneEnterEvent { zone_id, entity_id });
            }
        }
    }

    for event in ends.read() {
        let (e1, e2) = (event.collider1, event.collider2);
        if zones.contains(e1) {
            if let (Some(zone_id), Some(entity_id)) =
                (id_index.id_of(e1), id_index.id_of(e2))
            {
                exit.write(XrZoneExitEvent { zone_id, entity_id });
            }
        } else if zones.contains(e2) {
            if let (Some(zone_id), Some(entity_id)) =
                (id_index.id_of(e2), id_index.id_of(e1))
            {
                exit.write(XrZoneExitEvent { zone_id, entity_id });
            }
        }
    }
}
