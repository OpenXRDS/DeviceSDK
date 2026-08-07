//! Trigger-action sequencing runtime.
//!
//! See `docs/done/xrds-scenegraph-trigger-action-sequencing.md` for the design
//! and `docs/done/xrds-trigger-action-v1.md` for the implementation record
//! (this file is Phases 3-4, 7, 8, 9, 9a and 10) — the whole system is done,
//! nothing is left planned.
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
    XrdsAction, XrdsActionTarget, XrdsActionValue, XrdsAxis, XrdsCrossing, XrdsEaseCurve,
    XrdsObservable, XrdsSceneAnimationRepeatMode, XrdsSceneGltfAnimationSelector,
    XrdsSceneGltfPlayback, XrdsThresholdWatcher, XrdsTrack,
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

/// Runtime mirror of `XrdsSceneDocument::tracks` — the document-level
/// name → [`XrdsTrack`] lookup that `XrdsTriggerBinding::track` resolves
/// against. Replaced wholesale on every full document import (see
/// `reimport::sync_track_registry`), matching how the rest of import treats
/// the document as complete, authoritative state rather than something to
/// merge into.
#[derive(Resource, Debug, Clone, Default)]
pub struct XrdsTrackRegistry(pub std::collections::HashMap<String, XrdsTrack>);

/// Which entities are currently being driven by which running Track.
///
/// This is the runtime half of the one-asset-at-a-time rule: a Track will not
/// start if any asset it drives is already held by another running Track.
/// See `docs/done/xrds-track-model-plan.md` §4 for why the policy is
/// *reject the newcomer* rather than preempt or queue — briefly, a scenario
/// that completes is worth more than one that starts, and a partially-applied
/// Track plays *wrong* rather than not at all, which is far harder to debug.
///
/// Keyed on the **resolved `Entity`**, deliberately, not on the authored
/// target: `SelfNode`/`TriggerSource` only become concrete when fired, so two
/// Tracks both using `SelfNode` on different nodes must not collide.
#[derive(Resource, Debug, Default)]
pub struct XrdsTrackAssetLocks {
    /// entity → the agent holding it.
    held: std::collections::HashMap<Entity, Entity>,
    /// The most recent refusal, for the editor to surface. Without this a
    /// rejected Track is a silent no-op, which is the one real weakness of
    /// the reject policy.
    pub last_conflict: Option<XrdsTrackConflict>,
}

/// Why a Track refused to start.
#[derive(Debug, Clone)]
pub struct XrdsTrackConflict {
    pub blocked_track: String,
    pub contended: Vec<Entity>,
}

impl XrdsTrackAssetLocks {
    /// Entities from `wanted` that some *other* agent already holds.
    fn conflicts(&self, wanted: &[Entity], me: Entity) -> Vec<Entity> {
        wanted
            .iter()
            .copied()
            .filter(|e| self.held.get(e).is_some_and(|holder| *holder != me))
            .collect()
    }

    fn acquire(&mut self, wanted: &[Entity], agent: Entity) {
        for e in wanted {
            self.held.insert(*e, agent);
        }
    }

    /// Drops every lock held by `agent`.
    ///
    /// **Load-bearing.** If this is missed when an agent despawns, the locks
    /// leak and every Track sharing those assets is blocked forever — the
    /// single most likely bug in this design, so it has its own test.
    pub fn release_agent(&mut self, agent: Entity) {
        self.held.retain(|_, holder| *holder != agent);
    }

    /// Every entity `agent` currently holds. The record of what a running
    /// Track has touched, which is what an editor preview needs in order to
    /// restore those nodes when it stops.
    pub fn entities_held_by(&self, agent: Entity) -> Vec<Entity> {
        self.held
            .iter()
            .filter(|(_, holder)| **holder == agent)
            .map(|(entity, _)| *entity)
            .collect()
    }

    pub fn holder_of(&self, entity: Entity) -> Option<Entity> {
        self.held.get(&entity).copied()
    }

    pub fn is_empty(&self) -> bool {
        self.held.is_empty()
    }
}

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
    registry: Res<XrdsTrackRegistry>,
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
            spawn_binding_track(&mut commands, target, source, binding, &registry, false);
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
fn spawn_binding_track(
    commands: &mut Commands,
    target: Entity,
    source: Option<Entity>,
    binding: &XrdsTriggerBinding,
    registry: &XrdsTrackRegistry,
    is_recovery: bool,
) {
    let Some(name) = binding.track.as_deref() else {
        // Authored but unwired. `track_diagnostics` already warns about this
        // at author time, so staying quiet here avoids a per-firing log spam
        // for a state the editor is already flagging.
        return;
    };
    match registry.0.get(name) {
        Some(track) => {
            spawn_track_agent_deferred(commands, target, source, name, track, 0, is_recovery);
        }
        None => log::warn!(
            "XrdsTriggerBinding.track named {name:?} on {target:?}, which has no matching entry              in XrdsSceneDocument::tracks — nothing fired."
        ),
    }
}

