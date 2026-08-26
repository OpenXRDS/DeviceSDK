//! Tests for authored Track data and `track_diagnostics`.
//!
//! Rewritten wholesale for the Track model — see
//! `docs/done/xrds-track-model-plan.md`. The previous version of this file was
//! built around `XrdsSequence`, `Wait`, `Run` and `FireCustomEvent`, none of
//! which exist any more, so most of it was testing features rather than
//! needing porting.
//!
//! There are deliberately **no migration tests**: nothing was ever persisted
//! in the old schema, so there is no migration to test (plan doc §3).

use super::*;

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

fn node(id: u64) -> XrdsSceneNode {
    XrdsSceneNode {
        id: XrdsSceneNodeId(id),
        parent_id: None,
        name: format!("Node{id}"),
        enabled: true,
        visible: true,
        grabbable: false,
        transform: XrdsSceneTransform::default(),
        payload: XrdsSceneNodePayload::Empty,
        editor: XrdsEditorMetadata::default(),
        triggers: Vec::new(),
        watchers: Vec::new(),
    }
}

fn node_with_binding(id: u64, binding: XrdsTriggerBinding) -> XrdsSceneNode {
    XrdsSceneNode { triggers: vec![binding], ..node(id) }
}

fn node_with_watcher(id: u64, watcher: XrdsThresholdWatcher) -> XrdsSceneNode {
    XrdsSceneNode { watchers: vec![watcher], ..node(id) }
}

fn binding_for(track: &str) -> XrdsTriggerBinding {
    XrdsTriggerBinding {
        trigger: XrdsTriggerKind::ZoneEnter,
        track: Some(track.to_string()),
        effect: Default::default(),
        disabled: false,
        hand: None,
    }
}

/// One asset row driving `node_id`, with the given keys.
fn row(node_id: u64, keys: Vec<XrdsTrackKey>) -> XrdsTrackAsset {
    XrdsTrackAsset { when_finished: Default::default(), target: XrdsActionTarget::Node(XrdsSceneNodeId(node_id)), keys }
}

fn key(at_secs: f32, action: XrdsAction) -> XrdsTrackKey {
    XrdsTrackKey { at_secs, action }
}

/// A zero-duration `SetTransform`, i.e. what the deleted `Teleport` action was.
/// Kept under the old name so the many call sites below still read as "an
/// instant change".
fn teleport() -> XrdsAction {
    XrdsAction::SetTransform {
        position: Some([1.0, 0.0, 0.0]),
        rotation: None,
        scale: None,
        duration_secs: 0.0,
        ease: XrdsEaseCurve::Linear,
    }
}

fn animate(duration_secs: f32) -> XrdsAction {
    XrdsAction::SetTransform {
        position: Some([1.0, 0.0, 0.0]),
        rotation: None,
        scale: None,
        duration_secs,
        ease: XrdsEaseCurve::Cubic,
    }
}

fn named(name: &str, track: XrdsTrack) -> XrdsNamedTrack {
    XrdsNamedTrack { name: name.to_string(), track }
}

fn doc(nodes: Vec<XrdsSceneNode>, tracks: Vec<XrdsNamedTrack>) -> XrdsSceneDocument {
    XrdsSceneDocument { nodes, tracks, ..XrdsSceneDocument::default() }
}

/// Same as [`doc`] but with an asset catalog, for the texture-slot checks.
fn doc_with_assets(
    nodes: Vec<XrdsSceneNode>,
    tracks: Vec<XrdsNamedTrack>,
    assets: Vec<XrdsSceneAsset>,
) -> XrdsSceneDocument {
    XrdsSceneDocument { nodes, tracks, assets, ..XrdsSceneDocument::default() }
}

fn texture_asset(id: &str) -> XrdsSceneAsset {
    XrdsSceneAsset {
        id: id.to_string(),
        uri: format!("textures/{id}.png"),
        kind: XrdsSceneAssetKind::Texture,
    }
}

/// A `SetMaterial` that assigns `id` to the base-colour slot.
fn set_base_texture(id: Option<&str>) -> XrdsAction {
    XrdsAction::SetMaterial {
        base_color: None,
        metallic: None,
        roughness: None,
        texture: Some(XrdsActionTexture {
            slot: XrdsSceneMaterialTextureSlotKind::BaseColor,
            texture_asset_id: id.map(str::to_string),
        }),
    }
}

/// Titles of every diagnostic, for order-independent assertions.
fn titles(d: &XrdsSceneDocument) -> Vec<String> {
    d.track_diagnostics().into_iter().map(|x| x.title).collect()
}

fn has(d: &XrdsSceneDocument, title: &str) -> bool {
    titles(d).iter().any(|t| t == title)
}

fn find(d: &XrdsSceneDocument, title: &str) -> XrdsSceneTriggerDiagnostic {
    d.track_diagnostics()
        .into_iter()
        .find(|x| x.title == title)
        .unwrap_or_else(|| panic!("expected a {title:?} diagnostic, got {:?}", titles(d)))
}

// ---------------------------------------------------------------------------
// Round-trip / serde defaults
// ---------------------------------------------------------------------------

