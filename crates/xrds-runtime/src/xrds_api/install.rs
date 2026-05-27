use super::*;
use bevy::asset::embedded_asset;
use bevy::pbr::MaterialPlugin;
use bevy::prelude::*;
pub(super) fn install_xrds(app: &mut App) {
    if app.world().contains_resource::<XrdsInstalled>() {
        return;
    }

    embedded_asset!(app, "shaders/xrds_runtime_material_extension.wgsl");
    embedded_asset!(app, "shaders/xrds_runtime_material_prepass.wgsl");

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
    // Four registration points to guarantee every hierarchy entity has visibility
    // components before Bevy's VisibilityPropagate reads them:
    //
    // 1. SpawnScene (after scene_spawner_system) — fixes bone/skeleton entities the
    //    moment Bevy's scene spawner creates them from a loaded GLB.  This is the
    //    primary defence: without it, those entities arrive in Update without
    //    InheritedVisibility, triggering B0004 → wgpu encoder crash.
    // 2. First — catches anything left over from the previous frame.
    // 3. Update (after apply_queued_parent_changes_system) — covers XRDS-managed
    //    parent changes that happen during the same Update tick.
    // 4. PostUpdate (before VisibilityPropagate) — final safety net.
    app.add_systems(
        SpawnScene,
        ensure_visibility_hierarchy_components_system.after(bevy::scene::scene_spawner_system),
    );
    app.add_systems(First, ensure_visibility_hierarchy_components_system);
    app.add_systems(
        PostUpdate,
        ensure_visibility_hierarchy_components_system
            .before(bevy::camera::visibility::VisibilitySystems::VisibilityPropagate),
    );
    app.add_systems(
        Startup,
        apply_queued_parent_changes_system.after(spawn_surface_components_from_queue),
    );
    app.add_systems(Update, apply_surface_updates);
    app.add_systems(
        Update,
        apply_queued_parent_changes_system.after(apply_surface_updates),
    );
    // Immediately patch any parents that just received ChildOf but are missing
    // visibility components.  This is the primary defence against B0004.
    app.add_systems(
        Update,
        ensure_visibility_hierarchy_components_system.after(apply_queued_parent_changes_system),
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
    // Runs in PreUpdate — before Bevy's audio sink creation — so that entities whose
    // rodio decoder would panic are removed before Bevy ever tries to play them.
    app.add_systems(PreUpdate, pre_validate_audio_decoders_system);
    app.add_observer(apply_pending_gltf_animation_requests_on_scene_ready);
    // Fires synchronously inside scene_spawner_system when a scene is fully
    // instantiated — all ChildOf relationships are established at this point.
    // This is the only guaranteed-correct hook for patching visibility on bone
    // entities before VisibilityPropagate runs.
    app.add_observer(ensure_visibility_on_scene_instance_ready);
    app.init_resource::<XrdsInstalled>();
}

/// Gate that runs every `PreUpdate` frame.
///
/// For each audio entity with a pending `XrdsStoredAudioHandle` (no `AudioPlayer` yet):
/// - If the asset has loaded, attempts `rodio::Decoder` construction inside `catch_unwind`.
/// - On success: inserts `AudioPlayer` + `PlaybackSettings` so Bevy starts playback.
/// - On failure: logs the URI and extension and removes the handle so the entity stays
///   silent but the app keeps running.
///
/// Because `AudioPlayer` is never present until this system grants it, Bevy's
/// observer-based audio sink creation never fires on a bad file.
fn pre_validate_audio_decoders_system(
    mut commands: Commands,
    query: Query<
        (Entity, &Name, &XrdsStoredAudioHandle),
        Without<bevy::audio::AudioPlayer<bevy::audio::AudioSource>>,
    >,
    audio_sources: Option<Res<bevy::asset::Assets<bevy::audio::AudioSource>>>,
    asset_server: Res<AssetServer>,
) {
    use bevy::audio::Decodable;

    let Some(audio_sources) = audio_sources else {
        return; // AudioPlugin not present (test environments)
    };

    for (entity, name, stored) in query.iter() {
        match asset_server.load_state(stored.handle.id()) {
            bevy::asset::LoadState::Loaded => {}
            bevy::asset::LoadState::Failed(ref err) => {
                let extension = std::path::Path::new(&stored.uri)
                    .extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("unknown");
                error!(
                    "[XrdsAudioClip] Failed to load audio for entity '{name}': \
                     uri='{}' extension='.{extension}' error={err}",
                    stored.uri,
                );
                commands.entity(entity).remove::<XrdsStoredAudioHandle>();
                continue;
            }
            _ => continue, // still loading — check again next frame
        }

        let Some(source) = audio_sources.get(&stored.handle) else {
            continue;
        };

        let extension = std::path::Path::new(&stored.uri)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("unknown")
            .to_string();

        let source_clone = source.clone();
        let decode_ok = std::panic::catch_unwind(move || {
            let _ = source_clone.decoder();
        })
        .is_ok();

        if decode_ok {
            let playback = stored.playback;
            commands
                .entity(entity)
                .insert((bevy::audio::AudioPlayer(stored.handle.clone()), playback));
        } else {
            error!(
                "[XrdsAudioClip] Cannot decode audio for entity '{name}': \
                 uri='{}' extension='.{extension}' — rodio returned UnrecognizedFormat. \
                 The file may be corrupt, misnamed, or require a Bevy audio feature \
                 that is not enabled (e.g. 'mp3', 'vorbis', 'symphonia-wav').",
                stored.uri,
            );
            commands.entity(entity).remove::<XrdsStoredAudioHandle>();
        }
    }
}

fn ensure_visibility_hierarchy_components_system(world: &mut World) {
    // Collect (child, parent) pairs from the ChildOf hierarchy.
    let pairs: Vec<(Entity, Entity)> = {
        let mut q = world.query::<(Entity, &ChildOf)>();
        q.iter(world).map(|(child, co)| (child, co.0)).collect()
    };
    if pairs.is_empty() {
        return;
    }

    // child → parent lookup for ancestor traversal.
    let child_to_parent: HashMap<Entity, Entity> = pairs.iter().cloned().collect();

    // Collect unique entities that are missing one or more visibility components.
    let mut seen = HashSet::new();
    let mut entities_needing_fix: Vec<Entity> = Vec::new();
    for (child, parent) in &pairs {
        for &e_id in &[child, parent] {
            let e_id = *e_id;
            if seen.insert(e_id) {
                if let Ok(e) = world.get_entity(e_id) {
                    if !e.contains::<Visibility>()
                        || !e.contains::<InheritedVisibility>()
                        || !e.contains::<ViewVisibility>()
                        || !e.contains::<GlobalTransform>()
                    {
                        entities_needing_fix.push(e_id);
                    }
                }
            }
        }
    }
    if entities_needing_fix.is_empty() {
        return;
    }

    let needing_fix_set: HashSet<Entity> = entities_needing_fix.iter().cloned().collect();

    // Sort so that ancestors are processed before descendants.
    entities_needing_fix.sort_by_key(|&e| {
        let mut count = 0usize;
        let mut cur = e;
        let mut visited = HashSet::new();
        while let Some(&p) = child_to_parent.get(&cur) {
            if !visited.insert(p) {
                break; // cycle guard
            }
            if needing_fix_set.contains(&p) {
                count += 1;
            }
            cur = p;
        }
        count
    });

    // Insert missing components in topological order (ancestors first).
    for entity in entities_needing_fix {
        let Ok(mut e) = world.get_entity_mut(entity) else {
            continue;
        };
        // Use a single bundle-like insertion if possible, but individual is fine.
        if !e.contains::<Visibility>() {
            e.insert(Visibility::Visible);
        }
        if !e.contains::<InheritedVisibility>() {
            e.insert(InheritedVisibility::default());
        }
        if !e.contains::<ViewVisibility>() {
            e.insert(ViewVisibility::default());
        }
        if !e.contains::<GlobalTransform>() {
            e.insert(GlobalTransform::default());
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

/// Observer that fires synchronously inside `scene_spawner_system` the moment a
/// scene is fully instantiated.  At this point every bone/mesh entity has its
/// `ChildOf` component, so we can walk the entire subtree and guarantee that
/// every hierarchy entity has the visibility bundle before `VisibilityPropagate`
/// runs in PostUpdate.
fn ensure_visibility_on_scene_instance_ready(
    scene_ready: On<bevy::scene::SceneInstanceReady>,
    mut commands: Commands,
    children_query: Query<&Children>,
    has_query: Query<(
        Has<Visibility>,
        Has<InheritedVisibility>,
        Has<ViewVisibility>,
        Has<GlobalTransform>,
    )>,
) {
    let mut stack = vec![scene_ready.entity];
    while let Some(entity) = stack.pop() {
        if let Ok((has_vis, has_inh, has_view, has_gt)) = has_query.get(entity) {
            if !has_vis || !has_inh || !has_view || !has_gt {
                let mut cmd = commands.entity(entity);
                if !has_vis {
                    cmd.insert(Visibility::Visible);
                }
                if !has_inh {
                    cmd.insert(InheritedVisibility::default());
                }
                if !has_view {
                    cmd.insert(ViewVisibility::default());
                }
                if !has_gt {
                    cmd.insert(GlobalTransform::default());
                }
            }
        }
        if let Ok(children) = children_query.get(entity) {
            stack.extend(children);
        }
    }
}
