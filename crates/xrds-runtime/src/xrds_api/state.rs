use super::*;

type SurfaceInterpreter =
    Arc<dyn Fn(&dyn Any, &mut Commands, Option<&AssetServer>) -> Option<Entity> + Send + Sync>;
type SurfaceRecipeInterpreter = Arc<dyn Fn(&dyn Any) -> Option<XrdsGeometrySource> + Send + Sync>;
type SurfaceDescriptorCloner =
    Arc<dyn Fn(&World, Entity, Option<&str>) -> Option<Box<dyn Any + Send + Sync>> + Send + Sync>;

#[derive(Component, Debug, Clone)]
pub(super) struct XrdsStored<C>(pub(super) C);

#[derive(Component, Debug, Clone)]
pub(super) struct XrdsStoredMaterial(pub(super) XrdsMaterialParams);

/// Holds a pending audio load. `AudioPlayer` is NOT inserted until this component's
/// asset has loaded and its decoder has been validated. This prevents Bevy's observer
/// from panicking on unrecognised formats before we can intercept.
#[derive(Component, Debug, Clone)]
pub(super) struct XrdsStoredAudioHandle {
    pub(super) handle: bevy::asset::Handle<bevy::audio::AudioSource>,
    pub(super) uri: String,
    pub(super) playback: bevy::audio::PlaybackSettings,
}

#[derive(Component, Debug, Clone, PartialEq, Eq)]
pub(super) struct XrdsStoredEditorMetadata(pub(super) XrdsEditorMetadata);

#[derive(Component, Debug, Clone, PartialEq)]
pub(super) struct XrdsStoredSceneGltfNodeAuthoring(pub(super) XrdsSceneGltfNodeAuthoring);

/// Holds the strong Handle<Gltf> for a spawned GLTF entity so the parent
/// asset stays in Assets<Gltf> for the lifetime of the entity.
#[derive(Component, Debug, Clone)]
pub(super) struct XrdsStoredGltfHandle(pub(super) bevy::prelude::Handle<bevy::gltf::Gltf>);

/// Stores the authored HUD text payload on a spawned HUD text entity so that
/// `export_scene_node_in_world` can reconstruct the document node.
#[derive(Component, Debug, Clone)]
pub(super) struct XrdsStoredHudText(pub(super) xrds_scene_graph::XrdsSceneHudText);

/// Inserted on a `PlayerAnchor` entity when its document node carries a
/// `hud_template_id`.  Maps each HUD item's authored name to the Bevy entity
/// that renders it, so `XrdsUpdateContext::set_hud_item` can patch text at
/// runtime without a scene reimport.
#[derive(Component, Debug, Clone)]
pub struct XrdsStoredHudInstance {
    pub items: Vec<(String, Entity)>,
}


#[derive(Component, Debug, Clone, Copy)]
pub(super) struct XrdsDescriptorType(pub(super) TypeId);

#[derive(Resource)]
pub(super) struct XrdsIdAllocator {
    pub(super) next: u64,
}

#[derive(Resource, Debug, Clone, Default)]
pub(super) struct XrdsImportedAssetCatalog {
    pub(super) assets: Vec<XrdsSceneAsset>,
}

/// `(panel node entity, element name) → element entity`.
///
/// Exists because **nothing else tracks widget entities**. Elements are not
/// document nodes, so they have no `XrdsId` and `XrdsIdIndex` cannot hold them;
/// `XrdsStoredHudInstance` comes closest but is name-keyed *per anchor* and only
/// populated for head-locked panels. Closest precedent in shape is `XrdsIdIndex`.
///
/// Rebuilt by `spawn_panel_instances` on every import, and cleared there first:
/// element entities are despawned and respawned wholesale on reimport, so a
/// surviving entry would point at a dead entity and an `Element` target would
/// resolve to nothing — or worse, to a recycled id.
#[derive(Resource, Debug, Clone, Default)]
pub struct XrdsPanelElementIndex {
    pub(super) map: HashMap<(Entity, String), Entity>,
}

