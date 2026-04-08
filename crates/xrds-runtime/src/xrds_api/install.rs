use super::*;
use bevy::asset::RenderAssetUsages;
use bevy::core_pipeline::core_3d::graph::Core3d;
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::mesh::Indices;
use bevy::post_process::bloom::Bloom;
use bevy::render::camera::CameraRenderGraph;
use bevy::render::render_resource::PrimitiveTopology;
use std::path::{Path, PathBuf};

pub(super) fn install_xrds(app: &mut App) {
    if app.world().contains_resource::<XrdsInstalled>() {
        return;
    }

    app.add_plugins(XrdsComponentsPlugin);
    app.init_resource::<XrdsIdAllocator>();
    app.init_resource::<XrdsIdIndex>();
    app.init_resource::<XrdsHierarchyIndex>();
    app.init_resource::<SurfaceInterpreterRegistry>();
    app.init_resource::<SurfaceDescriptorRegistry>();
    app.init_resource::<QueuedSurfaceComponents>();
    app.init_resource::<QueuedParentChanges>();
    app.init_resource::<SurfaceUpdateRegistry>();
    app.init_resource::<QueuedSurfaceUpdates>();
    app.init_resource::<PendingGltfAnimationRequests>();
    app.init_resource::<ActiveGltfAnimationStates>();
    app.init_resource::<PendingGltfMorphTargetOverrideRequests>();

    {
        let mut registry = app.world_mut().resource_mut::<SurfaceInterpreterRegistry>();
        register_default_interpreters(&mut registry);
    }
    {
        let mut registry = app.world_mut().resource_mut::<SurfaceUpdateRegistry>();
        register_default_updaters(&mut registry);
    }
    {
        let mut registry = app.world_mut().resource_mut::<SurfaceDescriptorRegistry>();
        register_default_descriptor_cloners(&mut registry);
    }

    app.add_systems(Startup, spawn_surface_components_from_queue);
    app.add_systems(First, ensure_visibility_hierarchy_components_system);
    app.add_systems(
        Startup,
        apply_queued_parent_changes_system.after(spawn_surface_components_from_queue),
    );
    app.add_systems(Update, apply_surface_updates);
    app.add_systems(
        Update,
        apply_queued_parent_changes_system.after(apply_surface_updates),
    );
    app.add_systems(
        Update,
        apply_pending_gltf_animation_requests_system.after(apply_queued_parent_changes_system),
    );
    app.add_systems(
        Update,
        apply_pending_gltf_morph_target_override_requests_system
            .after(apply_pending_gltf_animation_requests_system),
    );
    app.add_observer(apply_pending_gltf_animation_requests_on_scene_ready);
    app.init_resource::<XrdsInstalled>();
}

fn ensure_visibility_hierarchy_components_system(world: &mut World) {
    let mut query = world.query::<&ChildOf>();
    let parents: Vec<Entity> = query.iter(world).map(|child_of| child_of.0).collect();

    for parent in parents {
        let mut entity = world.entity_mut(parent);
        if !entity.contains::<Visibility>() {
            entity.insert(Visibility::Visible);
        }
        if !entity.contains::<InheritedVisibility>() {
            entity.insert(InheritedVisibility::default());
        }
        if !entity.contains::<ViewVisibility>() {
            entity.insert(ViewVisibility::default());
        }
        if !entity.contains::<GlobalTransform>() {
            entity.insert(GlobalTransform::default());
        }
    }
}

pub(super) fn build_transform(t: &TransformParams) -> Transform {
    Transform {
        translation: Vec3::from_array(t.translation),
        rotation: Quat::from_xyzw(
            t.rotation_quat_xyzw[0],
            t.rotation_quat_xyzw[1],
            t.rotation_quat_xyzw[2],
            t.rotation_quat_xyzw[3],
        ),
        scale: Vec3::from_array(t.scale),
    }
}

pub(super) fn build_visibility(visible: bool) -> Visibility {
    if visible {
        Visibility::Visible
    } else {
        Visibility::Hidden
    }
}

pub(super) fn build_visibility_hierarchy_components(
    visible: bool,
) -> (Visibility, InheritedVisibility, ViewVisibility) {
    (
        build_visibility(visible),
        InheritedVisibility::default(),
        ViewVisibility::default(),
    )
}

fn resolve_gltf_document_path(path: &str) -> Option<PathBuf> {
    let document_path = path.split('#').next().unwrap_or(path);
    let document_path = Path::new(document_path);

    let candidates = if document_path.is_absolute() {
        vec![document_path.to_path_buf()]
    } else {
        vec![
            document_path.to_path_buf(),
            Path::new("assets").join(document_path),
        ]
    };

    candidates.into_iter().find(|candidate| candidate.is_file())
}

