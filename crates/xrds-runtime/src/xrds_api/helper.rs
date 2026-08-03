use super::*;
use std::collections::{BTreeMap, HashMap, HashSet};

fn editor_metadata_for_entity_in_world(world: &World, entity: Entity) -> XrdsEditorMetadata {
    world
        .get::<XrdsStoredEditorMetadata>(entity)
        .map(|stored| stored.0.clone())
        .unwrap_or_default()
}

fn scene_gltf_playback_from_pending_request(
    request: &PendingGltfAnimationRequest,
) -> xrds_scene_graph::XrdsSceneGltfPlayback {
    xrds_scene_graph::XrdsSceneGltfPlayback {
        selector: (&request.selector).into(),
        repeat: request.options.repeat.into(),
        speed: request.options.speed,
        start_paused: request.options.start_paused,
    }
}

fn scene_gltf_playback_from_active_state(
    state: &XrdsGltfAnimationState,
) -> xrds_scene_graph::XrdsSceneGltfPlayback {
    xrds_scene_graph::XrdsSceneGltfPlayback {
        selector: match &state.animation.name {
            Some(name) => xrds_scene_graph::XrdsSceneGltfAnimationSelector::Name(name.clone()),
            None => xrds_scene_graph::XrdsSceneGltfAnimationSelector::Index(state.animation.index),
        },
        repeat: state.repeat.into(),
        speed: state.speed,
        start_paused: state.paused,
    }
}

fn export_gltf_default_playback_for_entity_in_world(
    world: &World,
    entity: Entity,
    stored: Option<xrds_scene_graph::XrdsSceneGltfPlayback>,
) -> Option<xrds_scene_graph::XrdsSceneGltfPlayback> {
    if let Some(state) = world
        .get_resource::<ActiveGltfAnimationStates>()
        .and_then(|resource| resource.states.get(&entity))
    {
        return Some(scene_gltf_playback_from_active_state(state));
    }

    if let Some(request) = world
        .get_resource::<PendingGltfAnimationRequests>()
        .and_then(|resource| resource.requests.get(&entity))
    {
        return Some(scene_gltf_playback_from_pending_request(request));
    }

    stored
}

fn apply_editor_metadata_to_node(
    world: &World,
    entity: Entity,
    mut node: XrdsSceneNode,
) -> XrdsSceneNode {
    node.editor = editor_metadata_for_entity_in_world(world, entity);
    node
}

fn asset_catalog_id_seed(uri: &str) -> String {
    let mut seed = String::with_capacity(uri.len());
    let mut previous_was_separator = false;

    for character in uri.chars() {
        if character.is_ascii_alphanumeric() {
            seed.push(character.to_ascii_lowercase());
            previous_was_separator = false;
        } else if !previous_was_separator {
            seed.push('-');
            previous_was_separator = true;
        }
    }

    let trimmed = seed.trim_matches('-');
    if trimmed.is_empty() {
        "gltf-asset".to_string()
    } else {
        format!("gltf-{trimmed}")
    }
}

fn unique_asset_id(preferred: String, used_ids: &mut HashSet<String>) -> String {
    if used_ids.insert(preferred.clone()) {
        return preferred;
    }

    let mut counter = 2usize;
    loop {
        let candidate = format!("{preferred}-{counter}");
        if used_ids.insert(candidate.clone()) {
            return candidate;
        }
        counter += 1;
    }
}

fn reconstruct_asset_catalog(nodes: &[XrdsSceneNode]) -> Vec<XrdsSceneAsset> {
    let mut assets = Vec::new();
    let mut asset_index_by_uri = HashMap::<String, usize>::new();
    let mut used_ids = HashSet::new();

    for node in nodes {
        let XrdsSceneNodePayload::GltfAsset(asset) = &node.payload else {
            continue;
        };

        if asset_index_by_uri.contains_key(&asset.asset_uri) {
            continue;
        }

        let preferred_id = node
            .editor
            .source
            .as_ref()
            .and_then(|source| source.asset_id.clone())
            .filter(|asset_id| !asset_id.trim().is_empty())
            .unwrap_or_else(|| asset_catalog_id_seed(&asset.asset_uri));

        let id = unique_asset_id(preferred_id, &mut used_ids);
        asset_index_by_uri.insert(asset.asset_uri.clone(), assets.len());
        assets.push(XrdsSceneAsset {
            id,
            uri: asset.asset_uri.clone(),
            kind: XrdsSceneAssetKind::Gltf,
        });
    }

    assets
}

fn merge_asset_catalogs(
    preserved_assets: &[XrdsSceneAsset],
    reconstructed_assets: Vec<XrdsSceneAsset>,
) -> Vec<XrdsSceneAsset> {
    let mut assets = reconstructed_assets;
    let mut used_ids: HashSet<String> = assets.iter().map(|asset| asset.id.clone()).collect();

    for asset in preserved_assets {
        if used_ids.contains(&asset.id) {
            continue;
        }

        used_ids.insert(asset.id.clone());
        assets.push(asset.clone());
    }

    assets
}

pub(super) fn merge_imported_asset_catalog(world: &mut World, assets: &[XrdsSceneAsset]) {
    let existing_assets = world.resource::<XrdsImportedAssetCatalog>().assets.clone();
    let merged = merge_asset_catalogs(assets, existing_assets);
    world.resource_mut::<XrdsImportedAssetCatalog>().assets = merged;
}

pub(super) fn id_of_in_world<C>(world: &World, handle: &Handle<C>) -> Option<XrdsId> {
    world.resource::<XrdsIdIndex>().id_of(handle.entity())
}

pub(super) fn entity_of_id_in_world(world: &World, id: XrdsId) -> Option<Entity> {
    world.resource::<XrdsIdIndex>().entity_of(id)
}

pub(super) fn handle_of_id_in_world<C>(world: &World, id: XrdsId) -> Option<Handle<C>> {
    entity_of_id_in_world(world, id).map(Handle::from)
}

pub(super) fn reserve_runtime_id_in_world(
    world: &mut World,
    id: XrdsId,
) -> Result<(), XrdsSceneImportError> {
    if world.resource::<XrdsIdIndex>().contains_id(id) {
        return Err(XrdsSceneImportError::DuplicateRuntimeId(id));
    }

    let next = world.resource::<XrdsIdAllocator>().next;
    if id.0 >= next {
        world.resource_mut::<XrdsIdAllocator>().next = id.0.saturating_add(1);
    }

    Ok(())
}

