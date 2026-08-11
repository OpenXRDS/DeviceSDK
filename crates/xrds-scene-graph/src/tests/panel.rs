//! Tests for panel templates and `panel_diagnostics` — see
//! `docs/xrds-widget-template-plan.md` §A1.

use super::*;

// ---------------------------------------------------------------------------
// Fixtures
// ---------------------------------------------------------------------------

fn button(local_position: [f32; 2]) -> XrdsSceneWorldWidget {
    XrdsSceneWorldWidget::Button(XrdsSceneWorldButton {
        label: "OK".to_string(),
        local_position,
        ..Default::default()
    })
}

fn label() -> XrdsSceneWorldWidget {
    XrdsSceneWorldWidget::Label(XrdsSceneWorldLabel::default())
}

fn slider() -> XrdsSceneWorldWidget {
    XrdsSceneWorldWidget::Slider(XrdsSceneWorldSlider::default())
}

fn toggle() -> XrdsSceneWorldWidget {
    XrdsSceneWorldWidget::Toggle(XrdsSceneWorldToggle::default())
}

fn binding(kind: XrdsTriggerKind, track: Option<&str>) -> XrdsTriggerBinding {
    XrdsTriggerBinding {
        trigger: kind,
        track: track.map(str::to_string),
        effect: Default::default(),
        disabled: false,
        hand: None,
    }
}

fn template(name: &str, elements: Vec<XrdsPanelElement>) -> XrdsPanelTemplate {
    XrdsPanelTemplate { name: name.to_string(), elements, ..XrdsPanelTemplate::default() }
}

fn doc(panels: Vec<XrdsPanelTemplate>, tracks: Vec<&str>) -> XrdsSceneDocument {
    XrdsSceneDocument {
        panels,
        tracks: tracks
            .into_iter()
            .map(|n| XrdsNamedTrack { name: n.to_string(), track: XrdsTrack::default() })
            .collect(),
        ..XrdsSceneDocument::default()
    }
}

/// A document with one template (id 1) plus one Panel node instancing it, whose
/// `element_triggers` are `wiring`. This is the shape every element-trigger
/// diagnostic now works on: bindings live on the instance, never the template.
fn doc_with_instance(
    elements: Vec<XrdsPanelElement>,
    tracks: Vec<&str>,
    wiring: Vec<(&str, Vec<XrdsTriggerBinding>)>,
) -> XrdsSceneDocument {
    let t = XrdsPanelTemplate { id: XrdsPanelTemplateId(1), ..template("P", elements) };
    let mut instance = XrdsScenePanelInstance { template_id: XrdsPanelTemplateId(1), ..Default::default() };
    for (name, bindings) in wiring {
        instance.set_triggers(name, bindings);
    }
    let mut d = doc(vec![t], tracks);
    d.nodes.push(XrdsSceneNode {
        payload: XrdsSceneNodePayload::Panel(instance),
        ..panel_node(10, 1)
    });
    d
}

fn titles(d: &XrdsSceneDocument) -> Vec<String> {
    d.panel_diagnostics().into_iter().map(|x| x.title).collect()
}

fn has(d: &XrdsSceneDocument, title: &str) -> bool {
    titles(d).iter().any(|t| t == title)
}

// ---------------------------------------------------------------------------
// Schema / round-trip
// ---------------------------------------------------------------------------

#[test]
fn a_panel_template_round_trips() {
    let t = template(
        "MainMenu",
        vec![
            XrdsPanelElement::new("start", button([0.0, 0.1])),
            XrdsPanelElement::new("title", label()),
        ],
    );
    let json = serde_json::to_string(&t).expect("serialise");
    let back: XrdsPanelTemplate = serde_json::from_str(&json).expect("deserialise");
    assert_eq!(t, back);
}

#[test]
fn a_template_carries_no_trigger_bindings_at_all() {
    // The load-bearing property of the instance-owned model: if a `triggers` key
    // ever reappears on a serialised template, one template instanced on three
    // floors is back to firing all three doors from any one button.
    let t = template("P", vec![XrdsPanelElement::new("go", button([0.0, 0.0]))]);
    let json = serde_json::to_string(&t).expect("serialise");
    assert!(!json.contains("trigger"), "template must hold no bindings: {json}");
}

