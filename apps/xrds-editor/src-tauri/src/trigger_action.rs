//! Editor bridge for Track authoring. Mirrors `hud_library.rs`'s shape: a
//! snapshot serializer plus a command dispatcher, both operating on the exact
//! same `XrdsSceneDocument` data the runtime consumes
//! (`XrdsSceneDocument::tracks`, `XrdsSceneNode::triggers`/`.watchers`).
//!
//! See `docs/done/xrds-track-model-plan.md` for the model. The command surface is
//! row-addressed — `(track, asset_index, key_index)` — because an event
//! belongs to an asset row, not to a flat list.

use bevy::log::error;
use xrds_components::XrGrabHand;
use xrds_scene_graph::{
    XrdsAction, XrdsActionTarget, XrdsActionValue, XrdsAxis, XrdsCrossing, XrdsEaseCurve,
    XrdsNamedTrack, XrdsObservable, XrdsSceneAnimationRepeatMode, XrdsSceneDocument,
    XrdsSceneGltfAnimationSelector, XrdsSceneGltfPlayback, XrdsSceneNodeId,
    XrdsSceneTriggerDiagnostic, XrdsSceneTriggerDiagnosticSeverity, XrdsThresholdWatcher,
    XrdsTrack, XrdsTrackAsset, XrdsTrackKey, XrdsTriggerBinding, XrdsTriggerKind,
};
use crate::bridge::{
    ActionTargetDto, ActionValueDto, EditorCommand, NamedTrackDto, NodeBindingSummaryDto,
    NodeWatcherSummaryDto, ObservableDto, ThresholdWatcherDto, TriggerBindingDto,
    TriggerDiagnosticDto, XrdsActionDto, XrdsTrackAssetDto, XrdsTrackKeyDto, XrdsTriggerKindDto,
};
use crate::editor_state::{EditorSession, EditorState};

// ---------------------------------------------------------------------------
// Snapshot serializers
// ---------------------------------------------------------------------------

pub fn build_tracks_dto(doc: &XrdsSceneDocument) -> Vec<NamedTrackDto> {
    doc.tracks
        .iter()
        .map(|entry| NamedTrackDto {
            name: entry.name.clone(),
            assets: entry
                .track
                .assets
                .iter()
                .map(|asset| XrdsTrackAssetDto {
                    target: action_target_to_dto(&asset.target),
                    // Resolved here so the frontend can label a row without
                    // walking the hierarchy. `None` for a SelfNode/
                    // TriggerSource row, or a Node target that no longer
                    // exists — the latter is separately diagnosed.
                    node_name: match asset.target {
                        XrdsActionTarget::Node(id) => doc.node(id).map(|n| n.name.clone()),
                        _ => None,
                    },
                    keys: asset.keys.iter().map(track_key_to_dto).collect(),
                })
                .collect(),
            duration_secs: entry.track.duration_secs,
            effective_duration_secs: entry.track.effective_duration_secs(),
            looping: entry.track.looping,
        })
        .collect()
}

/// Registry-level diagnostics only (`node_id: None`) — for
/// `EditorSnapshot::track_diagnostics`.
pub fn build_track_diagnostics_dto(doc: &XrdsSceneDocument) -> Vec<TriggerDiagnosticDto> {
    doc.track_diagnostics()
        .iter()
        .filter(|d| d.node_id.is_none())
        .map(diagnostic_to_dto)
        .collect()
}

/// Called from `inspector.rs::build_node_inspector` — this node's bindings,
/// watchers, and this node's subset (`node_id == Some(id)`) of diagnostics.
pub fn build_node_triggers_dto(node_triggers: &[XrdsTriggerBinding]) -> Vec<TriggerBindingDto> {
    node_triggers.iter().map(binding_to_dto).collect()
}

pub fn build_node_watchers_dto(node_watchers: &[XrdsThresholdWatcher]) -> Vec<ThresholdWatcherDto> {
    node_watchers.iter().map(watcher_to_dto).collect()
}