#[test]
fn track_round_trips_every_surviving_action_variant() {
    let track = XrdsTrack {
        assets: vec![
            row(
                1,
                vec![
                    key(
                        0.0,
                        XrdsAction::PlayGltfAnimation {
                            playback: XrdsSceneGltfPlayback {
                                selector: XrdsSceneGltfAnimationSelector::Name("Open".to_string()),
                                repeat: XrdsSceneAnimationRepeatMode::Once,
                                speed: 1.5,
                                start_paused: false,
                            },
                        },
                    ),
                    key(0.5, XrdsAction::StopGltfAnimation),
                    key(1.0, XrdsAction::SetVisible(false)),
                    key(1.5, teleport()),
                    key(2.0, animate(0.75)),
                ],
            ),
            XrdsTrackAsset { when_finished: Default::default(),
                target: XrdsActionTarget::TriggerSource,
                keys: vec![
                    key(
                        0.0,
                        XrdsAction::ModifyHealth {
                            delta: XrdsActionValue::FromTriggerSource,
                        },
                    ),
                    key(
                        0.25,
                        XrdsAction::SetMaterial {
                            base_color: Some([1.0, 0.0, 0.0, 1.0]),
                            metallic: Some(0.25),
                            roughness: None,
                            texture: None,
                        },
                    ),
                ],
            },
        ],
        duration_secs: Some(3.0),
        looping: true,
    };

    let json = serde_json::to_string_pretty(&track).expect("serialise");
    let restored: XrdsTrack = serde_json::from_str(&json).expect("deserialise");
    assert_eq!(track, restored, "round-trip produced a different Track");
}

#[test]
fn track_and_binding_deserialize_from_an_empty_object() {
    // Every field carries #[serde(default)], so a hand-authored minimal
    // document still loads — the additive-schema-evolution guarantee.
    let track: XrdsTrack = serde_json::from_str("{}").expect("track from {}");
    assert!(track.assets.is_empty());
    assert_eq!(track.duration_secs, None);
    assert!(!track.looping);

    let binding: XrdsTriggerBinding = serde_json::from_str("{}").expect("binding from {}");
    assert_eq!(binding.trigger, XrdsTriggerKind::ZoneEnter);
    assert_eq!(binding.track, None);
    assert!(!binding.disabled);
    assert_eq!(binding.hand, None);
}

#[test]
fn asset_row_target_defaults_to_self_node() {
    let asset: XrdsTrackAsset = serde_json::from_str("{}").expect("asset from {}");
    assert_eq!(asset.target, XrdsActionTarget::SelfNode);
    assert!(asset.keys.is_empty());
}

#[test]
fn set_transform_ease_defaults_to_cubic_when_omitted() {
    let json = r#"{"at_secs":0.0,"action":{"kind":"SetTransform","data":{
        "position":null,"rotation":null,"scale":null,"duration_secs":1.0}}}"#;
    let k: XrdsTrackKey = serde_json::from_str(json).expect("deserialise");
    match k.action {
        XrdsAction::SetTransform { ease, .. } => assert_eq!(ease, XrdsEaseCurve::Cubic),
        other => panic!("expected AnimateTransform, got {other:?}"),
    }
}

#[test]
fn a_payload_less_unrecognized_action_does_not_destroy_the_whole_document() {
    // Forward compatibility: an action tag this build has never heard of
    // must degrade to one skipped key, not a failed scene load. Realistic
    // here because scenes get pushed to a Quest APK that may lag the editor.
    //
    // NOTE this only holds for a *payload-less* unknown tag. See the ignored
    // test below for the payload-carrying case, which is broken.
    let json = r#"{
        "assets":[{"target":{"Node":7},"keys":[
            {"at_secs":0.0,"action":{"kind":"SomeFutureAction"}},
            {"at_secs":1.0,"action":{"kind":"StopGltfAnimation"}}
        ]}]
    }"#;
    let track: XrdsTrack = serde_json::from_str(json).expect("document must still load");
    assert_eq!(track.assets[0].keys.len(), 2);
    assert_eq!(track.assets[0].keys[0].action, XrdsAction::Unknown);
    assert_eq!(track.assets[0].keys[1].action, XrdsAction::StopGltfAnimation);
    assert!(!track.assets[0].keys[0].action.is_valid_in_track());
}

/// Was a known bug, recorded as an executable spec rather than prose, now
/// fixed: `XrdsAction` has a hand-written `Deserialize` that checks the
/// `kind` tag against a known-tags list *before* touching `data`, so an
/// unrecognized action with a payload degrades to `Unknown` instead of
/// failing the whole document. See `XrdsAction::Unknown`'s doc comment and
/// `docs/done/xrds-track-model-plan.md` §9.
#[test]
fn a_payload_carrying_unrecognized_action_should_not_destroy_the_whole_document() {
    // The fixture names a kind that is *deliberately* fictional, and must stay that
    // way. It has now gone stale twice: `PlayAudio` until 2026-08-19, then
    // `PlayVideo` until 2026-08-25 — each time a plausible-sounding placeholder
    // became a real action, and this test failed because the kind then parses and
    // its `data` does not match. A name no one would ever ship ends that cycle;
    // `an_unknown_trigger_kind_loads_and_is_inert` below already uses one.
    //
    // Worth knowing, since the failures showed it: the graceful path covers an
    // unknown *kind*, not a known kind carrying an unexpected payload. Adding a
    // field to an existing action is therefore a hard break for older builds, where
    // an unknown action is not. That is why `PlayAudio` and `PlayVideo` both ship
    // with no fields rather than a speculative volume or clip override.
    let json = r#"{
        "assets":[{"target":{"Node":7},"keys":[
            {"at_secs":0.0,"action":{"kind":"SomeFutureAction","data":{"clip":"intro.mp4"}}}
        ]}]
    }"#;
    let track: XrdsTrack =
        serde_json::from_str(json).expect("a newer editor's action must not break the load");
    assert_eq!(track.assets[0].keys[0].action, XrdsAction::Unknown);
}

