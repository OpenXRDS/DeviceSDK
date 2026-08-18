use super::*;

pub(super) fn register_default_interpreters(registry: &mut SurfaceInterpreterRegistry) {
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
    registry.register_recipe_only::<XrdsCapsule, _>(|capsule| XrdsGeometrySource::PbrCapsule {
        radius: capsule.radius,
        half_length: capsule.length * 0.5,
        material: XrdsMaterialParams::default(),
    });
    registry.register_entity::<XrdsCapsule, _>(|capsule, commands, _asset_server| {
        spawn_capsule_descriptor(commands, capsule)
    });
    // register_entity only, no register_recipe_only: an effect has no mesh, so
    // there is nothing for the geometry-recipe/rebuild path to produce. Same
    // shape as XrdsWorldPanel/XrdsAudioClip/XrdsText.
    registry.register_entity::<XrdsEffect, _>(|effect, commands, _asset_server| {
        spawn_effect_descriptor(commands, effect)
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
    registry.register_entity::<XrdsWorldPanel, _>(|panel, commands, _asset_server| {
        spawn_world_panel_descriptor(commands, panel)
    });
    registry.register_entity::<XrdsAudioClip, _>(|audio, commands, _asset_server| {
        spawn_audio_clip_descriptor(commands, audio)
    });
    registry.register_entity::<XrdsText, _>(|text, commands, _asset_server| {
        spawn_text_descriptor(commands, text)
    });
    registry.register_entity::<XrdsExtrudedText, _>(|text, commands, _asset_server| {
        spawn_extruded_text_descriptor(commands, text)
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

pub(super) fn register_default_descriptor_cloners(registry: &mut SurfaceDescriptorRegistry) {
    registry.register_clone::<XrdsNode>();
    registry.register_clone::<XrdsCamera>();
    registry.register_clone::<XrdsGltfAsset>();
    registry.register_clone::<XrdsCube>();
    registry.register_clone::<XrdsCylinder>();
    registry.register_clone::<XrdsCapsule>();
    registry.register_clone::<XrdsEffect>();
    registry.register_clone::<XrdsSphere>();
    registry.register_clone::<XrdsPlane3D>();
    registry.register_clone::<XrdsTetrahedron>();
    registry.register_clone::<XrdsPointLight>();
    registry.register_clone::<XrdsDirectionalLight>();
    registry.register_clone::<XrdsSpotLight>();
    registry.register_clone::<XrdsAmbientLight>();
    registry.register_clone::<XrdsAudioClip>();
    registry.register_clone::<XrdsText>();
    registry.register_clone::<XrdsExtrudedText>();
}