/// Every trigger binding across the *whole* document, each tagged with its
/// owning node's id/name — not just the currently-selected node.
///
/// Added for the sequencer redesign's "Triggers" hierarchy grouping and its
/// reverse lookup ("which node's binding names runnable R"): without this,
/// the frontend snapshot only ever carries one node's bindings
/// (`selected_node.triggers`), so a hierarchy-wide view has nothing to
/// derive from. Purely a snapshot-serialization addition — the persisted
/// `.xrds` document schema is unchanged, this only affects what the
/// read-only `EditorSnapshot` exposes, same as `runnable_diagnostics`.
pub fn build_all_node_bindings_dto(doc: &XrdsSceneDocument) -> Vec<NodeBindingSummaryDto> {
    doc.nodes.iter()
        .flat_map(|node| node.triggers.iter().enumerate().map(move |(binding_index, binding)| {
            NodeBindingSummaryDto {
                node_id: node.id.0,
                node_name: node.name.clone(),
                binding_index,
                binding: binding_to_dto(binding),
            }
        }))
        .collect()
}

/// Every threshold watcher across the *whole* document, each tagged with
/// its owning node's id/name — same rationale as
/// `build_all_node_bindings_dto` above, for the Hierarchy Triggers
/// grouping's Watchers sub-row.
pub fn build_all_node_watchers_dto(doc: &XrdsSceneDocument) -> Vec<NodeWatcherSummaryDto> {
    doc.nodes.iter()
        .flat_map(|node| node.watchers.iter().enumerate().map(move |(watcher_index, watcher)| {
            NodeWatcherSummaryDto {
                node_id: node.id.0,
                node_name: node.name.clone(),
                watcher_index,
                watcher: watcher_to_dto(watcher),
            }
        }))
        .collect()
}

pub fn build_node_trigger_diagnostics_dto(
    doc: &XrdsSceneDocument,
    id: XrdsSceneNodeId,
) -> Vec<TriggerDiagnosticDto> {
    doc.track_diagnostics().iter()
        .filter(|d| d.node_id == Some(id))
        .map(diagnostic_to_dto)
        .collect()
}

// ---------------------------------------------------------------------------
// Command handlers
// ---------------------------------------------------------------------------

