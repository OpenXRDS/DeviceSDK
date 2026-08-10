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

    // ── 3. Catalog, environment, and the panel registry ───────────────────────
    merge_imported_asset_catalog(world, &document.assets);
    store_imported_scene_environment_in_world(world, document.environment().cloned());
    sync_panel_registry(world, document);

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

    // ── 4b2. Spawn panel-template instances ──────────────────────────────────
    // After the id index is populated (it resolves each Panel node to its
    // entity) and before trigger tagging, so element bindings and node bindings
    // land in the same pass order.
    spawn_panel_instances(world, document);

    // ── 4c. Tag grabbable entities ────────────────────────────────────────────
    tag_grabbable_entities(world, document);

    // ── 4d. Tag spawn zone entities ───────────────────────────────────────────
    tag_spawn_zone_entities(world, document);

    // ── 4e. Tag trigger-binding entities ──────────────────────────────────────
    tag_trigger_binding_entities(world, document);

    // ── 4f. Tag threshold-watcher entities ────────────────────────────────────
    tag_threshold_watcher_entities(world, document);

    // ── 4g. Sync the runnable registry ────────────────────────────────────────
    sync_track_registry(world, document);

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
                // Instantiate the head-locked panel template this anchor links.
                // There used to be a second `hud_template_id` branch here, with a
                // precedence rule between them; the unification left exactly one
                // kind of template, so there is nothing to prefer.
                if let Some(tid) = a.panel_template_id {
                    if let Some(template) = document.panel_template(tid) {
                        let template = template.clone();
                        let instance = spawn_panel_template_head_locked(
                            world,
                            entity,
                            &template,
                            a.panel_depth,
                            // See api.rs `link_panel`: an anchor link has no
                            // place for per-element bindings. §A6-2 replaces this
                            // whole branch with a `Panel` node under the anchor.
                            &Default::default(),
                        );
                        if let Ok(mut e) = world.get_entity_mut(entity) {
                            e.insert(instance);
                        }
                    } else {
                        log::warn!(
                            "PlayerAnchor {:?} links panel template {tid:?}, which is not in this \
                             document — nothing head-locked.",
                            node.id
                        );
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

/// Insert [`crate::xrds_api::trigger_action::XrdsThresholdWatchers`] on every
/// entity whose scene document node has non-empty `watchers` data. Mirrors
/// [`tag_trigger_binding_entities`] exactly, including the toggle-safe
/// remove-when-empty behavior. Spawns the authored definition only —
/// per-watcher crossing state lives in a separate, runtime-only component
/// that this does not touch (re-tagging on a live reimport must not reset
/// a watcher's in-progress state).
pub(super) fn tag_threshold_watcher_entities(world: &mut World, document: &XrdsSceneDocument) {
    for node in &document.nodes {
        let Some(entity) = world.resource::<XrdsIdIndex>().entity_of(node.id.into()) else {
            continue;
        };
        let Ok(mut e) = world.get_entity_mut(entity) else { continue; };
        if node.watchers.is_empty() {
            e.remove::<crate::xrds_api::trigger_action::XrdsThresholdWatchers>();
        } else {
            e.insert(crate::xrds_api::trigger_action::XrdsThresholdWatchers(
                node.watchers.clone(),
            ));
        }
    }
}

/// Replaces [`crate::xrds_api::trigger_action::XrdsTrackRegistry`] wholesale
/// from `document.tracks` — matching every other tag_* helper here in
/// treating the document as complete, authoritative state rather than
/// something to merge into.
pub(super) fn sync_track_registry(world: &mut World, document: &XrdsSceneDocument) {
    let map = document
        .tracks
        .iter()
        .map(|entry| (entry.name.clone(), entry.track.clone()))
        .collect();
    world.insert_resource(crate::xrds_api::trigger_action::XrdsTrackRegistry(map));
}

/// Replaces [`XrdsImportedPanelLibrary`] wholesale from `document.panels`, for
/// the same reason [`sync_track_registry`] exists.
///
/// Without this the registry is import-only: a `Panel` node carries nothing but
/// a `template_id`, so `export_scene_document` produced documents whose panels
/// resolved to nothing, and a save/load cycle silently deleted every panel
/// template. Identical to the bug the Track registry export fixed.
///
/// Called from **both** import paths. `reimport_scene_in_world` and
/// `XrdsAPI::import_scene_document` do not share a body, and a helper wired into
/// only one of them is the shape of the `tag_player_anchor_entities` gap.
pub(super) fn sync_panel_registry(world: &mut World, document: &XrdsSceneDocument) {
    world.insert_resource(XrdsImportedPanelLibrary { templates: document.panels.clone() });
}

/// Insert [`XrdsPlayerSpawnZone`] on every entity whose scene document node is a
/// `PlayerSpawnZone` payload.  Called after reimport so the API can query zone positions.
/// Spawns each `Panel` node's visuals and elements from its referenced template.
///
/// Runs as a pass over the document rather than inside `to_runtime_node`,
/// because a `Panel` node carries only a `template_id` and resolving it needs
/// the document — which that per-node conversion deliberately does not have.
/// Same shape as [`tag_spawn_zone_entities`] below.
///
/// Each element goes through
/// [`crate::xrds_api::trigger_action::spawn_panel_element_in_world`], which is
/// what attaches the element's authored triggers to the entity its widget events
/// will target. That is the step that makes element triggers fire at all.
///
/// A template instanced N times produces N independent sets of element entities,
/// which is the point of the template/instance split — and is why the elements
/// are spawned per instance here rather than once per template.
///
/// **Attachment is decided by the hierarchy.** A Panel node whose ancestors
/// include a `PlayerAnchor` is head-locked; anywhere else it is a world panel.
/// Nothing on the payload says which, because parenting already does.
pub(super) fn spawn_panel_instances(world: &mut World, document: &XrdsSceneDocument) {
    // Cleared before repopulating, not merged into: element entities are
    // despawned and respawned wholesale on reimport, so a surviving entry would
    // point at a dead entity — or, once Bevy recycles the id, at an unrelated one.
    world.insert_resource(XrdsPanelElementIndex::default());

    for node in &document.nodes {
        let XrdsSceneNodePayload::Panel(ref instance) = node.payload else { continue };
        let Some(panel_entity) = world.resource::<XrdsIdIndex>().entity_of(node.id.into()) else {
            continue;
        };
        let Some(template) = document.panel_template(instance.template_id) else {
            // Dangling reference. Diagnosed at author time by
            // `panel_diagnostics`; here it just means an empty node, which is
            // better than refusing to load the scene.
            log::warn!(
                "Panel node {:?} references template {:?}, which is not in this document — \
                 nothing to spawn.",
                node.id,
                instance.template_id
            );
            continue;
        };

        // Head-locked when an ancestor is a PlayerAnchor. Resolved from the
        // document rather than from Bevy's hierarchy because parent links are
        // still queued at this point in the import.
        let anchor = head_locked_anchor_of(document, node)
            .and_then(|id| world.resource::<XrdsIdIndex>().entity_of(id.into()));

        let mut items: Vec<(String, Entity)> = Vec::new();
        for element in &template.elements {
            // Bindings come from this node, so two instances of one template can
            // drive two different targets. A binding whose key names no element
            // is simply never reached here — `panel_diagnostics` reports it
            // rather than this silently dropping it.
            let entity = crate::xrds_api::trigger_action::spawn_panel_element_in_world(
                world,
                panel_entity,
                element,
                instance.triggers_for(&element.name),
            );

            if anchor.is_some() {
                // The node's own transform places the panel plane in *camera-local*
                // space (X right, Y up, -Z forward); the element sits on that plane
                // at its canvas position. Composing them is what replaces the old
                // scalar `panel_depth` — an author gets rotation and offset, not
                // just a distance.
                //
                // Careful: this is the node's **local** transform, not its world
                // position. Feeding a world position in as a camera-local offset is
                // the documented anchor-offset mistake.
                let [x, y] = element.local_position();
                let t = &node.transform;
                let [rx, ry, rz, rw] = t.rotation_quat_xyzw;
                let base = Transform {
                    translation: Vec3::from_array(t.translation),
                    rotation: Quat::from_xyzw(rx, ry, rz, rw),
                    scale: Vec3::from_array(t.scale),
                };
                let local_offset = base * Transform::from_translation(Vec3::new(x, y, 0.0));
                if let Ok(mut e) = world.get_entity_mut(entity) {
                    e.insert((
                        local_offset,
                        crate::xrds_api::anchor::XrdsHeadLocked { local_offset },
                    ));
                }
            }

            // Registered for **every** panel, head-locked or not: an `Element`
            // action target addresses `(panel node, element name)` and does not
            // care how the panel is attached.
            world
                .resource_mut::<XrdsPanelElementIndex>()
                .insert(panel_entity, element.name.clone(), entity);

            items.push((element.name.clone(), entity));
        }

        // `set_hud_item(anchor_id, name)` resolves `XrdsStoredHudInstance` on the
        // anchor, and its signature predates all of this. Contributing to the
        // anchor's component — extending rather than replacing, so two panels under
        // one anchor both stay addressable — keeps that public API working
        // unchanged now that a HUD is a node rather than a field.
        if let Some(anchor_entity) = anchor {
            if !items.is_empty() {
                if let Ok(mut e) = world.get_entity_mut(anchor_entity) {
                    let mut merged = e
                        .get::<crate::xrds_api::state::XrdsStoredHudInstance>()
                        .map(|h| h.items.clone())
                        .unwrap_or_default();
                    merged.extend(items);
                    e.insert(crate::xrds_api::state::XrdsStoredHudInstance { items: merged });
                }
            }
        }
    }
}

/// The nearest `PlayerAnchor` ancestor of `node`, if any — what makes a Panel
/// node head-locked rather than a world panel.
///
/// Walks the document's `parent_id` chain rather than Bevy's hierarchy: during
/// import the parent links are still queued in `QueuedParentChanges`, so the ECS
/// does not know about them yet.
///
/// Depth-bounded by the node count so a `parent_id` cycle in a hand-edited
/// document cannot hang the import.
pub(super) fn head_locked_anchor_of(
    document: &XrdsSceneDocument,
    node: &XrdsSceneNode,
) -> Option<xrds_scene_graph::XrdsSceneNodeId> {
    let mut current = node.parent_id;
    for _ in 0..document.nodes.len() {
        let id = current?;
        let parent = document.nodes.iter().find(|n| n.id == id)?;
        if matches!(parent.payload, XrdsSceneNodePayload::PlayerAnchor(_)) {
            return Some(parent.id);
        }
        current = parent.parent_id;
    }
    None
}

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