#[test]
fn an_instance_round_trips_its_element_wiring() {
    let mut i = XrdsScenePanelInstance {
        template_id: XrdsPanelTemplateId(3),
        ..Default::default()
    };
    i.set_triggers("go", vec![binding(XrdsTriggerKind::ButtonPress, Some("Play"))]);
    let json = serde_json::to_string(&i).expect("serialise");
    let back: XrdsScenePanelInstance = serde_json::from_str(&json).expect("deserialise");
    assert_eq!(i, back);
}

#[test]
fn an_instance_without_wiring_still_deserializes() {
    // Additive-schema guarantee: Panel nodes authored before bindings moved here.
    let json = r#"{"template_id":2}"#;
    let i: XrdsScenePanelInstance = serde_json::from_str(json).expect("deserialise");
    assert_eq!(i.template_id, XrdsPanelTemplateId(2));
    assert!(i.element_triggers.is_empty());
    assert!(i.triggers_for("anything").is_empty());
}

#[test]
fn setting_empty_bindings_removes_the_key_rather_than_storing_an_empty_list() {
    // An empty entry looks like wiring in the document and is not, and it would
    // make the dangling-key diagnostic fire on something the author cleared.
    let mut i = XrdsScenePanelInstance::default();
    i.set_triggers("go", vec![binding(XrdsTriggerKind::ButtonPress, Some("Play"))]);
    assert_eq!(i.element_triggers.len(), 1);
    i.set_triggers("go", vec![]);
    assert!(i.element_triggers.is_empty(), "{:?}", i.element_triggers);
}

#[test]
fn renaming_an_element_moves_its_wiring() {
    // Renames propagate because the intent is unambiguous — the element still
    // exists and is still the thing that was wired.
    let mut i = XrdsScenePanelInstance::default();
    let b = vec![binding(XrdsTriggerKind::ButtonPress, Some("Play"))];
    i.set_triggers("go", b.clone());
    i.rename_element("go", "start");
    assert!(i.triggers_for("go").is_empty());
    assert_eq!(i.triggers_for("start"), b.as_slice());
}

#[test]
fn renaming_an_unwired_element_is_a_no_op() {
    let mut i = XrdsScenePanelInstance::default();
    i.rename_element("go", "start");
    assert!(i.element_triggers.is_empty(), "must not invent an empty entry");
}

#[test]
fn the_registry_looks_templates_up_by_id_and_by_name() {
    // Both exist because instances store an id (stable across renames) while
    // authors pick by name.
    let mut d = doc(vec![template("MainMenu", vec![])], vec![]);
    d.panels[0].id = XrdsPanelTemplateId(7);

    assert_eq!(
        d.panel_template(XrdsPanelTemplateId(7)).map(|t| t.name.as_str()),
        Some("MainMenu")
    );
    assert_eq!(
        d.panel_template_by_name("MainMenu").map(|t| t.id),
        Some(XrdsPanelTemplateId(7))
    );
    assert!(d.panel_template(XrdsPanelTemplateId(99)).is_none());
    assert!(d.panel_template_mut(XrdsPanelTemplateId(7)).is_some());
}

#[test]
fn next_available_panel_template_id_avoids_collisions() {
    let mut d = doc(vec![template("A", vec![]), template("B", vec![])], vec![]);
    d.panels[0].id = XrdsPanelTemplateId(3);
    d.panels[1].id = XrdsPanelTemplateId(9);
    assert_eq!(d.next_available_panel_template_id(), XrdsPanelTemplateId(10));
    assert_eq!(
        XrdsSceneDocument::default().next_available_panel_template_id(),
        XrdsPanelTemplateId(1),
        "an empty registry should start at 1, not 0"
    );
}

#[test]
fn elements_are_addressed_by_name() {
    let t = template("P", vec![XrdsPanelElement::new("start", button([0.0, 0.0]))]);
    assert!(t.element("start").is_some());
    assert!(t.element("nope").is_none());
}

#[test]
fn a_panel_template_carries_no_placement() {
    // The whole point of "attachment is the only difference": a template holds
    // content, and depth/position belong to whatever instances it. If a
    // placement field ever appears here, one template can no longer be used at
    // two depths — which is the bug the retired `XrdsHudTemplate::depth` had.
    let json = serde_json::to_string(&XrdsPanelTemplate::default()).expect("serialise");
    for placement in ["depth", "translation", "anchor"] {
        assert!(!json.contains(placement), "template must not carry {placement:?}: {json}");
    }
}

// ---------------------------------------------------------------------------
// can_emit
// ---------------------------------------------------------------------------

