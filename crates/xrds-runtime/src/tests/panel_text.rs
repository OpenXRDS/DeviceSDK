//! Panel text is sized in metres, and is never blank by default.
//!
//! Both of these presented as "the text in the label is not visible", and neither
//! was a rendering fault: one was a unit mismatch that shrank every panel label to
//! 1/100 of its authored size, the other was an element that defaulted to an empty
//! string with nothing on the canvas to reveal the text field.

use super::*;
use bevy_rich_text3d::Text3dStyling;

fn panel_with(elements: Vec<xrds_scene_graph::XrdsPanelElement>) -> (App, Entity) {
    let mut app = xrds_test_app();
    let panel = app.world_mut().spawn(Transform::default()).id();
    for element in &elements {
        crate::xrds_api::trigger_action::spawn_panel_element_in_world(
            app.world_mut(),
            panel,
            element,
            &[],
        );
    }
    (app, panel)
}

/// `world_scale` of the first descendant that has text styling.
fn text_world_scale(app: &App, root: Entity) -> f32 {
    let mut stack = vec![root];
    while let Some(e) = stack.pop() {
        if let Some(styling) = app.world().get::<Text3dStyling>(e) {
            return styling.world_scale.expect("world_scale set").x;
        }
        if let Some(children) = app.world().get::<Children>(e) {
            stack.extend(children.iter());
        }
    }
    panic!("no text styling found under {root:?}");
}

#[test]
fn a_labels_em_size_is_its_authored_size_in_metres() {
    let mut label = xrds_scene_graph::XrdsSceneWorldLabel::default();
    label.text = "Hello".to_string();
    label.font_size = 0.05;
    let (app, panel) = panel_with(vec![xrds_scene_graph::XrdsPanelElement::new(
        "l",
        xrds_scene_graph::XrdsSceneWorldWidget::Label(label),
    )]);

    // Not 0.0005. `font_size` is documented as em size in metres; the pixel-based
    // `XrdsText3D` node's `* 0.01` does not apply here, and applying it rendered
    // every panel label half a millimetre tall.
    assert!(
        (text_world_scale(&app, panel) - 0.05).abs() < 1e-6,
        "expected 0.05 m per em, got {}",
        text_world_scale(&app, panel)
    );
}

#[test]
fn a_buttons_label_is_sized_in_metres_too() {
    let mut button = xrds_scene_graph::XrdsSceneWorldButton::default();
    button.label = "Press".to_string();
    button.font_size = 0.04;
    let (app, panel) = panel_with(vec![xrds_scene_graph::XrdsPanelElement::new(
        "b",
        xrds_scene_graph::XrdsSceneWorldWidget::Button(button),
    )]);

    assert!(
        (text_world_scale(&app, panel) - 0.04).abs() < 1e-6,
        "expected 0.04 m per em, got {}",
        text_world_scale(&app, panel)
    );
}

#[test]
fn a_label_at_its_default_size_is_big_enough_to_read() {
    // Guards the unit mismatch from returning as a "harmless" default change: 5 cm
    // on a 0.4 m panel is legible, 0.5 mm is not.
    let default_em = xrds_scene_graph::XrdsSceneWorldLabel::default().font_size;
    assert!(
        default_em >= 0.01,
        "a default em size below 1 cm is unreadable on a panel; got {default_em}"
    );
}

#[test]
fn a_new_label_carries_placeholder_text() {
    let label = xrds_scene_graph::XrdsSceneWorldLabel::default();
    assert!(
        !label.text.is_empty(),
        "an element you add and cannot see reads as broken, not as unfilled"
    );
}

#[test]
fn a_new_button_carries_placeholder_text() {
    let button = xrds_scene_graph::XrdsSceneWorldButton::default();
    assert!(!button.label.is_empty());
}
