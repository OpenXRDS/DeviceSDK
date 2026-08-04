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

    /// Starts a named entry from `XrdsSceneDocument::runnables` — how a
    /// sequence starts a timeline, a timeline starts a sequence, or either
    /// starts another instance of itself (see the cycle-detection note
    /// below).
    ///
    /// **Takes a bare name, not an inline runnable.** This is the
    /// recursion firewall: a name cannot contain another `XrdsAction`, so
    /// `Run` cannot make this data structure nest arbitrarily the way an
    /// inline reference could.
    ///
    /// `wait` is honored when this runs inside an `XrdsSequence` (blocks
    /// the queue until the started runnable finishes — natural, since a
    /// sequence is already completion-chained) and ignored, with a
    /// warning, when it fires from an `XrdsTimeline` key (a timeline that
    /// paused would break the absolute timing that is its entire purpose).
    ///
    /// **Cycles are not prevented, only escaped.** `A runs B runs A` is
    /// statically detectable in the registry and is flagged as an `Error`
    /// by `XrdsSceneDocument::trigger_diagnostics`, but authoring a loop is
    /// not blocked outright — other engines permit intentional event
    /// loops too. What is guaranteed is an escape: a causal chain depth is
    /// tracked at runtime and capped, and exceeding it fires
    /// `XrdsTriggerKind::RunawayDetected` rather than hanging silently.
    Run {
        runnable: String,
        #[serde(default = "default_run_wait")]
        wait: bool,
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

fn default_run_wait() -> bool {
    true
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
    /// Inline sequence, used when `runnable` is `None`. Ignored (with a
    /// diagnostic) when `runnable` is `Some` — see the field below.
    #[serde(default)]
    pub sequence: XrdsSequence,
    /// Names an entry in `XrdsSceneDocument::runnables` to run instead of
    /// `sequence` — the template case (Phase 9a): the same named
    /// `XrdsRunnable` (a `Sequence` **or** a `Timeline`) can be referenced
    /// by many bindings, and editing the registry entry affects all of
    /// them at once.
    ///
    /// Deliberately additive rather than replacing `sequence` with a
    /// `Named`/`Inline` enum: this keeps every existing inline-sequence
    /// binding (and every existing test literal) working unchanged.
    /// `Some` and a non-empty `sequence` set together is diagnosable
    /// author confusion (`sequence` is simply ignored), not a runtime
    /// error — see `trigger_diagnostics`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runnable: Option<String>,
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
    /// Also covers Phase 9a's registry: a `runnable`/`Run` reference naming
    /// an entry `XrdsSceneDocument::runnables` doesn't have, and a static
    /// cycle in the registry's own `Run`-graph (`A runs B runs A`) — the
    /// latter is `node_id: None` since a registry entry isn't any one
    /// node's fault.
    pub fn trigger_diagnostics(&self) -> Vec<XrdsSceneTriggerDiagnostic> {
        use XrdsSceneTriggerDiagnosticSeverity as Severity;

        let mut out = Vec::new();
        let known_ids: HashSet<XrdsSceneNodeId> = self.nodes.iter().map(|n| n.id).collect();
        let known_runnables: HashSet<&str> =
            self.runnables.iter().map(|r| r.name.as_str()).collect();

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
            // A threshold watcher is also a Custom emitter — a binding
            // listening for what a watcher fires must not be flagged as
            // "nothing fires this" just because no FireCustomEvent action
            // happens to emit the same name.
            for watcher in &node.watchers {
                if !watcher.disabled {
                    emitted_custom.insert(watcher.fires.as_str());
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
                        node_id: Some(node.id),
                        severity: Severity::Warning,
                        title: "No trigger kind selected".to_string(),
                        detail: format!(
                            "{where_} has no recognized trigger kind, so it can never fire. \
                             Either the author hasn't picked one yet (this is the editor's \
                             placeholder for a freshly added binding), or the scene was \
                             authored by a newer editor build using a kind this one doesn't know."
                        ),
                    }),
                    XrdsTriggerKind::Custom(name) if !emitted_custom.contains(name.as_str()) => {
                        out.push(XrdsSceneTriggerDiagnostic {
                            node_id: Some(node.id),
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

                // `runnable: Some(name)` takes priority over the inline
                // `sequence` at runtime (see `XrdsTriggerBinding::runnable`),
                // so the two are mutually exclusive in what actually
                // executes. Check the named case here; only walk the inline
                // sequence's own steps below when there is no named runnable
                // to defer to — otherwise "Empty sequence" and per-step
                // diagnostics would fire on dead data that never runs.
                if let Some(name) = &binding.runnable {
                    if !known_runnables.contains(name.as_str()) {
                        out.push(XrdsSceneTriggerDiagnostic {
                            node_id: Some(node.id),
                            severity: Severity::Error,
                            title: "Binding references unknown runnable".to_string(),
                            detail: format!(
                                "{where_} names runnable {name:?}, which has no matching entry \
                                 in XrdsSceneDocument::runnables — this binding will fire \
                                 nothing at runtime."
                            ),
                        });
                    }
                    if !binding.sequence.steps.is_empty() {
                        out.push(XrdsSceneTriggerDiagnostic {
                            node_id: Some(node.id),
                            severity: Severity::Info,
                            title: "Binding has both a named runnable and an inline sequence"
                                .to_string(),
                            detail: format!(
                                "{where_} sets runnable: Some({name:?}) but also has inline \
                                 sequence steps — the named runnable takes priority and the \
                                 inline sequence is ignored at runtime."
                            ),
                        });
                    }
                } else {
                    if binding.sequence.steps.is_empty() {
                        out.push(XrdsSceneTriggerDiagnostic {
                            node_id: Some(node.id),
                            severity: Severity::Info,
                            title: "Empty sequence".to_string(),
                            detail: format!("{where_} has no steps, so firing it does nothing."),
                        });
                    }

                    for (step_index, step) in binding.sequence.steps.iter().enumerate() {
                        let at = format!("{where_}, step #{step_index}");

                        match step {
                            XrdsAction::Unknown => out.push(XrdsSceneTriggerDiagnostic {
                                node_id: Some(node.id),
                                severity: Severity::Warning,
                                title: "Unrecognized action".to_string(),
                                detail: format!(
                                    "{at} is an action this build does not know and will be \
                                     skipped at runtime. The scene was likely authored by a \
                                     newer editor."
                                ),
                            }),

                            XrdsAction::PlayGltfAnimation { .. } | XrdsAction::StopGltfAnimation
                                if !node_is_gltf =>
                            {
                                out.push(XrdsSceneTriggerDiagnostic {
                                    node_id: Some(node.id),
                                    severity: Severity::Warning,
                                    title: "glTF animation action on a non-glTF node".to_string(),
                                    detail: format!(
                                        "{at} drives glTF animation, but this node payload is \
                                         not a glTF asset, so it will do nothing."
                                    ),
                                });
                            }

                            XrdsAction::FireCustomEvent { name }
                                if !bound_custom.contains(name.as_str()) =>
                            {
                                out.push(XrdsSceneTriggerDiagnostic {
                                    node_id: Some(node.id),
                                    severity: Severity::Info,
                                    title: "Custom event has no listener in this document"
                                        .to_string(),
                                    detail: format!(
                                        "{at} fires Custom({name}), which no binding in this \
                                         document listens for. Fine if application code handles \
                                         it."
                                    ),
                                });
                            }

                            XrdsAction::Run { runnable, .. }
                                if !known_runnables.contains(runnable.as_str()) =>
                            {
                                out.push(XrdsSceneTriggerDiagnostic {
                                    node_id: Some(node.id),
                                    severity: Severity::Error,
                                    title: "Run references unknown runnable".to_string(),
                                    detail: format!(
                                        "{at} runs {runnable:?}, which has no matching entry in \
                                         XrdsSceneDocument::runnables."
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
                                        node_id: Some(node.id),
                                        severity: Severity::Error,
                                        title: "Action targets a node that does not exist"
                                            .to_string(),
                                        detail: format!(
                                            "{at} targets node {id:?}, which is not in this \
                                             document."
                                        ),
                                    });
                                }
                            }
                        }
                    }
                }
            }

            for (index, watcher) in node.watchers.iter().enumerate() {
                if watcher.disabled {
                    continue;
                }
                let where_ = format!("watcher #{index} on node {:?}", node.id);

                if let XrdsObservable::DistanceTo { node: target } = &watcher.observable {
                    if !known_ids.contains(target) {
                        out.push(XrdsSceneTriggerDiagnostic {
                            node_id: Some(node.id),
                            severity: Severity::Error,
                            title: "Watcher measures distance to a node that does not exist"
                                .to_string(),
                            detail: format!(
                                "{where_} measures DistanceTo({target:?}), which is not in \
                                 this document."
                            ),
                        });
                    }
                }

                if watcher.hysteresis < 0.0 {
                    out.push(XrdsSceneTriggerDiagnostic {
                        node_id: Some(node.id),
                        severity: Severity::Warning,
                        title: "Negative hysteresis".to_string(),
                        detail: format!(
                            "{where_} has a negative hysteresis ({}); treated as 0.0 at \
                             runtime, so this has no effect.",
                            watcher.hysteresis
                        ),
                    });
                }

                if !bound_custom.contains(watcher.fires.as_str()) {
                    out.push(XrdsSceneTriggerDiagnostic {
                        node_id: Some(node.id),
                        severity: Severity::Info,
                        title: "Watcher fires a Custom trigger with no listener".to_string(),
                        detail: format!(
                            "{where_} fires Custom({}), which no binding in this document \
                             listens for. Fine if application code handles it.",
                            watcher.fires
                        ),
                    });
                }
            }
        }

        // --- Registry-level: unknown Run targets and Run-graph cycles ---
        // These are node-less (a registry entry may be referenced by many
        // nodes, or none yet), unlike everything above.
        let registry_map: std::collections::HashMap<String, &XrdsNamedRunnable> =
            self.runnables.iter().map(|r| (r.name.clone(), r)).collect();

        for entry in &self.runnables {
            for target in run_targets(&entry.runnable) {
                if !registry_map.contains_key(target) {
                    out.push(XrdsSceneTriggerDiagnostic {
                        node_id: None,
                        severity: Severity::Error,
                        title: "Named runnable references unknown runnable".to_string(),
                        detail: format!(
                            "Runnable {:?} contains Run({target:?}), which has no matching \
                             entry in XrdsSceneDocument::runnables.",
                            entry.name
                        ),
                    });
                }
            }
        }

        // `A runs B runs A` is not prevented — other engines permit
        // intentional event loops too — but it is flagged, since the
        // runtime only escapes it via the chain-depth cap
        // (`XrdsTriggerKind::RunawayDetected`), not by refusing to run.
        let mut done: HashSet<&str> = HashSet::new();
        let mut reported: HashSet<&str> = HashSet::new();
        for entry in &self.runnables {
            if !done.contains(entry.name.as_str()) {
                let mut stack: Vec<&str> = Vec::new();
                visit_runnable_for_cycles(
                    &entry.name,
                    &registry_map,
                    &mut stack,
                    &mut done,
                    &mut reported,
                    &mut out,
                );
            }
        }

        out
    }
}

/// Every name a `Run` action inside `runnable` targets — used only by the
/// registry-level cycle/unknown-reference checks in `trigger_diagnostics`.
fn run_targets(runnable: &XrdsRunnable) -> Vec<&str> {
    match runnable {
        XrdsRunnable::Sequence(seq) => seq
            .steps
            .iter()
            .filter_map(|action| match action {
                XrdsAction::Run { runnable, .. } => Some(runnable.as_str()),
                _ => None,
            })
            .collect(),
        XrdsRunnable::Timeline(timeline) => timeline
            .keys
            .iter()
            .filter_map(|key| match &key.action {
                XrdsAction::Run { runnable, .. } => Some(runnable.as_str()),
                _ => None,
            })
            .collect(),
    }
}

/// DFS cycle detection over the registry's `Run`-graph. `stack` is the
/// current path; finding `name` already on it means everything from its
/// first occurrence onward forms a cycle. `done` short-circuits re-walking
/// an already-fully-explored (acyclic-from-here) name; `reported` stops the
/// same cycle being pushed once per member node.
fn visit_runnable_for_cycles<'a>(
    name: &'a str,
    registry_map: &std::collections::HashMap<String, &'a XrdsNamedRunnable>,
    stack: &mut Vec<&'a str>,
    done: &mut HashSet<&'a str>,
    reported: &mut HashSet<&'a str>,
    out: &mut Vec<XrdsSceneTriggerDiagnostic>,
) {
    if done.contains(name) {
        return;
    }
    if let Some(pos) = stack.iter().position(|n| *n == name) {
        let cycle_path = &stack[pos..];
        for member in cycle_path {
            if reported.insert(member) {
                out.push(XrdsSceneTriggerDiagnostic {
                    node_id: None,
                    severity: XrdsSceneTriggerDiagnosticSeverity::Error,
                    title: "Cycle in the Run registry".to_string(),
                    detail: format!(
                        "Named runnable {member:?} is part of a Run cycle ({}). Not rejected \
                         — intentional event loops are allowed — but the runtime only escapes \
                         it via the chain-depth cap (see XrdsTriggerKind::RunawayDetected), so \
                         double check this is deliberate.",
                        cycle_path
                            .iter()
                            .map(|n| format!("{n:?}"))
                            .collect::<Vec<_>>()
                            .join(" -> ")
                    ),
                });
            }
        }
        return;
    }

    stack.push(name);
    if let Some(entry) = registry_map.get(name) {
        for target in run_targets(&entry.runnable) {
            if registry_map.contains_key(target) {
                visit_runnable_for_cycles(target, registry_map, stack, done, reported, out);
            }
        }
    }
    stack.pop();
    done.insert(name);
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
