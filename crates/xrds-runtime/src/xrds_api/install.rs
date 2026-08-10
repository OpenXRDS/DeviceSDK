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
    app.add_plugins(avian3d::prelude::PhysicsPlugins::default());
    // Execution substrate for the trigger-action sequencer — see
    // docs/done/xrds-trigger-action-v1.md Phase 0.
    app.add_plugins(bevy_sequential_actions::SequentialActionsPlugin);
    // Not added in test builds: cosmic-text panics in headless environments with no
    // system fonts. Round-trip tests only verify document serialization, not rendering.
    #[cfg(not(test))]
    {
        // LoadFonts is inserted by Runtime::build_bevy_app with absolute paths derived from
        // RuntimeParameters::asset_path. If XrdsAPI::attach is called outside of a Runtime
        // context, fall back to system fonts so text rendering still works.
        let has_load_fonts = app.world().contains_resource::<bevy_rich_text3d::LoadFonts>();
        app.add_plugins(bevy_rich_text3d::Text3dPlugin {
            load_system_fonts: !has_load_fonts,
            asynchronous_load: false,
            // 1024-wide atlas matches the official examples (arabic.rs, spooky.rs).
            // Needed because size:128 glyphs can reach 192px+ on HiDPI displays,
            // leaving only 2 glyphs per row in the default 512-wide atlas.
            default_atlas_dimension: (1024, 512),
            ..Default::default()
        });
        app.add_plugins(bevy_fontmesh::FontMeshPlugin::<bevy::pbr::StandardMaterial>::default());
    }
    // Not added in test builds either: OutlinePlugin::build() calls
    // .sub_app_mut(RenderApp), which unconditionally requires Bevy's render
    // sub-app to already exist (normally created by bevy_render::RenderPlugin).
    // The minimal headless xrds_test_app() harness never adds that, so this
    // would panic in every test that calls XrdsAPI::attach. Real apps go
    // through Runtime::build_bevy_app, which includes full DefaultPlugins
    // (RenderPlugin included), so this is never skipped outside tests.
    #[cfg(not(test))]
    if !app.is_plugin_added::<bevy_mod_outline::OutlinePlugin>() {
        app.add_plugins(bevy_mod_outline::OutlinePlugin);
    }
    app.init_resource::<crate::xrds_api::anchor::ActivePlayerAnchorEntity>();
    app.init_resource::<XrdsIdAllocator>();
    app.init_resource::<XrdsIdIndex>();
    app.init_resource::<XrdsHierarchyIndex>();
    app.init_resource::<XrdsImportedAssetCatalog>();
    app.init_resource::<XrdsImportedSceneEnvironment>();
    app.init_resource::<XrdsImportedPanelLibrary>();
    app.init_resource::<XrdsPanelElementIndex>();
    app.init_resource::<SurfaceInterpreterRegistry>();
    app.init_resource::<SurfaceDescriptorRegistry>();
    app.init_resource::<QueuedSurfaceComponents>();
    app.init_resource::<QueuedParentChanges>();
    app.init_resource::<SurfaceUpdateRegistry>();
    app.init_resource::<QueuedSurfaceUpdates>();
    app.init_resource::<PendingGltfAnimationRequests>();
    app.init_resource::<ActiveGltfAnimationStates>();
    app.init_resource::<PendingGltfMorphTargetOverrideRequests>();
    app.init_resource::<crate::xrds_api::grab::XrGrabState>();
    app.init_resource::<xrds_components::XrdsWorldPointerState>();
    app.init_resource::<xrds_components::XrdsWorldPointerCursors>();
    // Always present so world-UI systems run on desktop too (the OpenXR plugin only
    // initialises it when an XR runtime is attached). Desktop hosts (e.g. the editor's
    // play mode) drive it synthetically from mouse input.
    app.init_resource::<xrds_openxr::XrInput>();
    app.init_resource::<crate::xrds_api::environment::XrdsAnchorExposureOverride>();
    app.init_resource::<crate::xrds_api::trigger_action::XrdsTrackRegistry>();
    // Which entities each running Track holds. Must exist before any Track
    // can start — `spawn_track_agent_in_world` consults it to decide whether
    // to reject a newcomer.
    app.init_resource::<crate::xrds_api::trigger_action::XrdsTrackAssetLocks>();
    app.add_message::<xrds_components::XrGrabEvent>();
    app.add_message::<xrds_components::XrDropEvent>();
    app.add_message::<xrds_components::XrZoneEnterEvent>();
    app.add_message::<xrds_components::XrZoneExitEvent>();
    app.add_message::<xrds_components::XrWorldHoverEnterEvent>();
    app.add_message::<xrds_components::XrWorldHoverExitEvent>();
    app.add_message::<xrds_components::XrWorldButtonPressEvent>();
    app.add_message::<xrds_components::XrWorldButtonReleaseEvent>();
    app.add_message::<xrds_components::XrWorldSliderChangeEvent>();
    app.add_message::<xrds_components::XrWorldToggleEvent>();
    app.add_message::<crate::xrds_api::trigger_action::XrdsCustomTriggerEvent>();
    app.add_message::<crate::xrds_api::trigger_action::XrdsGltfAnimationCompleteEvent>();
    app.add_message::<crate::xrds_api::trigger_action::XrdsThresholdCrossedEvent>();

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
            .after(XrdsUpdateSystemSet)
            .before(bevy::camera::visibility::VisibilitySystems::VisibilityPropagate),
    );
    // Billboard — single() fails when there are 2 XR eye cameras; kept separate so
    // billboard can keep its parent-relative rotation path unchanged.
    app.add_systems(
        PostUpdate,
        crate::xrds_api::billboard::billboard_system
            .after(bevy::transform::TransformSystems::Propagate)
            .before(bevy::camera::visibility::VisibilitySystems::VisibilityPropagate),
    );
    // Player root sync — runs BEFORE TransformPropagate so the updated player body
    // position propagates down to all anchor children in the same frame.
    // Only fires when an XrdsPlayerCamera entity exists (deployed runtime only;
    // the editor uses its own sync_player_root_system with PlayerPawnMarker).
    app.add_systems(
        PostUpdate,
        crate::xrds_api::anchor::sync_player_root_system
            .before(bevy::transform::TransformSystems::Propagate),
    );
    // Anchor modes — must run AFTER TransformPropagate so camera GlobalTransforms are
    // fresh, and write GlobalTransform directly so VisibilityPropagate sees the correct
    // world position this frame without waiting for a second Propagate pass.
    //
    // teleport_on_anchor_switch_system runs first (only fires when XrdsPlayerCamera
    // exists) so the updated camera position is visible to all anchor-mode systems
    // within the same frame as the switch.
    app.add_systems(
        PostUpdate,
        (
            crate::xrds_api::anchor::teleport_on_anchor_switch_system,
            crate::xrds_api::anchor::head_locked_system,
            crate::xrds_api::anchor::body_locked_system,
            crate::xrds_api::anchor::comfort_pinned_system,
            crate::xrds_api::anchor::cylindrical_system,
            crate::xrds_api::anchor::apply_anchor_fov_system,
            crate::xrds_api::anchor::apply_anchor_exposure_system,
        )
            .chain()
            .after(bevy::transform::TransformSystems::Propagate)
            .before(bevy::camera::visibility::VisibilitySystems::VisibilityPropagate),
    );
    // Diagnostic: log whether head-locked HUD entities appear in each Camera3d's
    // VisibleEntities after check_visibility runs.  Runs every 90 frames to avoid spam.
    app.add_systems(
        PostUpdate,
        crate::xrds_api::anchor::vis_diag_system
            .after(bevy::camera::visibility::VisibilitySystems::CheckVisibility),
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
    // NoFrustumCulling (used on primitives, text anchors, and world-UI meshes to
    // survive XR multi-view culling) makes Bevy's calculate_bounds skip the entity,
    // so it never receives an Aabb — and every Aabb-based feature (grab raycast,
    // XrdsAPI::raycast, zones) silently stops seeing it. Backfill the Aabb from
    // the mesh ourselves; NoFrustumCulling still disables culling either way.
    app.add_systems(Update, ensure_aabbs_for_unculled_meshes_system);
    app.add_systems(Update, crate::xrds_api::grab::grab_system);
    app.add_systems(Update, crate::xrds_api::zone::zone_collision_system);
    // Timeline scheduler (docs/done/xrds-trigger-action-v1.md Phase 9) —
    // absolute-time, concurrent choreography, run independently of the
    // trigger/sequence machinery below.
    // Adopt authored edits into already-running agents *before* advancing them,
    // so a duration/looping change made while a Track is playing takes effect
    // on this frame rather than a lap later. Explicitly ordered rather than
    // relying on registration order, which Bevy does not guarantee.
    app.add_systems(
        Update,
        crate::xrds_api::trigger_action::sync_live_track_agents
            .before(crate::xrds_api::trigger_action::advance_tracks),
    );
    app.add_systems(Update, crate::xrds_api::trigger_action::advance_tracks);
    // Trigger-action sequencing (docs/done/xrds-trigger-action-v1.md
    // Phase 3). Explicitly ordered after zone_collision_system rather than
    // relying on Bevy's event double-buffering to hide a missing constraint:
    // this way a zone entered on frame N fires its sequence on frame N, not N+1.
    // One consume_triggers registration per XrdsTriggerEvent implementor — adding
    // a new trigger source later is one more line here plus one trait impl.
    app.add_systems(
        Update,
        (
            crate::xrds_api::trigger_action::consume_triggers::<xrds_components::XrZoneEnterEvent>,
            crate::xrds_api::trigger_action::consume_triggers::<xrds_components::XrZoneExitEvent>,
            crate::xrds_api::trigger_action::consume_triggers::<
                crate::xrds_api::trigger_action::XrdsGltfAnimationCompleteEvent,
            >,
            // XR grab interaction.
            crate::xrds_api::trigger_action::consume_triggers::<xrds_components::XrGrabEvent>,
            crate::xrds_api::trigger_action::consume_triggers::<xrds_components::XrDropEvent>,
            // World-space UI. Every registration here is the same one line
            // plus a ~5-line trait impl — the whole point of the pluggable
            // XrdsTriggerEvent design.
            crate::xrds_api::trigger_action::consume_triggers::<
                xrds_components::XrWorldHoverEnterEvent,
            >,
            crate::xrds_api::trigger_action::consume_triggers::<
                xrds_components::XrWorldHoverExitEvent,
            >,
            crate::xrds_api::trigger_action::consume_triggers::<
                xrds_components::XrWorldButtonPressEvent,
            >,
            crate::xrds_api::trigger_action::consume_triggers::<
                xrds_components::XrWorldButtonReleaseEvent,
            >,
            crate::xrds_api::trigger_action::consume_triggers::<
                xrds_components::XrWorldSliderChangeEvent,
            >,
            crate::xrds_api::trigger_action::consume_triggers::<
                xrds_components::XrWorldToggleEvent,
            >,
            // Threshold watchers (docs/done/xrds-trigger-action-v1.md
            // Phase 8) — a crossing is just another way to fire Custom, so it
            // reuses this same generic consumer, no special-casing.
            crate::xrds_api::trigger_action::consume_triggers::<
                crate::xrds_api::trigger_action::XrdsThresholdCrossedEvent,
            >,
        )
            .after(crate::xrds_api::zone::zone_collision_system),
    );
    // All three run in Last: by then Bevy's animation and transform-propagation
    // systems have both advanced this frame, and SequentialActionsPlugin has
    // advanced the action queues.
    app.add_systems(
        Last,
        (
            // Corrects the cached XrdsGltfAnimationState.playing flag and emits
            // AnimationComplete triggers.
            crate::xrds_api::trigger_action::sync_completed_gltf_animation_triggers,
            // Reads GlobalTransform (fresh as of this frame's PostUpdate
            // propagation) and emits XrdsThresholdCrossedEvent on qualifying
            // crossings.
            crate::xrds_api::trigger_action::evaluate_threshold_watchers,
            // Reaps ephemeral per-firing agents once their queue drains.
            crate::xrds_api::trigger_action::despawn_finished_sequence_agents,
        ),
    );
    // Advances in-flight SetTransform tweens. No explicit ordering
    // against SequentialActionsPlugin's own per-frame advancement — if it
    // happens to run first on a given frame, `is_finished` just sees this
    // tween's completion one frame later, the same one-frame-latency cost
    // already accepted for `evaluate_threshold_watchers` above, not a
    // correctness issue either way.
    app.add_systems(Update, crate::xrds_api::trigger_action::advance_transform_tweens);
    app.add_systems(Update, crate::xrds_api::world_ui_pointer::world_ui_pointer_system);
    app.add_systems(Startup, crate::xrds_api::world_ui_pointer::spawn_world_ui_cursors_system);
    // Layout runs after the pointer system (so pointer state is fresh) but before the
    // button/slider/toggle interaction systems — ensuring `local_position` is correct
    // when those systems do their per-widget hit-tests.
    app.add_systems(
        Update,
        crate::xrds_api::world_ui_layout::world_ui_layout_system
            .after(crate::xrds_api::world_ui_pointer::world_ui_pointer_system),
    );
    // Button, slider, toggle systems run after layout so positions are stable.
    app.add_systems(
        Update,
        (
            crate::xrds_api::world_ui_button::world_ui_button_system,
            crate::xrds_api::world_ui_slider::world_ui_slider_system,
            crate::xrds_api::world_ui_toggle::world_ui_toggle_system,
        )
            .after(crate::xrds_api::world_ui_layout::world_ui_layout_system),
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

/// Backfill `Aabb` on mesh entities that opted out of frustum culling.
///
/// Bevy's `calculate_bounds` skips `NoFrustumCulling` entities entirely, so they
/// never get an `Aabb` — which silently removes them from every Aabb-based
/// feature in the SDK (grab hover/raycast, `XrdsAPI` raycasts, zone checks).
/// Computing the box from the mesh keeps those features working; culling stays
/// disabled because `NoFrustumCulling` takes precedence over a present `Aabb`.
///
/// Also refreshes the box when the mesh asset is modified in place (text meshes
/// regenerate as their string changes) — Bevy only does that for culled entities,
/// so without this a growing HUD label would keep its original smaller hitbox.
fn ensure_aabbs_for_unculled_meshes_system(
    mut commands: Commands,
    meshes: Res<Assets<Mesh>>,
    mut mesh_events: MessageReader<AssetEvent<Mesh>>,
    missing: Query<
        (Entity, &Mesh3d),
        (
            With<bevy::camera::visibility::NoFrustumCulling>,
            Without<bevy::camera::primitives::Aabb>,
        ),
    >,
    unculled: Query<(Entity, &Mesh3d), With<bevy::camera::visibility::NoFrustumCulling>>,
) {
    use bevy::camera::primitives::MeshAabb;

    let compute = |mesh3d: &Mesh3d| meshes.get(&mesh3d.0).and_then(|m| m.compute_aabb());

    let modified: std::collections::HashSet<AssetId<Mesh>> = mesh_events
        .read()
        .filter_map(|event| match event {
            AssetEvent::Modified { id } => Some(*id),
            _ => None,
        })
        .collect();
    if !modified.is_empty() {
        for (entity, mesh3d) in &unculled {
            if modified.contains(&mesh3d.0.id()) {
                if let Some(aabb) = compute(mesh3d) {
                    commands.entity(entity).insert(aabb);
                }
            }
        }
    }

    for (entity, mesh3d) in &missing {
        // Mesh may still be loading — retried automatically next frame.
        if let Some(aabb) = compute(mesh3d) {
            commands.entity(entity).insert(aabb);
        }
    }
}