pub(super) fn validate_gltf_source(path: &str, scene_index: usize) -> Result<(), String> {
    if path.trim().is_empty() {
        return Err("glTF asset path is empty".to_string());
    }

    let Some(document_path) = resolve_gltf_document_path(path) else {
        return Err(format!("glTF asset '{path}' was not found"));
    };

    let Some(extension) = document_path.extension().and_then(|ext| ext.to_str()) else {
        return Err(format!("glTF asset '{path}' has no file extension"));
    };

    if !matches!(extension.to_ascii_lowercase().as_str(), "gltf" | "glb") {
        return Err(format!("glTF asset '{path}' must end in .gltf or .glb"));
    }

    let gltf = gltf::Gltf::open(&document_path)
        .map_err(|error| format!("failed to parse glTF asset '{path}': {error}"))?;

    let scene_count = gltf.scenes().count();
    if scene_count == 0 {
        return Err(format!("glTF asset '{path}' contains no scenes"));
    }

    if scene_index >= scene_count {
        return Err(format!(
            "glTF asset '{path}' does not contain scene index {scene_index} (available: 0..{})",
            scene_count - 1
        ));
    }

    Ok(())
}

fn sync_surface_common_state(
    world: &mut World,
    entity: Entity,
    name: String,
    transform: TransformParams,
    visible: bool,
) {
    world.entity_mut(entity).insert((
        Name::new(name),
        build_transform(&transform),
        GlobalTransform::default(),
        build_visibility_hierarchy_components(visible),
    ));
}

fn apply_mesh_to_entity(world: &mut World, entity: Entity, mesh: Mesh) {
    let existing_handle = world.get::<Mesh3d>(entity).map(|handle| handle.0.clone());

    match existing_handle {
        Some(handle) => {
            let mut replacement = None;
            if let Some(mut meshes) = world.get_resource_mut::<Assets<Mesh>>() {
                if let Some(existing) = meshes.get_mut(&handle) {
                    *existing = mesh;
                } else {
                    replacement = Some(meshes.add(mesh));
                }
            }

            if let Some(new_handle) = replacement {
                world.entity_mut(entity).insert(Mesh3d(new_handle));
            }
        }
        None => {
            if let Some(mut meshes) = world.get_resource_mut::<Assets<Mesh>>() {
                let handle = meshes.add(mesh);
                world.entity_mut(entity).insert(Mesh3d(handle));
            }
        }
    }
}

fn standard_material_from_authored(params: XrdsMaterialParams) -> StandardMaterial {
    let mut base_color = params.base_color;
    base_color.rgba[3] *= params.opacity.clamp(0.0, 1.0);
    let alpha = base_color.rgba[3];
    let alpha_mode = match params.pbr.alpha_mode {
        XrdsMaterialAlphaMode::Auto => {
            if alpha < 1.0 {
                AlphaMode::Blend
            } else {
                AlphaMode::Opaque
            }
        }
        XrdsMaterialAlphaMode::Opaque => AlphaMode::Opaque,
        XrdsMaterialAlphaMode::Mask => AlphaMode::Mask(params.pbr.alpha_cutoff.clamp(0.0, 1.0)),
        XrdsMaterialAlphaMode::Blend => AlphaMode::Blend,
    };

    StandardMaterial {
        base_color: base_color.into(),
        emissive: params.emissive.into(),
        perceptual_roughness: params.pbr.perceptual_roughness.clamp(0.0, 1.0),
        metallic: params.pbr.metallic.clamp(0.0, 1.0),
        reflectance: params.pbr.reflectance.clamp(0.0, 1.0),
        double_sided: params.pbr.double_sided,
        alpha_mode,
        unlit: params.unlit,
        ..default()
    }
}

pub(super) fn apply_authored_material_to_entity(
    world: &mut World,
    entity: Entity,
    params: XrdsMaterialParams,
) {
    let existing_handle = world
        .get::<MeshMaterial3d<StandardMaterial>>(entity)
        .map(|handle| handle.0.clone());

    let material_value = standard_material_from_authored(params);

    match existing_handle {
        Some(handle) => {
            let mut replacement = None;
            if let Some(mut materials) = world.get_resource_mut::<Assets<StandardMaterial>>() {
                if let Some(material) = materials.get_mut(&handle) {
                    *material = material_value.clone();
                } else {
                    replacement = Some(materials.add(material_value.clone()));
                }
            }

            if let Some(new_handle) = replacement {
                world.entity_mut(entity).insert(MeshMaterial3d(new_handle));
            }
        }
        None => {
            if let Some(mut materials) = world.get_resource_mut::<Assets<StandardMaterial>>() {
                let handle = materials.add(material_value);
                world.entity_mut(entity).insert(MeshMaterial3d(handle));
            }
        }
    }

    world.entity_mut(entity).insert(XrdsStoredMaterial(params));
}

