//! World-level scene reimport — despawns all XRDS entities and re-spawns from a document.

use super::*;
use bevy::ecs::world::CommandQueue;

/// Despawn every XRDS-tracked entity, clear the id/hierarchy indices, then
/// re-spawn all nodes from `document`.
///
/// Called via `XrdsUpdateContext::reimport_scene()` when the editor adds,
/// removes, or structurally changes scene nodes.
pub(super) fn reimport_scene_in_world(
    world: &mut World,
    document: &XrdsSceneDocument,
) -> Result<Vec<XrdsId>, XrdsSceneImportError> {
    // ── 1. Despawn all existing XRDS entities ─────────────────────────────────
    let existing: Vec<Entity> = world
        .resource::<XrdsIdIndex>()
        .entity_to_id
        .keys()
        .copied()
        .collect();
    for entity in existing {
        if world.entities().contains(entity) {
            world.despawn(entity);
        }
    }

    // ── 2. Clear indices ──────────────────────────────────────────────────────
    world.resource_mut::<XrdsIdIndex>().id_to_entity.clear();
    world.resource_mut::<XrdsIdIndex>().entity_to_id.clear();
    *world.resource_mut::<XrdsHierarchyIndex>() = XrdsHierarchyIndex::default();
    world.resource_mut::<QueuedParentChanges>().changes.clear();

    // ── 3. Catalog and environment ────────────────────────────────────────────
    merge_imported_asset_catalog(world, &document.assets);
    store_imported_scene_environment_in_world(world, document.environment().cloned());

    // Reset global ambient light to Bevy's default before spawning nodes.
    // Any AmbientLight node in the document will override this in step 4.
    // Without this reset, deleting an AmbientLight node would leave stale
    // brightness from the previous import.
    world.insert_resource(AmbientLight::default());

    // ── 4. Spawn nodes ────────────────────────────────────────────────────────
    let runtime_nodes = document.to_runtime_nodes()?;
    let mut imported_ids = Vec::new();
    let mut material_updates: Vec<(Entity, XrdsMaterialParams)> = Vec::new();

    for node in &runtime_nodes {
        let id = node.id; // XrdsId
        if reserve_runtime_id_in_world(world, id).is_err() {
            continue; // skip if id is somehow already reserved
        }

        // Each spawn function internally uses `commands.queue(|world|{...})` to
        // defer world mutations.  A CommandQueue lets us apply them immediately
        // from this exclusive-world context without needing `&mut App`.
        let mut queue = CommandQueue::default();
        let entity_opt: Option<Entity> = {
            let mut commands = Commands::new(&mut queue, world);
            let entity = spawn_runtime_component(&mut commands, &node.component);
            if let Some(e) = entity {
                commands
                    .entity(e)
                    .insert(XrdsDescriptorType(node.component.type_id()));
            }
            entity
        };
        queue.apply(world); // execute deferred closures now

        let Some(entity) = entity_opt else {
            continue;
        };

        world.resource_mut::<XrdsIdIndex>().register(id, entity);
        imported_ids.push(id);

        if let Some(mat) = node.material.clone() {
            material_updates.push((entity, mat));
        }

        // Track hierarchy
        let parent_xrds_id = node.parent_id;
        world
            .resource_mut::<XrdsHierarchyIndex>()
            .set_parent(id, parent_xrds_id);
        if let Some(pid) = parent_xrds_id {
            world
                .resource_mut::<QueuedParentChanges>()
                .changes
                .push(QueuedParentChange {
                    child_id: id,
                    parent_id: Some(pid),
                });
        }
    }

    // ── 5. Apply materials ────────────────────────────────────────────────────
    for (entity, mat) in material_updates {
        set_material_params_for_entity_in_world(world, entity, mat);
    }

    // ── 6. Bevy parent-child hierarchy ────────────────────────────────────────
    apply_queued_parent_changes_system(world);

    // ── 7. Scene environment ──────────────────────────────────────────────────
    apply_imported_scene_environment_policy_in_world(world);

    Ok(imported_ids)
}

/// Spawn a single document node without despawning any existing entities.
///
/// Used for incremental additions (e.g. palette placement) where only one new
/// node is inserted into an otherwise stable scene.  Merges catalog assets,
/// spawns the entity, registers it in XRDS indices, applies materials, and
/// wires the Bevy parent-child hierarchy — all without touching existing
/// entities.
pub(super) fn spawn_document_node_in_world(
    world: &mut World,
    document: &XrdsSceneDocument,
    target_id: XrdsId,
) -> Result<XrdsId, XrdsSceneImportError> {
    merge_imported_asset_catalog(world, &document.assets);

    let runtime_nodes = document.to_runtime_nodes()?;
    let node = runtime_nodes
        .iter()
        .find(|n| n.id == target_id)
        .ok_or_else(|| {
            XrdsSceneImportError::InvalidDocument(format!(
                "node {target_id:?} not found in document"
            ))
        })?
        .clone();

    reserve_runtime_id_in_world(world, target_id)?;

    let mut queue = CommandQueue::default();
    let entity_opt: Option<Entity> = {
        let mut commands = Commands::new(&mut queue, world);
        let entity = spawn_runtime_component(&mut commands, &node.component);
        if let Some(e) = entity {
            commands
                .entity(e)
                .insert(XrdsDescriptorType(node.component.type_id()));
        }
        entity
    };
    queue.apply(world);

    let Some(entity) = entity_opt else {
        return Err(XrdsSceneImportError::InvalidDocument(format!(
            "spawn returned None for {target_id:?}"
        )));
    };

    world.resource_mut::<XrdsIdIndex>().register(target_id, entity);

    if let Some(mat) = node.material.clone() {
        set_material_params_for_entity_in_world(world, entity, mat);
    }

    let parent_xrds_id = node.parent_id;
    world
        .resource_mut::<XrdsHierarchyIndex>()
        .set_parent(target_id, parent_xrds_id);
    if let Some(pid) = parent_xrds_id {
        world
            .resource_mut::<QueuedParentChanges>()
            .changes
            .push(QueuedParentChange { child_id: target_id, parent_id: Some(pid) });
    }
    apply_queued_parent_changes_system(world);

    Ok(target_id)
}