pub(super) fn export_scene_node_in_world(
    world: &World,
    id: XrdsId,
    entity: Entity,
    parent_id: Option<XrdsId>,
) -> Result<XrdsSceneNode, XrdsSceneExportError> {
    let node_id = id.into();
    let parent_node_id = parent_id.map(Into::into);

    if let Some(descriptor) = world.get::<XrdsStored<XrdsNode>>(entity) {
        return Ok(apply_editor_metadata_to_node(
            world,
            entity,
            XrdsSceneNode::from_xrds_node(node_id, parent_node_id, &descriptor.0),
        ));
    }

    if let Some(descriptor) = world.get::<XrdsStored<XrdsCamera>>(entity) {
        return Ok(apply_editor_metadata_to_node(
            world,
            entity,
            XrdsSceneNode::from_xrds_camera(node_id, parent_node_id, &descriptor.0),
        ));
    }

    if let Some(descriptor) = world.get::<XrdsStored<XrdsGltfAsset>>(entity) {
        return Ok(apply_editor_metadata_to_node(
            world,
            entity,
            XrdsSceneNode::from_xrds_gltf_asset(node_id, parent_node_id, &descriptor.0),
        ));
    }

    if let Some(descriptor) = world.get::<XrdsStored<XrdsCube>>(entity) {
        return Ok(apply_editor_metadata_to_node(
            world,
            entity,
            XrdsSceneNode::from_xrds_cube(
                node_id,
                parent_node_id,
                &descriptor.0,
                material_params_for_entity_in_world(world, entity),
            ),
        ));
    }

    if let Some(descriptor) = world.get::<XrdsStored<XrdsCylinder>>(entity) {
        return Ok(apply_editor_metadata_to_node(
            world,
            entity,
            XrdsSceneNode::from_xrds_cylinder(
                node_id,
                parent_node_id,
                &descriptor.0,
                material_params_for_entity_in_world(world, entity),
            ),
        ));
    }

    if let Some(descriptor) = world.get::<XrdsStored<XrdsSphere>>(entity) {
        return Ok(apply_editor_metadata_to_node(
            world,
            entity,
            XrdsSceneNode::from_xrds_sphere(
                node_id,
                parent_node_id,
                &descriptor.0,
                material_params_for_entity_in_world(world, entity),
            ),
        ));
    }

    if let Some(descriptor) = world.get::<XrdsStored<XrdsPlane3D>>(entity) {
        return Ok(apply_editor_metadata_to_node(
            world,
            entity,
            XrdsSceneNode::from_xrds_plane3d(
                node_id,
                parent_node_id,
                &descriptor.0,
                material_params_for_entity_in_world(world, entity),
            ),
        ));
    }

    if let Some(descriptor) = world.get::<XrdsStored<XrdsTetrahedron>>(entity) {
        return Ok(apply_editor_metadata_to_node(
            world,
            entity,
            XrdsSceneNode::from_xrds_tetrahedron(
                node_id,
                parent_node_id,
                &descriptor.0,
                material_params_for_entity_in_world(world, entity),
            ),
        ));
    }

    if let Some(descriptor) = world.get::<XrdsStored<XrdsAmbientLight>>(entity) {
        return Ok(apply_editor_metadata_to_node(
            world,
            entity,
            XrdsSceneNode::from_xrds_ambient_light(node_id, parent_node_id, &descriptor.0),
        ));
    }

    if let Some(descriptor) = world.get::<XrdsStored<XrdsDirectionalLight>>(entity) {
        return Ok(apply_editor_metadata_to_node(
            world,
            entity,
            XrdsSceneNode::from_xrds_directional_light(node_id, parent_node_id, &descriptor.0),
        ));
    }

    if let Some(descriptor) = world.get::<XrdsStored<XrdsPointLight>>(entity) {
        return Ok(apply_editor_metadata_to_node(
            world,
            entity,
            XrdsSceneNode::from_xrds_point_light(node_id, parent_node_id, &descriptor.0),
        ));
    }

    if let Some(descriptor) = world.get::<XrdsStored<XrdsSpotLight>>(entity) {
        return Ok(apply_editor_metadata_to_node(
            world,
            entity,
            XrdsSceneNode::from_xrds_spot_light(node_id, parent_node_id, &descriptor.0),
        ));
    }

    if let Some(descriptor) = world.get::<XrdsStored<XrdsAudioClip>>(entity) {
        return Ok(apply_editor_metadata_to_node(
            world,
            entity,
            XrdsSceneNode::from_xrds_audio_clip(node_id, parent_node_id, &descriptor.0),
        ));
    }

    if let Some(stored) = world.get::<XrdsStoredHudText>(entity) {
        let name = world
            .get::<bevy::prelude::Name>(entity)
            .map(|n| n.as_str())
            .unwrap_or("HUD Text");
        return Ok(apply_editor_metadata_to_node(
            world,
            entity,
            XrdsSceneNode::from_hud_text(node_id, parent_node_id, name, stored.0.clone()),
        ));
    }

    if let Some(descriptor) = world.get::<XrdsStored<XrdsText>>(entity) {
        return Ok(apply_editor_metadata_to_node(
            world,
            entity,
            XrdsSceneNode::from_xrds_text(node_id, parent_node_id, &descriptor.0),
        ));
    }

    if let Some(descriptor) = world.get::<XrdsStored<XrdsExtrudedText>>(entity) {
        return Ok(apply_editor_metadata_to_node(
            world,
            entity,
            XrdsSceneNode::from_xrds_extruded_text(node_id, parent_node_id, &descriptor.0),
        ));
    }

    Err(XrdsSceneExportError::UnsupportedRuntimeDescriptor(id))
}

pub(super) fn export_scene_document_in_world(
    world: &World,
    mut metadata: XrdsSceneMetadata,
) -> Result<XrdsSceneDocument, XrdsSceneExportError> {
    if metadata.environment.is_none() {
        metadata.environment = imported_scene_environment_in_world(world);
    }

    let hierarchy = world.resource::<XrdsHierarchyIndex>();
    let ids = world.resource::<XrdsIdIndex>();

    let mut ordered_ids: Vec<_> = ids.id_to_entity.keys().copied().collect();
    ordered_ids.sort_by_key(|id| id.0);

    let mut nodes = Vec::with_capacity(ordered_ids.len());
    let mut gltf_node_authoring = BTreeMap::new();
    for id in ordered_ids {
        let entity = ids
            .entity_of(id)
            .ok_or(XrdsSceneExportError::MissingRuntimeEntity(id))?;
        let parent_id = hierarchy.parent_id_of(id);
        let mut node = export_scene_node_in_world(world, id, entity, parent_id)?;
        if world.get::<xrds_components::XrGrabbable>(entity).is_some() {
            node.grabbable = true;
        }
        if let Some(bindings) =
            world.get::<crate::xrds_api::trigger_action::XrdsTriggerBindings>(entity)
        {
            node.triggers = bindings.0.clone();
        }
        if let Some(watchers) =
            world.get::<crate::xrds_api::trigger_action::XrdsThresholdWatchers>(entity)
        {
            node.watchers = watchers.0.clone();
        }
        if matches!(node.payload, XrdsSceneNodePayload::GltfAsset(_)) {
            let mut authoring = world
                .get::<XrdsStoredSceneGltfNodeAuthoring>(entity)
                .map(|stored| stored.0.clone())
                .unwrap_or_default();

            authoring.default_playback = export_gltf_default_playback_for_entity_in_world(
                world,
                entity,
                authoring.default_playback.clone(),
            );

            if let Some(runtime_overrides) =
                export_gltf_morph_target_overrides_for_entity_in_world(world, entity)?
            {
                authoring.morph_target_overrides = runtime_overrides;
            }

            if authoring.default_playback.is_some() || !authoring.morph_target_overrides.is_empty()
            {
                gltf_node_authoring.insert(id.0, authoring);
            }
        }
        nodes.push(node);
    }

    let assets = merge_asset_catalogs(
        &world.resource::<XrdsImportedAssetCatalog>().assets,
        reconstruct_asset_catalog(&nodes),
    );

    let hud_library = world
        .get_resource::<XrdsImportedHudLibrary>()
        .map(|r| r.templates.clone())
        .unwrap_or_default();

    let document = XrdsSceneDocument {
        metadata,
        assets,
        nodes,
        gltf_node_authoring,
        hud_library,
        ..Default::default()
    };
    document.validate()?;
    Ok(document)
}

pub(super) fn queue_update_in_world<C, P>(world: &mut World, handle: &Handle<C>, patch: P)
where
    C: XrdsComponent + Send + Sync + 'static,
    P: Send + Sync + 'static,
{
    world
        .resource_mut::<QueuedSurfaceUpdates>()
        .updates
        .push(QueuedSurfaceUpdate {
            entity: handle.entity(),
            component_type: TypeId::of::<C>(),
            patch_type: TypeId::of::<P>(),
            patch: Box::new(patch),
        });
}

pub(super) fn set_transform_in_world<C>(
    world: &mut World,
    handle: &Handle<C>,
    params: TransformParams,
) where
    C: XrdsComponent + Send + Sync + 'static,
{
    if !apply_surface_patch_now::<C, TransformParams>(world, handle.entity(), params) {
        apply_transform_to_entity(world, handle.entity(), params);
    }
}