/// Resolves an authored [`XrdsActionTarget`] to a concrete entity.
///
/// Free function rather than a method so a Track's asset rows can be resolved
/// at spawn time — the asset locks have to be taken on real entities, and
/// `SelfNode`/`TriggerSource` only become concrete once a firing supplies
/// `self_entity`/`source`.
pub fn resolve_action_target(
    selector: &XrdsActionTarget,
    self_entity: Entity,
    source: Option<Entity>,
    id_index: Option<&XrdsIdIndex>,
) -> Option<Entity> {
    match selector {
        XrdsActionTarget::SelfNode => Some(self_entity),
        XrdsActionTarget::TriggerSource => source,
        XrdsActionTarget::Node(node_id) => {
            id_index.and_then(|index| index.entity_of((*node_id).into()))
        }
    }
}

/// Spawns a [`XrdsTrackAgent`] for one Track — absolute-time, concurrent
/// choreography over a set of assets, on its own clock.
///
/// Returns `None` without spawning when the Track has no events, or when
/// **any asset it drives is already held by another running Track**. That
/// second case is the reject-the-newcomer guard (plan doc §4): the refusal is
/// logged *and* recorded in [`XrdsTrackAssetLocks::last_conflict`], because a
/// silently-refused Track is otherwise indistinguishable from one that was
/// never triggered.
///
/// Re-firing a Track that is already running is **not** a conflict — the
/// running agent is despawned and a fresh one started, so a re-trigger
/// replays from the beginning rather than rejecting itself.
///
/// Takes `&mut World` rather than `Commands` so the conflict check, the lock
/// acquisition and the spawn happen as one atomic step. Two Tracks fired in
/// the same frame would otherwise both pass a deferred check and both
/// acquire.
/// Flattens a Track's rows into one time-sorted schedule, resolving each
/// row's target to a concrete entity.
///
/// Shared by [`spawn_track_agent_in_world`] and [`sync_live_track_agents`] on
/// purpose: a running agent re-reading the authored Track must schedule it
/// *identically* to a fresh spawn, and two copies of this would drift.
fn schedule_track_keys(
    track: &XrdsTrack,
    target: Entity,
    source: Option<Entity>,
    id_index: Option<&XrdsIdIndex>,
) -> Vec<XrdsTrackScheduledKey> {
    let resolved: Vec<Option<Entity>> = track
        .assets
        .iter()
        .map(|asset| resolve_action_target(&asset.target, target, source, id_index))
        .collect();

    let mut keys: Vec<XrdsTrackScheduledKey> = track
        .assets
        .iter()
        .zip(resolved)
        .flat_map(|(asset, entity)| {
            asset
                .keys
                .iter()
                .map(move |k| XrdsTrackScheduledKey {
                    at_secs: k.at_secs,
                    action: k.action.clone(),
                    entity,
                })
                .collect::<Vec<_>>()
        })
        .collect();
    keys.sort_by(|a, b| a.at_secs.total_cmp(&b.at_secs));
    keys
}

/// The distinct entities a schedule drives, sorted — the identity a re-sync
/// compares to decide whether an edit was structural.
fn scheduled_entities(keys: &[XrdsTrackScheduledKey]) -> Vec<Entity> {
    let mut out: Vec<Entity> = keys.iter().filter_map(|k| k.entity).collect();
    out.sort();
    out.dedup();
    out
}

pub fn spawn_track_agent_in_world(
    world: &mut World,
    target: Entity,
    source: Option<Entity>,
    name: &str,
    track: &XrdsTrack,
    chain_depth: u32,
    is_recovery: bool,
) -> Option<Entity> {
    if track.key_count() == 0 {
        return None;
    }

    // Flatten rows into one time-sorted schedule, resolving each row's target
    // now so the locks below are taken on real entities.
    let keys = {
        let id_index = world.get_resource::<XrdsIdIndex>();
        schedule_track_keys(track, target, source, id_index)
    };

    let mut wanted: Vec<Entity> = keys.iter().filter_map(|k| k.entity).collect();
    wanted.sort();
    wanted.dedup();

    // **No same-Track re-fire special case.** There used to be one here that
    // despawned any running agent with this Track's *name* before spawning, so
    // a second firing silently restarted the first.
    //
    // It was keyed too coarsely, and it hid the policy that already exists.
    // Locks key on resolved *entities*, so the guard below answers all three
    // cases correctly on its own:
    //
    // - Fired twice from the same source: same assets, so the second firing is
    //   refused and the first run keeps going.
    // - Fired from several sources onto *disjoint* assets (N instances of one
    //   panel template, each driving its own door via a `TriggerSource` row):
    //   they run concurrently, because nothing is contended.
    // - Fired from several sources onto the *same* asset: the first holds it,
    //   the rest are refused.
    //
    // That is one uniform rule — first run has priority, a running Track is
    // never preempted — instead of a name-based restart that made "3 buttons
    // pressed together" mean "only the last one did anything".
    //
    // Preempting is still possible, but only *explicitly*: the editor's preview
    // transport calls `preview_stop_track_in_world` before starting (which is
    // what makes the ⏮ restart button work), and the expert path has
    // `stop_sequences_on`/`stop_all_sequences`.
    world.init_resource::<XrdsTrackAssetLocks>();
    let agent = world.spawn_empty().id();

    {
        let mut locks = world.resource_mut::<XrdsTrackAssetLocks>();
        let contended = locks.conflicts(&wanted, agent);
        if !contended.is_empty() {
            locks.last_conflict = Some(XrdsTrackConflict {
                blocked_track: name.to_string(),
                contended: contended.clone(),
            });
            log::warn!(
                "Track {name:?} was not started: {} of its assets are already held by another                  running Track ({contended:?}). A Track runs whole or not at all — see the                  reject-the-newcomer policy.",
                contended.len()
            );
            drop(locks);
            world.despawn(agent);
            return None;
        }
        locks.acquire(&wanted, agent);
    }

    // Only looping Tracks need this, and only they pay for it: a one-shot
    // Track never laps, so there is nothing to put back.
    let initial = if track.looping {
        wanted.iter().map(|e| capture_asset_state(world, *e)).collect()
    } else {
        Vec::new()
    };

    let duration_secs = track.effective_duration_secs();
    world.entity_mut(agent).insert(XrdsTrackAgent {
        target,
        source,
        chain_depth,
        is_recovery,
        name: name.to_string(),
        keys,
        next_key_index: 0,
        elapsed_secs: 0.0,
        duration_secs,
        looping: track.looping,
        paused: false,
        initial,
    });
    Some(agent)
}

