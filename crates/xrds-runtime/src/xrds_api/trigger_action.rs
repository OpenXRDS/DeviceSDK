//! Trigger-action sequencing runtime.
//!
//! See `docs/xrds-scenegraph-trigger-action-sequencing.md` for the design,
//! `docs/done/xrds-trigger-action-v1.md` for the implementation record (this
//! file is Phases 3-4, 7 and 10), and
//! `docs/xrds-trigger-action-implementation-plan.md` for what is still ahead.
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
    fn target(&self) -> XrdsTriggerRef {
        XrdsTriggerRef::Id(self.node_id)
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

/// How a trigger event names an entity.
///
/// Existing XRDS events are split on this: zone/grab/hover events carry a
/// stable `XrdsId`, while the world-UI button/slider/toggle events carry a
/// raw Bevy `Entity`. Rather than force every event to normalize (which
/// would mean changing types that already have other consumers), the trait
/// reports whichever it has and [`consume_triggers`] resolves it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum XrdsTriggerRef {
    Id(XrdsId),
    Entity(Entity),
}

impl XrdsTriggerRef {
    /// Resolves to a live entity, going through the id index only when
    /// needed.
    pub fn resolve(self, index: &XrdsIdIndex) -> Option<Entity> {
        match self {
            Self::Id(id) => index.entity_of(id),
            Self::Entity(entity) => Some(entity),
        }
    }
}

/// Any message that should be able to fire an authored sequence
/// implements this. Adding a new trigger source (e.g. an `avian3d`
/// collision event, or an app's own gameplay event) is one trait impl plus
/// one `consume_triggers::<E>` registration — the data model and consumer
/// logic never change.
pub trait XrdsTriggerEvent: Message {
    /// Whose bindings to check — the node the sequence is authored on.
    fn target(&self) -> XrdsTriggerRef;

    /// What *caused* the trigger, when meaningfully distinct from the
    /// target (e.g. the bullet that hit the player). Defaults to the
    /// target so sources with no separate cause — a timer, say — don't
    /// have to think about it.
    fn source(&self) -> XrdsTriggerRef {
        self.target()
    }

    fn kind(&self) -> XrdsTriggerKind;

    /// Which controller caused this, if the source reports one. Defaults to
    /// `None` — most trigger sources (zone enter/exit, animation
    /// completion, app-defined `Custom` events) have no controller to
    /// report at all.
    fn hand(&self) -> Option<xrds_components::XrGrabHand> {
        None
    }
}

impl XrdsTriggerEvent for xrds_components::XrZoneEnterEvent {
    fn target(&self) -> XrdsTriggerRef {
        XrdsTriggerRef::Id(self.zone_id)
    }
    fn source(&self) -> XrdsTriggerRef {
        XrdsTriggerRef::Id(self.entity_id)
    }
    fn kind(&self) -> XrdsTriggerKind {
        XrdsTriggerKind::ZoneEnter
    }
}

impl XrdsTriggerEvent for xrds_components::XrZoneExitEvent {
    fn target(&self) -> XrdsTriggerRef {
        XrdsTriggerRef::Id(self.zone_id)
    }
    fn source(&self) -> XrdsTriggerRef {
        XrdsTriggerRef::Id(self.entity_id)
    }
    fn kind(&self) -> XrdsTriggerKind {
        XrdsTriggerKind::ZoneExit
    }
}

// --- XR interaction: grab / drop -----------------------------------------
// The canonical XR interaction pair (Unity XRI's SelectEntered/SelectExited
// equivalent). `hand` is not an entity, so source defaults to target.

impl XrdsTriggerEvent for xrds_components::XrGrabEvent {
    fn target(&self) -> XrdsTriggerRef {
        XrdsTriggerRef::Id(self.id)
    }
    fn kind(&self) -> XrdsTriggerKind {
        XrdsTriggerKind::Grabbed
    }
    fn hand(&self) -> Option<xrds_components::XrGrabHand> {
        Some(self.hand)
    }
}

impl XrdsTriggerEvent for xrds_components::XrDropEvent {
    fn target(&self) -> XrdsTriggerRef {
        XrdsTriggerRef::Id(self.id)
    }
    fn kind(&self) -> XrdsTriggerKind {
        XrdsTriggerKind::Dropped
    }
    fn hand(&self) -> Option<xrds_components::XrGrabHand> {
        Some(self.hand)
    }
}

// --- World-space UI ------------------------------------------------------

impl XrdsTriggerEvent for xrds_components::XrWorldHoverEnterEvent {
    fn target(&self) -> XrdsTriggerRef {
        XrdsTriggerRef::Id(self.panel_id)
    }
    fn kind(&self) -> XrdsTriggerKind {
        XrdsTriggerKind::HoverEnter
    }
    fn hand(&self) -> Option<xrds_components::XrGrabHand> {
        Some(self.hand)
    }
}

impl XrdsTriggerEvent for xrds_components::XrWorldHoverExitEvent {
    fn target(&self) -> XrdsTriggerRef {
        XrdsTriggerRef::Id(self.panel_id)
    }
    fn kind(&self) -> XrdsTriggerKind {
        XrdsTriggerKind::HoverExit
    }
    fn hand(&self) -> Option<xrds_components::XrGrabHand> {
        Some(self.hand)
    }
}

impl XrdsTriggerEvent for xrds_components::XrWorldButtonPressEvent {
    fn target(&self) -> XrdsTriggerRef {
        XrdsTriggerRef::Entity(self.button_entity)
    }
    fn kind(&self) -> XrdsTriggerKind {
        XrdsTriggerKind::ButtonPress
    }
    fn hand(&self) -> Option<xrds_components::XrGrabHand> {
        Some(self.hand)
    }
}

