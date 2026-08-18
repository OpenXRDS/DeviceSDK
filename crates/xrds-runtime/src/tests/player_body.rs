use super::*;
use avian3d::prelude::{Collider, RigidBody};
use crate::xrds_api::{
    XrdsPlayerBody, XrdsPlayerBodyCollider, XrdsPlayerBodyConfig, XrdsPlayerCamera,
    XRDS_PLAYER_ID,
};

/// Spawns the marker the way a host app does — the SDK never spawns the player itself,
/// so every test here goes through `XrdsPlayerCamera` exactly as `spawn_app_camera` and
/// the editor's `viewport_camera` do.
fn spawn_player_camera(app: &mut App, eye_height: f32) -> Entity {
    app.world_mut()
        .spawn((
            XrdsPlayerCamera,
            Transform::from_xyz(0.0, eye_height, 0.0),
            GlobalTransform::default(),
        ))
        .id()
}

fn body_of(app: &mut App) -> Option<(Entity, XrdsPlayerBodyCollider)> {
    let mut q = app
        .world_mut()
        .query::<(Entity, &XrdsPlayerBodyCollider)>();
    q.iter(app.world()).next().map(|(e, c)| (e, *c))
}

/// The capsule must stand on the floor, not hang off the camera.
///
/// Worth its own assertion because getting it wrong is invisible in code review and
/// still "works" — a capsule centred on a 1.6 m camera spans 0.75..2.45, so it clears
/// any floor-level zone pad entirely while looking perfectly reasonable.
#[test]
fn player_body_capsule_base_sits_on_the_floor_not_at_eye_height() {
    let mut app = xrds_test_app();
    let eye_height = 1.6;
    let camera = spawn_player_camera(&mut app, eye_height);

    app.update();

    let (collider, owner) = body_of(&mut app).expect("a body should have been attached");
    assert_eq!(owner.camera, camera, "body should point at its camera");

    // Local offset, so it composes with the camera transform to put the body centre at
    // height/2 in world space.
    let tf = app
        .world()
        .get::<Transform>(collider)
        .expect("collider needs a Transform to carry the offset");
    let body = XrdsPlayerBody::default();
    let expected = body.height * 0.5 - eye_height;
    assert!(
        (tf.translation.y - expected).abs() < 1e-6,
        "expected local offset {expected}, got {}",
        tf.translation.y
    );

    // World-space sanity: base on the floor, top at standing height.
    let centre = eye_height + tf.translation.y;
    assert!((centre - body.height * 0.5).abs() < 1e-6, "body centre");

    assert!(app.world().get::<Collider>(collider).is_some());
    assert!(
        matches!(
            app.world().get::<RigidBody>(camera),
            Some(RigidBody::Kinematic)
        ),
        "kinematic, so locomotion's transform writes are not fought by the solver"
    );
}

/// Without this registration the collider is inert: `zone_collision_system` reads
/// `id_of` for *both* entities and drops the event when either is missing. A collider
/// with no id looks completely correct and fires nothing.
#[test]
fn player_body_is_registered_under_the_reserved_id() {
    let mut app = xrds_test_app();
    spawn_player_camera(&mut app, 1.6);
    app.update();

    let (collider, _) = body_of(&mut app).expect("body");
    let index = app.world().resource::<XrdsIdIndex>();
    assert_eq!(index.id_of(collider), Some(XRDS_PLAYER_ID));
    assert_eq!(index.entity_of(XRDS_PLAYER_ID), Some(collider));
    assert_eq!(
        XRDS_PLAYER_ID.0, 0,
        "must stay outside the allocator's range, which starts at 1"
    );
}

/// Observer mode: a camera that moves through the world without touching it.
#[test]
fn no_body_is_attached_when_the_config_is_none() {
    let mut app = xrds_test_app();
    app.insert_resource(XrdsPlayerBodyConfig(None));
    let camera = spawn_player_camera(&mut app, 1.6);

    app.update();

    assert!(
        body_of(&mut app).is_none(),
        "observer mode must not get a collider"
    );
    assert!(app.world().get::<RigidBody>(camera).is_none());
    assert!(app
        .world()
        .resource::<XrdsIdIndex>()
        .entity_of(XRDS_PLAYER_ID)
        .is_none());
}

/// The editor removes `XrdsPlayerCamera` from one camera and inserts it on another when
/// toggling play mode (`viewport_camera.rs:315`). A body left on the old camera would
/// keep firing zone events from wherever that camera sits — phantom triggers, not an
/// obvious leak.
#[test]
fn moving_the_player_marker_moves_the_body_and_leaves_nothing_behind() {
    let mut app = xrds_test_app();
    let first = spawn_player_camera(&mut app, 1.6);
    app.update();

    let (first_body, _) = body_of(&mut app).expect("first body");

    // Hand the marker to a different camera, as the editor does.
    app.world_mut().entity_mut(first).remove::<XrdsPlayerCamera>();
    let second = spawn_player_camera(&mut app, 1.2);
    app.update();

    assert!(
        app.world().get_entity(first_body).is_err(),
        "the old collider must be despawned, not orphaned"
    );
    assert!(
        app.world().get::<RigidBody>(first).is_none(),
        "the old camera must lose its rigid body too"
    );

    let (second_body, owner) = body_of(&mut app).expect("a body should follow the marker");
    assert_eq!(owner.camera, second);
    assert_ne!(second_body, first_body);

    // Exactly one body, and the reserved id points at the live one rather than a
    // despawned or recycled entity.
    let mut q = app.world_mut().query::<&XrdsPlayerBodyCollider>();
    assert_eq!(q.iter(app.world()).count(), 1, "never two bodies");
    assert_eq!(
        app.world().resource::<XrdsIdIndex>().entity_of(XRDS_PLAYER_ID),
        Some(second_body)
    );

    // The new camera's eye height is different, so the offset must have been recomputed
    // rather than copied.
    let tf = app.world().get::<Transform>(second_body).expect("transform");
    let expected = XrdsPlayerBody::default().height * 0.5 - 1.2;
    assert!((tf.translation.y - expected).abs() < 1e-6);
}

/// A radius at or past half the height would produce a negative cylinder length, which
/// panics inside the capsule constructor.
#[test]
fn a_degenerate_body_shape_is_clamped_rather_than_panicking() {
    let mut app = xrds_test_app();
    app.insert_resource(XrdsPlayerBodyConfig(Some(XrdsPlayerBody {
        height: 1.0,
        radius: 5.0,
    })));
    spawn_player_camera(&mut app, 1.6);

    app.update();

    let (collider, _) = body_of(&mut app).expect("a clamped body should still attach");
    assert!(app.world().get::<Collider>(collider).is_some());
}