/// Returns true if a full scene reimport is needed after the command.
pub fn apply_trigger_action_command(
    cmd: &EditorCommand,
    session: &mut EditorSession,
    state: &mut EditorState,
) -> bool {
    match cmd {
        // --- Track registry ---
        EditorCommand::CreateTrack { name } => {
            let name = name.clone();
            match session.0.edit(|doc| {
                if doc.track(&name).is_some() {
                    error!("[track] CreateTrack: {name:?} already exists");
                    return;
                }
                doc.tracks.push(XrdsNamedTrack { name, track: XrdsTrack::default() });
            }) {
                Ok(_) => {}
                Err(e) => error!("[track] CreateTrack failed: {:?}", e),
            }
            false
        }

        EditorCommand::DeleteTrack { name } => {
            let name = name.clone();
            match session.0.edit(|doc| {
                doc.tracks.retain(|t| t.name != name);
                // Clear bindings that named it, rather than leaving them
                // pointing at nothing: a dangling name is diagnosable but a
                // cleared one is honest about what happened.
                for node in &mut doc.nodes {
                    for binding in &mut node.triggers {
                        if binding.track.as_deref() == Some(name.as_str()) {
                            binding.track = None;
                        }
                    }
                }
            }) {
                Ok(_) => {}
                Err(e) => error!("[track] DeleteTrack failed: {:?}", e),
            }
            true // the registry the runtime mirrors changed
        }

        EditorCommand::RenameTrack { old_name, new_name } => {
            let old_name = old_name.clone();
            let new_name = new_name.clone();
            match session.0.edit(|doc| {
                if doc.track(&new_name).is_some() {
                    error!("[track] RenameTrack: {new_name:?} already exists");
                    return;
                }
                if let Some(t) = doc.track_mut(&old_name) {
                    t.name = new_name.clone();
                } else {
                    return;
                }
                // Re-point every binding, so a rename never breaks wiring.
                for node in &mut doc.nodes {
                    for binding in &mut node.triggers {
                        if binding.track.as_deref() == Some(old_name.as_str()) {
                            binding.track = Some(new_name.clone());
                        }
                    }
                }
            }) {
                Ok(_) => {}
                Err(e) => error!("[track] RenameTrack failed: {:?}", e),
            }
            true
        }

        EditorCommand::SetTrackLooping { name, looping } => {
            let name = name.clone();
            let looping = *looping;
            match session.0.edit(|doc| {
                if let Some(t) = doc.track_mut(&name) {
                    t.track.looping = looping;
                }
            }) {
                Ok(_) => {}
                Err(e) => error!("[track] SetTrackLooping failed: {:?}", e),
            }
            true
        }

        EditorCommand::SetTrackDuration { name, duration_secs } => {
            let name = name.clone();
            let duration_secs = *duration_secs;
            match session.0.edit(|doc| {
                if let Some(t) = doc.track_mut(&name) {
                    t.track.duration_secs = duration_secs;
                }
            }) {
                Ok(_) => {}
                Err(e) => error!("[track] SetTrackDuration failed: {:?}", e),
            }
            true
        }

        // --- Asset rows ---
        EditorCommand::AddTrackAsset { track, node_id } => {
            let track = track.clone();
            let node_id = XrdsSceneNodeId(*node_id);
            match session.0.edit(|doc| {
                let Some(entry) = doc.track_mut(&track) else {
                    error!("[track] AddTrackAsset: no Track named {track:?}");
                    return;
                };
                // One row per asset. Refusing here means the UI cannot create
                // a state `track_diagnostics` would immediately flag.
                if entry
                    .track
                    .assets
                    .iter()
                    .any(|a| a.target == XrdsActionTarget::Node(node_id))
                {
                    error!("[track] AddTrackAsset: {node_id:?} already has a row in {track:?}");
                    return;
                }
                entry.track.assets.push(XrdsTrackAsset {
                    target: XrdsActionTarget::Node(node_id),
                    keys: Vec::new(),
                });
            }) {
                Ok(_) => {}
                Err(e) => error!("[track] AddTrackAsset failed: {:?}", e),
            }
            true
        }

        EditorCommand::RemoveTrackAsset { track, asset_index } => {
            let track = track.clone();
            let asset_index = *asset_index;
            match session.0.edit(|doc| {
                if let Some(entry) = doc.track_mut(&track) {
                    if asset_index < entry.track.assets.len() {
                        entry.track.assets.remove(asset_index);
                    }
                }
            }) {
                Ok(_) => {}
                Err(e) => error!("[track] RemoveTrackAsset failed: {:?}", e),
            }
            true
        }

        EditorCommand::SetTrackAssetTarget { track, asset_index, node_id } => {
            let track = track.clone();
            let asset_index = *asset_index;
            let node_id = XrdsSceneNodeId(*node_id);
            match session.0.edit(|doc| {
                let Some(entry) = doc.track_mut(&track) else { return };
                if entry
                    .track
                    .assets
                    .iter()
                    .enumerate()
                    .any(|(i, a)| i != asset_index && a.target == XrdsActionTarget::Node(node_id))
                {
                    error!("[track] SetTrackAssetTarget: {node_id:?} already has a row in {track:?}");
                    return;
                }
                if let Some(asset) = entry.track.assets.get_mut(asset_index) {
                    asset.target = XrdsActionTarget::Node(node_id);
                }
            }) {
                Ok(_) => {}
                Err(e) => error!("[track] SetTrackAssetTarget failed: {:?}", e),
            }
            true
        }

        // --- Events on a row ---
        EditorCommand::AddTrackKey { track, asset_index, at_secs, kind } => {
            let track = track.clone();
            let asset_index = *asset_index;
            let at_secs = *at_secs;
            let Some(action) = default_action_for_kind(kind) else {
                error!("[track] AddTrackKey: unknown kind {kind:?}");
                return false;
            };
            match session.0.edit(|doc| {
                if let Some(entry) = doc.track_mut(&track) {
                    if let Some(asset) = entry.track.assets.get_mut(asset_index) {
                        asset.keys.push(XrdsTrackKey { at_secs, action });
                        // Keep each row sorted so the editor never has to, and
                        // so a key's index means the same thing on both sides.
                        asset.keys.sort_by(|a, b| a.at_secs.total_cmp(&b.at_secs));
                    }
                }
            }) {
                Ok(_) => {}
                Err(e) => error!("[track] AddTrackKey failed: {:?}", e),
            }
            true
        }

        EditorCommand::RemoveTrackKey { track, asset_index, key_index } => {
            let track = track.clone();
            let asset_index = *asset_index;
            let key_index = *key_index;
            match session.0.edit(|doc| {
                if let Some(entry) = doc.track_mut(&track) {
                    if let Some(asset) = entry.track.assets.get_mut(asset_index) {
                        if key_index < asset.keys.len() {
                            asset.keys.remove(key_index);
                        }
                    }
                }
            }) {
                Ok(_) => {}
                Err(e) => error!("[track] RemoveTrackKey failed: {:?}", e),
            }
            true
        }

        EditorCommand::SetTrackKey { track, asset_index, key_index, key } => {
            let track = track.clone();
            let asset_index = *asset_index;
            let key_index = *key_index;
            let new_key = track_key_from_dto(key);
            match session.0.edit(|doc| {
                if let Some(entry) = doc.track_mut(&track) {
                    if let Some(asset) = entry.track.assets.get_mut(asset_index) {
                        if let Some(slot) = asset.keys.get_mut(key_index) {
                            *slot = new_key;
                        }
                        asset.keys.sort_by(|a, b| a.at_secs.total_cmp(&b.at_secs));
                    }
                }
            }) {
                Ok(_) => {}
                Err(e) => error!("[track] SetTrackKey failed: {:?}", e),
            }
            true
        }

        // --- Editor preview transport ---
        // Deliberately separate from `SetPlayMode`: previewing one Track is not
        // running the simulation. Here we only record the intent; the drain
        // belongs in `bevy_scene.rs`, which owns world access.

        EditorCommand::PreviewPlayTrack { name } => {
            state.pending_track_preview =
                Some(crate::editor_state::TrackPreviewRequest::Play(name.clone()));
            false
        }
        EditorCommand::PreviewPauseTrack { paused } => {
            state.pending_track_preview =
                Some(crate::editor_state::TrackPreviewRequest::Pause(*paused));
            false
        }
        EditorCommand::PreviewStopTrack => {
            state.pending_track_preview = Some(crate::editor_state::TrackPreviewRequest::Stop);
            false
        }

        // --- Per-node bindings ---
        EditorCommand::AddTriggerBinding { node_id } => {
            let id = XrdsSceneNodeId(*node_id);
            match session.0.edit(|doc| {
                if let Some(node) = doc.node_mut(id) {
                    // Editor-only choice, not the domain type's own Default:
                    // a freshly added binding should visibly need a trigger
                    // kind picked, not silently start as "ZoneEnter" (which
                    // reads as already configured and safe to leave alone).
                    // `Unknown` already means "never fires" at runtime, so
                    // it doubles as an inert "none selected yet" placeholder
                    // — see the matching `— none selected —` option in
                    // TriggersSection (Inspector.tsx).
                    node.triggers.push(XrdsTriggerBinding {
                        trigger: XrdsTriggerKind::Unknown,
                        ..Default::default()
                    });
                }
            }) {
                Ok(_) => {}
                Err(e) => error!("[trigger_action] AddTriggerBinding failed: {:?}", e),
            }
            true
        }

        EditorCommand::RemoveTriggerBinding { node_id, index } => {
            let id = XrdsSceneNodeId(*node_id);
            let index = *index;
            match session.0.edit(|doc| {
                if let Some(node) = doc.node_mut(id) {
                    if index < node.triggers.len() {
                        node.triggers.remove(index);
                    }
                }
            }) {
                Ok(_) => {}
                Err(e) => error!("[trigger_action] RemoveTriggerBinding failed: {:?}", e),
            }
            true
        }

        EditorCommand::SetTriggerBindingTrigger { node_id, index, trigger } => {
            let id = XrdsSceneNodeId(*node_id);
            let index = *index;
            let trigger = trigger_kind_from_dto(trigger);
            match session.0.edit(|doc| {
                if let Some(node) = doc.node_mut(id) {
                    if let Some(b) = node.triggers.get_mut(index) {
                        b.trigger = trigger;
                    }
                }
            }) {
                Ok(_) => {}
                Err(e) => error!("[trigger_action] SetTriggerBindingTrigger failed: {:?}", e),
            }
            true
        }

        EditorCommand::SetTriggerBindingHand { node_id, index, hand } => {
            let id = XrdsSceneNodeId(*node_id);
            let index = *index;
            let hand = hand_from_dto(hand);
            match session.0.edit(|doc| {
                if let Some(node) = doc.node_mut(id) {
                    if let Some(b) = node.triggers.get_mut(index) {
                        b.hand = hand;
                    }
                }
            }) {
                Ok(_) => {}
                Err(e) => error!("[trigger_action] SetTriggerBindingHand failed: {:?}", e),
            }
            true
        }

        EditorCommand::SetTriggerBindingDisabled { node_id, index, disabled } => {
            let id = XrdsSceneNodeId(*node_id);
            let index = *index;
            let disabled = *disabled;
            match session.0.edit(|doc| {
                if let Some(node) = doc.node_mut(id) {
                    if let Some(b) = node.triggers.get_mut(index) {
                        b.disabled = disabled;
                    }
                }
            }) {
                Ok(_) => {}
                Err(e) => error!("[trigger_action] SetTriggerBindingDisabled failed: {:?}", e),
            }
            true
        }

        EditorCommand::SetTriggerBindingTrack { node_id, index, track } => {
            let id = XrdsSceneNodeId(*node_id);
            let index = *index;
            let track = track.clone();
            match session.0.edit(|doc| {
                if let Some(node) = doc.node_mut(id) {
                    if let Some(b) = node.triggers.get_mut(index) {
                        b.track = track;
                    }
                }
            }) {
                Ok(_) => {}
                Err(e) => error!("[track] SetTriggerBindingTrack failed: {:?}", e),
            }
            true
        }

        // --- Per-node watchers ---
        EditorCommand::AddWatcher { node_id } => {
            let id = XrdsSceneNodeId(*node_id);
            match session.0.edit(|doc| {
                if let Some(node) = doc.node_mut(id) {
                    node.watchers.push(XrdsThresholdWatcher {
                        observable: XrdsObservable::Height,
                        crossing: XrdsCrossing::default(),
                        value: 0.0,
                        hysteresis: 0.0,
                        fires: "watcher".to_string(),
                        disabled: false,
                    });
                }
            }) {
                Ok(_) => {}
                Err(e) => error!("[trigger_action] AddWatcher failed: {:?}", e),
            }
            true
        }

        EditorCommand::RemoveWatcher { node_id, index } => {
            let id = XrdsSceneNodeId(*node_id);
            let index = *index;
            match session.0.edit(|doc| {
                if let Some(node) = doc.node_mut(id) {
                    if index < node.watchers.len() {
                        node.watchers.remove(index);
                    }
                }
            }) {
                Ok(_) => {}
                Err(e) => error!("[trigger_action] RemoveWatcher failed: {:?}", e),
            }
            true
        }

        EditorCommand::SetWatcher { node_id, index, watcher } => {
            let id = XrdsSceneNodeId(*node_id);
            let index = *index;
            let watcher = watcher_from_dto(watcher);
            match session.0.edit(|doc| {
                if let Some(node) = doc.node_mut(id) {
                    if let Some(slot) = node.watchers.get_mut(index) {
                        *slot = watcher;
                    }
                }
            }) {
                Ok(_) => {}
                Err(e) => error!("[trigger_action] SetWatcher failed: {:?}", e),
            }
            true
        }

        EditorCommand::PreviewFireTrigger { node_id, index } => {
            let id = XrdsSceneNodeId(*node_id);
            // Read-only — actually firing needs an XrdsUpdateContext, which
            // only exists in update() (see bevy_scene.rs), not here. Stash
            // what to fire; update() drains it and calls ctx.fire_trigger().
            let binding = session.0.document().node(id)
                .and_then(|node| node.triggers.get(*index));
            match binding {
                Some(b) => {
                    state.pending_fire_trigger = Some((id, b.trigger.clone(), b.hand));
                }
                None => error!("[trigger_action] PreviewFireTrigger: no binding #{index} on node {node_id}"),
            }
            false
        }

        _ => false,
    }
}