impl XrdsTriggerEvent for xrds_components::XrWorldButtonReleaseEvent {
    fn target(&self) -> XrdsTriggerRef {
        XrdsTriggerRef::Entity(self.button_entity)
    }
    fn kind(&self) -> XrdsTriggerKind {
        XrdsTriggerKind::ButtonRelease
    }
    fn hand(&self) -> Option<xrds_components::XrGrabHand> {
        Some(self.hand)
    }
}

impl XrdsTriggerEvent for xrds_components::XrWorldSliderChangeEvent {
    fn target(&self) -> XrdsTriggerRef {
        XrdsTriggerRef::Entity(self.slider_entity)
    }
    fn kind(&self) -> XrdsTriggerKind {
        XrdsTriggerKind::SliderChange
    }
    fn hand(&self) -> Option<xrds_components::XrGrabHand> {
        Some(self.hand)
    }
}

impl XrdsTriggerEvent for xrds_components::XrWorldToggleEvent {
    fn target(&self) -> XrdsTriggerRef {
        XrdsTriggerRef::Entity(self.toggle_entity)
    }
    fn kind(&self) -> XrdsTriggerKind {
        XrdsTriggerKind::ToggleChange
    }
    fn hand(&self) -> Option<xrds_components::XrGrabHand> {
        Some(self.hand)
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
        let Some(target) = event.target().resolve(&id_index) else {
            continue;
        };
        let Ok(node_bindings) = bindings.get(target) else {
            continue;
        };
        let source = event.source().resolve(&id_index);
        let kind = event.kind();
        let hand = event.hand();

        for binding in node_bindings.0.iter().filter(|b| {
            // `binding.hand: None` matches any hand (including a source
            // that reports none), preserving the old behavior for every
            // binding that doesn't opt into this filter. `Some(h)` only
            // matches an event that reports that exact hand — an event
            // with no hand (hand() == None) can never satisfy a filter,
            // which is correct: it has nothing to compare against.
            !b.disabled && b.trigger == kind && (b.hand.is_none() || b.hand == hand)
        }) {
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

/// Fires a trigger on a node directly, without waiting for the real event
/// that would normally produce it.
///
/// Runs every binding on that node matching `kind`, exactly as
/// [`consume_triggers`] would. Returns how many sequences it started, so a
/// caller can tell "nothing was bound" from "it ran".
///
/// Intended for an editor's "preview this sequence" affordance and for
/// application-level tests, where waiting for a real zone collision or
/// button press is impractical.
pub fn fire_trigger_in_world(
    world: &mut World,
    node: XrdsId,
    kind: &XrdsTriggerKind,
    hand: Option<xrds_components::XrGrabHand>,
) -> usize {
    let Some(target) = world.resource::<XrdsIdIndex>().entity_of(node) else {
        return 0;
    };
    let Some(bindings) = world.get::<XrdsTriggerBindings>(target) else {
        return 0;
    };

    // Skips disabled bindings and applies the same hand filter as
    // consume_triggers — an editor preview that ignored either would
    // misrepresent runtime.
    let sequences: Vec<XrdsSequence> = bindings
        .0
        .iter()
        .filter(|b| !b.disabled && &b.trigger == kind && (b.hand.is_none() || b.hand == hand))
        .map(|b| b.sequence.clone())
        .collect();

    let count = sequences.len();
    for sequence in sequences {
        // Same shape as consume_triggers: target is its own source, since
        // nothing external caused this.
        let mut commands = world.commands();
        spawn_sequence_agent(&mut commands, target, Some(target), &sequence);
    }
    world.flush();
    count
}

/// Cancels every in-flight sequence targeting `node`. Returns how many were
/// stopped.
///
/// The manual half of the runaway-loop escape hatch (see the plan doc), and
/// independently useful for aborting a cutscene or tearing down before a
/// scene transition.
pub fn stop_sequences_on_in_world(world: &mut World, node: XrdsId) -> usize {
    let Some(target) = world.resource::<XrdsIdIndex>().entity_of(node) else {
        return 0;
    };

    let doomed: Vec<Entity> = world
        .query::<(Entity, &XrdsSequenceAgent)>()
        .iter(world)
        .filter(|(_, agent)| agent.target == target)
        .map(|(entity, _)| entity)
        .collect();

    let count = doomed.len();
    for agent in doomed {
        // Despawning the agent drops its queue; bevy-sequential-actions
        // reports StopReason to each action's on_stop as it goes.
        if let Ok(entity) = world.get_entity_mut(agent) {
            entity.despawn();
        }
    }
    count
}

/// Cancels every in-flight sequence in the world. Returns how many were
/// stopped.
pub fn stop_all_sequences_in_world(world: &mut World) -> usize {
    let doomed: Vec<Entity> = world
        .query_filtered::<Entity, With<XrdsSequenceAgent>>()
        .iter(world)
        .collect();

    let count = doomed.len();
    for agent in doomed {
        if let Ok(entity) = world.get_entity_mut(agent) {
            entity.despawn();
        }
    }
    count
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

            // An action this build doesn't recognize — almost certainly a
            // document written by a newer editor. Skip it and keep the rest
            // of the sequence running; the alternative (failing the whole
            // document at load) is far worse. See XrdsAction::Unknown.
            XrdsAction::Unknown => {
                log::warn!(
                    "Skipping an unrecognized XrdsAction on {:?} — this scene was likely \
                     authored by a newer build of the editor than this runtime.",
                    self.target
                );
                true
            }
        }
    }

    fn on_stop(&mut self, _agent: Option<Entity>, _world: &mut World, _reason: StopReason) {}
}
