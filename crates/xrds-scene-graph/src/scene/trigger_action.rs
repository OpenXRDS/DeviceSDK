//! Authored trigger-action sequencing data. See
//! `docs/done/xrds-scenegraph-trigger-action-sequencing.md` for the design
//! rationale and `docs/done/xrds-trigger-action-v1.md` for the
//! implementation record (this file is Phases 1-2, 7 and 10).
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
///
/// **Adjacently tagged** (`{"kind": "...", "data": ...}`) rather than
/// serde's default external tagging. That is what allows the `Unknown`
/// fallback below: `#[serde(other)]` is only permitted on internally- or
/// adjacently-tagged enums, and internal tagging cannot represent the
/// newtype variant `SetVisible(bool)`.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "kind", content = "data")]
pub enum XrdsAction {
    /// Reuses the existing `XrdsSceneGltfPlayback` (selector + repeat +
    /// speed + start_paused) already defined for glTF node authoring —
    /// no new mirror type needed.
    PlayGltfAnimation {
        playback: XrdsSceneGltfPlayback,
    },
    StopGltfAnimation,
    SetVisible(bool),
    /// Moves translation/rotation/scale toward the given values over
    /// `duration_secs`, easing by `ease`. Each of `position`/`rotation`/`scale`
    /// is independently optional — `None` leaves that component untouched,
    /// matching how a partial keyframe override (e.g. only Position Y, not
    /// Rotation) is expressed in the editor.
    ///
    /// `rotation` is Euler XYZ degrees, matching [`XrdsAxis`]'s convention
    /// elsewhere in this module.
    ///
    /// **`duration_secs <= 0.0` applies instantly.** That case is what a
    /// separate `Teleport` action used to be, and it was deleted as redundant:
    /// the runtime's zero-duration path writes the target transform directly,
    /// taking the current rotation/scale for unset fields — byte-for-byte what
    /// `Teleport` did, except this can also set rotation and scale instantly,
    /// which `Teleport` could not. Named `SetTransform`, not
    /// `AnimateTransform`: "animate" would misdescribe half its uses.
    ///
    /// This is the only action with a duration of its own. Each Track key runs
    /// as its own one-step agent (see the runtime's `fire_track_key`), so that
    /// duration never delays the Track's own advancement — it only decides how
    /// long this one tween takes, which is what the editor draws as a bar
    /// rather than a dot.
    SetTransform {
        position: Option<[f32; 3]>,
        rotation: Option<[f32; 3]>,
        scale: Option<[f32; 3]>,
        duration_secs: f32,
        #[serde(default)]
        ease: XrdsEaseCurve,
    },
    /// Instant material override. Each field independently optional —
    /// `None` leaves that property as whatever it already was. No
    /// interpolated counterpart yet (unlike `SetTransform`) — add one later by
    /// reusing `SetTransform`'s interpolator infrastructure if a real case
    /// needs it.
    ///
    /// No `target` field: applies to whichever asset row it sits on, same as
    /// every other action. It used to carry its own target — a leftover from
    /// before rows were asset-scoped — which meant it could silently apply to
    /// a *different* node than its row, invisibly to the cross-Track conflict
    /// check (see the deleted "Action escapes its asset row" diagnostic).
    SetMaterial {
        base_color: Option<[f32; 4]>,
        metallic: Option<f32>,
        roughness: Option<f32>,
        /// Swaps the image in **one** texture slot. `None` leaves every slot
        /// alone; `Some` with a `texture_asset_id` of `None` *clears* that one
        /// slot.
        ///
        /// One slot per event, deliberately: a whole-slot-set replacement
        /// would make "set the base colour map" silently drop an authored
        /// normal map. Driving several slots at one instant is just several
        /// events sharing a timestamp on the same row, which the sequencer
        /// stacks into sub-lanes.
        #[serde(default)]
        texture: Option<XrdsActionTexture>,
    },
    /// No `target` field, for the same reason as [`SetMaterial`](XrdsAction::SetMaterial).
    ModifyHealth {
        delta: XrdsActionValue,
    },
    /// Any action variant this build does not recognize.
    ///
    /// **Why this exists:** without it, a document written by a newer
    /// editor (say, containing a `PlayAudio` action from the backlog) would
    /// fail to deserialize on an older runtime — and because the action is
    /// nested inside the document, the *entire scene* would fail to load,
    /// not just that one step. This is a realistic scenario here: scenes
    /// get pushed to a Quest APK that may lag the editor.
    ///
    /// An unrecognized action degrades to a logged no-op instead.
    ///
    /// **Lossy, deliberately:** an older build that loads and re-exports a
    /// document will drop actions it did not understand rather than
    /// preserve them byte-for-byte. Total parse failure is the worse harm.
    ///
    /// Not `#[serde(other)]`: that attribute requires a unit variant, and
    /// the derive still feeds the adjacent `data` payload to whichever
    /// variant it lands on — including the fallback — so a payload-carrying
    /// unknown action (e.g. a future `PlayAudio { clip }`) would fail to
    /// deserialize and take the *entire* document down with it, defeating
    /// the point. See the hand-written `Deserialize` impl below, which
    /// checks the tag against the known set *before* touching `data` at
    /// all, so an unrecognized tag never gets its payload parsed.
    Unknown,
}