// ---------------------------------------------------------------------------
// Step-target resolution
// ---------------------------------------------------------------------------

fn default_action_for_kind(kind: &str) -> Option<XrdsAction> {
    Some(match kind {
        "PlayGltfAnimation" => XrdsAction::PlayGltfAnimation { playback: XrdsSceneGltfPlayback::default() },
        "StopGltfAnimation" => XrdsAction::StopGltfAnimation,
        "SetVisible" => XrdsAction::SetVisible(true),
        "SetTransform" => XrdsAction::SetTransform {
            position: Some([0.0, 0.0, 0.0]),
            rotation: None,
            scale: None,
            duration_secs: 1.0,
            ease: XrdsEaseCurve::default(),
        },
        "SetMaterial" => XrdsAction::SetMaterial {
            base_color: None,
            metallic: None,
            roughness: None,
            texture: None,
        },
        "ModifyHealth" => XrdsAction::ModifyHealth {
            delta: XrdsActionValue::Fixed(0.0),
        },
        _ => return None,
    })
}

// ---------------------------------------------------------------------------
// DTO <-> domain conversions
// ---------------------------------------------------------------------------

fn action_to_dto(a: &XrdsAction) -> XrdsActionDto {
    match a {
        XrdsAction::PlayGltfAnimation { playback } => XrdsActionDto::PlayGltfAnimation {
            clip_index: match playback.selector {
                XrdsSceneGltfAnimationSelector::Index(i) => i,
                XrdsSceneGltfAnimationSelector::Name(_) => 0,
            },
            speed: playback.speed,
            repeat: match playback.repeat {
                XrdsSceneAnimationRepeatMode::Once => "Once".to_string(),
                XrdsSceneAnimationRepeatMode::Loop => "Loop".to_string(),
            },
            start_paused: playback.start_paused,
        },
        XrdsAction::StopGltfAnimation => XrdsActionDto::StopGltfAnimation,
        XrdsAction::SetVisible(v) => XrdsActionDto::SetVisible(*v),
        XrdsAction::SetTransform { position, rotation, scale, duration_secs, ease } =>
            XrdsActionDto::SetTransform {
                position: *position,
                rotation: *rotation,
                scale: *scale,
                duration_secs: *duration_secs,
                ease: ease_curve_to_dto(ease),
            },
        XrdsAction::SetMaterial { base_color, metallic, roughness, texture } => XrdsActionDto::SetMaterial {
            base_color: *base_color,
            metallic: *metallic,
            roughness: *roughness,
            texture: texture.as_ref().map(|t| crate::bridge::ActionTextureDto {
                slot: texture_slot_to_dto(t.slot),
                texture_asset_id: t.texture_asset_id.clone(),
            }),
        },
        XrdsAction::ModifyHealth { delta } => XrdsActionDto::ModifyHealth {
            delta: action_value_to_dto(delta),
        },
        XrdsAction::Unknown => XrdsActionDto::Unknown,
    }
}