pub(super) fn material_params_for_entity(
    world: &World,
    entity: Entity,
) -> Option<XrdsMaterialParams> {
    world
        .get::<XrdsStoredMaterial>(entity)
        .map(|material| material.0)
}

pub(super) fn apply_spawn_recipe_to_entity(
    world: &mut World,
    entity: Entity,
    recipe: XrdsGeometrySource,
    name: String,
    transform: TransformParams,
    visible: bool,
) {
    sync_surface_common_state(world, entity, name, transform, visible);

    match recipe {
        XrdsGeometrySource::GltfScene { path, scene_index } => {
            if let Err(error) = validate_gltf_source(&path, scene_index) {
                warn!(
                    "Ignoring invalid glTF recipe update for entity {:?}: {error}",
                    entity
                );
                return;
            }

            let scene_handle = {
                let server = world.resource::<AssetServer>();
                let path = build_scene_asset_path(&path, scene_index);
                server.load::<Scene>(path)
            };

            world.entity_mut(entity).remove::<Mesh3d>();
            world
                .entity_mut(entity)
                .remove::<MeshMaterial3d<StandardMaterial>>();
            world.entity_mut(entity).insert((
                SceneRoot(scene_handle),
                GlobalTransform::default(),
                build_visibility_hierarchy_components(true),
            ));
        }
        XrdsGeometrySource::PbrSphere { radius, material } => {
            world.entity_mut(entity).remove::<SceneRoot>();
            apply_mesh_to_entity(world, entity, Mesh::from(Sphere { radius }));
            apply_authored_material_to_entity(world, entity, material);
        }
        XrdsGeometrySource::PbrCuboid {
            half_extents,
            material,
        } => {
            let [x, y, z] = half_extents;
            world.entity_mut(entity).remove::<SceneRoot>();
            apply_mesh_to_entity(
                world,
                entity,
                Mesh::from(Cuboid::new(x * 2.0, y * 2.0, z * 2.0)),
            );
            apply_authored_material_to_entity(world, entity, material);
        }
        XrdsGeometrySource::PbrCylinder {
            radius,
            half_height,
            material,
        } => {
            world.entity_mut(entity).remove::<SceneRoot>();
            apply_mesh_to_entity(
                world,
                entity,
                Mesh::from(Cylinder {
                    radius,
                    half_height,
                }),
            );
            apply_authored_material_to_entity(world, entity, material);
        }
        XrdsGeometrySource::PbrPlane { size, material } => {
            world.entity_mut(entity).remove::<SceneRoot>();
            apply_mesh_to_entity(
                world,
                entity,
                Mesh::from(Plane3d::default().mesh().size(size[0], size[1])),
            );
            apply_authored_material_to_entity(world, entity, material);
        }
        XrdsGeometrySource::PbrTetrahedron { vertices, material } => {
            world.entity_mut(entity).remove::<SceneRoot>();
            apply_mesh_to_entity(world, entity, tetrahedron_mesh(vertices));
            apply_authored_material_to_entity(world, entity, material);
        }
    }
}

pub(super) fn cylinder_recipe_and_common_state_for(
    world: &World,
    entity: Entity,
) -> Option<(XrdsGeometrySource, String, TransformParams, bool)> {
    let descriptor = cylinder_descriptor_ref(world, entity)?;
    let material = material_params_for_entity(world, entity).unwrap_or_default();
    let recipe = XrdsGeometrySource::PbrCylinder {
        radius: descriptor.radius,
        half_height: descriptor.height * 0.5,
        material,
    };
    Some((
        recipe,
        descriptor.name.clone(),
        descriptor.transform,
        descriptor.visible,
    ))
}

pub(super) fn cube_recipe_and_common_state_for(
    world: &World,
    entity: Entity,
) -> Option<(XrdsGeometrySource, String, TransformParams, bool)> {
    let descriptor = cube_descriptor_ref(world, entity)?;
    let material = material_params_for_entity(world, entity).unwrap_or_default();
    let recipe = XrdsGeometrySource::PbrCuboid {
        half_extents: [
            descriptor.size[0] * 0.5,
            descriptor.size[1] * 0.5,
            descriptor.size[2] * 0.5,
        ],
        material,
    };
    Some((
        recipe,
        descriptor.name.clone(),
        descriptor.transform,
        descriptor.visible,
    ))
}

