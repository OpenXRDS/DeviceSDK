use super::*;
use xrds_components::{
    XrdsWorldToggle, XrdsWorldPointerState, XrdsWorldSurface,
    XrWorldToggleEvent, XrGrabHand,
};
use xrds_openxr::XrInput;

/// Runtime component that caches track and thumb entity handles for instant visual updates.
#[derive(Component)]
pub(super) struct XrdsWorldToggleParts {
    pub track: Entity,
    pub thumb: Entity,
}

/// Per-frame exclusive system that drives toggle press interaction.
///
/// A single trigger press while hovering within the toggle's bounds flips
/// [`XrdsWorldToggle::checked`], swaps the track colour, repositions the thumb,
/// and fires [`XrWorldToggleEvent`].
pub(super) fn world_ui_toggle_system(world: &mut World) {
    let Some(xr) = world.get_resource::<XrInput>().cloned() else { return; };
    let pointer_state = world.resource::<XrdsWorldPointerState>().clone();

    // Phase 1 — collect without keeping borrows alive.
    let toggles: Vec<(Entity, XrdsWorldToggle, Entity, Entity, Entity)> = {
        let mut q = world.query_filtered::<
            (Entity, &XrdsWorldToggle, &ChildOf, &XrdsWorldToggleParts),
            bevy::prelude::Without<xrds_components::XrdsWorldElementDisabled>,
        >();
        q.iter(world)
            .map(|(e, t, co, parts)| (e, t.clone(), co.0, parts.track, parts.thumb))
            .collect()
    };

    for (toggle_entity, toggle, panel_entity, track_entity, thumb_entity) in toggles {
        // Find a hand whose trigger just fired while hovering within the toggle bounds.
        let press_hand = [
            (XrGrabHand::Left,  pointer_state.left.as_ref(),  xr.left.select_just_pressed),
            (XrGrabHand::Right, pointer_state.right.as_ref(), xr.right.select_just_pressed),
        ]
        .into_iter()
        .find_map(|(hand, hit_opt, just_pressed)| {
            if !just_pressed { return None; }
            let hit = hit_opt?;
            if hit.entity != panel_entity { return None; }

            let panel_size = world.get::<XrdsWorldSurface>(panel_entity)?.size;
            let px = (hit.uv.x - 0.5) * panel_size.x;
            let py = (hit.uv.y - 0.5) * panel_size.y;

            let y_tol = toggle.size[1].max(0.03);
            let in_bounds =
                (px - toggle.local_position[0]).abs() <= toggle.size[0] * 0.5
                && (py - toggle.local_position[1]).abs() <= y_tol * 0.5;

            if in_bounds { Some(hand) } else { None }
        });

        let Some(hand) = press_hand else { continue; };

        let new_checked = !toggle.checked;
        apply_toggle_visuals(world, toggle_entity, track_entity, thumb_entity, &toggle, new_checked);
        world.write_message(XrWorldToggleEvent { toggle_entity, checked: new_checked, hand });
    }
}

/// Apply visual changes (track colour swap + thumb reposition) for a new `checked` state.
fn apply_toggle_visuals(
    world: &mut World,
    toggle_entity: Entity,
    track_entity:  Entity,
    thumb_entity:  Entity,
    toggle: &XrdsWorldToggle,
    new_checked: bool,
) {
    // Flip component.
    if let Ok(mut e) = world.get_entity_mut(toggle_entity) {
        if let Some(mut t) = e.get_mut::<XrdsWorldToggle>() {
            t.checked = new_checked;
        }
    }

    // Update track material colour in-place to avoid cross-module Handle type issues.
    let [r, g, b, a] = if new_checked { toggle.track_on_color } else { toggle.track_off_color };
    let track_mat_handle = world
        .get::<MeshMaterial3d<StandardMaterial>>(track_entity)
        .map(|m| m.0.clone());
    if let Some(handle) = track_mat_handle {
        if let Some(mat) = world.resource_mut::<Assets<StandardMaterial>>().get_mut(&handle) {
            mat.base_color = bevy::color::Color::srgba(r, g, b, a);
        }
    }

    // Reposition thumb.
    let travel = toggle.size[0] * 0.5 - toggle.size[1] * 0.85 * 0.5;
    let thumb_x = if new_checked { travel } else { -travel };
    if let Ok(mut e) = world.get_entity_mut(thumb_entity) {
        e.insert(Transform::from_xyz(thumb_x, 0.0, 0.001));
    }
}

/// Set toggle state programmatically, updating visuals without firing an event.
pub(super) fn set_toggle_in_world(world: &mut World, entity: Entity, checked: bool) {
    let data = world.get::<XrdsWorldToggle>(entity).cloned();
    let Some(toggle) = data else { return; };

    if toggle.checked == checked { return; }

    let parts = world.get::<XrdsWorldToggleParts>(entity)
        .map(|p| (p.track, p.thumb));
    let Some((track, thumb)) = parts else { return; };

    apply_toggle_visuals(world, entity, track, thumb, &toggle, checked);
}
