//! A panel is grabbed by its handle, never by its face.
//!
//! The face has to stay clickable — that is what `world_ui_button_system` needs —
//! so arming grab on it means every button press risks dragging the panel instead.
//! These tests pin both halves: that the handle appears and disappears with the
//! grabbable state, and that the grab gate actually refuses a face hit.

use super::*;
use xrds_components::{XrGrabHandle, XrGrabHandleOnly, XrGrabbable};

/// Panel node with a backdrop, as `apply_panel_backdrop_in_world` leaves it.
fn spawn_panel_surface(app: &mut App, w: f32, h: f32) -> Entity {
    app.world_mut()
        .spawn((XrdsWorldSurface::new(w, h), Transform::default()))
        .id()
}

fn handle_children(app: &mut App, panel: Entity) -> Vec<Entity> {
    let Some(children) = app.world().get::<Children>(panel) else { return vec![] };
    children
        .iter()
        .filter(|&c| app.world().get::<XrGrabHandle>(c).is_some())
        .collect()
}

#[test]
fn a_grabbable_panel_gains_a_handle_and_is_no_longer_grabbable_by_its_face() {
    let mut app = xrds_test_app();
    let panel = spawn_panel_surface(&mut app, 0.6, 0.4);
    app.world_mut().entity_mut(panel).insert(XrGrabbable);

    app.update();

    assert_eq!(
        handle_children(&mut app, panel).len(),
        1,
        "a grabbable panel should have exactly one grab handle"
    );
    assert!(
        app.world().get::<XrGrabHandleOnly>(panel).is_some(),
        "the panel must be marked handle-only, or grab still arms on the whole face"
    );
}

#[test]
fn the_handle_hangs_below_the_panel_so_it_never_covers_the_clickable_face() {
    let mut app = xrds_test_app();
    let panel = spawn_panel_surface(&mut app, 0.6, 0.4);
    app.world_mut().entity_mut(panel).insert(XrGrabbable);
    app.update();

    let handle = handle_children(&mut app, panel)[0];
    let y = app.world().get::<Transform>(handle).expect("handle transform").translation.y;
    assert!(
        y < -0.2,
        "handle should sit below the panel's bottom edge (-0.2), found y = {y}"
    );
}

#[test]
fn clearing_grabbable_removes_the_handle() {
    let mut app = xrds_test_app();
    let panel = spawn_panel_surface(&mut app, 0.6, 0.4);
    app.world_mut().entity_mut(panel).insert(XrGrabbable);
    app.update();
    assert_eq!(handle_children(&mut app, panel).len(), 1);

    // The editor's checkbox route: the marker goes away, the document is untouched.
    app.world_mut().entity_mut(panel).remove::<XrGrabbable>();
    app.update();

    assert!(
        handle_children(&mut app, panel).is_empty(),
        "handle should be despawned once the panel stops being grabbable"
    );
    assert!(
        app.world().get::<XrGrabHandleOnly>(panel).is_none(),
        "handle-only marker must be cleared too, or a later grab can never start"
    );
}

#[test]
fn a_head_locked_panel_gets_no_handle_because_a_grab_would_be_overwritten() {
    let mut app = xrds_test_app();
    let panel = spawn_panel_surface(&mut app, 0.6, 0.4);
    app.world_mut().entity_mut(panel).insert((XrGrabbable, XrdsHeadLocked { local_offset: Transform::default() }));

    app.update();

    assert!(
        handle_children(&mut app, panel).is_empty(),
        "head_locked_system rewrites the Transform every frame, so a handle would lie"
    );
}

#[test]
fn head_locking_a_grabbable_panel_later_takes_its_handle_away() {
    let mut app = xrds_test_app();
    let panel = spawn_panel_surface(&mut app, 0.6, 0.4);
    app.world_mut().entity_mut(panel).insert(XrGrabbable);
    app.update();
    assert_eq!(handle_children(&mut app, panel).len(), 1);

    app.world_mut().entity_mut(panel).insert(XrdsHeadLocked { local_offset: Transform::default() });
    app.update();

    assert!(
        handle_children(&mut app, panel).is_empty(),
        "re-parenting a panel onto the player anchor must retire its handle"
    );
}