pub(super) fn sphere_recipe_and_common_state_for(
    world: &World,
    entity: Entity,
) -> Option<(XrdsGeometrySource, String, TransformParams, bool)> {
    let descriptor = sphere_descriptor_ref(world, entity)?;
    let material = material_params_for_entity(world, entity).unwrap_or_default();
    let recipe = XrdsGeometrySource::PbrSphere {
        radius: descriptor.radius,
        material,
    };
    Some((
        recipe,
        descriptor.name.clone(),
        descriptor.transform,
        descriptor.visible,
    ))
}

pub(super) fn plane_recipe_and_common_state_for(
    world: &World,
    entity: Entity,
) -> Option<(XrdsGeometrySource, String, TransformParams, bool)> {
    let descriptor = plane_descriptor_ref(world, entity)?;
    let material = material_params_for_entity(world, entity).unwrap_or_default();
    let recipe = XrdsGeometrySource::PbrPlane {
        size: descriptor.size,
        material,
    };
    Some((
        recipe,
        descriptor.name.clone(),
        descriptor.transform,
        descriptor.visible,
    ))
}

pub(super) fn tetrahedron_recipe_and_common_state_for(
    world: &World,
    entity: Entity,
) -> Option<(XrdsGeometrySource, String, TransformParams, bool)> {
    let descriptor = world
        .get::<XrdsStored<XrdsTetrahedron>>(entity)
        .map(|descriptor| &descriptor.0)?;
    let material = material_params_for_entity(world, entity).unwrap_or_default();
    let recipe = XrdsGeometrySource::PbrTetrahedron {
        vertices: descriptor.vertices.map(Into::into),
        material,
    };
    Some((
        recipe,
        descriptor.name.clone(),
        descriptor.transform,
        descriptor.visible,
    ))
}

fn camera_transform_with_look_at(camera: &XrdsCamera) -> TransformParams {
    let mut params = camera.transform;
    if let Some(target) = camera.look_at {
        let pos = Vec3::from_array(params.translation);
        let rot = Transform::from_translation(pos)
            .looking_at(Vec3::from_array(target), Vec3::Y)
            .rotation;
        params.rotation_quat_xyzw = [rot.x, rot.y, rot.z, rot.w];
    }
    params
}

fn spawn_node_descriptor(commands: &mut Commands, node: &XrdsNode) -> Entity {
    let entity = commands.spawn_empty().id();
    let descriptor = node.clone();
    let name = node.name.clone();
    let transform = node.transform;
    let visible = node.visible;

    commands.queue(move |world: &mut World| {
        world.entity_mut(entity).insert((
            Name::new(name),
            build_transform(&transform),
            build_visibility(visible),
            XrdsStored(descriptor),
        ));
    });

    entity
}

fn spawn_camera_descriptor(commands: &mut Commands, camera: &XrdsCamera) -> Entity {
    let entity = commands.spawn_empty().id();
    let descriptor = camera.clone();
    let name = camera.name.clone();
    let transform = camera_transform_with_look_at(camera);
    let visible = camera.visible;
    let clear_color = camera.clear_color;
    let tonemapping = camera.tonemapping;
    let bloom = camera.bloom;
    let projection = camera.projection;

    commands.queue(move |world: &mut World| {
        let mut entity_mut = world.entity_mut(entity);
        entity_mut.insert((
            Name::new(name),
            build_transform(&transform),
            build_visibility(visible),
            XrdsStored(descriptor),
        ));

        projection.insert_into(&mut entity_mut);

        if let Some(mut camera_component) = entity_mut.get_mut::<Camera>() {
            camera_component.clear_color = clear_color.into();
        } else {
            entity_mut.insert(Camera {
                clear_color: clear_color.into(),
                ..default()
            });
        }

        entity_mut.insert(Tonemapping::from(tonemapping));
        if let Some(bloom) = bloom.to_bevy() {
            entity_mut.insert(bloom);
        } else {
            entity_mut.remove::<Bloom>();
        }

        if entity_mut.contains::<Camera>()
            && !entity_mut.contains::<CameraRenderGraph>()
            && !entity_mut.contains::<Camera2d>()
            && !entity_mut.contains::<Camera3d>()
        {
            entity_mut.insert((CameraRenderGraph::new(Core3d), Camera3d::default()));
        }
    });

    entity
}