fn action_from_dto(a: &XrdsActionDto) -> XrdsAction {
    match a {
        XrdsActionDto::PlayGltfAnimation { clip_index, speed, repeat, start_paused } =>
            XrdsAction::PlayGltfAnimation {
                playback: XrdsSceneGltfPlayback {
                    selector: XrdsSceneGltfAnimationSelector::Index(*clip_index),
                    repeat: if repeat == "Once" {
                        XrdsSceneAnimationRepeatMode::Once
                    } else {
                        XrdsSceneAnimationRepeatMode::Loop
                    },
                    speed: *speed,
                    start_paused: *start_paused,
                },
            },
        XrdsActionDto::StopGltfAnimation => XrdsAction::StopGltfAnimation,
        XrdsActionDto::SetVisible(v) => XrdsAction::SetVisible(*v),
        XrdsActionDto::SetTransform { position, rotation, scale, duration_secs, ease } =>
            XrdsAction::SetTransform {
                position: *position,
                rotation: *rotation,
                scale: *scale,
                duration_secs: *duration_secs,
                ease: ease_curve_from_dto(ease),
            },
        XrdsActionDto::SetMaterial { base_color, metallic, roughness, texture } => XrdsAction::SetMaterial {
            base_color: *base_color,
            metallic: *metallic,
            roughness: *roughness,
            texture: texture.as_ref().map(|t| xrds_scene_graph::XrdsActionTexture {
                slot: texture_slot_from_dto(&t.slot),
                texture_asset_id: t.texture_asset_id.clone(),
            }),
        },
        XrdsActionDto::ModifyHealth { delta } => XrdsAction::ModifyHealth {
            delta: action_value_from_dto(delta),
        },
        XrdsActionDto::Unknown => XrdsAction::Unknown,
    }
}