#[test]
fn an_unknown_trigger_kind_loads_and_is_inert() {
    let json = r#"{"trigger":{"kind":"SomeFutureTrigger"},"track":"T"}"#;
    let binding: XrdsTriggerBinding = serde_json::from_str(json).expect("must still load");
    assert_eq!(binding.trigger, XrdsTriggerKind::Unknown);
}

// ---------------------------------------------------------------------------
// XrdsTrack / XrdsAction helpers
// ---------------------------------------------------------------------------

#[test]
fn self_duration_is_non_zero_only_for_interpolation() {
    assert_eq!(animate(1.5).self_duration_secs(), 1.5);
    assert_eq!(teleport().self_duration_secs(), 0.0);
    assert_eq!(XrdsAction::StopGltfAnimation.self_duration_secs(), 0.0);
    // A negative authored duration must not produce a negative span.
    assert_eq!(animate(-2.0).self_duration_secs(), 0.0);
}

#[test]
fn effective_duration_includes_the_last_keys_interpolation_tail() {
    // The whole point of the tail: a Track whose final key animates for 2s
    // must not report a duration that cuts that animation off.
    let track = XrdsTrack {
        assets: vec![row(1, vec![key(3.0, animate(2.0))])],
        ..XrdsTrack::default()
    };
    assert_eq!(track.effective_duration_secs(), 5.0);
}

#[test]
fn an_authored_duration_wins_over_the_computed_span() {
    let track = XrdsTrack {
        assets: vec![row(1, vec![key(3.0, animate(2.0))])],
        duration_secs: Some(10.0),
        ..XrdsTrack::default()
    };
    assert_eq!(track.effective_duration_secs(), 10.0);
}

#[test]
fn effective_duration_spans_every_row_not_just_the_first() {
    let track = XrdsTrack {
        assets: vec![
            row(1, vec![key(1.0, teleport())]),
            row(2, vec![key(8.0, teleport())]),
        ],
        ..XrdsTrack::default()
    };
    assert_eq!(track.effective_duration_secs(), 8.0);
}

#[test]
fn effective_duration_of_an_empty_track_is_zero() {
    assert_eq!(XrdsTrack::default().effective_duration_secs(), 0.0);
}

#[test]
fn flattened_keys_sorts_across_rows_and_keeps_each_keys_own_target() {
    let track = XrdsTrack {
        assets: vec![
            row(1, vec![key(2.0, teleport()), key(0.0, teleport())]),
            row(2, vec![key(1.0, teleport())]),
        ],
        ..XrdsTrack::default()
    };
    let flat = track.flattened_keys();
    let times: Vec<f32> = flat.iter().map(|(_, k)| k.at_secs).collect();
    assert_eq!(times, vec![0.0, 1.0, 2.0], "must be sorted by time across rows");

    // The row's target has to travel with the key — that is the whole
    // mechanism by which one Track drives several nodes.
    let owners: Vec<XrdsActionTarget> = flat.iter().map(|(t, _)| (*t).clone()).collect();
    assert_eq!(owners[0], XrdsActionTarget::Node(XrdsSceneNodeId(1)));
    assert_eq!(owners[1], XrdsActionTarget::Node(XrdsSceneNodeId(2)));
    assert_eq!(owners[2], XrdsActionTarget::Node(XrdsSceneNodeId(1)));
}

#[test]
fn two_keys_sharing_a_timestamp_both_survive_flattening() {
    // Concurrency on one beat is a feature, not a duplicate to collapse.
    let track = XrdsTrack {
        assets: vec![
            row(1, vec![key(1.0, teleport())]),
            row(2, vec![key(1.0, teleport())]),
        ],
        ..XrdsTrack::default()
    };
    assert_eq!(track.flattened_keys().len(), 2);
    assert_eq!(track.key_count(), 2);
}

#[test]
fn owned_nodes_reports_only_concrete_node_rows() {
    // SelfNode/TriggerSource resolve at fire time, so they have no
    // authoring-time identity and cannot take part in conflict checks.
    let track = XrdsTrack {
        assets: vec![
            row(1, vec![]),
            XrdsTrackAsset { when_finished: Default::default(), target: XrdsActionTarget::SelfNode, keys: vec![] },
            XrdsTrackAsset { when_finished: Default::default(), target: XrdsActionTarget::TriggerSource, keys: vec![] },
            row(2, vec![]),
        ],
        ..XrdsTrack::default()
    };
    assert_eq!(track.owned_nodes(), vec![XrdsSceneNodeId(1), XrdsSceneNodeId(2)]);
}

// ---------------------------------------------------------------------------
// Binding diagnostics
// ---------------------------------------------------------------------------

fn healthy_track() -> XrdsTrack {
    XrdsTrack { assets: vec![row(1, vec![key(0.0, teleport())])], ..XrdsTrack::default() }
}

#[test]
fn diagnostics_are_quiet_on_a_healthy_document() {
    let d = doc(
        vec![node_with_binding(1, binding_for("Open"))],
        vec![named("Open", healthy_track())],
    );
    assert_eq!(d.track_diagnostics(), Vec::new(), "healthy document produced diagnostics");
}

#[test]
fn diagnostics_flag_a_binding_naming_a_missing_track() {
    let d = doc(vec![node_with_binding(1, binding_for("Nope"))], Vec::new());
    let diag = find(&d, "Binding names a missing Track");
    assert_eq!(diag.severity, XrdsSceneTriggerDiagnosticSeverity::Error);
    assert_eq!(diag.node_id, Some(XrdsSceneNodeId(1)));
    assert!(diag.detail.contains("\"Nope\""), "detail should quote the name: {}", diag.detail);
}

