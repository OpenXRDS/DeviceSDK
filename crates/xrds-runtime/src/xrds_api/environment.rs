use super::*;
use bevy::camera::Exposure;
use bevy::core_pipeline::Skybox;
use bevy::pbr::{DistanceFog, FogFalloff};
use xrds_scene_graph::XrdsSceneEnvironment;

#[derive(Resource, Debug, Clone, Default, PartialEq)]
pub(super) struct XrdsImportedSceneEnvironment(pub(super) Option<XrdsSceneEnvironment>);

/// Asset ids already reported as missing, so each is reported once.
///
/// The environment policy is re-applied every frame — deliberately, since assets
/// can finish loading later — and the missing-asset warnings sat directly in that
/// path, so one unresolvable id produced a warning per frame per camera and buried
/// every other log line. Reported 2026-08-19 as an "infinite loop" of warnings; it
/// was not a loop, just an unthrottled warning in a per-frame system.
///
/// Keyed by id rather than using `warn_once!`, which is per call site: with that, a
/// second asset failing after the first would be silent, which is the failure this
/// warning exists to prevent.
///
/// A process-wide static rather than a resource, because the resolvers take
/// `&World` and threading `&mut World` through four functions to throttle a log
/// line is a poor trade. The consequence is that the set is not cleared between
/// apps in one process — only relevant to tests, which do not assert on warnings.
fn warn_missing_environment_asset_once(world: &World, what: &str, asset_id: &str) {
    use std::sync::{Mutex, OnceLock};

    static REPORTED: OnceLock<Mutex<std::collections::HashSet<String>>> = OnceLock::new();
    let reported = REPORTED.get_or_init(|| Mutex::new(std::collections::HashSet::new()));

    if !reported.lock().unwrap().insert(format!("{what}:{asset_id}")) {
        return;
    }

    // Two very different causes, and naming the wrong one sends people looking in
    // the wrong place. An earlier version of this message asserted the kind was
    // wrong; the actual case was an asset absent from the catalog entirely, and the
    // message duly sent the reader to check kinds that were already correct.
    let wrong_kind = world
        .get_resource::<XrdsImportedAssetCatalog>()
        .map(|catalog| catalog.assets.iter().any(|a| a.id == asset_id))
        .unwrap_or(false);

    if wrong_kind {
        warn!(
            "Scene {what} asset '{asset_id}' is in the runtime catalog but not as an \
             EnvironmentMap. Skybox and IBL need a cubemap: re-import it, and note \
             that a .ktx2 is only treated as an EnvironmentMap when its header \
             reports 6 faces."
        );
    } else {
        warn!(
            "Scene {what} asset '{asset_id}' is not in the runtime asset catalog at \
             all. The document references it, so it was probably imported without the \
             runtime being told — check that the import path triggers a reimport."
        );
    }
}

#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq)]
struct XrdsManagedSceneIblEnvironment;

#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq)]
struct XrdsManagedSceneSkyboxEnvironment;

#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq)]
struct XrdsManagedSceneAtmosphereEnvironment;

#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq)]
struct XrdsManagedSceneExposureEnvironment;

#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq)]
struct XrdsManagedSceneFogEnvironment;

/// Stores the per-anchor exposure override (ev100) set by `apply_anchor_exposure_system`.
/// `None` means no override — the scene-wide exposure applies.
#[derive(Resource, Debug, Clone, Default)]
pub(super) struct XrdsAnchorExposureOverride(pub(super) Option<f32>);

/// Opt-in marker: cameras with this component always receive scene environment
/// settings (fog, exposure, IBL) regardless of whether they were spawned by XRDS.
///
/// Intended for editor cameras or any camera that should respond to scene-level
/// environment policy without being part of the XRDS scene graph.
#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct XrdsReceivesEnvironment;

pub(super) fn store_imported_scene_environment_in_world(
    world: &mut World,
    environment: Option<XrdsSceneEnvironment>,
) {
    world.resource_mut::<XrdsImportedSceneEnvironment>().0 = environment;
}