impl XrdsPanelElementIndex {
    pub fn element_of(&self, panel: Entity, name: &str) -> Option<Entity> {
        // Borrowing a (Entity, String) key by (&Entity, &str) needs an owned key
        // or a custom Borrow impl; the allocation happens once per resolution, at
        // Track *spawn* time rather than per frame, so it is not worth the
        // machinery to avoid.
        self.map.get(&(panel, name.to_string())).copied()
    }

    pub(super) fn insert(&mut self, panel: Entity, name: String, element: Entity) {
        self.map.insert((panel, name), element);
    }
}

/// The authored [`XrdsPanelTemplate`] registry, held so
/// `export_scene_document` can put it back. Mirrors `XrdsTrackRegistry`: a
/// `Panel` node stores only a `template_id`, so the registry is the content and
/// losing it on export empties every panel in the document.
#[derive(Resource, Debug, Clone, Default)]
pub(super) struct XrdsImportedPanelLibrary {
    pub(super) templates: Vec<xrds_scene_graph::XrdsPanelTemplate>,
}

impl Default for XrdsIdAllocator {
    fn default() -> Self {
        Self { next: 1 }
    }
}

#[derive(Resource, Default)]
pub struct XrdsIdIndex {
    pub(super) id_to_entity: HashMap<XrdsId, Entity>,
    pub(super) entity_to_id: HashMap<Entity, XrdsId>,
}

impl XrdsIdIndex {
    pub(super) fn register(&mut self, id: XrdsId, entity: Entity) {
        self.id_to_entity.insert(id, entity);
        self.entity_to_id.insert(entity, id);
    }

    /// Look up the XRDS id for a Bevy entity.
    ///
    /// Useful for viewport picking: the clicked `Entity` (from a
    /// `Pointer<Click>` event) maps back to the authored `XrdsId`,
    /// which converts to `XrdsSceneNodeId` for session operations.
    pub fn id_of(&self, entity: Entity) -> Option<XrdsId> {
        self.entity_to_id.get(&entity).copied()
    }

    /// Look up the Bevy entity for an XRDS id.
    ///
    /// Used by editor outline systems that need to add/remove render components
    /// (e.g. `Wireframe`) to the live entity when the selection changes.
    pub fn entity_of(&self, id: XrdsId) -> Option<Entity> {
        self.id_to_entity.get(&id).copied()
    }

    pub(super) fn contains_id(&self, id: XrdsId) -> bool {
        self.id_to_entity.contains_key(&id)
    }

    /// Drop an entity's registration, both directions.
    ///
    /// Added for the player body, whose collider is torn down and rebuilt when the
    /// `XrdsPlayerCamera` marker moves. Leaving a stale entry would let `entity_of`
    /// hand out a despawned entity — or worse, a recycled one.
    pub(super) fn unregister(&mut self, entity: Entity) {
        if let Some(id) = self.entity_to_id.remove(&entity) {
            self.id_to_entity.remove(&id);
        }
    }
}

#[derive(Resource, Debug, Default)]
pub(super) struct XrdsHierarchyIndex {
    parent_of: HashMap<XrdsId, Option<XrdsId>>,
    children_of: HashMap<XrdsId, Vec<XrdsId>>,
}

impl XrdsHierarchyIndex {
    fn remove_child_id(children: &mut Vec<XrdsId>, child_id: XrdsId) {
        children.retain(|candidate| *candidate != child_id);
    }

    pub(super) fn ensure_node(&mut self, id: XrdsId) {
        self.parent_of.entry(id).or_insert(None);
        self.children_of.entry(id).or_default();
    }

    pub(super) fn parent_id_of(&self, id: XrdsId) -> Option<XrdsId> {
        self.parent_of.get(&id).copied().flatten()
    }

    pub(super) fn child_ids_of(&self, id: XrdsId) -> Vec<XrdsId> {
        self.children_of.get(&id).cloned().unwrap_or_default()
    }