#[test]
fn only_interactive_kinds_emit_and_each_emits_its_own() {
    let b = XrdsPanelElement::new("b", button([0.0, 0.0]));
    assert!(b.can_emit(&XrdsTriggerKind::ButtonPress));
    assert!(b.can_emit(&XrdsTriggerKind::ButtonRelease));
    assert!(!b.can_emit(&XrdsTriggerKind::SliderChange));

    let s = XrdsPanelElement::new("s", slider());
    assert!(s.can_emit(&XrdsTriggerKind::SliderChange));
    assert!(!s.can_emit(&XrdsTriggerKind::ButtonPress));

    let t = XrdsPanelElement::new("t", toggle());
    assert!(t.can_emit(&XrdsTriggerKind::ToggleChange));
    assert!(!t.can_emit(&XrdsTriggerKind::ButtonPress));

    let l = XrdsPanelElement::new("l", label());
    assert!(!l.is_interactive());
    for kind in [
        XrdsTriggerKind::ButtonPress,
        XrdsTriggerKind::SliderChange,
        XrdsTriggerKind::ToggleChange,
    ] {
        assert!(!l.can_emit(&kind), "a Label emits nothing");
    }
}

#[test]
fn an_element_cannot_emit_node_scoped_kinds() {
    // `Custom` and `RunawayDetected` dispatch to a document node's XrdsId, and
    // an element has no id — it is addressed as (panel, element name).
    let b = XrdsPanelElement::new("b", button([0.0, 0.0]));
    assert!(!b.can_emit(&XrdsTriggerKind::Custom("boom".to_string())));
    assert!(!b.can_emit(&XrdsTriggerKind::RunawayDetected));
    assert!(!b.can_emit(&XrdsTriggerKind::ZoneEnter));
}

// ---------------------------------------------------------------------------
// Diagnostics
// ---------------------------------------------------------------------------

#[test]
fn a_well_formed_panel_and_instance_are_quiet() {
    let d = doc_with_instance(
        vec![XrdsPanelElement::new("start", button([0.0, 0.0]))],
        vec!["Play"],
        vec![("start", vec![binding(XrdsTriggerKind::ButtonPress, Some("Play"))])],
    );
    assert!(d.panel_diagnostics().is_empty(), "{:?}", titles(&d));
}

#[test]
fn a_template_with_no_instances_is_quiet() {
    // Templates are a library: authoring one and not placing it yet is normal,
    // and now that bindings live on instances there is nothing on an unplaced
    // template left to validate beyond names.
    let d = doc(vec![template("Shelf", vec![XrdsPanelElement::new("go", button([0.0, 0.0]))])], vec![]);
    assert!(d.panel_diagnostics().is_empty(), "{:?}", titles(&d));
}

#[test]
fn diagnostics_flag_a_duplicate_element_name_as_an_error() {
    // Names are how elements are addressed, so a duplicate is ambiguous rather
    // than untidy.
    let d = doc(
        vec![template(
            "P",
            vec![
                XrdsPanelElement::new("ok", button([0.0, 0.0])),
                XrdsPanelElement::new("ok", button([0.1, 0.0])),
            ],
        )],
        vec![],
    );
    assert!(has(&d, "Duplicate element name"), "{:?}", titles(&d));
    let diag = d
        .panel_diagnostics()
        .into_iter()
        .find(|x| x.title == "Duplicate element name")
        .expect("present");
    assert_eq!(diag.severity, XrdsSceneTriggerDiagnosticSeverity::Error);
}

#[test]
fn a_duplicate_is_reported_once_not_once_per_extra_copy() {
    let d = doc(
        vec![template(
            "P",
            vec![
                XrdsPanelElement::new("ok", button([0.0, 0.0])),
                XrdsPanelElement::new("ok", button([0.1, 0.0])),
                XrdsPanelElement::new("ok", button([0.2, 0.0])),
            ],
        )],
        vec![],
    );
    let n = titles(&d).iter().filter(|t| *t == "Duplicate element name").count();
    assert_eq!(n, 1, "three copies of one name is still one problem");
}