/// One texture-slot assignment for [`XrdsAction::SetMaterial`].
///
/// Only the asset id is authored here — `uv`/`sampler` on the underlying
/// [`XrdsSceneTextureRef`] keep their defaults. Per-event UV offset/tiling is
/// a real thing to want eventually (scrolling a texture over time), but it is
/// a separate feature: it needs interpolation to be useful, and this variant
/// applies instantly.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct XrdsActionTexture {
    pub slot: XrdsSceneMaterialTextureSlotKind,
    /// Id of a `XrdsSceneAssetKind::Texture` entry in the document's asset
    /// catalog. `None` clears the slot instead of assigning one.
    #[serde(default)]
    pub texture_asset_id: Option<String>,
}

/// Shadow of [`XrdsAction`] holding only its real variants, used solely by
/// the `Deserialize` impl below. Must be kept in sync by hand whenever a
/// variant is added to `XrdsAction` — same cost as growing `XrdsAction`
/// itself, and the reason `KNOWN_ACTION_KINDS` sits right next to it.
#[derive(Deserialize)]
#[serde(tag = "kind", content = "data")]
enum XrdsActionKnown {
    PlayGltfAnimation {
        playback: XrdsSceneGltfPlayback,
    },
    StopGltfAnimation,
    SetVisible(bool),
    SetTransform {
        position: Option<[f32; 3]>,
        rotation: Option<[f32; 3]>,
        scale: Option<[f32; 3]>,
        duration_secs: f32,
        #[serde(default)]
        ease: XrdsEaseCurve,
    },
    SetMaterial {
        base_color: Option<[f32; 4]>,
        metallic: Option<f32>,
        roughness: Option<f32>,
        #[serde(default)]
        texture: Option<XrdsActionTexture>,
    },
    ModifyHealth {
        delta: XrdsActionValue,
    },
}

impl From<XrdsActionKnown> for XrdsAction {
    fn from(known: XrdsActionKnown) -> Self {
        match known {
            XrdsActionKnown::PlayGltfAnimation { playback } => {
                XrdsAction::PlayGltfAnimation { playback }
            }
            XrdsActionKnown::StopGltfAnimation => XrdsAction::StopGltfAnimation,
            XrdsActionKnown::SetVisible(visible) => XrdsAction::SetVisible(visible),
            XrdsActionKnown::SetTransform { position, rotation, scale, duration_secs, ease } => {
                XrdsAction::SetTransform { position, rotation, scale, duration_secs, ease }
            }
            XrdsActionKnown::SetMaterial { base_color, metallic, roughness, texture } => {
                XrdsAction::SetMaterial { base_color, metallic, roughness, texture }
            }
            XrdsActionKnown::ModifyHealth { delta } => {
                XrdsAction::ModifyHealth { delta }
            }
        }
    }
}

/// Wire tag strings for every real `XrdsAction` variant — the exact tag
/// values `#[serde(tag = "kind")]` produces, i.e. the Rust identifiers,
/// since no variant renames `kind`. Add to this whenever a variant is added.
const KNOWN_ACTION_KINDS: &[&str] = &[
    "PlayGltfAnimation",
    "StopGltfAnimation",
    "SetVisible",
    "SetTransform",
    "SetMaterial",
    "ModifyHealth",
];

impl<'de> Deserialize<'de> for XrdsAction {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        // Capture the whole `{"kind": ..., "data": ...}` shape as a generic
        // value first, so the tag can be checked *before* `data` is parsed
        // against anything. That ordering is the entire fix: it is what lets
        // an unrecognized tag skip past its payload instead of the derive
        // trying (and failing) to fit that payload into a unit variant.
        let value = serde_json::Value::deserialize(deserializer)?;
        let kind = value.get("kind").and_then(serde_json::Value::as_str).unwrap_or_default();
        if !KNOWN_ACTION_KINDS.contains(&kind) {
            return Ok(XrdsAction::Unknown);
        }
        XrdsActionKnown::deserialize(value)
            .map(XrdsAction::from)
            .map_err(serde::de::Error::custom)
    }
}

/// Ease curve for [`XrdsAction::SetTransform`]. Each variant is
/// implicitly ease-*out* only for v1 (matching how these are commonly
/// shown/labeled in NLE-style tools) — `Linear` has no direction to speak
/// of, so it's exact either way. `QuadIn`/`QuadInOut`/`CubicIn`/
/// `CubicInOut` are cheap to add later as new variants if a real case
/// needs them; not built now since nothing has asked for one yet.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum XrdsEaseCurve {
    Linear,
    Quad,
    #[default]
    Cubic,
}

