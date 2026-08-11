//! Children of an anchor-driven entity must not lag it by a frame.
//!
//! The anchor-mode systems run after `TransformSystems::Propagate` (they need a
//! fresh camera pose) and write their own `GlobalTransform` directly. Bevy has
//! already propagated to their children by then, using the *previous* pose, so
//! without a second targeted pass the children are permanently one frame stale.
//! On a head-locked panel that shows up as the labels trailing the backdrop they
//! are written on — reported as the label "stuttering on move".

use super::*;
use crate::xrds_api::anchor::{
    propagate_anchor_subtrees_system, XrdsBodyLocked, XrdsHeadLocked,
};

/// Bare app with only the propagation system, so nothing else can move anything
/// and mask what is being tested.
fn propagation_app() -> App {
    let mut app = App::new();
    app.add_systems(Update, propagate_anchor_subtrees_system);
    app
}

fn head_locked_at(app: &mut App, x: f32) -> Entity {
    let tf = Transform::from_xyz(x, 0.0, 0.0);
    app.world_mut()
        .spawn((tf, GlobalTransform::from(tf), XrdsHeadLocked { local_offset: tf }))
        .id()
}

fn child_at(app: &mut App, parent: Entity, offset: Vec3) -> Entity {
    let child = app
        .world_mut()
        .spawn((Transform::from_translation(offset), GlobalTransform::default()))
        .id();
    app.world_mut().entity_mut(child).insert(ChildOf(parent));
    child
}

fn global_x(app: &App, e: Entity) -> f32 {
    app.world().get::<GlobalTransform>(e).expect("global").translation().x
}

#[test]
fn a_child_follows_its_anchored_parent_in_the_same_frame() {
    let mut app = propagation_app();
    let panel = head_locked_at(&mut app, 10.0);
    let label = child_at(&mut app, panel, Vec3::new(0.25, 0.0, 0.0));

    app.update();

    assert!(
        (global_x(&app, label) - 10.25).abs() < 1e-5,
        "expected the label at 10.25, found {} — it is still on the stale pose",
        global_x(&app, label)
    );
}

#[test]
fn a_child_keeps_up_when_the_parent_moves_again() {
    // The stutter is a *moving* artefact: one static frame can look correct while
    // every subsequent move is a frame behind.
    let mut app = propagation_app();
    let panel = head_locked_at(&mut app, 0.0);
    let label = child_at(&mut app, panel, Vec3::new(0.25, 0.0, 0.0));
    app.update();

    // Stand in for head_locked_system writing a new pose after Propagate.
    let moved = Transform::from_xyz(5.0, 0.0, 0.0);
    app.world_mut().entity_mut(panel).insert((moved, GlobalTransform::from(moved)));
    app.update();

    assert!(
        (global_x(&app, label) - 5.25).abs() < 1e-5,
        "expected 5.25 after the parent moved, found {}",
        global_x(&app, label)
    );
}

#[test]
fn propagation_reaches_grandchildren() {
    // A button's text is a child of the button, which is a child of the panel.
    let mut app = propagation_app();
    let panel = head_locked_at(&mut app, 10.0);
    let button = child_at(&mut app, panel, Vec3::new(0.25, 0.0, 0.0));
    let text = child_at(&mut app, button, Vec3::new(0.05, 0.0, 0.0));

    app.update();

    assert!(
        (global_x(&app, text) - 10.30).abs() < 1e-5,
        "expected the button's text at 10.30, found {}",
        global_x(&app, text)
    );
}

#[test]
fn a_nested_anchored_child_keeps_its_own_pose() {
    // It owns its GlobalTransform — overwriting it from the parent would apply the
    // camera pose twice.
    let mut app = propagation_app();
    let panel = head_locked_at(&mut app, 10.0);

    let own = Transform::from_xyz(-3.0, 0.0, 0.0);
    let nested = app
        .world_mut()
        .spawn((own, GlobalTransform::from(own), XrdsBodyLocked { local_offset: own }))
        .id();
    app.world_mut().entity_mut(nested).insert(ChildOf(panel));

    app.update();

    assert!(
        (global_x(&app, nested) - -3.0).abs() < 1e-5,
        "expected the nested anchored entity to keep -3.0, found {}",
        global_x(&app, nested)
    );
}

#[test]
fn descendants_of_a_nested_anchored_child_follow_that_child_not_the_outer_parent() {
    // The nested entity is a root of this pass in its own right, so its subtree is
    // still reached — but anchored to *it* (-3.0 + 1.0), never to the outer panel
    // (which would give 10.0 + -3.0 + 1.0). That distinction is the whole reason the
    // walk skips anchored children instead of treating them as ordinary links.
    let mut app = propagation_app();
    let panel = head_locked_at(&mut app, 10.0);

    let own = Transform::from_xyz(-3.0, 0.0, 0.0);
    let nested = app
        .world_mut()
        .spawn((own, GlobalTransform::from(own), XrdsBodyLocked { local_offset: own }))
        .id();
    app.world_mut().entity_mut(nested).insert(ChildOf(panel));
    let leaf = child_at(&mut app, nested, Vec3::new(1.0, 0.0, 0.0));

    app.update();

    assert!(
        (global_x(&app, leaf) - -2.0).abs() < 1e-5,
        "expected the leaf at -2.0, following the nested anchor; found {}",
        global_x(&app, leaf)
    );
}

#[test]
fn an_unanchored_hierarchy_is_untouched() {
    // Ordinary scene nodes are Bevy's business; touching them here would duplicate
    // propagation for the whole scene.
    let mut app = propagation_app();
    let root = app
        .world_mut()
        .spawn((Transform::from_xyz(10.0, 0.0, 0.0), GlobalTransform::default()))
        .id();
    let child = child_at(&mut app, root, Vec3::new(0.25, 0.0, 0.0));

    app.update();

    assert!(global_x(&app, child).abs() < 1e-5);
}
