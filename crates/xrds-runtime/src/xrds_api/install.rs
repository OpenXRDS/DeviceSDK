use super::*;
use bevy::pbr::MaterialPlugin;
pub(super) fn install_xrds(app: &mut App) {
    if app.world().contains_resource::<XrdsInstalled>() {
        return;
    }

    app.add_plugins(XrdsComponentsPlugin);
    app.add_plugins(MaterialPlugin::<XrdsRuntimeMaterial>::default());
    app.init_resource::<XrdsIdAllocator>();
    app.init_resource::<XrdsIdIndex>();
    app.init_resource::<XrdsHierarchyIndex>();
    app.init_resource::<XrdsImportedAssetCatalog>();
    app.init_resource::<XrdsImportedSceneEnvironment>();
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
    app.add_systems(
        Update,
        sync_imported_scene_environment_policy_system
            .after(apply_pending_gltf_morph_target_override_requests_system),
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