/// Dispatch to the correct typed spawn function for each runtime component variant.
/// Returns `None` only for glTF nodes whose source file fails validation.
fn spawn_runtime_component(
    commands: &mut Commands,
    component: &XrdsSceneRuntimeComponent,
) -> Option<Entity> {
    match component {
        XrdsSceneRuntimeComponent::Node(c) => Some(spawn_node_descriptor(commands, c)),
        XrdsSceneRuntimeComponent::Camera(c) => Some(spawn_camera_descriptor(commands, c)),
        XrdsSceneRuntimeComponent::GltfAsset(c) => spawn_gltf_descriptor(commands, c),
        XrdsSceneRuntimeComponent::Cube(c) => Some(spawn_cube_descriptor(commands, c)),
        XrdsSceneRuntimeComponent::Cylinder(c) => Some(spawn_cylinder_descriptor(commands, c)),
        XrdsSceneRuntimeComponent::Sphere(c) => Some(spawn_sphere_descriptor(commands, c)),
        XrdsSceneRuntimeComponent::Plane3D(c) => Some(spawn_plane_descriptor(commands, c)),
        XrdsSceneRuntimeComponent::Tetrahedron(c) => {
            let entity = execute_spawn_recipe(
                commands,
                XrdsGeometrySource::PbrTetrahedron {
                    vertices: c.vertices.map(Into::into),
                    material: XrdsMaterialParams::default(),
                },
                c.name.clone(),
                c.transform,
                c.visible,
            );
            commands.entity(entity).insert(XrdsStored(c.clone()));
            Some(entity)
        }
        XrdsSceneRuntimeComponent::PointLight(c) => Some(spawn_point_light_descriptor(commands, c)),
        XrdsSceneRuntimeComponent::DirectionalLight(c) => {
            Some(spawn_directional_light_descriptor(commands, c))
        }
        XrdsSceneRuntimeComponent::SpotLight(c) => Some(spawn_spot_light_descriptor(commands, c)),
        XrdsSceneRuntimeComponent::AmbientLight(c) => {
            Some(spawn_ambient_light_descriptor(commands, c))
        }
        XrdsSceneRuntimeComponent::AudioClip(c) => Some(spawn_audio_clip_descriptor(commands, c)),
        XrdsSceneRuntimeComponent::HudText(hud) => {
            Some(spawn_hud_text_for_reimport(commands, hud))
        }
        XrdsSceneRuntimeComponent::Text(c) => Some(spawn_text_descriptor(commands, c)),
    }
}

fn spawn_hud_text_for_reimport(
    commands: &mut Commands,
    hud: &xrds_scene_graph::XrdsHudTextData,
) -> Entity {
    use bevy::text::{TextColor, TextFont};
    use bevy::ui::{Node, PositionType, Val};

    let [r, g, b, a] = hud.color;
    let [ox, oy] = hud.offset;

    let mut node = Node {
        position_type: PositionType::Absolute,
        ..Default::default()
    };

    match hud.anchor {
        xrds_scene_graph::XrdsHudAnchor::TopLeft => {
            node.top = Val::Px(oy);
            node.left = Val::Px(ox);
        }
        xrds_scene_graph::XrdsHudAnchor::TopCenter => {
            node.top = Val::Px(oy);
            node.left = Val::Percent(50.0);
        }
        xrds_scene_graph::XrdsHudAnchor::TopRight => {
            node.top = Val::Px(oy);
            node.right = Val::Px(ox);
        }
        xrds_scene_graph::XrdsHudAnchor::MiddleLeft => {
            node.top = Val::Percent(50.0);
            node.left = Val::Px(ox);
        }
        xrds_scene_graph::XrdsHudAnchor::Center => {
            node.top = Val::Percent(50.0);
            node.left = Val::Percent(50.0);
        }
        xrds_scene_graph::XrdsHudAnchor::MiddleRight => {
            node.top = Val::Percent(50.0);
            node.right = Val::Px(ox);
        }
        xrds_scene_graph::XrdsHudAnchor::BottomLeft => {
            node.bottom = Val::Px(oy);
            node.left = Val::Px(ox);
        }
        xrds_scene_graph::XrdsHudAnchor::BottomCenter => {
            node.bottom = Val::Px(oy);
            node.left = Val::Percent(50.0);
        }
        xrds_scene_graph::XrdsHudAnchor::BottomRight => {
            node.bottom = Val::Px(oy);
            node.right = Val::Px(ox);
        }
    }

    let stored = XrdsStoredHudText(xrds_scene_graph::XrdsSceneHudText {
        text: hud.text.clone(),
        font_size: hud.font_size,
        color: hud.color,
        anchor: hud.anchor,
        offset: hud.offset,
    });

    commands
        .spawn((
            bevy::ui::widget::Text::new(hud.text.clone()),
            node,
            TextFont { font_size: hud.font_size, ..Default::default() },
            TextColor(bevy::color::Color::srgba(r, g, b, a)),
            stored,
        ))
        .id()
}
