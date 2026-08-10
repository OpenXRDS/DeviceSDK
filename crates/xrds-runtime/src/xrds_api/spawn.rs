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

        use bevy::camera::visibility::NoFrustumCulling;
        world.entity_mut(entity).insert((
            Name::new(name),
            Mesh3d(mesh),
            build_transform(&transform),
            build_visibility_hierarchy_components(visible),
            XrdsStored(descriptor),
            NoFrustumCulling,
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

        use bevy::camera::visibility::NoFrustumCulling;
        world.entity_mut(entity).insert((
            Name::new(name),
            Mesh3d(mesh),
            build_transform(&transform),
            build_visibility_hierarchy_components(visible),
            XrdsStored(descriptor),
            NoFrustumCulling,
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

        use bevy::camera::visibility::NoFrustumCulling;
        world.entity_mut(entity).insert((
            Name::new(name),
            Mesh3d(mesh),
            build_transform(&transform),
            build_visibility_hierarchy_components(visible),
            XrdsStored(descriptor),
            NoFrustumCulling,
        ));
        apply_authored_material_to_entity(world, entity, material);
        insert_physics_components(world, entity, physics_body, gravity_scale, mass, |_| {
            avian3d::prelude::Collider::sphere(radius)
        }, [0.0f32; 1]);
    });

    entity
}

pub(super) fn spawn_world_panel_descriptor(commands: &mut Commands, panel: &XrdsWorldPanel) -> Entity {
    let entity    = commands.spawn_empty().id();
    let descriptor = panel.clone();
    let name      = panel.name.clone();
    let transform = panel.transform;
    let visible   = panel.visible;
    let size      = panel.size;
    let color     = panel.color;
    let opacity   = panel.opacity;

    commands.queue(move |world: &mut World| {
        // Flat quad mesh on the XY plane; normals point local +Z (front face).
        let mesh = {
            let mut meshes = world.resource_mut::<Assets<Mesh>>();
            meshes.add(Mesh::from(bevy::math::primitives::Rectangle::new(size[0], size[1])))
        };

        let effective_alpha = color[3] * opacity;
        let material = XrdsMaterialParams {
            base_color: XrdsColor { rgba: [color[0], color[1], color[2], effective_alpha] },
            unlit: true,
            ..XrdsMaterialParams::default()
        };

        use bevy::camera::visibility::NoFrustumCulling;
        world.entity_mut(entity).insert((
            Name::new(name),
            Mesh3d(mesh),
            build_transform(&transform),
            build_visibility_hierarchy_components(visible),
            XrdsStored(descriptor),
            XrdsWorldSurface::new(size[0], size[1]),
            NoFrustumCulling,
        ));
        apply_authored_material_to_entity(world, entity, material);
    });

    entity
}

/// Gives an already-spawned `Panel` node entity its backdrop and pointer surface.
///
/// **Without this a panel's buttons cannot be pressed at all.** `world_ui_button_system`
/// requires two things of an element's parent: that the pointer hit *it*
/// (`hit.entity == panel_entity`), and that it carry [`XrdsWorldSurface`], whose
/// `size` converts the hit's UV into panel-local metres. A `Panel` node is spawned
/// as a bare `XrdsNode` — no mesh, so `find_nearest_surface` can never hit it, and
/// no surface, so even a hit could not be mapped. Elements rendered, hover never
/// fired, and every authored element trigger was unreachable in a running scene.
///
/// It also renders the template's `size`/`background`, which nothing drew before —
/// so a placed panel showed its elements floating with no panel behind them.
///
/// Applied to the *node's own* entity rather than a child, deliberately: elements
/// are parented to that entity and the button system reads `ChildOf` to find the
/// panel, so a separate backdrop child would put the surface one level away from
/// where every element looks for it.
///
/// Mirrors `spawn_world_panel_descriptor`'s recipe (quad on XY, unlit, alpha ×
/// opacity, `NoFrustumCulling`) rather than sharing code with it: that one builds a
/// whole entity from a descriptor, this one adorns an existing one.
pub(super) fn apply_panel_backdrop_in_world(
    world: &mut World,
    entity: Entity,
    template: &xrds_scene_graph::XrdsPanelTemplate,
) {
    use bevy::camera::visibility::NoFrustumCulling;

    let [w, h] = template.size;
    // A zero-sized panel would produce a degenerate mesh and a surface no ray can
    // hit; skip rather than spawn something invisible and unhittable.
    if !(w > 0.0 && h > 0.0) {
        log::warn!(
            "Panel template {:?} has size {w}×{h}; no backdrop or pointer surface spawned.",
            template.name
        );
        return;
    }

    let mesh = {
        let mut meshes = world.resource_mut::<Assets<Mesh>>();
        meshes.add(Mesh::from(bevy::math::primitives::Rectangle::new(w, h)))
    };

    let c = template.background.color;
    let material = XrdsMaterialParams {
        base_color: XrdsColor { rgba: [c[0], c[1], c[2], c[3] * template.background.opacity] },
        unlit: true,
        ..XrdsMaterialParams::default()
    };

    if let Ok(mut e) = world.get_entity_mut(entity) {
        e.insert((Mesh3d(mesh), XrdsWorldSurface::new(w, h), NoFrustumCulling));
    } else {
        return;
    }
    apply_authored_material_to_entity(world, entity, material);
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

        use bevy::camera::visibility::NoFrustumCulling;
        world.entity_mut(entity).insert((
            Name::new(name),
            Mesh3d(mesh),
            build_transform(&transform),
            build_visibility_hierarchy_components(visible),
            XrdsStored(descriptor),
            NoFrustumCulling,
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
            bevy::light::NotShadowCaster,
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

/// Instantiates a panel template head-locked to `anchor_entity` — the camera
/// half of "attachment is the only difference".
///
/// Exactly the same elements a scene-placed `Panel` node would spawn; only the
/// placement differs. Each element goes through `spawn_panel_element_in_world`,
/// so its authored triggers land on its entity and fire like any other binding —
/// which means a HUD can now carry buttons and sliders, not just text.
///
/// **Returns [`super::state::XrdsStoredHudInstance`] deliberately.** That is the
/// same component `set_hud_item` already resolves against, keyed by element
/// name, so a public API that predates all of this keeps working unchanged
/// against a migrated template. Preserving it was the cheapest part of the
/// migration precisely because both models address by name.
///
/// `depth` comes from the *anchor*, not the template (see
/// `XrdsScenePlayerAnchor::panel_depth`): that is what lets one template be
/// instanced at two different depths, which `XrdsHudTemplate::depth` could not.
///
/// `element_triggers` is the wiring for this attachment, keyed by element name —
/// the same map a scene-placed `Panel` node carries. The anchor-link path has
/// nowhere to store one and passes an empty map, which is exactly the asymmetry
/// §A6-2 removes by making a head-locked panel a `Panel` node parented under the
/// anchor rather than a field on it.
pub(super) fn spawn_panel_template_head_locked(
    world: &mut World,
    anchor_entity: Entity,
    template: &xrds_scene_graph::XrdsPanelTemplate,
    depth: f32,
    element_triggers: &std::collections::BTreeMap<String, Vec<xrds_scene_graph::XrdsTriggerBinding>>,
) -> super::state::XrdsStoredHudInstance {
    use crate::xrds_api::anchor::XrdsHeadLocked;

    let mut items: Vec<(String, Entity)> = Vec::new();

    for element in &template.elements {
        let entity = crate::xrds_api::trigger_action::spawn_panel_element_in_world(
            world,
            anchor_entity,
            element,
            element_triggers.get(&element.name).map_or(&[], Vec::as_slice),
        );

        // The element spawned at its canvas position on a panel plane; the
        // attachment decides where that plane sits. Camera-local space is X
        // right, Y up, -Z forward, so the canvas lands `depth` metres ahead.
        let [x, y] = element.local_position();
        let local_offset = Transform::from_translation(Vec3::new(x, y, -depth));
        if let Ok(mut e) = world.get_entity_mut(entity) {
            e.insert((local_offset, XrdsHeadLocked { local_offset }));
        }

        items.push((element.name.clone(), entity));
    }

    super::state::XrdsStoredHudInstance { items }
}

// ── World-space widget spawn functions ────────────────────────────────────────

/// Runtime component that caches the three pre-created material handles for a button so
/// the button system can swap colours without rebuilding assets each frame.
#[derive(bevy::prelude::Component)]
pub(super) struct XrdsWorldButtonMaterials {
    pub normal:  bevy::prelude::Handle<StandardMaterial>,
    pub hover:   bevy::prelude::Handle<StandardMaterial>,
    pub pressed: bevy::prelude::Handle<StandardMaterial>,
}

/// Spawn a world-space label as a child of `panel_entity`.
pub(super) fn spawn_world_label_entity(
    world: &mut World,
    panel_entity: Entity,
    params: &xrds_components::XrdsWorldLabelParams,
) -> Entity {
    use bevy::render::alpha::AlphaMode;
    use bevy_rich_text3d::{Text3d, Text3dStyling, TextAtlas};
    use bevy::color::Srgba;
    use bevy::camera::visibility::NoFrustumCulling;
    use bevy::light::NotShadowCaster;

    let [lx, ly]  = params.local_position;
    let [r, g, b, a] = params.color;
    let font_size    = params.font_size;
    let text         = params.text.clone();

    let entity = world.spawn_empty().id();

    let material_handle = {
        let mut materials = world.resource_mut::<Assets<StandardMaterial>>();
        materials.add(StandardMaterial {
            base_color_texture: Some(TextAtlas::DEFAULT_IMAGE.clone()),
            alpha_mode: AlphaMode::Mask(0.5),
            unlit: true,
            cull_mode: None,
            ..Default::default()
        })
    };

    world.entity_mut(entity).insert((
        bevy::prelude::Name::new(format!("WLabel:{text}")),
        Text3d::new(text),
        Text3dStyling {
            size: 128.0,
            world_scale: Some(bevy::math::Vec2::splat(font_size * 0.01)),
            color: Srgba::new(r, g, b, a),
            ..Default::default()
        },
        Mesh3d::default(),
        MeshMaterial3d(material_handle),
        Transform::from_xyz(lx, ly, 0.001),
        build_visibility_hierarchy_components(true),
        xrds_components::XrdsWorldLabel { local_position: [lx, ly], layout_size: params.layout_size },
        ChildOf(panel_entity),
        NotShadowCaster,
        NoFrustumCulling,
    ));

    entity
}

/// Spawn a world-space button (background quad + text child) as a child of `panel_entity`.
pub(super) fn spawn_world_button_entity(
    world: &mut World,
    panel_entity: Entity,
    params: &xrds_components::XrdsWorldButtonParams,
) -> Entity {
    use bevy::render::alpha::AlphaMode;
    use bevy_rich_text3d::{Text3d, Text3dStyling, TextAtlas};
    use bevy::color::{Color, Srgba};
    use bevy::camera::visibility::NoFrustumCulling;
    use bevy::light::NotShadowCaster;

    let [lx, ly]        = params.local_position;
    let [bw, bh]        = params.size;
    let [nr, ng, nb, na] = params.normal_color;
    let [hr, hg, hb, ha] = params.hover_color;
    let [pr, pg, pb, pa] = params.pressed_color;
    let [lr, lg, lb, la] = params.label_color;
    let font_size        = params.font_size;
    let label_text       = params.label.clone();

    // — Button background entity —
    let button_entity = world.spawn_empty().id();

    let (mesh, normal_mat, hover_mat, pressed_mat) = {
        let mesh = {
            let mut meshes = world.resource_mut::<Assets<Mesh>>();
            meshes.add(Mesh::from(bevy::math::primitives::Rectangle::new(bw, bh)))
        };
        let mut mats = world.resource_mut::<Assets<StandardMaterial>>();
        let nm = mats.add(StandardMaterial {
            base_color: Color::srgba(nr, ng, nb, na),
            unlit: true,
            alpha_mode: AlphaMode::Blend,
            ..Default::default()
        });
        let hm = mats.add(StandardMaterial {
            base_color: Color::srgba(hr, hg, hb, ha),
            unlit: true,
            alpha_mode: AlphaMode::Blend,
            ..Default::default()
        });
        let pm = mats.add(StandardMaterial {
            base_color: Color::srgba(pr, pg, pb, pa),
            unlit: true,
            alpha_mode: AlphaMode::Blend,
            ..Default::default()
        });
        (mesh, nm, hm, pm)
    };

    world.entity_mut(button_entity).insert((
        bevy::prelude::Name::new(format!("WButton:{label_text}")),
        Mesh3d(mesh),
        MeshMaterial3d(normal_mat.clone()),
        Transform::from_xyz(lx, ly, 0.001),
        build_visibility_hierarchy_components(true),
        xrds_components::XrdsWorldButton {
            local_position: [lx, ly],
            size: [bw, bh],
            normal_color:  params.normal_color,
            hover_color:   params.hover_color,
            pressed_color: params.pressed_color,
        },
        xrds_components::XrdsWorldButtonState::default(),
        XrdsWorldButtonMaterials { normal: normal_mat, hover: hover_mat, pressed: pressed_mat },
        ChildOf(panel_entity),
        NoFrustumCulling,
    ));

    // — Label text child —
    let text_entity = world.spawn_empty().id();

    let text_mat = {
        let mut materials = world.resource_mut::<Assets<StandardMaterial>>();
        materials.add(StandardMaterial {
            base_color_texture: Some(TextAtlas::DEFAULT_IMAGE.clone()),
            alpha_mode: AlphaMode::Mask(0.5),
            unlit: true,
            cull_mode: None,
            ..Default::default()
        })
    };

    world.entity_mut(text_entity).insert((
        bevy::prelude::Name::new(format!("WButton_Text:{label_text}")),
        Text3d::new(label_text),
        Text3dStyling {
            size: 128.0,
            world_scale: Some(bevy::math::Vec2::splat(font_size * 0.01)),
            color: Srgba::new(lr, lg, lb, la),
            ..Default::default()
        },
        Mesh3d::default(),
        MeshMaterial3d(text_mat),
        Transform::from_xyz(0.0, 0.0, 0.001),
        build_visibility_hierarchy_components(true),
        ChildOf(button_entity),
        NotShadowCaster,
        NoFrustumCulling,
    ));

    button_entity
}

/// Spawn a world-space image (textured quad) as a child of `panel_entity`.
pub(super) fn spawn_world_image_entity(
    world: &mut World,
    panel_entity: Entity,
    params: &xrds_components::XrdsWorldImageParams,
) -> Entity {
    use bevy::render::alpha::AlphaMode;
    use bevy::color::Color;
    use bevy::camera::visibility::NoFrustumCulling;

    let [lx, ly]        = params.local_position;
    let [iw, ih]        = params.size;
    let [tr, tg, tb, ta] = params.tint;
    let asset_path      = params.asset_path.clone();

    let entity = world.spawn_empty().id();

    let (mesh, material_handle) = {
        let mesh = {
            let mut meshes = world.resource_mut::<Assets<Mesh>>();
            meshes.add(Mesh::from(bevy::math::primitives::Rectangle::new(iw, ih)))
        };
        let texture = world
            .resource::<bevy::asset::AssetServer>()
            .load(asset_path);
        let mat = {
            let mut materials = world.resource_mut::<Assets<StandardMaterial>>();
            materials.add(StandardMaterial {
                base_color_texture: Some(texture),
                base_color: Color::srgba(tr, tg, tb, ta),
                unlit: true,
                alpha_mode: AlphaMode::Blend,
                ..Default::default()
            })
        };
        (mesh, mat)
    };

    world.entity_mut(entity).insert((
        bevy::prelude::Name::new("WImage"),
        Mesh3d(mesh),
        MeshMaterial3d(material_handle),
        Transform::from_xyz(lx, ly, 0.001),
        build_visibility_hierarchy_components(true),
        xrds_components::XrdsWorldImage { local_position: [lx, ly], size: [iw, ih] },
        ChildOf(panel_entity),
        NoFrustumCulling,
    ));

    entity
}

/// Spawn a world-space slider (track quad + thumb quad) as a child of `panel_entity`.
pub(super) fn spawn_world_slider_entity(
    world: &mut World,
    panel_entity: Entity,
    params: &xrds_components::XrdsWorldSliderParams,
) -> Entity {
    use bevy::render::alpha::AlphaMode;
    use bevy::color::Color;
    use bevy::camera::visibility::NoFrustumCulling;
    use super::world_ui_slider::XrdsWorldSliderParts;

    let [lx, ly]         = params.local_position;
    let [tw, th]         = params.size;
    let [trr, trg, trb, tra] = params.track_color;
    let [tmr, tmg, tmb, tma] = params.thumb_color;
    let ts               = params.thumb_size;

    // Root entity — invisible transform anchor.
    let root = world.spawn_empty().id();

    // Track mesh + material.
    let (track_mesh, track_mat) = {
        let mesh = world.resource_mut::<Assets<Mesh>>()
            .add(Mesh::from(bevy::math::primitives::Rectangle::new(tw, th)));
        let mat = world.resource_mut::<Assets<StandardMaterial>>()
            .add(StandardMaterial {
                base_color: Color::srgba(trr, trg, trb, tra),
                unlit: true,
                alpha_mode: AlphaMode::Blend,
                ..Default::default()
            });
        (mesh, mat)
    };

    // Thumb mesh + material.
    let (thumb_mesh, thumb_mat) = {
        let mesh = world.resource_mut::<Assets<Mesh>>()
            .add(Mesh::from(bevy::math::primitives::Rectangle::new(ts, ts)));
        let mat = world.resource_mut::<Assets<StandardMaterial>>()
            .add(StandardMaterial {
                base_color: Color::srgba(tmr, tmg, tmb, tma),
                unlit: true,
                alpha_mode: AlphaMode::Blend,
                ..Default::default()
            });
        (mesh, mat)
    };

    // Track entity.
    let track_entity = world.spawn_empty().id();
    world.entity_mut(track_entity).insert((
        bevy::prelude::Name::new("WSlider_Track"),
        Mesh3d(track_mesh),
        MeshMaterial3d(track_mat),
        Transform::from_xyz(0.0, 0.0, 0.0),
        build_visibility_hierarchy_components(true),
        ChildOf(root),
        NoFrustumCulling,
    ));

    // Thumb entity — initial X position from value.
    let slider = xrds_components::XrdsWorldSlider {
        local_position: params.local_position,
        size:          params.size,
        min:           params.min,
        max:           params.max,
        value:         params.value,
        track_color:   params.track_color,
        fill_color:    params.fill_color,
        thumb_color:   params.thumb_color,
        thumb_size:    params.thumb_size,
        dragging_hand: None,
    };
    let thumb_x = slider.thumb_x();

    let thumb_entity = world.spawn_empty().id();
    world.entity_mut(thumb_entity).insert((
        bevy::prelude::Name::new("WSlider_Thumb"),
        Mesh3d(thumb_mesh),
        MeshMaterial3d(thumb_mat),
        Transform::from_xyz(thumb_x, 0.0, 0.001),
        build_visibility_hierarchy_components(true),
        ChildOf(root),
        NoFrustumCulling,
    ));

    // Root entity.
    world.entity_mut(root).insert((
        bevy::prelude::Name::new("WSlider"),
        Transform::from_xyz(lx, ly, 0.001),
        build_visibility_hierarchy_components(true),
        slider,
        XrdsWorldSliderParts { thumb: thumb_entity },
        ChildOf(panel_entity),
        NoFrustumCulling,
    ));

    root
}

/// Spawn a world-space toggle (track quad + thumb quad) as a child of `panel_entity`.
pub(super) fn spawn_world_toggle_entity(
    world: &mut World,
    panel_entity: Entity,
    params: &xrds_components::XrdsWorldToggleParams,
) -> Entity {
    use bevy::render::alpha::AlphaMode;
    use bevy::color::Color;
    use bevy::camera::visibility::NoFrustumCulling;
    use super::world_ui_toggle::XrdsWorldToggleParts;

    let [lx, ly]           = params.local_position;
    let [tw, th]           = params.size;
    let [or_, og, ob, oa]  = params.track_off_color;
    let [nr, ng, nb, na]   = params.track_on_color;
    let [tmr, tmg, tmb, tma] = params.thumb_color;
    let thumb_side = th * 0.85;

    // Single track material — colour updated in-place at runtime via Assets<StandardMaterial>.
    let initial_color = if params.checked {
        Color::srgba(nr, ng, nb, na)
    } else {
        Color::srgba(or_, og, ob, oa)
    };
    let track_mat = world.resource_mut::<Assets<StandardMaterial>>()
        .add(StandardMaterial {
            base_color: initial_color,
            unlit: true,
            alpha_mode: AlphaMode::Blend,
            ..Default::default()
        });

    let track_mesh = world.resource_mut::<Assets<Mesh>>()
        .add(Mesh::from(bevy::math::primitives::Rectangle::new(tw, th)));
    let thumb_mesh = world.resource_mut::<Assets<Mesh>>()
        .add(Mesh::from(bevy::math::primitives::Rectangle::new(thumb_side, thumb_side)));
    let thumb_mat = world.resource_mut::<Assets<StandardMaterial>>()
        .add(StandardMaterial {
            base_color: Color::srgba(tmr, tmg, tmb, tma),
            unlit: true,
            alpha_mode: AlphaMode::Blend,
            ..Default::default()
        });

    let root = world.spawn_empty().id();

    // Track entity.
    let track_entity = world.spawn_empty().id();
    world.entity_mut(track_entity).insert((
        bevy::prelude::Name::new("WToggle_Track"),
        Mesh3d(track_mesh),
        MeshMaterial3d(track_mat),
        Transform::from_xyz(0.0, 0.0, 0.0),
        build_visibility_hierarchy_components(true),
        ChildOf(root),
        NoFrustumCulling,
    ));

    // Thumb entity.
    let thumb_entity = world.spawn_empty().id();
    let travel = tw * 0.5 - thumb_side * 0.5;
    let thumb_x = if params.checked { travel } else { -travel };
    world.entity_mut(thumb_entity).insert((
        bevy::prelude::Name::new("WToggle_Thumb"),
        Mesh3d(thumb_mesh),
        MeshMaterial3d(thumb_mat),
        Transform::from_xyz(thumb_x, 0.0, 0.001),
        build_visibility_hierarchy_components(true),
        ChildOf(root),
        NoFrustumCulling,
    ));

    // Root entity.
    world.entity_mut(root).insert((
        bevy::prelude::Name::new("WToggle"),
        Transform::from_xyz(lx, ly, 0.001),
        build_visibility_hierarchy_components(true),
        xrds_components::XrdsWorldToggle {
            local_position: params.local_position,
            size:           params.size,
            checked:        params.checked,
            track_off_color: params.track_off_color,
            track_on_color:  params.track_on_color,
            thumb_color:     params.thumb_color,
        },
        XrdsWorldToggleParts { track: track_entity, thumb: thumb_entity },
        ChildOf(panel_entity),
        NoFrustumCulling,
    ));

    root
}

/// Spawn a single world-UI widget from a serialised [`XrdsSceneWorldWidget`] definition.
///
/// Called by `import_runtime_nodes` when a `WorldPanel` scene node is imported.
/// The widget becomes a direct child of `panel_entity`.
/// Spawn a single world-UI widget from a serialised
/// [`xrds_scene_graph::XrdsSceneWorldWidget`] definition, returning its entity.
///
/// **The return value is load-bearing.** This used to discard it, which is why
/// authored widget triggers could never fire: the four widget trigger kinds
/// target the widget's own entity, `consume_triggers` requires an
/// `XrdsTriggerBindings` component *on that entity*, and with the entity thrown
/// away there was nothing to attach it to. See
/// `crate::xrds_api::trigger_action::spawn_panel_element_in_world`.
pub(super) fn spawn_world_widget_from_scene(
    world: &mut World,
    panel_entity: Entity,
    widget: &xrds_scene_graph::XrdsSceneWorldWidget,
) -> Entity {
    use xrds_components::{
        XrdsWorldButtonParams, XrdsWorldImageParams, XrdsWorldLabelParams,
        XrdsWorldSliderParams, XrdsWorldToggleParams,
    };
    use xrds_scene_graph::XrdsSceneWorldWidget;

    match widget {
        XrdsSceneWorldWidget::Label(l) => {
            spawn_world_label_entity(world, panel_entity, &XrdsWorldLabelParams {
                text:           l.text.clone(),
                font_size:      l.font_size,
                color:          l.color,
                local_position: l.local_position,
                layout_size:    l.layout_size,
            })
        }
        XrdsSceneWorldWidget::Button(b) => {
            spawn_world_button_entity(world, panel_entity, &XrdsWorldButtonParams {
                label:          b.label.clone(),
                font_size:      b.font_size,
                label_color:    b.label_color,
                size:           b.size,
                local_position: b.local_position,
                normal_color:   b.normal_color,
                hover_color:    b.hover_color,
                pressed_color:  b.pressed_color,
            })
        }
        XrdsSceneWorldWidget::Image(i) => {
            spawn_world_image_entity(world, panel_entity, &XrdsWorldImageParams {
                asset_path:     i.asset_path.clone(),
                size:           i.size,
                local_position: i.local_position,
                tint:           i.tint,
            })
        }
        XrdsSceneWorldWidget::Slider(s) => {
            spawn_world_slider_entity(world, panel_entity, &XrdsWorldSliderParams {
                min:            s.min,
                max:            s.max,
                value:          s.value,
                size:           s.size,
                local_position: s.local_position,
                track_color:    s.track_color,
                fill_color:     s.fill_color,
                thumb_color:    s.thumb_color,
                thumb_size:     s.thumb_size,
            })
        }
        XrdsSceneWorldWidget::Toggle(t) => {
            spawn_world_toggle_entity(world, panel_entity, &XrdsWorldToggleParams {
                checked:         t.checked,
                size:            t.size,
                local_position:  t.local_position,
                track_off_color: t.track_off_color,
                track_on_color:  t.track_on_color,
                thumb_color:     t.thumb_color,
            })
        }
    }
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