impl XrdsAction {
    /// How long this action itself takes, in seconds. Non-zero only for
    /// [`SetTransform`](XrdsAction::SetTransform) — every other
    /// action applies instantly, including glTF playback, which is
    /// deliberately fire-and-forget (requesting playback completes at
    /// once; the clip's own length is not waited on).
    ///
    /// Drives a Track's fallback duration and the editor's dot-vs-bar
    /// rendering, so both agree on what "instant" means without either
    /// re-deriving it.
    pub fn self_duration_secs(&self) -> f32 {
        match self {
            XrdsAction::SetTransform { duration_secs, .. } => duration_secs.max(0.0),
            _ => 0.0,
        }
    }

    /// Whether this action is valid inside a Track.
    ///
    /// Everything in the current vocabulary is, which is the point: the
    /// actions that were not (`Wait`, `Run`, `FireCustomEvent`) no longer
    /// exist. Only `Unknown` is rejected, so a document from a *newer*
    /// editor reports one clear diagnostic rather than silently dropping a
    /// key.
    pub fn is_valid_in_track(&self) -> bool {
        !matches!(self, XrdsAction::Unknown)
    }

}

impl XrdsTriggerKind {
    /// Whether this kind's runtime event reports which hand caused it.
    ///
    /// A `hand` filter on a kind that reports none can never match, making
    /// the binding permanently and silently unfireable — see
    /// `track_diagnostics`. Kept as one method rather than an inline
    /// `matches!` so the editor's picker and the diagnostic cannot drift
    /// apart on which kinds qualify.
    pub fn carries_hand(&self) -> bool {
        matches!(
            self,
            XrdsTriggerKind::Grabbed
                | XrdsTriggerKind::Dropped
                | XrdsTriggerKind::HoverEnter
                | XrdsTriggerKind::HoverExit
                | XrdsTriggerKind::ButtonPress
                | XrdsTriggerKind::ButtonRelease
                | XrdsTriggerKind::SliderChange
                | XrdsTriggerKind::ToggleChange
        )
    }
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


/// The recognized trigger kinds an author can pick from today (e.g. in an
/// `xrds-editor` dropdown). Grows by one variant each time a new
/// `XrdsTriggerEvent` implementor is wired in on the runtime side — see
/// the design doc's "open/pluggable trigger mechanism" section for why
/// that's cheap (one trait impl + one system registration, no changes
/// here).
/// Not `Copy`: the `Custom` variant carries a `String`.
///
/// Adjacently tagged for the same reason as [`XrdsAction`] — see the
/// `Unknown` variant at the bottom.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(tag = "kind", content = "data")]
pub enum XrdsTriggerKind {
    // --- Interaction zones ---
    #[default]
    ZoneEnter,
    ZoneExit,

    // --- XR grab interaction ---
    /// This entity was picked up by a controller.
    Grabbed,
    /// This entity was released.
    Dropped,

    // --- World-space UI ---
    /// A pointer ray entered this panel's surface.
    HoverEnter,
    /// A pointer ray left this panel's surface.
    HoverExit,
    /// This world-UI button was pressed.
    ButtonPress,
    /// This world-UI button was released.
    ButtonRelease,
    /// This world-UI slider's value changed. **The new value is not
    /// currently reachable from a sequence** — see the note on
    /// `XrdsActionValue::FromTriggerSource`.
    SliderChange,
    /// This world-UI toggle flipped. Same value caveat as `SliderChange`.
    ToggleChange,

    // --- Animation ---
    /// Every glTF animation on this node has finished playing. Never fires
    /// for `Loop` playback (a looping clip has no completion, by
    /// definition) nor for playback ended by an explicit stop — only for
    /// a clip that ran to its end.
    ///
    /// This is what makes "play an animation, then do something else"
    /// expressible: put the follow-up in a second binding keyed on this
    /// kind, rather than needing the first sequence to block.
    AnimationComplete,

    /// An application-defined trigger, matched by name.
    ///
    /// The inbound counterpart to [`XrdsAction::FireCustomEvent`]: that
    /// lets a sequence call *out* to app code, this lets app code fire a
    /// sequence *in*. App code defines its own message type, implements
    /// `XrdsTriggerEvent` returning `Custom("its-name")`, and registers
    /// `consume_triggers::<ItsEvent>` — no SDK change needed.
    ///
    /// **This is also how continuous state becomes a trigger.** Values
    /// like rotation angle or position have no natural "moment" — they
    /// change every frame — so they are deliberately not modeled as
    /// trigger kinds. Gameplay code watches the value, decides when it
    /// matters (the threshold is domain knowledge the SDK cannot have),
    /// and fires a `Custom` trigger at that point.
    ///
    /// Trade-off accepted knowingly: this is string-matched, so a typo
    /// silently never fires. It stays a plain *name* rather than a query
    /// path or expression, so it cannot grow into a scripting surface.
    Custom(String),

