use super::*;
use xrds_components::{
    XrdsWorldButton, XrdsWorldImage, XrdsWorldLabel, XrdsWorldLayout,
    XrdsWorldSlider, XrdsWorldToggle,
};

/// Per-frame exclusive system that repositions panel-child widgets according to the
/// panel's [`XrdsWorldLayout`] component.
///
/// Runs **before** the button/slider/toggle interaction systems so that each widget's
/// `local_position` field (used for pointer hit-tests) is correct for the current frame.
///
/// Panels without an [`XrdsWorldLayout`] component are not touched — widget `local_position`
/// values stay exactly as set at spawn time.
pub(super) fn world_ui_layout_system(world: &mut World) {
    // Phase 1 — collect panels with a non-None layout (no borrows kept alive).
    let panels: Vec<(Entity, XrdsWorldLayout)> = {
        let mut q = world.query::<(Entity, &XrdsWorldLayout)>();
        q.iter(world)
            .filter(|(_, l)| !matches!(l, XrdsWorldLayout::None))
            .map(|(e, l)| (e, l.clone()))
            .collect()
    };

    for (panel_entity, layout) in panels {
        // Collect the panel's direct widget children.
        let child_entities: Vec<Entity> = {
            let mut q = world.query::<(Entity, &ChildOf)>();
            q.iter(world)
                .filter_map(|(e, co)| if co.0 == panel_entity { Some(e) } else { None })
                .collect()
        };

        let items: Vec<(Entity, [f32; 2])> = child_entities
            .iter()
            .filter_map(|&e| widget_layout_size(world, e).map(|s| (e, s)))
            .collect();

        if items.is_empty() {
            continue;
        }

        let positions = compute_positions(&layout, &items);

        for (entity, pos) in positions {
            apply_widget_position(world, entity, pos);
        }
    }
}

/// Returns the layout slot size `[width, height]` (metres) for a widget entity,
/// or `None` if the entity is not a recognised widget type.
fn widget_layout_size(world: &World, entity: Entity) -> Option<[f32; 2]> {
    if let Some(b) = world.get::<XrdsWorldButton>(entity) {
        return Some(b.size);
    }
    if let Some(l) = world.get::<XrdsWorldLabel>(entity) {
        return Some(l.layout_size);
    }
    if let Some(s) = world.get::<XrdsWorldSlider>(entity) {
        return Some([s.size[0], s.thumb_size * 1.5]);
    }
    if let Some(t) = world.get::<XrdsWorldToggle>(entity) {
        return Some(t.size);
    }
    if let Some(i) = world.get::<XrdsWorldImage>(entity) {
        return Some(i.size);
    }
    None
}

/// Dispatch to the correct layout algorithm.
fn compute_positions(
    layout: &XrdsWorldLayout,
    items: &[(Entity, [f32; 2])],
) -> Vec<(Entity, [f32; 2])> {
    match layout {
        XrdsWorldLayout::None => vec![],
        XrdsWorldLayout::VStack { gap } => vstack(items, *gap),
        XrdsWorldLayout::HStack { gap } => hstack(items, *gap),
        XrdsWorldLayout::Grid { cols, gap } => grid(items, *cols, *gap),
    }
}

/// Stack children top-to-bottom, horizontally centred on panel Y axis.
fn vstack(items: &[(Entity, [f32; 2])], gap: f32) -> Vec<(Entity, [f32; 2])> {
    let n = items.len();
    let total_h: f32 =
        items.iter().map(|(_, s)| s[1]).sum::<f32>() + gap * n.saturating_sub(1) as f32;
    let mut y = total_h * 0.5;
    let mut result = Vec::with_capacity(n);
    for &(entity, size) in items {
        y -= size[1] * 0.5;
        result.push((entity, [0.0, y]));
        y -= size[1] * 0.5 + gap;
    }
    result
}