fn ease_curve_to_dto(e: &XrdsEaseCurve) -> String {
    match e {
        XrdsEaseCurve::Linear => "Linear".to_string(),
        XrdsEaseCurve::Quad => "Quad".to_string(),
        XrdsEaseCurve::Cubic => "Cubic".to_string(),
    }
}

fn ease_curve_from_dto(e: &str) -> XrdsEaseCurve {
    match e {
        "Linear" => XrdsEaseCurve::Linear,
        "Quad" => XrdsEaseCurve::Quad,
        _ => XrdsEaseCurve::Cubic,
    }
}

fn texture_slot_to_dto(s: xrds_scene_graph::XrdsSceneMaterialTextureSlotKind) -> String {
    use xrds_scene_graph::XrdsSceneMaterialTextureSlotKind as K;
    match s {
        K::BaseColor => "BaseColor",
        K::MetallicRoughness => "MetallicRoughness",
        K::Normal => "Normal",
        K::Occlusion => "Occlusion",
        K::Emissive => "Emissive",
    }
    .to_string()
}

/// Unknown slot names fall back to `BaseColor` rather than failing the command
/// — same lenient-parse convention as `ease`/`repeat` here. A bad slot string
/// can only come from a bridge-version mismatch, which the banner already
/// reports.
pub fn texture_slot_from_dto(s: &str) -> xrds_scene_graph::XrdsSceneMaterialTextureSlotKind {
    use xrds_scene_graph::XrdsSceneMaterialTextureSlotKind as K;
    match s {
        "MetallicRoughness" => K::MetallicRoughness,
        "Normal" => K::Normal,
        "Occlusion" => K::Occlusion,
        "Emissive" => K::Emissive,
        _ => K::BaseColor,
    }
}

