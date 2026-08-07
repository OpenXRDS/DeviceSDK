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
            XrdsPanelElement {
                name: "start".to_string(),
                kind: button([0.0, 0.1]),
                triggers: vec![binding(XrdsTriggerKind::ButtonPress, Some("Play"))],
            },
            XrdsPanelElement::new("title", label()),
        ],
    );
    let json = serde_json::to_string(&t).expect("serialise");
    let back: XrdsPanelTemplate = serde_json::from_str(&json).expect("deserialise");
    assert_eq!(t, back);
}

#[test]
fn an_element_defaults_to_firing_nothing() {
    // `triggers` is `#[serde(default)]`, so a hand-authored element without it
    // still loads — the additive-schema guarantee.
    let json = r#"{"name":"title","kind":{"Label":{"text":"Hi","font_size":0.05,
        "color":[1,1,1,1],"local_position":[0,0],"layout_size":[0.2,0.06]}}}"#;
    let e: XrdsPanelElement = serde_json::from_str(json).expect("deserialise");
    assert!(e.triggers.is_empty());
    assert_eq!(e.name, "title");
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
    // two depths — which is the bug `XrdsHudTemplate::depth` has.
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
fn a_well_formed_panel_is_quiet() {
    let d = doc(
        vec![template(
            "MainMenu",
            vec![XrdsPanelElement {
                name: "start".to_string(),
                kind: button([0.0, 0.0]),
                triggers: vec![binding(XrdsTriggerKind::ButtonPress, Some("Play"))],
            }],
        )],
        vec!["Play"],
    );
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
    // The likeliest cause is changing an element's kind and leaving the binding
    // behind — inert, not wrong, so a warning.
    let d = doc(
        vec![template(
            "P",
            vec![XrdsPanelElement {
                name: "title".to_string(),
                kind: label(),
                triggers: vec![binding(XrdsTriggerKind::ButtonPress, Some("Play"))],
            }],
        )],
        vec!["Play"],
    );
    assert!(has(&d, "Element cannot emit this trigger"), "{:?}", titles(&d));
    let diag = d
        .panel_diagnostics()
        .into_iter()
        .find(|x| x.title == "Element cannot emit this trigger")
        .expect("present");
    assert_eq!(diag.severity, XrdsSceneTriggerDiagnosticSeverity::Warning);
    assert!(diag.detail.contains("Label"), "should name the kind: {}", diag.detail);
}

#[test]
fn diagnostics_flag_an_element_binding_naming_a_missing_track() {
    let d = doc(
        vec![template(
            "P",
            vec![XrdsPanelElement {
                name: "start".to_string(),
                kind: button([0.0, 0.0]),
                triggers: vec![binding(XrdsTriggerKind::ButtonPress, Some("Nope"))],
            }],
        )],
        vec!["Play"],
    );
    assert!(has(&d, "Element binding names a missing Track"), "{:?}", titles(&d));
}

#[test]
fn diagnostics_warn_about_an_element_binding_that_runs_nothing() {
    let d = doc(
        vec![template(
            "P",
            vec![XrdsPanelElement {
                name: "start".to_string(),
                kind: button([0.0, 0.0]),
                triggers: vec![binding(XrdsTriggerKind::ButtonPress, None)],
            }],
        )],
        vec![],
    );
    assert!(has(&d, "Element binding runs nothing"), "{:?}", titles(&d));
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
