//! Trigger-action sequencing runtime.
//!
//! See `docs/xrds-scenegraph-trigger-action-sequencing.md` for the design
//! and `docs/xrds-trigger-action-implementation-plan.md` for the phased
//! build-out (this file is Phases 3-4).
//!
//! Two collaborating-but-separate systems live here:
//!
//! 1. **Trigger-action** — [`XrdsTriggerEvent`] + [`consume_triggers`]:
//!    an open, pluggable mechanism where any message type can fire a
//!    sequence by implementing one trait. Never suppresses: every firing
//!    of every trigger produces its own independent sequence run, matching
//!    Unity/Unreal/Godot's convention of always reporting the event and
//!    letting the consumer decide.
//! 2. **Sequencing** — [`XrdsActionRunner`] on top of
//!    `bevy-sequential-actions`: an ordered action queue per *ephemeral
//!    agent entity*, one spawned per trigger firing and despawned when its
//!    queue drains.

use super::*;
use bevy::prelude::*;
use bevy_sequential_actions::*;
use xrds_scene_graph::{
    XrdsAction, XrdsActionTarget, XrdsActionValue, XrdsSceneAnimationRepeatMode,
    XrdsSceneGltfAnimationSelector, XrdsSceneGltfPlayback, XrdsSequence, XrdsTriggerBinding,
    XrdsTriggerKind,
};

/// Runtime component holding a node's authored trigger bindings — spawned
/// at scene-document import (see `reimport::tag_trigger_binding_entities`),
/// carrying `xrds_scene_graph::XrdsTriggerBinding` data directly (no
/// runtime-side mirror type needed: `xrds-runtime` already depends on
/// `xrds-scene-graph`).
///
/// Inert until a matching trigger event actually arrives — nothing is
/// enqueued at import time.
#[derive(Component, Debug, Clone, Default)]
pub struct XrdsTriggerBindings(pub Vec<XrdsTriggerBinding>);

/// Generic numeric payload read by [`XrdsActionValue::FromTriggerSource`].
///
/// **Ordinary gameplay code is responsible for inserting this** with a
/// meaningful value — e.g. a bullet's fire-system setting its own damage
/// amount when it spawns. The trigger-action layer only ever *reads* it,
/// never computes it: deciding what the number means (weapon type,
/// upgrades, falloff) is gameplay logic, which stays outside this layer by
/// design.
///
/// If an action asks for `FromTriggerSource` and the source entity has no
/// `XrdsTriggerValue`, the action degrades to `0.0` with a warning rather
/// than panicking.
#[derive(Component, Debug, Clone, Copy)]
pub struct XrdsTriggerValue(pub f32);

/// Plain numeric health slot mutated by [`XrdsAction::ModifyHealth`].
///
/// Like [`XrdsTriggerValue`], this is a data slot only — the SDK provides
/// somewhere to put the number and an action that changes it. Reacting to
/// it (death, respawn, UI) is gameplay code's job, per the scope boundary
/// in the design doc.
#[derive(Component, Debug, Clone, Copy)]
pub struct XrdsHealth(pub f32);

/// Emitted by [`XrdsAction::FireCustomEvent`] — the escape hatch into
/// expert-layer Rust for behavior not modeled as a first-class action.
#[derive(Message, Debug, Clone)]
pub struct XrdsCustomTriggerEvent {
    pub name: String,
    /// The node the sequence was authored on.
    pub target: Entity,
    /// Whatever caused the trigger to fire, when distinct from `target`.
    pub source: Option<Entity>,
}

/// Fired when every glTF animation on a node has run to its end.
///
/// Not emitted for `Loop` playback (a looping clip has no completion) nor
/// for playback ended by an explicit `stop_gltf_animation` — only for a
/// clip that finished on its own. Emitted by
/// [`sync_completed_gltf_animation_triggers`], which is also what keeps
/// `gltf_animation_state()`'s `playing` flag honest.
///
/// Usable directly as a Bevy message, or — since it implements
/// [`XrdsTriggerEvent`] — as an authored
/// `XrdsTriggerKind::AnimationComplete` trigger.
#[derive(Message, Debug, Clone, Copy)]
pub struct XrdsGltfAnimationCompleteEvent {
    /// XRDS id of the node whose animation finished.
    pub node_id: XrdsId,
}