pub(super) fn imported_scene_environment_in_world(world: &World) -> Option<XrdsSceneEnvironment> {
    world
        .get_resource::<XrdsImportedSceneEnvironment>()
        .and_then(|resource| resource.0.clone())
}

pub(super) fn apply_imported_scene_environment_policy_in_world(world: &mut World) {
    sync_managed_scene_ibl_in_world(world);
    sync_managed_scene_skybox_in_world(world);
    sync_managed_scene_atmosphere_in_world(world);
    sync_managed_scene_exposure_in_world(world);
    sync_managed_scene_fog_in_world(world);
}

/// Applies procedural atmospheric scattering to every 3-D camera.
///
/// Bevy's `Atmosphere` is a post-process over whatever the camera already drew, so
/// it composes with a skybox rather than replacing it — a starry skybox still shows
/// through at night. That is upstream's design, not an accident of ordering here.
///
/// Suppressed under passthrough for the same reason as the skybox: it paints a sky
/// over the transparent clear the passthrough layer needs, so an author would tick
/// Passthrough and get a computed sky instead of the room.
fn sync_managed_scene_atmosphere_in_world(world: &mut World) {
    use bevy::pbr::Atmosphere;

    let wants_atmosphere = imported_scene_environment_in_world(world)
        .and_then(|environment| environment.atmosphere)
        .is_some()
        && !world
            .get_resource::<xrds_openxr::OpenXrPassthroughEnabled>()
            .is_some_and(|e| e.0);

    if !wants_atmosphere {
        let entities: Vec<Entity> = {
            let mut query = world.query_filtered::<
                Entity,
                (With<Camera3d>, With<XrdsManagedSceneAtmosphereEnvironment>),
            >();
            query.iter(world).collect()
        };
        for entity in entities {
            let mut entity_mut = world.entity_mut(entity);
            entity_mut.remove::<Atmosphere>();
            entity_mut.remove::<XrdsManagedSceneAtmosphereEnvironment>();
        }
        return;
    }

    let cameras: Vec<(Entity, bool)> = {
        let mut query = world.query_filtered::<(Entity, Option<&Atmosphere>), With<Camera3d>>();
        query
            .iter(world)
            .map(|(entity, existing)| (entity, existing.is_some()))
            .collect()
    };

    for (entity, has_atmosphere) in cameras {
        if has_atmosphere {
            continue;
        }

        // The atmosphere's `render_sky` pass **samples the depth texture** (it needs
        // to know where geometry is, for aerial perspective), but `Camera3d::default`
        // creates depth as `RENDER_ATTACHMENT` only. Bevy adds `TEXTURE_BINDING`
        // itself for occlusion-culling views (`configure_occlusion_culling_view_targets`)
        // and *not* for atmosphere, so without this the bind group fails validation:
        //
        //     create_bind_group 'render_sky_bind_group'
        //     Usage flags TextureUsages(RENDER_ATTACHMENT) ... do not contain
        //     required usage flags TextureUsages(TEXTURE_BINDING)
        //
        // The flag is left in place if atmosphere is later switched off: it only
        // *permits* sampling, costs nothing on its own, and removing it would mean
        // guessing whether something else has since come to depend on it.
        if let Some(mut camera_3d) = world.get_mut::<Camera3d>(entity) {
            let usages: bevy::render::render_resource::TextureUsages =
                camera_3d.depth_texture_usages.into();
            camera_3d.depth_texture_usages =
                (usages | bevy::render::render_resource::TextureUsages::TEXTURE_BINDING).into();
        }

        // `Atmosphere` requires `Hdr`, which Bevy inserts for us. That is the whole
        // performance question of this feature: it adds a float intermediate render
        // target, which is cheap on desktop and not obviously so on a Quest.
        world
            .entity_mut(entity)
            .insert((Atmosphere::default(), XrdsManagedSceneAtmosphereEnvironment));
    }
}

