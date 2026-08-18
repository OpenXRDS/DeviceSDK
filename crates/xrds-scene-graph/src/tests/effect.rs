use super::*;

/// A scene authored before `XrdsSceneEffect` grew its current field set must
/// still load. The format is already in users' hands, so every field carries a
/// serde default; a bare `{}` stands in for that older document.
///
/// This also pins the defaults themselves. They are not arbitrary — they are the
/// values verified rendering correctly on Quest 3 hardware, so a silent drift to
/// zeros (an easy accident when adding a field) would produce invisible effects
/// on load rather than an error.
#[test]
fn a_minimal_effect_payload_deserializes_to_the_verified_defaults() {
    let effect: XrdsSceneEffect =
        serde_json::from_str("{}").expect("an empty effect payload should fall back to defaults");

    assert_eq!(effect, XrdsSceneEffect::default());

    assert_eq!(effect.kind, XrdsSceneEffectKind::Burst);
    assert!(effect.auto_play);
    assert_eq!(effect.burst_count, 300);
    assert_eq!(effect.spawn_rate, 100.0);
    assert_eq!(effect.lifetime_secs, 1.5);
    assert_eq!(effect.size_min, 0.05);
    assert_eq!(effect.size_max, 0.15);
    assert_eq!(effect.color_start, [1.0, 0.85, 0.35, 1.0]);
    assert_eq!(effect.color_end, [0.5, 0.08, 0.0, 0.0]);
    assert_eq!(effect.speed_min, 0.8);
    assert_eq!(effect.speed_max, 1.6);
    assert!(effect.omnidirectional);
    assert_eq!(effect.spread_deg, 45.0);
    assert_eq!(effect.gravity, [0.0, -1.2, 0.0]);
    assert_eq!(effect.emission_radius, 0.05);
    // Phase 5a fields. drag/fade_edge/fade_scene mirror bevy_firework's own
    // ParticleSettings defaults, which the spawner was already inheriting, so
    // these values keep older scenes looking exactly as they did.
    assert_eq!(effect.blend, XrdsSceneEffectBlend::Blend);
    assert_eq!(effect.size_end, 1.0);
    assert_eq!(effect.drag, 0.2);
    assert_eq!(effect.fade_edge, 0.7);
    assert_eq!(effect.fade_scene, 1.0);
}