#[test]
fn diagnostics_warn_when_an_element_cannot_emit_its_trigger() {
    // The likeliest cause is changing an element's kind in the template and
    // leaving the instance's binding behind — which the instance cannot see.
    // Inert, not wrong, so a warning.
    let d = doc_with_instance(
        vec![XrdsPanelElement::new("title", label())],
        vec!["Play"],
        vec![("title", vec![binding(XrdsTriggerKind::ButtonPress, Some("Play"))])],
    );
    let diag = d
        .panel_diagnostics()
        .into_iter()
        .find(|x| x.title == "Element cannot emit this trigger")
        .unwrap_or_else(|| panic!("{:?}", titles(&d)));
    assert_eq!(diag.severity, XrdsSceneTriggerDiagnosticSeverity::Warning);
    assert!(diag.detail.contains("Label"), "should name the kind: {}", diag.detail);
    // Attributed to the node, so the scene Inspector can show it in place —
    // template-owned bindings had nowhere to point.
    assert_eq!(diag.node_id, Some(XrdsSceneNodeId(10)));
}

#[test]
fn diagnostics_flag_an_element_binding_naming_a_missing_track() {
    let d = doc_with_instance(
        vec![XrdsPanelElement::new("start", button([0.0, 0.0]))],
        vec!["Play"],
        vec![("start", vec![binding(XrdsTriggerKind::ButtonPress, Some("Nope"))])],
    );
    assert!(has(&d, "Element binding names a missing Track"), "{:?}", titles(&d));
}

#[test]
fn diagnostics_warn_about_an_element_binding_that_runs_nothing() {
    let d = doc_with_instance(
        vec![XrdsPanelElement::new("start", button([0.0, 0.0]))],
        vec![],
        vec![("start", vec![binding(XrdsTriggerKind::ButtonPress, None)])],
    );
    assert!(has(&d, "Element binding runs nothing"), "{:?}", titles(&d));
}

#[test]
fn diagnostics_flag_wiring_that_names_a_deleted_element() {
    // The new hazard, replacing the one that moving bindings removed: deleting a
    // template element orphans every instance's binding to it. Kept rather than
    // dropped, so the authored wiring can be repointed instead of lost.
    let d = doc_with_instance(
        vec![XrdsPanelElement::new("start", button([0.0, 0.0]))],
        vec!["Play"],
        vec![("longGone", vec![binding(XrdsTriggerKind::ButtonPress, Some("Play"))])],
    );
    let diag = d
        .panel_diagnostics()
        .into_iter()
        .find(|x| x.title == "Binding names an element the template does not have")
        .unwrap_or_else(|| panic!("{:?}", titles(&d)));
    assert_eq!(diag.severity, XrdsSceneTriggerDiagnosticSeverity::Error);
    assert!(diag.detail.contains("longGone"), "must name it: {}", diag.detail);
}

#[test]
fn an_orphaned_binding_is_reported_once_not_also_as_a_track_problem() {
    // Two diagnostics for one mistake reads as two mistakes: once the element is
    // missing, its bindings' Track names are not worth separately checking.
    let d = doc_with_instance(
        vec![XrdsPanelElement::new("start", button([0.0, 0.0]))],
        vec![],
        vec![("longGone", vec![binding(XrdsTriggerKind::ButtonPress, Some("AlsoMissing"))])],
    );
    assert!(has(&d, "Binding names an element the template does not have"));
    assert!(!has(&d, "Element binding names a missing Track"), "{:?}", titles(&d));
}

#[test]
fn two_instances_of_one_template_wire_independently() {
    // The capability the whole change is for: floor 1's button opens floor 1's
    // door. Under template-owned bindings this was impossible to express.
    let t = XrdsPanelTemplate {
        id: XrdsPanelTemplateId(1),
        ..template("Elevator", vec![XrdsPanelElement::new("go", button([0.0, 0.0]))])
    };
    let mut d = doc(vec![t], vec!["OpenFloor1", "OpenFloor3"]);
    for (node_id, track) in [(10u64, "OpenFloor1"), (11, "OpenFloor3")] {
        let mut i = XrdsScenePanelInstance {
            template_id: XrdsPanelTemplateId(1),
            ..Default::default()
        };
        i.set_triggers("go", vec![binding(XrdsTriggerKind::ButtonPress, Some(track))]);
        d.nodes.push(XrdsSceneNode {
            payload: XrdsSceneNodePayload::Panel(i),
            ..panel_node(node_id, 1)
        });
    }

    assert!(d.panel_diagnostics().is_empty(), "{:?}", titles(&d));
    assert_eq!(d.panel_instance_count(XrdsPanelTemplateId(1)), 2);
}

