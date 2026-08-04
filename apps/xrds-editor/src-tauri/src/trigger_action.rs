//! Editor bridge for trigger-action authoring (Phase 6 — see
//! `docs/done/xrds-trigger-action-editor-plan.md`). Mirrors `hud_library.rs`'s
//! shape: a snapshot serializer plus a command dispatcher, both operating
//! on the exact same `XrdsSceneDocument` data the runtime already consumes
//! (`XrdsSceneDocument::runnables`, `XrdsSceneNode::triggers`/`.watchers`).

use bevy::log::error;
use xrds_components::XrGrabHand;
use xrds_scene_graph::{
    XrdsAction, XrdsActionTarget, XrdsActionValue, XrdsAxis, XrdsCrossing, XrdsNamedRunnable,
    XrdsObservable, XrdsRunnable, XrdsSceneAnimationRepeatMode, XrdsSceneDocument,
    XrdsSceneGltfAnimationSelector, XrdsSceneGltfPlayback, XrdsSceneNodeId,
    XrdsSceneTriggerDiagnostic, XrdsSceneTriggerDiagnosticSeverity, XrdsSequence, XrdsThresholdWatcher,
    XrdsTimeline, XrdsTimelineKey, XrdsTriggerBinding, XrdsTriggerKind,
};
use crate::bridge::{
    ActionTargetDto, ActionValueDto, EditorCommand, NamedRunnableDto, ObservableDto,
    RunnableBodyDto, StepTargetDto, ThresholdWatcherDto, TriggerBindingDto, TriggerDiagnosticDto,
    XrdsActionDto, XrdsSequenceDto, XrdsTimelineKeyDto, XrdsTriggerKindDto,
};
use crate::editor_state::{EditorSession, EditorState};

// ---------------------------------------------------------------------------
// Snapshot serializers
// ---------------------------------------------------------------------------

pub fn build_runnables_dto(doc: &XrdsSceneDocument) -> Vec<NamedRunnableDto> {
    doc.runnables.iter().map(|r| NamedRunnableDto {
        name: r.name.clone(),
        body: match &r.runnable {
            XrdsRunnable::Sequence(seq) => RunnableBodyDto::Sequence {
                steps: seq.steps.iter().map(action_to_dto).collect(),
            },
            XrdsRunnable::Timeline(tl) => RunnableBodyDto::Timeline {
                keys: tl.keys.iter().map(timeline_key_to_dto).collect(),
                duration_secs: tl.duration_secs,
                looping: tl.looping,
            },
        },
    }).collect()
}

