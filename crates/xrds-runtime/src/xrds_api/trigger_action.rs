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
    XrdsAction, XrdsActionTarget, XrdsActionValue, XrdsAxis, XrdsCrossing, XrdsObservable,
    XrdsRunnable, XrdsSceneAnimationRepeatMode, XrdsSceneGltfAnimationSelector,
    XrdsSceneGltfPlayback, XrdsSequence, XrdsThresholdWatcher, XrdsTimeline, XrdsTimelineKey,
    XrdsTriggerBinding, XrdsTriggerKind,
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

/// Max causal chain depth for [`XrdsAction::Run`] before the runaway escape
/// hatch fires — not a rate limit, a *depth* cap. A rate limit can't tell a
/// real infinite loop apart from legitimately high-frequency input (e.g.
/// `SliderChange`); chain depth can. See `XrdsTriggerKind::RunawayDetected`.
pub const MAX_RUN_CHAIN_DEPTH: u32 = 64;

/// Runtime mirror of `XrdsSceneDocument::runnables` — the document-level
/// name → `XrdsRunnable` lookup that `XrdsTriggerBinding::runnable` and
/// `XrdsAction::Run` resolve against. Replaced wholesale on every full
/// document import (see `reimport::sync_runnable_registry`), matching how
/// the rest of import treats the document as complete, authoritative state
/// rather than something to merge into.
#[derive(Resource, Debug, Clone, Default)]
pub struct XrdsRunnableRegistry(pub std::collections::HashMap<String, XrdsRunnable>);

/// Marks an ephemeral per-firing agent entity and records which entities
/// its actions apply to. One is spawned per trigger firing and despawned
/// by [`despawn_finished_sequence_agents`] once its queue drains.
#[derive(Component, Debug, Clone, Copy)]
pub struct XrdsSequenceAgent {
    pub target: Entity,
    pub source: Option<Entity>,
    /// How many `Run` hops caused this agent to exist — 0 for an agent
    /// spawned directly by a trigger firing. Propagated to `Run`'s child
    /// agent as `chain_depth + 1`; see [`MAX_RUN_CHAIN_DEPTH`].
    pub chain_depth: u32,
    /// Whether this agent's chain was started by a `RunawayDetected` firing.
    /// Propagated to every descendant `Run` spawns. Guarantees the recovery
    /// path can never itself trigger another `RunawayDetected` — see
    /// `XrdsActionRunner`'s `Run` handling.
    pub is_recovery: bool,
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
    registry: Res<XrdsRunnableRegistry>,
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
            spawn_binding_runnable(&mut commands, target, source, binding, &registry, false);
        }
    }
}

/// Starts whatever a trigger binding names, at chain depth 0. `runnable:
/// Some(name)` resolves through the document's runnable registry (the
/// template case, Phase 9a); `None` falls back to the binding's own inline
/// `sequence`. An unresolvable name fires nothing, with a warning, rather
/// than panicking — the same forward-compat posture as `XrdsAction::Unknown`.
///
/// `is_recovery` is `true` only when this firing came from
/// [`fire_runaway_detected_in_world`] — see [`XrdsSequenceAgent::is_recovery`].
fn spawn_binding_runnable(
    commands: &mut Commands,
    target: Entity,
    source: Option<Entity>,
    binding: &XrdsTriggerBinding,
    registry: &XrdsRunnableRegistry,
    is_recovery: bool,
) {
    match &binding.runnable {
        Some(name) => match registry.0.get(name) {
            Some(XrdsRunnable::Sequence(sequence)) => {
                spawn_sequence_agent_with_depth(commands, target, source, sequence, 0, is_recovery);
            }
            Some(XrdsRunnable::Timeline(timeline)) => {
                spawn_timeline_agent_with_depth(commands, target, source, timeline, 0, is_recovery);
            }
            None => {
                log::warn!(
                    "XrdsTriggerBinding.runnable named {name:?} on {target:?}, which has no \
                     matching entry in XrdsSceneDocument::runnables — nothing fired."
                );
            }
        },
        None => {
            spawn_sequence_agent_with_depth(
                commands,
                target,
                source,
                &binding.sequence,
                0,
                is_recovery,
            );
        }
    }
}

