use super::*;
use xrds_components::{
    XrdsWorldButton, XrdsWorldButtonState, XrdsWorldPointerState, XrdsWorldSurface,
    XrWorldButtonPressEvent, XrWorldButtonReleaseEvent, XrGrabHand,
};
use super::spawn::XrdsWorldButtonMaterials;
use xrds_openxr::XrInput;

/// Per-frame exclusive system that:
///
/// 1. Reads the current world-UI pointer hits from [`XrdsWorldPointerState`].
/// 2. For each [`XrdsWorldButton`], checks whether the pointer UV falls within its bounds.
/// 3. Drives the [`XrdsWorldButtonState`] state machine (Idle → Hovered → Pressed).
/// 4. Swaps the button background material to match the current state.
/// 5. Fires [`XrWorldButtonPressEvent`] / [`XrWorldButtonReleaseEvent`] on transitions.
pub(super) fn world_ui_button_system(world: &mut World) {
    let Some(xr) = world.get_resource::<XrInput>().cloned() else { return; };
    let pointer_state = world.resource::<XrdsWorldPointerState>().clone();

    // Phase 1 — collect button data without keeping a query borrow alive.
    let buttons: Vec<(Entity, XrdsWorldButton, XrdsWorldButtonState, Entity)> = {
        // Disabled elements are skipped here, which is the whole mechanism: the
        // widget still renders, it just stops being a pointer target.
        let mut q = world.query_filtered::<
            (Entity, &XrdsWorldButton, &XrdsWorldButtonState, &ChildOf),
            bevy::prelude::Without<xrds_components::XrdsWorldElementDisabled>,
        >();
        q.iter(world)
            .map(|(e, b, s, co)| (e, b.clone(), *s, co.0))
            .collect()
    };

    for (button_entity, button, old_state, panel_entity) in &buttons {
        let button_entity = *button_entity;
        let panel_entity  = *panel_entity;

        // Determine which hand (if any) is hovering this button.
        let hovered_by = [
            (XrGrabHand::Left,  &pointer_state.left),
            (XrGrabHand::Right, &pointer_state.right),
        ]
        .iter()
        .find_map(|(hand, hit_opt)| {
            let hit = hit_opt.as_ref()?;
            if hit.entity != panel_entity { return None; }

            let panel_size = world.get::<XrdsWorldSurface>(panel_entity)?.size;

            // Convert panel UV (0..1, 0..1) → panel-local metres.
            let px = (hit.uv.x - 0.5) * panel_size.x;
            let py = (hit.uv.y - 0.5) * panel_size.y;

            let half_w = button.size[0] * 0.5;
            let half_h = button.size[1] * 0.5;

            if (px - button.local_position[0]).abs() <= half_w
                && (py - button.local_position[1]).abs() <= half_h
            {
                Some(*hand)
            } else {
                None
            }
        });

        // Resolve trigger state for the hovering hand.
        let just_pressed = hovered_by.map_or(false, |h| match h {
            XrGrabHand::Left  => xr.left.select_just_pressed,
            XrGrabHand::Right => xr.right.select_just_pressed,
        });
        let just_released = match old_state {
            XrdsWorldButtonState::Pressed => {
                // Release fires for the hand that caused the press; we don't track
                // which hand that was, so check both for any release.
                xr.left.select_just_released || xr.right.select_just_released
            }
            _ => false,
        };

        // State machine.
        let new_state = match old_state {
            XrdsWorldButtonState::Pressed => {
                if just_released {
                    if hovered_by.is_some() { XrdsWorldButtonState::Hovered }
                    else { XrdsWorldButtonState::Idle }
                } else {
                    XrdsWorldButtonState::Pressed
                }
            }
            _ => {
                if hovered_by.is_some() {
                    if just_pressed { XrdsWorldButtonState::Pressed }
                    else { XrdsWorldButtonState::Hovered }
                } else {
                    XrdsWorldButtonState::Idle
                }
            }
        };

        if new_state == *old_state { continue; }

        // Apply new state component.
        if let Ok(mut e) = world.get_entity_mut(button_entity) {
            e.insert(new_state);
        }

        // Swap background material colour.
        let mat_handles = world.get::<XrdsWorldButtonMaterials>(button_entity)
            .map(|m| (m.normal.clone(), m.hover.clone(), m.pressed.clone()));
        if let Some((normal, hover, pressed)) = mat_handles {
            let new_mat = match new_state {
                XrdsWorldButtonState::Idle    => normal,
                XrdsWorldButtonState::Hovered => hover,
                XrdsWorldButtonState::Pressed => pressed,
            };
            if let Ok(mut e) = world.get_entity_mut(button_entity) {
                e.insert(MeshMaterial3d(new_mat));
            }
        }

        // Fire events on transitions.
        if new_state == XrdsWorldButtonState::Pressed {
            let hand = hovered_by.unwrap_or(XrGrabHand::Right);
            world.write_message(XrWorldButtonPressEvent { button_entity, hand });
        }
        if *old_state == XrdsWorldButtonState::Pressed
            && new_state != XrdsWorldButtonState::Pressed
        {
            // Determine the releasing hand (first one that just released).
            let hand = if xr.left.select_just_released { XrGrabHand::Left }
                       else { XrGrabHand::Right };
            world.write_message(XrWorldButtonReleaseEvent { button_entity, hand });
        }
    }
}