/// What one asset looked like when its Track started, so a looping Track can
/// put it back at the top of each lap.
///
/// Captured at *spawn*, not read from the authored document, and that is the
/// meaningful choice: a Track may be fired when its assets are somewhere the
/// document never said (another Track moved them, gameplay moved them, an
/// earlier lap of this same Track moved them). "Repeat this choreography from
/// where it began" is what an author means by loop; snapping to authored
/// values instead would teleport assets on the first lap boundary.
///
/// **Health is deliberately absent.** `ModifyHealth` accumulates gameplay
/// state, and restoring it every lap would make a looping health drain a
/// permanent no-op — the loop would undo exactly what it just did. Only
/// presentation state (where it is, whether you can see it, what it looks
/// like) is restored.
#[derive(Debug, Clone)]
pub struct XrdsTrackAssetInitial {
    entity: Entity,
    transform: Option<Transform>,
    visibility: Option<Visibility>,
    material: Option<XrdsMaterialParams>,
}

fn capture_asset_state(world: &mut World, entity: Entity) -> XrdsTrackAssetInitial {
    XrdsTrackAssetInitial {
        entity,
        transform: world.get::<Transform>(entity).copied(),
        visibility: world.get::<Visibility>(entity).copied(),
        material: material_params_for_entity_in_world(world, entity),
    }
}

/// Puts every captured asset back, and — the part that is easy to miss —
/// strips any in-flight [`XrdsTransformTween`] first.
///
/// Without that strip, a `SetTransform` still mid-glide when the lap wraps
/// keeps interpolating toward last lap's destination and overwrites the
/// restore a frame later, so the loop visibly drifts. Exactly the bug the
/// preview's stop path already had to solve.
fn restore_asset_states(world: &mut World, states: &[XrdsTrackAssetInitial]) {
    for state in states {
        if world.get_entity(state.entity).is_err() {
            continue; // despawned mid-Track; nothing to restore onto
        }
        world.entity_mut(state.entity).remove::<XrdsTransformTween>();
        if let Some(transform) = state.transform {
            if let Some(mut t) = world.get_mut::<Transform>(state.entity) {
                *t = transform;
            }
        }
        if let Some(visibility) = state.visibility {
            if let Some(mut v) = world.get_mut::<Visibility>(state.entity) {
                *v = visibility;
            }
        }
        if let Some(material) = state.material.clone() {
            set_material_params_for_entity_in_world(world, state.entity, material);
        }
    }
}

/// `Commands`-side wrapper around [`spawn_track_agent_in_world`], for use
/// from systems. The spawn is deferred to the command queue, so the returned
/// entity is not available to the caller.
pub fn spawn_track_agent_deferred(
    commands: &mut Commands,
    target: Entity,
    source: Option<Entity>,
    name: &str,
    track: &XrdsTrack,
    chain_depth: u32,
    is_recovery: bool,
) {
    let name = name.to_string();
    let track = track.clone();
    commands.queue(move |world: &mut World| {
        spawn_track_agent_in_world(
            world, target, source, &name, &track, chain_depth, is_recovery,
        );
    });
}

// ---------------------------------------------------------------------------
// Panel elements
// ---------------------------------------------------------------------------

/// Spawns one [`XrdsPanelElement`] onto `panel_entity` and tags it with the
/// element's authored triggers, returning the element's entity.
///
/// **This is the whole reason authored widget triggers can fire.** The chain
/// was already almost complete:
///
/// - The four widget events target the widget's own entity
///   (`XrdsTriggerRef::Entity(self.button_entity)`), not a document node.
/// - `XrdsTriggerRef::Entity(e).resolve()` returns `Some(e)` — a pass-through,
///   no id lookup, so nothing needed changing in [`consume_triggers`].
/// - But `consume_triggers` then requires [`XrdsTriggerBindings`] *on that
///   entity*, and the only thing that ever inserted it was
///   `tag_trigger_binding_entities`, which walks `document.nodes`. Widgets are
///   not nodes, so their entities never got one and every event was dropped.
///
/// So the missing piece was exactly this: an element carries its own triggers,
/// and they land on the entity the event will target. Nothing else.
///
/// Mirrors `tag_trigger_binding_entities`' **remove-when-empty** behaviour, so
/// clearing the last binding actually detaches the component rather than
/// leaving an empty list that still matches a query.
pub fn spawn_panel_element_in_world(
    world: &mut World,
    panel_entity: Entity,
    element: &xrds_scene_graph::XrdsPanelElement,
) -> Entity {
    let entity = crate::xrds_api::spawn::spawn_world_widget_from_scene(
        world,
        panel_entity,
        &element.kind,
    );
    set_element_trigger_bindings(world, entity, &element.triggers);
    entity
}

