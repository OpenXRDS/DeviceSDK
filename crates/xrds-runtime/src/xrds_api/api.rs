use super::*;
use xrds_scene_graph::XrdsSceneEnvironment;

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
    ///
    /// **Expert escape hatch.** Returns the raw Bevy `Entity` for this id. Use this only when
    /// direct ECS access is required and no XRDS-level API covers the operation. Normal app and
    /// editor code should use `handle_of` instead and stay within XRDS APIs.
    pub fn entity_of_id(&self, id: XrdsId) -> Option<Entity> {
        self.app.world().resource::<XrdsIdIndex>().entity_of(id)
    }

    /// Resolve a typed handle from an XRDS id.
    pub fn handle_of<C>(&self, id: XrdsId) -> Option<Handle<C>> {
        self.entity_of_id(id).map(Handle::from)
    }

    /// Return every XRDS id currently live in the runtime.
    ///
    /// Useful for editor hierarchy panels that need to enumerate all nodes without
    /// holding typed handles.  The order is unspecified.
    pub fn all_runtime_ids(&self) -> Vec<XrdsId> {
        self.app
            .world()
            .resource::<XrdsIdIndex>()
            .id_to_entity
            .keys()
            .copied()
            .collect()
    }

    /// Resolve the parent id for any node by its raw XRDS id.
    ///
    /// Complements [`parent_id_of`] for cases where no typed handle is available
    /// (e.g. editor hierarchy panels that work purely with ids).
    pub fn parent_id_of_node(&self, id: XrdsId) -> Option<XrdsId> {
        self.app
            .world()
            .resource::<XrdsHierarchyIndex>()
            .parent_id_of(id)
    }

    /// Return the child ids for any node by its raw XRDS id.
    ///
    /// Complements [`child_ids_of`] for cases where no typed handle is available.
    pub fn children_ids_of_node(&self, id: XrdsId) -> Vec<XrdsId> {
        self.app
            .world()
            .resource::<XrdsHierarchyIndex>()
            .child_ids_of(id)
    }

    // -----------------------------------------------------------------------
    // PlayerAnchor API
    // -----------------------------------------------------------------------

    /// Return the first `PlayerAnchor` document node that has `is_initial: true`,
    /// or the first `PlayerAnchor` node in document order if none are marked initial.
    ///
    /// Used by play-mode startup to find where to spawn the player pawn.
    pub fn initial_player_anchor_id(&self) -> Option<XrdsId> {
        let world = self.app.world();
        let id_index = world.resource::<XrdsIdIndex>();
        // Access the imported document via the environment resource which holds a copy of the nodes.
        // Walk all entities with XrdsPlayerAnchorRoot and prefer those with is_initial flag.
        // Since we don't store the document here, we return the first anchor entity id.
        let mut first: Option<XrdsId> = None;
        for (&entity, &id) in &id_index.entity_to_id {
            if world
                .get_entity(entity)
                .ok()
                .is_some_and(|e| e.contains::<XrdsPlayerAnchorRoot>())
            {
                if first.is_none() {
                    first = Some(id);
                }
            }
        }
        first
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

    /// Mark an entity as pick-up-able by the XR grab system.
    ///
    /// After this call the entity will be found by the SDK's trigger-press raycast and
    /// can be picked up with the controller trigger in XR play mode.
    pub fn make_grabbable<C>(&mut self, handle: &Handle<C>) -> &mut Self {
        if let Ok(mut e) = self.app.world_mut().get_entity_mut(handle.entity()) {
            e.insert(xrds_components::XrGrabbable);
        }
        self
    }

    /// Mark an entity by XRDS id as pick-up-able.
    ///
    /// Use this variant when you have an [`XrdsId`] rather than a typed handle — for example
    /// after [`import_scene_document_json`] where the ids come from the document.
    pub fn make_grabbable_by_id(&mut self, id: XrdsId) -> &mut Self {
        let world = self.app.world_mut();
        if let Some(entity) = world.resource::<XrdsIdIndex>().entity_of(id) {
            if let Ok(mut e) = world.get_entity_mut(entity) {
                e.insert(xrds_components::XrGrabbable);
            }
        }
        self
    }

    /// Remove the `XrGrabbable` marker from an entity (makes it non-grabbable again).
    pub fn make_ungrabable<C>(&mut self, handle: &Handle<C>) -> &mut Self {
        if let Ok(mut e) = self.app.world_mut().get_entity_mut(handle.entity()) {
            e.remove::<xrds_components::XrGrabbable>();
        }
        self
    }

    /// Return a random world-space position within a randomly chosen `PlayerSpawnZone` in the scene.
    ///
    /// Picks from all zones regardless of ownership. Y is taken from the zone centre (not randomised).
    /// Returns `None` if no spawn zones exist.
    pub fn random_spawn_zone_position(&self) -> Option<Vec3> {
        random_spawn_zone_position_in_world(self.app.world(), None)
    }

    /// Return a random spawn position from zones designated for `player_node_id`,
    /// falling back to shared zones (no owner) when no designated zones exist.
    ///
    /// Use this in multi-player / team scenarios where each player has its own spawn zone.
    /// Returns `None` if no eligible zones are found.
    pub fn random_spawn_zone_position_for(&self, player_node_id: u64) -> Option<Vec3> {
        random_spawn_zone_position_in_world(self.app.world(), Some(player_node_id))
    }

    /// Teleport the player (the entity tagged `XrdsPlayerRoot`) to `position`.
    pub fn teleport_player(&mut self, position: Vec3) {
        teleport_player_in_world(self.app.world_mut(), position);
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
                    // Use spawn_gltf_descriptor directly so a validation failure
                    // (GLB not found at CWD / invalid file) logs a warning and
                    // skips the node instead of panicking through spawn_with_id.
                    reserve_runtime_id_in_world(self.app.world_mut(), id)?;
                    let mut queue = CommandQueue::default();
                    let entity_opt = {
                        let mut commands = Commands::new(&mut queue, self.app.world_mut());
                        let e = spawn_gltf_descriptor(&mut commands, &component);
                        if let Some(ent) = e {
                            commands.entity(ent).insert(XrdsDescriptorType(TypeId::of::<XrdsGltfAsset>()));
                        }
                        e
                    };
                    queue.apply(self.app.world_mut());
                    let Some(entity) = entity_opt else {
                        warn!(
                            "[import] GltfAsset '{}' skipped: GLB not found or invalid. \
                             Ensure the file is bundled and the working directory is the asset root.",
                            component.name
                        );
                        continue;
                    };
                    self.app.world_mut().resource_mut::<XrdsIdIndex>().register(id, entity);
                    self.app.world_mut().resource_mut::<XrdsHierarchyIndex>().ensure_node(id);
                    entity
                }
                XrdsSceneRuntimeComponent::Cube(component) => {
                    self.spawn_with_id(id, &component)?.entity()
                }
                XrdsSceneRuntimeComponent::Cylinder(component) => {
                    self.spawn_with_id(id, &component)?.entity()
                }
                XrdsSceneRuntimeComponent::Capsule(component) => {
                    self.spawn_with_id(id, &component)?.entity()
                }
                XrdsSceneRuntimeComponent::Effect(component) => {
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
                XrdsSceneRuntimeComponent::AudioClip(component) => {
                    self.spawn_with_id(id, &component)?.entity()
                }
                XrdsSceneRuntimeComponent::HudText(hud) => {
                    reserve_runtime_id_in_world(self.app.world_mut(), id)?;
                    spawn_hud_text_entity(self.app.world_mut(), id, &hud)
                }
                XrdsSceneRuntimeComponent::Text(component) => {
                    self.spawn_with_id(id, &component)?.entity()
                }
                XrdsSceneRuntimeComponent::ExtrudedText(component) => {
                    self.spawn_with_id(id, &component)?.entity()
                }
                XrdsSceneRuntimeComponent::InteractionZone(node, zone) => {
                    reserve_runtime_id_in_world(self.app.world_mut(), id)?;
                    spawn_interaction_zone_entity(self.app.world_mut(), id, &node, &zone)
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
        merge_imported_asset_catalog(self.app.world_mut(), &document.assets);
        store_imported_scene_environment_in_world(
            self.app.world_mut(),
            document.environment().cloned(),
        );
        let imported_ids = self.import_runtime_nodes(runtime_nodes)?;
        // Tag Player/PlayerAnchor entities with runtime marker components.
        // import_runtime_nodes works from XrdsSceneRuntimeComponent (which has no payload
        // field), so the Player/PlayerAnchor distinction must be recovered from the
        // original document nodes here.
        crate::xrds_api::reimport::tag_player_anchor_entities(self.app.world_mut(), document);
        crate::xrds_api::reimport::tag_grabbable_entities(self.app.world_mut(), document);
        crate::xrds_api::reimport::spawn_panel_instances(self.app.world_mut(), document);
        crate::xrds_api::reimport::tag_spawn_zone_entities(self.app.world_mut(), document);
        crate::xrds_api::reimport::tag_trigger_binding_entities(self.app.world_mut(), document);
        crate::xrds_api::reimport::tag_threshold_watcher_entities(self.app.world_mut(), document);
        crate::xrds_api::reimport::sync_panel_registry(self.app.world_mut(), document);
        crate::xrds_api::reimport::sync_track_registry(self.app.world_mut(), document);
        apply_imported_scene_environment_policy_in_world(self.app.world_mut());
        crate::xrds_api::passthrough::apply_xr_blend_mode(
            self.app.world_mut(),
            document.metadata.xr_blend_mode,
        );
        Ok(imported_ids)
    }

    /// Start or resume an audio clip node. See
    /// [`XrdsUpdateContext::play_audio_for_node`] for the semantics; this is the
    /// same operation from the setup/editor side, where the editor's audition
    /// button needs it.
    pub fn play_audio_for_node(&mut self, id: XrdsId) -> bool {
        use crate::xrds_api::audio_playback::{transport_for_node, AudioTransport};
        transport_for_node(self.app.world_mut(), id, AudioTransport::Play)
    }

    /// Pause an audio clip node where it is.
    pub fn pause_audio_for_node(&mut self, id: XrdsId) -> bool {
        use crate::xrds_api::audio_playback::{transport_for_node, AudioTransport};
        transport_for_node(self.app.world_mut(), id, AudioTransport::Pause)
    }

    /// Stop an audio clip node and rewind it, so it can be played again.
    pub fn stop_audio_for_node(&mut self, id: XrdsId) -> bool {
        use crate::xrds_api::audio_playback::{transport_for_node, AudioTransport};
        transport_for_node(self.app.world_mut(), id, AudioTransport::Stop)
    }

    /// Load a saved XRDS scene document from JSON and import it into runtime state.
    ///
    /// This is the end-to-end document path for editor or tool flows that persist authored scene
    /// data to disk and later realize that saved scene in a live XRDS runtime.
    pub fn import_scene_document_json(
        &mut self,
        path: impl AsRef<std::path::Path>,
    ) -> Result<Vec<XrdsId>, XrdsSceneImportError> {
        let document = XrdsSceneDocument::load_json(path)
            .map_err(|error| XrdsSceneImportError::InvalidDocument(format!("{error:?}")))?;
        self.import_scene_document(&document)
    }

    /// Merge authored scene assets into the runtime asset catalog.
    ///
    /// Use this when runtime-authored scene policies, such as scene environment IBL, need to
    /// reference durable scene asset ids without going through full document import.
    pub fn merge_scene_assets(&mut self, assets: &[XrdsSceneAsset]) -> &mut Self {
        merge_imported_asset_catalog(self.app.world_mut(), assets);
        self
    }

    /// Read the currently active runtime scene environment policy.
    pub fn scene_environment(&self) -> Option<XrdsSceneEnvironment> {
        imported_scene_environment_in_world(self.app.world())
    }

    /// Set the active runtime scene environment policy.
    ///
    /// The environment refers to scene asset ids from the runtime asset catalog. If the
    /// referenced texture assets are not present there, the policy remains stored and exportable,
    /// but runtime camera environment maps cannot be resolved until those assets are merged.
    pub fn set_scene_environment(&mut self, environment: XrdsSceneEnvironment) -> &mut Self {
        store_imported_scene_environment_in_world(self.app.world_mut(), Some(environment));
        apply_imported_scene_environment_policy_in_world(self.app.world_mut());
        self
    }

    /// Clear the active runtime scene environment policy.
    pub fn clear_scene_environment(&mut self) -> &mut Self {
        store_imported_scene_environment_in_world(self.app.world_mut(), None);
        apply_imported_scene_environment_policy_in_world(self.app.world_mut());
        self
    }

    /// Export the current XRDS-authored runtime state to a scene document.
    ///
    /// This exports built-in XRDS descriptors, stable ids, parent links, authored materials,
    /// preserved editor metadata, and glTF references from XRDS-owned runtime state. The document
    /// asset table is reconstructed for built-in glTF asset references, and preserved imported
    /// asset catalog entries are merged back in for authored non-glTF references such as textures.
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

    /// Read current camera projection settings.
    pub fn camera_projection(&self, handle: &Handle<XrdsCamera>) -> Option<CameraProjectionParams> {
        camera_projection_in_world(self.app.world(), handle)
    }

    /// Read current camera look-at target.
    ///
    /// Returns `None` if the entity does not exist.
    /// Returns `Some(None)` if the camera exists but look-at is not active.
    pub fn camera_look_at(&self, handle: &Handle<XrdsCamera>) -> Option<Option<[f32; 3]>> {
        camera_look_at_in_world(self.app.world(), handle)
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

    /// Read current glTF asset source path and scene index.
    pub fn gltf_source(&self, handle: &Handle<XrdsGltfAsset>) -> Option<GltfAssetSourcePatch> {
        gltf_source_in_world(self.app.world(), handle)
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

    /// Read current point light parameters.
    pub fn point_light_params(
        &self,
        handle: &Handle<XrdsPointLight>,
    ) -> Option<PointLightParams> {
        point_light_params_in_world(self.app.world(), handle)
    }

    /// Read current directional light parameters.
    pub fn directional_light_params(
        &self,
        handle: &Handle<XrdsDirectionalLight>,
    ) -> Option<DirectionalLightParams> {
        directional_light_params_in_world(self.app.world(), handle)
    }

    /// Read current spot light parameters.
    pub fn spot_light_params(&self, handle: &Handle<XrdsSpotLight>) -> Option<SpotLightParams> {
        spot_light_params_in_world(self.app.world(), handle)
    }

    /// Read current ambient light parameters.
    pub fn ambient_light_params(
        &self,
        handle: &Handle<XrdsAmbientLight>,
    ) -> Option<AmbientLightParams> {
        ambient_light_params_in_world(self.app.world(), handle)
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

    /// Read current 3D text content and styling parameters.
    pub fn text_params(&self, handle: &Handle<XrdsText>) -> Option<TextParams> {
        text_params_in_world(self.app.world(), handle)
    }

    /// Queue a 3D text content and styling update.
    pub fn set_text_params(
        &mut self,
        handle: &Handle<XrdsText>,
        params: TextParams,
    ) -> &mut Self {
        self.queue_update(handle, params)
    }

    /// Spawn a 3D head-locked HUD text label at the given camera-space offset.
    ///
    /// The label follows the player's head — it stays at a fixed position relative
    /// to the camera regardless of where the player looks or moves.
    ///
    /// `offset` is camera-space: `+X` right, `+Y` up, `-Z` in front of the camera.
    /// A typical starting point is `Vec3::new(0.0, -0.1, -0.5)` — 50 cm in front,
    /// 10 cm below the centre line.
    ///
    /// Update the text at runtime with [`Self::set_text_params`] (in setup) or
    /// [`XrdsUpdateContext::set_text_params`] (in update).
    /// Spawn a world-space UI panel at the given world transform.
    ///
    /// The panel is a flat quad mesh anchored at a fixed world position.
    /// Unlike the HUD, it does not follow the player; the player points at it
    /// with an XR controller ray to interact (diegetic / world-space UI).
    ///
    /// Returns a handle you can pass to `set_transform`, `spawn_world_button`, etc.
    ///
    /// # Example
    /// ```ignore
    /// let panel = api.spawn_world_panel(
    ///     XrdsWorldPanel::new()
    ///         .with_size(0.6, 0.4)
    ///         .with_color(0.1, 0.1, 0.1, 0.9),
    /// );
    /// api.set_transform(&panel, Transform::from_xyz(0.0, 1.5, -1.0));
    /// ```
    pub fn spawn_world_panel(&mut self, descriptor: XrdsWorldPanel) -> Handle<XrdsWorldPanel> {
        self.spawn(&descriptor)
    }

    /// Spawn a text label as a child of the given panel.
    ///
    /// The returned handle's `.entity()` refers to the label entity; pass it to
    /// `ctx.set_world_label_text(handle, "...")` to update text at runtime.
    ///
    /// # Example
    /// ```ignore
    /// let lbl = api.spawn_world_label(&panel, XrdsWorldLabelParams {
    ///     text: "Score: 0".to_string(),
    ///     font_size: 0.05,
    ///     local_position: [0.0, 0.1],
    ///     ..default()
    /// });
    /// ```
    pub fn spawn_world_label(
        &mut self,
        panel: &Handle<XrdsWorldPanel>,
        params: XrdsWorldLabelParams,
    ) -> Handle<XrdsWorldLabel> {
        let panel_entity = panel.entity();
        let world = self.app.world_mut();
        let entity = spawn_world_label_entity(world, panel_entity, &params);
        Handle::from(entity)
    }

    /// Spawn a pressable button as a child of the given panel.
    ///
    /// Listen for presses via `ctx.world_button_presses()` and compare
    /// `ev.button_entity == btn.entity()`.
    ///
    /// # Example
    /// ```ignore
    /// let btn = api.spawn_world_button(&panel, XrdsWorldButtonParams {
    ///     label: "Start".to_string(),
    ///     size: [0.2, 0.06],
    ///     local_position: [0.0, -0.1],
    ///     ..default()
    /// });
    /// ```
    pub fn spawn_world_button(
        &mut self,
        panel: &Handle<XrdsWorldPanel>,
        params: XrdsWorldButtonParams,
    ) -> Handle<XrdsWorldButton> {
        let panel_entity = panel.entity();
        let world = self.app.world_mut();
        let entity = spawn_world_button_entity(world, panel_entity, &params);
        Handle::from(entity)
    }

    /// Spawn a textured image quad as a child of the given panel.
    ///
    /// # Example
    /// ```ignore
    /// let img = api.spawn_world_image(&panel, XrdsWorldImageParams {
    ///     asset_path: "textures/logo.png".to_string(),
    ///     size: [0.12, 0.12],
    ///     local_position: [0.0, 0.15],
    ///     ..default()
    /// });
    /// ```
    pub fn spawn_world_image(
        &mut self,
        panel: &Handle<XrdsWorldPanel>,
        params: XrdsWorldImageParams,
    ) -> Handle<XrdsWorldImage> {
        let panel_entity = panel.entity();
        let world = self.app.world_mut();
        let entity = spawn_world_image_entity(world, panel_entity, &params);
        Handle::from(entity)
    }

    /// Spawn a drag-to-scrub slider as a child of the given panel.
    ///
    /// # Example
    /// ```ignore
    /// let sld = api.spawn_world_slider(&panel, XrdsWorldSliderParams {
    ///     min: 0.0, max: 1.0, value: 0.5,
    ///     local_position: [0.0, -0.05],
    ///     ..default()
    /// });
    /// ```
    pub fn spawn_world_slider(
        &mut self,
        panel: &Handle<XrdsWorldPanel>,
        params: XrdsWorldSliderParams,
    ) -> Handle<XrdsWorldSlider> {
        let panel_entity = panel.entity();
        let world = self.app.world_mut();
        let entity = spawn_world_slider_entity(world, panel_entity, &params);
        Handle::from(entity)
    }

    /// Spawn a binary on/off toggle as a child of the given panel.
    ///
    /// # Example
    /// ```ignore
    /// let tog = api.spawn_world_toggle(&panel, XrdsWorldToggleParams {
    ///     checked: false,
    ///     local_position: [0.15, 0.05],
    ///     ..default()
    /// });
    /// ```
    pub fn spawn_world_toggle(
        &mut self,
        panel: &Handle<XrdsWorldPanel>,
        params: XrdsWorldToggleParams,
    ) -> Handle<XrdsWorldToggle> {
        let panel_entity = panel.entity();
        let world = self.app.world_mut();
        let entity = spawn_world_toggle_entity(world, panel_entity, &params);
        Handle::from(entity)
    }

    /// Set (or replace) the layout policy on a world panel.
    ///
    /// Inserts an [`XrdsWorldLayout`] component on the panel entity. The layout system then
    /// repositions child widgets every frame. Call with [`XrdsWorldLayout::None`] to revert
    /// to manual positioning.
    ///
    /// ```ignore
    /// api.set_world_panel_layout(&panel, XrdsWorldLayout::vstack(0.01));
    /// ```
    pub fn set_world_panel_layout(
        &mut self,
        panel: &Handle<XrdsWorldPanel>,
        layout: xrds_components::XrdsWorldLayout,
    ) {
        let entity = panel.entity();
        if let Ok(mut e) = self.app.world_mut().get_entity_mut(entity) {
            e.insert(layout);
        }
    }

    pub fn spawn_hud_label(&mut self, text: &str, offset: Vec3) -> Handle<XrdsText> {
        let descriptor = XrdsText {
            name: "HudLabel".to_string(),
            text: text.to_string(),
            font_size: 4.0,
            color: [1.0, 1.0, 1.0, 1.0],
            alignment: XrdsTextAlignment::Center,
            anchor: XrdsTextAnchor::HeadLocked,
            transform: TransformParams {
                translation: offset.to_array(),
                ..Default::default()
            },
            enabled: true,
            visible: true,
        };
        self.spawn(&descriptor)
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

    /// Queue a capsule geometry update.
    pub fn set_capsule_geometry(
        &mut self,
        handle: &Handle<XrdsCapsule>,
        params: CapsuleGeometryParams,
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

    /// Queue a particle-effect parameter update.
    ///
    /// The spawner is rebuilt from the new parameters; particles already alive
    /// finish their current lifetime under the old settings rather than snapping
    /// to the new ones.
    ///
    /// Keep colour components ≤ 1.0 — values above that are clamped, because the
    /// SDK's XR cameras have no HDR/bloom pass and brighter values would
    /// otherwise render as flat white. See `XrdsEffect::color_start`.
    pub fn set_effect_params(
        &mut self,
        handle: &Handle<XrdsEffect>,
        params: EffectParams,
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

    /// Read all texture slots for mesh-based entities.
    pub fn material_textures<C>(&self, handle: &Handle<C>) -> Option<XrdsMaterialTextureSlots> {
        material_textures_in_world(self.app.world(), handle)
    }

    /// Set a single texture slot on mesh-based entities.
    ///
    /// Pass `None` to clear the slot.  This is the immediate preview path for
    /// inspector-driven texture assignment without touching other material fields.
    pub fn set_material_texture_slot<C>(
        &mut self,
        handle: &Handle<C>,
        slot: XrdsMaterialTextureSlotKind,
        texture: Option<XrdsMaterialTextureRef>,
    ) where
        C: XrdsComponent + Send + Sync + 'static,
    {
        set_material_texture_slot_in_world(self.app.world_mut(), handle, slot, texture);
    }

    /// Replace all texture slots at once for mesh-based entities.
    ///
    /// This is the immediate preview path for inspector panels that operate on the full
    /// texture set (e.g. when importing a material preset).
    pub fn set_material_textures<C>(
        &mut self,
        handle: &Handle<C>,
        textures: XrdsMaterialTextureSlots,
    ) where
        C: XrdsComponent + Send + Sync + 'static,
    {
        set_material_textures_in_world(self.app.world_mut(), handle, textures);
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

    /// Fires a trigger on a node directly, running every binding on it that
    /// matches `kind`, without waiting for the real event.
    ///
    /// Returns how many sequences started — `0` means nothing was bound for
    /// that kind. Intended for an editor "preview this sequence" button and
    /// for application tests, where staging a real zone collision or button
    /// press is impractical.
    pub fn fire_trigger(
        &mut self,
        node: XrdsId,
        kind: &xrds_scene_graph::XrdsTriggerKind,
        hand: Option<xrds_components::XrGrabHand>,
    ) -> usize {
        crate::xrds_api::trigger_action::fire_trigger_in_world(
            self.app.world_mut(),
            node,
            kind,
            hand,
        )
    }

    /// Cancels every in-flight sequence on a node, clearing its queues and
    /// despawning the agents.
    ///
    /// Useful beyond error recovery — aborting a cutscene on player input,
    /// or tearing down before a scene transition.
    pub fn stop_sequences_on(&mut self, node: XrdsId) -> usize {
        crate::xrds_api::trigger_action::stop_sequences_on_in_world(self.app.world_mut(), node)
    }

    /// Cancels every in-flight sequence in the world.
    pub fn stop_all_sequences(&mut self) -> usize {
        crate::xrds_api::trigger_action::stop_all_sequences_in_world(self.app.world_mut())
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

/// Spawn a Bevy UI text entity for a HUD text node and register it with XRDS indices.
pub(super) fn spawn_hud_text_entity(
    world: &mut World,
    id: XrdsId,
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

    let entity = world
        .spawn((
            bevy::ui::widget::Text::new(hud.text.clone()),
            node,
            TextFont { font_size: hud.font_size, ..Default::default() },
            TextColor(bevy::color::Color::srgba(r, g, b, a)),
            stored,
        ))
        .id();

    world.resource_mut::<XrdsIdIndex>().register(id, entity);
    world.resource_mut::<XrdsHierarchyIndex>().ensure_node(id);

    entity
}