#[test]
fn diagnostics_warn_about_a_binding_that_runs_nothing() {
    // Authored-but-unwired is the normal intermediate state, so this is a
    // warning rather than an error.
    let binding = XrdsTriggerBinding { track: None, ..binding_for("unused") };
    let d = doc(vec![node_with_binding(1, binding)], Vec::new());
    assert_eq!(
        find(&d, "Binding runs nothing").severity,
        XrdsSceneTriggerDiagnosticSeverity::Warning
    );
}

#[test]
fn diagnostics_flag_a_hand_filter_on_a_handless_trigger_kind() {
    let binding = XrdsTriggerBinding {
        trigger: XrdsTriggerKind::ZoneEnter,
        hand: Some(xrds_components::XrGrabHand::Left),
        ..binding_for("Open")
    };
    let d = doc(vec![node_with_binding(1, binding)], vec![named("Open", healthy_track())]);
    // Error, not Warning: it cannot ever fire, it does not merely misbehave.
    assert_eq!(
        find(&d, "Hand filter on a trigger kind with no hand").severity,
        XrdsSceneTriggerDiagnosticSeverity::Error
    );
}

#[test]
fn diagnostics_allow_a_hand_filter_on_a_grab_binding() {
    let binding = XrdsTriggerBinding {
        trigger: XrdsTriggerKind::Grabbed,
        hand: Some(xrds_components::XrGrabHand::Right),
        ..binding_for("Open")
    };
    let d = doc(vec![node_with_binding(1, binding)], vec![named("Open", healthy_track())]);
    assert!(!has(&d, "Hand filter on a trigger kind with no hand"), "{:?}", titles(&d));
}

#[test]
fn carries_hand_agrees_with_the_hand_filter_diagnostic() {
    // One source of truth: the editor's kind picker and this diagnostic both
    // read `carries_hand`, so drift between them would be a real bug.
    for kind in [
        XrdsTriggerKind::Grabbed,
        XrdsTriggerKind::Dropped,
        XrdsTriggerKind::HoverEnter,
        XrdsTriggerKind::HoverExit,
        XrdsTriggerKind::ButtonPress,
        XrdsTriggerKind::ButtonRelease,
        XrdsTriggerKind::SliderChange,
        XrdsTriggerKind::ToggleChange,
    ] {
        assert!(kind.carries_hand(), "{kind:?} should carry a hand");
    }
    for kind in [
        XrdsTriggerKind::ZoneEnter,
        XrdsTriggerKind::ZoneExit,
        XrdsTriggerKind::AnimationComplete,
        XrdsTriggerKind::Custom("x".to_string()),
        XrdsTriggerKind::Unknown,
    ] {
        assert!(!kind.carries_hand(), "{kind:?} should not carry a hand");
    }
}

#[test]
fn diagnostics_stay_quiet_about_disabled_bindings() {
    // A parked binding is intentionally inert; complaining about it would
    // make the "switch this off to isolate a problem" workflow noisy.
    let binding = XrdsTriggerBinding {
        trigger: XrdsTriggerKind::Custom("never_emitted".to_string()),
        disabled: true,
        ..binding_for("Open")
    };
    let d = doc(vec![node_with_binding(1, binding)], vec![named("Open", healthy_track())]);
    assert!(!has(&d, "Nothing emits this Custom trigger"), "{:?}", titles(&d));
}

// ---------------------------------------------------------------------------
// Track-shape diagnostics
// ---------------------------------------------------------------------------

#[test]
fn diagnostics_flag_an_asset_appearing_twice_in_one_track() {
    // The one-row-per-asset rule: two rows for one node means two schedules
    // fighting over it from inside the same Track.
    let d = doc(
        vec![node(1)],
        vec![named(
            "T",
            XrdsTrack {
                assets: vec![
                    row(1, vec![key(0.0, teleport())]),
                    row(1, vec![key(1.0, teleport())]),
                ],
                ..XrdsTrack::default()
            },
        )],
    );
    let diag = find(&d, "Asset appears twice in one Track");
    assert_eq!(diag.severity, XrdsSceneTriggerDiagnosticSeverity::Error);
    assert_eq!(diag.node_id, None, "a registry problem is not one node's fault");
}

#[test]
fn diagnostics_flag_an_asset_row_targeting_a_missing_node() {
    let d = doc(
        vec![node(1)],
        vec![named(
            "T",
            XrdsTrack { assets: vec![row(99, vec![key(0.0, teleport())])], ..XrdsTrack::default() },
        )],
    );
    assert!(has(&d, "Asset row targets a missing node"), "{:?}", titles(&d));
}

#[test]
fn diagnostics_flag_an_empty_track_and_an_empty_row() {
    let d = doc(vec![node(1)], vec![named("Empty", XrdsTrack::default())]);
    assert!(has(&d, "Empty Track"), "{:?}", titles(&d));

    let d2 = doc(
        vec![node(1)],
        vec![named("T", XrdsTrack { assets: vec![row(1, vec![])], ..XrdsTrack::default() })],
    );
    assert!(has(&d2, "Asset row has no events"), "{:?}", titles(&d2));
}

#[test]
fn diagnostics_flag_a_negative_event_time() {
    let d = doc(
        vec![node(1)],
        vec![named(
            "T",
            XrdsTrack { assets: vec![row(1, vec![key(-1.0, teleport())])], ..XrdsTrack::default() },
        )],
    );
    assert!(has(&d, "Event at a negative time"), "{:?}", titles(&d));
}

#[test]
fn diagnostics_flag_an_event_past_the_authored_duration() {
    let d = doc(
        vec![node(1)],
        vec![named(
            "T",
            XrdsTrack {
                assets: vec![row(1, vec![key(9.0, teleport())])],
                duration_secs: Some(2.0),
                ..XrdsTrack::default()
            },
        )],
    );
    assert!(has(&d, "Event past the Track's end"), "{:?}", titles(&d));
}