impl XrdsTriggerEvent for XrdsGltfAnimationCompleteEvent {
    fn target(&self) -> XrdsId {
        self.node_id
    }
    fn kind(&self) -> XrdsTriggerKind {
        XrdsTriggerKind::AnimationComplete
    }
}

/// Detects completed glTF playback, corrects the cached
/// `XrdsGltfAnimationState.playing` flag, and emits
/// [`XrdsGltfAnimationCompleteEvent`] so it can drive authored sequences.
///
/// Runs in `Last`, by which point Bevy's animation systems have advanced
/// this frame's playback. The resulting trigger is therefore consumed on
/// the following frame's `Update` — messages live long enough for that, so
/// the one-frame latency is the only cost.
pub fn sync_completed_gltf_animation_triggers(world: &mut World) {
    let completed = crate::xrds_api::helper::sync_completed_gltf_animations_in_world(world);
    if completed.is_empty() {
        return;
    }

    let node_ids: Vec<XrdsId> = {
        let index = world.resource::<XrdsIdIndex>();
        completed
            .into_iter()
            .filter_map(|entity| index.id_of(entity))
            .collect()
    };

    for node_id in node_ids {
        world.write_message(XrdsGltfAnimationCompleteEvent { node_id });
    }
}

/// Marks an ephemeral per-firing agent entity and records which entities
/// its actions apply to. One is spawned per trigger firing and despawned
/// by [`despawn_finished_sequence_agents`] once its queue drains.
#[derive(Component, Debug, Clone, Copy)]
pub struct XrdsSequenceAgent {
    pub target: Entity,
    pub source: Option<Entity>,
}

/// Any message that should be able to fire an authored sequence
/// implements this. Adding a new trigger source (e.g. an `avian3d`
/// collision event) is one trait impl plus one
/// `consume_triggers::<E>` registration — the data model and consumer
/// logic never change.
///
/// Methods work in `XrdsId`, not `Entity`, because the existing zone
/// events carry stable XRDS ids; [`consume_triggers`] resolves them
/// through [`XrdsIdIndex`].
pub trait XrdsTriggerEvent: Message {
    /// Whose bindings to check — the node the sequence is authored on.
    fn target(&self) -> XrdsId;

    /// What *caused* the trigger, when meaningfully distinct from the
    /// target (e.g. the bullet that hit the player). Defaults to the
    /// target so sources with no separate cause — a timer, say — don't
    /// have to think about it.
    fn source(&self) -> XrdsId {
        self.target()
    }

    fn kind(&self) -> XrdsTriggerKind;
}

impl XrdsTriggerEvent for xrds_components::XrZoneEnterEvent {
    fn target(&self) -> XrdsId {
        self.zone_id
    }
    fn source(&self) -> XrdsId {
        self.entity_id
    }
    fn kind(&self) -> XrdsTriggerKind {
        XrdsTriggerKind::ZoneEnter
    }
}

impl XrdsTriggerEvent for xrds_components::XrZoneExitEvent {
    fn target(&self) -> XrdsId {
        self.zone_id
    }
    fn source(&self) -> XrdsId {
        self.entity_id
    }
    fn kind(&self) -> XrdsTriggerKind {
        XrdsTriggerKind::ZoneExit
    }
}

/// Registered once per [`XrdsTriggerEvent`] implementor. Spawns one
/// ephemeral agent per matching binding per firing — never dedupes or
/// suppresses, so two different sources firing the same trigger run
/// independently instead of queueing behind each other.
pub fn consume_triggers<E: XrdsTriggerEvent>(
    mut events: MessageReader<E>,
    bindings: Query<&XrdsTriggerBindings>,
    id_index: Res<XrdsIdIndex>,
    mut commands: Commands,
) {
    for event in events.read() {
        let Some(target) = id_index.entity_of(event.target()) else {
            continue;
        };
        let Ok(node_bindings) = bindings.get(target) else {
            continue;
        };
        let source = id_index.entity_of(event.source());
        let kind = event.kind();

        for binding in node_bindings.0.iter().filter(|b| b.trigger == kind) {
            spawn_sequence_agent(&mut commands, target, source, &binding.sequence);
        }
    }
}

