use super::*;
use bevy::camera::Exposure;
use bevy::core_pipeline::Skybox;
use bevy::pbr::{DistanceFog, FogFalloff};
use xrds_scene_graph::XrdsSceneEnvironment;

#[derive(Resource, Debug, Clone, Default, PartialEq)]
pub(super) struct XrdsImportedSceneEnvironment(pub(super) Option<XrdsSceneEnvironment>);

#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq)]
struct XrdsManagedSceneIblEnvironment;

#[derive(Component, Debug, Clone, Copy, Default, PartialEq, Eq)]
struct XrdsManagedSceneSkyboxEnvironment;

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
    sync_managed_scene_exposure_in_world(world);
    sync_managed_scene_fog_in_world(world);
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
        warn!(
            "Scene IBL diffuse asset '{}' was not found in the runtime asset catalog",
            ibl.diffuse_asset_id
        );
        return None;
    };
    let Some(specular_uri) = resolve_texture_asset_uri_in_world(world, &ibl.specular_asset_id)
    else {
        warn!(
            "Scene IBL specular asset '{}' was not found in the runtime asset catalog",
            ibl.specular_asset_id
        );
        return None;
    };

    let (diffuse_map, specular_map) = {
        let server = world.resource::<AssetServer>();
        (
            server.load::<Image>(diffuse_uri),
            server.load::<Image>(specular_uri),
        )
    };

    Some(EnvironmentMapLight {
        diffuse_map,
        specular_map,
        intensity: ibl.intensity,
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
        warn!(
            "Scene skybox asset '{}' was not found in the runtime asset catalog",
            skybox.texture_asset_id
        );
        return None;
    };

    let image = {
        let server = world.resource::<AssetServer>();
        server.load::<Image>(texture_uri)
    };

    Some(Skybox {
        image,
        brightness: skybox.brightness,
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

fn clear_managed_scene_skybox_in_world(world: &mut World) {
    let entities: Vec<Entity> = {
        let mut query = world
            .query_filtered::<Entity, (With<Camera3d>, With<XrdsManagedSceneSkyboxEnvironment>)>();
        query.iter(world).collect()
    };

    for entity in entities {
        let mut entity_mut = world.entity_mut(entity);
        entity_mut.remove::<Skybox>();
        entity_mut.remove::<XrdsManagedSceneSkyboxEnvironment>();
    }
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
