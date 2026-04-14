use super::*;
use bevy::core_pipeline::core_3d::graph::Core3d;
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::post_process::bloom::Bloom;
use bevy::render::camera::CameraRenderGraph;

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

pub(super) fn spawn_node_descriptor(commands: &mut Commands, node: &XrdsNode) -> Entity {
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

pub(super) fn spawn_camera_descriptor(commands: &mut Commands, camera: &XrdsCamera) -> Entity {
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

pub(super) fn spawn_gltf_descriptor(
    commands: &mut Commands,
    asset: &XrdsGltfAsset,
) -> Option<Entity> {
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

pub(super) fn spawn_cube_descriptor(commands: &mut Commands, cube: &XrdsCube) -> Entity {
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

pub(super) fn spawn_cylinder_descriptor(
    commands: &mut Commands,
    cylinder: &XrdsCylinder,
) -> Entity {
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

pub(super) fn spawn_sphere_descriptor(commands: &mut Commands, sphere: &XrdsSphere) -> Entity {
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

pub(super) fn spawn_plane_descriptor(commands: &mut Commands, plane: &XrdsPlane3D) -> Entity {
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

pub(super) fn spawn_ambient_light_descriptor(
    commands: &mut Commands,
    light: &XrdsAmbientLight,
) -> Entity {
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

pub(super) fn spawn_directional_light_descriptor(
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

pub(super) fn spawn_point_light_descriptor(
    commands: &mut Commands,
    light: &XrdsPointLight,
) -> Entity {
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

pub(super) fn spawn_spot_light_descriptor(
    commands: &mut Commands,
    light: &XrdsSpotLight,
) -> Entity {
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

pub(super) fn build_scene_asset_path(path: &str, scene_index: usize) -> String {
    if path.contains('#') {
        path.to_string()
    } else {
        format!("{path}#Scene{scene_index}")
    }
}