    pub(super) fn parent_map_snapshot(&self) -> HashMap<XrdsId, Option<XrdsId>> {
        self.parent_of.clone()
    }

    pub(super) fn set_parent(&mut self, child_id: XrdsId, parent_id: Option<XrdsId>) {
        self.ensure_node(child_id);

        let previous_parent_id = self.parent_id_of(child_id);
        if let Some(old_parent_id) = previous_parent_id {
            if Some(old_parent_id) != parent_id {
                if let Some(children) = self.children_of.get_mut(&old_parent_id) {
                    Self::remove_child_id(children, child_id);
                }
            }
        }

        self.parent_of.insert(child_id, parent_id);

        if let Some(parent_id) = parent_id {
            self.ensure_node(parent_id);
            let children = self.children_of.entry(parent_id).or_default();
            if !children.contains(&child_id) {
                children.push(child_id);
            }
        }
    }

    pub(super) fn remove_node(&mut self, id: XrdsId) -> Vec<XrdsId> {
        if let Some(Some(parent_id)) = self.parent_of.remove(&id) {
            if let Some(children) = self.children_of.get_mut(&parent_id) {
                Self::remove_child_id(children, id);
            }
        }

        let children = self.children_of.remove(&id).unwrap_or_default();
        for child_id in &children {
            self.parent_of.insert(*child_id, None);
        }

        children
    }
}

#[derive(Resource, Default)]
pub(super) struct SurfaceInterpreterRegistry {
    pub(super) interpreters: HashMap<TypeId, SurfaceInterpreter>,
    recipes: HashMap<TypeId, SurfaceRecipeInterpreter>,
}

#[derive(Resource, Default)]
pub(super) struct SurfaceDescriptorRegistry {
    cloners: HashMap<TypeId, SurfaceDescriptorCloner>,
}

impl SurfaceDescriptorRegistry {
    pub(super) fn register_clone<C>(&mut self)
    where
        C: XrdsMutableComponent + Clone + Send + Sync + 'static,
    {
        let boxed: SurfaceDescriptorCloner = Arc::new(|world, entity, name_override| {
            let stored = world.get::<XrdsStored<C>>(entity)?;
            let mut descriptor = stored.0.clone();
            if let Some(name) = name_override {
                descriptor.set_name(name.to_owned());
            }
            Some(Box::new(descriptor))
        });

        self.cloners.insert(TypeId::of::<C>(), boxed);
    }

    pub(super) fn clone_descriptor_for_entity(
        &self,
        world: &World,
        entity: Entity,
        descriptor_type: TypeId,
        name_override: Option<&str>,
    ) -> Option<Box<dyn Any + Send + Sync>> {
        let cloner = self.cloners.get(&descriptor_type)?;
        cloner(world, entity, name_override)
    }
}

impl SurfaceInterpreterRegistry {
    pub(super) fn register_recipe_only<C, F>(&mut self, interpreter: F)
    where
        C: XrdsComponent + Send + Sync + 'static,
        F: Fn(&C) -> XrdsGeometrySource + Send + Sync + 'static,
    {
        let boxed: SurfaceRecipeInterpreter = Arc::new(move |component| {
            let typed = component.downcast_ref::<C>()?;
            Some(interpreter(typed))
        });
        self.recipes.insert(TypeId::of::<C>(), boxed);
    }

    pub(super) fn register_entity<C, F>(&mut self, interpreter: F)
    where
        C: XrdsComponent + Send + Sync + 'static,
        F: Fn(&C, &mut Commands, Option<&AssetServer>) -> Entity + Send + Sync + 'static,
    {
        self.register_optional_entity::<C, _>(move |typed, commands, asset_server| {
            Some(interpreter(typed, commands, asset_server))
        });
    }