fn spawn_gltf_descriptor(commands: &mut Commands, asset: &XrdsGltfAsset) -> Option<Entity> {
    if let Err(error) = validate_gltf_source(&asset.gltf_asset_path, asset.scene_index) {
        warn!("Skipping glTF spawn for '{}': {error}", asset.name);
        return None;
    }

    let entity = commands.spawn_empty().id();
    let descriptor = asset.clone();
    let name = asset.name.clone();
    let transform = asset.transform;
    let visible = asset.visible;
    let path = asset.gltf_asset_path.clone();
    let scene_index = asset.scene_index;

    commands.queue(move |world: &mut World| {
        let scene_handle = {
            let server = world.resource::<AssetServer>();
            let asset_path = build_scene_asset_path(&path, scene_index);
            server.load::<Scene>(asset_path)
        };

        world.entity_mut(entity).insert((
            Name::new(name),
            SceneRoot(scene_handle),
            build_transform(&transform),
            GlobalTransform::default(),
            build_visibility_hierarchy_components(visible),
            XrdsStored(descriptor),
        ));
    });

    Some(entity)
}

fn spawn_cube_descriptor(commands: &mut Commands, cube: &XrdsCube) -> Entity {
    let entity = commands.spawn_empty().id();
    let descriptor = cube.clone();
    let name = cube.name.clone();
    let transform = cube.transform;
    let visible = cube.visible;
    let size = cube.size;
    let material = XrdsMaterialParams::default();

    commands.queue(move |world: &mut World| {
        let mesh = {
            let mut meshes = world.resource_mut::<Assets<Mesh>>();
            meshes.add(Mesh::from(Cuboid::new(size[0], size[1], size[2])))
        };

        world.entity_mut(entity).insert((
            Name::new(name),
            Mesh3d(mesh),
            build_transform(&transform),
            build_visibility(visible),
            XrdsStored(descriptor),
        ));
        apply_authored_material_to_entity(world, entity, material);
    });

    entity
}

fn spawn_cylinder_descriptor(commands: &mut Commands, cylinder: &XrdsCylinder) -> Entity {
    let entity = commands.spawn_empty().id();
    let descriptor = cylinder.clone();
    let name = cylinder.name.clone();
    let transform = cylinder.transform;
    let visible = cylinder.visible;
    let radius = cylinder.radius;
    let height = cylinder.height;
    let material = XrdsMaterialParams::default();

    commands.queue(move |world: &mut World| {
        let mesh = {
            let mut meshes = world.resource_mut::<Assets<Mesh>>();
            meshes.add(Mesh::from(Cylinder::new(radius, height)))
        };

        world.entity_mut(entity).insert((
            Name::new(name),
            Mesh3d(mesh),
            build_transform(&transform),
            build_visibility(visible),
            XrdsStored(descriptor),
        ));
        apply_authored_material_to_entity(world, entity, material);
    });

    entity
}

fn spawn_sphere_descriptor(commands: &mut Commands, sphere: &XrdsSphere) -> Entity {
    let entity = commands.spawn_empty().id();
    let descriptor = sphere.clone();
    let name = sphere.name.clone();
    let transform = sphere.transform;
    let visible = sphere.visible;
    let radius = sphere.radius;
    let material = XrdsMaterialParams::default();

    commands.queue(move |world: &mut World| {
        let mesh = {
            let mut meshes = world.resource_mut::<Assets<Mesh>>();
            meshes.add(Mesh::from(Sphere::new(radius)))
        };

        world.entity_mut(entity).insert((
            Name::new(name),
            Mesh3d(mesh),
            build_transform(&transform),
            build_visibility(visible),
            XrdsStored(descriptor),
        ));
        apply_authored_material_to_entity(world, entity, material);
    });

    entity
}

fn spawn_plane_descriptor(commands: &mut Commands, plane: &XrdsPlane3D) -> Entity {
    let entity = commands.spawn_empty().id();
    let descriptor = plane.clone();
    let name = plane.name.clone();
    let transform = plane.transform;
    let visible = plane.visible;
    let size = plane.size;
    let material = XrdsMaterialParams::default();

    commands.queue(move |world: &mut World| {
        let mesh = {
            let mut meshes = world.resource_mut::<Assets<Mesh>>();
            meshes.add(Mesh::from(Plane3d::default().mesh().size(size[0], size[1])))
        };

        world.entity_mut(entity).insert((
            Name::new(name),
            Mesh3d(mesh),
            build_transform(&transform),
            build_visibility(visible),
            XrdsStored(descriptor),
        ));
        apply_authored_material_to_entity(world, entity, material);
    });

    entity
}

