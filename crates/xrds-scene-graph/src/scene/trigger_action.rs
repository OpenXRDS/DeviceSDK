//! Authored trigger-action sequencing data. See
//! `docs/xrds-scenegraph-trigger-action-sequencing.md` for the design
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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
    /// **Known limitation:** this is lossy. `#[serde(other)]` requires a
    /// unit variant, so the original payload cannot be retained — an older
    /// build that loads and re-exports a document will drop actions it did
    /// not understand. Total parse failure is the worse harm, so this
    /// trade is deliberate; making it lossless needs a hand-written
    /// `Deserialize`, tracked as a follow-up.
    #[serde(other)]
    Unknown,
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
    #[serde(default)]
    pub sequence: XrdsSequence,
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
    /// The node whose bindings the problem was found on.
    pub node_id: XrdsSceneNodeId,
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
    /// Validates authored trigger-action data, catching the failure modes
    /// that are otherwise **silent at runtime**.
    ///
    /// The worst of these is a `Custom` trigger whose name nothing emits:
    /// "never fires" is indistinguishable from "not triggered yet", so
    /// without a diagnostic there is nothing to debug against. Dangling node
    /// targets and actions needing a payload the node lacks are the same
    /// story — a runtime `warn!` only helps if someone happens to be
    /// watching the log at that moment.
    ///
    /// Note: `XrdsAction::Run` / named-runnable reference checks belong here
    /// too once Phase 9a lands; this is their intended home.
    pub fn trigger_diagnostics(&self) -> Vec<XrdsSceneTriggerDiagnostic> {
        use XrdsSceneTriggerDiagnosticSeverity as Severity;

        let mut out = Vec::new();
        let known_ids: HashSet<XrdsSceneNodeId> = self.nodes.iter().map(|n| n.id).collect();

        // Custom names bound somewhere in this document, and custom names it
        // emits. Either side can legitimately live in application code, so a
        // mismatch is Info rather than Error.
        let mut bound_custom: HashSet<&str> = HashSet::new();
        let mut emitted_custom: HashSet<&str> = HashSet::new();
        for node in &self.nodes {
            for binding in &node.triggers {
                // Disabled bindings neither listen nor emit. Counting them
                // would let a parked emitter suppress the "nothing fires
                // this" warning on a live listener.
                if binding.disabled {
                    continue;
                }
                if let XrdsTriggerKind::Custom(name) = &binding.trigger {
                    bound_custom.insert(name.as_str());
                }
                for step in &binding.sequence.steps {
                    if let XrdsAction::FireCustomEvent { name } = step {
                        emitted_custom.insert(name.as_str());
                    }
                }
            }
        }

        for node in &self.nodes {
            let node_is_gltf = matches!(node.payload, XrdsSceneNodePayload::GltfAsset(_));

            for (index, binding) in node.triggers.iter().enumerate() {
                // A parked binding is deliberately inert, so nagging about
                // its contents is noise the author has to dismiss on every
                // pass. Anything genuinely wrong resurfaces the moment it is
                // re-enabled, since diagnostics run continuously.
                if binding.disabled {
                    continue;
                }

                let where_ = format!("binding #{index} on node {:?}", node.id);

                match &binding.trigger {
                    XrdsTriggerKind::Unknown => out.push(XrdsSceneTriggerDiagnostic {
                        node_id: node.id,
                        severity: Severity::Warning,
                        title: "Unrecognized trigger kind".to_string(),
                        detail: format!(
                            "{where_} uses a trigger kind this build does not know, so it can \
                             never fire. The scene was likely authored by a newer editor."
                        ),
                    }),
                    XrdsTriggerKind::Custom(name) if !emitted_custom.contains(name.as_str()) => {
                        out.push(XrdsSceneTriggerDiagnostic {
                            node_id: node.id,
                            severity: Severity::Info,
                            title: "Custom trigger has no emitter in this document".to_string(),
                            detail: format!(
                                "{where_} listens for Custom({name}), which nothing in this \
                                 document fires. Fine if application code fires it; otherwise \
                                 it is a typo and the binding will silently never run."
                            ),
                        });
                    }
                    _ => {}
                }

                // A hand filter on a trigger kind that never reports a hand
                // (ZoneEnter/Exit, AnimationComplete, Custom) can never
                // match — the binding is permanently unfireable, and
                // silently so. Error, not Warning: this is not "might not
                // behave as intended," it genuinely cannot work.
                if binding.hand.is_some()
                    && !matches!(
                        binding.trigger,
                        XrdsTriggerKind::Grabbed
                            | XrdsTriggerKind::Dropped
                            | XrdsTriggerKind::HoverEnter
                            | XrdsTriggerKind::HoverExit
                            | XrdsTriggerKind::ButtonPress
                            | XrdsTriggerKind::ButtonRelease
                            | XrdsTriggerKind::SliderChange
                            | XrdsTriggerKind::ToggleChange
                    )
                {
                    out.push(XrdsSceneTriggerDiagnostic {
                        node_id: node.id,
                        severity: Severity::Error,
                        title: "Hand filter on a trigger kind with no hand".to_string(),
                        detail: format!(
                            "{where_} restricts to a specific hand, but {:?} never reports one \
                             — this binding can never fire.",
                            binding.trigger
                        ),
                    });
                }

                if binding.sequence.steps.is_empty() {
                    out.push(XrdsSceneTriggerDiagnostic {
                        node_id: node.id,
                        severity: Severity::Info,
                        title: "Empty sequence".to_string(),
                        detail: format!("{where_} has no steps, so firing it does nothing."),
                    });
                }

                for (step_index, step) in binding.sequence.steps.iter().enumerate() {
                    let at = format!("{where_}, step #{step_index}");

                    match step {
                        XrdsAction::Unknown => out.push(XrdsSceneTriggerDiagnostic {
                            node_id: node.id,
                            severity: Severity::Warning,
                            title: "Unrecognized action".to_string(),
                            detail: format!(
                                "{at} is an action this build does not know and will be skipped \
                                 at runtime. The scene was likely authored by a newer editor."
                            ),
                        }),

                        XrdsAction::PlayGltfAnimation { .. } | XrdsAction::StopGltfAnimation
                            if !node_is_gltf =>
                        {
                            out.push(XrdsSceneTriggerDiagnostic {
                                node_id: node.id,
                                severity: Severity::Warning,
                                title: "glTF animation action on a non-glTF node".to_string(),
                                detail: format!(
                                    "{at} drives glTF animation, but this node payload is not a \
                                     glTF asset, so it will do nothing."
                                ),
                            });
                        }

                        XrdsAction::FireCustomEvent { name }
                            if !bound_custom.contains(name.as_str()) =>
                        {
                            out.push(XrdsSceneTriggerDiagnostic {
                                node_id: node.id,
                                severity: Severity::Info,
                                title: "Custom event has no listener in this document".to_string(),
                                detail: format!(
                                    "{at} fires Custom({name}), which no binding in this document \
                                     listens for. Fine if application code handles it."
                                ),
                            });
                        }

                        _ => {}
                    }

                    // Dangling explicit node target — the one genuinely
                    // unworkable case, hence Error.
                    if let XrdsAction::ModifyHealth { target, .. } = step {
                        if let XrdsActionTarget::Node(id) = target {
                            if !known_ids.contains(id) {
                                out.push(XrdsSceneTriggerDiagnostic {
                                    node_id: node.id,
                                    severity: Severity::Error,
                                    title: "Action targets a node that does not exist".to_string(),
                                    detail: format!(
                                        "{at} targets node {id:?}, which is not in this document."
                                    ),
                                });
                            }
                        }
                    }
                }
            }
        }

        out
    }
}
