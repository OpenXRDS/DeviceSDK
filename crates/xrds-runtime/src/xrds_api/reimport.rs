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

    // ── 3. Catalog, environment, and HUD library ──────────────────────────────
    merge_imported_asset_catalog(world, &document.assets);
    store_imported_scene_environment_in_world(world, document.environment().cloned());
    world.resource_mut::<XrdsImportedHudLibrary>().templates = document.hud_library.clone();

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

    // ── 4b. Tag Player / PlayerAnchor entities ────────────────────────────────
    tag_player_anchor_entities(world, document);

    // ── 4c. Tag grabbable entities ────────────────────────────────────────────
    tag_grabbable_entities(world, document);

    // ── 4d. Tag spawn zone entities ───────────────────────────────────────────
    tag_spawn_zone_entities(world, document);

    // ── 4e. Tag trigger-binding entities ──────────────────────────────────────
    tag_trigger_binding_entities(world, document);

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

    // Tag Player / PlayerAnchor / grabbable entities.
    if let Some(doc_node) = document.nodes.iter().find(|n| XrdsId::from(n.id) == target_id) {
        if let Ok(mut e) = world.get_entity_mut(entity) {
            match &doc_node.payload {
                XrdsSceneNodePayload::Player(_) => { e.insert(XrdsPlayerRoot); }
                XrdsSceneNodePayload::PlayerAnchor(a) => {
                    e.insert(XrdsPlayerAnchorRoot);
                    e.insert(XrdsAnchorFov(a.fov_deg));
                    e.insert(XrdsAnchorExposure(a.exposure));
                    if a.is_initial { e.insert(XrdsInitialAnchor); }
                    let world_tf = authored_world_transform(&document.nodes, doc_node);
                    e.insert(PlayerAnchorCameraPose {
                        translation: world_tf.translation,
                        rotation:    world_tf.rotation,
                        fov_deg:     a.fov_deg,
                    });
                }
                _ => {}
            }
            if doc_node.grabbable {
                e.insert(xrds_components::XrGrabbable);
            }
        }
    }

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
        XrdsSceneRuntimeComponent::ExtrudedText(c) => {
            Some(spawn_extruded_text_descriptor(commands, c))
        }
        XrdsSceneRuntimeComponent::WorldPanel(panel_desc, widgets, scene_layout) => {
            let entity = spawn_world_panel_descriptor(commands, panel_desc);
            let widgets = widgets.clone();
            let scene_layout = scene_layout.clone();
            commands.queue(move |world: &mut World| {
                for widget in &widgets {
                    spawn_world_widget_from_scene(world, entity, widget);
                }
                use xrds_scene_graph::XrdsSceneWorldLayout;
                let layout_opt = match &scene_layout {
                    XrdsSceneWorldLayout::None => None,
                    XrdsSceneWorldLayout::VStack { gap } => {
                        Some(xrds_components::XrdsWorldLayout::VStack { gap: *gap })
                    }
                    XrdsSceneWorldLayout::HStack { gap } => {
                        Some(xrds_components::XrdsWorldLayout::HStack { gap: *gap })
                    }
                    XrdsSceneWorldLayout::Grid { cols, gap } => {
                        Some(xrds_components::XrdsWorldLayout::Grid { cols: *cols, gap: *gap })
                    }
                };
                if let Some(layout) = layout_opt {
                    if let Ok(mut e) = world.get_entity_mut(entity) {
                        e.insert(layout);
                    }
                }
            });
            Some(entity)
        }
        XrdsSceneRuntimeComponent::InteractionZone(node, zone) => {
            use avian3d::prelude::{CollisionEventsEnabled, Sensor};
            let collider = match zone.shape {
                xrds_components::XrdsInteractionZoneShape::Sphere { radius } => {
                    avian3d::prelude::Collider::sphere(radius)
                }
                xrds_components::XrdsInteractionZoneShape::Box { half_extents: [hx, hy, hz] } => {
                    avian3d::prelude::Collider::cuboid(hx * 2.0, hy * 2.0, hz * 2.0)
                }
            };
            Some(
                commands
                    .spawn((
                        bevy::prelude::Name::new(node.name.clone()),
                        build_transform(&node.transform),
                        build_visibility_hierarchy_components(node.visible),
                        collider,
                        Sensor,
                        CollisionEventsEnabled,
                        *zone,
                    ))
                    .id(),
            )
        }
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