fn tetra_positions_and_normals(vertices: [[f32; 3]; 4]) -> (Vec<[f32; 3]>, Vec<[f32; 3]>) {
    let faces = [[0usize, 2usize, 1usize], [0, 1, 3], [0, 3, 2], [1, 2, 3]];
    let mut positions = Vec::with_capacity(12);
    let mut normals = Vec::with_capacity(12);

    for [a, b, c] in faces {
        let va = Vec3::from_array(vertices[a]);
        let vb = Vec3::from_array(vertices[b]);
        let vc = Vec3::from_array(vertices[c]);
        let normal = (vb - va).cross(vc - va).normalize_or_zero().to_array();

        positions.push(vertices[a]);
        positions.push(vertices[b]);
        positions.push(vertices[c]);
        normals.push(normal);
        normals.push(normal);
        normals.push(normal);
    }

    (positions, normals)
}

fn tetrahedron_mesh(vertices: [[f32; 3]; 4]) -> Mesh {
    let (positions, normals) = tetra_positions_and_normals(vertices);
    let indices: Vec<u32> = (0..positions.len() as u32).collect();

    Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::RENDER_WORLD,
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_indices(Indices::U32(indices))
}

fn spawn_ambient_light_descriptor(commands: &mut Commands, light: &XrdsAmbientLight) -> Entity {
    let entity = commands.spawn_empty().id();
    let descriptor = light.clone();
    let name = light.name.clone();
    let transform = light.transform;
    let visible = light.visible;
    let color = light.color;
    let brightness = light.brightness;
    let affects_lightmapped_meshes = light.affects_lightmapped_meshes;

    commands.queue(move |world: &mut World| {
        let visibility = build_visibility(visible);
        world.insert_resource(AmbientLight {
            color: color.into(),
            brightness,
            affects_lightmapped_meshes,
        });
        world.entity_mut(entity).insert((
            Name::new(name),
            build_transform(&transform),
            visibility,
            XrdsStored(descriptor),
        ));
    });

    entity
}

fn spawn_directional_light_descriptor(
    commands: &mut Commands,
    light: &XrdsDirectionalLight,
) -> Entity {
    let entity = commands.spawn_empty().id();
    let descriptor = light.clone();
    let name = light.name.clone();
    let transform = light.transform;
    let visible = light.visible;
    let color = light.color;
    let illuminance = light.illuminance;
    let shadows = light.shadows;

    commands.queue(move |world: &mut World| {
        world.entity_mut(entity).insert((
            Name::new(name),
            build_transform(&transform),
            build_visibility(visible),
            DirectionalLight {
                color: color.into(),
                illuminance,
                shadows_enabled: shadows,
                ..default()
            },
            XrdsStored(descriptor),
        ));
    });

    entity
}

fn spawn_point_light_descriptor(commands: &mut Commands, light: &XrdsPointLight) -> Entity {
    let entity = commands.spawn_empty().id();
    let descriptor = light.clone();
    let name = light.name.clone();
    let transform = light.transform;
    let visible = light.visible;
    let color = light.color;
    let intensity = light.intensity;
    let range = light.range;
    let radius = light.radius;
    let shadows = light.shadows;

    commands.queue(move |world: &mut World| {
        world.entity_mut(entity).insert((
            Name::new(name),
            build_transform(&transform),
            build_visibility(visible),
            PointLight {
                color: color.into(),
                intensity,
                range,
                radius,
                shadows_enabled: shadows,
                ..default()
            },
            XrdsStored(descriptor),
        ));
    });

    entity
}

fn spawn_spot_light_descriptor(commands: &mut Commands, light: &XrdsSpotLight) -> Entity {
    let entity = commands.spawn_empty().id();
    let descriptor = light.clone();
    let name = light.name.clone();
    let transform = light.transform;
    let visible = light.visible;
    let color = light.color;
    let intensity = light.intensity;
    let range = light.range;
    let inner_angle = light.inner_angle;
    let outer_angle = light.outer_angle;
    let shadows = light.shadows;

    commands.queue(move |world: &mut World| {
        world.entity_mut(entity).insert((
            Name::new(name),
            build_transform(&transform),
            build_visibility(visible),
            SpotLight {
                color: color.into(),
                intensity,
                range,
                inner_angle,
                outer_angle,
                shadows_enabled: shadows,
                ..default()
            },
            XrdsStored(descriptor),
        ));
    });

    entity
}

fn build_scene_asset_path(path: &str, scene_index: usize) -> String {
    if path.contains('#') {
        path.to_string()
    } else {
        format!("{path}#Scene{scene_index}")
    }
}

