use super::*;

pub(super) fn clone_boxed_descriptor(
    world: &World,
    entity: Entity,
    name_override: Option<&str>,
) -> Option<Box<dyn Any + Send + Sync>> {
    let descriptor_type = descriptor_type_of(world, entity)?;
    world
        .resource::<SurfaceDescriptorRegistry>()
        .clone_descriptor_for_entity(world, entity, descriptor_type, name_override)
}

pub(super) fn spawn_boxed_surface_component(
    world: &mut World,
    id: XrdsId,
    component: Box<dyn Any + Send + Sync>,
    parent_id: Option<XrdsId>,
) -> Option<Entity> {
    let component_type = component.as_ref().type_id();
    let interpreter = {
        let registry = world.resource::<SurfaceInterpreterRegistry>();
        registry.interpreters.get(&component_type).cloned()
    }?;

    let asset_server = world.get_resource::<AssetServer>();
    let mut command_queue = CommandQueue::default();
    let spawned = {
        let mut commands = Commands::new(&mut command_queue, world);
        interpreter(component.as_ref(), &mut commands, asset_server)
    };
    command_queue.apply(world);

    if let Some(entity) = spawned {
        world.resource_mut::<XrdsIdIndex>().register(id, entity);
        world.resource_mut::<XrdsHierarchyIndex>().ensure_node(id);
        apply_parent_changes(
            world,
            vec![QueuedParentChange {
                child_id: id,
                parent_id,
            }],
        );
    }

    spawned
}

pub(super) fn collect_subtree_ids(world: &World, root_id: XrdsId) -> Vec<XrdsId> {
    let mut result = Vec::new();
    let mut stack = vec![root_id];

    let hierarchy = world.resource::<XrdsHierarchyIndex>();

    while let Some(id) = stack.pop() {
        result.push(id);

        if world.resource::<XrdsIdIndex>().entity_of(id).is_some() {
            let mut children = hierarchy.child_ids_of(id);
            children.reverse();
            stack.extend(children);
        }
    }

    result
}

pub(super) fn unregister_entities(world: &mut World, entities: &[Entity]) {
    let removed_ids: Vec<XrdsId> = {
        let ids = world.resource::<XrdsIdIndex>();
        entities
            .iter()
            .filter_map(|entity| ids.id_of(*entity))
            .collect()
    };

    {
        let mut hierarchy = world.resource_mut::<XrdsHierarchyIndex>();

        for id in &removed_ids {
            let _ = hierarchy.remove_node(*id);
        }
    }

    if let Some(mut registry) = world.get_resource_mut::<XrdsRegistry>() {
        for entity in entities {
            registry.unregister(*entity);
        }
    }

    let mut ids = world.resource_mut::<XrdsIdIndex>();
    for entity in entities {
        if let Some(id) = ids.entity_to_id.remove(entity) {
            ids.id_to_entity.remove(&id);
        }
    }
}

pub(super) fn duplicate_name(name: &str) -> String {
    format!("{name} Copy")
}

fn would_create_cycle(
    child_id: XrdsId,
    candidate_parent_id: XrdsId,
    parent_map: &HashMap<XrdsId, Option<XrdsId>>,
) -> bool {
    let mut current = Some(candidate_parent_id);
    let mut steps = 0usize;

    while let Some(parent_id) = current {
        if parent_id == child_id {
            return true;
        }

        current = parent_map.get(&parent_id).copied().flatten();
        steps += 1;

        if steps > parent_map.len() {
            return true;
        }
    }

    false
}