fn sync_managed_scene_ibl_in_world(world: &mut World) {
    let Some(environment_map) = resolve_imported_scene_environment_light_in_world(world) else {
        clear_managed_scene_ibl_in_world(world);
        return;
    };

    let cameras: Vec<(Entity, bool, bool)> = {
        let mut query = world.query_filtered::<(
            Entity,
            Option<&EnvironmentMapLight>,
            Option<&XrdsManagedSceneIblEnvironment>,
        ), With<Camera3d>>();
        query
            .iter(world)
            .map(|(entity, environment, managed)| {
                (entity, environment.is_some(), managed.is_some())
            })
            .collect()
    };

    for (entity, has_environment, managed) in cameras {
        if has_environment && !managed {
            continue;
        }

        world
            .entity_mut(entity)
            .insert((environment_map.clone(), XrdsManagedSceneIblEnvironment));
    }
}

fn sync_managed_scene_skybox_in_world(world: &mut World) {
    // Passthrough wins over a skybox, and the two cannot coexist.
    //
    // A skybox is inserted on every `Camera3d`, which includes the XR eye cameras,
    // and it draws into the main pass — so it paints opaque over the transparent
    // clear that passthrough depends on. The projection layer would then be alpha=1
    // everywhere and the passthrough layer beneath it never revealed: the author
    // ticks Passthrough, sees their skybox, and has no way to tell why the room
    // never appears.
    //
    // Suppressed rather than diagnosed because the combination is contradictory
    // rather than merely awkward — a skybox is a *virtual* background and
    // passthrough is a request for the real one. Logged rather than silent, since
    // an authored setting is being overridden.
    if world
        .get_resource::<xrds_openxr::OpenXrPassthroughEnabled>()
        .is_some_and(|e| e.0)
    {
        if clear_managed_scene_skybox_in_world(world) {
            info!(
                "[environment] skybox suppressed while passthrough is on — a skybox would \
                 paint over the real world. Turn passthrough off to see it."
            );
        }
        return;
    }

    let Some(skybox) = resolve_imported_scene_skybox_in_world(world) else {
        clear_managed_scene_skybox_in_world(world);
        return;
    };

    let cameras: Vec<(Entity, bool, bool)> = {
        let mut query = world.query_filtered::<(
            Entity,
            Option<&Skybox>,
            Option<&XrdsManagedSceneSkyboxEnvironment>,
        ), With<Camera3d>>();
        query
            .iter(world)
            .map(|(entity, existing_skybox, managed)| {
                (entity, existing_skybox.is_some(), managed.is_some())
            })
            .collect()
    };

    for (entity, has_skybox, managed) in cameras {
        if has_skybox && !managed {
            continue;
        }

        world
            .entity_mut(entity)
            .insert((skybox.clone(), XrdsManagedSceneSkyboxEnvironment));
    }
}

pub(super) fn sync_managed_scene_exposure_in_world(world: &mut World) {
    let Some(exposure) = resolve_imported_scene_exposure_in_world(world) else {
        clear_managed_scene_exposure_in_world(world);
        return;
    };

    let cameras: Vec<(Entity, bool, bool, bool)> = {
        let mut query = world.query_filtered::<(
            Entity,
            Option<&Exposure>,
            Option<&XrdsManagedSceneExposureEnvironment>,
            Option<&XrdsReceivesEnvironment>,
        ), With<Camera3d>>();
        query
            .iter(world)
            .map(|(entity, existing, managed, receives)| {
                (entity, existing.is_some(), managed.is_some(), receives.is_some())
            })
            .collect()
    };

    for (entity, has_exposure, managed, receives) in cameras {
        if has_exposure && !managed && !receives {
            continue; // skip cameras with user-set exposure that haven't opted in
        }
        world
            .entity_mut(entity)
            .insert((exposure, XrdsManagedSceneExposureEnvironment));
    }
}

