//! Authored trigger-action sequencing data. See
//! `docs/xrds-scenegraph-trigger-action-sequencing.md` for the design
//! rationale and `docs/xrds-trigger-action-implementation-plan.md` for the
//! phased build-out this belongs to (Phase 1).
//!
//! Everything in this file is plain, closed-vocabulary document data — no
//! script/behavior/branching logic. Execution state (which step is
//! currently running) is a separate, runtime-only concern and never lives
//! here.

use super::*;

/// One parameterized, closed-vocabulary effect. This enum is the actual
/// Blueprint/Verse-avoidance guarantee for the sequencer — no arbitrary
/// logic, no branching, just named operations with data. Grows by adding
/// variants, same cost model as adding a new `XrdsSceneNodePayload` kind.
/// See `docs/xrds-trigger-action-backlog.md` for candidate future
/// variants not yet promoted into this enum.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum XrdsAction {
    /// Reuses the existing `XrdsSceneGltfPlayback` (selector + repeat +
    /// speed + start_paused) already defined for glTF node authoring —
    /// no new mirror type needed.
    PlayGltfAnimation {
        playback: XrdsSceneGltfPlayback,
    },
    StopGltfAnimation,
    SetVisible(bool),
    Teleport {
        destination: [f32; 3],
    },
    ModifyHealth {
        #[serde(default)]
        target: XrdsActionTarget,
        delta: XrdsActionValue,
    },
    Wait {
        seconds: f32,
    },
    /// Escape hatch into expert-layer Rust for anything not yet modeled as
    /// a first-class variant above.
    FireCustomEvent {
        name: String,
    },
}

/// Which entity an `XrdsAction` applies to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum XrdsActionTarget {
    /// The node the sequence is authored on.
    #[default]
    SelfNode,
    /// An explicitly-named other node.
    Node(XrdsSceneNodeId),
    /// Whichever entity fired the trigger (e.g. the bullet, not the
    /// player it hit).
    TriggerSource,
}

/// A value baked in at author time, or pulled from whatever fired the
/// trigger. `FromTriggerSource` is resolved at execution time by reading
/// a generic `XrdsTriggerValue` component off the trigger's source entity
/// — see the implementation plan's Phase 3/4 for the accessor mechanism
/// (Option C in the design doc: gameplay code sets the value, the
/// sequencer only reads it, never computes it).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum XrdsActionValue {
    Fixed(f32),
    FromTriggerSource,
}

impl Default for XrdsActionValue {
    fn default() -> Self {
        Self::Fixed(0.0)
    }
}

/// An ordered list of `XrdsAction`s — purely data, no execution state.
/// Runs sequentially, matching `bevy-sequential-actions`' actual, spike-
/// verified behavior (see the design doc's evaluation section).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct XrdsSequence {
    #[serde(default)]
    pub steps: Vec<XrdsAction>,
}

/// The recognized trigger kinds an author can pick from today (e.g. in an
/// `xrds-editor` dropdown). Grows by one variant each time a new
/// `XrdsTriggerEvent` implementor is wired in on the runtime side — see
/// the design doc's "open/pluggable trigger mechanism" section for why
/// that's cheap (one trait impl + one system registration, no changes
/// here).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum XrdsTriggerKind {
    #[default]
    ZoneEnter,
    ZoneExit,
    /// Every glTF animation on this node has finished playing. Never fires
    /// for `Loop` playback (a looping clip has no completion, by
    /// definition) nor for playback ended by an explicit stop — only for
    /// a clip that ran to its end.
    ///
    /// This is what makes "play an animation, then do something else"
    /// expressible: put the follow-up in a second binding keyed on this
    /// kind, rather than needing the first sequence to block.
    AnimationComplete,
}

/// "When trigger kind K fires for this node, run sequence S." A node can
/// have several bindings (e.g. one for `ZoneEnter`, a different one for
/// `ZoneExit`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct XrdsTriggerBinding {
    #[serde(default)]
    pub trigger: XrdsTriggerKind,
    #[serde(default)]
    pub sequence: XrdsSequence,
}