#[test]
fn the_handle_is_not_churned_every_frame() {
    let mut app = xrds_test_app();
    let panel = spawn_panel_surface(&mut app, 0.6, 0.4);
    app.world_mut().entity_mut(panel).insert(XrGrabbable);
    app.update();
    let first = handle_children(&mut app, panel);

    app.update();
    app.update();

    assert_eq!(
        handle_children(&mut app, panel),
        first,
        "respawning the handle each frame would churn meshes and drop a live grab"
    );
}

// ── The gate itself ──────────────────────────────────────────────────────────
// `grab_may_start_from` is what turns the handle from decoration into a rule.

#[test]
fn grab_may_not_start_from_a_handle_only_panels_own_face() {
    let mut app = xrds_test_app();
    let panel = spawn_panel_surface(&mut app, 0.6, 0.4);
    app.world_mut().entity_mut(panel).insert(XrGrabbable);
    app.update();

    assert!(
        !crate::xrds_api::grab::grab_may_start_from(app.world(), panel, panel),
        "a hit on the backdrop must not arm grab — that is the whole point"
    );
}

#[test]
fn grab_may_not_start_from_an_element_sitting_on_the_face() {
    let mut app = xrds_test_app();
    let panel = spawn_panel_surface(&mut app, 0.6, 0.4);
    app.world_mut().entity_mut(panel).insert(XrGrabbable);
    app.update();

    // A button, as `spawn_panel_element_in_world` parents it.
    let button = app.world_mut().spawn(Transform::default()).id();
    app.world_mut().entity_mut(button).insert(ChildOf(panel));

    assert!(
        !crate::xrds_api::grab::grab_may_start_from(app.world(), button, panel),
        "pressing a button must never drag its panel"
    );
}

#[test]
fn grab_may_start_from_the_handle() {
    let mut app = xrds_test_app();
    let panel = spawn_panel_surface(&mut app, 0.6, 0.4);
    app.world_mut().entity_mut(panel).insert(XrGrabbable);
    app.update();

    let handle = handle_children(&mut app, panel)[0];
    assert!(
        crate::xrds_api::grab::grab_may_start_from(app.world(), handle, panel),
        "the handle is the one surface that must arm grab"
    );
}

#[test]
fn an_ordinary_prop_is_still_grabbable_anywhere() {
    let mut app = xrds_test_app();
    // No `XrGrabHandleOnly` — a cube, a GLTF submesh, anything that is not UI.
    let prop = app.world_mut().spawn(Transform::default()).id();
    let submesh = app.world_mut().spawn(Transform::default()).id();
    app.world_mut().entity_mut(submesh).insert(ChildOf(prop));

    assert!(
        crate::xrds_api::grab::grab_may_start_from(app.world(), submesh, prop),
        "the handle rule must narrow grab for panels only, never for props"
    );
}

// ── Head-locked panels and world rays ────────────────────────────────────────
// A head-locked panel sits between the viewer and everything else by construction.
// `raycast_world` returns the nearest hit, so before this it swallowed every grab
// and every `ctx.raycast()` and nothing else in the scene could be pointed at.

use bevy::camera::primitives::Aabb;
use xrds_components::XrdsId;

/// A raycastable box of half-extent 0.25 at `z`, registered under `id` so
/// `find_xrds_ancestor` can resolve it.
fn spawn_raycastable(app: &mut App, id: u64, z: f32) -> Entity {
    let entity = app
        .world_mut()
        .spawn((
            Transform::from_xyz(0.0, 0.0, z),
            GlobalTransform::from(Transform::from_xyz(0.0, 0.0, z)),
            Aabb::from_min_max(Vec3::splat(-0.25), Vec3::splat(0.25)),
        ))
        .id();
    app.world_mut().resource_mut::<XrdsIdIndex>().register(XrdsId(id), entity);
    entity
}