/// Tag `XrdsPlayerRoot` and `XrdsPlayerAnchorRoot` (+ related components) on
/// every entity whose document node carries a `Player` or `PlayerAnchor` payload.
///
/// Called after node entities are spawned — both from the full `reimport_scene_in_world`
/// path (editor) and from the `import_scene_document` / `import_scene_document_json` path
/// (exported xrds-app runtime).  Without this step, `ActivePlayerAnchorEntity` has nothing
/// to select and all body-locked anchor systems fall back to legacy "all active" behaviour.
///
/// Uses `eprintln!` rather than `info!` so the output is visible even before Bevy's tracing
/// subscriber is initialised (xrds-app calls `setup()` inside `on_construct`, before the
/// event loop starts).
pub(super) fn tag_player_anchor_entities(
    world: &mut World,
    document: &XrdsSceneDocument,
) {
    let anchor_nodes_in_doc = document.nodes.iter()
        .filter(|n| matches!(n.payload, XrdsSceneNodePayload::PlayerAnchor(_)))
        .count();
    let player_nodes_in_doc = document.nodes.iter()
        .filter(|n| matches!(n.payload, XrdsSceneNodePayload::Player(_)))
        .count();
    eprintln!(
        "[xrds-runtime] tag_player_anchor_entities: document has {anchor_nodes_in_doc} PlayerAnchor(s), {player_nodes_in_doc} Player(s)"
    );

    let mut anchor_tagged = 0u32;
    let mut player_tagged = 0u32;

    for node in &document.nodes {
        let entity = match world.resource::<XrdsIdIndex>().entity_of(node.id.into()) {
            Some(e) => e,
            None => {
                if matches!(
                    node.payload,
                    XrdsSceneNodePayload::PlayerAnchor(_) | XrdsSceneNodePayload::Player(_)
                ) {
                    eprintln!(
                        "[xrds-runtime] tag: node '{}' id={:?} ({}) has NO entity in XrdsIdIndex — skipped",
                        node.name,
                        node.id,
                        match &node.payload {
                            XrdsSceneNodePayload::PlayerAnchor(_) => "PlayerAnchor",
                            XrdsSceneNodePayload::Player(_) => "Player",
                            _ => "other",
                        }
                    );
                }
                continue;
            }
        };
        match &node.payload {
            XrdsSceneNodePayload::Player(_) => {
                if let Ok(mut e) = world.get_entity_mut(entity) {
                    e.insert(XrdsPlayerRoot);
                    player_tagged += 1;
                }
            }
            XrdsSceneNodePayload::PlayerAnchor(a) => {
                if let Ok(mut e) = world.get_entity_mut(entity) {
                    e.insert(XrdsPlayerAnchorRoot);
                    e.insert(XrdsAnchorFov(a.fov_deg));
                    e.insert(XrdsAnchorExposure(a.exposure));
                    if a.is_initial { e.insert(XrdsInitialAnchor); }
                    let world_tf = authored_world_transform(&document.nodes, node);
                    eprintln!(
                        "[xrds-runtime] tag: PlayerAnchor '{}' (is_initial={}) → entity {:?}, world_pos={:?}, fov={}°",
                        node.name, a.is_initial, entity, world_tf.translation, a.fov_deg
                    );
                    e.insert(PlayerAnchorCameraPose {
                        translation: world_tf.translation,
                        rotation:    world_tf.rotation,
                        fov_deg:     a.fov_deg,
                    });
                    anchor_tagged += 1;
                }
                // Spawn HUD instance if this anchor has a linked template.
                if let Some(tid) = a.hud_template_id {
                    if let Some(template) = document.hud_template(tid) {
                        let template = template.clone();
                        let hud_instance = spawn_hud_instance_for_anchor(world, entity, &template);
                        if let Ok(mut e) = world.get_entity_mut(entity) {
                            e.insert(hud_instance);
                        }
                    }
                }
            }
            _ => {}
        }
    }
    eprintln!(
        "[xrds-runtime] tag_player_anchor_entities complete: tagged {anchor_tagged} PlayerAnchor(s), {player_tagged} Player(s)"
    );
}

