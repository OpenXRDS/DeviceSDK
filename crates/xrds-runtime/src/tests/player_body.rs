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

/// Mirrors what the **editor actually does**, which is not what the first version of
/// these tests assumed. `sync_stereo_cameras` (`viewport_camera.rs`) does not move the
/// marker between two cameras: it removes `XrdsPlayerCamera` from *one* entity when
/// stereo preview turns off and inserts it back on the *same* entity when it turns on —
/// and the insert runs **every frame** while enabled.
///
/// Two things have to hold. Toggling must not accumulate bodies, and the steady state
/// (re-inserting a component that is already present) must not build a second one.
#[test]
fn toggling_the_marker_on_one_entity_never_accumulates_bodies() {
    let mut app = xrds_test_app();
    let camera = spawn_player_camera(&mut app, 1.6);
    app.update();

    let count = |app: &mut App| {
        let mut q = app.world_mut().query::<&XrdsPlayerBodyCollider>();
        q.iter(app.world()).count()
    };
    assert_eq!(count(&mut app), 1, "one body after the first attach");

    for round in 0..3 {
        // Stereo preview off.
        app.world_mut().entity_mut(camera).remove::<XrdsPlayerCamera>();
        app.update();
        assert_eq!(
            count(&mut app),
            0,
            "round {round}: body should be gone while the marker is absent"
        );
        assert!(
            app.world().get::<RigidBody>(camera).is_none(),
            "round {round}: rigid body should be stripped too"
        );

        // Stereo preview on.
        app.world_mut().entity_mut(camera).insert(XrdsPlayerCamera);
        app.update();
        assert_eq!(count(&mut app), 1, "round {round}: exactly one body again");

        // The editor re-inserts every frame while enabled; that must be inert.
        for _ in 0..3 {
            app.world_mut().entity_mut(camera).insert(XrdsPlayerCamera);
            app.update();
        }
        assert_eq!(
            count(&mut app),
            1,
            "round {round}: re-inserting an existing marker must not add a body"
        );
    }

    // The reserved id must still point at the one live body, not a despawned entity.
    let (body, _) = body_of(&mut app).expect("body");
    assert_eq!(
        app.world().resource::<XrdsIdIndex>().entity_of(XRDS_PLAYER_ID),
        Some(body)
    );
}

/// The player must still walk through walls: this feature gives zones something to
/// detect, it does not add movement blocking. Blocking would need a locomotion
/// shapecast, and quietly acquiring it here would be a significant behaviour change.
///
/// The first assertion is a **control**. Without it the test would pass just as happily
/// in a world where physics never steps at all, which would make the real assertion
/// meaningless.
#[test]
fn the_player_body_does_not_block_movement_through_static_geometry() {
    use avian3d::prelude::LinearVelocity;

    let mut app = xrds_test_app();

    // A wall straight ahead of the camera's path.
    app.world_mut().spawn((
        RigidBody::Static,
        Collider::cuboid(4.0, 4.0, 0.2),
        Transform::from_xyz(0.0, 2.0, -2.0),
        GlobalTransform::default(),
    ));

    // Control: a dynamic body under gravity. If this does not move, physics is not
    // running and the rest of this test proves nothing.
    let probe = app
        .world_mut()
        .spawn((
            RigidBody::Dynamic,
            Collider::sphere(0.1),
            LinearVelocity(Vec3::new(0.0, -1.0, 0.0)),
            Transform::from_xyz(5.0, 5.0, 0.0),
            GlobalTransform::default(),
        ))
        .id();

    let camera = spawn_player_camera(&mut app, 1.6);
    app.update();
    assert!(body_of(&mut app).is_some(), "body should be attached");

    let probe_start = app.world().get::<Transform>(probe).unwrap().translation.y;

    // Walk the camera straight through the wall, as locomotion does: by writing the
    // transform directly, frame by frame.
    for step in 1..=20 {
        let z = 1.0 - step as f32 * 0.25;
        app.world_mut()
            .entity_mut(camera)
            .get_mut::<Transform>()
            .unwrap()
            .translation = Vec3::new(0.0, 1.6, z);
        app.update();
    }

    let probe_end = app.world().get::<Transform>(probe).unwrap().translation.y;
    assert!(
        probe_end < probe_start - 1e-4,
        "control failed: the dynamic probe did not move ({probe_start} -> {probe_end}), \
         so physics is not stepping and this test cannot say anything about blocking"
    );

    let ended_at = app.world().get::<Transform>(camera).unwrap().translation;
    assert!(
        (ended_at.z - (-4.0)).abs() < 1e-4,
        "the player must end where locomotion put them, i.e. past the wall at z=-4; \
         got {ended_at:?} — the body is blocking movement, which it must not do"
    );
}
