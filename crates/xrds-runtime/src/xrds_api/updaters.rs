use super::*;

fn register_common_stored_updaters<C>(registry: &mut SurfaceUpdateRegistry)
where
    C: XrdsMutableComponent + Send + Sync + 'static,
{
    registry.register::<C, TransformParams, _>(|world, entity, params| {
        apply_transform_to_entity(world, entity, *params);
        let _ = with_stored_descriptor_mut::<C, _>(world, entity, |descriptor| {
            *descriptor.local_transform_mut() = *params;
        });
    });

    registry.register::<C, ParentPatch, _>(|world, entity, params| {
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

    registry.register::<C, NamePatch, _>(|world, entity, params| {
        world
            .entity_mut(entity)
            .insert(Name::new(params.name.clone()));
        let _ = with_stored_descriptor_mut::<C, _>(world, entity, |descriptor| {
            descriptor.set_name(params.name.clone());
        });
    });

    registry.register::<C, VisibilityPatch, _>(|world, entity, params| {
        world
            .entity_mut(entity)
            .insert(build_visibility(params.visible));
        let _ = with_stored_descriptor_mut::<C, _>(world, entity, |descriptor| {
            descriptor.set_visible(params.visible);
        });
    });
}

fn register_stored_cylinder_updaters(registry: &mut SurfaceUpdateRegistry) {
    register_common_stored_updaters::<XrdsCylinder>(registry);

    registry.register::<XrdsCylinder, XrdsColor, _>(|world, entity, color| {
        let mut params = material_params_for_entity(world, entity).unwrap_or_default();
        params.base_color = *color;
        apply_authored_material_to_entity(world, entity, params);
    });

    registry.register::<XrdsCylinder, XrdsMaterialParams, _>(|world, entity, params| {
        apply_authored_material_to_entity(world, entity, *params);
    });

    registry.register::<XrdsCylinder, CylinderGeometryParams, _>(|world, entity, params| {
        if with_stored_descriptor_mut::<XrdsCylinder, _>(world, entity, |descriptor| {
            descriptor.radius = params.radius;
            descriptor.height = params.height;
        })
        .is_none()
        {
            return;
        }

        let Some((recipe, name, transform, visible)) =
            cylinder_recipe_and_common_state_for(world, entity)
        else {
            return;
        };

        apply_spawn_recipe_to_entity(world, entity, recipe, name, transform, visible);
    });
}

fn register_stored_cube_updaters(registry: &mut SurfaceUpdateRegistry) {
    register_common_stored_updaters::<XrdsCube>(registry);

    registry.register::<XrdsCube, XrdsColor, _>(|world, entity, color| {
        let mut params = material_params_for_entity(world, entity).unwrap_or_default();
        params.base_color = *color;
        apply_authored_material_to_entity(world, entity, params);
    });

    registry.register::<XrdsCube, XrdsMaterialParams, _>(|world, entity, params| {
        apply_authored_material_to_entity(world, entity, *params);
    });

    registry.register::<XrdsCube, CubeGeometryParams, _>(|world, entity, params| {
        if with_stored_descriptor_mut::<XrdsCube, _>(world, entity, |descriptor| {
            descriptor.size = params.size;
        })
        .is_none()
        {
            return;
        }

        let Some((recipe, name, transform, visible)) =
            cube_recipe_and_common_state_for(world, entity)
        else {
            return;
        };

        apply_spawn_recipe_to_entity(world, entity, recipe, name, transform, visible);
    });
}

fn register_stored_sphere_updaters(registry: &mut SurfaceUpdateRegistry) {
    register_common_stored_updaters::<XrdsSphere>(registry);

    registry.register::<XrdsSphere, XrdsColor, _>(|world, entity, color| {
        let mut params = material_params_for_entity(world, entity).unwrap_or_default();
        params.base_color = *color;
        apply_authored_material_to_entity(world, entity, params);
    });

    registry.register::<XrdsSphere, XrdsMaterialParams, _>(|world, entity, params| {
        apply_authored_material_to_entity(world, entity, *params);
    });

    registry.register::<XrdsSphere, SphereGeometryParams, _>(|world, entity, params| {
        if with_stored_descriptor_mut::<XrdsSphere, _>(world, entity, |descriptor| {
            descriptor.radius = params.radius;
        })
        .is_none()
        {
            return;
        }

        let Some((recipe, name, transform, visible)) =
            sphere_recipe_and_common_state_for(world, entity)
        else {
            return;
        };

        apply_spawn_recipe_to_entity(world, entity, recipe, name, transform, visible);
    });
}

fn register_stored_plane_updaters(registry: &mut SurfaceUpdateRegistry) {
    register_common_stored_updaters::<XrdsPlane3D>(registry);

    registry.register::<XrdsPlane3D, XrdsColor, _>(|world, entity, color| {
        let mut params = material_params_for_entity(world, entity).unwrap_or_default();
        params.base_color = *color;
        apply_authored_material_to_entity(world, entity, params);
    });

    registry.register::<XrdsPlane3D, XrdsMaterialParams, _>(|world, entity, params| {
        apply_authored_material_to_entity(world, entity, *params);
    });

    registry.register::<XrdsPlane3D, Plane3DGeometryParams, _>(|world, entity, params| {
        if with_stored_descriptor_mut::<XrdsPlane3D, _>(world, entity, |descriptor| {
            descriptor.size = params.size;
        })
        .is_none()
        {
            return;
        }

        let Some((recipe, name, transform, visible)) =
            plane_recipe_and_common_state_for(world, entity)
        else {
            return;
        };

        apply_spawn_recipe_to_entity(world, entity, recipe, name, transform, visible);
    });
}

fn register_stored_tetrahedron_updaters(registry: &mut SurfaceUpdateRegistry) {
    register_common_stored_updaters::<XrdsTetrahedron>(registry);

    registry.register::<XrdsTetrahedron, XrdsColor, _>(|world, entity, color| {
        let mut params = material_params_for_entity(world, entity).unwrap_or_default();
        params.base_color = *color;
        apply_authored_material_to_entity(world, entity, params);
    });

    registry.register::<XrdsTetrahedron, XrdsMaterialParams, _>(|world, entity, params| {
        apply_authored_material_to_entity(world, entity, *params);
    });

    registry.register::<XrdsTetrahedron, TetrahedronGeometryParams, _>(|world, entity, params| {
        if with_stored_descriptor_mut::<XrdsTetrahedron, _>(world, entity, |descriptor| {
            descriptor.vertices = params.vertices.map(Into::into);
        })
        .is_none()
        {
            return;
        }

        let Some((recipe, name, transform, visible)) =
            tetrahedron_recipe_and_common_state_for(world, entity)
        else {
            return;
        };

        apply_spawn_recipe_to_entity(world, entity, recipe, name, transform, visible);
    });
}

fn register_stored_node_updaters(registry: &mut SurfaceUpdateRegistry) {
    register_common_stored_updaters::<XrdsNode>(registry);
}

fn register_stored_camera_updaters(registry: &mut SurfaceUpdateRegistry) {
    register_common_stored_updaters::<XrdsCamera>(registry);
}

fn register_stored_gltf_updaters(registry: &mut SurfaceUpdateRegistry) {
    register_common_stored_updaters::<XrdsGltfAsset>(registry);
}

fn register_stored_point_light_updaters(registry: &mut SurfaceUpdateRegistry) {
    register_common_stored_updaters::<XrdsPointLight>(registry);

    registry.register::<XrdsPointLight, PointLightParams, _>(|world, entity, params| {
        if let Some(mut light) = world.get_mut::<PointLight>(entity) {
            light.color = params.color.into();
            light.intensity = params.intensity;
            light.range = params.range;
            light.radius = params.radius;
            light.shadows_enabled = params.shadows;
        }
        let _ = with_stored_descriptor_mut::<XrdsPointLight, _>(world, entity, |descriptor| {
            descriptor.color = params.color;
            descriptor.intensity = params.intensity;
            descriptor.range = params.range;
            descriptor.radius = params.radius;
            descriptor.shadows = params.shadows;
        });
    });
}

fn register_stored_directional_light_updaters(registry: &mut SurfaceUpdateRegistry) {
    register_common_stored_updaters::<XrdsDirectionalLight>(registry);

    registry.register::<XrdsDirectionalLight, DirectionalLightParams, _>(
        |world, entity, params| {
            if let Some(mut light) = world.get_mut::<DirectionalLight>(entity) {
                light.color = params.color.into();
                light.illuminance = params.illuminance;
                light.shadows_enabled = params.shadows;
            }
            let _ = with_stored_descriptor_mut::<XrdsDirectionalLight, _>(
                world,
                entity,
                |descriptor| {
                    descriptor.color = params.color;
                    descriptor.illuminance = params.illuminance;
                    descriptor.shadows = params.shadows;
                },
            );
        },
    );
}

fn register_stored_spot_light_updaters(registry: &mut SurfaceUpdateRegistry) {
    register_common_stored_updaters::<XrdsSpotLight>(registry);

    registry.register::<XrdsSpotLight, SpotLightParams, _>(|world, entity, params| {
        if let Some(mut light) = world.get_mut::<SpotLight>(entity) {
            light.color = params.color.into();
            light.intensity = params.intensity;
            light.range = params.range;
            light.inner_angle = params.inner_angle;
            light.outer_angle = params.outer_angle;
            light.shadows_enabled = params.shadows;
        }
        let _ = with_stored_descriptor_mut::<XrdsSpotLight, _>(world, entity, |descriptor| {
            descriptor.color = params.color;
            descriptor.intensity = params.intensity;
            descriptor.range = params.range;
            descriptor.inner_angle = params.inner_angle;
            descriptor.outer_angle = params.outer_angle;
            descriptor.shadows = params.shadows;
        });
    });
}

fn register_stored_ambient_light_updaters(registry: &mut SurfaceUpdateRegistry) {
    register_common_stored_updaters::<XrdsAmbientLight>(registry);

    registry.register::<XrdsAmbientLight, AmbientLightParams, _>(|world, entity, params| {
        if let Some(mut ambient) = world.get_resource_mut::<AmbientLight>() {
            ambient.color = params.color.into();
            ambient.brightness = params.brightness;
            ambient.affects_lightmapped_meshes = params.affects_lightmapped_meshes;
        }
        let _ = with_stored_descriptor_mut::<XrdsAmbientLight, _>(world, entity, |descriptor| {
            descriptor.color = params.color;
            descriptor.brightness = params.brightness;
            descriptor.affects_lightmapped_meshes = params.affects_lightmapped_meshes;
        });
    });
}

fn register_default_mutable_updaters(registry: &mut SurfaceUpdateRegistry) {
    register_stored_node_updaters(registry);
    register_stored_camera_updaters(registry);
    register_stored_gltf_updaters(registry);
    register_stored_cube_updaters(registry);
    register_stored_cylinder_updaters(registry);
    register_stored_sphere_updaters(registry);
    register_stored_plane_updaters(registry);
    register_stored_tetrahedron_updaters(registry);
    register_stored_point_light_updaters(registry);
    register_stored_directional_light_updaters(registry);
    register_stored_spot_light_updaters(registry);
    register_stored_ambient_light_updaters(registry);
}

fn register_default_primitive_updaters(registry: &mut SurfaceUpdateRegistry) {
    let _ = registry;
}

pub(super) fn register_default_updaters(registry: &mut SurfaceUpdateRegistry) {
    register_default_mutable_updaters(registry);
    register_default_primitive_updaters(registry);

    registry.register::<XrdsCamera, CameraProjectionPatch, _>(|world, entity, patch| {
        let mut entity_mut = world.entity_mut(entity);
        patch.projection.insert_into(&mut entity_mut);
        let _ = with_stored_descriptor_mut::<XrdsCamera, _>(world, entity, |descriptor| {
            descriptor.projection = patch.projection;
        });
    });
    registry.register::<XrdsCamera, CameraLookAtPatch, _>(|world, entity, params| {
        let position = world
            .get::<Transform>(entity)
            .map(|t| t.translation)
            .unwrap_or(Vec3::ZERO);
        let rotation = params.look_at.map(|target| {
            Transform::from_translation(position)
                .looking_at(Vec3::from_array(target), Vec3::Y)
                .rotation
        });
        if let Some(rotation) = rotation {
            if let Some(mut transform) = world.get_mut::<Transform>(entity) {
                transform.rotation = rotation;
            }
        }
        let _ = with_stored_descriptor_mut::<XrdsCamera, _>(world, entity, |descriptor| {
            descriptor.look_at = params.look_at;
            if let Some(rotation) = rotation {
                descriptor.transform.rotation_quat_xyzw =
                    [rotation.x, rotation.y, rotation.z, rotation.w];
            }
        });
    });
    registry.register::<XrdsGltfAsset, GltfAssetSourcePatch, _>(|world, entity, params| {
        if let Err(error) = validate_gltf_source(&params.gltf_asset_path, params.scene_index) {
            warn!(
                "Ignoring invalid glTF update for entity {:?}: {error}",
                entity
            );
            return;
        }

        let scene_handle = {
            let server = world.resource::<AssetServer>();
            let path = if params.gltf_asset_path.contains('#') {
                params.gltf_asset_path.clone()
            } else {
                format!("{}#Scene{}", params.gltf_asset_path, params.scene_index)
            };
            server.load::<Scene>(path)
        };
        world.entity_mut(entity).insert((
            SceneRoot(scene_handle),
            GlobalTransform::default(),
            build_visibility_hierarchy_components(true),
        ));
        let _ = with_stored_descriptor_mut::<XrdsGltfAsset, _>(world, entity, |descriptor| {
            descriptor.gltf_asset_path = params.gltf_asset_path.clone();
            descriptor.scene_index = params.scene_index;
        });
    });
}