    pub(super) fn register_optional_entity<C, F>(&mut self, interpreter: F)
    where
        C: XrdsComponent + Send + Sync + 'static,
        F: Fn(&C, &mut Commands, Option<&AssetServer>) -> Option<Entity> + Send + Sync + 'static,
    {
        let boxed: SurfaceInterpreter = Arc::new(move |component, commands, asset_server| {
            let typed = component.downcast_ref::<C>()?;
            let entity = interpreter(typed, commands, asset_server)?;
            commands
                .entity(entity)
                .insert(XrdsDescriptorType(TypeId::of::<C>()));
            Some(entity)
        });
        self.interpreters.insert(TypeId::of::<C>(), boxed);
    }

    pub(super) fn register_recipe<C, F>(&mut self, interpreter: F)
    where
        C: XrdsComponent + Clone + Send + Sync + 'static,
        F: Fn(&C) -> XrdsGeometrySource + Send + Sync + 'static,
    {
        let interpreter = Arc::new(interpreter);
        let recipe_interpreter = Arc::clone(&interpreter);

        self.register_recipe_only::<C, _>(move |typed| recipe_interpreter.as_ref()(typed));

        self.register_entity::<C, _>(move |typed, commands, _asset_server| {
            let name = typed.name().to_string();
            let transform = *typed.local_transform();
            let visible = typed.is_visible();
            let entity = execute_spawn_recipe(
                commands,
                interpreter.as_ref()(typed),
                name,
                transform,
                visible,
            );
            commands.entity(entity).insert(XrdsStored(typed.clone()));
            entity
        });
    }

    pub(super) fn interpreter_for<C>(&self) -> Option<SurfaceInterpreter>
    where
        C: XrdsComponent + Send + Sync + 'static,
    {
        self.interpreters.get(&TypeId::of::<C>()).cloned()
    }

    pub(super) fn interpret(
        &self,
        component: &dyn Any,
        commands: &mut Commands,
        asset_server: Option<&AssetServer>,
    ) -> Option<Entity> {
        let Some(interpreter) = self.interpreters.get(&component.type_id()) else {
            return None;
        };
        interpreter(component, commands, asset_server)
    }

    pub(super) fn recipe_for_component(&self, component: &dyn Any) -> Option<XrdsGeometrySource> {
        let recipe = self.recipes.get(&component.type_id())?;
        recipe(component)
    }
}

#[derive(Resource, Default)]
pub(super) struct QueuedSurfaceComponents {
    pub(super) components: Vec<QueuedSurfaceComponent>,
}

#[derive(Resource, Default)]
pub(super) struct QueuedParentChanges {
    pub(super) changes: Vec<QueuedParentChange>,
}

pub(super) struct QueuedSurfaceComponent {
    pub(super) id: XrdsId,
    pub(super) component: Box<dyn Any + Send + Sync>,
    pub(super) parent_id: Option<XrdsId>,
}

pub(super) struct QueuedParentChange {
    pub(super) child_id: XrdsId,
    pub(super) parent_id: Option<XrdsId>,
}

pub(super) type SurfaceUpdater = Arc<dyn Fn(&mut World, Entity, &dyn Any) + Send + Sync>;

pub(super) struct QueuedSurfaceUpdate {
    pub(super) entity: Entity,
    pub(super) component_type: TypeId,
    pub(super) patch_type: TypeId,
    pub(super) patch: Box<dyn Any + Send + Sync>,
}

#[derive(Resource, Default)]
pub(super) struct QueuedSurfaceUpdates {
    pub(super) updates: Vec<QueuedSurfaceUpdate>,
}

#[derive(Resource, Default)]
pub(super) struct XrdsInstalled;

#[derive(Resource, Default)]
pub(super) struct SurfaceUpdateRegistry {
    updaters: HashMap<(TypeId, TypeId), SurfaceUpdater>,
}

