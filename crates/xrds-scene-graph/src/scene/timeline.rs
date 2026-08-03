//! Timeline-based composition and the runnable registry (Phase 9 / 9a).
//! See `docs/xrds-trigger-action-implementation-plan.md` for the design —
//! in particular the terminology section distinguishing a *timeline* from
//! `XrdsSequence` (an ordered queue). They are genuinely different
//! execution models, not two names for the same thing; conflating them
//! already caused one real misunderstanding mid-build.

use super::*;

/// One key on a timeline: run `action` at `at_secs`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct XrdsTimelineKey {
    pub at_secs: f32,
    pub action: XrdsAction,
}

/// Absolute-time, concurrent choreography — distinct from `XrdsSequence`'s
/// relative, completion-chained queue. Authoring does not need to
/// pre-sort `keys`; the runtime scheduler sorts once when a timeline
/// starts. Two keys sharing a timestamp is not an error — that IS the
/// concurrency mechanism a queue cannot express.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct XrdsTimeline {
    #[serde(default)]
    pub keys: Vec<XrdsTimelineKey>,
    /// Defaults to the last key's `at_secs` when absent.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub duration_secs: Option<f32>,
    #[serde(default)]
    pub looping: bool,
}

/// Either execution model — whatever an author actually wants for one
/// registry entry or one binding.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum XrdsRunnable {
    Sequence(XrdsSequence),
    Timeline(XrdsTimeline),
}

impl Default for XrdsRunnable {
    fn default() -> Self {
        Self::Sequence(XrdsSequence::default())
    }
}

/// A named entry in the document-level runnable registry
/// (`XrdsSceneDocument::runnables`).
///
/// This is the *template* half of the template/instance split: a registry
/// entry is "do Y", a trigger binding referencing it by name is "under X,
/// on this node, do Y" — the instance. Editing a template affects every
/// binding that names it; a binding with no name set instead runs its own
/// inline `sequence` (see `XrdsTriggerBinding::runnable`), affecting only
/// that one binding.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct XrdsNamedRunnable {
    pub name: String,
    pub runnable: XrdsRunnable,
}