/// Stack children left-to-right, vertically centred on panel X axis.
fn hstack(items: &[(Entity, [f32; 2])], gap: f32) -> Vec<(Entity, [f32; 2])> {
    let n = items.len();
    let total_w: f32 =
        items.iter().map(|(_, s)| s[0]).sum::<f32>() + gap * n.saturating_sub(1) as f32;
    let mut x = -total_w * 0.5;
    let mut result = Vec::with_capacity(n);
    for &(entity, size) in items {
        x += size[0] * 0.5;
        result.push((entity, [x, 0.0]));
        x += size[0] * 0.5 + gap;
    }
    result
}

/// Grid layout: `cols` columns, each cell centred on its column/row.
fn grid(items: &[(Entity, [f32; 2])], cols: usize, gap: [f32; 2]) -> Vec<(Entity, [f32; 2])> {
    let cols = cols.max(1);
    let rows = (items.len() + cols - 1) / cols;

    // Per-column max width and per-row max height.
    let mut col_w = vec![0.0f32; cols];
    let mut row_h = vec![0.0f32; rows];
    for (i, &(_, size)) in items.iter().enumerate() {
        col_w[i % cols] = col_w[i % cols].max(size[0]);
        row_h[i / cols] = row_h[i / cols].max(size[1]);
    }

    let total_w: f32 = col_w.iter().sum::<f32>() + gap[0] * (cols - 1) as f32;
    let total_h: f32 = row_h.iter().sum::<f32>() + gap[1] * (rows - 1) as f32;

    // Left edge of each column.
    let mut col_start = vec![0.0f32; cols];
    let mut x = -total_w * 0.5;
    for c in 0..cols {
        col_start[c] = x;
        x += col_w[c] + gap[0];
    }
    // Top edge of each row.
    let mut row_start = vec![0.0f32; rows];
    let mut y = total_h * 0.5;
    for r in 0..rows {
        row_start[r] = y;
        y -= row_h[r] + gap[1];
    }

    items
        .iter()
        .enumerate()
        .map(|(i, &(entity, _size))| {
            let c = i % cols;
            let r = i / cols;
            let px = col_start[c] + col_w[c] * 0.5;
            let py = row_start[r] - row_h[r] * 0.5;
            (entity, [px, py])
        })
        .collect()
}

/// Write a new panel-local position to a widget entity.
///
/// Updates:
/// - The widget component's `local_position` field (used for pointer hit-test maths).
/// - The entity's [`Transform`] (actual world placement relative to panel).
///
/// The Z offset (0.001 m in front of the panel quad) is always preserved.
fn apply_widget_position(world: &mut World, entity: Entity, pos: [f32; 2]) {
    // Detect widget type via immutable read — borrows are released after is_some() returns.
    let kind: u8 = if world.get::<XrdsWorldButton>(entity).is_some() {
        0
    } else if world.get::<XrdsWorldLabel>(entity).is_some() {
        1
    } else if world.get::<XrdsWorldSlider>(entity).is_some() {
        2
    } else if world.get::<XrdsWorldToggle>(entity).is_some() {
        3
    } else if world.get::<XrdsWorldImage>(entity).is_some() {
        4
    } else {
        return;
    };

    let Ok(mut e) = world.get_entity_mut(entity) else {
        return;
    };

    match kind {
        0 => {
            if let Some(mut b) = e.get_mut::<XrdsWorldButton>() {
                b.local_position = pos;
            }
        }
        1 => {
            if let Some(mut l) = e.get_mut::<XrdsWorldLabel>() {
                l.local_position = pos;
            }
        }
        2 => {
            if let Some(mut s) = e.get_mut::<XrdsWorldSlider>() {
                s.local_position = pos;
            }
        }
        3 => {
            if let Some(mut t) = e.get_mut::<XrdsWorldToggle>() {
                t.local_position = pos;
            }
        }
        4 => {
            if let Some(mut i) = e.get_mut::<XrdsWorldImage>() {
                i.local_position = pos;
            }
        }
        _ => {}
    }

    e.insert(Transform::from_xyz(pos[0], pos[1], 0.001));
}