#[test]
fn diagnostics_flag_an_unrecognized_action_in_a_track() {
    let d = doc(
        vec![node(1)],
        vec![named(
            "T",
            XrdsTrack {
                assets: vec![row(1, vec![key(0.0, XrdsAction::Unknown)])],
                ..XrdsTrack::default()
            },
        )],
    );
    assert!(has(&d, "Unrecognized action"), "{:?}", titles(&d));
}

#[test]
fn diagnostics_flag_gltf_playback_on_a_non_gltf_node() {
    let d = doc(
        vec![node(1)],
        vec![named(
            "T",
            XrdsTrack {
                assets: vec![row(1, vec![key(0.0, XrdsAction::StopGltfAnimation)])],
                ..XrdsTrack::default()
            },
        )],
    );
    assert!(has(&d, "glTF action on a non-glTF node"), "{:?}", titles(&d));
}

#[test]
fn diagnostics_skip_the_gltf_check_for_a_self_node_row() {
    // A SelfNode row's node is unknown until fire time, so there is nothing
    // to check the payload of — guessing would be a false positive.
    let d = doc(
        vec![node(1)],
        vec![named(
            "T",
            XrdsTrack {
                assets: vec![XrdsTrackAsset { when_finished: Default::default(),
                    target: XrdsActionTarget::SelfNode,
                    keys: vec![key(0.0, XrdsAction::StopGltfAnimation)],
                }],
                ..XrdsTrack::default()
            },
        )],
    );
    assert!(!has(&d, "glTF action on a non-glTF node"), "{:?}", titles(&d));
}

#[test]
fn diagnostics_flag_a_transform_that_changes_nothing() {
    let no_op = XrdsAction::SetTransform {
        position: None,
        rotation: None,
        scale: None,
        duration_secs: 1.0,
        ease: XrdsEaseCurve::Cubic,
    };
    let d = doc(
        vec![node(1)],
        vec![named(
            "T",
            XrdsTrack {
                assets: vec![row(1, vec![key(0.0, no_op), key(1.0, animate(0.0))])],
                ..XrdsTrack::default()
            },
        )],
    );
    let t = titles(&d);
    assert!(t.contains(&"Interpolation changes nothing".to_string()), "{t:?}");
    // Deliberately NOT warned about: with `Teleport` deleted, duration 0 is the
    // normal way to author an instant change. Warning on it would flag correct
    // authoring.
    assert!(!t.contains(&"Interpolation has no duration".to_string()), "{t:?}");
}

// ---------------------------------------------------------------------------
// SetMaterial texture slots
// ---------------------------------------------------------------------------

#[test]
fn a_texture_slot_assignment_round_trips() {
    let track = XrdsTrack {
        assets: vec![row(1, vec![key(0.0, set_base_texture(Some("asset:wood")))])],
        ..XrdsTrack::default()
    };
    let json = serde_json::to_string(&track).expect("serialise");
    let restored: XrdsTrack = serde_json::from_str(&json).expect("deserialise");
    assert_eq!(track, restored);
}

#[test]
fn set_material_texture_defaults_to_none_when_omitted() {
    // Additive-schema guarantee: a document written before texture slots
    // existed must still load, with no slot assignment.
    let json = r#"{"kind":"SetMaterial","data":{
        "base_color":null,"metallic":0.5,"roughness":null}}"#;
    let action: XrdsAction = serde_json::from_str(json).expect("deserialise");
    match action {
        XrdsAction::SetMaterial { texture, metallic, .. } => {
            assert_eq!(texture, None);
            assert_eq!(metallic, Some(0.5));
        }
        other => panic!("expected SetMaterial, got {other:?}"),
    }
}

#[test]
fn a_texture_slot_assignment_counts_as_setting_something() {
    // Assigning only a texture must NOT trip "Material change sets nothing" —
    // that check predates texture slots and would otherwise flag correct
    // authoring, which is how authors learn to ignore diagnostics.
    let d = doc_with_assets(
        vec![node(1)],
        vec![named(
            "T",
            XrdsTrack {
                assets: vec![row(1, vec![key(0.0, set_base_texture(Some("asset:wood")))])],
                ..XrdsTrack::default()
            },
        )],
        vec![texture_asset("asset:wood")],
    );
    assert!(!has(&d, "Material change sets nothing"), "{:?}", titles(&d));
}

#[test]
fn clearing_a_slot_also_counts_as_setting_something() {
    // `texture_asset_id: None` means "clear this slot", a real thing to
    // author — not an unset field.
    let d = doc_with_assets(
        vec![node(1)],
        vec![named(
            "T",
            XrdsTrack {
                assets: vec![row(1, vec![key(0.0, set_base_texture(None))])],
                ..XrdsTrack::default()
            },
        )],
        vec![],
    );
    assert!(!has(&d, "Material change sets nothing"), "{:?}", titles(&d));
    assert!(!has(&d, "Texture asset is not in the catalog"), "{:?}", titles(&d));
}

#[test]
fn diagnostics_flag_a_texture_id_that_is_not_in_the_catalog() {
    let d = doc_with_assets(
        vec![node(1)],
        vec![named(
            "T",
            XrdsTrack {
                assets: vec![row(1, vec![key(0.0, set_base_texture(Some("asset:missing")))])],
                ..XrdsTrack::default()
            },
        )],
        vec![texture_asset("asset:wood")],
    );
    let d1 = find(&d, "Texture asset is not in the catalog");
    assert_eq!(d1.severity, XrdsSceneTriggerDiagnosticSeverity::Error);
    assert!(d1.detail.contains("asset:missing"), "{}", d1.detail);
}