    /// Fired instead of letting a causal chain of triggers/actions run
    /// away — an `XrdsAction::Run` chain, or `FireCustomEvent` re-triggering
    /// its own listener, deep enough to exceed the runtime's depth cap.
    ///
    /// **This is the required escape hatch, not a general loop
    /// preventer.** Authoring an intentional loop is not blocked — other
    /// engines permit that too. When one *does* run away, this trigger
    /// lets a recovery sequence be *authored* (log it, reset state,
    /// whatever fits) rather than only discoverable as a silent hang.
    /// `XrdsAPI::stop_sequences_on`/`stop_all_sequences` are the manual
    /// half of the same escape hatch.
    RunawayDetected,

    /// A trigger kind this build does not recognize — same rationale and
    /// same lossiness caveat as [`XrdsAction::Unknown`].
    ///
    /// Nothing ever emits this, so a binding that deserializes to
    /// `Unknown` is simply inert rather than misfiring.
    #[serde(other)]
    Unknown,
}

/// "When trigger kind K fires for this node, run sequence S." A node can
/// have several bindings (e.g. one for `ZoneEnter`, a different one for
/// `ZoneExit`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct XrdsTriggerBinding {
    #[serde(default)]
    pub trigger: XrdsTriggerKind,
    /// Names an entry in `XrdsSceneDocument::tracks`.
    ///
    /// `None` is authored-but-unwired: the binding exists and its trigger
    /// is chosen, but nothing runs yet. `track_diagnostics` reports it as a
    /// warning rather than an error, since it is the normal intermediate
    /// state while authoring.
    ///
    /// There is deliberately no inline alternative. The previous schema
    /// carried both an inline `sequence` and this name, which meant two
    /// ways to author the same thing and a diagnostic for the case where an
    /// author set both. One way is simpler to author, explain and validate,
    /// and referencing by name is the form that lets several bindings share
    /// one piece of choreography.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub track: Option<String>,
    /// When true this rule is parked: it stays authored but never fires.
    ///
    /// For switching one rule off without deleting it — isolating which of
    /// several bindings misbehaves, or keeping a rule around before it is
    /// ready. The editor equivalent of the checkbox beside a component.
    ///
    /// **Named negatively on purpose.** `XrdsSceneNode::enabled` already
    /// exists and means something different (whether the node is
    /// instantiated at all), so a second `enabled` with different semantics
    /// nested inside it would be actively confusing. The negative form also
    /// makes plain `#[serde(default)]` correct: serde's bool default is
    /// `false`, so an `enabled` field would silently disable every binding
    /// in every existing document on load.
    ///
    /// Follows the same serde shape as `XrdsSceneNode::grabbable`.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub disabled: bool,
    /// Restricts this binding to a specific controller. `None` (the
    /// default) means any hand, matching the previous behavior where hand
    /// information was silently discarded.
    ///
    /// Several trigger sources carry which hand caused them —
    /// `XrGrabEvent`, `XrWorldButtonPressEvent`, and others — but that data
    /// had nowhere to go: an author had no way to say "only the left
    /// controller" for a grab-triggered binding, even though the event
    /// already reports it. Only meaningful for hand-carrying trigger kinds
    /// (`Grabbed`, `Dropped`, `HoverEnter`/`HoverExit`,
    /// `ButtonPress`/`ButtonRelease`, `SliderChange`, `ToggleChange`) — a
    /// filter set on `ZoneEnter`/`AnimationComplete`/`Custom` can never
    /// match, since those sources have no hand to report.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hand: Option<xrds_components::XrGrabHand>,
}

// ---------------------------------------------------------------------------
// Authoring diagnostics
// ---------------------------------------------------------------------------

/// One authoring problem found in a document's trigger-action data.
///
/// Deliberately shaped like `XrdsSceneAssetDiagnosticEntry` (subject /
/// severity / title / detail) so an editor can render asset and trigger
/// diagnostics in one uniform list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XrdsSceneTriggerDiagnostic {
    /// The node whose bindings the problem was found on. `None` for a
    /// problem in the document-level runnable registry itself (Phase 9a) —
    /// a `Run` cycle or dangling reference there isn't any one node's
    /// fault, since a registry entry may be referenced by many nodes or
    /// none yet.
    pub node_id: Option<XrdsSceneNodeId>,
    pub severity: XrdsSceneTriggerDiagnosticSeverity,
    pub title: String,
    pub detail: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum XrdsSceneTriggerDiagnosticSeverity {
    /// Worth knowing, not necessarily wrong.
    Info,
    /// Will probably not behave as intended.
    Warning,
    /// Cannot work — a reference that resolves to nothing.
    Error,
}

