use super::*;
use bevy::audio::{AudioSource, PlaybackMode, PlaybackSettings, Volume};
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
            GlobalTransform::default(),
            build_visibility_hierarchy_components(visible),
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
            build_visibility_hierarchy_components(visible),
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

        // Every XRDS camera acts as the spatial audio listener by default.
        // This matches the convention in Unity, Godot, and Unreal: the active camera
        // defines where the "ears" are. Expert code can remove or replace this component.
        if !entity_mut.contains::<bevy::audio::SpatialListener>() {
            entity_mut.insert(bevy::audio::SpatialListener::default());
        }
    });

    entity
}

pub(super) fn spawn_gltf_descriptor(
    commands: &mut Commands,
    asset: &XrdsGltfAsset,
) -> Option<Entity> {
    // [CRASH-ZONE 1] validate_gltf_source opens and parses the file synchronously.
    // If the path is wrong or the file is missing the spawn is skipped (no entity, no crash).
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
        let (scene_handle, gltf_handle) = {
            let server = world.resource::<AssetServer>();
            let relative_path = super::gltf::relativize_asset_path(&path);
            let asset_path = build_scene_asset_path(&path, scene_index);
            let scene = server.load::<Scene>(asset_path);
            let gltf = server.load::<bevy::gltf::Gltf>(relative_path);
            (scene, gltf)
        };
        world.entity_mut(entity).insert((
            Name::new(name.clone()),
            SceneRoot(scene_handle),
            XrdsStoredGltfHandle(gltf_handle),
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
            build_visibility_hierarchy_components(visible),
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
            build_visibility_hierarchy_components(visible),
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
            build_visibility_hierarchy_components(visible),
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
            build_visibility_hierarchy_components(visible),
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
    let affects_baked_lighting = light.affects_baked_lighting;

    commands.queue(move |world: &mut World| {
        world.insert_resource(AmbientLight {
            color: color.into(),
            brightness,
            affects_lightmapped_meshes: affects_baked_lighting,
        });
        world.entity_mut(entity).insert((
            Name::new(name),
            build_transform(&transform),
            build_visibility_hierarchy_components(visible),
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
            build_visibility_hierarchy_components(visible),
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
            build_visibility_hierarchy_components(visible),
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
            build_visibility_hierarchy_components(visible),
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

pub(super) fn spawn_audio_clip_descriptor(
    commands: &mut Commands,
    audio: &XrdsAudioClip,
) -> Entity {
    let entity = commands.spawn_empty().id();
    let descriptor = audio.clone();
    let name = audio.name.clone();
    let transform = audio.transform;
    let visible = audio.visible;
    let asset_id = audio.audio_asset_id.clone();
    let volume = audio.volume;
    let looped = audio.looped;
    let spatial = audio.spatial;
    let autoplay = audio.autoplay;

    commands.queue(move |world: &mut World| {
        // Resolve catalog asset id → URI, then load the audio source.
        let audio_uri = world
            .get_resource::<XrdsImportedAssetCatalog>()
            .and_then(|catalog| {
                catalog
                    .assets
                    .iter()
                    .find(|a| a.id == asset_id && a.kind == XrdsSceneAssetKind::Audio)
                    .map(|a| a.uri.clone())
            });

        let playback = PlaybackSettings {
            mode: if looped {
                PlaybackMode::Loop
            } else {
                PlaybackMode::Once
            },
            volume: Volume::Linear(volume.clamp(0.0, 1.0)),
            paused: !autoplay,
            spatial,
            ..PlaybackSettings::ONCE
        };

        let mut entity_mut = world.entity_mut(entity);
        entity_mut.insert((
            Name::new(name),
            build_transform(&transform),
            GlobalTransform::default(),
            build_visibility_hierarchy_components(visible),
            XrdsStored(descriptor),
        ));

        // Only attempt to load audio if AudioSource has been registered with the asset
        // server (i.e. AudioPlugin is present). In test environments that use minimal
        // plugins this guard avoids a panic from an unregistered asset type.
        let audio_registered = world.contains_resource::<Assets<AudioSource>>();
        if let (Some(uri), true) = (audio_uri, audio_registered) {
            let handle = world
                .resource::<AssetServer>()
                .load::<AudioSource>(uri.clone());
            // AudioPlayer is intentionally NOT inserted here.
            // `pre_validate_audio_decoders_system` will insert it once the asset has
            // loaded and the decoder has been verified with catch_unwind. This prevents
            // Bevy's observer-based audio system from panicking on unrecognised formats
            // before we can intercept.
            world.entity_mut(entity).insert(XrdsStoredAudioHandle {
                handle,
                uri,
                playback,
            });
        }
    });

    entity
}

// TODO: XrdsText currently uses Text2d which requires Camera2d to render. In apps that only
// have Camera3d (including the editor), text is invisible at runtime. The proper fix is a
// billboard mesh with a dynamically-generated text texture so it works with any camera.
// Until then, the editor uses an egui overlay to approximate text label rendering.
pub(super) fn spawn_text_descriptor(commands: &mut Commands, text: &XrdsText) -> Entity {
    use bevy::text::{TextColor, TextFont, TextLayout};

    let entity = commands.spawn_empty().id();
    let descriptor = text.clone();
    let name = text.name.clone();
    let transform = text.transform;
    let visible = text.visible;
    let content = text.text.clone();
    let font_size = text.font_size;
    let [r, g, b, a] = text.color;
    let justify = match text.alignment {
        XrdsTextAlignment::Left => Justify::Left,
        XrdsTextAlignment::Center => Justify::Center,
        XrdsTextAlignment::Right => Justify::Right,
    };

    commands.queue(move |world: &mut World| {
        world.entity_mut(entity).insert((
            Name::new(name),
            Text2d::new(content),
            TextFont { font_size, ..Default::default() },
            TextColor(bevy::color::Color::srgba(r, g, b, a)),
            TextLayout::new_with_justify(justify),
            build_transform(&transform),
            build_visibility_hierarchy_components(visible),
            XrdsStored(descriptor),
        ));
    });

    entity
}

pub(super) fn build_scene_asset_path(path: &str, scene_index: usize) -> String {
    // Relativize absolute paths (e.g. those stored when scene is unsaved)
    // so Bevy's AssetServer receives a path relative to its `assets/` root.
    let path = super::gltf::relativize_asset_path(path);
    if path.contains('#') {
        path
    } else {
        format!("{path}#Scene{scene_index}")
    }
}