pub(super) fn execute_spawn_recipe(
    commands: &mut Commands,
    recipe: XrdsGeometrySource,
    name: String,
    transform: TransformParams,
    visible: bool,
) -> Entity {
    match recipe {
        XrdsGeometrySource::GltfScene { path, scene_index } => {
            let entity = commands.spawn_empty().id();
            commands.queue(move |world: &mut World| {
                let scene_handle = {
                    let server = world.resource::<AssetServer>();
                    let path = build_scene_asset_path(&path, scene_index);
                    server.load::<Scene>(path)
                };
                world.entity_mut(entity).insert((
                    Name::new(name),
                    SceneRoot(scene_handle),
                    build_transform(&transform),
                    GlobalTransform::default(),
                    build_visibility_hierarchy_components(visible),
                ));
            });
            entity
        }
        XrdsGeometrySource::PbrSphere { radius, material } => {
            let entity = commands.spawn_empty().id();
            commands.queue(move |world: &mut World| {
                let mesh = {
                    let mut meshes = world.resource_mut::<Assets<Mesh>>();
                    meshes.add(Mesh::from(Sphere { radius }))
                };
                let material_handle = {
                    let mut materials = world.resource_mut::<Assets<StandardMaterial>>();
                    materials.add(standard_material_from_authored(material))
                };
                world.entity_mut(entity).insert((
                    Name::new(name),
                    Mesh3d(mesh),
                    MeshMaterial3d(material_handle),
                    build_transform(&transform),
                    build_visibility(visible),
                    XrdsStoredMaterial(material),
                ));
            });
            entity
        }
        XrdsGeometrySource::PbrCuboid {
            half_extents,
            material,
        } => {
            let entity = commands.spawn_empty().id();
            commands.queue(move |world: &mut World| {
                let mesh = {
                    let mut meshes = world.resource_mut::<Assets<Mesh>>();
                    let [x, y, z] = half_extents;
                    meshes.add(Mesh::from(Cuboid::new(x * 2.0, y * 2.0, z * 2.0)))
                };
                let material_handle = {
                    let mut materials = world.resource_mut::<Assets<StandardMaterial>>();
                    materials.add(standard_material_from_authored(material))
                };
                world.entity_mut(entity).insert((
                    Name::new(name),
                    Mesh3d(mesh),
                    MeshMaterial3d(material_handle),
                    build_transform(&transform),
                    build_visibility(visible),
                    XrdsStoredMaterial(material),
                ));
            });
            entity
        }
        XrdsGeometrySource::PbrCylinder {
            radius,
            half_height,
            material,
        } => {
            let entity = commands.spawn_empty().id();
            commands.queue(move |world: &mut World| {
                let mesh = {
                    let mut meshes = world.resource_mut::<Assets<Mesh>>();
                    meshes.add(Mesh::from(Cylinder {
                        radius,
                        half_height,
                    }))
                };
                let material_handle = {
                    let mut materials = world.resource_mut::<Assets<StandardMaterial>>();
                    materials.add(standard_material_from_authored(material))
                };
                world.entity_mut(entity).insert((
                    Name::new(name),
                    Mesh3d(mesh),
                    MeshMaterial3d(material_handle),
                    build_transform(&transform),
                    build_visibility(visible),
                    XrdsStoredMaterial(material),
                ));
            });
            entity
        }
        XrdsGeometrySource::PbrPlane { size, material } => {
            let entity = commands.spawn_empty().id();
            commands.queue(move |world: &mut World| {
                let mesh = {
                    let mut meshes = world.resource_mut::<Assets<Mesh>>();
                    meshes.add(Mesh::from(Plane3d::default().mesh().size(size[0], size[1])))
                };
                let material_handle = {
                    let mut materials = world.resource_mut::<Assets<StandardMaterial>>();
                    materials.add(standard_material_from_authored(material))
                };
                world.entity_mut(entity).insert((
                    Name::new(name),
                    Mesh3d(mesh),
                    MeshMaterial3d(material_handle),
                    build_transform(&transform),
                    build_visibility(visible),
                    XrdsStoredMaterial(material),
                ));
            });
            entity
        }
        XrdsGeometrySource::PbrTetrahedron { vertices, material } => {
            let entity = commands.spawn_empty().id();
            commands.queue(move |world: &mut World| {
                let mesh = {
                    let mut meshes = world.resource_mut::<Assets<Mesh>>();
                    meshes.add(tetrahedron_mesh(vertices))
                };
                let material_handle = {
                    let mut materials = world.resource_mut::<Assets<StandardMaterial>>();
                    materials.add(standard_material_from_authored(material))
                };
                world.entity_mut(entity).insert((
                    Name::new(name),
                    Mesh3d(mesh),
                    MeshMaterial3d(material_handle),
                    build_transform(&transform),
                    build_visibility(visible),
                    XrdsStoredMaterial(material),
                ));
            });
            entity
        }
    }
}

