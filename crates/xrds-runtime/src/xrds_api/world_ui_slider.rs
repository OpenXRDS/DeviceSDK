use super::*;
use xrds_components::{
    XrdsWorldSlider, XrdsWorldPointerState, XrdsWorldSurface,
    XrWorldSliderChangeEvent, XrGrabHand,
};
use xrds_openxr::XrInput;

/// Runtime component that caches the thumb entity handle for O(1) repositioning.
#[derive(Component)]
pub(super) struct XrdsWorldSliderParts {
    pub thumb: Entity,
}

/// Per-frame exclusive system that drives slider drag interaction.
///
/// - Detects trigger-press-start within slider bounds → begins drag.
/// - While trigger held: maps pointer UV.x to `[min, max]`; updates value + thumb position.
/// - Fires [`XrWorldSliderChangeEvent`] every frame the value changes during a drag.
/// - Trigger release or pointer leaving the panel ends the drag.
pub(super) fn world_ui_slider_system(world: &mut World) {
    let Some(xr) = world.get_resource::<XrInput>().cloned() else { return; };
    let pointer_state = world.resource::<XrdsWorldPointerState>().clone();

    // Phase 1 — collect without keeping borrows alive.
    let sliders: Vec<(Entity, XrdsWorldSlider, Entity, Entity)> = {
        let mut q = world.query::<(Entity, &XrdsWorldSlider, &ChildOf, &XrdsWorldSliderParts)>();
        q.iter(world)
            .map(|(e, s, co, parts)| (e, s.clone(), co.0, parts.thumb))
            .collect()
    };

    for (slider_entity, slider, panel_entity, thumb_entity) in sliders {
        // Find the pointer hit on this panel (either hand).
        let hand_hit = [
            (XrGrabHand::Left,  pointer_state.left.as_ref()),
            (XrGrabHand::Right, pointer_state.right.as_ref()),
        ]
        .into_iter()
        .find_map(|(hand, hit_opt)| {
            let hit = hit_opt?;
            if hit.entity != panel_entity { return None; }
            Some((hand, hit.uv))
        });

        let panel_size = match world.get::<XrdsWorldSurface>(panel_entity) {
            Some(s) => s.size,
            None => continue,
        };

        // Determine drag state transition.
        let new_dragging_hand = match slider.dragging_hand {
            Some(drag_hand) => {
                let still_held = match drag_hand {
                    XrGrabHand::Left  => xr.left.select,
                    XrGrabHand::Right => xr.right.select,
                };
                if still_held { Some(drag_hand) } else { None }
            }
            None => {
                // Check for fresh drag start.
                hand_hit.and_then(|(hand, uv)| {
                    let px = (uv.x - 0.5) * panel_size.x;
                    let py = (uv.y - 0.5) * panel_size.y;

                    // Use a generous Y tolerance (max of track height and 3 cm) so the
                    // narrow track is easy to hit on first press.
                    let y_tol = slider.size[1].max(0.03);
                    let in_bounds =
                        (px - slider.local_position[0]).abs() <= slider.size[0] * 0.5
                        && (py - slider.local_position[1]).abs() <= y_tol * 0.5;

                    let just_pressed = match hand {
                        XrGrabHand::Left  => xr.left.select_just_pressed,
                        XrGrabHand::Right => xr.right.select_just_pressed,
                    };
                    if in_bounds && just_pressed { Some(hand) } else { None }
                })
            }
        };

        // Compute new value from pointer x position while dragging.
        let new_value = if new_dragging_hand.is_some() {
            match hand_hit {
                Some((_, uv)) => {
                    let px = (uv.x - 0.5) * panel_size.x;
                    let track_left  = slider.local_position[0] - slider.size[0] * 0.5;
                    let track_right = slider.local_position[0] + slider.size[0] * 0.5;
                    let t = ((px - track_left) / (track_right - track_left)).clamp(0.0, 1.0);
                    slider.min + t * (slider.max - slider.min)
                }
                None => slider.value,
            }
        } else {
            slider.value
        };

        let value_changed = (new_value - slider.value).abs() > f32::EPSILON;
        let drag_changed  = new_dragging_hand != slider.dragging_hand;

        if !value_changed && !drag_changed { continue; }

        // Compute thumb X before mutating world.
        let thumb_x = {
            let range = slider.max - slider.min;
            let t = if range.abs() > 1e-9 { ((new_value - slider.min) / range).clamp(0.0, 1.0) } else { 0.0 };
            (t - 0.5) * (slider.size[0] - slider.thumb_size)
        };

        // Mutate slider component in-place.
        if let Ok(mut e) = world.get_entity_mut(slider_entity) {
            if let Some(mut s) = e.get_mut::<XrdsWorldSlider>() {
                s.value         = new_value;
                s.dragging_hand = new_dragging_hand;
            }
        }

        // Reposition thumb.
        if value_changed {
            if let Ok(mut e) = world.get_entity_mut(thumb_entity) {
                e.insert(Transform::from_xyz(thumb_x, 0.0, 0.001));
            }

            if let Some(hand) = new_dragging_hand {
                world.write_message(XrWorldSliderChangeEvent { slider_entity, value: new_value, hand });
            }
        }
    }
}

/// Update a slider's value programmatically and reposition its thumb.
pub(super) fn set_slider_value_in_world(world: &mut World, entity: Entity, value: f32) {
    let (min, max, size, thumb_size) = match world.get::<XrdsWorldSlider>(entity) {
        Some(s) => (s.min, s.max, s.size, s.thumb_size),
        None => return,
    };
    let value = value.clamp(min, max);

    if let Ok(mut e) = world.get_entity_mut(entity) {
        if let Some(mut s) = e.get_mut::<XrdsWorldSlider>() {
            s.value = value;
        }
    }

    let thumb_entity = world.get::<XrdsWorldSliderParts>(entity).map(|p| p.thumb);
    if let Some(thumb_entity) = thumb_entity {
        let range = max - min;
        let t = if range.abs() > 1e-9 { ((value - min) / range).clamp(0.0, 1.0) } else { 0.0 };
        let thumb_x = (t - 0.5) * (size[0] - thumb_size);
        if let Ok(mut e) = world.get_entity_mut(thumb_entity) {
            e.insert(Transform::from_xyz(thumb_x, 0.0, 0.001));
        }
    }
}