// NOTE: no asset-catalog builder here. `EditorSnapshot.asset_catalog` already
// existed (built by `palette::build_asset_catalog`, whose `kind` is already
// "Texture"/"Gltf"/…), so the texture picker consumes that rather than a
// second parallel DTO for the same data.

fn action_target_to_dto(t: &XrdsActionTarget) -> ActionTargetDto {
    match t {
        XrdsActionTarget::SelfNode => ActionTargetDto::SelfNode,
        XrdsActionTarget::Node(id) => ActionTargetDto::Node { id: id.0 },
        XrdsActionTarget::TriggerSource => ActionTargetDto::TriggerSource,
    }
}


fn action_value_to_dto(v: &XrdsActionValue) -> ActionValueDto {
    match v {
        XrdsActionValue::Fixed(value) => ActionValueDto::Fixed { value: *value },
        XrdsActionValue::FromTriggerSource => ActionValueDto::FromTriggerSource,
    }
}

fn action_value_from_dto(v: &ActionValueDto) -> XrdsActionValue {
    match v {
        ActionValueDto::Fixed { value } => XrdsActionValue::Fixed(*value),
        ActionValueDto::FromTriggerSource => XrdsActionValue::FromTriggerSource,
    }
}

fn track_key_to_dto(k: &XrdsTrackKey) -> XrdsTrackKeyDto {
    XrdsTrackKeyDto { at_secs: k.at_secs, action: action_to_dto(&k.action) }
}

fn track_key_from_dto(k: &XrdsTrackKeyDto) -> XrdsTrackKey {
    XrdsTrackKey { at_secs: k.at_secs, action: action_from_dto(&k.action) }
}

fn trigger_kind_to_dto(k: &XrdsTriggerKind) -> XrdsTriggerKindDto {
    match k {
        XrdsTriggerKind::ZoneEnter => XrdsTriggerKindDto::ZoneEnter,
        XrdsTriggerKind::ZoneExit => XrdsTriggerKindDto::ZoneExit,
        XrdsTriggerKind::Grabbed => XrdsTriggerKindDto::Grabbed,
        XrdsTriggerKind::Dropped => XrdsTriggerKindDto::Dropped,
        XrdsTriggerKind::HoverEnter => XrdsTriggerKindDto::HoverEnter,
        XrdsTriggerKind::HoverExit => XrdsTriggerKindDto::HoverExit,
        XrdsTriggerKind::ButtonPress => XrdsTriggerKindDto::ButtonPress,
        XrdsTriggerKind::ButtonRelease => XrdsTriggerKindDto::ButtonRelease,
        XrdsTriggerKind::SliderChange => XrdsTriggerKindDto::SliderChange,
        XrdsTriggerKind::ToggleChange => XrdsTriggerKindDto::ToggleChange,
        XrdsTriggerKind::AnimationComplete => XrdsTriggerKindDto::AnimationComplete,
        XrdsTriggerKind::RunawayDetected => XrdsTriggerKindDto::RunawayDetected,
        XrdsTriggerKind::Custom(name) => XrdsTriggerKindDto::Custom(name.clone()),
        XrdsTriggerKind::Unknown => XrdsTriggerKindDto::Unknown,
    }
}

fn trigger_kind_from_dto(k: &XrdsTriggerKindDto) -> XrdsTriggerKind {
    match k {
        XrdsTriggerKindDto::ZoneEnter => XrdsTriggerKind::ZoneEnter,
        XrdsTriggerKindDto::ZoneExit => XrdsTriggerKind::ZoneExit,
        XrdsTriggerKindDto::Grabbed => XrdsTriggerKind::Grabbed,
        XrdsTriggerKindDto::Dropped => XrdsTriggerKind::Dropped,
        XrdsTriggerKindDto::HoverEnter => XrdsTriggerKind::HoverEnter,
        XrdsTriggerKindDto::HoverExit => XrdsTriggerKind::HoverExit,
        XrdsTriggerKindDto::ButtonPress => XrdsTriggerKind::ButtonPress,
        XrdsTriggerKindDto::ButtonRelease => XrdsTriggerKind::ButtonRelease,
        XrdsTriggerKindDto::SliderChange => XrdsTriggerKind::SliderChange,
        XrdsTriggerKindDto::ToggleChange => XrdsTriggerKind::ToggleChange,
        XrdsTriggerKindDto::AnimationComplete => XrdsTriggerKind::AnimationComplete,
        XrdsTriggerKindDto::RunawayDetected => XrdsTriggerKind::RunawayDetected,
        XrdsTriggerKindDto::Custom(name) => XrdsTriggerKind::Custom(name.clone()),
        XrdsTriggerKindDto::Unknown => XrdsTriggerKind::Unknown,
    }
}

