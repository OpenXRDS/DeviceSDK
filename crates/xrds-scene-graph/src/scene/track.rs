//! Tracks — the single authored execution model for the sequencer.
//!
//! # Terminology
//!
//! - **Timeline** is the *ruler*: the time axis a Track is authored
//!   against. It is not a data type; it has no representation here.
//! - **Track** is a timeline-based sequence: a set of assets, each with
//!   action events pinned to absolute local times. A Track runs on its own
//!   clock, starting when a trigger fires it.
//! - **Asset row** is one node's lane within a Track. A node appears at
//!   most once per Track, so all of that node's events live on one row.
//!
//! # Why there is only one execution model
//!
//! This file previously also held `XrdsSequence` — a relative,
//! completion-chained queue — alongside the absolute-time timeline, as two
//! genuinely different execution models. That distinction turned out to be
//! illusory *for the action set that actually exists*: every action that
//! blocked a queue blocked for a duration known at author time (`Wait`'s
//! seconds, `AnimateTransform`'s `duration_secs`), and the one action whose
//! length is not knowable until runtime — glTF playback — was deliberately
//! non-blocking. So any sequence could be converted to absolute times by
//! accumulating authored durations, mechanically and losslessly.
//!
//! Two names for one model caused repeated real confusion, so the queue is
//! gone. `Wait` went with it (a key already carries its own time), as did
//! `Run` and `FireCustomEvent` — a Track cannot start another Track, by
//! design. Composition is "bind several Tracks to one trigger"; sequential
//! Track-to-Track chaining, when it is needed, wants a `TrackComplete`
//! trigger kind rather than a `Run` action.
//!
//! There is deliberately **no legacy-format migration**. Nothing had been
//! persisted in the previous schema when it changed, so there is nothing to
//! convert, and carrying a migration path for a format no document is
//! written in would be dead code plus a standing "did every load path call
//! `normalize`?" obligation. Forward compatibility is a separate concern and
//! is still handled — see [`XrdsAction::Unknown`].

use super::*;

/// One action event on an asset row, at an absolute local time within its
/// Track.
///
/// Carries no target of its own — the owning [`XrdsTrackAsset`] names the
/// node. That is what makes a Track's rows node-scoped rather than
/// action-category-scoped.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct XrdsTrackKey {
    pub at_secs: f32,
    pub action: XrdsAction,
}

/// One asset's row within a [`XrdsTrack`].
///
/// A node appears at most once per Track (enforced by `track_diagnostics`),
/// so this is the single place all of that node's events live. Two keys
/// sharing an `at_secs` is not an error — that is how concurrent change on
/// one asset is expressed (move it *and* recolour it on the same beat).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct XrdsTrackAsset {
    /// Which node this row drives. `Node(id)` is the normal case;
    /// `SelfNode`/`TriggerSource` resolve at fire time and so cannot
    /// participate in authoring-time conflict checks.
    #[serde(default)]
    pub target: XrdsActionTarget,
    #[serde(default)]
    pub keys: Vec<XrdsTrackKey>,
}

/// A timeline-based sequence: absolute-time, concurrent choreography over a
/// set of assets, on its own clock, started by a trigger.
///
/// Authoring does not need to pre-sort keys; the runtime scheduler sorts
/// once when a Track starts.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct XrdsTrack {
    #[serde(default)]
    pub assets: Vec<XrdsTrackAsset>,
    /// Defaults to the latest key's `at_secs` (plus its own interpolation
    /// tail) when absent — see [`XrdsTrack::effective_duration_secs`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_secs: Option<f32>,
    #[serde(default)]
    pub looping: bool,
}

impl XrdsTrack {
    /// Every key across every row, paired with the row's target, sorted by
    /// time. The order the runtime scheduler wants.
    pub fn flattened_keys(&self) -> Vec<(XrdsActionTarget, &XrdsTrackKey)> {
        let mut out: Vec<(XrdsActionTarget, &XrdsTrackKey)> = self
            .assets
            .iter()
            .flat_map(|a| a.keys.iter().map(move |k| (a.target.clone(), k)))
            .collect();
        out.sort_by(|(_, a), (_, b)| a.at_secs.total_cmp(&b.at_secs));
        out
    }

    pub fn key_count(&self) -> usize {
        self.assets.iter().map(|a| a.keys.len()).sum()
    }

    /// Authored duration, or the span the keys actually occupy.
    ///
    /// The fallback includes each key's own interpolation tail, so a Track
    /// whose last key animates for two seconds does not report a duration
    /// that cuts that animation off.
    pub fn effective_duration_secs(&self) -> f32 {
        if let Some(d) = self.duration_secs {
            return d;
        }
        self.assets
            .iter()
            .flat_map(|a| a.keys.iter())
            .map(|k| k.at_secs + k.action.self_duration_secs())
            .fold(0.0f32, f32::max)
    }

    /// Every concrete node this Track drives. `SelfNode`/`TriggerSource`
    /// rows are excluded — they have no authoring-time identity, so they
    /// cannot take part in the cross-Track conflict check.
    pub fn owned_nodes(&self) -> Vec<XrdsSceneNodeId> {
        self.assets
            .iter()
            .filter_map(|a| match a.target {
                XrdsActionTarget::Node(id) => Some(id),
                _ => None,
            })
            .collect()
    }
}

/// A named entry in the document-level Track registry
/// (`XrdsSceneDocument::tracks`).
///
/// Trigger bindings reference a Track by name, so the same choreography can
/// be fired from many places and edited in one.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct XrdsNamedTrack {
    pub name: String,
    pub track: XrdsTrack,
}
