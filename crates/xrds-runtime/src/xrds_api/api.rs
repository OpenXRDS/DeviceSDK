use super::*;

impl XrdsAPI<'_> {
    pub fn attach(app: &mut App) -> XrdsAPI<'_> {
        install_xrds(app);
        XrdsAPI { app }
    }

    /// Spawn a descriptor immediately and return a typed handle to the created entity.
    ///
    /// This is the direct runtime path. For editor-driven creation flows that need batching,
    /// ordering, or document-first staging, prefer [`Self::queue_spawn`].
    pub fn spawn<C>(&mut self, descriptor: &C) -> Handle<C>
    where
        C: XrdsComponent + Send + Sync + 'static,
    {
        let id = self.new_id();
        self.spawn_with_reserved_id(id, descriptor)
    }

    /// Spawn a descriptor immediately with an explicit XRDS id.
    ///
    /// This is the id-preserving runtime import path for editor documents and other authored data
    /// sources that already own stable ids.
    pub fn spawn_with_id<C>(
        &mut self,
        id: XrdsId,
        descriptor: &C,
    ) -> Result<Handle<C>, XrdsSceneImportError>
    where
        C: XrdsComponent + Send + Sync + 'static,
    {
        reserve_runtime_id_in_world(self.app.world_mut(), id)?;
        Ok(self.spawn_with_reserved_id(id, descriptor))
    }

    fn spawn_with_reserved_id<C>(&mut self, id: XrdsId, descriptor: &C) -> Handle<C>
    where
        C: XrdsComponent + Send + Sync + 'static,
    {
        let world = self.app.world_mut();
        let asset_server = world.get_resource::<AssetServer>();
        let interpreter = world
            .resource::<SurfaceInterpreterRegistry>()
            .interpreter_for::<C>()
            .unwrap_or_else(|| {
                panic!(
                    "No surface interpreter registered for type {}. Call register_surface_interpreter::<T>(...) first.",
                    std::any::type_name::<C>()
                )
            });

        let mut command_queue = CommandQueue::default();
        let entity = {
            let mut commands = Commands::new(&mut command_queue, world);
            interpreter(descriptor as &dyn Any, &mut commands, asset_server).unwrap_or_else(|| {
                panic!(
                    "Surface interpreter failed for type {}.",
                    std::any::type_name::<C>()
                )
            })
        };
        command_queue.apply(world);
        world.resource_mut::<XrdsIdIndex>().register(id, entity);
        world.resource_mut::<XrdsHierarchyIndex>().ensure_node(id);

        Handle::from(entity)
    }

    /// Allocate a unique XRDS id.
    pub fn new_id(&mut self) -> XrdsId {
        let world = self.app.world_mut();

        let mut candidate = {
            let allocator = world.resource::<XrdsIdAllocator>();
            allocator.next
        };

        while world
            .resource::<XrdsIdIndex>()
            .contains_id(XrdsId(candidate))
        {
            candidate += 1;
        }

        world.resource_mut::<XrdsIdAllocator>().next = candidate.saturating_add(1);

        XrdsId(candidate)
    }

    /// Resolve XRDS id from a typed handle.
    pub fn id_of<C>(&self, handle: &Handle<C>) -> Option<XrdsId> {
        self.app
            .world()
            .resource::<XrdsIdIndex>()
            .id_of(handle.entity())
    }

    /// Resolve a Bevy entity from an XRDS id.
    pub fn entity_of_id(&self, id: XrdsId) -> Option<Entity> {
        self.app.world().resource::<XrdsIdIndex>().entity_of(id)
    }

    /// Resolve a typed handle from an XRDS id.
    pub fn handle_of<C>(&self, id: XrdsId) -> Option<Handle<C>> {
        self.entity_of_id(id).map(Handle::from)
    }

    /// Resolve the authored XRDS parent id for a spawned entity.
    pub fn parent_id_of<C>(&self, handle: &Handle<C>) -> Option<XrdsId> {
        let id = self.id_of(handle)?;
        self.app
            .world()
            .resource::<XrdsHierarchyIndex>()
            .parent_id_of(id)
    }

    /// Resolve derived XRDS child ids for a spawned entity.
    pub fn child_ids_of<C>(&self, handle: &Handle<C>) -> Vec<XrdsId> {
        let Some(id) = self.id_of(handle) else {
            return Vec::new();
        };

        self.app
            .world()
            .resource::<XrdsHierarchyIndex>()
            .child_ids_of(id)
    }

    /// Rename an entity through XRDS and keep the descriptor name in sync.
    ///
    /// This is a queued commit helper. Prefer it for authoritative editor renames rather than
    /// directly mutating the Bevy [`Name`] component.
    pub fn rename<C>(&mut self, handle: &Handle<C>, name: impl Into<String>) -> &mut Self
    where
        C: XrdsMutableComponent + Send + Sync + 'static,
    {
        self.queue_update(handle, NamePatch { name: name.into() })
    }

    /// Change runtime visibility through XRDS and keep the descriptor state in sync.
    ///
    /// This is a queued commit helper for document-backed visibility changes.
    pub fn set_visible<C>(&mut self, handle: &Handle<C>, visible: bool) -> &mut Self
    where
        C: XrdsMutableComponent + Send + Sync + 'static,
    {
        self.queue_update(handle, VisibilityPatch { visible })
    }

    /// Queue a descriptor to spawn as a Bevy entity during startup.
    ///
    /// This is the preferred path for batched editor creation flows where object creation should
    /// be staged, ordered, and committed coherently.
    pub fn queue_spawn<C>(&mut self, component: C) -> XrdsId
    where
        C: XrdsComponent + Send + Sync + 'static,
    {
        self.queue_spawn_under(None, component)
    }

    /// Queue a descriptor to spawn with an explicit XRDS id.
    ///
    /// This is the id-preserving queued import path for document-backed creation flows.
    pub fn queue_spawn_with_id<C>(
        &mut self,
        id: XrdsId,
        component: C,
    ) -> Result<XrdsId, XrdsSceneImportError>
    where
        C: XrdsComponent + Send + Sync + 'static,
    {
        reserve_runtime_id_in_world(self.app.world_mut(), id)?;
        let mut queue = self
            .app
            .world_mut()
            .get_resource_or_insert_with(QueuedSurfaceComponents::default);
        queue.components.push(QueuedSurfaceComponent {
            id,
            component: Box::new(component),
            parent_id: None,
        });
        Ok(id)
    }

    fn queue_spawn_under<C>(&mut self, parent_id: Option<XrdsId>, component: C) -> XrdsId
    where
        C: XrdsComponent + Send + Sync + 'static,
    {
        let id = self.new_id();
        let mut queue = self
            .app
            .world_mut()
            .get_resource_or_insert_with(QueuedSurfaceComponents::default);
        queue.components.push(QueuedSurfaceComponent {
            id,
            component: Box::new(component),
            parent_id,
        });
        id
    }

    /// Queue multiple descriptors to spawn as Bevy entities during startup.
    ///
    /// Use this for multi-object add/paste/duplicate flows where creation should be treated as a
    /// single editor action.
    pub fn queue_spawn_many<C, I>(&mut self, components: I) -> Vec<XrdsId>
    where
        C: XrdsComponent + Send + Sync + 'static,
        I: IntoIterator<Item = C>,
    {
        let mut ids = Vec::new();
        for component in components {
            ids.push(self.queue_spawn(component));
        }
        ids
    }

    /// Queue a parent assignment for a startup-spawned or live XRDS entity.
    ///
    /// Structural hierarchy edits should stay queued so they can be applied in a consistent batch.
    pub fn queue_set_parent(&mut self, child_id: XrdsId, parent_id: Option<XrdsId>) -> &mut Self {
        self.app
            .world_mut()
            .resource_mut::<QueuedParentChanges>()
            .changes
            .push(QueuedParentChange {
                child_id,
                parent_id,
            });
        self
    }

    /// Import already-converted scene runtime nodes, preserving XRDS ids, parent links, and
    /// mesh materials.
    pub fn import_runtime_nodes<I>(&mut self, nodes: I) -> Result<Vec<XrdsId>, XrdsSceneImportError>
    where
        I: IntoIterator<Item = XrdsSceneRuntimeNode>,
    {
        let mut imported_ids = Vec::new();
        let mut parent_changes = Vec::new();
        let mut material_updates = Vec::new();
        let mut playback_requests = Vec::new();
        let mut morph_override_entities = Vec::new();

        for node in nodes {
            let XrdsSceneRuntimeNode {
                id,
                parent_id,
                component,
                material,
                editor,
                gltf_node_authoring,
            } = node;

            let is_gltf_component = matches!(&component, XrdsSceneRuntimeComponent::GltfAsset(_));

            let entity = match component {
                XrdsSceneRuntimeComponent::Node(component) => {
                    self.spawn_with_id(id, &component)?.entity()
                }
                XrdsSceneRuntimeComponent::Camera(component) => {
                    self.spawn_with_id(id, &component)?.entity()
                }
                XrdsSceneRuntimeComponent::GltfAsset(component) => {
                    self.spawn_with_id(id, &component)?.entity()
                }
                XrdsSceneRuntimeComponent::Cube(component) => {
                    self.spawn_with_id(id, &component)?.entity()
                }
                XrdsSceneRuntimeComponent::Cylinder(component) => {
                    self.spawn_with_id(id, &component)?.entity()
                }
                XrdsSceneRuntimeComponent::Sphere(component) => {
                    self.spawn_with_id(id, &component)?.entity()
                }
                XrdsSceneRuntimeComponent::Plane3D(component) => {
                    self.spawn_with_id(id, &component)?.entity()
                }
                XrdsSceneRuntimeComponent::Tetrahedron(component) => {
                    self.spawn_with_id(id, &component)?.entity()
                }
                XrdsSceneRuntimeComponent::AmbientLight(component) => {
                    self.spawn_with_id(id, &component)?.entity()
                }
                XrdsSceneRuntimeComponent::DirectionalLight(component) => {
                    self.spawn_with_id(id, &component)?.entity()
                }
                XrdsSceneRuntimeComponent::PointLight(component) => {
                    self.spawn_with_id(id, &component)?.entity()
                }
                XrdsSceneRuntimeComponent::SpotLight(component) => {
                    self.spawn_with_id(id, &component)?.entity()
                }
            };

            self.app
                .world_mut()
                .entity_mut(entity)
                .insert(XrdsStoredEditorMetadata(editor));

            if is_gltf_component {
                if let Some(authoring) = gltf_node_authoring {
                    if let Some(playback) = authoring.default_playback.clone() {
                        let selector = playback.selector.clone().into();
                        let options = playback.into();
                        playback_requests.push((
                            Handle::<XrdsGltfAsset>::from(entity),
                            selector,
                            options,
                        ));
                    }

                    if !authoring.morph_target_overrides.is_empty() {
                        morph_override_entities.push(entity);
                    }

                    self.app
                        .world_mut()
                        .entity_mut(entity)
                        .insert(XrdsStoredSceneGltfNodeAuthoring(authoring));
                }
            }

            if let Some(parent_id) = parent_id {
                parent_changes.push(QueuedParentChange {
                    child_id: id,
                    parent_id: Some(parent_id),
                });
            }

            if let Some(material) = material {
                material_updates.push((entity, material));
            }

            imported_ids.push(id);
        }

        let world = self.app.world_mut();
        for (entity, material) in material_updates {
            set_material_params_for_entity_in_world(world, entity, material);
        }
        apply_parent_changes(world, parent_changes);

        for (handle, selector, options) in playback_requests {
            world
                .resource_mut::<PendingGltfAnimationRequests>()
                .requests
                .insert(
                    handle.entity(),
                    PendingGltfAnimationRequest { selector, options },
                );
        }

        if !morph_override_entities.is_empty() {
            let mut pending = world.resource_mut::<PendingGltfMorphTargetOverrideRequests>();
            for entity in morph_override_entities {
                pending.entities.insert(entity);
            }
        }

        apply_pending_gltf_animation_requests_system(world);
        apply_pending_gltf_morph_target_override_requests_system(world);

        Ok(imported_ids)
    }

    /// Import a scene document into XRDS runtime state, preserving scene-document ids and
    /// hierarchy.
    pub fn import_scene_document(
        &mut self,
        document: &XrdsSceneDocument,
    ) -> Result<Vec<XrdsId>, XrdsSceneImportError> {
        let runtime_nodes = document.to_runtime_nodes()?;
        self.import_runtime_nodes(runtime_nodes)
    }

    /// Export the current XRDS-authored runtime state to a scene document.
    ///
    /// This exports built-in XRDS descriptors, stable ids, parent links, authored materials,
    /// preserved editor metadata, and glTF references from XRDS-owned runtime state. The document
    /// asset table is reconstructed for built-in glTF asset references, reusing preserved editor
    /// asset ids when available and generating deterministic ids otherwise.
    pub fn export_scene_document(&self) -> Result<XrdsSceneDocument, XrdsSceneExportError> {
        self.export_scene_document_with_metadata(XrdsSceneMetadata {
            name: "XRDS Runtime Scene".to_string(),
            ..Default::default()
        })
    }

    /// Export the current XRDS-authored runtime state to a scene document with caller-supplied
    /// scene metadata.
    pub fn export_scene_document_with_metadata(
        &self,
        metadata: XrdsSceneMetadata,
    ) -> Result<XrdsSceneDocument, XrdsSceneExportError> {
        export_scene_document_in_world(self.app.world(), metadata)
    }

    /// Register a low-level patch updater for a descriptor type.
    ///
    /// This is the expert escape hatch: the closure receives raw Bevy [`World`] access and is
    /// responsible for keeping runtime components and the stored XRDS descriptor in sync.
    pub fn register_updater<C, P, F>(&mut self, updater: F) -> &mut Self
    where
        C: XrdsComponent + Send + Sync + 'static,
        P: Send + Sync + 'static,
        F: Fn(&mut World, Entity, &P) + Send + Sync + 'static,
    {
        self.app
            .world_mut()
            .resource_mut::<SurfaceUpdateRegistry>()
            .register::<C, P, F>(updater);
        self
    }

    /// Register a high-level updater for recipe-backed custom surfaces.
    ///
    /// The closure only mutates the XRDS descriptor. After mutation, XRDS recomputes the
    /// surface recipe from the descriptor registered via [`Self::register_surface_interpreter`]
    /// and synchronizes the live entity's name, transform, visibility, mesh/material, or scene.
    ///
    /// Use this when you want custom patch support without direct Bevy [`World`] access.
    /// Types without a recipe registered through [`Self::register_surface_interpreter`] are not
    /// supported by this helper because XRDS cannot reconstruct arbitrary runtime state from a
    /// descriptor alone.
    pub fn register_recipe_updater<C, P, F>(&mut self, updater: F) -> &mut Self
    where
        C: XrdsComponent + Send + Sync + 'static,
        P: Send + Sync + 'static,
        F: Fn(&mut C, &P) + Send + Sync + 'static,
    {
        self.register_updater::<C, P, _>(move |world, entity, patch| {
            if let Some(mut descriptor) = world.get_mut::<XrdsStored<C>>(entity) {
                updater(&mut descriptor.0, patch);
            } else {
                return;
            }

            let Some(descriptor) = world
                .get::<XrdsStored<C>>(entity)
                .map(|stored| &stored.0)
            else {
                return;
            };

            let Some(recipe) = world
                .resource::<SurfaceInterpreterRegistry>()
                .recipe_for_component(descriptor)
            else {
                warn!(
                    "No recipe-backed surface interpreter registered for type {}; skipping high-level patch sync",
                    std::any::type_name::<C>()
                );
                return;
            };

            apply_spawn_recipe_to_entity(
                world,
                entity,
                recipe,
                descriptor.name().to_string(),
                *descriptor.local_transform(),
                descriptor.is_visible(),
            );
        })
    }

    /// Queue an authoritative XRDS patch for a live entity.
    ///
    /// Prefer this for committed editor changes. Immediate helpers such as [`Self::set_translation`]
    /// and [`Self::set_material_base_color`] are intended for interactive preview; this method is
    /// the document/undo-friendly path for the final committed edit.
    pub fn queue_update<C, P>(&mut self, handle: &Handle<C>, patch: P) -> &mut Self
    where
        C: XrdsComponent + Send + Sync + 'static,
        P: Send + Sync + 'static,
    {
        self.app
            .world_mut()
            .resource_mut::<QueuedSurfaceUpdates>()
            .updates
            .push(QueuedSurfaceUpdate {
                entity: handle.entity(),
                component_type: TypeId::of::<C>(),
                patch_type: TypeId::of::<P>(),
                patch: Box::new(patch),
            });
        self
    }

    /// Queue a camera projection update.
    pub fn set_camera_projection(
        &mut self,
        handle: &Handle<XrdsCamera>,
        projection: CameraProjectionParams,
    ) -> &mut Self {
        self.queue_update(handle, CameraProjectionPatch { projection })
    }

    /// Queue a perspective camera projection update.
    pub fn set_camera_perspective(
        &mut self,
        handle: &Handle<XrdsCamera>,
        params: PerspectiveCameraParams,
    ) -> &mut Self {
        self.set_camera_projection(handle, CameraProjectionParams::Perspective(params))
    }

    /// Queue an orthographic camera projection update.
    pub fn set_camera_orthographic(
        &mut self,
        handle: &Handle<XrdsCamera>,
        params: OrthographicCameraParams,
    ) -> &mut Self {
        self.set_camera_projection(handle, CameraProjectionParams::Orthographic(params))
    }

    /// Queue a camera look-at update.
    pub fn set_camera_look_at(
        &mut self,
        handle: &Handle<XrdsCamera>,
        target: Option<[f32; 3]>,
    ) -> &mut Self {
        self.queue_update(handle, CameraLookAtPatch { look_at: target })
    }

    /// Queue a glTF source update.
    pub fn set_gltf_source(
        &mut self,
        handle: &Handle<XrdsGltfAsset>,
        gltf_asset_path: impl Into<String>,
        scene_index: usize,
    ) -> &mut Self {
        self.queue_update(
            handle,
            GltfAssetSourcePatch {
                gltf_asset_path: gltf_asset_path.into(),
                scene_index,
            },
        )
    }

    /// Queue a point light parameter update.
    pub fn set_point_light_params(
        &mut self,
        handle: &Handle<XrdsPointLight>,
        params: PointLightParams,
    ) -> &mut Self {
        self.queue_update(handle, params)
    }

    /// Queue a directional light parameter update.
    pub fn set_directional_light_params(
        &mut self,
        handle: &Handle<XrdsDirectionalLight>,
        params: DirectionalLightParams,
    ) -> &mut Self {
        self.queue_update(handle, params)
    }

    /// Queue a spot light parameter update.
    pub fn set_spot_light_params(
        &mut self,
        handle: &Handle<XrdsSpotLight>,
        params: SpotLightParams,
    ) -> &mut Self {
        self.queue_update(handle, params)
    }

    /// Queue an ambient light parameter update.
    pub fn set_ambient_light_params(
        &mut self,
        handle: &Handle<XrdsAmbientLight>,
        params: AmbientLightParams,
    ) -> &mut Self {
        self.queue_update(handle, params)
    }

    /// Queue a cube geometry update.
    pub fn set_cube_geometry(
        &mut self,
        handle: &Handle<XrdsCube>,
        params: CubeGeometryParams,
    ) -> &mut Self {
        self.queue_update(handle, params)
    }

    /// Queue a cylinder geometry update.
    pub fn set_cylinder_geometry(
        &mut self,
        handle: &Handle<XrdsCylinder>,
        params: CylinderGeometryParams,
    ) -> &mut Self {
        self.queue_update(handle, params)
    }

    /// Queue a sphere geometry update.
    pub fn set_sphere_geometry(
        &mut self,
        handle: &Handle<XrdsSphere>,
        params: SphereGeometryParams,
    ) -> &mut Self {
        self.queue_update(handle, params)
    }

    /// Queue a plane geometry update.
    pub fn set_plane_geometry(
        &mut self,
        handle: &Handle<XrdsPlane3D>,
        params: Plane3DGeometryParams,
    ) -> &mut Self {
        self.queue_update(handle, params)
    }

    /// Queue a tetrahedron geometry update.
    pub fn set_tetrahedron_geometry(
        &mut self,
        handle: &Handle<XrdsTetrahedron>,
        params: TetrahedronGeometryParams,
    ) -> &mut Self {
        self.queue_update(handle, params)
    }

    /// Set a parent for a live entity. `None` detaches it to the scene root.
    ///
    /// This is a queued structural commit helper.
    pub fn set_parent<C>(&mut self, handle: &Handle<C>, parent_id: Option<XrdsId>) -> &mut Self
    where
        C: XrdsMutableComponent + Send + Sync + 'static,
    {
        self.queue_update(handle, ParentPatch { parent_id })
    }

    /// Delete an entity and its XRDS child subtree.
    ///
    /// This is an authoritative structural edit and should be treated like a document commit.
    pub fn delete<C>(&mut self, handle: &Handle<C>) -> bool
    where
        C: XrdsMutableComponent + Send + Sync + 'static,
    {
        let Some(root_id) = self.id_of(handle) else {
            return false;
        };

        let world = self.app.world_mut();

        let subtree_ids = collect_subtree_ids(world, root_id);
        let entities: Vec<Entity> = {
            let ids = world.resource::<XrdsIdIndex>();
            subtree_ids
                .iter()
                .filter_map(|id| ids.entity_of(*id))
                .collect()
        };

        unregister_entities(world, &entities);

        for entity in entities.into_iter().rev() {
            world.entity_mut(entity).despawn();
        }

        true
    }

    /// Duplicate an entity subtree, preserving internal hierarchy and returning the new root handle.
    ///
    /// This is an authoritative structural edit and should be treated like a document commit.
    pub fn duplicate<C>(&mut self, handle: &Handle<C>) -> Option<Handle<C>>
    where
        C: XrdsMutableComponent + Clone + Send + Sync + 'static,
    {
        let root_id = self.id_of(handle)?;
        let root_parent_id = self.parent_id_of(handle);

        let subtree_ids = {
            let world = self.app.world_mut();
            collect_subtree_ids(world, root_id)
        };

        let mut duplicated_ids = HashMap::new();
        for old_id in &subtree_ids {
            duplicated_ids.insert(*old_id, self.new_id());
        }

        let duplicated_root_id = duplicated_ids.get(&root_id).copied()?;

        let duplicated_descriptors = {
            let world = self.app.world();
            let mut clones = Vec::new();

            for old_id in &subtree_ids {
                let entity = world.resource::<XrdsIdIndex>().entity_of(*old_id)?;
                let parent_id = world.resource::<XrdsHierarchyIndex>().parent_id_of(*old_id);
                let new_id = duplicated_ids.get(old_id).copied()?;
                let new_parent_id = if *old_id == root_id {
                    root_parent_id
                } else {
                    parent_id.and_then(|parent_id| duplicated_ids.get(&parent_id).copied())
                };
                let name_override = if *old_id == root_id {
                    Some(duplicate_name(
                        world
                            .get::<Name>(entity)
                            .map(|name| name.as_str())
                            .unwrap_or("Object"),
                    ))
                } else {
                    None
                };

                let boxed = clone_boxed_descriptor(world, entity, name_override.as_deref())?;
                let material = material_params_for_entity_in_world(world, entity);
                clones.push((new_id, boxed, new_parent_id, material));
            }

            clones
        };

        let world = self.app.world_mut();
        let mut new_root_entity = None;
        for (id, descriptor, parent_id, material) in duplicated_descriptors {
            let entity = spawn_boxed_surface_component(world, id, descriptor, parent_id)?;
            if let Some(material) = material {
                set_material_params_for_entity_in_world(world, entity, material);
            }
            if id == duplicated_root_id {
                new_root_entity = Some(entity);
            }
        }

        new_root_entity.map(Handle::from)
    }

    /// Async-friendly wrapper over [`Self::queue_update`].
    ///
    /// Useful for app code that already runs in async contexts: enqueue immediately,
    /// then let XRDS flush changes on the next frame.
    pub async fn update<C, P>(&mut self, handle: &Handle<C>, patch: P)
    where
        C: XrdsComponent + Send + Sync + 'static,
        P: Send + Sync + 'static,
    {
        self.queue_update(handle, patch);
    }

    /// Register a per-frame Bevy system (expert escape hatch).
    pub fn add_update_system<M>(
        &mut self,
        system: impl IntoScheduleConfigs<bevy::ecs::system::ScheduleSystem, M>,
    ) -> &mut Self {
        self.app.add_systems(Update, system);
        self
    }

    /// Register a one-shot startup system (runs once at `Startup`).
    pub fn add_startup_system<M>(
        &mut self,
        system: impl IntoScheduleConfigs<bevy::ecs::system::ScheduleSystem, M>,
    ) -> &mut Self {
        self.app.add_systems(Startup, system);
        self
    }

    /// Insert an application-level resource accessible from any system.
    pub fn insert_resource<R: Resource>(&mut self, resource: R) -> &mut Self {
        self.app.insert_resource(resource);
        self
    }

    // ── Transform / visibility setters ───────────────────────────────────────

    /// Overwrite the full transform of a spawned entity from [`TransformParams`].
    ///
    /// This is the immediate preview path. In an editor, use it while dragging/manipulating,
    /// then persist the final committed transform through [`Self::queue_update`].
    pub fn set_transform<C>(&mut self, handle: &Handle<C>, params: TransformParams)
    where
        C: XrdsComponent + Send + Sync + 'static,
    {
        set_transform_in_world(self.app.world_mut(), handle, params);
    }

    /// Set only the world-space translation of a spawned entity.
    ///
    /// This is intended for interactive preview updates rather than document commits.
    pub fn set_translation<C>(&mut self, handle: &Handle<C>, translation: [f32; 3])
    where
        C: XrdsComponent + Send + Sync + 'static,
    {
        set_translation_in_world(self.app.world_mut(), handle, translation);
    }

    /// Set only the rotation of a spawned entity (quaternion `[x, y, z, w]`).
    ///
    /// This is intended for interactive preview updates rather than document commits.
    pub fn set_rotation<C>(&mut self, handle: &Handle<C>, xyzw: [f32; 4])
    where
        C: XrdsComponent + Send + Sync + 'static,
    {
        set_rotation_in_world(self.app.world_mut(), handle, xyzw);
    }

    /// Set only the non-uniform scale of a spawned entity.
    ///
    /// This is intended for interactive preview updates rather than document commits.
    pub fn set_scale<C>(&mut self, handle: &Handle<C>, scale: [f32; 3])
    where
        C: XrdsComponent + Send + Sync + 'static,
    {
        set_scale_in_world(self.app.world_mut(), handle, scale);
    }

    /// Show or hide a spawned entity.
    ///
    /// This mutates runtime state immediately. For authoritative editor visibility changes,
    /// prefer the queued [`Self::set_visible`] helper above.
    pub fn set_visibility<C>(&mut self, handle: &Handle<C>, visible: bool)
    where
        C: XrdsComponent + Send + Sync + 'static,
    {
        set_visibility_in_world(self.app.world_mut(), handle, visible);
    }

    /// Read material base color for mesh-based entities (e.g. cube, cylinder).
    pub fn material_base_color<C>(&self, handle: &Handle<C>) -> Option<XrdsColor> {
        material_base_color_in_world(self.app.world(), handle)
    }

    /// Read authored material parameters for mesh-based entities.
    pub fn material_params<C>(&self, handle: &Handle<C>) -> Option<XrdsMaterialParams> {
        material_params_in_world(self.app.world(), handle)
    }

    /// Read advanced XRDS-native PBR settings for mesh-based entities.
    pub fn material_pbr_params<C>(&self, handle: &Handle<C>) -> Option<XrdsMaterialPbrParams> {
        material_pbr_params_in_world(self.app.world(), handle)
    }

    /// Set material base color for mesh-based entities (e.g. cube, cylinder).
    ///
    /// This is the immediate preview path for inspector interactions.
    pub fn set_material_base_color<C>(&mut self, handle: &Handle<C>, color: XrdsColor)
    where
        C: XrdsComponent + Send + Sync + 'static,
    {
        set_material_base_color_in_world(self.app.world_mut(), handle, color);
    }

    /// Read material emissive color for mesh-based entities.
    pub fn material_emissive<C>(&self, handle: &Handle<C>) -> Option<XrdsLinearRgba> {
        material_emissive_in_world(self.app.world(), handle)
    }

    /// Set material emissive color for mesh-based entities.
    ///
    /// This is the immediate preview path for inspector interactions.
    pub fn set_material_emissive<C>(&mut self, handle: &Handle<C>, emissive: XrdsLinearRgba)
    where
        C: XrdsComponent + Send + Sync + 'static,
    {
        set_material_emissive_in_world(self.app.world_mut(), handle, emissive);
    }

    /// Set advanced XRDS-native PBR settings for mesh-based entities.
    ///
    /// This is the immediate preview path for inspector-driven advanced material previews.
    pub fn set_material_pbr_params<C>(&mut self, handle: &Handle<C>, pbr: XrdsMaterialPbrParams)
    where
        C: XrdsComponent + Send + Sync + 'static,
    {
        set_material_pbr_params_in_world(self.app.world_mut(), handle, pbr);
    }

    /// Set authored material parameters for mesh-based entities.
    ///
    /// This is the immediate preview path for inspector interactions.
    pub fn set_material_params<C>(&mut self, handle: &Handle<C>, params: XrdsMaterialParams)
    where
        C: XrdsComponent + Send + Sync + 'static,
    {
        set_material_params_in_world(self.app.world_mut(), handle, params);
    }

    pub fn gltf_load_status(&self, handle: &Handle<XrdsGltfAsset>) -> Option<XrdsGltfLoadStatus> {
        gltf_load_status_in_world(self.app.world(), handle)
    }

    pub fn gltf_animations(
        &self,
        handle: &Handle<XrdsGltfAsset>,
    ) -> Result<Vec<XrdsGltfAnimationInfo>, XrdsGltfRuntimeError> {
        gltf_animations_in_world(self.app.world(), handle)
    }

    pub fn play_gltf_animation(
        &mut self,
        handle: &Handle<XrdsGltfAsset>,
        selector: XrdsGltfAnimationSelector,
        options: XrdsGltfAnimationPlaybackOptions,
    ) -> Result<(), XrdsGltfRuntimeError> {
        play_gltf_animation_in_world(self.app.world_mut(), handle, selector, options)
    }

    pub fn stop_gltf_animation(
        &mut self,
        handle: &Handle<XrdsGltfAsset>,
    ) -> Result<(), XrdsGltfRuntimeError> {
        stop_gltf_animation_in_world(self.app.world_mut(), handle)
    }

    pub fn pause_gltf_animation(
        &mut self,
        handle: &Handle<XrdsGltfAsset>,
    ) -> Result<(), XrdsGltfRuntimeError> {
        pause_gltf_animation_in_world(self.app.world_mut(), handle)
    }

    pub fn resume_gltf_animation(
        &mut self,
        handle: &Handle<XrdsGltfAsset>,
    ) -> Result<(), XrdsGltfRuntimeError> {
        resume_gltf_animation_in_world(self.app.world_mut(), handle)
    }

    pub fn gltf_animation_state(
        &self,
        handle: &Handle<XrdsGltfAsset>,
    ) -> Result<Option<XrdsGltfAnimationState>, XrdsGltfRuntimeError> {
        gltf_animation_state_in_world(self.app.world(), handle)
    }

    pub fn gltf_morph_targets(
        &self,
        handle: &Handle<XrdsGltfAsset>,
    ) -> Result<Vec<XrdsGltfMorphTargetSet>, XrdsGltfRuntimeError> {
        gltf_morph_targets_in_world(self.app.world(), handle)
    }

    pub fn gltf_morph_target_weights(
        &self,
        handle: &Handle<XrdsGltfAsset>,
    ) -> Result<Vec<XrdsGltfMorphTargetWeights>, XrdsGltfRuntimeError> {
        gltf_morph_target_weights_in_world(self.app.world(), handle)
    }

    pub fn set_gltf_morph_target_weight(
        &mut self,
        handle: &Handle<XrdsGltfAsset>,
        node: &XrdsGltfNodeLocator,
        mesh_name: Option<&str>,
        selector: XrdsGltfMorphTargetSelector,
        weight: f32,
    ) -> Result<(), XrdsGltfRuntimeError> {
        set_gltf_morph_target_weight_in_world(
            self.app.world_mut(),
            handle,
            node,
            mesh_name,
            selector,
            weight,
        )
    }

    // ── Component queries ─────────────────────────────────────────────────────

    /// Read a Bevy component on the entity behind a handle.
    pub fn get_component<T: Component, C>(&self, handle: &Handle<C>) -> Option<&T> {
        self.app.world().get::<T>(handle.entity())
    }

    /// Mutably access a Bevy component on the entity behind a handle.
    pub fn get_component_mut<T: Component<Mutability = bevy::ecs::component::Mutable>, C>(
        &mut self,
        handle: &Handle<C>,
    ) -> Option<Mut<'_, T>> {
        self.app.world_mut().get_mut::<T>(handle.entity())
    }

    // ── Escape hatch ──────────────────────────────────────────────────────────

    /// Direct access to the underlying Bevy [`App`] for advanced integrations.
    pub fn get(&mut self) -> &mut App {
        &mut self.app
    }

    /// Zero-argument interpreter for descriptors that implement [`XrdsAssetComponent`].
    ///
    /// The SDK reads `asset_path()` and `scene_index()` from the descriptor automatically at
    /// spawn time — no closure or recipe construction needed on the caller side.
    /// If the asset path is empty at spawn time a warning is logged and nothing is spawned.
    pub fn register_asset_interpreter<C>(&mut self) -> &mut Self
    where
        C: XrdsAssetComponent + Send + Sync + 'static,
    {
        let mut registry = self
            .app
            .world_mut()
            .get_resource_or_insert_with(SurfaceInterpreterRegistry::default);
        registry.register_optional_entity::<C, _>(|typed, commands, _| {
            let path = typed.asset_path().to_string();
            if path.is_empty() {
                warn!(
                    "register_asset_interpreter: '{}' has an empty asset_path — skipping spawn. \
                     Set asset_path or use register_surface_interpreter with a fallback recipe.",
                    typed.name()
                );
                return None;
            }
            if let Err(error) = validate_gltf_source(&path, typed.scene_index()) {
                warn!(
                    "register_asset_interpreter: '{}' has an invalid glTF asset source — skipping spawn: {error}",
                    typed.name()
                );
                return None;
            }
            let name = typed.name().to_string();
            let transform = *typed.local_transform();
            let visible = typed.is_visible();
            let scene_index = typed.scene_index();
            Some(execute_spawn_recipe(
                commands,
                XrdsGeometrySource::GltfScene { path, scene_index },
                name,
                transform,
                visible,
            ))
        });
        self
    }

    fn register_common_surface_updaters_internal<C>(&mut self) -> &mut Self
    where
        C: XrdsMutableComponent + Send + Sync + 'static,
    {
        self.register_updater::<C, TransformParams, _>(|world, entity, params| {
            apply_transform_to_entity(world, entity, *params);
            let _ = with_stored_descriptor_mut::<C, _>(world, entity, |descriptor| {
                *descriptor.local_transform_mut() = *params;
            });
        });

        self.register_updater::<C, ParentPatch, _>(|world, entity, params| {
            let Some(id) = world.resource::<XrdsIdIndex>().id_of(entity) else {
                return;
            };

            world
                .resource_mut::<QueuedParentChanges>()
                .changes
                .push(QueuedParentChange {
                    child_id: id,
                    parent_id: params.parent_id,
                });
        });

        self.register_updater::<C, NamePatch, _>(|world, entity, params| {
            world
                .entity_mut(entity)
                .insert(Name::new(params.name.clone()));
            let _ = with_stored_descriptor_mut::<C, _>(world, entity, |descriptor| {
                descriptor.set_name(params.name.clone());
            });
        });

        self.register_updater::<C, VisibilityPatch, _>(|world, entity, params| {
            world
                .entity_mut(entity)
                .insert(build_visibility(params.visible));
            let _ = with_stored_descriptor_mut::<C, _>(world, entity, |descriptor| {
                descriptor.set_visible(params.visible);
            });
        });

        self
    }

    /// Register the default XRDS transform/name/visibility/parent patch behavior for a custom
    /// descriptor type that is stored on its spawned entity.
    pub fn register_common_surface_updaters<C>(&mut self) -> &mut Self
    where
        C: XrdsMutableComponent + Send + Sync + 'static,
    {
        self.register_common_surface_updaters_internal::<C>()
    }

    /// Register a fully custom entity interpreter for a descriptor type.
    ///
    /// This is the expert escape hatch.
    /// Prefer [`Self::register_surface_interpreter`] for normal SDK extension work so custom
    /// descriptors still realize through XRDS-owned geometry and asset sources.
    ///
    /// Unlike [`Self::register_surface_interpreter`], this path does not go through the closed
    /// [`XrdsGeometrySource`] enum. The closure can spawn any Bevy entity structure it wants and
    /// return the root entity that XRDS should track.
    ///
    /// XRDS still applies the authored descriptor's `name`, `transform`, and `visible` state to
    /// the returned root entity and stores a clone of the descriptor for later XRDS updates.
    pub fn register_entity_interpreter<C, F>(&mut self, interpreter: F) -> &mut Self
    where
        C: XrdsMutableComponent + Clone + Send + Sync + 'static,
        F: Fn(&C, &mut Commands, Option<&AssetServer>) -> Entity + Send + Sync + 'static,
    {
        let mut registry = self
            .app
            .world_mut()
            .get_resource_or_insert_with(SurfaceInterpreterRegistry::default);
        registry.register_entity::<C, _>(move |typed, commands, asset_server| {
            let entity = interpreter(typed, commands, asset_server);
            let descriptor = typed.clone();
            let name = typed.name().to_string();
            let transform = *typed.local_transform();
            let visible = typed.is_visible();

            commands.queue(move |world: &mut World| {
                world.entity_mut(entity).insert((
                    Name::new(name),
                    build_transform(&transform),
                    build_visibility(visible),
                    XrdsStored(descriptor),
                ));
            });

            entity
        });

        self.register_common_surface_updaters_internal::<C>()
    }

    /// Register a recipe interpreter for a surface component type.
    ///
    /// This is the preferred extension path for custom visible XRDS types.
    ///
    /// The closure receives `&C` and returns an [`XrdsGeometrySource`] describing only the
    /// XRDS-owned geometry/material or asset source. `name`, `transform`, and `visible` are
    /// filled automatically from the descriptor's [`XrdsObject`] and [`XrdsComponent`]
    /// implementations.
    pub fn register_surface_interpreter<C, F>(&mut self, interpreter: F) -> &mut Self
    where
        C: XrdsComponent + Clone + Send + Sync + 'static,
        F: Fn(&C) -> XrdsGeometrySource + Send + Sync + 'static,
    {
        let mut registry = self
            .app
            .world_mut()
            .get_resource_or_insert_with(SurfaceInterpreterRegistry::default);
        registry.register_recipe::<C, F>(interpreter);
        self
    }

    /// Register descriptor cloning support for runtime duplication.
    ///
    /// Built-in XRDS descriptors are registered automatically. Custom mutable descriptors should
    /// opt in when they need [`Self::duplicate`] to work through the runtime-owned storage model.
    pub fn register_descriptor_clone<C>(&mut self) -> &mut Self
    where
        C: XrdsMutableComponent + Clone + Send + Sync + 'static,
    {
        let mut registry = self
            .app
            .world_mut()
            .get_resource_or_insert_with(SurfaceDescriptorRegistry::default);
        registry.register_clone::<C>();
        self
    }
}