impl XrdsSceneDocument {
    /// Validates authored Track data, catching the failure modes that are
    /// otherwise **silent at runtime**.
    ///
    /// Silence is the whole reason this exists. A binding naming a Track
    /// that no longer exists, an event aimed at a deleted node, or two
    /// Tracks fighting over one asset all produce *nothing happening* —
    /// indistinguishable from "not triggered yet". A runtime `warn!` only
    /// helps someone already watching the log.
    ///
    /// Diagnostics with `node_id: None` come from the Track registry
    /// itself: a registry entry may be referenced by many bindings or none,
    /// so it is not any one node's fault.
    pub fn track_diagnostics(&self) -> Vec<XrdsSceneTriggerDiagnostic> {
        use XrdsSceneTriggerDiagnosticSeverity as Severity;

        let mut out = Vec::new();
        let node_ids: std::collections::HashSet<XrdsSceneNodeId> =
            self.nodes.iter().map(|n| n.id).collect();
        let gltf_nodes: std::collections::HashSet<XrdsSceneNodeId> = self
            .nodes
            .iter()
            .filter(|n| matches!(n.payload, XrdsSceneNodePayload::GltfAsset(_)))
            .map(|n| n.id)
            .collect();

        // -- Track names ------------------------------------------------------
        // Names are keys, so a bad one silently breaks wiring rather than
        // looking wrong. The editor refuses these at input
        // (`normalize_authored_name`), but a document can also be built in Rust
        // or hand-edited, so the same rules are reported here rather than
        // trusted to have been enforced upstream.
        for entry in &self.tracks {
            match crate::normalize_authored_name(&entry.name) {
                Ok(canonical) if canonical != entry.name => {
                    out.push(XrdsSceneTriggerDiagnostic {
                        node_id: None,
                        severity: Severity::Error,
                        title: "Track name has surrounding whitespace".to_string(),
                        detail: format!(
                            "Track {:?} is not the same key as {canonical:?}, but renders \
                             identically. A binding naming one will silently miss the other.",
                            entry.name
                        ),
                    });
                }
                Err(e) => out.push(XrdsSceneTriggerDiagnostic {
                    node_id: None,
                    severity: Severity::Error,
                    title: "Track name is not usable".to_string(),
                    detail: format!("Track {:?}: {}", entry.name, e.message()),
                }),
                Ok(_) => {}
            }
        }

        for (first, second) in
            crate::names_differing_only_by_case(self.tracks.iter().map(|t| t.name.as_str()))
        {
            out.push(XrdsSceneTriggerDiagnostic {
                node_id: None,
                severity: Severity::Warning,
                title: "Two Tracks differ only by case".to_string(),
                detail: format!(
                    "{first:?} and {second:?} are separate Tracks. That is legal, but a binding \
                     naming one when the other was meant is invisible in review."
                ),
            });
        }

        // -- Per-node bindings ------------------------------------------------
        for node in &self.nodes {
            for (i, binding) in node.triggers.iter().enumerate() {
                let where_ = format!("node {:?} binding #{i}", node.id);

                match &binding.track {
                    None => out.push(XrdsSceneTriggerDiagnostic {
                        node_id: Some(node.id),
                        severity: Severity::Warning,
                        title: "Binding runs nothing".to_string(),
                        detail: format!(
                            "{where_} has no Track selected, so the trigger fires and nothing \
                             happens. Normal while authoring; pick a Track to wire it up."
                        ),
                    }),
                    Some(name) if self.track(name).is_none() => {
                        out.push(XrdsSceneTriggerDiagnostic {
                            node_id: Some(node.id),
                            severity: Severity::Error,
                            title: "Binding names a missing Track".to_string(),
                            detail: format!(
                                "{where_} names Track {name:?}, which is not in the document. It \
                                 was probably renamed or deleted; nothing will run."
                            ),
                        })
                    }
                    Some(_) => {}
                }

                // A hand filter on a kind that never reports a hand can never
                // match. Error, not Warning: this is not "might misbehave", it
                // genuinely cannot fire.
                if binding.hand.is_some() && !binding.trigger.carries_hand() {
                    out.push(XrdsSceneTriggerDiagnostic {
                        node_id: Some(node.id),
                        severity: Severity::Error,
                        title: "Hand filter on a trigger kind with no hand".to_string(),
                        detail: format!(
                            "{where_} restricts to a specific hand, but {:?} never reports one \
                             — this binding can never fire.",
                            binding.trigger
                        ),
                    });
                }
            }
        }

        // -- Track registry ---------------------------------------------------
        for entry in &self.tracks {
            let where_ = format!("Track {:?}", entry.name);
            let track = &entry.track;

            if track.assets.is_empty() {
                out.push(XrdsSceneTriggerDiagnostic {
                    node_id: None,
                    severity: Severity::Warning,
                    title: "Empty Track".to_string(),
                    detail: format!("{where_} has no assets, so firing it does nothing."),
                });
            }

            // An asset appears at most once per Track: all of its events
            // belong on its single row. Two rows for one asset means two
            // schedules driving one node from inside the same Track, which is
            // exactly the fight the one-row rule exists to prevent.
            let mut seen: Vec<&XrdsActionTarget> = Vec::new();
            for asset in &track.assets {
                if seen.contains(&&asset.target) {
                    out.push(XrdsSceneTriggerDiagnostic {
                        node_id: None,
                        severity: Severity::Error,
                        title: "Asset appears twice in one Track".to_string(),
                        detail: format!(
                            "{where_} has more than one row for {:?}. Merge them: an asset gets \
                             one row per Track, holding all of its events.",
                            asset.target
                        ),
                    });
                } else {
                    seen.push(&asset.target);
                }

                if asset.keys.is_empty() {
                    out.push(XrdsSceneTriggerDiagnostic {
                        node_id: None,
                        severity: Severity::Warning,
                        title: "Asset row has no events".to_string(),
                        detail: format!(
                            "{where_}'s row for {:?} has no events, so it does nothing. It still \
                             reserves the asset against other Tracks.",
                            asset.target
                        ),
                    });
                }

                if let XrdsActionTarget::Node(id) = asset.target {
                    if !node_ids.contains(&id) {
                        out.push(XrdsSceneTriggerDiagnostic {
                            node_id: None,
                            severity: Severity::Error,
                            title: "Asset row targets a missing node".to_string(),
                            detail: format!(
                                "{where_} has a row for node {id:?}, which is not in the \
                                 document. It was probably deleted; the row will do nothing."
                            ),
                        });
                    }
                }

                for (k, key) in asset.keys.iter().enumerate() {
                    let at = format!(
                        "{where_}, {:?} @ {:.2}s (event #{k})",
                        asset.target, key.at_secs
                    );

                    if key.at_secs < 0.0 {
                        out.push(XrdsSceneTriggerDiagnostic {
                            node_id: None,
                            severity: Severity::Error,
                            title: "Event at a negative time".to_string(),
                            detail: format!(
                                "{at} sits before the Track starts, so it fires immediately."
                            ),
                        });
                    }

                    if !key.action.is_valid_in_track() {
                        out.push(XrdsSceneTriggerDiagnostic {
                            node_id: None,
                            severity: Severity::Error,
                            title: "Unrecognized action".to_string(),
                            detail: format!(
                                "{at} is an action this build does not understand — written by a \
                                 newer editor than this one. It is skipped at runtime."
                            ),
                        });
                    }

                    // No "action escapes its own row" check any more: that
                    // was only reachable through SetMaterial/ModifyHealth's
                    // own `target` field, which is gone (see their doc
                    // comments) — every action now applies to whichever
                    // asset row it sits on, with nothing left to escape to.

                    // glTF playback needs a node that has a glTF payload; on
                    // anything else it is a silent no-op.
                    let needs_gltf = matches!(
                        key.action,
                        XrdsAction::PlayGltfAnimation { .. } | XrdsAction::StopGltfAnimation
                    );
                    if needs_gltf {
                        if let XrdsActionTarget::Node(id) = asset.target {
                            if node_ids.contains(&id) && !gltf_nodes.contains(&id) {
                                out.push(XrdsSceneTriggerDiagnostic {
                                    node_id: None,
                                    severity: Severity::Error,
                                    title: "glTF action on a non-glTF node".to_string(),
                                    detail: format!(
                                        "{at} controls glTF playback, but node {id:?} has no \
                                         glTF payload — nothing will play."
                                    ),
                                });
                            }
                        }
                    }

                    // `duration_secs` is deliberately not bound: the only check
                    // that read it was the zero-duration warning, deleted when
                    // `Teleport` was removed (duration 0 is now the normal way
                    // to author an instant change).
                    if let XrdsAction::SetTransform { position, rotation, scale, .. } = &key.action
                    {
                        if position.is_none() && rotation.is_none() && scale.is_none() {
                            out.push(XrdsSceneTriggerDiagnostic {
                                node_id: None,
                                severity: Severity::Warning,
                                title: "Interpolation changes nothing".to_string(),
                                detail: format!(
                                    "{at} leaves position, rotation and scale all unset, so it \
                                     animates to where the asset already is."
                                ),
                            });
                        }
                        // Deliberately NO zero-duration warning. With
                        // `Teleport` deleted, `duration_secs == 0` is the
                        // normal way to author an instant change — warning on
                        // it would flag correct authoring, which is how you
                        // train people to ignore diagnostics.
                    }

                    if let XrdsAction::SetMaterial {
                        base_color,
                        metallic,
                        roughness,
                        texture,
                    } = &key.action
                    {
                        if base_color.is_none()
                            && metallic.is_none()
                            && roughness.is_none()
                            && texture.is_none()
                        {
                            out.push(XrdsSceneTriggerDiagnostic {
                                node_id: None,
                                severity: Severity::Warning,
                                title: "Material change sets nothing".to_string(),
                                detail: format!("{at} leaves every material property unset."),
                            });
                        }

                        // A texture id that names nothing in the catalog
                        // resolves to no image at runtime, so the slot is left
                        // as-is and the event looks like it silently did
                        // nothing. Worth an error rather than a warning: unlike
                        // an unset field, this is never intentional.
                        if let Some(t) = texture {
                            if let Some(id) = &t.texture_asset_id {
                                let known = self.assets.iter().any(|a| {
                                    a.id == *id && a.kind == XrdsSceneAssetKind::Texture
                                });
                                if !known {
                                    out.push(XrdsSceneTriggerDiagnostic {
                                        node_id: None,
                                        severity: Severity::Error,
                                        title: "Texture asset is not in the catalog".to_string(),
                                        detail: format!(
                                            "{at} assigns {id:?} to the {:?} slot, but no texture \
                                             asset with that id exists in this document. The slot \
                                             will keep whatever it already had.",
                                            t.slot
                                        ),
                                    });
                                }
                            }
                        }
                    }
                }
            }

            // Events past an authored duration never fire.
            if let Some(duration) = track.duration_secs {
                for asset in &track.assets {
                    for key in &asset.keys {
                        if key.at_secs > duration {
                            out.push(XrdsSceneTriggerDiagnostic {
                                node_id: None,
                                severity: Severity::Warning,
                                title: "Event past the Track's end".to_string(),
                                detail: format!(
                                    "{where_} runs for {duration}s, but {:?} has an event at \
                                     {:.2}s — it will never fire.",
                                    asset.target, key.at_secs
                                ),
                            });
                        }
                    }
                }
            }
        }

        out.extend(self.watcher_diagnostics(&node_ids));
        out.extend(self.track_conflict_diagnostics());
        out
    }