/// Cast down -Z from the origin, as a hand aim pose does.
fn cast(app: &mut App) -> Vec<XrdsId> {
    crate::xrds_api::raycast::raycast_world(app.world_mut(), Vec3::ZERO, Vec3::NEG_Z, 20.0)
        .into_iter()
        .map(|h| h.id)
        .collect()
}

#[test]
fn a_head_locked_panel_does_not_swallow_rays_aimed_past_it() {
    let mut app = xrds_test_app();
    let panel = spawn_raycastable(&mut app, 1, -1.5); // the HUD, nearest
    spawn_raycastable(&mut app, 2, -5.0); // a cube behind it
    app.world_mut()
        .entity_mut(panel)
        .insert(XrdsHeadLocked { local_offset: Transform::default() });

    assert_eq!(
        cast(&mut app),
        vec![XrdsId(2)],
        "the cube behind the HUD must be reachable; the HUD itself must not be a hit"
    );
}

#[test]
fn a_world_panel_still_blocks_rays_so_its_grab_handle_keeps_working() {
    let mut app = xrds_test_app();
    spawn_raycastable(&mut app, 1, -1.5);
    spawn_raycastable(&mut app, 2, -5.0);

    // No XrdsHeadLocked — grab resolves through raycast_world, so a world panel
    // must stay hittable or its handle becomes unreachable.
    assert_eq!(cast(&mut app), vec![XrdsId(1), XrdsId(2)]);
}

#[test]
fn a_head_locked_elements_mesh_is_skipped_along_with_its_panel() {
    let mut app = xrds_test_app();
    let panel = spawn_raycastable(&mut app, 1, -1.5);
    spawn_raycastable(&mut app, 2, -5.0);
    app.world_mut()
        .entity_mut(panel)
        .insert(XrdsHeadLocked { local_offset: Transform::default() });

    // A button on the panel: its own mesh, no XrdsId and no head-lock mark of its
    // own — it resolves to the panel. Checking the hit mesh instead of the resolved
    // node would let this one through and re-block the ray.
    let button = app
        .world_mut()
        .spawn((
            Transform::from_xyz(0.0, 0.0, -1.4),
            GlobalTransform::from(Transform::from_xyz(0.0, 0.0, -1.4)),
            Aabb::from_min_max(Vec3::splat(-0.1), Vec3::splat(0.1)),
        ))
        .id();
    app.world_mut().entity_mut(button).insert(ChildOf(panel));

    assert_eq!(
        cast(&mut app),
        vec![XrdsId(2)],
        "an element's mesh must not reintroduce the panel its parent is exempt from"
    );
}

// ── Transparent backdrops ────────────────────────────────────────────────────
// The backdrop mesh is cosmetic; `XrdsWorldSurface` is what makes elements
// hittable. A transparent panel therefore has to keep the surface and drop the
// mesh — keeping an invisible mesh would leave the `Aabb` that blocks grab.

fn template_with_alpha(alpha: f32) -> xrds_scene_graph::XrdsPanelTemplate {
    let mut t = xrds_scene_graph::XrdsPanelTemplate::default();
    t.size = [0.6, 0.4];
    t.background.color = [0.1, 0.1, 0.12, alpha];
    t.background.opacity = 1.0;
    t
}

fn apply_backdrop(app: &mut App, alpha: f32) -> Entity {
    let entity = app.world_mut().spawn(Transform::default()).id();
    let template = template_with_alpha(alpha);
    crate::xrds_api::spawn::apply_panel_backdrop_in_world(app.world_mut(), entity, &template);
    entity
}

#[test]
fn an_opaque_panel_gets_both_a_backdrop_mesh_and_a_pointer_surface() {
    let mut app = xrds_test_app();
    let panel = apply_backdrop(&mut app, 0.9);

    assert!(app.world().get::<Mesh3d>(panel).is_some(), "an opaque panel should be drawn");
    assert!(
        app.world().get::<XrdsWorldSurface>(panel).is_some(),
        "without the surface no element on the panel can ever be pressed"
    );
}