#[test]
fn diagnostics_reject_a_non_texture_asset_in_a_texture_slot() {
    // An id that exists but names a glTF/audio asset resolves to no image at
    // runtime, so it is the same silent failure as a missing id.
    let d = doc_with_assets(
        vec![node(1)],
        vec![named(
            "T",
            XrdsTrack {
                assets: vec![row(1, vec![key(0.0, set_base_texture(Some("asset:model")))])],
                ..XrdsTrack::default()
            },
        )],
        vec![XrdsSceneAsset {
            id: "asset:model".to_string(),
            uri: "models/thing.glb".to_string(),
            kind: XrdsSceneAssetKind::Gltf,
        }],
    );
    assert!(has(&d, "Texture asset is not in the catalog"), "{:?}", titles(&d));
}

#[test]
fn diagnostics_flag_a_material_change_that_sets_nothing() {
    let d = doc(
        vec![node(1)],
        vec![named(
            "T",
            XrdsTrack {
                assets: vec![row(
                    1,
                    vec![key(
                        0.0,
                        XrdsAction::SetMaterial {
                            base_color: None,
                            metallic: None,
                            roughness: None,
                            texture: None,
                        },
                    )],
                )],
                ..XrdsTrack::default()
            },
        )],
    );
    assert!(has(&d, "Material change sets nothing"), "{:?}", titles(&d));
}

// ---------------------------------------------------------------------------
// Cross-Track conflicts
// ---------------------------------------------------------------------------

#[test]
fn diagnostics_warn_when_two_tracks_share_an_asset() {
    // Authoring the overlap is allowed — the two may never be fired
    // together. It is reported so the author learns the constraint before
    // hitting the runtime's reject-the-newcomer guard.
    let d = doc(
        vec![node(1), node(2)],
        vec![
            named(
                "A",
                XrdsTrack {
                    assets: vec![row(1, vec![key(0.0, teleport())])],
                    ..XrdsTrack::default()
                },
            ),
            named(
                "B",
                XrdsTrack {
                    assets: vec![
                        row(1, vec![key(0.0, teleport())]),
                        row(2, vec![key(0.0, teleport())]),
                    ],
                    ..XrdsTrack::default()
                },
            ),
        ],
    );
    let diag = find(&d, "Two Tracks share an asset");
    assert_eq!(diag.severity, XrdsSceneTriggerDiagnosticSeverity::Warning);
    assert!(
        diag.detail.contains("\"A\"") && diag.detail.contains("\"B\""),
        "should name both Tracks: {}",
        diag.detail
    );
}

#[test]
fn diagnostics_are_quiet_when_tracks_have_disjoint_assets() {
    // The whole point of the rule: disjoint Tracks may run concurrently.
    let d = doc(
        vec![node(1), node(2)],
        vec![
            named(
                "A",
                XrdsTrack {
                    assets: vec![row(1, vec![key(0.0, teleport())])],
                    ..XrdsTrack::default()
                },
            ),
            named(
                "B",
                XrdsTrack {
                    assets: vec![row(2, vec![key(0.0, teleport())])],
                    ..XrdsTrack::default()
                },
            ),
        ],
    );
    assert!(!has(&d, "Two Tracks share an asset"), "{:?}", titles(&d));
}

#[test]
fn a_looping_track_sharing_an_asset_is_an_error_not_a_warning() {
    // A looping Track never releases its assets, so the other can never run
    // at all — permanent rather than situational.
    let d = doc(
        vec![node(1)],
        vec![
            named(
                "Ambient",
                XrdsTrack {
                    assets: vec![row(1, vec![key(0.0, teleport())])],
                    looping: true,
                    ..XrdsTrack::default()
                },
            ),
            named(
                "Blocked",
                XrdsTrack {
                    assets: vec![row(1, vec![key(0.0, teleport())])],
                    ..XrdsTrack::default()
                },
            ),
        ],
    );
    let diag = find(&d, "A looping Track blocks another forever");
    assert_eq!(diag.severity, XrdsSceneTriggerDiagnosticSeverity::Error);
    // It must name the looping side as the cause, not merely list both.
    assert!(
        diag.detail.contains("\"Ambient\" loops"),
        "should name the looping Track as the cause: {}",
        diag.detail
    );
    assert!(
        !has(&d, "Two Tracks share an asset"),
        "the looping error replaces the plain warning rather than doubling it"
    );
}

#[test]
fn self_node_rows_never_produce_a_false_conflict() {
    // Two Tracks each using SelfNode resolve to different entities at fire
    // time, so treating them as a shared asset would be wrong.
    let self_row =
        || XrdsTrackAsset { when_finished: Default::default(), target: XrdsActionTarget::SelfNode, keys: vec![key(0.0, teleport())] };
    let d = doc(
        vec![node(1)],
        vec![
            named("A", XrdsTrack { assets: vec![self_row()], ..XrdsTrack::default() }),
            named("B", XrdsTrack { assets: vec![self_row()], ..XrdsTrack::default() }),
        ],
    );
    assert!(!has(&d, "Two Tracks share an asset"), "{:?}", titles(&d));
    assert!(!has(&d, "A looping Track blocks another forever"), "{:?}", titles(&d));
}

// ---------------------------------------------------------------------------
// Threshold watchers
// ---------------------------------------------------------------------------

fn watcher(fires: &str) -> XrdsThresholdWatcher {
    XrdsThresholdWatcher {
        observable: XrdsObservable::Height,
        crossing: XrdsCrossing::Above,
        value: 1.0,
        hysteresis: 0.0,
        fires: fires.to_string(),
        disabled: false,
    }
}