    /// Threshold-watcher problems, plus the `Custom`-trigger-with-no-emitter
    /// check.
    ///
    /// That last one is the most valuable diagnostic here and the reason the
    /// whole family exists: a `Custom` binding whose name nothing fires is
    /// indistinguishable at runtime from one that simply has not been
    /// triggered yet, so without this there is nothing to debug against.
    ///
    /// Emitters are enabled watchers' `fires` names. It is a **warning**, not
    /// an error, because expert-layer Rust can fire a custom trigger too —
    /// the document cannot see that, so "nothing in this document emits it"
    /// is a suspicion, not a proof.
    fn watcher_diagnostics(
        &self,
        node_ids: &std::collections::HashSet<XrdsSceneNodeId>,
    ) -> Vec<XrdsSceneTriggerDiagnostic> {
        use XrdsSceneTriggerDiagnosticSeverity as Severity;
        let mut out = Vec::new();

        // Disabled watchers are parked, so they neither emit nor warrant
        // complaints of their own.
        let emitted: std::collections::HashSet<&str> = self
            .nodes
            .iter()
            .flat_map(|n| n.watchers.iter())
            .filter(|w| !w.disabled)
            .map(|w| w.fires.as_str())
            .collect();

        for node in &self.nodes {
            for (i, binding) in node.triggers.iter().enumerate() {
                if binding.disabled {
                    continue;
                }
                if let XrdsTriggerKind::Custom(name) = &binding.trigger {
                    if !emitted.contains(name.as_str()) {
                        out.push(XrdsSceneTriggerDiagnostic {
                            node_id: Some(node.id),
                            severity: Severity::Warning,
                            title: "Nothing emits this Custom trigger".to_string(),
                            detail: format!(
                                "node {:?} binding #{i} listens for Custom({name:?}), which no \
                                 threshold watcher in this document fires. Fine if application \
                                 code fires it; otherwise this binding never runs.",
                                node.id
                            ),
                        });
                    }
                }
            }

            for (i, watcher) in node.watchers.iter().enumerate() {
                if watcher.disabled {
                    continue;
                }
                let where_ = format!("node {:?} watcher #{i}", node.id);

                if let XrdsObservable::DistanceTo { node: other } = watcher.observable {
                    if !node_ids.contains(&other) {
                        out.push(XrdsSceneTriggerDiagnostic {
                            node_id: Some(node.id),
                            severity: Severity::Error,
                            title: "Watcher measures distance to a missing node".to_string(),
                            detail: format!(
                                "{where_} measures distance to node {other:?}, which is not in \
                                 the document — the distance can never be computed."
                            ),
                        });
                    }
                }

                // Hysteresis is a dead-band width; a negative one has no
                // meaning and would widen rather than damp the band.
                if watcher.hysteresis < 0.0 {
                    out.push(XrdsSceneTriggerDiagnostic {
                        node_id: Some(node.id),
                        severity: Severity::Error,
                        title: "Watcher has negative hysteresis".to_string(),
                        detail: format!(
                            "{where_} has hysteresis {}, which is not a meaningful dead-band \
                             width. Use 0 for none.",
                            watcher.hysteresis
                        ),
                    });
                }

                if watcher.fires.trim().is_empty() {
                    out.push(XrdsSceneTriggerDiagnostic {
                        node_id: Some(node.id),
                        severity: Severity::Error,
                        title: "Watcher fires an empty name".to_string(),
                        detail: format!(
                            "{where_} has no name to fire, so nothing can listen for it."
                        ),
                    });
                }
            }
        }

        out
    }