#[test]
fn a_transparent_panel_keeps_its_pointer_surface() {
    let mut app = xrds_test_app();
    let panel = apply_backdrop(&mut app, 0.0);

    assert!(
        app.world().get::<XrdsWorldSurface>(panel).is_some(),
        "a transparent HUD must still be able to receive button presses"
    );
}

#[test]
fn a_transparent_panel_has_no_mesh_so_it_blocks_neither_light_nor_rays() {
    let mut app = xrds_test_app();
    let panel = apply_backdrop(&mut app, 0.0);
    app.update(); // let the Aabb backfill run

    assert!(
        app.world().get::<Mesh3d>(panel).is_none(),
        "an invisible mesh would still carry the Aabb that raycast_world hits"
    );
    assert!(
        app.world().get::<bevy::camera::primitives::Aabb>(panel).is_none(),
        "no mesh must mean no Aabb, or 'transparent' is a trap"
    );
}

#[test]
fn a_transparent_panel_is_still_grabbable_by_its_handle() {
    let mut app = xrds_test_app();
    let panel = apply_backdrop(&mut app, 0.0);
    app.world_mut().entity_mut(panel).insert(XrGrabbable);
    app.update();

    // The handle keys off XrdsWorldSurface, not off the backdrop mesh, so losing the
    // backdrop must not cost a world panel its only way of being moved.
    assert_eq!(handle_children(&mut app, panel).len(), 1);
}

// ── Only an interactive panel captures the pointer ───────────────────────────
// `nearest_panel_hit` claims the ray for a whole surface rectangle, so an
// info-only panel used to swallow the pointer across its entire area while
// offering nothing to press — worst on a HUD, which sits in front of the eye.

fn template_with(elements: Vec<xrds_scene_graph::XrdsPanelElement>)
    -> xrds_scene_graph::XrdsPanelTemplate
{
    let mut t = template_with_alpha(0.9);
    t.elements = elements;
    t
}

fn label(name: &str) -> xrds_scene_graph::XrdsPanelElement {
    xrds_scene_graph::XrdsPanelElement::new(
        name,
        xrds_scene_graph::XrdsSceneWorldWidget::Label(Default::default()),
    )
}

fn button(name: &str) -> xrds_scene_graph::XrdsPanelElement {
    xrds_scene_graph::XrdsPanelElement::new(
        name,
        xrds_scene_graph::XrdsSceneWorldWidget::Button(Default::default()),
    )
}

fn surface_of(app: &mut App, elements: Vec<xrds_scene_graph::XrdsPanelElement>) -> XrdsWorldSurface {
    let entity = app.world_mut().spawn(Transform::default()).id();
    let template = template_with(elements);
    crate::xrds_api::spawn::apply_panel_backdrop_in_world(app.world_mut(), entity, &template);
    app.world().get::<XrdsWorldSurface>(entity).expect("surface").clone()
}

#[test]
fn an_info_only_panel_does_not_capture_the_pointer() {
    let mut app = xrds_test_app();
    let surface = surface_of(&mut app, vec![label("info")]);

    assert!(
        !surface.enabled,
        "a panel with nothing pressable must not swallow the ray across its area"
    );
}

#[test]
fn an_empty_panel_does_not_capture_the_pointer() {
    let mut app = xrds_test_app();
    assert!(!surface_of(&mut app, vec![]).enabled);
}

#[test]
fn a_panel_with_a_button_still_captures_the_pointer() {
    let mut app = xrds_test_app();
    let surface = surface_of(&mut app, vec![label("caption"), button("go")]);

    assert!(
        surface.enabled,
        "one interactive element is enough — otherwise its button could never fire"
    );
}

#[test]
fn the_surface_still_carries_the_panel_size_even_when_it_does_not_capture() {
    let mut app = xrds_test_app();
    // Size is what maps a hit to panel-local metres. A disabled surface that forgot
    // it would break the moment a button was added.
    assert_eq!(surface_of(&mut app, vec![label("info")]).size, Vec2::new(0.6, 0.4));
}
