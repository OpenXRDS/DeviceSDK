use super::*;
use bevy::asset::RenderAssetUsages;
use bevy::mesh::Indices;
use bevy::render::render_resource::PrimitiveTopology;

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

fn apply_scene_recipe_to_entity(
    world: &mut World,
    entity: Entity,
    path: String,
    scene_index: usize,
) {
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
        .remove::<MeshMaterial3d<XrdsRuntimeMaterial>>();
    world.entity_mut(entity).insert((
        SceneRoot(scene_handle),
        GlobalTransform::default(),
        build_visibility_hierarchy_components(true),
    ));
}

fn apply_pbr_recipe_to_entity(
    world: &mut World,
    entity: Entity,
    mesh: Mesh,
    material: XrdsMaterialParams,
) {
    world.entity_mut(entity).remove::<SceneRoot>();
    apply_mesh_to_entity(world, entity, mesh);
    apply_authored_material_to_entity(world, entity, material);
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
            apply_scene_recipe_to_entity(world, entity, path, scene_index);
        }
        XrdsGeometrySource::PbrSphere { radius, material } => {
            apply_pbr_recipe_to_entity(world, entity, Mesh::from(Sphere { radius }), material);
        }
        XrdsGeometrySource::PbrCuboid {
            half_extents,
            material,
        } => {
            let [x, y, z] = half_extents;
            apply_pbr_recipe_to_entity(
                world,
                entity,
                Mesh::from(Cuboid::new(x * 2.0, y * 2.0, z * 2.0)),
                material,
            );
        }
        XrdsGeometrySource::PbrCylinder {
            radius,
            half_height,
            material,
        } => {
            apply_pbr_recipe_to_entity(
                world,
                entity,
                Mesh::from(Cylinder {
                    radius,
                    half_height,
                }),
                material,
            );
        }
        XrdsGeometrySource::PbrCapsule {
            radius,
            half_length,
            material,
        } => {
            apply_pbr_recipe_to_entity(
                world,
                entity,
                Mesh::from(Capsule3d {
                    radius,
                    half_length,
                }),
                material,
            );
        }
        XrdsGeometrySource::PbrPlane { size, material } => {
            apply_pbr_recipe_to_entity(
                world,
                entity,
                Mesh::from(Plane3d::default().mesh().size(size[0], size[1])),
                material,
            );
        }
        XrdsGeometrySource::PbrTetrahedron { vertices, material } => {
            apply_pbr_recipe_to_entity(world, entity, tetrahedron_mesh(vertices), material);
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

pub(super) fn capsule_recipe_and_common_state_for(
    world: &World,
    entity: Entity,
) -> Option<(XrdsGeometrySource, String, TransformParams, bool)> {
    let descriptor = capsule_descriptor_ref(world, entity)?;
    let material = material_params_for_entity(world, entity).unwrap_or_default();
    let recipe = XrdsGeometrySource::PbrCapsule {
        radius: descriptor.radius,
        half_length: descriptor.length * 0.5,
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
        RenderAssetUsages::default(),
    )
    .with_inserted_attribute(Mesh::ATTRIBUTE_POSITION, positions)
    .with_inserted_attribute(Mesh::ATTRIBUTE_NORMAL, normals)
    .with_inserted_indices(Indices::U32(indices))
}

fn insert_spawned_pbr_recipe_entity(
    world: &mut World,
    entity: Entity,
    name: String,
    transform: TransformParams,
    visible: bool,
    mesh: Mesh,
    material: XrdsMaterialParams,
) {
    let mesh_handle = {
        let mut meshes = world.resource_mut::<Assets<Mesh>>();
        meshes.add(mesh)
    };
    let material_value = runtime_material_from_authored_in_world(Some(world), material.clone());
    let material_handle = {
        let mut materials = world.resource_mut::<Assets<XrdsRuntimeMaterial>>();
        materials.add(material_value)
    };

    world.entity_mut(entity).insert((
        Name::new(name),
        Mesh3d(mesh_handle),
        MeshMaterial3d(material_handle),
        build_transform(&transform),
        build_visibility_hierarchy_components(visible),
        XrdsStoredMaterial(material),
    ));
}

fn spawn_scene_recipe_entity(
    commands: &mut Commands,
    path: String,
    scene_index: usize,
    name: String,
    transform: TransformParams,
    visible: bool,
) -> Entity {
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

fn spawn_pbr_recipe_entity(
    commands: &mut Commands,
    name: String,
    transform: TransformParams,
    visible: bool,
    mesh: Mesh,
    material: XrdsMaterialParams,
) -> Entity {
    let entity = commands.spawn_empty().id();
    commands.queue(move |world: &mut World| {
        insert_spawned_pbr_recipe_entity(world, entity, name, transform, visible, mesh, material);
    });
    entity
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
            spawn_scene_recipe_entity(commands, path, scene_index, name, transform, visible)
        }
        XrdsGeometrySource::PbrSphere { radius, material } => spawn_pbr_recipe_entity(
            commands,
            name,
            transform,
            visible,
            Mesh::from(Sphere { radius }),
            material,
        ),
        XrdsGeometrySource::PbrCuboid {
            half_extents,
            material,
        } => {
            let [x, y, z] = half_extents;
            spawn_pbr_recipe_entity(
                commands,
                name,
                transform,
                visible,
                Mesh::from(Cuboid::new(x * 2.0, y * 2.0, z * 2.0)),
                material,
            )
        }
        XrdsGeometrySource::PbrCylinder {
            radius,
            half_height,
            material,
        } => spawn_pbr_recipe_entity(
            commands,
            name,
            transform,
            visible,
            Mesh::from(Cylinder {
                radius,
                half_height,
            }),
            material,
        ),
        XrdsGeometrySource::PbrCapsule {
            radius,
            half_length,
            material,
        } => spawn_pbr_recipe_entity(
            commands,
            name,
            transform,
            visible,
            Mesh::from(Capsule3d {
                radius,
                half_length,
            }),
            material,
        ),
        XrdsGeometrySource::PbrPlane { size, material } => spawn_pbr_recipe_entity(
            commands,
            name,
            transform,
            visible,
            Mesh::from(Plane3d::default().mesh().size(size[0], size[1])),
            material,
        ),
        XrdsGeometrySource::PbrTetrahedron { vertices, material } => spawn_pbr_recipe_entity(
            commands,
            name,
            transform,
            visible,
            tetrahedron_mesh(vertices),
            material,
        ),
    }
}