/// Compute an authored node's world-space Transform by walking up the document
/// parent chain and composing local transforms.
///
/// All transforms are authored (pre-runtime) values; no Bevy `GlobalTransform`
/// is involved, so this is safe to call during scene import before any
/// `TransformPropagate` pass has run.
fn authored_world_transform(
    nodes: &[xrds_scene_graph::XrdsSceneNode],
    node:  &xrds_scene_graph::XrdsSceneNode,
) -> Transform {
    let local = Transform {
        translation: Vec3::from_array(node.transform.translation),
        rotation:    Quat::from_array(node.transform.rotation_quat_xyzw),
        scale:       Vec3::from_array(node.transform.scale),
    };
    let Some(parent_id) = node.parent_id else { return local; };
    let Some(parent) = nodes.iter().find(|n| n.id == parent_id) else { return local; };
    let parent_world = authored_world_transform(nodes, parent);
    Transform {
        translation: parent_world.transform_point(local.translation),
        rotation:    parent_world.rotation * local.rotation,
        scale:       parent_world.scale * local.scale,
    }
}

/// Insert [`XrGrabbable`] on every entity whose scene document node has `grabbable: true`.
/// Remove it from any that have `grabbable: false` (handles a toggle from the editor).
pub(super) fn tag_grabbable_entities(world: &mut World, document: &XrdsSceneDocument) {
    for node in &document.nodes {
        let Some(entity) = world.resource::<XrdsIdIndex>().entity_of(node.id.into()) else {
            continue;
        };
        let Ok(mut e) = world.get_entity_mut(entity) else { continue; };
        if node.grabbable {
            e.insert(xrds_components::XrGrabbable);
        } else {
            e.remove::<xrds_components::XrGrabbable>();
        }
    }
}

/// Insert [`crate::xrds_api::trigger_action::XrdsTriggerBindings`] on every entity whose
/// scene document node has non-empty `triggers` data. Remove it from any that have none
/// (handles a toggle from the editor, same as [`tag_grabbable_entities`]). This spawns the
/// authored *definition* only — nothing is enqueued onto a `bevy-sequential-actions` queue
/// here; that only happens when a matching trigger event actually fires (Phase 3).
pub(super) fn tag_trigger_binding_entities(world: &mut World, document: &XrdsSceneDocument) {
    for node in &document.nodes {
        let Some(entity) = world.resource::<XrdsIdIndex>().entity_of(node.id.into()) else {
            continue;
        };
        let Ok(mut e) = world.get_entity_mut(entity) else { continue; };
        if node.triggers.is_empty() {
            e.remove::<crate::xrds_api::trigger_action::XrdsTriggerBindings>();
        } else {
            e.insert(crate::xrds_api::trigger_action::XrdsTriggerBindings(
                node.triggers.clone(),
            ));
        }
    }
}

/// Insert [`XrdsPlayerSpawnZone`] on every entity whose scene document node is a
/// `PlayerSpawnZone` payload.  Called after reimport so the API can query zone positions.
pub(super) fn tag_spawn_zone_entities(world: &mut World, document: &XrdsSceneDocument) {
    for node in &document.nodes {
        let XrdsSceneNodePayload::PlayerSpawnZone(ref z) = node.payload else { continue; };
        let Some(entity) = world.resource::<XrdsIdIndex>().entity_of(node.id.into()) else {
            continue;
        };
        let Ok(mut e) = world.get_entity_mut(entity) else { continue; };
        e.insert(xrds_components::XrdsPlayerSpawnZone {
            size: bevy::math::Vec3::from_array(z.size),
            player_node_id: z.player_node_id,
        });
    }
}