/// Spawns one ephemeral agent carrying the whole sequence. Public so
/// expert-layer code and tests can kick off an authored sequence directly
/// without going through a trigger.
pub fn spawn_sequence_agent(
    commands: &mut Commands,
    target: Entity,
    source: Option<Entity>,
    sequence: &XrdsSequence,
) {
    if sequence.steps.is_empty() {
        return;
    }

    let agent = commands
        .spawn((SequentialActions, XrdsSequenceAgent { target, source }))
        .id();

    let runners: Vec<BoxedAction> = sequence
        .steps
        .iter()
        .map(|action| {
            Box::new(XrdsActionRunner::new(action.clone(), target, source)) as BoxedAction
        })
        .collect();

    commands.actions(agent).add(runners);
}

/// Despawns ephemeral agents whose queue has fully drained. Without this
/// every trigger firing would leak an entity.
pub fn despawn_finished_sequence_agents(
    agents: Query<(Entity, &ActionQueue, &CurrentAction), With<XrdsSequenceAgent>>,
    mut commands: Commands,
) {
    for (entity, queue, current) in &agents {
        if queue.is_empty() && current.is_none() {
            commands.entity(entity).despawn();
        }
    }
}

/// Bridges one authored [`XrdsAction`] into `bevy-sequential-actions`'
/// [`Action`] trait. Actions apply to the *target*/*source* entities
/// recorded here, never to the agent entity the queue lives on.
pub struct XrdsActionRunner {
    action: XrdsAction,
    target: Entity,
    source: Option<Entity>,
    /// Only used by `Wait` — `is_finished` gets `&self`, so it can't tick
    /// a `Timer`; storing an absolute deadline is the workaround (same
    /// pattern as `examples/expert/sequential_actions_spike.rs`).
    deadline_secs: Option<f32>,
}

impl XrdsActionRunner {
    pub fn new(action: XrdsAction, target: Entity, source: Option<Entity>) -> Self {
        Self {
            action,
            target,
            source,
            deadline_secs: None,
        }
    }

    /// Resolves an authored target selector to a live entity.
    fn resolve_target(&self, selector: &XrdsActionTarget, world: &World) -> Option<Entity> {
        match selector {
            XrdsActionTarget::SelfNode => Some(self.target),
            XrdsActionTarget::TriggerSource => self.source,
            XrdsActionTarget::Node(node_id) => world
                .get_resource::<XrdsIdIndex>()
                .and_then(|index| index.entity_of((*node_id).into())),
        }
    }

    /// Resolves an authored value, reading [`XrdsTriggerValue`] off the
    /// source entity for `FromTriggerSource`. Degrades to `0.0` with a
    /// warning when the slot is absent — gameplay code owns populating it.
    fn resolve_value(&self, value: &XrdsActionValue, world: &World) -> f32 {
        match value {
            XrdsActionValue::Fixed(v) => *v,
            XrdsActionValue::FromTriggerSource => {
                let slot = self
                    .source
                    .and_then(|source| world.get::<XrdsTriggerValue>(source));
                match slot {
                    Some(XrdsTriggerValue(v)) => *v,
                    None => {
                        log::warn!(
                            "XrdsAction asked for FromTriggerSource but the trigger source \
                             {:?} has no XrdsTriggerValue component — using 0.0. Gameplay \
                             code is responsible for inserting it.",
                            self.source
                        );
                        0.0
                    }
                }
            }
        }
    }
}

fn runtime_animation_selector(
    selector: &XrdsSceneGltfAnimationSelector,
) -> XrdsGltfAnimationSelector {
    match selector {
        XrdsSceneGltfAnimationSelector::Index(i) => XrdsGltfAnimationSelector::Index(*i),
        XrdsSceneGltfAnimationSelector::Name(name) => {
            XrdsGltfAnimationSelector::Name(name.clone())
        }
    }
}