fn sync_managed_scene_fog_in_world(world: &mut World) {
    let Some(fog) = resolve_imported_scene_fog_in_world(world) else {
        clear_managed_scene_fog_in_world(world);
        return;
    };

    let cameras: Vec<(Entity, bool, bool, bool)> = {
        let mut query = world.query_filtered::<(
            Entity,
            Option<&DistanceFog>,
            Option<&XrdsManagedSceneFogEnvironment>,
            Option<&XrdsReceivesEnvironment>,
        ), With<Camera3d>>();
        query
            .iter(world)
            .map(|(entity, existing, managed, receives)| {
                (entity, existing.is_some(), managed.is_some(), receives.is_some())
            })
            .collect()
    };

    for (entity, has_fog, managed, receives) in cameras {
        if has_fog && !managed && !receives {
            continue;
        }
        world
            .entity_mut(entity)
            .insert((fog.clone(), XrdsManagedSceneFogEnvironment));
    }
}

pub(super) fn sync_imported_scene_environment_policy_system(world: &mut World) {
    apply_imported_scene_environment_policy_in_world(world);
}

fn resolve_imported_scene_environment_light_in_world(world: &World) -> Option<EnvironmentMapLight> {
    let Some(ibl) =
        imported_scene_environment_in_world(world).and_then(|environment| environment.ibl)
    else {
        return None;
    };

    let Some(diffuse_uri) = resolve_texture_asset_uri_in_world(world, &ibl.diffuse_asset_id) else {
        warn_missing_environment_asset_once(world, "IBL diffuse", &ibl.diffuse_asset_id);
        return None;
    };
    let Some(specular_uri) = resolve_texture_asset_uri_in_world(world, &ibl.specular_asset_id)
    else {
        warn_missing_environment_asset_once(world, "IBL specular", &ibl.specular_asset_id);
        return None;
    };

    let (diffuse_map, specular_map) = {
        let server = world.resource::<AssetServer>();
        (
            server.load::<Image>(diffuse_uri),
            server.load::<Image>(specular_uri),
        )
    };

    // The environment lighting turns with the sky.
    //
    // Rotating the skybox to place the sun and leaving the lighting where it was
    // would put reflections and ambient light on the wrong side of the scene —
    // visible on any smooth metal, and puzzling because the sky *looks* right. The
    // two are one environment, so one yaw drives both.
    //
    // **Same sign as the skybox**, negated, despite the two being documented
    // differently. Bevy calls `Skybox::rotation` a *view space* rotation and
    // `EnvironmentMapLight::rotation` a *world space* one, which is why this first
    // used the yaw unnegated — and the result was a reflection that turned the
    // opposite way to the sky, confirmed by looking at a metal sphere while dragging
    // the slider. The wording differs; the behaviour does not. Both rotate the
    // sampling direction, so both take the same negation.
    //
    // The yaw lives on the skybox settings, so IBL only rotates when a skybox is
    // also present. An IBL-only scene that needs rotating would need its own field;
    // no workflow has asked for that, and inventing one now would add a second
    // control that must agree with the first.
    let yaw = imported_scene_environment_in_world(world)
        .and_then(|environment| environment.skybox)
        .map(|skybox| skybox.yaw_deg)
        .unwrap_or(0.0);

    Some(EnvironmentMapLight {
        diffuse_map,
        specular_map,
        intensity: ibl.intensity,
        rotation: Quat::from_rotation_y(-yaw.to_radians()),
        ..default()
    })
}

fn resolve_imported_scene_skybox_in_world(world: &World) -> Option<Skybox> {
    let Some(skybox) =
        imported_scene_environment_in_world(world).and_then(|environment| environment.skybox)
    else {
        return None;
    };

    let Some(texture_uri) = resolve_texture_asset_uri_in_world(world, &skybox.texture_asset_id)
    else {
        warn_missing_environment_asset_once(world, "skybox", &skybox.texture_asset_id);
        return None;
    };

    let image = {
        let server = world.resource::<AssetServer>();
        server.load::<Image>(texture_uri)
    };

    Some(Skybox {
        image,
        brightness: skybox.brightness,
        // Negated so a positive yaw turns the *sky* the way the author expects.
        // `Skybox::rotation` is applied in view space — it rotates the sampling
        // direction, not the sky — so rotating the lookup by +y appears to swing the
        // sky by -y. Getting this backwards is invisible in code review and obvious
        // the moment someone drags the slider.
        rotation: Quat::from_rotation_y(-skybox.yaw_deg.to_radians()),
        ..default()
    })
}