#[test]
fn threshold_watcher_round_trips() {
    let w = watcher("lifted");
    let json = serde_json::to_string_pretty(&w).expect("serialise");
    let restored: XrdsThresholdWatcher = serde_json::from_str(&json).expect("deserialise");
    assert_eq!(w, restored);
}

#[test]
fn a_watcher_counts_as_an_emitter_for_a_custom_binding() {
    let listener = XrdsTriggerBinding {
        trigger: XrdsTriggerKind::Custom("lifted".to_string()),
        ..binding_for("T")
    };
    let d = doc(
        vec![node_with_binding(1, listener), node_with_watcher(2, watcher("lifted"))],
        vec![named("T", healthy_track())],
    );
    assert!(!has(&d, "Nothing emits this Custom trigger"), "{:?}", titles(&d));
}

#[test]
fn diagnostics_warn_about_a_custom_trigger_nothing_emits() {
    // The most valuable diagnostic here: "never fires" and "not triggered
    // yet" are indistinguishable at runtime.
    let listener = XrdsTriggerBinding {
        trigger: XrdsTriggerKind::Custom("nobody_fires_this".to_string()),
        ..binding_for("T")
    };
    let d = doc(vec![node_with_binding(1, listener)], vec![named("T", healthy_track())]);
    // Warning, not Error: expert-layer Rust can fire custom triggers, and
    // the document cannot see that.
    assert_eq!(
        find(&d, "Nothing emits this Custom trigger").severity,
        XrdsSceneTriggerDiagnosticSeverity::Warning
    );
}

#[test]
fn a_disabled_watcher_is_not_an_emitter_and_reports_nothing_itself() {
    let listener = XrdsTriggerBinding {
        trigger: XrdsTriggerKind::Custom("lifted".to_string()),
        ..binding_for("T")
    };
    let mut w = watcher("lifted");
    w.disabled = true;
    w.hysteresis = -5.0; // would otherwise be flagged
    let d = doc(
        vec![node_with_binding(1, listener), node_with_watcher(2, w)],
        vec![named("T", healthy_track())],
    );
    assert!(
        has(&d, "Nothing emits this Custom trigger"),
        "a parked watcher must not count as an emitter: {:?}",
        titles(&d)
    );
    assert!(
        !has(&d, "Watcher has negative hysteresis"),
        "a parked watcher should not report its own problems: {:?}",
        titles(&d)
    );
}

#[test]
fn diagnostics_flag_a_distance_watcher_targeting_a_missing_node() {
    let mut w = watcher("close");
    w.observable = XrdsObservable::DistanceTo { node: XrdsSceneNodeId(99) };
    let d = doc(vec![node_with_watcher(1, w)], Vec::new());
    assert!(has(&d, "Watcher measures distance to a missing node"), "{:?}", titles(&d));
}

#[test]
fn diagnostics_flag_negative_hysteresis_and_an_empty_fires_name() {
    let mut w = watcher("");
    w.hysteresis = -1.0;
    let d = doc(vec![node_with_watcher(1, w)], Vec::new());
    let t = titles(&d);
    assert!(t.contains(&"Watcher has negative hysteresis".to_string()), "{t:?}");
    assert!(t.contains(&"Watcher fires an empty name".to_string()), "{t:?}");
}

// ---------------------------------------------------------------------------
// Authorable stop (§A5)
// ---------------------------------------------------------------------------

fn stop_binding(track: &str) -> XrdsTriggerBinding {
    XrdsTriggerBinding {
        trigger: XrdsTriggerKind::ButtonPress,
        track: Some(track.to_string()),
        effect: XrdsTriggerEffect::Stop,
        disabled: false,
        hand: None,
    }
}