/// Spawns one ephemeral agent carrying the whole sequence, at chain depth 0.
/// Public so expert-layer code and tests can kick off an authored sequence
/// directly without going through a trigger.
pub fn spawn_sequence_agent(
    commands: &mut Commands,
    target: Entity,
    source: Option<Entity>,
    sequence: &XrdsSequence,
) -> Option<Entity> {
    spawn_sequence_agent_with_depth(commands, target, source, sequence, 0, false)
}

/// Same as [`spawn_sequence_agent`] but at an explicit chain depth and
/// recovery flag — used by `XrdsAction::Run` to propagate `chain_depth + 1`
/// and `is_recovery` to the runnable it starts. Returns `None` (and spawns
/// nothing) for an empty sequence.
pub fn spawn_sequence_agent_with_depth(
    commands: &mut Commands,
    target: Entity,
    source: Option<Entity>,
    sequence: &XrdsSequence,
    chain_depth: u32,
    is_recovery: bool,
) -> Option<Entity> {
    if sequence.steps.is_empty() {
        return None;
    }

    let agent = commands
        .spawn((
            SequentialActions,
            XrdsSequenceAgent { target, source, chain_depth, is_recovery },
        ))
        .id();

    let runners: Vec<BoxedAction> = sequence
        .steps
        .iter()
        .map(|action| {
            Box::new(XrdsActionRunner::new(action.clone(), target, source, chain_depth, is_recovery))
                as BoxedAction
        })
        .collect();

    commands.actions(agent).add(runners);
    Some(agent)
}

/// Same as [`spawn_sequence_agent_with_depth`] but for an `XrdsTimeline` —
/// absolute-time, concurrent choreography rather than a queue. See
/// [`XrdsTimelineAgent`]. Returns `None` for a timeline with no keys.
pub fn spawn_timeline_agent_with_depth(
    commands: &mut Commands,
    target: Entity,
    source: Option<Entity>,
    timeline: &XrdsTimeline,
    chain_depth: u32,
    is_recovery: bool,
) -> Option<Entity> {
    if timeline.keys.is_empty() {
        return None;
    }

    let mut keys = timeline.keys.clone();
    keys.sort_by(|a, b| a.at_secs.total_cmp(&b.at_secs));
    let duration_secs = timeline
        .duration_secs
        .unwrap_or_else(|| keys.last().map(|k| k.at_secs).unwrap_or(0.0));

    let agent = commands
        .spawn(XrdsTimelineAgent {
            target,
            source,
            chain_depth,
            is_recovery,
            keys,
            next_key_index: 0,
            elapsed_secs: 0.0,
            duration_secs,
            looping: timeline.looping,
        })
        .id();
    Some(agent)
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
    let matching: Vec<XrdsTriggerBinding> = bindings
        .0
        .iter()
        .filter(|b| !b.disabled && &b.trigger == kind && (b.hand.is_none() || b.hand == hand))
        .cloned()
        .collect();

    let count = matching.len();
    let registry = world.resource::<XrdsRunnableRegistry>().clone();
    for binding in &matching {
        // Same shape as consume_triggers: target is its own source, since
        // nothing external caused this.
        let mut commands = world.commands();
        spawn_binding_runnable(&mut commands, target, Some(target), binding, &registry, false);
    }
    world.flush();
    count
}