fn hand_to_dto(h: Option<XrGrabHand>) -> Option<String> {
    h.map(|h| match h {
        XrGrabHand::Left => "Left".to_string(),
        XrGrabHand::Right => "Right".to_string(),
    })
}

fn hand_from_dto(h: &Option<String>) -> Option<XrGrabHand> {
    match h.as_deref() {
        Some("Left") => Some(XrGrabHand::Left),
        Some("Right") => Some(XrGrabHand::Right),
        _ => None,
    }
}

fn binding_to_dto(b: &XrdsTriggerBinding) -> TriggerBindingDto {
    TriggerBindingDto {
        trigger: trigger_kind_to_dto(&b.trigger),
        disabled: b.disabled,
        hand: hand_to_dto(b.hand),
        track: b.track.clone(),
    }
}

fn observable_to_dto(o: &XrdsObservable) -> ObservableDto {
    match o {
        XrdsObservable::RotationDegrees { axis } => ObservableDto::RotationDegrees {
            axis: match axis {
                XrdsAxis::X => "X".to_string(),
                XrdsAxis::Y => "Y".to_string(),
                XrdsAxis::Z => "Z".to_string(),
            },
        },
        XrdsObservable::DistanceTo { node } => ObservableDto::DistanceTo { node: node.0 },
        XrdsObservable::Height => ObservableDto::Height,
        XrdsObservable::ScaleMagnitude => ObservableDto::ScaleMagnitude,
    }
}

fn observable_from_dto(o: &ObservableDto) -> XrdsObservable {
    match o {
        ObservableDto::RotationDegrees { axis } => XrdsObservable::RotationDegrees {
            axis: match axis.as_str() {
                "X" => XrdsAxis::X,
                "Z" => XrdsAxis::Z,
                _ => XrdsAxis::Y,
            },
        },
        ObservableDto::DistanceTo { node } => XrdsObservable::DistanceTo { node: XrdsSceneNodeId(*node) },
        ObservableDto::Height => XrdsObservable::Height,
        ObservableDto::ScaleMagnitude => XrdsObservable::ScaleMagnitude,
    }
}

fn watcher_to_dto(w: &XrdsThresholdWatcher) -> ThresholdWatcherDto {
    ThresholdWatcherDto {
        observable: observable_to_dto(&w.observable),
        crossing: match w.crossing {
            XrdsCrossing::Above => "Above".to_string(),
            XrdsCrossing::Below => "Below".to_string(),
            XrdsCrossing::Either => "Either".to_string(),
        },
        value: w.value,
        hysteresis: w.hysteresis,
        fires: w.fires.clone(),
        disabled: w.disabled,
    }
}

fn watcher_from_dto(w: &ThresholdWatcherDto) -> XrdsThresholdWatcher {
    XrdsThresholdWatcher {
        observable: observable_from_dto(&w.observable),
        crossing: match w.crossing.as_str() {
            "Above" => XrdsCrossing::Above,
            "Below" => XrdsCrossing::Below,
            _ => XrdsCrossing::Either,
        },
        value: w.value,
        hysteresis: w.hysteresis,
        fires: w.fires.clone(),
        disabled: w.disabled,
    }
}

fn diagnostic_to_dto(d: &XrdsSceneTriggerDiagnostic) -> TriggerDiagnosticDto {
    TriggerDiagnosticDto {
        node_id: d.node_id.map(|id| id.0),
        severity: match d.severity {
            XrdsSceneTriggerDiagnosticSeverity::Info => "info".to_string(),
            XrdsSceneTriggerDiagnosticSeverity::Warning => "warning".to_string(),
            XrdsSceneTriggerDiagnosticSeverity::Error => "error".to_string(),
        },
        title: d.title.clone(),
        detail: d.detail.clone(),
    }
}