fn register_default_interpreters(registry: &mut SurfaceInterpreterRegistry) {
    registry.register_entity::<XrdsNode, _>(|node, commands, _asset_server| {
        spawn_node_descriptor(commands, node)
    });
    registry.register_entity::<XrdsCamera, _>(|camera, commands, _asset_server| {
        spawn_camera_descriptor(commands, camera)
    });
    registry.register_entity::<XrdsPointLight, _>(|light, commands, _asset_server| {
        spawn_point_light_descriptor(commands, light)
    });
    registry.register_entity::<XrdsDirectionalLight, _>(|light, commands, _asset_server| {
        spawn_directional_light_descriptor(commands, light)
    });
    registry.register_entity::<XrdsSpotLight, _>(|light, commands, _asset_server| {
        spawn_spot_light_descriptor(commands, light)
    });
    registry.register_entity::<XrdsAmbientLight, _>(|light, commands, _asset_server| {
        spawn_ambient_light_descriptor(commands, light)
    });
    registry.register_optional_entity::<XrdsGltfAsset, _>(|asset, commands, asset_server| {
        let _ = asset_server;
        spawn_gltf_descriptor(commands, asset)
    });
    registry.register_recipe_only::<XrdsCube, _>(|cube| XrdsGeometrySource::PbrCuboid {
        half_extents: [cube.size[0] * 0.5, cube.size[1] * 0.5, cube.size[2] * 0.5],
        material: XrdsMaterialParams::default(),
    });
    registry.register_entity::<XrdsCube, _>(|cube, commands, _asset_server| {
        spawn_cube_descriptor(commands, cube)
    });
    registry.register_recipe_only::<XrdsCylinder, _>(|cylinder| XrdsGeometrySource::PbrCylinder {
        radius: cylinder.radius,
        half_height: cylinder.height * 0.5,
        material: XrdsMaterialParams::default(),
    });
    registry.register_entity::<XrdsCylinder, _>(|cylinder, commands, _asset_server| {
        spawn_cylinder_descriptor(commands, cylinder)
    });
    registry.register_recipe_only::<XrdsSphere, _>(|sphere| XrdsGeometrySource::PbrSphere {
        radius: sphere.radius,
        material: XrdsMaterialParams::default(),
    });
    registry.register_entity::<XrdsSphere, _>(|sphere, commands, _asset_server| {
        spawn_sphere_descriptor(commands, sphere)
    });
    registry.register_recipe_only::<XrdsPlane3D, _>(|plane| XrdsGeometrySource::PbrPlane {
        size: plane.size,
        material: XrdsMaterialParams::default(),
    });
    registry.register_entity::<XrdsPlane3D, _>(|plane, commands, _asset_server| {
        spawn_plane_descriptor(commands, plane)
    });
    registry.register_recipe_only::<XrdsTetrahedron, _>(|tetrahedron| {
        XrdsGeometrySource::PbrTetrahedron {
            vertices: tetrahedron.vertices.map(Into::into),
            material: XrdsMaterialParams::default(),
        }
    });
    registry.register_entity::<XrdsTetrahedron, _>(|tetrahedron, commands, _asset_server| {
        let entity = execute_spawn_recipe(
            commands,
            XrdsGeometrySource::PbrTetrahedron {
                vertices: tetrahedron.vertices.map(Into::into),
                material: XrdsMaterialParams::default(),
            },
            tetrahedron.name.clone(),
            tetrahedron.transform,
            tetrahedron.visible,
        );
        commands
            .entity(entity)
            .insert(XrdsStored(tetrahedron.clone()));
        entity
    });
}

fn register_default_descriptor_cloners(registry: &mut SurfaceDescriptorRegistry) {
    registry.register_clone::<XrdsNode>();
    registry.register_clone::<XrdsCamera>();
    registry.register_clone::<XrdsGltfAsset>();
    registry.register_clone::<XrdsCube>();
    registry.register_clone::<XrdsCylinder>();
    registry.register_clone::<XrdsSphere>();
    registry.register_clone::<XrdsPlane3D>();
    registry.register_clone::<XrdsTetrahedron>();
    registry.register_clone::<XrdsPointLight>();
    registry.register_clone::<XrdsDirectionalLight>();
    registry.register_clone::<XrdsSpotLight>();
    registry.register_clone::<XrdsAmbientLight>();
}