#[test]
fn naming_policy_applies_to_panels_and_elements_too() {
    // Same validator as Track names — one policy, not three near-copies.
    let d = doc(vec![template("__reserved", vec![])], vec![]);
    assert!(has(&d, "Panel name is not usable"), "{:?}", titles(&d));

    let d2 = doc(vec![template("P ", vec![])], vec![]);
    assert!(has(&d2, "Panel name is not canonical"), "{:?}", titles(&d2));

    let d3 = doc(
        vec![template("P", vec![XrdsPanelElement::new("start ", button([0.0, 0.0]))])],
        vec![],
    );
    assert!(has(&d3, "Element name is not canonical"), "{:?}", titles(&d3));

    let d4 = doc(
        vec![template("P", vec![XrdsPanelElement::new("", button([0.0, 0.0]))])],
        vec![],
    );
    assert!(has(&d4, "Element name is not usable"), "{:?}", titles(&d4));
}

#[test]
fn diagnostics_warn_when_names_differ_only_by_case() {
    let d = doc(vec![template("Menu", vec![]), template("menu", vec![])], vec![]);
    assert!(has(&d, "Two panels differ only by case"), "{:?}", titles(&d));

    let d2 = doc(
        vec![template(
            "P",
            vec![
                XrdsPanelElement::new("Start", button([0.0, 0.0])),
                XrdsPanelElement::new("start", button([0.1, 0.0])),
            ],
        )],
        vec![],
    );
    assert!(has(&d2, "Two elements differ only by case"), "{:?}", titles(&d2));
}

#[test]
fn panel_problems_do_not_leak_into_track_diagnostics() {
    // Kept separate so the editor can show panel problems in the panel
    // workspace instead of mixing them into the Sequencer list.
    let d = doc(
        vec![template("P", vec![XrdsPanelElement::new("ok", button([0.0, 0.0]))])],
        vec![],
    );
    assert!(
        !d.track_diagnostics().iter().any(|x| x.detail.contains("element")),
        "track diagnostics should not report element problems"
    );
}

// ---------------------------------------------------------------------------
// Attachments
// ---------------------------------------------------------------------------

fn panel_node(id: u64, template: u64) -> XrdsSceneNode {
    XrdsSceneNode {
        id: XrdsSceneNodeId(id),
        parent_id: None,
        name: format!("Panel{id}"),
        enabled: true,
        visible: true,
        grabbable: false,
        transform: XrdsSceneTransform::default(),
        payload: XrdsSceneNodePayload::Panel(XrdsScenePanelInstance {
            template_id: XrdsPanelTemplateId(template),
            ..Default::default()
        }),
        editor: XrdsEditorMetadata::default(),
        triggers: Vec::new(),
        watchers: Vec::new(),
    }
}

fn doc_with_nodes(nodes: Vec<XrdsSceneNode>, panels: Vec<XrdsPanelTemplate>) -> XrdsSceneDocument {
    XrdsSceneDocument { nodes, panels, ..XrdsSceneDocument::default() }
}

#[test]
fn diagnostics_flag_a_panel_instance_naming_a_missing_template() {
    let d = doc_with_nodes(vec![panel_node(30, 404)], vec![]);
    assert!(has(&d, "Panel instance names a missing template"), "{:?}", titles(&d));
}

// ---------------------------------------------------------------------------
// Element action targets (Phase B)
// ---------------------------------------------------------------------------

fn element_row_track(panel: u64, element: &str) -> XrdsTrack {
    XrdsTrack {
        assets: vec![XrdsTrackAsset {
            target: XrdsActionTarget::Element {
                panel: XrdsSceneNodeId(panel),
                name: element.to_string(),
            },
            keys: vec![XrdsTrackKey {
                at_secs: 0.0,
                action: XrdsAction::SetTransform {
                    position: Some([1.0, 0.0, 0.0]),
                    rotation: None,
                    scale: None,
                    duration_secs: 0.0,
                    ease: XrdsEaseCurve::Linear,
                },
            }],
        }],
        ..XrdsTrack::default()
    }
}

fn track_titles(d: &XrdsSceneDocument) -> Vec<String> {
    d.track_diagnostics().into_iter().map(|x| x.title).collect()
}