    /// Cross-Track asset conflicts.
    ///
    /// Two Tracks driving the same asset cannot run at the same time — the
    /// runtime refuses to start a Track whose assets are already held
    /// (reject-the-newcomer). Authoring the overlap is allowed, since the
    /// two may never actually be fired together; it is reported so the
    /// author learns the constraint before hitting it.
    ///
    /// A *looping* Track is different in kind: it never releases its assets,
    /// so anything sharing one can never run at all. Permanent rather than
    /// situational, so it is an error.
    fn track_conflict_diagnostics(&self) -> Vec<XrdsSceneTriggerDiagnostic> {
        use XrdsSceneTriggerDiagnosticSeverity as Severity;
        let mut out = Vec::new();

        for (i, a) in self.tracks.iter().enumerate() {
            for b in self.tracks.iter().skip(i + 1) {
                let a_nodes = a.track.owned_nodes();
                let shared: Vec<XrdsSceneNodeId> = b
                    .track
                    .owned_nodes()
                    .into_iter()
                    .filter(|id| a_nodes.contains(id))
                    .collect();
                if shared.is_empty() {
                    continue;
                }

                let list = shared
                    .iter()
                    .map(|id| format!("{id:?}"))
                    .collect::<Vec<_>>()
                    .join(", ");

                // Name the looping side: it is the one making this permanent.
                let looping = match (a.track.looping, b.track.looping) {
                    (true, _) => Some((&a.name, &b.name)),
                    (_, true) => Some((&b.name, &a.name)),
                    _ => None,
                };

                match looping {
                    Some((loops, blocked)) => out.push(XrdsSceneTriggerDiagnostic {
                        node_id: None,
                        severity: Severity::Error,
                        title: "A looping Track blocks another forever".to_string(),
                        detail: format!(
                            "Track {loops:?} loops and shares {list} with Track {blocked:?}. A \
                             looping Track never releases its assets, so {blocked:?} can never \
                             run. Stop looping {loops:?}, or give them separate assets."
                        ),
                    }),
                    None => out.push(XrdsSceneTriggerDiagnostic {
                        node_id: None,
                        severity: Severity::Warning,
                        title: "Two Tracks share an asset".to_string(),
                        detail: format!(
                            "Tracks {:?} and {:?} both drive {list}, so they cannot run at the \
                             same time — whichever is fired second is refused while the first is \
                             still running.",
                            a.name, b.name
                        ),
                    }),
                }
            }
        }

        out
    }
}

