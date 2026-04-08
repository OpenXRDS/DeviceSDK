use super::*;

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
}