#[test]
fn an_element_target_is_not_copy_but_round_trips() {
    // `XrdsActionTarget` gave up `Copy` for this variant; the schema still has to
    // survive a save/load.
    let t = XrdsActionTarget::Element {
        panel: XrdsSceneNodeId(10),
        name: "go".to_string(),
    };
    let back: XrdsActionTarget =
        serde_json::from_str(&serde_json::to_string(&t).expect("serialise")).expect("deserialise");
    assert_eq!(t, back);
}

#[test]
fn a_valid_element_row_is_quiet() {
    let mut d = doc_with_instance(
        vec![XrdsPanelElement::new("go", button([0.0, 0.0]))],
        vec![],
        vec![],
    );
    d.tracks.push(XrdsNamedTrack {
        name: "Light".to_string(),
        track: element_row_track(10, "go"),
    });
    assert!(track_titles(&d).is_empty(), "{:?}", track_titles(&d));
}

#[test]
fn diagnostics_flag_an_element_row_on_a_missing_panel() {
    let mut d = doc_with_instance(
        vec![XrdsPanelElement::new("go", button([0.0, 0.0]))],
        vec![],
        vec![],
    );
    d.tracks.push(XrdsNamedTrack {
        name: "Light".to_string(),
        track: element_row_track(404, "go"),
    });
    assert!(
        track_titles(&d)
            .iter()
            .any(|t| t == "Asset row targets an element of a missing panel"),
        "{:?}",
        track_titles(&d)
    );
}

#[test]
fn diagnostics_flag_an_element_row_naming_an_element_the_panel_lacks() {
    // The rename case: the panel is fine, the element moved out from under the
    // row. Distinguished from a missing panel because the fix is different.
    let mut d = doc_with_instance(
        vec![XrdsPanelElement::new("go", button([0.0, 0.0]))],
        vec![],
        vec![],
    );
    d.tracks.push(XrdsNamedTrack {
        name: "Light".to_string(),
        track: element_row_track(10, "notAnElement"),
    });
    let titles = track_titles(&d);
    assert!(
        titles.iter().any(|t| t == "Asset row targets an element the panel does not have"),
        "{titles:?}"
    );
    assert!(
        !titles.iter().any(|t| t == "Asset row targets an element of a missing panel"),
        "the panel exists, so only one of the two applies: {titles:?}"
    );
}

#[test]
fn diagnostics_flag_an_element_row_on_a_node_that_is_not_a_panel() {
    // Not a rename — a wrong reference. Worth its own message so the author is not
    // sent hunting for a renamed element that never existed.
    let mut d = doc(vec![], vec![]);
    d.nodes.push(XrdsSceneNode {
        payload: XrdsSceneNodePayload::Empty,
        ..panel_node(20, 1)
    });
    d.tracks.push(XrdsNamedTrack {
        name: "Light".to_string(),
        track: element_row_track(20, "go"),
    });
    assert!(
        track_titles(&d)
            .iter()
            .any(|t| t == "Asset row targets an element of a non-panel node"),
        "{:?}",
        track_titles(&d)
    );
}

#[test]
fn two_element_rows_for_one_element_are_still_one_row_per_asset() {
    // The one-row-per-asset rule is about the *target*, so it has to see an
    // element target as an asset like any other.
    let mut d = doc_with_instance(
        vec![XrdsPanelElement::new("go", button([0.0, 0.0]))],
        vec![],
        vec![],
    );
    let mut track = element_row_track(10, "go");
    track.assets.push(track.assets[0].clone());
    d.tracks.push(XrdsNamedTrack { name: "Light".to_string(), track });
    assert!(
        track_titles(&d).iter().any(|t| t == "Asset appears twice in one Track"),
        "{:?}",
        track_titles(&d)
    );
}

#[test]
fn element_rows_on_two_instances_are_two_different_assets() {
    // The addressing property, at the schema level: same element name, different
    // panel node, so no duplicate-asset complaint.
    let t = XrdsPanelTemplate {
        id: XrdsPanelTemplateId(1),
        ..template("P", vec![XrdsPanelElement::new("go", button([0.0, 0.0]))])
    };
    let mut d = doc(vec![t], vec![]);
    d.nodes.push(panel_node(10, 1));
    d.nodes.push(panel_node(11, 1));
    let mut track = element_row_track(10, "go");
    track.assets.push(element_row_track(11, "go").assets.remove(0));
    d.tracks.push(XrdsNamedTrack { name: "Light".to_string(), track });
    assert!(track_titles(&d).is_empty(), "{:?}", track_titles(&d));
}