pub(super) fn set_translation_in_world<C>(
    world: &mut World,
    handle: &Handle<C>,
    translation: [f32; 3],
) where
    C: XrdsComponent + Send + Sync + 'static,
{
    let mut params = match transform_params_for_entity(world, handle.entity()) {
        Some(params) => params,
        None => return,
    };
    params.translation = translation;
    set_transform_in_world(world, handle, params);
}

pub(super) fn set_rotation_in_world<C>(world: &mut World, handle: &Handle<C>, xyzw: [f32; 4])
where
    C: XrdsComponent + Send + Sync + 'static,
{
    let mut params = match transform_params_for_entity(world, handle.entity()) {
        Some(params) => params,
        None => return,
    };
    params.rotation_quat_xyzw = xyzw;
    let quat = Quat::from_xyzw(xyzw[0], xyzw[1], xyzw[2], xyzw[3]);
    let (x_deg, y_deg, z_deg) = quat.to_euler(EulerRot::XYZ);
    params.rotation_euler_xyz_deg = [x_deg.to_degrees(), y_deg.to_degrees(), z_deg.to_degrees()];
    set_transform_in_world(world, handle, params);
}

pub(super) fn set_scale_in_world<C>(world: &mut World, handle: &Handle<C>, scale: [f32; 3])
where
    C: XrdsComponent + Send + Sync + 'static,
{
    let mut params = match transform_params_for_entity(world, handle.entity()) {
        Some(params) => params,
        None => return,
    };
    params.scale = scale;
    set_transform_in_world(world, handle, params);
}

pub(super) fn set_visibility_in_world<C>(world: &mut World, handle: &Handle<C>, visible: bool)
where
    C: XrdsComponent + Send + Sync + 'static,
{
    if !apply_surface_patch_now::<C, VisibilityPatch>(
        world,
        handle.entity(),
        VisibilityPatch { visible },
    ) {
        world
            .entity_mut(handle.entity())
            .insert(crate::xrds_api::install::build_visibility_hierarchy_components(visible));
    }
}

pub(super) fn material_params_for_entity_in_world(
    world: &World,
    entity: Entity,
) -> Option<XrdsMaterialParams> {
    world
        .get::<XrdsStoredMaterial>(entity)
        .map(|material| material.0.clone())
}

pub(super) fn set_material_params_for_entity_in_world(
    world: &mut World,
    entity: Entity,
    params: XrdsMaterialParams,
) {
    apply_authored_material_to_entity(world, entity, params);
}

pub(super) fn material_params_in_world<C>(
    world: &World,
    handle: &Handle<C>,
) -> Option<XrdsMaterialParams> {
    material_params_for_entity_in_world(world, handle.entity())
}

pub(super) fn material_pbr_params_in_world<C>(
    world: &World,
    handle: &Handle<C>,
) -> Option<XrdsMaterialPbrParams> {
    material_params_in_world(world, handle).map(|params| params.pbr)
}

pub(super) fn set_material_params_in_world<C>(
    world: &mut World,
    handle: &Handle<C>,
    params: XrdsMaterialParams,
) {
    set_material_params_for_entity_in_world(world, handle.entity(), params);
}

pub(super) fn set_material_pbr_params_in_world<C>(
    world: &mut World,
    handle: &Handle<C>,
    pbr: XrdsMaterialPbrParams,
) {
    let mut params = material_params_in_world(world, handle).unwrap_or_default();
    params.pbr = pbr;
    set_material_params_in_world(world, handle, params);
}


pub(super) fn material_base_color_in_world<C>(
    world: &World,
    handle: &Handle<C>,
) -> Option<XrdsColor> {
    material_params_in_world(world, handle).map(|params| params.base_color)
}

pub(super) fn set_material_base_color_in_world<C>(
    world: &mut World,
    handle: &Handle<C>,
    color: XrdsColor,
) {
    let mut params = material_params_in_world(world, handle).unwrap_or_default();
    params.base_color = color;
    set_material_params_in_world(world, handle, params);
}

pub(super) fn material_emissive_in_world<C>(
    world: &World,
    handle: &Handle<C>,
) -> Option<XrdsLinearRgba> {
    material_params_in_world(world, handle).map(|params| params.emissive)
}

pub(super) fn material_textures_in_world<C>(
    world: &World,
    handle: &Handle<C>,
) -> Option<XrdsMaterialTextureSlots> {
    material_params_in_world(world, handle).map(|params| params.textures)
}

pub(super) fn set_material_texture_slot_in_world<C>(
    world: &mut World,
    handle: &Handle<C>,
    slot: XrdsMaterialTextureSlotKind,
    texture: Option<XrdsMaterialTextureRef>,
) {
    let mut params = material_params_in_world(world, handle).unwrap_or_default();
    params.textures.set(slot, texture);
    set_material_params_in_world(world, handle, params);
}

pub(super) fn set_material_textures_in_world<C>(
    world: &mut World,
    handle: &Handle<C>,
    textures: XrdsMaterialTextureSlots,
) {
    let mut params = material_params_in_world(world, handle).unwrap_or_default();
    params.textures = textures;
    set_material_params_in_world(world, handle, params);
}

pub(super) fn camera_projection_in_world(
    world: &World,
    handle: &Handle<XrdsCamera>,
) -> Option<CameraProjectionParams> {
    world
        .get::<XrdsStored<XrdsCamera>>(handle.entity())
        .map(|stored| stored.0.projection)
}

/// Returns `None` if the camera entity does not exist.
/// Returns `Some(None)` if the camera exists but look-at is not active.
pub(super) fn camera_look_at_in_world(
    world: &World,
    handle: &Handle<XrdsCamera>,
) -> Option<Option<[f32; 3]>> {
    world
        .get::<XrdsStored<XrdsCamera>>(handle.entity())
        .map(|stored| stored.0.look_at)
}

pub(super) fn gltf_source_in_world(
    world: &World,
    handle: &Handle<XrdsGltfAsset>,
) -> Option<GltfAssetSourcePatch> {
    world
        .get::<XrdsStored<XrdsGltfAsset>>(handle.entity())
        .map(|stored| GltfAssetSourcePatch {
            gltf_asset_path: stored.0.gltf_asset_path.clone(),
            scene_index: stored.0.scene_index,
        })
}

pub(super) fn point_light_params_in_world(
    world: &World,
    handle: &Handle<XrdsPointLight>,
) -> Option<PointLightParams> {
    world
        .get::<XrdsStored<XrdsPointLight>>(handle.entity())
        .map(|stored| PointLightParams {
            color: stored.0.color,
            intensity: stored.0.intensity,
            range: stored.0.range,
            radius: stored.0.radius,
            shadows: stored.0.shadows,
        })
}

pub(super) fn directional_light_params_in_world(
    world: &World,
    handle: &Handle<XrdsDirectionalLight>,
) -> Option<DirectionalLightParams> {
    world
        .get::<XrdsStored<XrdsDirectionalLight>>(handle.entity())
        .map(|stored| DirectionalLightParams {
            color: stored.0.color,
            illuminance: stored.0.illuminance,
            shadows: stored.0.shadows,
        })
}

pub(super) fn spot_light_params_in_world(
    world: &World,
    handle: &Handle<XrdsSpotLight>,
) -> Option<SpotLightParams> {
    world
        .get::<XrdsStored<XrdsSpotLight>>(handle.entity())
        .map(|stored| SpotLightParams {
            color: stored.0.color,
            intensity: stored.0.intensity,
            range: stored.0.range,
            inner_angle: stored.0.inner_angle,
            outer_angle: stored.0.outer_angle,
            shadows: stored.0.shadows,
        })
}

pub(super) fn ambient_light_params_in_world(
    world: &World,
    handle: &Handle<XrdsAmbientLight>,
) -> Option<AmbientLightParams> {
    world
        .get::<XrdsStored<XrdsAmbientLight>>(handle.entity())
        .map(|stored| AmbientLightParams {
            color: stored.0.color,
            brightness: stored.0.brightness,
            affects_baked_lighting: stored.0.affects_baked_lighting,
        })
}