/// Fires `XrdsTriggerKind::RunawayDetected` on `node`, marking every agent it
/// spawns as a *recovery* chain (see [`XrdsSequenceAgent::is_recovery`]).
/// Otherwise identical to [`fire_trigger_in_world`]. Kept separate rather
/// than adding an `is_recovery` parameter to the public `fire_trigger_in_world`
/// — that flag is purely an internal escape-hatch guarantee, not something a
/// caller (editor preview, expert code) should ever be able to set.
fn fire_runaway_detected_in_world(world: &mut World, node: XrdsId) -> usize {
    let Some(target) = world.resource::<XrdsIdIndex>().entity_of(node) else {
        return 0;
    };
    let Some(bindings) = world.get::<XrdsTriggerBindings>(target) else {
        return 0;
    };

    let matching: Vec<XrdsTriggerBinding> = bindings
        .0
        .iter()
        .filter(|b| !b.disabled && b.trigger == XrdsTriggerKind::RunawayDetected)
        .cloned()
        .collect();

    let count = matching.len();
    let registry = world.resource::<XrdsRunnableRegistry>().clone();
    for binding in &matching {
        let mut commands = world.commands();
        spawn_binding_runnable(&mut commands, target, Some(target), binding, &registry, true);
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

    let mut doomed: Vec<Entity> = world
        .query::<(Entity, &XrdsSequenceAgent)>()
        .iter(world)
        .filter(|(_, agent)| agent.target == target)
        .map(|(entity, _)| entity)
        .collect();
    doomed.extend(
        world
            .query::<(Entity, &XrdsTimelineAgent)>()
            .iter(world)
            .filter(|(_, agent)| agent.target == target)
            .map(|(entity, _)| entity),
    );

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

/// Cancels every in-flight sequence and timeline in the world. Returns how
/// many were stopped.
pub fn stop_all_sequences_in_world(world: &mut World) -> usize {
    let mut doomed: Vec<Entity> = world
        .query_filtered::<Entity, With<XrdsSequenceAgent>>()
        .iter(world)
        .collect();
    doomed.extend(
        world
            .query_filtered::<Entity, With<XrdsTimelineAgent>>()
            .iter(world),
    );

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

/// Runtime agent driving one in-flight `XrdsTimeline` — absolute-time,
/// concurrent choreography, unlike `XrdsSequenceAgent`'s ordered, relative
/// queue. Never blocks: each key is fired by spawning it as its own
/// one-step sequence agent (see [`advance_timelines`]) rather than
/// duplicating action-execution here.
#[derive(Component, Debug, Clone)]
pub struct XrdsTimelineAgent {
    pub target: Entity,
    pub source: Option<Entity>,
    pub chain_depth: u32,
    pub is_recovery: bool,
    /// Sorted ascending by `at_secs` once, at spawn time — authoring does
    /// not need to pre-sort.
    keys: Vec<XrdsTimelineKey>,
    next_key_index: usize,
    elapsed_secs: f32,
    duration_secs: f32,
    looping: bool,
}

/// Advances every in-flight [`XrdsTimelineAgent`], firing every key crossed
/// this frame. Uses a `while` loop rather than a single `if`, so a long
/// frame (or a `duration_secs` shorter than one frame) never silently drops
/// a key. `duration_secs <= 0.0` fires every key immediately instead of
/// hot-spinning at one key per frame forever.
pub fn advance_timelines(
    time: Res<Time>,
    mut agents: Query<(Entity, &mut XrdsTimelineAgent)>,
    mut commands: Commands,
) {
    let dt = time.delta_secs();
    for (entity, mut agent) in &mut agents {
        if agent.duration_secs <= 0.0 {
            while agent.next_key_index < agent.keys.len() {
                fire_timeline_key(&mut commands, &agent);
                agent.next_key_index += 1;
            }
            if agent.looping {
                agent.next_key_index = 0;
            } else {
                commands.entity(entity).despawn();
            }
            continue;
        }

        agent.elapsed_secs += dt;
        while agent.next_key_index < agent.keys.len()
            && agent.keys[agent.next_key_index].at_secs <= agent.elapsed_secs
        {
            fire_timeline_key(&mut commands, &agent);
            agent.next_key_index += 1;
        }

        if agent.elapsed_secs >= agent.duration_secs {
            if agent.looping {
                agent.elapsed_secs %= agent.duration_secs;
                agent.next_key_index = 0;
                // Fire any keys at/before the wrapped elapsed time right
                // away, same while-loop, so a key at 0.0 doesn't wait a
                // full lap before it fires again.
                while agent.next_key_index < agent.keys.len()
                    && agent.keys[agent.next_key_index].at_secs <= agent.elapsed_secs
                {
                    fire_timeline_key(&mut commands, &agent);
                    agent.next_key_index += 1;
                }
            } else {
                commands.entity(entity).despawn();
            }
        }
    }
}

/// Fires one timeline key by spawning it as its own one-step sequence
/// agent, at the timeline's chain depth. `Wait` inside a timeline key is
/// meaningless — the key already carries its own `at_secs` — so it is
/// skipped with a warning rather than silently stalling that one step.
fn fire_timeline_key(commands: &mut Commands, agent: &XrdsTimelineAgent) {
    let key = &agent.keys[agent.next_key_index];
    if matches!(key.action, XrdsAction::Wait { .. }) {
        log::warn!(
            "XrdsAction::Wait inside a timeline key on {:?} is meaningless — a timeline key \
             already carries its own at_secs. Skipping.",
            agent.target
        );
        return;
    }
    let sequence = XrdsSequence { steps: vec![key.action.clone()] };
    spawn_sequence_agent_with_depth(
        commands,
        agent.target,
        agent.source,
        &sequence,
        agent.chain_depth,
        agent.is_recovery,
    );
}

/// Bridges one authored [`XrdsAction`] into `bevy-sequential-actions`'
/// [`Action`] trait. Actions apply to the *target*/*source* entities
/// recorded here, never to the agent entity the queue lives on.
pub struct XrdsActionRunner {
    action: XrdsAction,
    target: Entity,
    source: Option<Entity>,
    /// How many `Run` hops led to this action running — see
    /// [`MAX_RUN_CHAIN_DEPTH`]. Propagated to a `Run`'s child agent as
    /// `chain_depth + 1`.
    chain_depth: u32,
    /// Whether this action's chain was started by a `RunawayDetected`
    /// recovery firing — see `XrdsSequenceAgent::is_recovery`. If a `Run`
    /// running inside a recovery chain itself hits the depth cap, it is
    /// dropped with a hard error instead of firing `RunawayDetected` again,
    /// guaranteeing the recovery path can never loop through itself.
    is_recovery: bool,
    /// Only used by `Wait` — `is_finished` gets `&self`, so it can't tick
    /// a `Timer`; storing an absolute deadline is the workaround (same
    /// pattern as `examples/expert/sequential_actions_spike.rs`).
    deadline_secs: Option<f32>,
    /// Only used by `Run { wait: true }` targeting a sequence — the child
    /// agent this action is blocking on. `is_finished` reports done once
    /// this entity no longer exists.
    waiting_on: Option<Entity>,
}

impl XrdsActionRunner {
    pub fn new(
        action: XrdsAction,
        target: Entity,
        source: Option<Entity>,
        chain_depth: u32,
        is_recovery: bool,
    ) -> Self {
        Self {
            action,
            target,
            source,
            chain_depth,
            is_recovery,
            deadline_secs: None,
            waiting_on: None,
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
        match &self.action {
            XrdsAction::Wait { .. } => match self.deadline_secs {
                Some(deadline) => world.resource::<Time>().elapsed_secs() >= deadline,
                None => true,
            },
            // `Run { wait: true }` targeting a sequence: done once the
            // child agent it spawned has despawned. `waiting_on` is `None`
            // for `wait: false`, an unresolved runnable, or a timeline
            // target — all of which finish immediately in `on_start`.
            XrdsAction::Run { .. } => match self.waiting_on {
                Some(child) => world.get_entity(child).is_err(),
                None => true,
            },
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

            XrdsAction::Run { runnable, wait } => {
                if self.chain_depth >= MAX_RUN_CHAIN_DEPTH {
                    if self.is_recovery {
                        // The recovery path itself looped. Drop it here,
                        // hard, and do NOT fire RunawayDetected again — the
                        // one guarantee this escape hatch makes is that the
                        // breaker can never recurse through its own
                        // recovery, per the design doc's "guaranteed
                        // escape" section.
                        log::error!(
                            "Run chain depth cap ({MAX_RUN_CHAIN_DEPTH}) reached again inside a \
                             RunawayDetected recovery chain on {:?} (Run({runnable:?})) — \
                             dropping this chain without re-firing RunawayDetected.",
                            self.target
                        );
                    } else {
                        log::warn!(
                            "Run chain depth cap ({MAX_RUN_CHAIN_DEPTH}) reached resolving \
                             Run({runnable:?}) on {:?} — stopping this chain and firing \
                             RunawayDetected instead of recursing further.",
                            self.target
                        );
                        if let Some(node_id) = world.resource::<XrdsIdIndex>().id_of(self.target) {
                            fire_runaway_detected_in_world(world, node_id);
                        }
                    }
                    return true;
                }

                let Some(entry) = world
                    .resource::<XrdsRunnableRegistry>()
                    .0
                    .get(&runnable)
                    .cloned()
                else {
                    log::warn!(
                        "XrdsAction::Run referenced unknown runnable {runnable:?} on {:?} — no \
                         such entry in XrdsSceneDocument::runnables. Skipping.",
                        self.target
                    );
                    return true;
                };

                match entry {
                    XrdsRunnable::Sequence(sequence) => {
                        let mut commands = world.commands();
                        let child = spawn_sequence_agent_with_depth(
                            &mut commands,
                            self.target,
                            self.source,
                            &sequence,
                            self.chain_depth + 1,
                            self.is_recovery,
                        );
                        world.flush();
                        if wait {
                            self.waiting_on = child;
                            child.is_none()
                        } else {
                            true
                        }
                    }
                    XrdsRunnable::Timeline(timeline) => {
                        if wait {
                            log::warn!(
                                "Run {{ wait: true }} targeting timeline runnable {runnable:?} \
                                 on {:?} — timelines are concurrent choreography, not a queue \
                                 step, so `wait` is ignored and it runs fire-and-forget.",
                                self.target
                            );
                        }
                        let mut commands = world.commands();
                        spawn_timeline_agent_with_depth(
                            &mut commands,
                            self.target,
                            self.source,
                            &timeline,
                            self.chain_depth + 1,
                            self.is_recovery,
                        );
                        world.flush();
                        true
                    }
                }
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

// ---------------------------------------------------------------------------
// Threshold watchers — continuous values to discrete Custom triggers (Phase 8)
// ---------------------------------------------------------------------------

/// Runtime component holding a node's authored threshold watchers — spawned
/// at scene-document import (see `reimport::tag_threshold_watcher_entities`),
/// mirroring [`XrdsTriggerBindings`] exactly.
#[derive(Component, Debug, Clone, Default)]
pub struct XrdsThresholdWatchers(pub Vec<XrdsThresholdWatcher>);

/// Per-watcher crossing state — **not** authored data, never touched by
/// import/export/reimport. Parallel to the node's `XrdsThresholdWatchers.0`
/// by index. `None` until a watcher's first evaluation (there is no real
/// "previous value" yet, so the first read primes the state without firing);
/// `Some(is_above)` afterward.
#[derive(Component, Debug, Clone, Default)]
pub struct XrdsThresholdWatcherState(Vec<Option<bool>>);

/// Fired when a threshold watcher crosses its value in a direction its
/// `crossing` setting allows.
///
/// Implements [`XrdsTriggerEvent`] as `XrdsTriggerKind::Custom(fires)` — a
/// watcher crossing is just another way to fire a `Custom` trigger, so it
/// composes with the exact same bindings an app-defined event would use.
#[derive(Message, Debug, Clone)]
pub struct XrdsThresholdCrossedEvent {
    pub node_id: XrdsId,
    pub fires: String,
}

impl XrdsTriggerEvent for XrdsThresholdCrossedEvent {
    fn target(&self) -> XrdsTriggerRef {
        XrdsTriggerRef::Id(self.node_id)
    }
    fn kind(&self) -> XrdsTriggerKind {
        XrdsTriggerKind::Custom(self.fires.clone())
    }
}

/// Reads one [`XrdsObservable`] off `entity`'s world transform.
///
/// `RotationDegrees` uses Euler XYZ decomposition — see the type's own doc
/// comment for the gimbal-lock caveat this carries. `DistanceTo` resolves
/// its target through `XrdsIdIndex`; a target that can't be resolved (not
/// yet imported, or dangling — `trigger_diagnostics` should have already
/// flagged the latter at author time) reads as `None`, and the watcher
/// simply skips evaluation that frame rather than guessing a value.
fn read_observable(
    world: &World,
    entity: Entity,
    observable: &XrdsObservable,
) -> Option<f32> {
    let transform = world.get::<GlobalTransform>(entity)?;
    match observable {
        XrdsObservable::RotationDegrees { axis } => {
            let (x, y, z) = transform.rotation().to_euler(EulerRot::XYZ);
            let radians = match axis {
                XrdsAxis::X => x,
                XrdsAxis::Y => y,
                XrdsAxis::Z => z,
            };
            Some(radians.to_degrees())
        }
        XrdsObservable::DistanceTo { node } => {
            let other = world.resource::<XrdsIdIndex>().entity_of((*node).into())?;
            let other_transform = world.get::<GlobalTransform>(other)?;
            Some(transform.translation().distance(other_transform.translation()))
        }
        XrdsObservable::Height => Some(transform.translation().y),
        XrdsObservable::ScaleMagnitude => Some(transform.scale().length()),
    }
}

/// Evaluates every threshold watcher each frame, updates crossing state, and
/// emits [`XrdsThresholdCrossedEvent`] for each qualifying crossing.
///
/// Runs in `Last`, after `TransformSystems::Propagate` (`PostUpdate`) has
/// updated `GlobalTransform` for anything that moved this frame — same
/// placement as [`sync_completed_gltf_animation_triggers`], and for the same
/// reason: the resulting event is consumed on the next frame's `Update`,
/// a one-frame latency that's the only cost.
pub fn evaluate_threshold_watchers(world: &mut World) {
    let entities: Vec<Entity> = world
        .query_filtered::<Entity, With<XrdsThresholdWatchers>>()
        .iter(world)
        .collect();

    let mut crossings: Vec<(XrdsId, String)> = Vec::new();

    for entity in entities {
        let Some(node_id) = world.resource::<XrdsIdIndex>().id_of(entity) else {
            continue;
        };
        // Cloned out, not borrowed: read_observable takes `&World`, so a
        // live borrow of the component here would conflict with it.
        let watchers = world.get::<XrdsThresholdWatchers>(entity).unwrap().0.clone();

        // Grown lazily to match `watchers.len()` — the vec starts empty via
        // Default and a document can add watchers to an already-imported
        // node (e.g. via a live editor reimport).
        let mut state = world
            .get::<XrdsThresholdWatcherState>(entity)
            .cloned()
            .unwrap_or_default();
        state.0.resize(watchers.len(), None);

        for (index, watcher) in watchers.iter().enumerate() {
            if watcher.disabled {
                continue;
            }
            let Some(value) = read_observable(world, entity, &watcher.observable) else {
                continue;
            };

            let hysteresis = watcher.hysteresis.max(0.0);
            let previous = state.0[index];
            let is_above = match previous {
                // Sticky band: once Above, must fall below (value -
                // hysteresis) to become Below, and vice versa — this is
                // what makes hysteresis actually suppress chatter at the
                // boundary, rather than just widening a single instant.
                Some(true) => value >= watcher.value - hysteresis,
                Some(false) => value > watcher.value + hysteresis,
                // First evaluation: prime from the raw threshold, no
                // hysteresis band yet to be sticky against, and — per
                // `previous.is_some()` below — never fires.
                None => value >= watcher.value,
            };

            if let Some(prev_is_above) = previous {
                if prev_is_above != is_above {
                    let fires_now = match watcher.crossing {
                        XrdsCrossing::Above => is_above,
                        XrdsCrossing::Below => !is_above,
                        XrdsCrossing::Either => true,
                    };
                    if fires_now {
                        crossings.push((node_id, watcher.fires.clone()));
                    }
                }
            }

            state.0[index] = Some(is_above);
        }

        world.entity_mut(entity).insert(state);
    }

    for (node_id, fires) in crossings {
        world.write_message(XrdsThresholdCrossedEvent { node_id, fires });
    }
}
