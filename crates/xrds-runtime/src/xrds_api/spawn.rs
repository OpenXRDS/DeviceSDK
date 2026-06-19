use super::*;
use avian3d::prelude::{Collider, RigidBody};
use bevy::audio::{AudioSource, PlaybackMode, PlaybackSettings, Volume};
use bevy::core_pipeline::core_3d::graph::Core3d;
use bevy::core_pipeline::tonemapping::Tonemapping;
use bevy::post_process::bloom::Bloom;
use bevy::render::camera::CameraRenderGraph;

fn insert_physics_components<T, F>(
    world: &mut World,
    entity: Entity,
    physics_body: XrdsPhysicsBody,
    gravity_scale: f32,
    mass: f32,
    make_collider: F,
    params: T,
) where
    F: FnOnce(T) -> Collider,
{
    match physics_body {
        XrdsPhysicsBody::None => {}
        XrdsPhysicsBody::Static => {
            world.entity_mut(entity).insert((RigidBody::Static, make_collider(params)));
        }
        XrdsPhysicsBody::Dynamic => {
            // SweptCcd prevents tunneling for fast-moving bodies (especially boxes whose
            // corner-based contact detection is less robust than sphere's center check).
            world.entity_mut(entity).insert((
                RigidBody::Dynamic,
                make_collider(params),
                avian3d::prelude::SweptCcd::default(),
                avian3d::prelude::GravityScale(gravity_scale),
                avian3d::prelude::Mass(mass),
            ));
        }
    }
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
    let physics_body = cube.physics_body;
    let gravity_scale = cube.gravity_scale;
    let mass = cube.mass;
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
        insert_physics_components(world, entity, physics_body, gravity_scale, mass, |pb| {
            avian3d::prelude::Collider::cuboid(pb[0] / 2.0, pb[1] / 2.0, pb[2] / 2.0)
        }, size);
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
    let physics_body = cylinder.physics_body;
    let gravity_scale = cylinder.gravity_scale;
    let mass = cylinder.mass;
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
        // Avian3d cylinder: radius, half_height
        insert_physics_components(world, entity, physics_body, gravity_scale, mass, |_| {
            avian3d::prelude::Collider::cylinder(radius, height / 2.0)
        }, [0.0f32; 1]);
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
    let physics_body = sphere.physics_body;
    let gravity_scale = sphere.gravity_scale;
    let mass = sphere.mass;
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
        insert_physics_components(world, entity, physics_body, gravity_scale, mass, |_| {
            avian3d::prelude::Collider::sphere(radius)
        }, [0.0f32; 1]);
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
    let physics_body = plane.physics_body;
    let gravity_scale = plane.gravity_scale;
    let mass = plane.mass;
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
        // Half-space: infinite solid below the plane's surface — no tunneling, perfect alignment.
        insert_physics_components(world, entity, physics_body, gravity_scale, mass, |_| {
            avian3d::prelude::Collider::half_space(Vec3::Y)
        }, size);
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

pub(super) fn spawn_text_descriptor(commands: &mut Commands, text: &XrdsText) -> Entity {
    use bevy::color::Srgba;
    use bevy::pbr::StandardMaterial;
    use bevy::render::alpha::AlphaMode;
    use bevy_rich_text3d::{Text3d, Text3dStyling, TextAlign, TextAtlas};
    use crate::xrds_api::anchor::{
        XrdsBodyLocked, XrdsComfortPinned, XrdsCylindrical, XrdsHeadLocked,
    };
    use crate::XrdsBillboard;

    let entity        = commands.spawn_empty().id();
    let descriptor    = text.clone();
    let name          = text.name.clone();
    let transform     = text.transform;
    let visible       = text.visible;
    let content       = text.text.clone();
    let font_size     = text.font_size;
    let [r, g, b, a]  = text.color;
    let anchor        = text.anchor;
    let local_offset  = build_transform(&text.transform);
    let text_align    = match text.alignment {
        XrdsTextAlignment::Left   => TextAlign::Left,
        XrdsTextAlignment::Center => TextAlign::Center,
        XrdsTextAlignment::Right  => TextAlign::Right,
    };

    commands.queue(move |world: &mut World| {
        let material_handle = {
            let mut materials = world.resource_mut::<bevy::asset::Assets<StandardMaterial>>();
            materials.add(StandardMaterial {
                base_color_texture: Some(TextAtlas::DEFAULT_IMAGE.clone()),
                // Mask(0.5) renders in the opaque pass — reliably visible in XR.
                // AlphaMode::Blend would go through the transparent pass which fails in
                // many XR configurations (stereo depth-sort, swapchain alpha channel).
                // Mask clips sub-threshold alpha at glyph edges (slight aliasing) but
                // the text is always rendered. This matches the official 3D example.
                alpha_mode: AlphaMode::Mask(0.5),
                unlit: true,
                cull_mode: None,
                ..Default::default()
            })
        };
        world.entity_mut(entity).insert((
            Name::new(name),
            Text3d::new(content),
            Text3dStyling {
                // size = rasterization quality in pixels (does not affect world scale).
                // 128px gives sharper glyphs than the 64px in the official 3D example.
                // world_scale = em size in world units; font_size 24 → 0.24 m per em.
                size: 128.0,
                world_scale: Some(bevy::math::Vec2::splat(font_size * 0.01)),
                color: Srgba::new(r, g, b, a),
                align: text_align,
                ..Default::default()
            },
            bevy::prelude::Mesh3d::default(),
            bevy::prelude::MeshMaterial3d(material_handle),
            build_transform(&transform),
            build_visibility_hierarchy_components(visible),
            XrdsStored(descriptor),
        ));
        // Insert the anchor marker after all base components are present.
        // NoFrustumCulling bypasses Bevy's AABB-based frustum culling, which would
        // otherwise cull these entities at their authored position before the anchor
        // system relocates them to the correct HUD position each frame.
        use bevy::camera::visibility::NoFrustumCulling;
        match anchor {
            XrdsTextAnchor::Billboard                  => { world.entity_mut(entity).insert(XrdsBillboard); }
            XrdsTextAnchor::HeadLocked                 => { world.entity_mut(entity).insert((XrdsHeadLocked { local_offset }, NoFrustumCulling)); }
            XrdsTextAnchor::BodyLocked                 => { world.entity_mut(entity).insert((XrdsBodyLocked { local_offset }, NoFrustumCulling)); }
            XrdsTextAnchor::ComfortPinned { depth_m }  => { world.entity_mut(entity).insert((XrdsComfortPinned { depth_m, local_offset }, NoFrustumCulling)); }
            XrdsTextAnchor::Cylindrical  { radius_m }  => { world.entity_mut(entity).insert((XrdsCylindrical  { radius_m, local_offset }, NoFrustumCulling)); }
            XrdsTextAnchor::World                      => {}
        }
    });

    entity
}

pub(super) fn spawn_extruded_text_descriptor(
    commands: &mut Commands,
    text: &XrdsExtrudedText,
) -> Entity {
    use bevy::color::Color;
    use bevy_fontmesh::prelude::{FontMesh, JustifyText, TextAnchor, TextMesh, TextMeshStyle};

    let entity = commands.spawn_empty().id();
    let descriptor = text.clone();
    let name = text.name.clone();
    // Apply font_size as uniform scale: bevy_fontmesh generates 1 em ≈ 1 world unit.
    // font_size 24 → scale 0.24 m/em (matches flat text world_scale formula).
    let mut transform = text.transform;
    let scale_factor = text.font_size * 0.01;
    transform.scale = [
        transform.scale[0] * scale_factor,
        transform.scale[1] * scale_factor,
        transform.scale[2] * scale_factor,
    ];
    let visible = text.visible;
    let content = text.text.clone();
    let [r, g, b, _a] = text.color;
    let depth = text.depth;
    let justify = match text.alignment {
        XrdsExtrudedTextAlignment::Left => JustifyText::Left,
        XrdsExtrudedTextAlignment::Center => JustifyText::Center,
        XrdsExtrudedTextAlignment::Right => JustifyText::Right,
    };

    commands.queue(move |world: &mut World| {
        let font_handle: bevy::asset::Handle<FontMesh> = world
            .resource::<bevy::asset::AssetServer>()
            .load("fonts/NotoSans-Regular.ttf");

        let material_handle = {
            let mut materials =
                world.resource_mut::<bevy::asset::Assets<bevy::pbr::StandardMaterial>>();
            materials.add(bevy::pbr::StandardMaterial {
                base_color: Color::srgb(r, g, b),
                ..Default::default()
            })
        };

        world.entity_mut(entity).insert((
            bevy::prelude::Name::new(name),
            TextMesh {
                text: content,
                font: font_handle,
                style: TextMeshStyle {
                    depth,
                    anchor: TextAnchor::Center,
                    justify,
                    ..Default::default()
                },
            },
            bevy::prelude::Mesh3d::default(),
            bevy::prelude::MeshMaterial3d(material_handle),
            build_transform(&transform),
            build_visibility_hierarchy_components(visible),
            XrdsStored(descriptor),
        ));
    });

    entity
}

/// Spawn an interaction zone as an avian3d sensor collider, registering it in the XRDS id index.
/// No mesh is created — the zone is purely a trigger volume.
pub(super) fn spawn_interaction_zone_entity(
    world: &mut World,
    id: XrdsId,
    node: &xrds_components::world::XrdsNode,
    zone: &xrds_components::XrdsInteractionZone,
) -> Entity {
    use avian3d::prelude::{CollisionEventsEnabled, Sensor};

    let collider = match zone.shape {
        xrds_components::XrdsInteractionZoneShape::Sphere { radius } => {
            Collider::sphere(radius)
        }
        xrds_components::XrdsInteractionZoneShape::Box { half_extents: [hx, hy, hz] } => {
            Collider::cuboid(hx * 2.0, hy * 2.0, hz * 2.0)
        }
    };

    let entity = world
        .spawn((
            bevy::prelude::Name::new(node.name.clone()),
            build_transform(&node.transform),
            build_visibility_hierarchy_components(node.visible),
            collider,
            Sensor,
            CollisionEventsEnabled,
            *zone,
        ))
        .id();

    world.resource_mut::<XrdsIdIndex>().register(id, entity);
    world.resource_mut::<XrdsHierarchyIndex>().ensure_node(id);

    entity
}

/// Spawn 3D text entities for every item in `template` and parent them to
/// `anchor_entity`.  Returns the `XrdsStoredHudInstance` to be inserted on the
/// anchor.  Intended to be called from `tag_player_anchor_entities` immediately
/// after the anchor is tagged so that item entities exist before the first frame.
pub(super) fn spawn_hud_instance_for_anchor(
    world: &mut World,
    anchor_entity: Entity,
    template: &xrds_scene_graph::XrdsHudTemplate,
) -> super::state::XrdsStoredHudInstance {
    use bevy::camera::visibility::NoFrustumCulling;
    use bevy::color::Srgba;
    use bevy::pbr::StandardMaterial;
    use bevy::render::alpha::AlphaMode;
    use bevy_rich_text3d::{Text3d, Text3dStyling, TextAlign, TextAtlas};
    use crate::xrds_api::anchor::XrdsHeadLocked;

    let depth = template.depth;

    let material_handle = {
        let mut materials = world.resource_mut::<bevy::asset::Assets<StandardMaterial>>();
        materials.add(StandardMaterial {
            base_color_texture: Some(TextAtlas::DEFAULT_IMAGE.clone()),
            alpha_mode: AlphaMode::Mask(0.5),
            unlit: true,
            cull_mode: None,
            ..Default::default()
        })
    };

    let mut item_pairs: Vec<(String, Entity)> = Vec::new();

    for item in &template.items {
        let [r, g, b, a] = item.color;
        let [ix, iy]     = item.position;
        let font_size    = item.font_size;

        // The head-locked offset places the item in camera-local space:
        // X right, Y up, -Z forward at `depth` metres in front of the lens.
        let local_offset = Transform::from_translation(Vec3::new(ix, iy, -depth));

        let item_entity = world.spawn((
            bevy::prelude::Name::new(item.name.clone()),
            Text3d::new(item.text.clone()),
            Text3dStyling {
                size: 128.0,
                world_scale: Some(bevy::math::Vec2::splat(font_size * 0.01)),
                color: Srgba::new(r, g, b, a),
                align: TextAlign::Center,
                ..Default::default()
            },
            bevy::prelude::Mesh3d::default(),
            bevy::prelude::MeshMaterial3d(material_handle.clone()),
            Transform::from_translation(Vec3::new(ix, iy, -depth)),
            GlobalTransform::default(),
            build_visibility_hierarchy_components(true),
            XrdsHeadLocked { local_offset },
            NoFrustumCulling,
        )).id();

        world.entity_mut(anchor_entity).add_child(item_entity);
        item_pairs.push((item.name.clone(), item_entity));
    }

    super::state::XrdsStoredHudInstance { items: item_pairs }
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