pub(super) fn text_params_in_world(
    world: &World,
    handle: &Handle<XrdsText>,
) -> Option<TextParams> {
    world
        .get::<XrdsStored<XrdsText>>(handle.entity())
        .map(|stored| TextParams {
            text: stored.0.text.clone(),
            font_size: stored.0.font_size,
            color: stored.0.color,
            alignment: stored.0.alignment,
        })
}

pub(super) fn gltf_load_status_from_error(
    error: &Arc<bevy::asset::AssetLoadError>,
) -> XrdsGltfLoadStatus {
    XrdsGltfLoadStatus::Failed(error.to_string())
}

pub(super) fn gltf_load_status_for_entity_in_world(
    world: &World,
    entity: Entity,
) -> Option<XrdsGltfLoadStatus> {
    // Use the SceneRoot handle (Handle<Scene>) to check load state. The Scene
    // sub-asset is produced by the GLTF loader, so its state faithfully reflects
    // whether the .glb file has been parsed. Checking the Gltf parent handle
    // is not safe here because calling asset_server.load() and immediately
    // dropping the returned handle every frame can interfere with Bevy's
    // ref-count tracking and prevent the asset from ever reaching Loaded.
    let scene_root = world.get::<SceneRoot>(entity)?;
    let asset_server = world.get_resource::<AssetServer>()?;

    match asset_server.load_state(scene_root.id()) {
        bevy::asset::LoadState::Failed(error) => return Some(gltf_load_status_from_error(&error)),
        bevy::asset::LoadState::NotLoaded => return Some(XrdsGltfLoadStatus::NotLoaded),
        bevy::asset::LoadState::Loading => return Some(XrdsGltfLoadStatus::Loading),
        bevy::asset::LoadState::Loaded => {}
    }

    let scene_status = match asset_server.recursive_dependency_load_state(scene_root.id()) {
        bevy::asset::RecursiveDependencyLoadState::Failed(error) => {
            return Some(gltf_load_status_from_error(&error));
        }
        bevy::asset::RecursiveDependencyLoadState::Loaded => XrdsGltfLoadStatus::Loaded,
        bevy::asset::RecursiveDependencyLoadState::NotLoaded
        | bevy::asset::RecursiveDependencyLoadState::Loading => XrdsGltfLoadStatus::Loading,
    };

    Some(scene_status)
}

pub(super) fn gltf_load_status_in_world(
    world: &World,
    handle: &Handle<XrdsGltfAsset>,
) -> Option<XrdsGltfLoadStatus> {
    gltf_load_status_for_entity_in_world(world, handle.entity())
}

fn gltf_descriptor_for_entity_in_world(world: &World, entity: Entity) -> Option<&XrdsGltfAsset> {
    world
        .get::<XrdsStored<XrdsGltfAsset>>(entity)
        .map(|stored| &stored.0)
}

fn gltf_asset_handle_for_entity_in_world(
    world: &World,
    entity: Entity,
) -> Result<bevy::prelude::Handle<bevy::gltf::Gltf>, XrdsGltfRuntimeError> {
    let descriptor = gltf_descriptor_for_entity_in_world(world, entity)
        .ok_or(XrdsGltfRuntimeError::NotAGltfRuntimeEntity)?;
    // Use the handle that was stored at spawn time. Calling asset_server.load()
    // here and dropping the result was the root cause: without a persistent
    // strong handle the Gltf asset is never inserted into Assets<Gltf>.
    world
        .get::<XrdsStoredGltfHandle>(entity)
        .map(|stored| stored.0.clone())
        .ok_or_else(|| XrdsGltfRuntimeError::AssetNotLoaded(descriptor.gltf_asset_path.clone()))
}

fn gltf_asset_for_entity_in_world<'w>(
    world: &'w World,
    entity: Entity,
) -> Result<&'w bevy::gltf::Gltf, XrdsGltfRuntimeError> {
    let handle = gltf_asset_handle_for_entity_in_world(world, entity)?;
    let descriptor = gltf_descriptor_for_entity_in_world(world, entity)
        .ok_or(XrdsGltfRuntimeError::NotAGltfRuntimeEntity)?;
    world
        .get_resource::<Assets<bevy::gltf::Gltf>>()
        .and_then(|assets| assets.get(&handle))
        .ok_or_else(|| XrdsGltfRuntimeError::AssetNotLoaded(descriptor.gltf_asset_path.clone()))
}

fn gltf_animation_selector_label(selector: &XrdsGltfAnimationSelector) -> String {
    match selector {
        XrdsGltfAnimationSelector::Index(index) => format!("index {index}"),
        XrdsGltfAnimationSelector::Name(name) => format!("name '{name}'"),
    }
}

fn gltf_animation_info_from_asset(
    world: &World,
    gltf: &bevy::gltf::Gltf,
    index: usize,
    clip_handle: &bevy::prelude::Handle<bevy::animation::AnimationClip>,
) -> XrdsGltfAnimationInfo {
    let duration_secs = world
        .get_resource::<Assets<bevy::animation::AnimationClip>>()
        .and_then(|clips| clips.get(clip_handle))
        .map(|clip| clip.duration());

    let name = gltf
        .named_animations
        .iter()
        .find_map(|(candidate, named_handle)| {
            if named_handle == clip_handle {
                Some(candidate.to_string())
            } else {
                None
            }
        });

    XrdsGltfAnimationInfo {
        index,
        name,
        duration_secs,
    }
}

fn gltf_animation_info_without_metadata(
    world: &World,
    index: usize,
    clip_handle: &bevy::prelude::Handle<bevy::animation::AnimationClip>,
) -> XrdsGltfAnimationInfo {
    let duration_secs = world
        .get_resource::<Assets<bevy::animation::AnimationClip>>()
        .and_then(|clips| clips.get(clip_handle))
        .map(|clip| clip.duration());

    XrdsGltfAnimationInfo {
        index,
        name: None,
        duration_secs,
    }
}

fn resolve_gltf_animation_selection_for_entity_in_world(
    world: &World,
    entity: Entity,
    selector: &XrdsGltfAnimationSelector,
) -> Result<
    (
        usize,
        bevy::prelude::Handle<bevy::animation::AnimationClip>,
        XrdsGltfAnimationInfo,
    ),
    XrdsGltfRuntimeError,
> {
    if let XrdsGltfAnimationSelector::Index(index) = selector {
        let descriptor = gltf_descriptor_for_entity_in_world(world, entity)
            .ok_or(XrdsGltfRuntimeError::NotAGltfRuntimeEntity)?;
        let asset_server = world.get_resource::<AssetServer>().ok_or_else(|| {
            XrdsGltfRuntimeError::AssetNotLoaded(descriptor.gltf_asset_path.clone())
        })?;

        let clip_handle = asset_server.load::<bevy::animation::AnimationClip>(
            bevy::gltf::GltfAssetLabel::Animation(*index)
                .from_asset(super::gltf::relativize_asset_path(&descriptor.gltf_asset_path)),
        );

        if let Ok(gltf) = gltf_asset_for_entity_in_world(world, entity) {
            if let Some(gltf_clip_handle) = gltf.animations.get(*index).cloned() {
                // Prefer the handle that the GLTF loader already registered so that
                // the AnimationGraph references exactly the same AssetId that the
                // loader used when building AnimationTarget IDs on bone entities.
                let info = gltf_animation_info_from_asset(world, gltf, *index, &gltf_clip_handle);
                return Ok((*index, gltf_clip_handle, info));
            }

            return Err(XrdsGltfRuntimeError::AnimationNotFound(
                gltf_animation_selector_label(selector),
            ));
        }

        // GLTF parent asset not yet in Assets<Gltf> — fall back to a direct load
        // so the request can still be processed (will retry next frame if needed).
        let info = gltf_animation_info_without_metadata(world, *index, &clip_handle);
        return Ok((*index, clip_handle, info));
    }

    let gltf = gltf_asset_for_entity_in_world(world, entity)?;

    let selected = match selector {
        XrdsGltfAnimationSelector::Index(index) => gltf
            .animations
            .get(*index)
            .cloned()
            .map(|handle| (*index, handle)),
        XrdsGltfAnimationSelector::Name(name) => {
            gltf.named_animations
                .iter()
                .find_map(|(candidate, handle)| {
                    if candidate.as_ref() == name.as_str() {
                        gltf.animations
                            .iter()
                            .position(|animation| animation == handle)
                            .map(|index| (index, handle.clone()))
                    } else {
                        None
                    }
                })
        }
    };

    let (index, clip_handle) = selected.ok_or_else(|| {
        XrdsGltfRuntimeError::AnimationNotFound(gltf_animation_selector_label(selector))
    })?;
    let info = gltf_animation_info_from_asset(world, gltf, index, &clip_handle);
    Ok((index, clip_handle, info))
}