fn runtime_playback_options(playback: &XrdsSceneGltfPlayback) -> XrdsGltfAnimationPlaybackOptions {
    XrdsGltfAnimationPlaybackOptions {
        repeat: match playback.repeat {
            XrdsSceneAnimationRepeatMode::Once => XrdsAnimationRepeatMode::Once,
            XrdsSceneAnimationRepeatMode::Loop => XrdsAnimationRepeatMode::Loop,
        },
        speed: playback.speed,
        start_paused: playback.start_paused,
    }
}

impl Action for XrdsActionRunner {
    fn is_finished(&self, _agent: Entity, world: &World) -> bool {
        match (&self.action, self.deadline_secs) {
            (XrdsAction::Wait { .. }, Some(deadline)) => {
                world.resource::<Time>().elapsed_secs() >= deadline
            }
            // Everything else applies instantly in `on_start`. glTF
            // playback is fire-and-forget for v1 — the sequence advances
            // as soon as playback is requested rather than waiting for the
            // clip to finish. Waiting on clip completion needs a
            // `Wait`-style poll against gltf_animation_state and is
            // tracked as a follow-up, not silently assumed here.
            _ => true,
        }
    }

    fn on_start(&mut self, _agent: Entity, world: &mut World) -> bool {
        match self.action.clone() {
            XrdsAction::Wait { seconds } => {
                let now = world.resource::<Time>().elapsed_secs();
                self.deadline_secs = Some(now + seconds);
                seconds <= 0.0
            }

            XrdsAction::SetVisible(visible) => {
                if let Some(mut visibility) = world.get_mut::<Visibility>(self.target) {
                    *visibility = if visible {
                        Visibility::Inherited
                    } else {
                        Visibility::Hidden
                    };
                }
                true
            }

            XrdsAction::Teleport { destination } => {
                if let Some(mut transform) = world.get_mut::<Transform>(self.target) {
                    transform.translation = Vec3::from_array(destination);
                }
                true
            }

            XrdsAction::ModifyHealth { target, delta } => {
                let amount = self.resolve_value(&delta, world);
                if let Some(entity) = self.resolve_target(&target, world) {
                    if let Some(mut health) = world.get_mut::<XrdsHealth>(entity) {
                        health.0 += amount;
                    } else {
                        log::warn!(
                            "XrdsAction::ModifyHealth targeted entity {entity:?}, which has no \
                             XrdsHealth component — ignoring."
                        );
                    }
                }
                true
            }

            XrdsAction::PlayGltfAnimation { playback } => {
                let request = PendingGltfAnimationRequest {
                    selector: runtime_animation_selector(&playback.selector),
                    options: runtime_playback_options(&playback),
                };
                match apply_gltf_animation_request_for_entity_in_world(
                    world,
                    self.target,
                    &request,
                ) {
                    Ok(true) => {}
                    // Asset not ready yet: queue it so the existing
                    // scene-ready observer applies it on load, matching
                    // how imperative playback requests already behave.
                    Ok(false) => {
                        world
                            .resource_mut::<PendingGltfAnimationRequests>()
                            .requests
                            .insert(self.target, request);
                    }
                    Err(error) => log::warn!(
                        "XrdsAction::PlayGltfAnimation on {:?} failed: {error:?}",
                        self.target
                    ),
                }
                true
            }

            XrdsAction::StopGltfAnimation => {
                for player_entity in
                    animation_player_entities_for_root_in_world(world, self.target)
                {
                    if let Some(mut player) =
                        world.get_mut::<bevy::animation::AnimationPlayer>(player_entity)
                    {
                        player.stop_all();
                    }
                }
                true
            }

            XrdsAction::FireCustomEvent { name } => {
                world.write_message(XrdsCustomTriggerEvent {
                    name,
                    target: self.target,
                    source: self.source,
                });
                true
            }
        }
    }

    fn on_stop(&mut self, _agent: Option<Entity>, _world: &mut World, _reason: StopReason) {}
}