// ---------------------------------------------------------------------------
// Threshold watchers — continuous values to discrete triggers (Phase 8)
// ---------------------------------------------------------------------------

/// Which local axis a rotation is measured around, for
/// [`XrdsObservable::RotationDegrees`].
///
/// Extracted via Euler XYZ decomposition of the node's world rotation —
/// simple and well-defined for the common "this hinge/valve/dial has
/// turned past N degrees" case, with the usual Euler-angle caveat: near a
/// gimbal-lock configuration, a single axis's reading can behave
/// unintuitively. Not a concern for the typical single-axis hinge/dial
/// case this is aimed at.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum XrdsAxis {
    X,
    #[default]
    Y,
    Z,
}

/// A closed set of continuous quantities that can be watched. Deliberately
/// not an arbitrary property path — see the module-level rationale for why
/// continuous state stays out of the trigger-kind vocabulary itself.
///
/// All measured in world space (via the node's `GlobalTransform`), since a
/// watcher answering "has this rotated past 90°" almost always means in the
/// world, not relative to a possibly-rotating parent.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum XrdsObservable {
    RotationDegrees { axis: XrdsAxis },
    DistanceTo { node: XrdsSceneNodeId },
    /// World-space `translation.y`.
    Height,
    /// `scale.length()` — one number for non-uniform scale, so "grown
    /// past 2x" has an unambiguous meaning without picking an axis.
    ScaleMagnitude,
}

/// Which direction(s) across the threshold should fire.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum XrdsCrossing {
    Above,
    Below,
    #[default]
    Either,
}

/// Watches one continuous value on this node and fires a `Custom` trigger
/// each time it crosses `value`, in the direction(s) `crossing` allows.
///
/// Re-arms automatically: every crossing fires, including a value that
/// crosses back and forth repeatedly. A one-shot `once` flag is deliberately
/// not included in this v1 — add it if a real use case needs it, per the
/// project's general "don't build for hypothetical needs" stance.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct XrdsThresholdWatcher {
    pub observable: XrdsObservable,
    #[serde(default)]
    pub crossing: XrdsCrossing,
    pub value: f32,
    /// Deadband around `value`: the watcher must move at least this far
    /// past the threshold, and then back at least this far, before it will
    /// re-fire in the other direction. Without this, a value hovering
    /// exactly at the threshold fires every frame it wobbles across it.
    #[serde(default)]
    pub hysteresis: f32,
    /// Fired as `XrdsTriggerKind::Custom(fires)` on each qualifying
    /// crossing — bind to it the same way as any other `Custom` trigger.
    pub fires: String,
    /// Parks this watcher without deleting it, same semantics and same
    /// reasoning as `XrdsTriggerBinding::disabled`.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub disabled: bool,
}