fn descendant_entities_with_paths_in_world(
    world: &World,
    root: Entity,
) -> Vec<(Entity, Vec<usize>)> {
    let mut descendants = Vec::new();
    let mut stack = Vec::new();

    if let Some(children) = world.get::<Children>(root) {
        for (child_index, child) in children.iter().enumerate().rev() {
            stack.push((child, vec![child_index]));
        }
    }

    while let Some((entity, path)) = stack.pop() {
        descendants.push((entity, path.clone()));

        if let Some(children) = world.get::<Children>(entity) {
            for (child_index, child) in children.iter().enumerate().rev() {
                let mut child_path = path.clone();
                child_path.push(child_index);
                stack.push((child, child_path));
            }
        }
    }

    descendants
}

pub(super) fn animation_player_entities_for_root_in_world(
    world: &World,
    root: Entity,
) -> Vec<Entity> {
    descendant_entities_with_paths_in_world(world, root)
        .into_iter()
        .filter_map(|(entity, _)| world.get::<AnimationPlayer>(entity).map(|_| entity))
        .collect()
}

pub(super) fn apply_gltf_animation_request_for_entity_in_world(
    world: &mut World,
    entity: Entity,
    request: &PendingGltfAnimationRequest,
) -> Result<bool, XrdsGltfRuntimeError> {
    let (clip_handle, animation_info) = match resolve_gltf_animation_selection_for_entity_in_world(
        world,
        entity,
        &request.selector,
    ) {
        Ok((_, clip_handle, info)) => (clip_handle, info),
        Err(XrdsGltfRuntimeError::AssetNotLoaded(_)) => return Ok(false),
        Err(error) => return Err(error),
    };

    let player_entities = animation_player_entities_for_root_in_world(world, entity);
    if player_entities.is_empty() {
        debug!("gltf animation: no AnimationPlayer found in descendants of {:?}, will retry", entity);
        return Ok(false);
    }

    let (graph, graph_index) = AnimationGraph::from_clip(clip_handle);
    let graph_handle = world.resource_mut::<Assets<AnimationGraph>>().add(graph);

    for player_entity in player_entities {
        // 1. Ensure visibility components on the player and its ancestors to fix B0004
        // which can cause the GlobalTransform to stop updating, making the drone look static.
        let mut cur = Some(player_entity);
        while let Some(curr_entity) = cur {
            let mut next_parent = None;
            if let Ok(mut e) = world.get_entity_mut(curr_entity) {
                if !e.contains::<Visibility>() { e.insert(Visibility::Visible); }
                if !e.contains::<InheritedVisibility>() { e.insert(InheritedVisibility::default()); }
                if !e.contains::<ViewVisibility>() { e.insert(ViewVisibility::default()); }
                if !e.contains::<GlobalTransform>() { e.insert(GlobalTransform::default()); }
                
                if let Some(co) = e.get::<ChildOf>() {
                    next_parent = Some(co.0);
                }
            }
            cur = next_parent;
        }

        // 2. Start playback BEFORE inserting the graph handle — matches the
        // gltf_samples_check pattern (player.play() → commands.insert(graph))
        // so that active_animations is populated when animate_targets evaluates.
        if let Some(mut player) = world.get_mut::<AnimationPlayer>(player_entity) {
            let active = player.play(graph_index);

            match request.options.repeat {
                XrdsAnimationRepeatMode::Loop => { active.repeat(); }
                XrdsAnimationRepeatMode::Once => {}
            }

            active.set_speed(request.options.speed);

            if request.options.start_paused {
                active.pause();
            }

            println!("XRDS Diagnostic: Started animation node {:?} on player {:?}", graph_index, player_entity);
        }

        // 3. Insert graph handle AFTER play() so the handle insertion cannot
        // clear or invalidate the active animation we just registered.
        world
            .entity_mut(player_entity)
            .insert(AnimationGraphHandle(graph_handle.clone()));

        // Diagnostic: count bone entities linked to this player via AnimationTarget.
        // Zero means no bones will animate — a sign of entity hierarchy mismatch.
        {
            let mut q = world.query::<&bevy::animation::AnimationTarget>();
            let target_count = q.iter(world).filter(|t| t.player == player_entity).count();
            if target_count == 0 {
                warn!(
                    "XRDS: AnimationPlayer {:?} has NO AnimationTarget descendants — \
                     bone transforms will not be applied, animation will appear static",
                    player_entity
                );
            } else {
                println!(
                    "XRDS: {} bone target(s) linked to player {:?}",
                    target_count, player_entity
                );
            }
        }
    }

    world
        .resource_mut::<ActiveGltfAnimationStates>()
        .states
        .insert(
            entity,
            XrdsGltfAnimationState {
                animation: animation_info,
                playing: !request.options.start_paused,
                paused: request.options.start_paused,
                repeat: request.options.repeat,
                speed: request.options.speed,
            },
        );

    Ok(true)
}

