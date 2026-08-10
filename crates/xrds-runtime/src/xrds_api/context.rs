use super::*;
use xrds_scene_graph::XrdsSceneEnvironment;

///
/// Build this inside an exclusive Bevy system with [`XrdsUpdateContext::new`] to access
/// XRDS-level time, input, and mutation helpers without dropping down to raw Bevy APIs.
pub struct XrdsUpdateContext<'w> {
    pub(super) world: &'w mut World,
}

impl XrdsUpdateContext<'_> {
    pub fn new(world: &mut World) -> XrdsUpdateContext<'_> {
        XrdsUpdateContext { world }
    }

    /// Seconds elapsed since the app started.
    pub fn elapsed_secs(&self) -> f32 {
        self.world.resource::<Time>().elapsed_secs()
    }

    /// Seconds elapsed since the last frame.
    pub fn delta_secs(&self) -> f32 {
        self.world.resource::<Time>().delta_secs()
    }

    /// Merge authored scene assets into the runtime asset catalog.
    pub fn merge_scene_assets(&mut self, assets: &[XrdsSceneAsset]) -> &mut Self {
        merge_imported_asset_catalog(self.world, assets);
        self
    }

    /// Read the currently active runtime scene environment policy.
    pub fn scene_environment(&self) -> Option<XrdsSceneEnvironment> {
        imported_scene_environment_in_world(self.world)
    }

    /// Set the active runtime scene environment policy.
    pub fn set_scene_environment(&mut self, environment: XrdsSceneEnvironment) -> &mut Self {
        store_imported_scene_environment_in_world(self.world, Some(environment));
        apply_imported_scene_environment_policy_in_world(self.world);
        self
    }

    /// Clear the active runtime scene environment policy.
    pub fn clear_scene_environment(&mut self) -> &mut Self {
        store_imported_scene_environment_in_world(self.world, None);
        apply_imported_scene_environment_policy_in_world(self.world);
        self
    }

    /// Returns true on the frame where `key` transitions from up -> down.
    pub fn key_just_pressed(&self, key: XrdsKey) -> bool {
        let key = key.into_bevy();
        self.world
            .get_resource::<ButtonInput<KeyCode>>()
            .is_some_and(|keys| keys.just_pressed(key))
    }

    /// Returns true while `key` is held down.
    pub fn key_pressed(&self, key: XrdsKey) -> bool {
        let key = key.into_bevy();
        self.world
            .get_resource::<ButtonInput<KeyCode>>()
            .is_some_and(|keys| keys.pressed(key))
    }

    /// Overwrite the full transform from [`TransformParams`].
    ///
    /// This is the immediate preview path. In an editor, prefer this during live gizmo drags
    /// or viewport interaction, then commit the final change back through a queued/document-backed
    /// patch once the interaction ends.
    pub fn set_transform<C>(&mut self, handle: &Handle<C>, params: TransformParams)
    where
        C: XrdsComponent + Send + Sync + 'static,
    {
        set_transform_in_world(self.world, handle, params);
    }

    /// Set only the translation of an entity.
    ///
    /// This is intended for interactive preview updates. For authoritative editor commits,
    /// batch the final transform change through [`Self::queue_update`].
    pub fn set_translation<C>(&mut self, handle: &Handle<C>, translation: [f32; 3])
    where
        C: XrdsComponent + Send + Sync + 'static,
    {
        set_translation_in_world(self.world, handle, translation);
    }

    /// Apply translation directly to any live node by its XRDS id.
    ///
    /// This is the immediate-preview path for the editor's inspector fields and
    /// transform gizmo: the runtime component is updated without going through a
    /// typed handle, so it works for any node type.  For the authoritative
    /// session commit, call `XrdsSceneDocumentSession::set_node_transform`.
    pub fn set_translation_for_node(&mut self, id: XrdsId, translation: [f32; 3]) {
        if let Some(entity) = self.world.resource::<XrdsIdIndex>().entity_of(id) {
            if let Some(mut params) = transform_params_for_entity(self.world, entity) {
                params.translation = translation;
                apply_transform_to_entity(self.world, entity, params);
            }
        }
    }

    /// Apply rotation (quaternion `[x, y, z, w]`) to any live node by its XRDS id.
    pub fn set_rotation_for_node(&mut self, id: XrdsId, rotation_quat_xyzw: [f32; 4]) {
        if let Some(entity) = self.world.resource::<XrdsIdIndex>().entity_of(id) {
            if let Some(mut params) = transform_params_for_entity(self.world, entity) {
                params.rotation_quat_xyzw = rotation_quat_xyzw;
                apply_transform_to_entity(self.world, entity, params);
            }
        }
    }

    /// Update the `XrdsAnchorFov` component on a PlayerAnchor entity without reimport.
    /// Called during the live-preview phase so the FOV overlay reflects slider changes instantly.
    pub fn set_anchor_fov_for_node(&mut self, id: XrdsId, fov_deg: f32) {
        use crate::xrds_api::anchor::XrdsAnchorFov;
        if let Some(entity) = self.world.resource::<XrdsIdIndex>().entity_of(id) {
            if let Some(mut fov) = self.world.get_mut::<XrdsAnchorFov>(entity) {
                fov.0 = fov_deg;
            }
        }
    }

    /// Apply scale to any live node by its XRDS id.
    pub fn set_scale_for_node(&mut self, id: XrdsId, scale: [f32; 3]) {
        if let Some(entity) = self.world.resource::<XrdsIdIndex>().entity_of(id) {
            if let Some(mut params) = transform_params_for_entity(self.world, entity) {
                params.scale = scale;
                apply_transform_to_entity(self.world, entity, params);
            }
        }
    }

    /// Update the base color of an extruded-text node's `StandardMaterial` in-place.
    ///
    /// Use this instead of a full reimport for color-only changes: avoids the
    /// `bevy_fontmesh::update_text_meshes` deferred-command race condition where
    /// that system queues commands for a `TextMesh` entity and our reimport
    /// despawns it before the commands are applied.
    pub fn set_extruded_text_color_for_node(&mut self, id: XrdsId, color: [f32; 4]) {
        let Some(entity) = self.world.resource::<XrdsIdIndex>().entity_of(id) else {
            return;
        };
        let mat_handle = self.world
            .get::<bevy::prelude::MeshMaterial3d<bevy::pbr::StandardMaterial>>(entity)
            .map(|m| m.0.clone());
        let Some(handle) = mat_handle else { return; };
        let [r, g, b, _a] = color;
        if let Some(mut materials) =
            self.world.get_resource_mut::<bevy::asset::Assets<bevy::pbr::StandardMaterial>>()
        {
            if let Some(mat) = materials.get_mut(&handle) {
                mat.base_color = bevy::color::Color::srgb(r, g, b);
            }
        }
    }

    /// Apply material params to any live node by its XRDS id.
    pub fn set_material_params_for_node(&mut self, id: XrdsId, params: XrdsMaterialParams) {
        if let Some(entity) = self.world.resource::<XrdsIdIndex>().entity_of(id) {
            set_material_params_for_entity_in_world(self.world, entity, params);
        }
    }

    /// Apply point light color/intensity/range to any live node by its XRDS id.
    pub fn set_point_light_params_for_node(&mut self, id: XrdsId, color: [f32; 4], intensity: f32, range: f32) {
        if let Some(entity) = self.world.resource::<XrdsIdIndex>().entity_of(id) {
            if let Some(mut light) = self.world.get_mut::<PointLight>(entity) {
                light.color = Color::linear_rgba(color[0], color[1], color[2], color[3]);
                light.intensity = intensity;
                light.range = range;
            }
        }
    }

    /// Apply directional light color/illuminance to any live node by its XRDS id.
    pub fn set_directional_light_params_for_node(&mut self, id: XrdsId, color: [f32; 4], illuminance: f32) {
        if let Some(entity) = self.world.resource::<XrdsIdIndex>().entity_of(id) {
            if let Some(mut light) = self.world.get_mut::<DirectionalLight>(entity) {
                light.color = Color::linear_rgba(color[0], color[1], color[2], color[3]);
                light.illuminance = illuminance;
            }
        }
    }

    /// Apply spot light color/intensity/range/angles to any live node by its XRDS id.
    pub fn set_spot_light_params_for_node(&mut self, id: XrdsId, color: [f32; 4], intensity: f32, range: f32, inner_angle: f32, outer_angle: f32) {
        if let Some(entity) = self.world.resource::<XrdsIdIndex>().entity_of(id) {
            if let Some(mut light) = self.world.get_mut::<SpotLight>(entity) {
                light.color = Color::linear_rgba(color[0], color[1], color[2], color[3]);
                light.intensity = intensity;
                light.range = range;
                light.inner_angle = inner_angle;
                light.outer_angle = outer_angle;
            }
        }
    }

    /// Apply ambient light color/brightness to the global AmbientLight resource.
    pub fn set_ambient_light_params(&mut self, color: [f32; 4], brightness: f32) {
        if let Some(mut light) = self.world.get_resource_mut::<AmbientLight>() {
            light.color = Color::linear_rgba(color[0], color[1], color[2], color[3]);
            light.brightness = brightness;
        }
    }

    /// Show or hide a node in the 3D viewport by setting its Bevy `Visibility` component.
    pub fn set_visible_for_node(&mut self, id: XrdsId, visible: bool) {
        if let Some(entity) = self.world.resource::<XrdsIdIndex>().entity_of(id) {
            if let Some(mut v) = self.world.get_mut::<bevy::prelude::Visibility>(entity) {
                *v = if visible {
                    bevy::prelude::Visibility::Inherited
                } else {
                    bevy::prelude::Visibility::Hidden
                };
            }
        }
    }

    /// Set only the rotation (quaternion `[x, y, z, w]`).
    ///
    /// This is intended for interactive preview updates. For authoritative editor commits,
    /// batch the final transform change through [`Self::queue_update`].
    pub fn set_rotation<C>(&mut self, handle: &Handle<C>, xyzw: [f32; 4])
    where
        C: XrdsComponent + Send + Sync + 'static,
    {
        set_rotation_in_world(self.world, handle, xyzw);
    }

    pub fn rotate_x<C>(&mut self, handle: &Handle<C>, angle_rad: f32)
    where
        C: XrdsComponent + Send + Sync + 'static,
    {
        let mut params = match transform_params_for_entity(self.world, handle.entity()) {
            Some(params) => params,
            None => return,
        };
        let [x, y, z, w] = params.rotation_quat_xyzw;
        let mut rotation = Quat::from_xyzw(x, y, z, w);
        rotation *= Quat::from_rotation_x(angle_rad);
        params.rotation_quat_xyzw = [rotation.x, rotation.y, rotation.z, rotation.w];
        let (x_deg, y_deg, z_deg) = rotation.to_euler(EulerRot::XYZ);
        params.rotation_euler_xyz_deg =
            [x_deg.to_degrees(), y_deg.to_degrees(), z_deg.to_degrees()];
        self.set_transform(handle, params);
    }

    /// Incrementally rotate around the Y axis by `angle_rad`.
    ///
    /// This is an immediate preview helper for interactive controls.
    pub fn rotate_y<C>(&mut self, handle: &Handle<C>, angle_rad: f32)
    where
        C: XrdsComponent + Send + Sync + 'static,
    {
        let mut params = match transform_params_for_entity(self.world, handle.entity()) {
            Some(params) => params,
            None => return,
        };
        let [x, y, z, w] = params.rotation_quat_xyzw;
        let mut rotation = Quat::from_xyzw(x, y, z, w);
        rotation *= Quat::from_rotation_y(angle_rad);
        params.rotation_quat_xyzw = [rotation.x, rotation.y, rotation.z, rotation.w];
        let (x_deg, y_deg, z_deg) = rotation.to_euler(EulerRot::XYZ);
        params.rotation_euler_xyz_deg =
            [x_deg.to_degrees(), y_deg.to_degrees(), z_deg.to_degrees()];
        self.set_transform(handle, params);
    }

    pub fn rotate_z<C>(&mut self, handle: &Handle<C>, angle_rad: f32)
    where
        C: XrdsComponent + Send + Sync + 'static,
    {
        let mut params = match transform_params_for_entity(self.world, handle.entity()) {
            Some(params) => params,
            None => return,
        };
        let [x, y, z, w] = params.rotation_quat_xyzw;
        let mut rotation = Quat::from_xyzw(x, y, z, w);
        rotation *= Quat::from_rotation_z(angle_rad);
        params.rotation_quat_xyzw = [rotation.x, rotation.y, rotation.z, rotation.w];
        let (x_deg, y_deg, z_deg) = rotation.to_euler(EulerRot::XYZ);
        params.rotation_euler_xyz_deg =
            [x_deg.to_degrees(), y_deg.to_degrees(), z_deg.to_degrees()];
        self.set_transform(handle, params);
    }

    /// Set only the scale of an entity.
    ///
    /// This is intended for interactive preview updates. For authoritative editor commits,
    /// batch the final transform change through [`Self::queue_update`].
    pub fn set_scale<C>(&mut self, handle: &Handle<C>, scale: [f32; 3])
    where
        C: XrdsComponent + Send + Sync + 'static,
    {
        set_scale_in_world(self.world, handle, scale);
    }

    /// Show or hide an entity.
    ///
    /// This mutates the runtime immediately and is best used for viewport preview state.
    /// For committed editor visibility changes, prefer a queued patch through [`Self::queue_update`]
    /// or the higher-level [`XrdsAPI::set_visible`] commit helper.
    pub fn set_visibility<C>(&mut self, handle: &Handle<C>, visible: bool)
    where
        C: XrdsComponent + Send + Sync + 'static,
    {
        set_visibility_in_world(self.world, handle, visible);
    }

    /// Read current point light parameters.
    pub fn point_light_params(
        &self,
        handle: &Handle<XrdsPointLight>,
    ) -> Option<PointLightParams> {
        point_light_params_in_world(self.world, handle)
    }

    /// Read current directional light parameters.
    pub fn directional_light_params(
        &self,
        handle: &Handle<XrdsDirectionalLight>,
    ) -> Option<DirectionalLightParams> {
        directional_light_params_in_world(self.world, handle)
    }

    /// Read current spot light parameters.
    pub fn spot_light_params(&self, handle: &Handle<XrdsSpotLight>) -> Option<SpotLightParams> {
        spot_light_params_in_world(self.world, handle)
    }

    /// Read current ambient light parameters.
    pub fn ambient_light_params(
        &self,
        handle: &Handle<XrdsAmbientLight>,
    ) -> Option<AmbientLightParams> {
        ambient_light_params_in_world(self.world, handle)
    }

    /// Read current 3D text content and styling parameters.
    pub fn text_params(&self, handle: &Handle<XrdsText>) -> Option<TextParams> {
        text_params_in_world(self.world, handle)
    }

    /// Queue a 3D text content and styling update.
    pub fn set_text_params(&mut self, handle: &Handle<XrdsText>, params: TextParams) {
        self.queue_update(handle, params);
    }

    /// Read current point light intensity.
    pub fn point_light_intensity<C>(&self, handle: &Handle<C>) -> Option<f32> {
        self.world
            .get::<PointLight>(handle.entity())
            .map(|l| l.intensity)
    }

    /// Set point light intensity.
    ///
    /// This is the immediate preview path for interactive scrubbing.
    pub fn set_point_light_intensity<C>(&mut self, handle: &Handle<C>, intensity: f32) {
        if let Some(mut light) = self.world.get_mut::<PointLight>(handle.entity()) {
            light.intensity = intensity;
        }
    }

    /// Read current spot light intensity.
    pub fn spot_light_intensity<C>(&self, handle: &Handle<C>) -> Option<f32> {
        self.world
            .get::<SpotLight>(handle.entity())
            .map(|l| l.intensity)
    }

    /// Set spot light intensity.
    ///
    /// This is the immediate preview path for interactive scrubbing.
    pub fn set_spot_light_intensity<C>(&mut self, handle: &Handle<C>, intensity: f32) {
        if let Some(mut light) = self.world.get_mut::<SpotLight>(handle.entity()) {
            light.intensity = intensity;
        }
    }

    /// Read current directional light illuminance.
    pub fn directional_light_illuminance<C>(&self, handle: &Handle<C>) -> Option<f32> {
        self.world
            .get::<DirectionalLight>(handle.entity())
            .map(|l| l.illuminance)
    }

    /// Set directional light illuminance.
    ///
    /// This is the immediate preview path for interactive scrubbing.
    pub fn set_directional_light_illuminance<C>(&mut self, handle: &Handle<C>, illuminance: f32) {
        if let Some(mut light) = self.world.get_mut::<DirectionalLight>(handle.entity()) {
            light.illuminance = illuminance;
        }
    }

    /// Read current ambient light brightness.
    pub fn ambient_light_brightness<C>(&self, _handle: &Handle<C>) -> Option<f32> {
        self.world
            .get_resource::<AmbientLight>()
            .map(|l| l.brightness)
    }

    /// Set ambient light brightness.
    ///
    /// This is the immediate preview path for interactive scrubbing.
    pub fn set_ambient_light_brightness<C>(&mut self, _handle: &Handle<C>, brightness: f32) {
        if let Some(mut light) = self.world.get_resource_mut::<AmbientLight>() {
            light.brightness = brightness;
        }
    }

    /// Read material base color for mesh-based entities (e.g. cube, cylinder).
    pub fn material_base_color<C>(&self, handle: &Handle<C>) -> Option<XrdsColor> {
        material_base_color_in_world(self.world, handle)
    }

    /// Read authored material parameters for mesh-based entities.
    pub fn material_params<C>(&self, handle: &Handle<C>) -> Option<XrdsMaterialParams> {
        material_params_in_world(self.world, handle)
    }

    /// Read advanced XRDS-native PBR settings for mesh-based entities.
    pub fn material_pbr_params<C>(&self, handle: &Handle<C>) -> Option<XrdsMaterialPbrParams> {
        material_pbr_params_in_world(self.world, handle)
    }

    /// Set material base color for mesh-based entities (e.g. cube, cylinder).
    ///
    /// This is the immediate preview path for inspector scrubbing and hover previews.
    pub fn set_material_base_color<C>(&mut self, handle: &Handle<C>, color: XrdsColor) {
        set_material_base_color_in_world(self.world, handle, color);
    }

    /// Read material emissive color for mesh-based entities.
    pub fn material_emissive<C>(&self, handle: &Handle<C>) -> Option<XrdsLinearRgba> {
        material_emissive_in_world(self.world, handle)
    }

    /// Set material emissive color for mesh-based entities.
    ///
    /// This is the immediate preview path for inspector scrubbing and hover previews.
    pub fn set_material_emissive<C>(&mut self, handle: &Handle<C>, emissive: XrdsLinearRgba) {
        set_material_emissive_in_world(self.world, handle, emissive);
    }

    /// Set advanced XRDS-native PBR settings for mesh-based entities.
    ///
    /// This is the immediate preview path for inspector scrubbing and hover previews.
    pub fn set_material_pbr_params<C>(&mut self, handle: &Handle<C>, pbr: XrdsMaterialPbrParams) {
        set_material_pbr_params_in_world(self.world, handle, pbr);
    }

    /// Set authored material parameters for mesh-based entities.
    ///
    /// This is the immediate preview path for inspector-driven material previews.
    pub fn set_material_params<C>(&mut self, handle: &Handle<C>, params: XrdsMaterialParams) {
        set_material_params_in_world(self.world, handle, params);
    }

    /// Read all texture slots for mesh-based entities.
    pub fn material_textures<C>(&self, handle: &Handle<C>) -> Option<XrdsMaterialTextureSlots> {
        material_textures_in_world(self.world, handle)
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
    ) {
        set_material_texture_slot_in_world(self.world, handle, slot, texture);
    }

    /// Replace all texture slots at once for mesh-based entities.
    ///
    /// This is the immediate preview path for inspector panels that operate on the full
    /// texture set (e.g. when importing a material preset).
    pub fn set_material_textures<C>(
        &mut self,
        handle: &Handle<C>,
        textures: XrdsMaterialTextureSlots,
    ) {
        set_material_textures_in_world(self.world, handle, textures);
    }

    /// Resolve XRDS id from a typed handle.
    pub fn id_of<C>(&self, handle: &Handle<C>) -> Option<XrdsId> {
        id_of_in_world(self.world, handle)
    }

    /// Resolve a Bevy entity from an XRDS id.
    pub fn entity_of_id(&self, id: XrdsId) -> Option<Entity> {
        entity_of_id_in_world(self.world, id)
    }

    /// Resolve a typed handle from an XRDS id.
    pub fn handle_of<C>(&self, id: XrdsId) -> Option<Handle<C>> {
        handle_of_id_in_world(self.world, id)
    }

    /// Pause or unpause the physics simulation globally.
    /// Pausing stops all rigid-body integration — useful for edit-mode authoring so
    /// Dynamic objects stay at their authored positions instead of falling immediately.
    pub fn set_physics_paused(&mut self, paused: bool) {
        use bevy::prelude::Time;
        use avian3d::prelude::{Physics, PhysicsTime};
        if let Some(mut pt) = self.world.get_resource_mut::<Time<Physics>>() {
            if paused { pt.pause(); } else { pt.unpause(); }
        }
    }

    /// Set the gravity multiplier on a dynamic physics entity (live update, no reimport).
    /// Has no effect if the entity has no `RigidBody::Dynamic` component.
    pub fn set_gravity_scale_for_node(&mut self, id: XrdsId, scale: f32) {
        let Some(entity) = self.world.resource::<XrdsIdIndex>().entity_of(id) else { return; };
        self.world.entity_mut(entity).insert(avian3d::prelude::GravityScale(scale));
    }

    /// Set the mass (kg) on a dynamic physics entity (live update, no reimport).
    /// Has no effect if the entity has no `RigidBody::Dynamic` component.
    pub fn set_mass_for_node(&mut self, id: XrdsId, mass_kg: f32) {
        let Some(entity) = self.world.resource::<XrdsIdIndex>().entity_of(id) else { return; };
        self.world.entity_mut(entity).insert(avian3d::prelude::Mass(mass_kg));
    }

    /// Update the perspective FOV on a camera entity (live preview).
    /// Modifies `Projection::Perspective.fov` in-place without reimporting.
    pub fn set_camera_fov_for_node(&mut self, id: XrdsId, fov_deg: f32) {
        use bevy::prelude::Projection;
        let Some(entity) = self.world.resource::<XrdsIdIndex>().entity_of(id) else { return; };
        if let Some(mut proj) = self.world.get_mut::<Projection>(entity) {
            if let Projection::Perspective(ref mut p) = *proj {
                p.fov = fov_deg.to_radians();
            }
        }
    }

    /// Return the animation clip names for a GLTF entity, in clip-index order.
    ///
    /// Returns an empty vec if the entity was not spawned by XRDS, if the GLTF
    /// asset is not yet fully loaded, or if the asset has no animations.
    pub fn gltf_clip_names(&self, id: XrdsId) -> Vec<(usize, String)> {
        use bevy::gltf::Gltf;
        use bevy::prelude::Assets;
        use crate::xrds_api::state::XrdsStoredGltfHandle;

        let Some(entity) = self.world.resource::<XrdsIdIndex>().entity_of(id) else {
            return Vec::new();
        };
        let Some(handle) = self.world.get::<XrdsStoredGltfHandle>(entity) else {
            return Vec::new();
        };
        let Some(gltf) = self.world.get_resource::<Assets<Gltf>>().and_then(|a| a.get(&handle.0)) else {
            return Vec::new();
        };
        gltf.animations
            .iter()
            .enumerate()
            .map(|(i, anim_h)| {
                let name = gltf
                    .named_animations
                    .iter()
                    .find(|(_, h)| h.id() == anim_h.id())
                    .map(|(n, _)| n.to_string())
                    .unwrap_or_else(|| format!("Clip {i}"));
                (i, name)
            })
            .collect()
    }

    /// Read an arbitrary Bevy resource from the live world.
    ///
    /// This is the escape hatch for `XrdsApp::update()` implementations that
    /// need to react to custom app-level resources (e.g. editor state, input
    /// queues) without direct `World` access.  Returns `None` if the resource
    /// has not been inserted.
    pub fn resource<T: bevy::prelude::Resource>(&self) -> Option<&T> {
        self.world.get_resource::<T>()
    }

    /// Mutably access an arbitrary Bevy resource from `update()`.
    pub fn resource_mut<T: bevy::prelude::Resource>(&mut self) -> Option<bevy::prelude::Mut<T>> {
        self.world.get_resource_mut::<T>()
    }

    /// Despawn all XRDS entities and re-spawn everything from `document`.
    ///
    /// Use this when the document has structural changes (new or deleted nodes)
    /// that cannot be applied with incremental `set_*_for_node` calls.
    pub fn reimport_scene(
        &mut self,
        document: &XrdsSceneDocument,
    ) -> Result<Vec<XrdsId>, XrdsSceneImportError> {
        reimport::reimport_scene_in_world(self.world, document)
    }

    /// Spawn a single new node from `document` without despawning any existing
    /// entities.  Use this for incremental additions such as palette placement.
    /// `id` must be the `XrdsId` of a node that exists in `document` but has
    /// not yet been registered in the XRDS runtime.
    pub fn spawn_document_node(
        &mut self,
        id: XrdsId,
        document: &XrdsSceneDocument,
    ) -> Result<XrdsId, XrdsSceneImportError> {
        reimport::spawn_document_node_in_world(self.world, document, id)
    }

    /// Queue a typed update patch from within `on_update`.
    ///
    /// This is the authoritative commit path. Use it for edits that should land as XRDS patches,
    /// especially final editor commits, multi-object edits, and any mutation that should be
    /// recorded in document/undo state rather than applied as a transient preview.
    pub fn queue_update<C, P>(&mut self, handle: &Handle<C>, patch: P)
    where
        C: XrdsComponent + Send + Sync + 'static,
        P: Send + Sync + 'static,
    {
        queue_update_in_world(self.world, handle, patch);
    }

    /// Read current camera projection settings.
    pub fn camera_projection(&self, handle: &Handle<XrdsCamera>) -> Option<CameraProjectionParams> {
        camera_projection_in_world(self.world, handle)
    }

    /// Read current camera look-at target.
    ///
    /// Returns `None` if the entity does not exist.
    /// Returns `Some(None)` if the camera exists but look-at is not active.
    pub fn camera_look_at(&self, handle: &Handle<XrdsCamera>) -> Option<Option<[f32; 3]>> {
        camera_look_at_in_world(self.world, handle)
    }

    /// Queue a camera projection update.
    pub fn set_camera_projection(
        &mut self,
        handle: &Handle<XrdsCamera>,
        projection: CameraProjectionParams,
    ) {
        self.queue_update(handle, CameraProjectionPatch { projection });
    }

    /// Queue a perspective camera projection update.
    pub fn set_camera_perspective(
        &mut self,
        handle: &Handle<XrdsCamera>,
        params: PerspectiveCameraParams,
    ) {
        self.set_camera_projection(handle, CameraProjectionParams::Perspective(params));
    }

    /// Queue an orthographic camera projection update.
    pub fn set_camera_orthographic(
        &mut self,
        handle: &Handle<XrdsCamera>,
        params: OrthographicCameraParams,
    ) {
        self.set_camera_projection(handle, CameraProjectionParams::Orthographic(params));
    }

    /// Queue a camera look-at update.
    pub fn set_camera_look_at(&mut self, handle: &Handle<XrdsCamera>, target: Option<[f32; 3]>) {
        self.queue_update(handle, CameraLookAtPatch { look_at: target });
    }

    /// Read current glTF asset source path and scene index.
    pub fn gltf_source(&self, handle: &Handle<XrdsGltfAsset>) -> Option<GltfAssetSourcePatch> {
        gltf_source_in_world(self.world, handle)
    }

    /// Queue a glTF source update.
    pub fn set_gltf_source(
        &mut self,
        handle: &Handle<XrdsGltfAsset>,
        gltf_asset_path: impl Into<String>,
        scene_index: usize,
    ) {
        self.queue_update(
            handle,
            GltfAssetSourcePatch {
                gltf_asset_path: gltf_asset_path.into(),
                scene_index,
            },
        );
    }

    pub fn gltf_load_status(&self, handle: &Handle<XrdsGltfAsset>) -> Option<XrdsGltfLoadStatus> {
        gltf_load_status_in_world(self.world, handle)
    }

    pub fn gltf_animations(
        &self,
        handle: &Handle<XrdsGltfAsset>,
    ) -> Result<Vec<XrdsGltfAnimationInfo>, XrdsGltfRuntimeError> {
        gltf_animations_in_world(self.world, handle)
    }

    pub fn play_gltf_animation(
        &mut self,
        handle: &Handle<XrdsGltfAsset>,
        selector: XrdsGltfAnimationSelector,
        options: XrdsGltfAnimationPlaybackOptions,
    ) -> Result<(), XrdsGltfRuntimeError> {
        play_gltf_animation_in_world(self.world, handle, selector, options)
    }

    pub fn stop_gltf_animation(
        &mut self,
        handle: &Handle<XrdsGltfAsset>,
    ) -> Result<(), XrdsGltfRuntimeError> {
        stop_gltf_animation_in_world(self.world, handle)
    }

    pub fn pause_gltf_animation(
        &mut self,
        handle: &Handle<XrdsGltfAsset>,
    ) -> Result<(), XrdsGltfRuntimeError> {
        pause_gltf_animation_in_world(self.world, handle)
    }

    pub fn resume_gltf_animation(
        &mut self,
        handle: &Handle<XrdsGltfAsset>,
    ) -> Result<(), XrdsGltfRuntimeError> {
        resume_gltf_animation_in_world(self.world, handle)
    }

    pub fn gltf_animation_state(
        &self,
        handle: &Handle<XrdsGltfAsset>,
    ) -> Result<Option<XrdsGltfAnimationState>, XrdsGltfRuntimeError> {
        gltf_animation_state_in_world(self.world, handle)
    }

    pub fn gltf_morph_targets(
        &self,
        handle: &Handle<XrdsGltfAsset>,
    ) -> Result<Vec<XrdsGltfMorphTargetSet>, XrdsGltfRuntimeError> {
        gltf_morph_targets_in_world(self.world, handle)
    }

    pub fn gltf_morph_target_weights(
        &self,
        handle: &Handle<XrdsGltfAsset>,
    ) -> Result<Vec<XrdsGltfMorphTargetWeights>, XrdsGltfRuntimeError> {
        gltf_morph_target_weights_in_world(self.world, handle)
    }

    pub fn set_gltf_morph_target_weight(
        &mut self,
        handle: &Handle<XrdsGltfAsset>,
        node: &XrdsGltfNodeLocator,
        mesh_name: Option<&str>,
        selector: XrdsGltfMorphTargetSelector,
        weight: f32,
    ) -> Result<(), XrdsGltfRuntimeError> {
        set_gltf_morph_target_weight_in_world(self.world, handle, node, mesh_name, selector, weight)
    }

    /// Update the displayed text of a named HUD item on a specific `PlayerAnchor`.
    ///
    /// `anchor_id` — the `XrdsId` of the `PlayerAnchor` node.
    /// `item_name` — the element's authored name (e.g. `"hp"`, `"status"`).
    /// `text`      — the new text content.
    /// `color`     — optional RGBA override; `None` keeps the authored color.
    ///
    /// Does nothing if the anchor has no linked panel instance or the name is not
    /// found.
    ///
    /// **Name and contract deliberately unchanged** through the panel-template
    /// unification. It resolves against `XrdsStoredHudInstance`, which was always
    /// keyed by name, and the panel path populates exactly the same component —
    /// so what used to be an `XrdsHudItemDef` is now a `Label` element and this
    /// call is unaffected. That is why unification cost the public API nothing.
    pub fn set_hud_item(
        &mut self,
        anchor_id: XrdsId,
        item_name: &str,
        text: &str,
        color: Option<[f32; 4]>,
    ) {
        use bevy_rich_text3d::{Text3d, Text3dStyling};
        use bevy::color::Srgba;

        let anchor_entity = match self.world.resource::<XrdsIdIndex>().entity_of(anchor_id) {
            Some(e) => e,
            None => return,
        };

        let item_entity = {
            let hud = match self.world.get::<XrdsStoredHudInstance>(anchor_entity) {
                Some(h) => h,
                None => return,
            };
            hud.items.iter()
                .find(|(name, _)| name == item_name)
                .map(|(_, e)| *e)
        };

        let Some(item_entity) = item_entity else { return; };

        let text = text.to_string();
        if let Some(mut t3d) = self.world.get_mut::<Text3d>(item_entity) {
            *t3d = Text3d::new(text);
        }
        if let Some([r, g, b, a]) = color {
            if let Some(mut styling) = self.world.get_mut::<Text3dStyling>(item_entity) {
                styling.color = Srgba::new(r, g, b, a);
            }
        }
    }

    /// Mark an entity as pick-up-able by the XR grab system.
    ///
    /// Use this from `update()` to enable grabbing at runtime — for example after a proximity
    /// check or when entering a specific game state.
    pub fn make_grabbable(&mut self, id: XrdsId) -> &mut Self {
        if let Some(entity) = self.world.resource::<XrdsIdIndex>().entity_of(id) {
            if let Ok(mut e) = self.world.get_entity_mut(entity) {
                e.insert(xrds_components::XrGrabbable);
            }
        }
        self
    }

    /// Remove the `XrGrabbable` marker from an entity.
    pub fn make_ungrabable(&mut self, id: XrdsId) -> &mut Self {
        if let Some(entity) = self.world.resource::<XrdsIdIndex>().entity_of(id) {
            if let Ok(mut e) = self.world.get_entity_mut(entity) {
                e.remove::<xrds_components::XrGrabbable>();
            }
        }
        self
    }

    /// Cast a ray against all XRDS scene entities and return hits sorted nearest-first.
    ///
    /// Uses world-space AABB intersection. One hit per XRDS entity — GLTF submesh children
    /// all resolve to their common XRDS ancestor, so only the closest hit per entity is kept.
    ///
    /// # Example
    /// ```ignore
    /// if let Some(xr) = ctx.resource::<XrInput>() {
    ///     if let Some(pose) = xr.right.pose {
    ///         let hits = ctx.raycast(pose.translation, pose.rotation * Vec3::NEG_Z, 5.0);
    ///         if let Some(hit) = hits.first() {
    ///             // hit.id, hit.distance, hit.point
    ///         }
    ///     }
    /// }
    /// ```
    pub fn raycast(&mut self, origin: Vec3, direction: Vec3, max_distance: f32) -> Vec<XrRayhit> {
        super::raycast::raycast_world(self.world, origin, direction, max_distance)
    }

    /// Return a random world-space position within a randomly chosen `PlayerSpawnZone` in the scene.
    ///
    /// Picks from all zones regardless of ownership. Y is not randomised.
    /// Returns `None` if no spawn zones are present.
    pub fn random_spawn_zone_position(&self) -> Option<Vec3> {
        random_spawn_zone_position_in_world(self.world, None)
    }

    /// Return a random spawn position from zones designated for `player_node_id`,
    /// falling back to shared zones (no owner) when no designated zones exist.
    pub fn random_spawn_zone_position_for(&self, player_node_id: u64) -> Option<Vec3> {
        random_spawn_zone_position_in_world(self.world, Some(player_node_id))
    }

    /// Teleport the player (the entity tagged `XrdsPlayerRoot`) to `position`.
    pub fn teleport_player(&mut self, position: Vec3) {
        teleport_player_in_world(self.world, position);
    }

    // ── World-space UI ────────────────────────────────────────────────────────

    /// Iterate button press events fired this frame.
    ///
    /// Compare `ev.button_entity == btn.entity()` to identify the source button.
    ///
    /// # Example
    /// ```ignore
    /// for ev in ctx.world_button_presses() {
    ///     if ev.button_entity == start_btn.entity() {
    ///         ctx.set_world_label_text(&status_lbl, "Started!");
    ///     }
    /// }
    /// ```
    pub fn world_button_presses(
        &self,
    ) -> impl Iterator<Item = &xrds_components::XrWorldButtonPressEvent> {
        use bevy::ecs::message::Messages;
        self.world
            .get_resource::<Messages<xrds_components::XrWorldButtonPressEvent>>()
            .into_iter()
            .flat_map(|m| m.iter_current_update_messages())
    }

    /// Iterate button release events fired this frame.
    pub fn world_button_releases(
        &self,
    ) -> impl Iterator<Item = &xrds_components::XrWorldButtonReleaseEvent> {
        use bevy::ecs::message::Messages;
        self.world
            .get_resource::<Messages<xrds_components::XrWorldButtonReleaseEvent>>()
            .into_iter()
            .flat_map(|m| m.iter_current_update_messages())
    }

    /// Replace the text content of a world-space label.
    ///
    /// The label entity is re-meshed by `bevy_rich_text3d` on the next frame.
    pub fn set_world_label_text(
        &mut self,
        handle: &Handle<xrds_components::XrdsWorldLabel>,
        text: impl Into<String>,
    ) {
        use bevy_rich_text3d::Text3d;
        let entity = handle.entity();
        if let Ok(mut e) = self.world.get_entity_mut(entity) {
            e.insert(Text3d::new(text.into()));
        }
    }

    /// Iterate slider change events fired this frame.
    ///
    /// # Example
    /// ```ignore
    /// for ev in ctx.world_slider_changes() {
    ///     if ev.slider_entity == vol_slider.entity() {
    ///         set_volume(ev.value);
    ///     }
    /// }
    /// ```
    pub fn world_slider_changes(
        &self,
    ) -> impl Iterator<Item = &xrds_components::XrWorldSliderChangeEvent> {
        use bevy::ecs::message::Messages;
        self.world
            .get_resource::<Messages<xrds_components::XrWorldSliderChangeEvent>>()
            .into_iter()
            .flat_map(|m| m.iter_current_update_messages())
    }

    /// Iterate toggle state-change events fired this frame.
    ///
    /// # Example
    /// ```ignore
    /// for ev in ctx.world_toggle_events() {
    ///     if ev.toggle_entity == shadows_tog.entity() {
    ///         ctx.set_shadows_enabled(ev.checked);
    ///     }
    /// }
    /// ```
    pub fn world_toggle_events(
        &self,
    ) -> impl Iterator<Item = &xrds_components::XrWorldToggleEvent> {
        use bevy::ecs::message::Messages;
        self.world
            .get_resource::<Messages<xrds_components::XrWorldToggleEvent>>()
            .into_iter()
            .flat_map(|m| m.iter_current_update_messages())
    }

    /// Set a slider to a specific value and reposition its thumb immediately.
    ///
    /// Does not fire [`XrWorldSliderChangeEvent`].
    pub fn set_world_slider_value(
        &mut self,
        handle: &Handle<xrds_components::XrdsWorldSlider>,
        value: f32,
    ) {
        super::world_ui_slider::set_slider_value_in_world(self.world, handle.entity(), value);
    }

    /// Set a toggle's checked state and update visuals immediately.
    ///
    /// Does not fire [`XrWorldToggleEvent`].
    pub fn set_world_toggle(
        &mut self,
        handle: &Handle<xrds_components::XrdsWorldToggle>,
        checked: bool,
    ) {
        super::world_ui_toggle::set_toggle_in_world(self.world, handle.entity(), checked);
    }

    /// Change the layout policy on a world panel at runtime.
    ///
    /// Takes effect on the next frame when the layout system runs.
    pub fn set_world_panel_layout(
        &mut self,
        panel: &Handle<xrds_components::XrdsWorldPanel>,
        layout: xrds_components::XrdsWorldLayout,
    ) {
        let entity = panel.entity();
        if let Ok(mut e) = self.world.get_entity_mut(entity) {
            e.insert(layout);
        }
    }

    /// Fires a trigger on a node directly, without waiting for the real
    /// event that would normally produce it (a zone collision, a grab, a
    /// button press, …). Runs every matching, non-disabled binding on that
    /// node, exactly as [`super::trigger_action::consume_triggers`] would.
    /// Returns how many sequences/timelines it started, so a caller can
    /// tell "nothing was bound" from "it ran".
    ///
    /// The [`XrdsAPI`](super::api::XrdsAPI) counterpart of this exists for
    /// setup-time use; this is the `update()`-time equivalent — e.g. an
    /// editor's "preview this trigger" button, which has no other way to
    /// generate a real `ZoneEnter`/`Grabbed`/etc event from a desktop UI.
    pub fn fire_trigger(
        &mut self,
        node: XrdsId,
        kind: &xrds_scene_graph::XrdsTriggerKind,
        hand: Option<xrds_components::XrGrabHand>,
    ) -> usize {
        super::trigger_action::fire_trigger_in_world(self.world, node, kind, hand)
    }

    /// Starts a named Track as an editor preview, replacing any current one.
    ///
    /// Deliberately distinct from play mode: previewing one Track is not running
    /// the simulation. Goes through the ordinary asset-conflict guard, so a
    /// preview of a Track whose assets are already held is refused exactly as a
    /// real firing would be — the preview should show what would actually
    /// happen, including the refusal.
    ///
    /// Returns `false` when there was nothing to preview.
    pub fn preview_play_track(&mut self, name: &str) -> bool {
        super::trigger_action::preview_play_track_in_world(self.world, name).is_some()
    }

    /// Pauses or resumes the preview. A paused Track keeps its asset locks.
    pub fn preview_pause_track(&mut self, paused: bool) -> bool {
        super::trigger_action::preview_pause_track_in_world(self.world, paused)
    }

    /// Stops the preview and returns every node it was driving, so the caller
    /// can restore those nodes from its authored document. The runtime cannot do
    /// that itself — only the editor holds the document to restore from.
    pub fn preview_stop_track(&mut self) -> Vec<XrdsId> {
        super::trigger_action::preview_stop_track_in_world(self.world)
    }

    /// `(name, elapsed_secs, duration_secs, playing)` for the preview, or `None`
    /// when nothing is previewing. Drives the transport readout and playhead.
    pub fn track_preview_state(&mut self) -> Option<(String, f32, f32, bool)> {
        super::trigger_action::track_preview_state_in_world(self.world)
    }
}