/// `blend` is a wire enum, so a bad or future value must not silently become
/// `Blend` — it should fail loudly at parse time like any other tagged enum here.
#[test]
fn effect_blend_round_trips_and_rejects_unknown_values() {
    let effect: XrdsSceneEffect =
        serde_json::from_str(r#"{"blend":"Add"}"#).expect("Add should parse");
    assert_eq!(effect.blend, XrdsSceneEffectBlend::Add);

    let json = serde_json::to_string(&effect).expect("serialise");
    let back: XrdsSceneEffect = serde_json::from_str(&json).expect("reload");
    assert_eq!(back.blend, XrdsSceneEffectBlend::Add);

    assert!(
        serde_json::from_str::<XrdsSceneEffect>(r#"{"blend":"Screen"}"#).is_err(),
        "an unknown blend must be an error, not a silent fallback to Blend"
    );
}

/// Partial payloads must merge with defaults rather than being rejected, which
/// is what lets a hand-edited or older `scene.json` keep working.
#[test]
fn a_partial_effect_payload_keeps_defaults_for_absent_fields() {
    let effect: XrdsSceneEffect =
        serde_json::from_str(r#"{"kind":"Trail","spawn_rate":25.0,"auto_play":false}"#)
            .expect("a partial effect payload should deserialize");

    assert_eq!(effect.kind, XrdsSceneEffectKind::Trail);
    assert_eq!(effect.spawn_rate, 25.0);
    assert!(!effect.auto_play);
    // Untouched fields fall back rather than zeroing out.
    assert_eq!(effect.lifetime_secs, 1.5);
    assert_eq!(effect.burst_count, 300);
}

/// `auto_play` and `omnidirectional` are skipped when true, so the common case
/// stays terse in the file. They must still round-trip when false.
#[test]
fn effect_booleans_round_trip_when_false() {
    let mut effect = XrdsSceneEffect::default();
    effect.auto_play = false;
    effect.omnidirectional = false;

    let json = serde_json::to_string(&effect).expect("serialize");
    assert!(
        json.contains("auto_play"),
        "a false auto_play must be written out, not skipped: {json}"
    );

    let back: XrdsSceneEffect = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(back, effect);

    // ...and the true case is omitted, keeping typical documents small.
    let terse = serde_json::to_string(&XrdsSceneEffect::default()).expect("serialize default");
    assert!(
        !terse.contains("auto_play"),
        "a true auto_play should be skipped: {terse}"
    );
}

// ---------------------------------------------------------------------------
// PlayEffect action (Phase 4)
// ---------------------------------------------------------------------------

/// Guards the two hand-synced, *non*-compiler-enforced sites that `PlayEffect`
/// depends on: the `XrdsActionKnown` shadow enum and the `KNOWN_ACTION_KINDS`
/// wire-tag list. Miss either and the action silently deserialises as
/// `XrdsAction::Unknown` — the Track keeps loading, the key keeps its slot, and
/// nothing fires, with no error anywhere.
#[test]
fn play_effect_survives_a_wire_round_trip() {
    let json = r#"{
        "assets":[{"target":{"Node":7},"keys":[
            {"at_secs":0.0,"action":{"kind":"PlayEffect","data":{"count":250}}},
            {"at_secs":1.0,"action":{"kind":"PlayEffect","data":{"count":null}}}
        ]}]
    }"#;
    let track: XrdsTrack = serde_json::from_str(json).expect("track should load");

    assert_eq!(
        track.assets[0].keys[0].action,
        XrdsAction::PlayEffect { count: Some(250) },
        "an explicit count must survive; Unknown here means KNOWN_ACTION_KINDS \
         or the XrdsActionKnown shadow enum is missing PlayEffect"
    );
    // `count: null` is what the editor emits for a freshly added action (its
    // factory builds `PlayEffect { count: None }`), and it must mean "use the
    // effect's authored count".
    //
    // Note a bare `{"kind":"PlayEffect"}` with no `data` at all does NOT parse:
    // serde's adjacent tagging requires `content` to be present for a struct
    // variant, and `#[serde(default)]` on the inner field cannot supply a
    // missing `data`. That matches every other struct-shaped action here
    // (PlayGltfAnimation, SetTransform), so it is a consistency, not a gap --
    // but it does mean hand-written JSON must include `"data":{}` at minimum.
    assert_eq!(
        track.assets[0].keys[1].action,
        XrdsAction::PlayEffect { count: None }
    );
    assert!(track.assets[0].keys[0].action.is_valid_in_track());

    // ...and it must serialise back to a tag that reloads as itself.
    let out = serde_json::to_string(&track).expect("serialise");
    let back: XrdsTrack = serde_json::from_str(&out).expect("reload");
    assert_eq!(back.assets[0].keys[0].action, track.assets[0].keys[0].action);
}

/// A `PlayEffect` aimed at a node with no effect payload fires nothing, and a
/// `PlayEffect` aimed at an auto-playing effect fires nothing *for a different
/// reason*. Both are invisible on-device, so both are reported at author time.
#[test]
fn play_effect_diagnostics_catch_both_ways_of_firing_nothing() {
    let mut doc = XrdsSceneDocument::default();

    // 1: a cube — wrong payload entirely.
    let mut cube = xrds_components::primitives::XrdsCube::new().with_name("Cube");
    cube.transform.translation = [0.0; 3];
    doc.nodes
        .push(XrdsSceneNode::from_xrds_cube(XrdsSceneNodeId(1), None, &cube, None));

    // 2: an auto-playing effect — right payload, but already spent at load.
    let mut auto = XrdsEffect::new().with_name("AutoBurst");
    auto.auto_play = true;
    doc.nodes
        .push(XrdsSceneNode::from_xrds_effect(XrdsSceneNodeId(2), None, &auto));

    // 3: a trigger-ready effect — should raise nothing.
    let mut idle = XrdsEffect::new().with_name("IdleBurst");
    idle.auto_play = false;
    doc.nodes
        .push(XrdsSceneNode::from_xrds_effect(XrdsSceneNodeId(3), None, &idle));

    let track_for = |node: u64| XrdsNamedTrack {
        name: format!("fire{node}"),
        track: XrdsTrack {
            assets: vec![XrdsTrackAsset {
                target: XrdsActionTarget::Node(XrdsSceneNodeId(node)),
                keys: vec![XrdsTrackKey {
                    at_secs: 0.0,
                    action: XrdsAction::PlayEffect { count: None },
                }],
                when_finished: Default::default(),
            }],
            ..Default::default()
        },
    };
    doc.tracks = vec![track_for(1), track_for(2), track_for(3)];

    let diags = doc.track_diagnostics();
    let titles: Vec<&str> = diags.iter().map(|d| d.title.as_str()).collect();

    assert!(
        titles.contains(&"Effect action on a non-effect node"),
        "firing an effect at a cube should be an error; got {titles:?}"
    );
    assert!(
        titles.contains(&"Effect also fires itself on load"),
        "an auto-playing effect that is also Track-fired should warn about the \n         duplicate load-time burst; got {titles:?}"
    );
    // The correctly-wired node must not be implicated in either message.
    let mentions_idle = diags
        .iter()
        .any(|d| d.detail.contains("XrdsSceneNodeId(3)"));
    assert!(
        !mentions_idle,
        "an auto_play=false effect is the correct target and should raise nothing"
    );
}

/// `StopEffect` is payload-less, so unlike `PlayEffect` a bare
/// `{"kind":"StopEffect"}` *does* parse — serde only demands `content` for
/// struct-shaped variants. Guards the same two hand-synced sites
/// (`XrdsActionKnown`, `KNOWN_ACTION_KINDS`).
#[test]
fn stop_effect_survives_a_wire_round_trip() {
    let json = r#"{
        "assets":[{"target":{"Node":7},"keys":[
            {"at_secs":0.0,"action":{"kind":"PlayEffect","data":{"count":null}}},
            {"at_secs":2.0,"action":{"kind":"StopEffect"}}
        ]}]
    }"#;
    let track: XrdsTrack = serde_json::from_str(json).expect("track should load");

    assert_eq!(
        track.assets[0].keys[1].action,
        XrdsAction::StopEffect,
        "Unknown here means KNOWN_ACTION_KINDS or the XrdsActionKnown shadow enum          is missing StopEffect"
    );
    assert!(track.assets[0].keys[1].action.is_valid_in_track());

    let out = serde_json::to_string(&track).expect("serialise");
    let back: XrdsTrack = serde_json::from_str(&out).expect("reload");
    assert_eq!(back.assets[0].keys[1].action, XrdsAction::StopEffect);
}

/// The "also fires itself on load" warning is about `PlayEffect` duplicating a
/// load-time burst. `StopEffect` on an auto-playing effect is perfectly sensible
/// — that is how you turn one off — so it must not warn.
#[test]
fn stop_effect_does_not_warn_about_auto_play() {
    let mut doc = XrdsSceneDocument::default();
    let mut auto = XrdsEffect::new().with_name("AutoTrail").with_kind(XrdsEffectKind::Trail);
    auto.auto_play = true;
    doc.nodes
        .push(XrdsSceneNode::from_xrds_effect(XrdsSceneNodeId(1), None, &auto));
    doc.tracks = vec![XrdsNamedTrack {
        name: "hush".to_string(),
        track: XrdsTrack {
            assets: vec![XrdsTrackAsset {
                target: XrdsActionTarget::Node(XrdsSceneNodeId(1)),
                keys: vec![XrdsTrackKey { at_secs: 0.0, action: XrdsAction::StopEffect }],
                when_finished: Default::default(),
            }],
            ..Default::default()
        },
    }];

    let titles: Vec<String> = doc
        .track_diagnostics()
        .into_iter()
        .map(|d| d.title)
        .collect();
    assert!(
        !titles.iter().any(|t| t.contains("also fires itself on load")),
        "StopEffect on an auto-playing effect is the correct way to stop it; got {titles:?}"
    );
}

// ---------------------------------------------------------------------------
// When Finished (per-row Restore | Keep)
// ---------------------------------------------------------------------------

/// Documents written before `when_finished` existed must load as `Restore`, and
/// the common case must not bloat the file. `Restore` is skipped on serialise;
/// `Keep` — the author's deliberate opt-in — must always survive.
#[test]
fn when_finished_defaults_to_restore_and_only_keep_is_written() {
    let asset: XrdsTrackAsset =
        serde_json::from_str(r#"{"target":{"Node":1},"keys":[]}"#).expect("older row should load");
    assert_eq!(asset.when_finished, XrdsWhenFinished::Restore);

    // Restore is the default, so it stays out of the file.
    let terse = serde_json::to_string(&asset).expect("serialise");
    assert!(
        !terse.contains("when_finished"),
        "Restore should be omitted to keep documents small: {terse}"
    );

    // Keep must round-trip, or an author's choice would silently revert to
    // Restore on the next save — visible only when a Track ends.
    let mut keeper = asset.clone();
    keeper.when_finished = XrdsWhenFinished::Keep;
    let json = serde_json::to_string(&keeper).expect("serialise");
    assert!(json.contains("Keep"), "Keep must be written: {json}");
    let back: XrdsTrackAsset = serde_json::from_str(&json).expect("reload");
    assert_eq!(back.when_finished, XrdsWhenFinished::Keep);
}

/// An unknown value is an error rather than a silent fallback to `Restore`,
/// which would quietly discard a `Keep` an author had set.
#[test]
fn an_unknown_when_finished_value_is_rejected() {
    assert!(
        serde_json::from_str::<XrdsTrackAsset>(
            r#"{"target":{"Node":1},"keys":[],"when_finished":"Freeze"}"#
        )
        .is_err(),
        "an unrecognised When Finished must not degrade to Restore"
    );
}

/// An effects-only Track grants itself time after its last event, so the common
/// "just fire this burst" case works with no duration set and no warning.
///
/// Scoped to effects-only on purpose: extending *every* Track whose last event is
/// instantaneous was tried and reverted, because the agent then outlived its last
/// event holding its asset locks and blocked rapid re-firing.
#[test]
fn an_effects_only_track_gets_time_after_its_last_event() {
    let row = |keys: Vec<XrdsTrackKey>| XrdsTrackAsset {
        target: XrdsActionTarget::Node(XrdsSceneNodeId(1)),
        keys,
        when_finished: Default::default(),
    };

    let effects_only = XrdsTrack {
        assets: vec![row(vec![XrdsTrackKey {
            at_secs: 1.5,
            action: XrdsAction::PlayEffect { count: None },
        }])],
        ..Default::default()
    };
    assert_eq!(
        effects_only.effective_duration_secs(),
        1.5 + EFFECT_ONLY_TRACK_TAIL_SECS,
        "an effects-only Track must outlast its own last event"
    );

    // A lone effect at t=0 previously gave duration 0, which sends advance_tracks
    // down its degenerate "fire everything and despawn in one frame" branch.
    let at_zero = XrdsTrack {
        assets: vec![row(vec![XrdsTrackKey {
            at_secs: 0.0,
            action: XrdsAction::PlayEffect { count: None },
        }])],
        ..Default::default()
    };
    assert!(at_zero.effective_duration_secs() > 0.0);

    // Mixed content gets no tail: those Tracks are fire-and-forget reactions and
    // must release their locks immediately.
    let mixed = XrdsTrack {
        assets: vec![row(vec![
            XrdsTrackKey { at_secs: 0.0, action: XrdsAction::SetVisible(true) },
            XrdsTrackKey { at_secs: 2.0, action: XrdsAction::PlayEffect { count: None } },
        ])],
        ..Default::default()
    };
    assert_eq!(mixed.effective_duration_secs(), 2.0);

    // An explicit duration is always the author's call.
    let explicit = XrdsTrack {
        assets: vec![row(vec![XrdsTrackKey {
            at_secs: 1.5,
            action: XrdsAction::PlayEffect { count: None },
        }])],
        duration_secs: Some(1.5),
        ..Default::default()
    };
    assert_eq!(explicit.effective_duration_secs(), 1.5);
}

/// The warning now only catches the *mixed* case, since effects-only Tracks fix
/// themselves. Firing an effect at the end of a Track that also does other things
/// still ends on that event, and still needs saying.
#[test]
fn only_a_mixed_track_warns_about_an_effect_at_its_end() {
    let mut doc = XrdsSceneDocument::default();
    let effect = XrdsEffect::new().with_name("Burst");
    doc.nodes
        .push(XrdsSceneNode::from_xrds_effect(XrdsSceneNodeId(1), None, &effect));

    let track_of = |keys: Vec<XrdsTrackKey>, when: XrdsWhenFinished| XrdsNamedTrack {
        name: "bang".to_string(),
        track: XrdsTrack {
            assets: vec![XrdsTrackAsset {
                target: XrdsActionTarget::Node(XrdsSceneNodeId(1)),
                keys,
                when_finished: when,
            }],
            ..Default::default()
        },
    };
    let warned = |doc: &XrdsSceneDocument| {
        doc.track_diagnostics()
            .into_iter()
            .any(|d| d.title == "Effect fires as the Track ends")
    };

    // Effects only -> handled by the tail, so silent.
    doc.tracks = vec![track_of(
        vec![XrdsTrackKey { at_secs: 1.5, action: XrdsAction::PlayEffect { count: None } }],
        XrdsWhenFinished::Restore,
    )];
    assert!(!warned(&doc), "an effects-only Track fixes itself and must not warn");

    // Mixed, effect last -> warns.
    doc.tracks = vec![track_of(
        vec![
            XrdsTrackKey { at_secs: 0.0, action: XrdsAction::SetVisible(true) },
            XrdsTrackKey { at_secs: 2.0, action: XrdsAction::PlayEffect { count: None } },
        ],
        XrdsWhenFinished::Restore,
    )];
    assert!(warned(&doc), "a mixed Track ending on an effect still needs the warning");

    // ...unless the row is Keep, which already means "leave it running".
    doc.tracks = vec![track_of(
        vec![
            XrdsTrackKey { at_secs: 0.0, action: XrdsAction::SetVisible(true) },
            XrdsTrackKey { at_secs: 2.0, action: XrdsAction::PlayEffect { count: None } },
        ],
        XrdsWhenFinished::Keep,
    )];
    assert!(!warned(&doc), "a Keep row already handles it");
}