pub(super) fn apply_pending_gltf_animation_requests_system(world: &mut World) {
    let pending_requests = world
        .get_resource::<PendingGltfAnimationRequests>()
        .map(|resource| {
            resource
                .requests
                .iter()
                .map(|(entity, request)| (*entity, request.clone()))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let mut resolved_entities = Vec::new();

    for (entity, request) in pending_requests {
        match apply_gltf_animation_request_for_entity_in_world(world, entity, &request) {
            Ok(true) => resolved_entities.push(entity),
            Ok(false) => {}
            Err(error) => {
                warn!(
                    "Removing pending glTF animation request for {:?}: {error:?}",
                    entity
                );
                resolved_entities.push(entity);
            }
        }
    }

    if !resolved_entities.is_empty() {
        let mut pending = world.resource_mut::<PendingGltfAnimationRequests>();
        for entity in resolved_entities {
            pending.requests.remove(&entity);
        }
    }
}

pub(super) fn apply_pending_gltf_animation_requests_on_scene_ready(
    scene_ready: On<bevy::scene::SceneInstanceReady>,
    mut pending: ResMut<PendingGltfAnimationRequests>,
    mut commands: Commands,
) {
    let entity = scene_ready.entity;
    // [CRASH-ZONE 4] SceneInstanceReady fires after Bevy's scene spawner creates
    // all skeleton/mesh child entities. Bone children may arrive without a parent
    // that has InheritedVisibility (B0004), which can crash the wgpu encoder.
    let Some(request) = pending.requests.remove(&entity) else {
        return;
    };
    commands.queue(move |world: &mut World| {
        match apply_gltf_animation_request_for_entity_in_world(world, entity, &request) {
            Ok(true) => {
            }
            Ok(false) => {
                world
                    .resource_mut::<PendingGltfAnimationRequests>()
                    .requests
                    .insert(entity, request.clone());
            }
            Err(error) => {
                warn!(
                    "Removing pending glTF animation request for {:?} after scene became ready: {error:?}",
                    entity
                );
            }
        }
    });
}

#[derive(Debug, Clone)]
struct ResolvedGltfMorphMesh {
    entity: Entity,
    morph_weights_owner: Option<Entity>,
    node: XrdsGltfNodeLocator,
    mesh_name: Option<String>,
    target_names: Vec<String>,
    weights: Vec<f32>,
}

fn nearest_morph_weights_owner_in_world(world: &World, mut entity: Entity) -> Option<Entity> {
    loop {
        if world
            .get::<bevy::mesh::morph::MorphWeights>(entity)
            .is_some()
        {
            return Some(entity);
        }

        let Some(parent) = world.get::<ChildOf>(entity).map(|child_of| child_of.0) else {
            return None;
        };
        entity = parent;
    }
}

fn gltf_morph_target_selector_label(selector: &XrdsGltfMorphTargetSelector) -> String {
    match selector {
        XrdsGltfMorphTargetSelector::Index(index) => format!("index {index}"),
        XrdsGltfMorphTargetSelector::Name(name) => format!("name '{name}'"),
    }
}

fn gltf_morph_mesh_label(node: &XrdsGltfNodeLocator, mesh_name: Option<&str>) -> String {
    let path = if node.node_index_path.is_empty() {
        "<root>".to_string()
    } else {
        node.node_index_path
            .iter()
            .map(|index| index.to_string())
            .collect::<Vec<_>>()
            .join("/")
    };

    match (node.node_name.as_deref(), mesh_name) {
        (Some(node_name), Some(mesh_name)) => {
            format!("node '{node_name}' at path {path}, mesh '{mesh_name}'")
        }
        (Some(node_name), None) => format!("node '{node_name}' at path {path}"),
        (None, Some(mesh_name)) => format!("path {path}, mesh '{mesh_name}'"),
        (None, None) => format!("path {path}"),
    }
}

fn gltf_morph_mesh_matches_request(
    mesh: &ResolvedGltfMorphMesh,
    node: &XrdsGltfNodeLocator,
    mesh_name: Option<&str>,
) -> bool {
    if !node.node_index_path.is_empty() && mesh.node.node_index_path != node.node_index_path {
        return false;
    }

    if let Some(node_name) = node.node_name.as_deref() {
        if mesh.node.node_name.as_deref() != Some(node_name) {
            return false;
        }
    }

    if let Some(mesh_name) = mesh_name {
        if mesh.mesh_name.as_deref() != Some(mesh_name) {
            return false;
        }
    }

    true
}

fn resolve_runtime_morph_target_index(
    names: &[String],
    selector: &XrdsGltfMorphTargetSelector,
) -> Result<usize, XrdsGltfRuntimeError> {
    match selector {
        XrdsGltfMorphTargetSelector::Index(index) => {
            names.get(*index).map(|_| *index).ok_or_else(|| {
                XrdsGltfRuntimeError::MorphTargetNotFound(gltf_morph_target_selector_label(
                    selector,
                ))
            })
        }
        XrdsGltfMorphTargetSelector::Name(name) => names
            .iter()
            .position(|candidate| candidate == name)
            .ok_or_else(|| {
                XrdsGltfRuntimeError::MorphTargetNotFound(gltf_morph_target_selector_label(
                    selector,
                ))
            }),
    }
}

fn resolved_gltf_morph_meshes_for_entity_in_world(
    world: &World,
    entity: Entity,
) -> Result<Vec<ResolvedGltfMorphMesh>, XrdsGltfRuntimeError> {
    if gltf_descriptor_for_entity_in_world(world, entity).is_none() {
        return Err(XrdsGltfRuntimeError::NotAGltfRuntimeEntity);
    }

    let asset_server = world
        .get_resource::<AssetServer>()
        .ok_or(XrdsGltfRuntimeError::NotAGltfRuntimeEntity)?;
    let meshes = world
        .get_resource::<Assets<Mesh>>()
        .ok_or(XrdsGltfRuntimeError::NotAGltfRuntimeEntity)?;

    let mut result = Vec::new();

    for (mesh_entity, node_index_path) in descendant_entities_with_paths_in_world(world, entity) {
        let Some(mesh_handle) = world.get::<Mesh3d>(mesh_entity).map(|mesh| mesh.0.clone()) else {
            continue;
        };

        let Some(mesh) = meshes.get(&mesh_handle) else {
            continue;
        };

        let Some(names) = mesh.morph_target_names() else {
            continue;
        };

        let morph_weights_owner = nearest_morph_weights_owner_in_world(world, mesh_entity);
        let mut weights = morph_weights_owner
            .and_then(|owner| {
                world
                    .get::<bevy::mesh::morph::MorphWeights>(owner)
                    .map(|existing| existing.weights().to_vec())
            })
            .or_else(|| {
                world
                    .get::<bevy::mesh::morph::MeshMorphWeights>(mesh_entity)
                    .map(|existing| existing.weights().to_vec())
            })
            .unwrap_or_else(|| vec![0.0; names.len()]);
        if weights.len() != names.len() {
            weights.resize(names.len(), 0.0);
        }

        result.push(ResolvedGltfMorphMesh {
            entity: mesh_entity,
            morph_weights_owner,
            node: XrdsGltfNodeLocator {
                node_index_path,
                node_name: world
                    .get::<Name>(mesh_entity)
                    .map(|name| name.as_str().to_string()),
            },
            mesh_name: asset_server
                .get_path(mesh_handle.id())
                .and_then(|path| path.label().map(ToString::to_string)),
            target_names: names.to_vec(),
            weights,
        });
    }

    if result.is_empty() {
        let descriptor = gltf_descriptor_for_entity_in_world(world, entity)
            .ok_or(XrdsGltfRuntimeError::NotAGltfRuntimeEntity)?;
        return Err(XrdsGltfRuntimeError::AssetNotLoaded(
            descriptor.gltf_asset_path.clone(),
        ));
    }

    Ok(result)
}

fn export_gltf_morph_target_overrides_for_entity_in_world(
    world: &World,
    entity: Entity,
) -> Result<Option<Vec<xrds_scene_graph::XrdsSceneGltfMorphTargetOverride>>, XrdsGltfRuntimeError> {
    let meshes = match resolved_gltf_morph_meshes_for_entity_in_world(world, entity) {
        Ok(meshes) => meshes,
        Err(XrdsGltfRuntimeError::AssetNotLoaded(_)) => return Ok(None),
        Err(error) => return Err(error),
    };

    Ok(Some(
        meshes
            .into_iter()
            .map(|mesh| xrds_scene_graph::XrdsSceneGltfMorphTargetOverride {
                node: xrds_scene_graph::XrdsSceneGltfNodeLocator {
                    node_index_path: mesh.node.node_index_path,
                    node_name: mesh.node.node_name,
                },
                mesh_name: mesh.mesh_name,
                weights: mesh
                    .target_names
                    .into_iter()
                    .zip(mesh.weights)
                    .enumerate()
                    .map(|(index, (name, weight))| {
                        xrds_scene_graph::XrdsSceneGltfMorphTargetWeight {
                            selector: if name.trim().is_empty() {
                                xrds_scene_graph::XrdsSceneGltfMorphTargetSelector::Index(index)
                            } else {
                                xrds_scene_graph::XrdsSceneGltfMorphTargetSelector::Name(name)
                            },
                            weight,
                        }
                    })
                    .collect(),
            })
            .collect(),
    ))
}

fn set_gltf_morph_target_weight_for_entity_in_world(
    world: &mut World,
    entity: Entity,
    node: &XrdsGltfNodeLocator,
    mesh_name: Option<&str>,
    selector: &XrdsGltfMorphTargetSelector,
    weight: f32,
) -> Result<(), XrdsGltfRuntimeError> {
    if !weight.is_finite() {
        return Err(XrdsGltfRuntimeError::InvalidMorphTargetWeight);
    }

    let meshes = resolved_gltf_morph_meshes_for_entity_in_world(world, entity)?;
    let mut matched = false;
    let mut updates = Vec::new();

    for mesh in meshes {
        if !gltf_morph_mesh_matches_request(&mesh, node, mesh_name) {
            continue;
        }

        matched = true;
        let index = resolve_runtime_morph_target_index(&mesh.target_names, selector)?;
        let mut weights = mesh.weights;
        weights[index] = weight;
        updates.push((mesh.entity, mesh.morph_weights_owner, weights));
    }

    if !matched {
        return Err(XrdsGltfRuntimeError::MorphTargetMeshNotFound(
            gltf_morph_mesh_label(node, mesh_name),
        ));
    }

    for (mesh_entity, morph_weights_owner, weights) in updates {
        if let Some(owner) = morph_weights_owner {
            if let Some(mut canonical) = world.get_mut::<bevy::mesh::morph::MorphWeights>(owner) {
                let canonical_weights = canonical.weights_mut();
                if canonical_weights.len() != weights.len() {
                    canonical_weights.fill(0.0);
                }
                let len = canonical_weights.len().min(weights.len());
                canonical_weights[..len].copy_from_slice(&weights[..len]);
            }
        }

        let component = bevy::mesh::morph::MeshMorphWeights::new(weights)
            .expect("runtime morph target weights should already satisfy Bevy limits");
        world.entity_mut(mesh_entity).insert(component);
    }

    Ok(())
}

pub(super) fn apply_gltf_morph_target_overrides_for_entity_in_world(
    world: &mut World,
    entity: Entity,
) -> Result<bool, String> {
    match gltf_asset_for_entity_in_world(world, entity) {
        Ok(_) => {}
        Err(XrdsGltfRuntimeError::AssetNotLoaded(_)) => return Ok(false),
        Err(other) => return Err(format!("{other:?}")),
    }

    let Some(authoring) = world
        .get::<XrdsStoredSceneGltfNodeAuthoring>(entity)
        .map(|stored| stored.0.clone())
    else {
        return Ok(true);
    };

    if authoring.morph_target_overrides.is_empty() {
        return Ok(true);
    }

    let meshes = resolved_gltf_morph_meshes_for_entity_in_world(world, entity).map_err(
        |error| match error {
            XrdsGltfRuntimeError::AssetNotLoaded(_) => "asset is not ready".to_string(),
            other => format!("{other:?}"),
        },
    )?;
    if meshes.is_empty() {
        return Ok(false);
    }

    let mut matched_override = false;
    for override_entry in &authoring.morph_target_overrides {
        let mesh_name = override_entry.mesh_name.as_deref();
        let node = XrdsGltfNodeLocator {
            node_index_path: override_entry.node.node_index_path.clone(),
            node_name: override_entry.node.node_name.clone(),
        };

        if meshes
            .iter()
            .any(|mesh| gltf_morph_mesh_matches_request(mesh, &node, mesh_name))
        {
            matched_override = true;
        }

        for authored_weight in &override_entry.weights {
            set_gltf_morph_target_weight_for_entity_in_world(
                world,
                entity,
                &node,
                mesh_name,
                &authored_weight.selector.clone().into(),
                authored_weight.weight,
            )
            .map_err(|error| format!("{error:?}"))?;
        }
    }

    if !matched_override {
        return Err(
            "no realized morph-target mesh matched the authored override selectors".to_string(),
        );
    }

    Ok(true)
}

pub(super) fn apply_pending_gltf_morph_target_override_requests_system(world: &mut World) {
    let pending_entities = world
        .get_resource::<PendingGltfMorphTargetOverrideRequests>()
        .map(|resource| resource.entities.iter().copied().collect::<Vec<_>>())
        .unwrap_or_default();

    let mut resolved_entities = Vec::new();

    for entity in pending_entities {
        match apply_gltf_morph_target_overrides_for_entity_in_world(world, entity) {
            Ok(true) => resolved_entities.push(entity),
            Ok(false) => {}
            Err(error) => {
                warn!(
                    "Removing pending glTF morph-target override request for {:?}: {}",
                    entity, error
                );
                resolved_entities.push(entity);
            }
        }
    }

    if !resolved_entities.is_empty() {
        let mut pending = world.resource_mut::<PendingGltfMorphTargetOverrideRequests>();
        for entity in resolved_entities {
            pending.entities.remove(&entity);
        }
    }
}

pub(super) fn gltf_animations_in_world(
    world: &World,
    handle: &Handle<XrdsGltfAsset>,
) -> Result<Vec<XrdsGltfAnimationInfo>, XrdsGltfRuntimeError> {
    let gltf = gltf_asset_for_entity_in_world(world, handle.entity())?;
    Ok(gltf
        .animations
        .iter()
        .enumerate()
        .map(|(index, clip_handle)| gltf_animation_info_from_asset(world, gltf, index, clip_handle))
        .collect())
}

pub(super) fn play_gltf_animation_in_world(
    world: &mut World,
    handle: &Handle<XrdsGltfAsset>,
    selector: XrdsGltfAnimationSelector,
    options: XrdsGltfAnimationPlaybackOptions,
) -> Result<(), XrdsGltfRuntimeError> {
    if gltf_descriptor_for_entity_in_world(world, handle.entity()).is_none() {
        return Err(XrdsGltfRuntimeError::NotAGltfRuntimeEntity);
    }

    let request = PendingGltfAnimationRequest { selector, options };

    match apply_gltf_animation_request_for_entity_in_world(world, handle.entity(), &request) {
        Ok(true) => {
            world
                .resource_mut::<PendingGltfAnimationRequests>()
                .requests
                .remove(&handle.entity());
            Ok(())
        }
        Ok(false) | Err(XrdsGltfRuntimeError::AssetNotLoaded(_)) => {
            world
                .resource_mut::<PendingGltfAnimationRequests>()
                .requests
                .insert(handle.entity(), request);
            Ok(())
        }
        Err(error) => Err(error),
    }
}

pub(super) fn stop_gltf_animation_in_world(
    world: &mut World,
    handle: &Handle<XrdsGltfAsset>,
) -> Result<(), XrdsGltfRuntimeError> {
    if gltf_descriptor_for_entity_in_world(world, handle.entity()).is_none() {
        return Err(XrdsGltfRuntimeError::NotAGltfRuntimeEntity);
    }

    for player_entity in animation_player_entities_for_root_in_world(world, handle.entity()) {
        if let Some(mut player) = world.get_mut::<AnimationPlayer>(player_entity) {
            player.stop_all();
        }
    }

    world
        .resource_mut::<PendingGltfAnimationRequests>()
        .requests
        .remove(&handle.entity());
    world
        .resource_mut::<ActiveGltfAnimationStates>()
        .states
        .remove(&handle.entity());
    Ok(())
}

pub(super) fn pause_gltf_animation_in_world(
    world: &mut World,
    handle: &Handle<XrdsGltfAsset>,
) -> Result<(), XrdsGltfRuntimeError> {
    if gltf_descriptor_for_entity_in_world(world, handle.entity()).is_none() {
        return Err(XrdsGltfRuntimeError::NotAGltfRuntimeEntity);
    }

    for player_entity in animation_player_entities_for_root_in_world(world, handle.entity()) {
        if let Some(mut player) = world.get_mut::<AnimationPlayer>(player_entity) {
            player.pause_all();
        }
    }

    if let Some(request) = world
        .resource_mut::<PendingGltfAnimationRequests>()
        .requests
        .get_mut(&handle.entity())
    {
        request.options.start_paused = true;
    }

    if let Some(state) = world
        .resource_mut::<ActiveGltfAnimationStates>()
        .states
        .get_mut(&handle.entity())
    {
        state.playing = false;
        state.paused = true;
    }

    Ok(())
}

pub(super) fn resume_gltf_animation_in_world(
    world: &mut World,
    handle: &Handle<XrdsGltfAsset>,
) -> Result<(), XrdsGltfRuntimeError> {
    if gltf_descriptor_for_entity_in_world(world, handle.entity()).is_none() {
        return Err(XrdsGltfRuntimeError::NotAGltfRuntimeEntity);
    }

    for player_entity in animation_player_entities_for_root_in_world(world, handle.entity()) {
        if let Some(mut player) = world.get_mut::<AnimationPlayer>(player_entity) {
            player.resume_all();
        }
    }

    if let Some(request) = world
        .resource_mut::<PendingGltfAnimationRequests>()
        .requests
        .get_mut(&handle.entity())
    {
        request.options.start_paused = false;
    }

    if let Some(state) = world
        .resource_mut::<ActiveGltfAnimationStates>()
        .states
        .get_mut(&handle.entity())
    {
        state.playing = true;
        state.paused = false;
    }

    Ok(())
}

pub(super) fn gltf_animation_state_in_world(
    world: &World,
    handle: &Handle<XrdsGltfAsset>,
) -> Result<Option<XrdsGltfAnimationState>, XrdsGltfRuntimeError> {
    if gltf_descriptor_for_entity_in_world(world, handle.entity()).is_none() {
        return Err(XrdsGltfRuntimeError::NotAGltfRuntimeEntity);
    }

    Ok(world
        .get_resource::<ActiveGltfAnimationStates>()
        .and_then(|resource| resource.states.get(&handle.entity()).cloned()))
}

pub(super) fn gltf_morph_targets_in_world(
    world: &World,
    handle: &Handle<XrdsGltfAsset>,
) -> Result<Vec<XrdsGltfMorphTargetSet>, XrdsGltfRuntimeError> {
    Ok(
        resolved_gltf_morph_meshes_for_entity_in_world(world, handle.entity())?
            .into_iter()
            .map(|mesh| XrdsGltfMorphTargetSet {
                node: mesh.node,
                mesh_name: mesh.mesh_name,
                targets: mesh
                    .target_names
                    .into_iter()
                    .enumerate()
                    .map(|(index, name)| XrdsGltfMorphTargetInfo {
                        index,
                        name: Some(name),
                    })
                    .collect(),
            })
            .collect(),
    )
}

pub(super) fn gltf_morph_target_weights_in_world(
    world: &World,
    handle: &Handle<XrdsGltfAsset>,
) -> Result<Vec<XrdsGltfMorphTargetWeights>, XrdsGltfRuntimeError> {
    Ok(
        resolved_gltf_morph_meshes_for_entity_in_world(world, handle.entity())?
            .into_iter()
            .map(|mesh| XrdsGltfMorphTargetWeights {
                node: mesh.node,
                mesh_name: mesh.mesh_name,
                weights: mesh
                    .target_names
                    .into_iter()
                    .zip(mesh.weights)
                    .enumerate()
                    .map(|(index, (name, weight))| XrdsGltfMorphTargetWeightValue {
                        target: XrdsGltfMorphTargetInfo {
                            index,
                            name: Some(name),
                        },
                        weight,
                    })
                    .collect(),
            })
            .collect(),
    )
}

pub(super) fn set_gltf_morph_target_weight_in_world(
    world: &mut World,
    handle: &Handle<XrdsGltfAsset>,
    node: &XrdsGltfNodeLocator,
    mesh_name: Option<&str>,
    selector: XrdsGltfMorphTargetSelector,
    weight: f32,
) -> Result<(), XrdsGltfRuntimeError> {
    set_gltf_morph_target_weight_for_entity_in_world(
        world,
        handle.entity(),
        node,
        mesh_name,
        &selector,
        weight,
    )
}

pub(super) fn set_material_emissive_in_world<C>(
    world: &mut World,
    handle: &Handle<C>,
    emissive: XrdsLinearRgba,
) {
    let mut params = material_params_in_world(world, handle).unwrap_or_default();
    params.emissive = emissive;
    set_material_params_in_world(world, handle, params);
}

/// Pick a random world-space position within a randomly chosen [`XrdsPlayerSpawnZone`].
/// Y comes from the zone entity's world-space Y centre; X and Z are randomised within the zone.
/// Returns `None` if no matching spawn zones are present.
///
/// `player_node_id`:
/// - `None`      → all zones are eligible (no ownership filter).
/// - `Some(id)`  → only zones owned by that player (`zone.player_node_id == Some(id)`) plus
///                 shared zones (`zone.player_node_id == None`) are eligible.
pub(super) fn random_spawn_zone_position_in_world(
    world: &World,
    player_node_id: Option<u64>,
) -> Option<Vec3> {
    let zones: Vec<(Vec3, Vec3)> = world.iter_entities()
        .filter_map(|er| {
            let gt   = er.get::<GlobalTransform>()?;
            let zone = er.get::<xrds_components::XrdsPlayerSpawnZone>()?;
            // Apply ownership filter when a player ID is requested.
            if let Some(pid) = player_node_id {
                if zone.player_node_id.is_some() && zone.player_node_id != Some(pid) {
                    return None;
                }
            }
            Some((gt.translation(), zone.size))
        })
        .collect();
    if zones.is_empty() { return None; }

    // Simple LCG seeded from subsecond time for variety across calls.
    let seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(42);
    let mut s = seed;
    let mut next_f = move || -> f32 {
        s = s.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        (s >> 8) as f32 / 16_777_216.0   // [0, 1)
    };

    let (center, size) = zones[(next_f() * zones.len() as f32) as usize % zones.len()];
    Some(Vec3::new(
        center.x + (next_f() - 0.5) * size.x,
        center.y,
        center.z + (next_f() - 0.5) * size.z,
    ))
}

/// Teleport the entity tagged [`XrdsPlayerRoot`] to `position`.
pub(super) fn teleport_player_in_world(world: &mut World, position: Vec3) {
    let mut q = world.query_filtered::<&mut Transform, With<XrdsPlayerRoot>>();
    for mut tf in q.iter_mut(world) {
        tf.translation = position;
    }
}

/// Syncs XRDS's cached `XrdsGltfAnimationState.playing` from the live
/// `AnimationPlayer`, and reports nodes whose playback just completed.
///
/// Fixes a real bug in the cached state: every other writer of
/// `ActiveGltfAnimationStates` is an imperative API call (play/stop/pause/
/// resume), so nothing ever cleared `playing` when a clip reached its
/// natural end — `gltf_animation_state()` would report `playing: true`
/// forever after a `Once` clip finished. This is the only writer driven by
/// what the engine is actually doing.
///
/// Returns the roots that completed on this call, so the caller can turn
/// them into `XrdsTriggerKind::AnimationComplete` trigger events.
pub(super) fn sync_completed_gltf_animations_in_world(world: &mut World) -> Vec<Entity> {
    // Only XRDS-tracked playback that we still believe is running. Paused
    // playback is skipped: it isn't finished, it's suspended.
    let tracked: Vec<Entity> = world
        .resource::<ActiveGltfAnimationStates>()
        .states
        .iter()
        .filter(|(_, state)| state.playing && !state.paused)
        .map(|(entity, _)| *entity)
        .collect();

    let mut completed = Vec::new();
    for root in tracked {
        // `AnimationPlayer::all_finished()` is deliberately NOT used here:
        // it's `.all()` over the active set, and `.all()` on an EMPTY set
        // is `true` — so an asset that is still loading (playback queued,
        // player not yet populated) would report "finished" immediately.
        // Require at least one active animation as well.
        let mut any_active = false;
        let mut all_finished = true;
        for player_entity in animation_player_entities_for_root_in_world(world, root) {
            if let Some(player) = world.get::<AnimationPlayer>(player_entity) {
                for (_, active) in player.playing_animations() {
                    any_active = true;
                    // `is_finished()` is always false for RepeatAnimation::Forever,
                    // which is what XRDS's `Loop` maps to — so looping playback
                    // correctly never completes.
                    if !active.is_finished() {
                        all_finished = false;
                    }
                }
            }
        }

        if any_active && all_finished {
            completed.push(root);
        }
    }

    // Flip the cached flag so `gltf_animation_state()` starts telling the
    // truth, and so this only reports each completion once.
    for root in &completed {
        if let Some(state) = world
            .resource_mut::<ActiveGltfAnimationStates>()
            .states
            .get_mut(root)
        {
            state.playing = false;
        }
    }

    completed
}