#[test]
fn effect_defaults_to_fire_and_is_omitted_when_serialised() {
    // Both halves of the additive-schema guarantee: a document authored before
    // stop existed keeps its exact meaning, and adding the field does not churn
    // every binding in every saved file.
    let b: XrdsTriggerBinding =
        serde_json::from_str(r#"{"trigger":{"kind":"ButtonPress"}}"#).expect("deserialise");
    assert_eq!(b.effect, XrdsTriggerEffect::Fire);

    let json = serde_json::to_string(&b).expect("serialise");
    assert!(!json.contains("effect"), "a Fire binding must not write the field: {json}");
}

#[test]
fn a_stop_binding_round_trips() {
    let b = stop_binding("Open");
    let back: XrdsTriggerBinding =
        serde_json::from_str(&serde_json::to_string(&b).expect("serialise")).expect("deserialise");
    assert_eq!(b, back);
}

#[test]
fn diagnostics_warn_about_a_stop_for_a_track_nothing_fires() {
    // A Stop is a no-op when nothing is running, so it never errors at runtime
    // and never shows up as a failure. That silence is why it is worth saying at
    // author time.
    let d = doc(
        vec![node_with_binding(700, stop_binding("Open"))],
        vec![named("Open", XrdsTrack::default())],
    );

    let titles: Vec<String> =
        d.track_diagnostics().into_iter().map(|x| x.title).collect();
    assert!(
        titles.iter().any(|t| t == "Stop binding for a Track nothing fires"),
        "{titles:?}"
    );
}

#[test]
fn a_stop_is_quiet_when_something_fires_the_same_track() {
    let d = doc(
        vec![XrdsSceneNode {
            triggers: vec![binding_for("Open"), stop_binding("Open")],
            ..node(701)
        }],
        vec![named("Open", XrdsTrack::default())],
    );
    let titles: Vec<String> =
        d.track_diagnostics().into_iter().map(|x| x.title).collect();
    assert!(
        !titles.iter().any(|t| t == "Stop binding for a Track nothing fires"),
        "{titles:?}"
    );
}

#[test]
fn a_panel_elements_fire_binding_satisfies_a_nodes_stop_binding() {
    // The reason `all_trigger_bindings` exists: a caller that walked only
    // `node.triggers` would ignore every panel button, and warn that nothing
    // fires a Track a panel starts perfectly well.
    let mut d = doc(vec![], vec![named("Open", XrdsTrack::default())]);
    d.panels.push(XrdsPanelTemplate {
        id: XrdsPanelTemplateId(1),
        name: "P".to_string(),
        elements: vec![XrdsPanelElement::new(
            "go",
            XrdsSceneWorldWidget::Button(XrdsSceneWorldButton::default()),
        )],
        ..XrdsPanelTemplate::default()
    });
    let mut instance =
        XrdsScenePanelInstance { template_id: XrdsPanelTemplateId(1), ..Default::default() };
    instance.set_triggers("go", vec![binding_for("Open")]);
    d.nodes.push(XrdsSceneNode {
        payload: XrdsSceneNodePayload::Panel(instance),
        ..node(702)
    });
    d.nodes.push(node_with_binding(703, stop_binding("Open")));

    let titles: Vec<String> =
        d.track_diagnostics().into_iter().map(|x| x.title).collect();
    assert!(
        !titles.iter().any(|t| t == "Stop binding for a Track nothing fires"),
        "a panel element's Fire binding counts: {titles:?}"
    );
}

#[test]
fn a_disabled_fire_binding_does_not_satisfy_a_stop() {
    // A parked start button means nothing starts it, so the Stop is still inert.
    let mut parked = binding_for("Open");
    parked.disabled = true;
    let d = doc(
        vec![XrdsSceneNode { triggers: vec![parked, stop_binding("Open")], ..node(704) }],
        vec![named("Open", XrdsTrack::default())],
    );
    let titles: Vec<String> =
        d.track_diagnostics().into_iter().map(|x| x.title).collect();
    assert!(
        titles.iter().any(|t| t == "Stop binding for a Track nothing fires"),
        "{titles:?}"
    );
}

#[test]
fn a_stop_naming_a_missing_track_reports_only_the_missing_track() {
    // Two diagnostics for one mistake reads as two mistakes.
    let d = doc(vec![node_with_binding(705, stop_binding("Ghost"))], vec![]);
    let titles: Vec<String> =
        d.track_diagnostics().into_iter().map(|x| x.title).collect();
    assert!(titles.iter().any(|t| t == "Binding names a missing Track"), "{titles:?}");
    assert!(
        !titles.iter().any(|t| t == "Stop binding for a Track nothing fires"),
        "{titles:?}"
    );
}

/// A `PlayVideo` whose `data` omits `repeat` loads, and loops.
///
/// Looping is the right default: a screen that keeps playing is a visible mistake,
/// one that stops after a single showing looks like a broken decoder.
///
/// Note precisely what `#[serde(default)]` buys, because it is less than it looks.
/// The encoding is adjacently tagged, so a struct variant *requires* its `data`
/// object; the default covers a missing **field**, not a missing `data`. Turning a
/// field-less action into one with fields is therefore a hard break in this
/// encoding — `{"kind":"PlayVideo"}` stops parsing — which is the same lesson
/// `a_payload_carrying_unrecognized_action_should_not_destroy_the_whole_document`
/// records, and the reason `repeat` went in before this shipped rather than after.
#[test]
fn a_play_video_without_a_repeat_field_defaults_to_looping() {
    let json = r#"{"kind":"PlayVideo","data":{}}"#;
    let action: XrdsAction = serde_json::from_str(json).expect("must load without `repeat`");
    assert_eq!(
        action,
        XrdsAction::PlayVideo {
            repeat: XrdsSceneAnimationRepeatMode::Loop
        }
    );
}

/// Turning a field-less action into one with fields kills the whole document.
///
/// Pinned down rather than assumed, and the answer was the unwelcome one: a *known*
/// kind whose `data` is missing does **not** degrade to `XrdsAction::Unknown` — it
/// fails the parse outright, taking the document with it. The graceful path covers
/// unknown kinds only, exactly as
/// `a_payload_carrying_unrecognized_action_should_not_destroy_the_whole_document`
/// says, and this is the concrete demonstration of the cost.
///
/// So `#[serde(default)]` on `repeat` buys less than it appears to: it covers a
/// missing *field* inside `data`, not a missing `data`. That is why `repeat` went
/// into `PlayVideo` before it shipped rather than after — the field-less spelling
/// never reached a saved scene, so nothing has to be migrated.
///
/// If a shipped action ever does need a new field, this is the shape of the
/// problem: not a lossy load, a refused one.
#[test]
fn adding_fields_to_a_shipped_action_would_break_older_documents() {
    let json = r#"{"kind":"PlayVideo"}"#;
    assert!(
        serde_json::from_str::<XrdsAction>(json).is_err(),
        "a known kind with no `data` is refused, not degraded — if this ever starts          passing, the graceful path has been widened and the hard-break warning          above should be revisited"
    );
}

/// And an explicit `Once` survives a round trip.
#[test]
fn play_video_repeat_round_trips() {
    let action = XrdsAction::PlayVideo {
        repeat: XrdsSceneAnimationRepeatMode::Once,
    };
    let json = serde_json::to_string(&action).expect("serialise");
    let back: XrdsAction = serde_json::from_str(&json).expect("deserialise");
    assert_eq!(action, back, "round trip lost the repeat mode: {json}");
}