/// Registry-level diagnostics only (`node_id: None`) — for `EditorSnapshot::runnable_diagnostics`.
pub fn build_runnable_diagnostics_dto(doc: &XrdsSceneDocument) -> Vec<TriggerDiagnosticDto> {
    doc.trigger_diagnostics().iter()
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

pub fn build_node_trigger_diagnostics_dto(
    doc: &XrdsSceneDocument,
    id: XrdsSceneNodeId,
) -> Vec<TriggerDiagnosticDto> {
    doc.trigger_diagnostics().iter()
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
        // --- Registry ---
        EditorCommand::CreateRunnable { name, kind } => {
            let name = name.clone();
            let Some(runnable) = default_runnable_for_kind(kind) else {
                error!("[trigger_action] CreateRunnable: unknown kind {kind:?}");
                return false;
            };
            match session.0.edit(|doc| {
                if doc.runnable(&name).is_some() {
                    error!("[trigger_action] CreateRunnable: {name:?} already exists");
                    return;
                }
                doc.runnables.push(XrdsNamedRunnable { name, runnable });
            }) {
                Ok(_) => {}
                Err(e) => error!("[trigger_action] CreateRunnable failed: {:?}", e),
            }
            false
        }

        EditorCommand::DeleteRunnable { name } => {
            let name = name.clone();
            match session.0.edit(|doc| {
                doc.runnables.retain(|r| r.name != name);
            }) {
                Ok(_) => {}
                Err(e) => error!("[trigger_action] DeleteRunnable failed: {:?}", e),
            }
            true // bindings/Run steps naming this runnable now dangle — reimport to be safe
        }

        EditorCommand::RenameRunnable { old_name, new_name } => {
            let old_name = old_name.clone();
            let new_name = new_name.clone();
            match session.0.edit(|doc| {
                if doc.runnable(&new_name).is_some() {
                    error!("[trigger_action] RenameRunnable: {new_name:?} already exists");
                    return;
                }
                if let Some(r) = doc.runnable_mut(&old_name) {
                    r.name = new_name;
                }
            }) {
                Ok(_) => {}
                Err(e) => error!("[trigger_action] RenameRunnable failed: {:?}", e),
            }
            false
        }

        EditorCommand::SetTimelineLooping { name, looping } => {
            let name = name.clone();
            let looping = *looping;
            match session.0.edit(|doc| {
                if let Some(XrdsRunnable::Timeline(tl)) = doc.runnable_mut(&name).map(|r| &mut r.runnable) {
                    tl.looping = looping;
                }
            }) {
                Ok(_) => {}
                Err(e) => error!("[trigger_action] SetTimelineLooping failed: {:?}", e),
            }
            false
        }

        EditorCommand::SetTimelineDuration { name, duration_secs } => {
            let name = name.clone();
            let duration_secs = *duration_secs;
            match session.0.edit(|doc| {
                if let Some(XrdsRunnable::Timeline(tl)) = doc.runnable_mut(&name).map(|r| &mut r.runnable) {
                    tl.duration_secs = duration_secs;
                }
            }) {
                Ok(_) => {}
                Err(e) => error!("[trigger_action] SetTimelineDuration failed: {:?}", e),
            }
            false
        }

        // --- Steps (registry sequence body OR a binding's inline sequence) ---
        EditorCommand::AddActionStep { target, kind } => {
            let target = target.clone();
            let Some(action) = default_action_for_kind(kind) else {
                error!("[trigger_action] AddActionStep: unknown kind {kind:?}");
                return true;
            };
            match session.0.edit(|doc| {
                if let Some(steps) = resolve_step_target_mut(doc, &target) {
                    steps.push(action);
                }
            }) {
                Ok(_) => {}
                Err(e) => error!("[trigger_action] AddActionStep failed: {:?}", e),
            }
            true
        }

        EditorCommand::RemoveActionStep { target, index } => {
            let target = target.clone();
            let index = *index;
            match session.0.edit(|doc| {
                if let Some(steps) = resolve_step_target_mut(doc, &target) {
                    if index < steps.len() {
                        steps.remove(index);
                    }
                }
            }) {
                Ok(_) => {}
                Err(e) => error!("[trigger_action] RemoveActionStep failed: {:?}", e),
            }
            true
        }

        EditorCommand::MoveActionStep { target, index, delta } => {
            let target = target.clone();
            let index = *index;
            let delta = *delta;
            match session.0.edit(|doc| {
                if let Some(steps) = resolve_step_target_mut(doc, &target) {
                    let len = steps.len() as i64;
                    let dst = index as i64 + delta as i64;
                    if (index as i64) < len && dst >= 0 && dst < len {
                        steps.swap(index, dst as usize);
                    }
                }
            }) {
                Ok(_) => {}
                Err(e) => error!("[trigger_action] MoveActionStep failed: {:?}", e),
            }
            true
        }

        EditorCommand::SetActionStep { target, index, action } => {
            let target = target.clone();
            let index = *index;
            let action = action_from_dto(action);
            match session.0.edit(|doc| {
                if let Some(steps) = resolve_step_target_mut(doc, &target) {
                    if let Some(slot) = steps.get_mut(index) {
                        *slot = action;
                    }
                }
            }) {
                Ok(_) => {}
                Err(e) => error!("[trigger_action] SetActionStep failed: {:?}", e),
            }
            true
        }

        // --- Timeline keys (registry timeline body only) ---
        EditorCommand::AddTimelineKey { name, at_secs, kind } => {
            let name = name.clone();
            let at_secs = *at_secs;
            let Some(action) = default_action_for_kind(kind) else {
                error!("[trigger_action] AddTimelineKey: unknown kind {kind:?}");
                return true;
            };
            match session.0.edit(|doc| {
                if let Some(XrdsRunnable::Timeline(tl)) = doc.runnable_mut(&name).map(|r| &mut r.runnable) {
                    tl.keys.push(XrdsTimelineKey { at_secs, action });
                }
            }) {
                Ok(_) => {}
                Err(e) => error!("[trigger_action] AddTimelineKey failed: {:?}", e),
            }
            true
        }

        EditorCommand::RemoveTimelineKey { name, index } => {
            let name = name.clone();
            let index = *index;
            match session.0.edit(|doc| {
                if let Some(XrdsRunnable::Timeline(tl)) = doc.runnable_mut(&name).map(|r| &mut r.runnable) {
                    if index < tl.keys.len() {
                        tl.keys.remove(index);
                    }
                }
            }) {
                Ok(_) => {}
                Err(e) => error!("[trigger_action] RemoveTimelineKey failed: {:?}", e),
            }
            true
        }

        EditorCommand::SetTimelineKey { name, index, key } => {
            let name = name.clone();
            let index = *index;
            let key = timeline_key_from_dto(key);
            match session.0.edit(|doc| {
                if let Some(XrdsRunnable::Timeline(tl)) = doc.runnable_mut(&name).map(|r| &mut r.runnable) {
                    if let Some(slot) = tl.keys.get_mut(index) {
                        *slot = key;
                    }
                }
            }) {
                Ok(_) => {}
                Err(e) => error!("[trigger_action] SetTimelineKey failed: {:?}", e),
            }
            true
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

        EditorCommand::SetTriggerBindingRunnable { node_id, index, runnable } => {
            let id = XrdsSceneNodeId(*node_id);
            let index = *index;
            let runnable = runnable.clone();
            match session.0.edit(|doc| {
                if let Some(node) = doc.node_mut(id) {
                    if let Some(b) = node.triggers.get_mut(index) {
                        b.runnable = runnable;
                    }
                }
            }) {
                Ok(_) => {}
                Err(e) => error!("[trigger_action] SetTriggerBindingRunnable failed: {:?}", e),
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

/// Resolves a `StepTargetDto` to the live `Vec<XrdsAction>` it addresses.
/// `Runnable` only resolves for a `Sequence` body — a `Timeline`'s actions
/// live inside its keys, addressed by the `*TimelineKey` commands instead.
fn resolve_step_target_mut<'a>(
    doc: &'a mut XrdsSceneDocument,
    target: &StepTargetDto,
) -> Option<&'a mut Vec<XrdsAction>> {
    match target {
        StepTargetDto::Runnable { name } => match &mut doc.runnable_mut(name)?.runnable {
            XrdsRunnable::Sequence(seq) => Some(&mut seq.steps),
            XrdsRunnable::Timeline(_) => None,
        },
        StepTargetDto::Binding { node_id, binding_index } => {
            let node = doc.node_mut(XrdsSceneNodeId(*node_id))?;
            Some(&mut node.triggers.get_mut(*binding_index)?.sequence.steps)
        }
    }
}

fn default_runnable_for_kind(kind: &str) -> Option<XrdsRunnable> {
    Some(match kind {
        "sequence" => XrdsRunnable::Sequence(XrdsSequence::default()),
        "timeline" => XrdsRunnable::Timeline(XrdsTimeline::default()),
        _ => return None,
    })
}

fn default_action_for_kind(kind: &str) -> Option<XrdsAction> {
    Some(match kind {
        "PlayGltfAnimation" => XrdsAction::PlayGltfAnimation { playback: XrdsSceneGltfPlayback::default() },
        "StopGltfAnimation" => XrdsAction::StopGltfAnimation,
        "SetVisible" => XrdsAction::SetVisible(true),
        "Teleport" => XrdsAction::Teleport { destination: [0.0, 0.0, 0.0] },
        "ModifyHealth" => XrdsAction::ModifyHealth {
            target: XrdsActionTarget::SelfNode,
            delta: XrdsActionValue::Fixed(0.0),
        },
        "Wait" => XrdsAction::Wait { seconds: 1.0 },
        "FireCustomEvent" => XrdsAction::FireCustomEvent { name: "event".to_string() },
        "Run" => XrdsAction::Run { runnable: String::new(), wait: true },
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
        XrdsAction::Teleport { destination } => XrdsActionDto::Teleport { destination: *destination },
        XrdsAction::ModifyHealth { target, delta } => XrdsActionDto::ModifyHealth {
            target: action_target_to_dto(target),
            delta: action_value_to_dto(delta),
        },
        XrdsAction::Wait { seconds } => XrdsActionDto::Wait { seconds: *seconds },
        XrdsAction::FireCustomEvent { name } => XrdsActionDto::FireCustomEvent { name: name.clone() },
        XrdsAction::Run { runnable, wait } => XrdsActionDto::Run { runnable: runnable.clone(), wait: *wait },
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
        XrdsActionDto::Teleport { destination } => XrdsAction::Teleport { destination: *destination },
        XrdsActionDto::ModifyHealth { target, delta } => XrdsAction::ModifyHealth {
            target: action_target_from_dto(target),
            delta: action_value_from_dto(delta),
        },
        XrdsActionDto::Wait { seconds } => XrdsAction::Wait { seconds: *seconds },
        XrdsActionDto::FireCustomEvent { name } => XrdsAction::FireCustomEvent { name: name.clone() },
        XrdsActionDto::Run { runnable, wait } => XrdsAction::Run { runnable: runnable.clone(), wait: *wait },
        XrdsActionDto::Unknown => XrdsAction::Unknown,
    }
}

fn action_target_to_dto(t: &XrdsActionTarget) -> ActionTargetDto {
    match t {
        XrdsActionTarget::SelfNode => ActionTargetDto::SelfNode,
        XrdsActionTarget::Node(id) => ActionTargetDto::Node { id: id.0 },
        XrdsActionTarget::TriggerSource => ActionTargetDto::TriggerSource,
    }
}

fn action_target_from_dto(t: &ActionTargetDto) -> XrdsActionTarget {
    match t {
        ActionTargetDto::SelfNode => XrdsActionTarget::SelfNode,
        ActionTargetDto::Node { id } => XrdsActionTarget::Node(XrdsSceneNodeId(*id)),
        ActionTargetDto::TriggerSource => XrdsActionTarget::TriggerSource,
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

fn timeline_key_to_dto(k: &XrdsTimelineKey) -> XrdsTimelineKeyDto {
    XrdsTimelineKeyDto { at_secs: k.at_secs, action: action_to_dto(&k.action) }
}

fn timeline_key_from_dto(k: &XrdsTimelineKeyDto) -> XrdsTimelineKey {
    XrdsTimelineKey { at_secs: k.at_secs, action: action_from_dto(&k.action) }
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
        sequence: XrdsSequenceDto {
            steps: b.sequence.steps.iter().map(action_to_dto).collect(),
        },
        disabled: b.disabled,
        hand: hand_to_dto(b.hand),
        runnable: b.runnable.clone(),
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