pub(super) fn apply_parent_changes(world: &mut World, changes: Vec<QueuedParentChange>) {
    if changes.is_empty() {
        return;
    }

    let mut requested_parent_map = world.resource::<XrdsHierarchyIndex>().parent_map_snapshot();
    let mut ordered_child_ids = Vec::new();

    for change in &changes {
        if !ordered_child_ids.contains(&change.child_id) {
            ordered_child_ids.push(change.child_id);
        }
        requested_parent_map.insert(change.child_id, change.parent_id);
    }

    for child_id in ordered_child_ids {
        let child_entity = {
            let ids = world.resource::<XrdsIdIndex>();
            ids.entity_of(child_id)
        };

        let Some(child_entity) = child_entity else {
            warn!(
                "No spawned entity found for queued parent change on XRDS id {:?}; skipping",
                child_id
            );
            continue;
        };

        let requested_parent_id = requested_parent_map.get(&child_id).copied().flatten();
        let resolved_parent_id = match requested_parent_id {
            Some(parent_id) if parent_id == child_id => {
                warn!("Ignoring self-parenting request for XRDS id {:?}", child_id);
                None
            }
            Some(parent_id) if would_create_cycle(child_id, parent_id, &requested_parent_map) => {
                warn!(
                    "Ignoring cyclic parenting request for XRDS id {:?} -> {:?}",
                    child_id, parent_id
                );
                None
            }
            Some(parent_id) => {
                let parent_exists = {
                    let ids = world.resource::<XrdsIdIndex>();
                    ids.entity_of(parent_id).is_some()
                };

                if parent_exists {
                    Some(parent_id)
                } else {
                    None
                }
            }
            None => None,
        };

        let parent_entity = resolved_parent_id.and_then(|parent_id| {
            let ids = world.resource::<XrdsIdIndex>();
            ids.entity_of(parent_id)
        });

        {
            let mut hierarchy = world.resource_mut::<XrdsHierarchyIndex>();
            hierarchy.set_parent(child_id, resolved_parent_id);
        }

        if let Some(parent_entity) = parent_entity {
            world.entity_mut(parent_entity).insert((
                Visibility::Visible,
                InheritedVisibility::default(),
                ViewVisibility::default(),
                GlobalTransform::default(),
            ));
            world
                .entity_mut(child_entity)
                .insert(ChildOf(parent_entity));
        } else {
            world.entity_mut(child_entity).remove::<ChildOf>();
        }
    }
}

pub(super) fn apply_surface_updates(world: &mut World) {
    let updates = std::mem::take(&mut world.resource_mut::<QueuedSurfaceUpdates>().updates);

    for update in updates {
        let updater = {
            let registry = world.resource::<SurfaceUpdateRegistry>();
            registry.updater_for(update.component_type, update.patch_type)
        };

        if let Some(updater) = updater {
            updater(world, update.entity, update.patch.as_ref());
        } else {
            warn!(
                "No surface updater registered for component {:?} with patch {:?}; skipping update",
                update.component_type, update.patch_type,
            );
        }
    }
}

pub(super) fn apply_queued_parent_changes(world: &mut World) {
    let changes = std::mem::take(&mut world.resource_mut::<QueuedParentChanges>().changes);

    apply_parent_changes(world, changes);
}

pub(super) fn spawn_surface_components_from_queue(
    mut commands: Commands,
    asset_server: Option<Res<AssetServer>>,
    interpreters: Res<SurfaceInterpreterRegistry>,
    mut ids: ResMut<XrdsIdIndex>,
    mut parent_changes: ResMut<QueuedParentChanges>,
    mut queue: ResMut<QueuedSurfaceComponents>,
) {
    let asset_server = asset_server.as_deref();
    let mut initial_parent_changes = Vec::new();

    for component in queue.components.drain(..) {
        let id = component.id;
        let parent_id = component.parent_id;
        if let Some(entity) =
            interpreters.interpret(component.component.as_ref(), &mut commands, asset_server)
        {
            ids.register(id, entity);
            initial_parent_changes.push(QueuedParentChange {
                child_id: id,
                parent_id,
            });
        } else {
            warn!(
                "No surface interpreter registered for component type {:?}; skipping spawn",
                component.component.as_ref().type_id()
            );
        }
    }

    if !initial_parent_changes.is_empty() {
        let existing_changes = std::mem::take(&mut parent_changes.changes);
        initial_parent_changes.extend(existing_changes);
        parent_changes.changes = initial_parent_changes;
    }
}

pub(super) fn apply_queued_parent_changes_system(world: &mut World) {
    apply_queued_parent_changes(world);
}