/// Attaches (or detaches) an element entity's trigger bindings.
///
/// Split out so re-authoring an element's triggers without respawning it uses
/// the same remove-when-empty rule as the initial spawn — two code paths
/// disagreeing about that is how an "unbound" element keeps firing.
pub fn set_element_trigger_bindings(
    world: &mut World,
    entity: Entity,
    triggers: &[XrdsTriggerBinding],
) {
    let Ok(mut e) = world.get_entity_mut(entity) else { return };
    if triggers.is_empty() {
        e.remove::<XrdsTriggerBindings>();
    } else {
        e.insert(XrdsTriggerBindings(triggers.to_vec()));
    }
}

// ---------------------------------------------------------------------------
// Editor preview transport
// ---------------------------------------------------------------------------

/// Marks the one [`XrdsTrackAgent`] started by the editor's preview transport.
///
/// Lets the editor pause/stop exactly its own preview without reaching into
/// Tracks that gameplay triggers started. Preview is deliberately single: two
/// simultaneous previews would fight over the same assets and the conflict
/// guard would just refuse the second, which would read as a bug.
#[derive(Component, Debug)]
pub struct XrdsTrackPreview;

/// Starts `name` as an editor preview, replacing any current preview.
///
/// Returns `None` when the Track is unknown, has no events, or has no asset row
/// that resolves to a real node — a Track made only of `SelfNode`/
/// `TriggerSource` rows has no meaningful preview, because those only become
/// concrete when a trigger actually fires and supplies them.
///
/// Note this still goes through the ordinary conflict guard: previewing a Track
/// whose assets a running Track already holds is refused, exactly as a trigger
/// firing would be. That is intentional — the preview should show you what
/// would really happen, including the refusal.
pub fn preview_play_track_in_world(world: &mut World, name: &str) -> Option<Entity> {
    let track = world.get_resource::<XrdsTrackRegistry>()?.0.get(name)?.clone();

    // Stop whatever was previewing first, so its locks are freed before the new
    // Track tries to claim them. Without this a preview could refuse itself.
    preview_stop_track_in_world(world);

    // A stand-in for `SelfNode` rows. There is no firing node during a preview,
    // so the first resolvable concrete row is the closest honest answer.
    let target = {
        let index = world.get_resource::<XrdsIdIndex>()?;
        track.assets.iter().find_map(|asset| match asset.target {
            XrdsActionTarget::Node(id) => index.entity_of(id.into()),
            _ => None,
        })
    };
    let Some(target) = target else {
        log::warn!(
            "Track {name:?} has no asset row resolving to a live node, so there is nothing to \
             preview. Rows targeting Self or the trigger source only become concrete when a \
             trigger fires."
        );
        return None;
    };

    let agent = spawn_track_agent_in_world(world, target, None, name, &track, 0, false)?;
    world.entity_mut(agent).insert(XrdsTrackPreview);
    Some(agent)
}

/// Pauses or resumes the preview. Returns whether a preview was found.
///
/// Pausing does **not** release the preview's asset locks — a paused Track
/// still owns its assets, so nothing else can start driving them mid-preview.
pub fn preview_pause_track_in_world(world: &mut World, paused: bool) -> bool {
    let Some(agent) = preview_agent(world) else { return false };
    if let Some(mut track_agent) = world.get_mut::<XrdsTrackAgent>(agent) {
        track_agent.paused = paused;
        return true;
    }
    false
}

/// Stops the preview and reports every node it was driving, so the caller can
/// put those nodes back where the document says they belong.
///
/// Restoring is the caller's job, not the runtime's: only the editor has the
/// authored document to restore *from*. This returns the ids and guarantees the
/// runtime-side cleanup — locks released, in-flight tweens stripped, agent gone.
///
/// Stripping the tweens matters: an `SetTransform` mid-flight leaves an
/// `XrdsTransformTween` on its target, and `advance_transform_tweens` would
/// happily keep driving it after the agent is gone, undoing the restore a frame
/// later.
pub fn preview_stop_track_in_world(world: &mut World) -> Vec<XrdsId> {
    let Some(agent) = preview_agent(world) else { return Vec::new() };

    // Collect the held entities *before* releasing, since the lock table is
    // where "what did this preview touch" is recorded.
    let entities: Vec<Entity> = world
        .get_resource::<XrdsTrackAssetLocks>()
        .map(|locks| locks.entities_held_by(agent))
        .unwrap_or_default();

    let ids: Vec<XrdsId> = {
        let index = world.get_resource::<XrdsIdIndex>();
        entities
            .iter()
            .filter_map(|e| index.and_then(|i| i.id_of(*e)))
            .collect()
    };

    for entity in &entities {
        if let Ok(mut e) = world.get_entity_mut(*entity) {
            e.remove::<XrdsTransformTween>();
        }
    }

    // Routed through the lock-releasing path rather than a bare despawn, so the
    // preview cannot leak locks the way any other despawn path must not.
    despawn_agents_releasing_locks(world, &[agent]);
    ids
}