fn resolve_imported_scene_exposure_in_world(world: &World) -> Option<Exposure> {
    // Anchor-level override takes priority over the scene-wide setting.
    if let Some(ev100) = world.get_resource::<XrdsAnchorExposureOverride>().and_then(|r| r.0) {
        return Some(Exposure { ev100 });
    }
    imported_scene_environment_in_world(world)
        .and_then(|environment| environment.exposure)
        .map(|exposure| Exposure { ev100: exposure.ev100 })
}

fn resolve_imported_scene_fog_in_world(world: &World) -> Option<DistanceFog> {
    imported_scene_environment_in_world(world)
        .and_then(|environment| environment.fog)
        .map(|fog| DistanceFog {
            color: Color::srgba(fog.color[0], fog.color[1], fog.color[2], fog.color[3]),
            falloff: FogFalloff::Linear {
                start: fog.start,
                end: fog.end,
            },
            ..default()
        })
}

fn clear_managed_scene_ibl_in_world(world: &mut World) {
    let entities: Vec<Entity> = {
        let mut query = world
            .query_filtered::<Entity, (With<Camera3d>, With<XrdsManagedSceneIblEnvironment>)>();
        query.iter(world).collect()
    };

    for entity in entities {
        let mut entity_mut = world.entity_mut(entity);
        entity_mut.remove::<EnvironmentMapLight>();
        entity_mut.remove::<XrdsManagedSceneIblEnvironment>();
    }
}

/// Removes the managed skybox from every camera. Returns whether anything was
/// removed, so a caller overriding an authored setting can say so once rather
/// than every frame.
fn clear_managed_scene_skybox_in_world(world: &mut World) -> bool {
    let entities: Vec<Entity> = {
        let mut query = world
            .query_filtered::<Entity, (With<Camera3d>, With<XrdsManagedSceneSkyboxEnvironment>)>();
        query.iter(world).collect()
    };

    let removed = !entities.is_empty();
    for entity in entities {
        let mut entity_mut = world.entity_mut(entity);
        entity_mut.remove::<Skybox>();
        entity_mut.remove::<XrdsManagedSceneSkyboxEnvironment>();
    }
    removed
}

fn clear_managed_scene_exposure_in_world(world: &mut World) {
    let entities: Vec<Entity> = {
        let mut query = world
            .query_filtered::<Entity, (With<Camera3d>, With<XrdsManagedSceneExposureEnvironment>)>(
            );
        query.iter(world).collect()
    };

    for entity in entities {
        let mut entity_mut = world.entity_mut(entity);
        entity_mut.remove::<Exposure>();
        entity_mut.remove::<XrdsManagedSceneExposureEnvironment>();
    }
}

fn clear_managed_scene_fog_in_world(world: &mut World) {
    let entities: Vec<Entity> = {
        let mut query = world
            .query_filtered::<Entity, (With<Camera3d>, With<XrdsManagedSceneFogEnvironment>)>();
        query.iter(world).collect()
    };

    for entity in entities {
        let mut entity_mut = world.entity_mut(entity);
        entity_mut.remove::<DistanceFog>();
        entity_mut.remove::<XrdsManagedSceneFogEnvironment>();
    }
}

fn resolve_texture_asset_uri_in_world(world: &World, asset_id: &str) -> Option<String> {
    let asset_id = asset_id.trim();
    if asset_id.is_empty() {
        return None;
    }

    world
        .get_resource::<XrdsImportedAssetCatalog>()?
        .assets
        .iter()
        .find(|asset| asset.id == asset_id && asset.kind == XrdsSceneAssetKind::EnvironmentMap)
        .map(|asset| asset.uri.clone())
}