impl SurfaceUpdateRegistry {
    pub(super) fn register<C, P, F>(&mut self, updater: F)
    where
        C: XrdsComponent + Send + Sync + 'static,
        P: Send + Sync + 'static,
        F: Fn(&mut World, Entity, &P) + Send + Sync + 'static,
    {
        let key = (TypeId::of::<C>(), TypeId::of::<P>());
        let boxed: SurfaceUpdater = Arc::new(move |world, entity, patch| {
            let Some(typed_patch) = patch.downcast_ref::<P>() else {
                return;
            };
            updater(world, entity, typed_patch);
        });
        self.updaters.insert(key, boxed);
    }

    pub(super) fn updater_for(
        &self,
        component_type: TypeId,
        patch_type: TypeId,
    ) -> Option<SurfaceUpdater> {
        self.updaters.get(&(component_type, patch_type)).cloned()
    }
}

pub(super) fn apply_transform_to_entity(
    world: &mut World,
    entity: Entity,
    params: TransformParams,
) {
    if let Some(mut t) = world.get_mut::<Transform>(entity) {
        t.translation = Vec3::from_array(params.translation);
        let [x, y, z, w] = params.rotation_quat_xyzw;
        t.rotation = Quat::from_xyzw(x, y, z, w);
        t.scale = Vec3::from_array(params.scale);
    }
}

fn transform_params_from_transform(transform: &Transform) -> TransformParams {
    TransformParams {
        translation: transform.translation.to_array(),
        rotation_quat_xyzw: [
            transform.rotation.x,
            transform.rotation.y,
            transform.rotation.z,
            transform.rotation.w,
        ],
        scale: transform.scale.to_array(),
    }
}

pub(super) fn transform_params_for_entity(
    world: &World,
    entity: Entity,
) -> Option<TransformParams> {
    world
        .get::<Transform>(entity)
        .map(transform_params_from_transform)
}

pub(super) fn apply_surface_patch_now<C, P>(world: &mut World, entity: Entity, patch: P) -> bool
where
    C: XrdsComponent + Send + Sync + 'static,
    P: Send + Sync + 'static,
{
    let updater = {
        let registry = world.resource::<SurfaceUpdateRegistry>();
        registry.updater_for(TypeId::of::<C>(), TypeId::of::<P>())
    };

    if let Some(updater) = updater {
        updater(world, entity, &patch);
        true
    } else {
        false
    }
}

pub(super) fn descriptor_type_of(world: &World, entity: Entity) -> Option<TypeId> {
    world
        .get::<XrdsDescriptorType>(entity)
        .map(|descriptor| descriptor.0)
}

pub(super) fn with_stored_descriptor_mut<C, R>(
    world: &mut World,
    entity: Entity,
    apply: impl FnOnce(&mut C) -> R,
) -> Option<R>
where
    C: XrdsComponent + Send + Sync + 'static,
{
    if let Some(mut descriptor) = world.get_mut::<XrdsStored<C>>(entity) {
        return Some(apply(&mut descriptor.0));
    }

    None
}

pub(super) fn cylinder_descriptor_ref(world: &World, entity: Entity) -> Option<&XrdsCylinder> {
    world
        .get::<XrdsStored<XrdsCylinder>>(entity)
        .map(|descriptor| &descriptor.0)
}

pub(super) fn capsule_descriptor_ref(world: &World, entity: Entity) -> Option<&XrdsCapsule> {
    world
        .get::<XrdsStored<XrdsCapsule>>(entity)
        .map(|descriptor| &descriptor.0)
}

pub(super) fn cube_descriptor_ref(world: &World, entity: Entity) -> Option<&XrdsCube> {
    world
        .get::<XrdsStored<XrdsCube>>(entity)
        .map(|descriptor| &descriptor.0)
}

pub(super) fn sphere_descriptor_ref(world: &World, entity: Entity) -> Option<&XrdsSphere> {
    world
        .get::<XrdsStored<XrdsSphere>>(entity)
        .map(|descriptor| &descriptor.0)
}

pub(super) fn plane_descriptor_ref(world: &World, entity: Entity) -> Option<&XrdsPlane3D> {
    world
        .get::<XrdsStored<XrdsPlane3D>>(entity)
        .map(|descriptor| &descriptor.0)
}