/// The preview's `(name, elapsed, duration, playing)`, for the editor's
/// transport readout and playhead. `None` when nothing is previewing.
pub fn track_preview_state_in_world(world: &mut World) -> Option<(String, f32, f32, bool)> {
    let agent = preview_agent(world)?;
    let track_agent = world.get::<XrdsTrackAgent>(agent)?;
    Some((
        track_agent.name.clone(),
        track_agent.elapsed_secs(),
        track_agent.duration_secs(),
        !track_agent.paused,
    ))
}

fn preview_agent(world: &mut World) -> Option<Entity> {
    world
        .query_filtered::<Entity, (With<XrdsTrackAgent>, With<XrdsTrackPreview>)>()
        .iter(world)
        .next()
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
    let registry = world.resource::<XrdsTrackRegistry>().clone();
    for binding in &matching {
        // Same shape as consume_triggers: target is its own source, since
        // nothing external caused this.
        let mut commands = world.commands();
        spawn_binding_track(&mut commands, target, Some(target), binding, &registry, false);
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
    let registry = world.resource::<XrdsTrackRegistry>().clone();
    for binding in &matching {
        let mut commands = world.commands();
        spawn_binding_track(&mut commands, target, Some(target), binding, &registry, true);
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

    // A Track counts as "on this node" if it was fired at it *or* if one of
    // its asset rows drives it. The second case is new with the Track model:
    // a Track fired at one node routinely drives several others, and stopping
    // "sequences on X" ought to stop whatever is currently animating X.
    let holder = world
        .get_resource::<XrdsTrackAssetLocks>()
        .and_then(|locks| locks.holder_of(target));
    doomed.extend(
        world
            .query::<(Entity, &XrdsTrackAgent)>()
            .iter(world)
            .filter(|(entity, agent)| agent.target == target || Some(*entity) == holder)
            .map(|(entity, _)| entity),
    );
    doomed.sort();
    doomed.dedup();

    despawn_agents_releasing_locks(world, &doomed)
}

/// Cancels every in-flight sequence and Track in the world. Returns how many
/// were stopped.
pub fn stop_all_sequences_in_world(world: &mut World) -> usize {
    let mut doomed: Vec<Entity> = world
        .query_filtered::<Entity, With<XrdsSequenceAgent>>()
        .iter(world)
        .collect();
    doomed.extend(world.query_filtered::<Entity, With<XrdsTrackAgent>>().iter(world));
    despawn_agents_releasing_locks(world, &doomed)
}

/// Despawns agents and drops any asset locks they held.
///
/// Every despawn path must go through here. A despawn that skips the release
/// leaks the lock, and every Track sharing that asset is then blocked
/// forever — the failure mode is permanent and looks like "the trigger just
/// stopped working", so it is worth the single choke point.
fn despawn_agents_releasing_locks(world: &mut World, agents: &[Entity]) -> usize {
    if let Some(mut locks) = world.get_resource_mut::<XrdsTrackAssetLocks>() {
        for agent in agents {
            locks.release_agent(*agent);
        }
    }
    let mut count = 0;
    for agent in agents {
        if let Ok(entity) = world.get_entity_mut(*agent) {
            // Despawning drops the queue; bevy-sequential-actions reports
            // StopReason to each action's on_stop as it goes.
            entity.despawn();
            count += 1;
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
pub struct XrdsTrackAgent {
    /// The entity this firing was aimed at — what a `SelfNode` row resolved
    /// against. Kept for diagnostics; rows already carry resolved entities.
    pub target: Entity,
    pub source: Option<Entity>,
    pub chain_depth: u32,
    pub is_recovery: bool,
    /// Which registry Track this is running, so a re-fire can find and
    /// replace its own running instance.
    pub name: String,
    /// Flattened across every asset row and sorted ascending by `at_secs`
    /// once, at spawn time — authoring does not need to pre-sort.
    keys: Vec<XrdsTrackScheduledKey>,
    next_key_index: usize,
    elapsed_secs: f32,
    duration_secs: f32,
    looping: bool,
    /// Editor preview pause. A paused agent keeps its asset locks — pausing
    /// is not releasing.
    pub paused: bool,
    /// What each asset looked like when this Track started, for looping
    /// restore. Empty for a non-looping Track, which never laps.
    initial: Vec<XrdsTrackAssetInitial>,
}

impl XrdsTrackAgent {
    /// How far into the Track this agent has played.
    ///
    /// Public so the editor can draw a live playhead during preview. There is
    /// deliberately no setter: seeking would need every crossed key
    /// re-evaluated, which is a different feature (see plan doc §5).
    pub fn elapsed_secs(&self) -> f32 {
        self.elapsed_secs
    }

    pub fn duration_secs(&self) -> f32 {
        self.duration_secs
    }

    pub fn looping(&self) -> bool {
        self.looping
    }

    /// Whether this agent captured anything to restore at a lap boundary.
    /// Only looping Tracks do — see [`XrdsTrackAssetInitial`].
    pub fn has_initial_state(&self) -> bool {
        !self.initial.is_empty()
    }
}

/// One key with its row's target already resolved to an entity.
///
/// `entity` is `None` when the row's target could not be resolved — a
/// `Node(id)` naming a deleted node, or `TriggerSource` on a firing with no
/// source. Such keys are kept in the schedule (so timing is unaffected) and
/// skipped when fired.
#[derive(Debug, Clone)]
struct XrdsTrackScheduledKey {
    at_secs: f32,
    action: XrdsAction,
    entity: Option<Entity>,
}

/// Advances every in-flight [`XrdsTrackAgent`], firing every key crossed this
/// frame. Uses a `while` loop rather than a single `if`, so a long frame (or a
/// `duration_secs` shorter than one frame) never silently drops a key.
/// `duration_secs <= 0.0` fires every key immediately instead of hot-spinning
/// at one key per frame forever.
///
/// Despawning is done here rather than in a separate reaper so the asset locks
/// are released in the same step the agent goes away — a release that lags the
/// despawn would block every Track sharing those assets.
pub fn advance_tracks(
    time: Res<Time>,
    mut agents: Query<(Entity, &mut XrdsTrackAgent)>,
    mut locks: ResMut<XrdsTrackAssetLocks>,
    mut commands: Commands,
) {
    let dt = time.delta_secs();
    for (entity, mut agent) in &mut agents {
        if agent.paused {
            continue;
        }

        if agent.duration_secs <= 0.0 {
            while agent.next_key_index < agent.keys.len() {
                fire_track_key(&mut commands, &agent);
                agent.next_key_index += 1;
            }
            if agent.looping {
                agent.next_key_index = 0;
                // Deliberately no initial-state restore here. With no
                // duration every key fires every frame, so a "lap" is one
                // frame and the restore would be overwritten by the very
                // keys it preceded — there is no interval during which the
                // restored state would be visible. This configuration is
                // already reported by `track_diagnostics`.
            } else {
                locks.release_agent(entity);
                commands.entity(entity).despawn();
            }
            continue;
        }

        agent.elapsed_secs += dt;
        while agent.next_key_index < agent.keys.len()
            && agent.keys[agent.next_key_index].at_secs <= agent.elapsed_secs
        {
            fire_track_key(&mut commands, &agent);
            agent.next_key_index += 1;
        }

        if agent.elapsed_secs >= agent.duration_secs {
            if agent.looping {
                agent.elapsed_secs %= agent.duration_secs;
                agent.next_key_index = 0;

                // A lap starts from where the Track started, not from wherever
                // the previous lap left things. Queued *before* this lap's keys
                // below so the restore lands first at the command flush, and
                // the new lap's events then apply on top of a clean slate —
                // otherwise the restore would undo the lap it just began.
                //
                // The Track does not rewind the *world*: only the assets this
                // Track owns are touched, and only their presentation state.
                if !agent.initial.is_empty() {
                    let states = agent.initial.clone();
                    commands.queue(move |world: &mut World| {
                        restore_asset_states(world, &states);
                    });
                }

                // Fire any keys at/before the wrapped time right away, same
                // while-loop, so a key at 0.0 doesn't wait a full lap.
                while agent.next_key_index < agent.keys.len()
                    && agent.keys[agent.next_key_index].at_secs <= agent.elapsed_secs
                {
                    fire_track_key(&mut commands, &agent);
                    agent.next_key_index += 1;
                }
            } else {
                locks.release_agent(entity);
                commands.entity(entity).despawn();
            }
        }
    }
}

/// Re-reads the authored Track for every live agent, so edits made while a
/// Track is running actually take effect.
///
/// [`XrdsTrackAgent`] is a *snapshot* taken at spawn — that is what makes the
/// hot path cheap (no registry lookup per frame per agent) but it also meant a
/// running Track ignored every authored change. The reported symptom was
/// editing a looping Track's duration and watching it keep lapping at the old
/// one; the same staleness applied to `looping` itself and to key timings.
///
/// **Structural edits are deliberately not adopted.** If the set of resolved
/// asset entities changed (a row added, removed, or re-pointed), this leaves
/// the agent alone: adopting it would require rewriting
/// [`XrdsTrackAssetLocks`] mid-flight, and a mistake there leaks a lock and
/// blocks that asset for the rest of the session — the single worst failure
/// mode in this system. It would also invalidate the loop-restore baseline in
/// `initial`, which was captured for the old set. Re-fire the Track (the
/// editor's ⏮ restart) to pick a structural change up.
pub fn sync_live_track_agents(
    registry: Option<Res<XrdsTrackRegistry>>,
    id_index: Option<Res<XrdsIdIndex>>,
    mut agents: Query<&mut XrdsTrackAgent>,
) {
    let Some(registry) = registry else { return };

    for mut agent in &mut agents {
        let Some(track) = registry.0.get(&agent.name) else { continue };

        let rebuilt = schedule_track_keys(
            track,
            agent.target,
            agent.source,
            id_index.as_ref().map(|r| r.as_ref()),
        );

        // Structural change → skip entirely (see the doc comment above).
        if scheduled_entities(&rebuilt) != scheduled_entities(&agent.keys) {
            continue;
        }

        let new_duration = track.effective_duration_secs();
        let unchanged = agent.looping == track.looping
            && agent.duration_secs == new_duration
            && agent.keys.len() == rebuilt.len()
            && agent
                .keys
                .iter()
                .zip(&rebuilt)
                .all(|(a, b)| a.at_secs == b.at_secs && a.action == b.action);
        if unchanged {
            continue;
        }

        agent.looping = track.looping;
        agent.duration_secs = new_duration;
        agent.keys = rebuilt;

        // A shortened duration can leave the clock past the end; wrap it so the
        // new duration takes effect on this lap rather than after one more lap
        // at the old length.
        if agent.duration_secs > 0.0 && agent.elapsed_secs >= agent.duration_secs {
            agent.elapsed_secs %= agent.duration_secs;
        }

        // Re-derive the cursor from the clock: keys already in the past must not
        // re-fire, and keys newly moved into the future must still be able to.
        agent.next_key_index = agent
            .keys
            .iter()
            .take_while(|k| k.at_secs <= agent.elapsed_secs)
            .count();
    }
}

/// Fires one Track key by spawning it as its own one-step agent, against the
/// entity its **asset row** resolved to — not the Track-wide target. That
/// per-row targeting is the whole point of the Track model: one Track drives
/// several assets.
fn fire_track_key(commands: &mut Commands, agent: &XrdsTrackAgent) {
    let key = &agent.keys[agent.next_key_index];
    let Some(entity) = key.entity else {
        log::warn!(
            "Track {:?} has an event at {:.2}s whose asset could not be resolved to an entity              — skipping it. Most likely the node was deleted, or a TriggerSource row fired from              a trigger with no source.",
            agent.name,
            key.at_secs
        );
        return;
    };

    let runner = XrdsActionRunner::new(
        key.action.clone(),
        entity,
        agent.source,
        agent.chain_depth,
        agent.is_recovery,
    );
    let one_step = commands
        .spawn((
            SequentialActions,
            XrdsSequenceAgent {
                target: entity,
                source: agent.source,
                chain_depth: agent.chain_depth,
                is_recovery: agent.is_recovery,
            },
        ))
        .id();
    commands.actions(one_step).add(Box::new(runner) as BoxedAction);
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

/// Runtime-only state for an in-flight [`XrdsAction::SetTransform`],
/// inserted on the *target* entity (not the agent) so overlapping/back-
/// to-back tweens on different targets never collide. Removed by
/// [`advance_transform_tweens`] once `elapsed >= duration` — its absence
/// is exactly what `XrdsActionRunner::is_finished` polls for, same pattern
/// as `Run { wait: true }` polling for its child agent's despawn.
#[derive(Component, Debug, Clone)]
pub(crate) struct XrdsTransformTween {
    start: Transform,
    target: Transform,
    elapsed: f32,
    duration: f32,
    ease: XrdsEaseCurve,
}

/// Maps `t` in `0.0..=1.0` through the given ease-out curve.
fn ease_out(curve: XrdsEaseCurve, t: f32) -> f32 {
    match curve {
        XrdsEaseCurve::Linear => t,
        XrdsEaseCurve::Quad => 1.0 - (1.0 - t) * (1.0 - t),
        XrdsEaseCurve::Cubic => 1.0 - (1.0 - t).powi(3),
    }
}

/// Advances every in-flight [`XrdsTransformTween`] by this frame's delta
/// time, applying the eased position/rotation/scale to `Transform`, and
/// removes the component once it reaches its duration — the runtime
/// counterpart to `XrdsAction::SetTransform`'s authored data. Runs in
/// `Update`, ahead of `SequentialActionsPlugin`'s own advancement later in
/// the frame, so `XrdsActionRunner::is_finished` sees this frame's result
/// immediately rather than one frame late.
///
/// **Pause-aware.** `advance_tracks` skipping a paused agent only stops it
/// from firing *new* keys — a `SetTransform` already mid-flight lives here,
/// as a tween on the *target* entity with no link back to the agent that
/// started it. Without checking that agent's `paused` flag, pausing a Track
/// looked like it did nothing whenever the pause landed mid-tween: the tween
/// kept gliding to completion regardless. So every frame, this collects the
/// entities held by any currently-paused agent (via `XrdsTrackAssetLocks`,
/// the same map `advance_tracks` uses to know what a Track owns) and skips
/// them — frozen only for the Track that is actually paused, not for any
/// other Track that happens to be running concurrently.
pub fn advance_transform_tweens(
    time: Res<Time>,
    agents: Query<(Entity, &XrdsTrackAgent)>,
    locks: Res<XrdsTrackAssetLocks>,
    mut query: Query<(Entity, &mut Transform, &mut XrdsTransformTween)>,
    mut commands: Commands,
) {
    let frozen: std::collections::HashSet<Entity> = agents
        .iter()
        .filter(|(_, agent)| agent.paused)
        .flat_map(|(agent_entity, _)| locks.entities_held_by(agent_entity))
        .collect();

    let dt = time.delta_secs();
    for (entity, mut transform, mut tween) in &mut query {
        if frozen.contains(&entity) {
            continue;
        }
        tween.elapsed += dt;
        let t = (tween.elapsed / tween.duration).clamp(0.0, 1.0);
        let eased = ease_out(tween.ease, t);
        transform.translation = tween.start.translation.lerp(tween.target.translation, eased);
        transform.rotation = tween.start.rotation.slerp(tween.target.rotation, eased);
        transform.scale = tween.start.scale.lerp(tween.target.scale, eased);
        if t >= 1.0 {
            commands.entity(entity).remove::<XrdsTransformTween>();
        }
    }
}

impl Action for XrdsActionRunner {
    fn is_finished(&self, _agent: Entity, world: &World) -> bool {
        match &self.action {
            // The only action with a duration of its own. Blocks until
            // `advance_transform_tweens` removes the tween component from the
            // target, keyed on component absence rather than a stored
            // deadline.
            //
            // Each Track key runs as its own one-step agent, so blocking here
            // never delays the Track's advancement — it only keeps this one
            // ephemeral agent alive for the length of the tween.
            XrdsAction::SetTransform { .. } => {
                world.get::<XrdsTransformTween>(self.target).is_none()
            }
            // Everything else applies instantly in `on_start`. glTF playback
            // is fire-and-forget: the agent finishes as soon as playback is
            // requested rather than waiting for the clip to end. Waiting on
            // clip completion wants a `TrackComplete`-style trigger, not a
            // blocking action, and is not assumed here.
            _ => true,
        }
    }

    fn on_start(&mut self, _agent: Entity, world: &mut World) -> bool {
        match self.action.clone() {
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

            XrdsAction::SetTransform { position, rotation, scale, duration_secs, ease } => {
                let Some(start) = world.get::<Transform>(self.target).copied() else {
                    log::warn!(
                        "XrdsAction::SetTransform on {:?}, which has no Transform — ignoring.",
                        self.target
                    );
                    return true;
                };
                let target = Transform {
                    translation: position.map(Vec3::from_array).unwrap_or(start.translation),
                    rotation: rotation
                        .map(|r| {
                            Quat::from_euler(
                                EulerRot::XYZ,
                                r[0].to_radians(),
                                r[1].to_radians(),
                                r[2].to_radians(),
                            )
                        })
                        .unwrap_or(start.rotation),
                    scale: scale.map(Vec3::from_array).unwrap_or(start.scale),
                };
                // duration_secs <= 0.0 applies instantly rather than
                // inserting a tween that would need a whole extra frame
                // (and a divide-by-zero-shaped clamp) to resolve — same
                // "instant when zero" treatment `Wait`/`advance_timelines`
                // already give a non-positive duration.
                if duration_secs <= 0.0 {
                    if let Some(mut transform) = world.get_mut::<Transform>(self.target) {
                        *transform = target;
                    }
                    true
                } else {
                    world.entity_mut(self.target).insert(XrdsTransformTween {
                        start,
                        target,
                        elapsed: 0.0,
                        duration: duration_secs,
                        ease,
                    });
                    false
                }
            }

            XrdsAction::SetMaterial { base_color, metallic, roughness, texture } => {
                // No own target any more — always applies to `self.target`,
                // the entity this key's *row* resolved to, same as every
                // other action. See the variant's doc comment.
                let entity = self.target;
                if let Some(mut params) = material_params_for_entity_in_world(world, entity) {
                    if let Some(rgba) = base_color {
                        params.base_color = XrdsColor { rgba };
                    }
                    if let Some(m) = metallic {
                        params.pbr.metallic = m;
                    }
                    if let Some(r) = roughness {
                        params.pbr.roughness = r;
                    }
                    // Only the named slot is touched, so assigning a base
                    // colour map cannot silently drop an authored normal map.
                    // `None` clears that slot. The id → uri → `Handle<Image>`
                    // resolution happens inside the apply below, off the
                    // imported asset catalog — nothing extra is needed here.
                    if let Some(t) = texture {
                        params.textures.set(
                            t.slot.into(),
                            t.texture_asset_id.as_ref().map(|id| XrdsMaterialTextureRef {
                                texture_asset_id: id.clone(),
                                uv: Default::default(),
                                sampler: Default::default(),
                            }),
                        );
                    }
                    set_material_params_for_entity_in_world(world, entity, params);
                } else {
                    log::warn!(
                        "XrdsAction::SetMaterial targeted entity {entity:?}, which has no \
                         material — ignoring."
                    );
                }
                true
            }

            XrdsAction::ModifyHealth { delta } => {
                let amount = self.resolve_value(&delta, world);
                let entity = self.target;
                if let Some(mut health) = world.get_mut::<XrdsHealth>(entity) {
                    health.0 += amount;
                } else {
                    log::warn!(
                        "XrdsAction::ModifyHealth targeted entity {entity:?}, which has no \
                         XrdsHealth component — ignoring."
                    );
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
