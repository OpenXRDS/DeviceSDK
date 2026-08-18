use bevy::ecs::message::MessageReader;
use bevy::prelude::*;
use openxr::ActiveActionSet;

use crate::openxr::{
    resources::{OpenXrFrameState, OpenXrInstance, OpenXrPrimaryReferenceSpace, OpenXrSpace},
    schedule::{openxr_in_state_focused, OpenXrRuntimeSystems, OpenXrSchedules},
    session::OpenXrSession,
};

// ---------------------------------------------------------------------------
// Public types — read these in app systems
// ---------------------------------------------------------------------------

/// Polled state of both XR controller/hand pointers. Available as a Bevy resource.
#[derive(Resource, Clone, Debug, Default)]
pub struct XrInput {
    pub left:  XrPointerState,
    pub right: XrPointerState,
}

/// State for one hand's input (controller or hand-tracked).
#[derive(Clone, Copy, Debug, Default)]
pub struct XrPointerState {
    /// World-space aim pose. `None` when the controller is not tracked or not connected.
    pub pose:              Option<Transform>,
    /// Trigger pull 0–1 (controller trigger or index-thumb pinch strength for hands).
    pub trigger:           f32,
    /// Grip squeeze 0–1 (controller grip or fist strength for hands).
    pub grip:              f32,
    /// Thumbstick 2D axis; zero for hand-tracking sources.
    pub thumbstick:        Vec2,
    /// True when trigger > 0.5 or face-button select is pressed.
    pub select:            bool,
    /// True for exactly one frame when `select` transitions false → true.
    /// Works for both controller (trigger threshold) and hand tracking (pinch).
    pub select_just_pressed:  bool,
    /// True for exactly one frame when `select` transitions true → false.
    pub select_just_released: bool,
    /// Secondary face button: Y on left hand, B on right hand.
    /// Useful for back/cancel actions. False for hand-tracking sources.
    pub menu:              bool,
    /// True for exactly one frame when `menu` is first pressed this sync.
    pub menu_just_pressed: bool,
    /// Thumbstick pressed down as a button. False for hand-tracking sources.
    pub thumbstick_click:  bool,
    /// True for exactly one frame when `thumbstick_click` is first pressed this sync.
    pub thumbstick_click_just_pressed: bool,
    /// Whether this state came from a physical controller or hand tracking.
    pub source:            XrInputSource,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum XrInputSource {
    #[default]
    Controller,
    Hand,
}

/// Which hand to target for haptic output.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum XrHand {
    Left,
    Right,
}

/// Send this message to trigger a haptic pulse on a controller.
///
/// Register with `app.add_message::<XrHapticRequest>()` (done automatically by `XrInputPlugin`).
/// Write with `MessageWriter<XrHapticRequest>` or `world.write_message(...)`.
#[derive(Clone, Copy, Debug, bevy::prelude::Message)]
pub struct XrHapticRequest {
    pub hand: XrHand,
    /// Vibration intensity, clamped to 0.0–1.0.
    pub amplitude: f32,
    /// Duration in seconds. Values ≤ 0 use the runtime minimum pulse.
    pub duration_secs: f32,
    /// Frequency in Hz. Use 0.0 to let the runtime choose.
    pub frequency: f32,
}

// ---------------------------------------------------------------------------
// Internal resource — stores OpenXR action handles
// ---------------------------------------------------------------------------

#[derive(Resource)]
pub(crate) struct OpenXrInput {
    pub action_set:       openxr::ActionSet,
    pub aim_pose:         openxr::Action<openxr::Posef>,
    pub trigger:          openxr::Action<f32>,
    pub grip:             openxr::Action<f32>,
    pub thumbstick:       openxr::Action<openxr::Vector2f>,
    pub select:           openxr::Action<bool>,
    /// Y button (left) / B button (right) — secondary face button.
    pub menu:             openxr::Action<bool>,
    /// Thumbstick pressed as a button.
    pub thumbstick_click: openxr::Action<bool>,
    /// Haptic output action — used by `apply_haptic_feedback_system`.
    pub haptic:           openxr::Action<openxr::Haptic>,
    /// Aim spaces created after session attach; `None` until `attach_xr_input` runs.
    pub aim_space_left:   Option<openxr::Space>,
    pub aim_space_right:  Option<openxr::Space>,
    pub path_left:        openxr::Path,
    pub path_right:       openxr::Path,
    // --- Hand tracking (XR_EXT_hand_tracking) ---
    /// `None` when the extension is unavailable or the runtime doesn't support it.
    pub hand_tracker_left:    Option<openxr::HandTracker>,
    pub hand_tracker_right:   Option<openxr::HandTracker>,
    /// Reference space used as the base for joint location queries.
    /// Must share the same SessionInner as the hand trackers (created in the same attach call).
    pub hand_reference_space: Option<openxr::Space>,
}

// ---------------------------------------------------------------------------
// Plugin
// ---------------------------------------------------------------------------

pub struct XrInputPlugin;

impl Plugin for XrInputPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<XrInput>();
        app.add_message::<XrHapticRequest>();
        app.add_systems(
            OpenXrSchedules::SessionCreate,
            create_xr_input.in_set(OpenXrRuntimeSystems::SessionCreate),
        );
        app.add_systems(
            OpenXrSchedules::SessionCreate,
            attach_xr_input.in_set(OpenXrRuntimeSystems::PostSessionCreate),
        );
        app.add_systems(
            OpenXrSchedules::Update,
            (
                poll_xr_input,
                apply_haptic_feedback_system,
            )
                .in_set(OpenXrRuntimeSystems::PreFrameLoop)
                .run_if(openxr_in_state_focused),
        );
    }
}

// ---------------------------------------------------------------------------
// Session-create systems
// ---------------------------------------------------------------------------

fn create_xr_input(world: &mut World) {
    let instance = world.resource::<OpenXrInstance>();
    let xr = &instance.instance;

    let path_left  = xr.string_to_path("/user/hand/left").unwrap();
    let path_right = xr.string_to_path("/user/hand/right").unwrap();

    let action_set = xr
        .create_action_set("xrds_input", "XRDS Input", 0)
        .expect("create action set");

    let subaction_paths = &[path_left, path_right];
    let aim_pose        = action_set.create_action::<openxr::Posef>    ("aim_pose",        "Aim Pose",        subaction_paths).unwrap();
    let trigger         = action_set.create_action::<f32>              ("trigger",         "Trigger",         subaction_paths).unwrap();
    let grip            = action_set.create_action::<f32>              ("grip",            "Grip",            subaction_paths).unwrap();
    let thumbstick      = action_set.create_action::<openxr::Vector2f> ("thumbstick",      "Thumbstick",      subaction_paths).unwrap();
    let select          = action_set.create_action::<bool>             ("select",          "Select",          subaction_paths).unwrap();
    let menu            = action_set.create_action::<bool>             ("menu",            "Menu",            subaction_paths).unwrap();
    let thumbstick_click = action_set.create_action::<bool>            ("thumbstick_click","Thumbstick Click",subaction_paths).unwrap();
    let haptic          = action_set.create_action::<openxr::Haptic>   ("haptic",          "Haptic Output",   subaction_paths).unwrap();

    // Meta Quest Touch / Oculus Touch controllers
    let oculus = xr.string_to_path("/interaction_profiles/oculus/touch_controller").unwrap();
    xr.suggest_interaction_profile_bindings(oculus, &[
        openxr::Binding::new(&aim_pose,         xr.string_to_path("/user/hand/left/input/aim/pose").unwrap()),
        openxr::Binding::new(&aim_pose,         xr.string_to_path("/user/hand/right/input/aim/pose").unwrap()),
        openxr::Binding::new(&trigger,          xr.string_to_path("/user/hand/left/input/trigger/value").unwrap()),
        openxr::Binding::new(&trigger,          xr.string_to_path("/user/hand/right/input/trigger/value").unwrap()),
        openxr::Binding::new(&grip,             xr.string_to_path("/user/hand/left/input/squeeze/value").unwrap()),
        openxr::Binding::new(&grip,             xr.string_to_path("/user/hand/right/input/squeeze/value").unwrap()),
        openxr::Binding::new(&thumbstick,       xr.string_to_path("/user/hand/left/input/thumbstick").unwrap()),
        openxr::Binding::new(&thumbstick,       xr.string_to_path("/user/hand/right/input/thumbstick").unwrap()),
        // Primary face buttons: X (left) / A (right) → select
        openxr::Binding::new(&select,           xr.string_to_path("/user/hand/left/input/x/click").unwrap()),
        openxr::Binding::new(&select,           xr.string_to_path("/user/hand/right/input/a/click").unwrap()),
        // Secondary face buttons: Y (left) / B (right) → menu
        openxr::Binding::new(&menu,             xr.string_to_path("/user/hand/left/input/y/click").unwrap()),
        openxr::Binding::new(&menu,             xr.string_to_path("/user/hand/right/input/b/click").unwrap()),
        // Thumbstick press
        openxr::Binding::new(&thumbstick_click, xr.string_to_path("/user/hand/left/input/thumbstick/click").unwrap()),
        openxr::Binding::new(&thumbstick_click, xr.string_to_path("/user/hand/right/input/thumbstick/click").unwrap()),
        // Haptic output
        openxr::Binding::new(&haptic, xr.string_to_path("/user/hand/left/output/haptic").unwrap()),
        openxr::Binding::new(&haptic, xr.string_to_path("/user/hand/right/output/haptic").unwrap()),
    ]).expect("Oculus Touch bindings");

    // KHR simple controller — generic / emulator fallback
    let khr = xr.string_to_path("/interaction_profiles/khr/simple_controller").unwrap();
    xr.suggest_interaction_profile_bindings(khr, &[
        openxr::Binding::new(&aim_pose, xr.string_to_path("/user/hand/left/input/aim/pose").unwrap()),
        openxr::Binding::new(&aim_pose, xr.string_to_path("/user/hand/right/input/aim/pose").unwrap()),
        openxr::Binding::new(&select,   xr.string_to_path("/user/hand/left/input/select/click").unwrap()),
        openxr::Binding::new(&select,   xr.string_to_path("/user/hand/right/input/select/click").unwrap()),
        openxr::Binding::new(&haptic,   xr.string_to_path("/user/hand/left/output/haptic").unwrap()),
        openxr::Binding::new(&haptic,   xr.string_to_path("/user/hand/right/output/haptic").unwrap()),
    ]).expect("KHR simple bindings");

    world.insert_resource(OpenXrInput {
        action_set, aim_pose, trigger, grip, thumbstick, select, menu, thumbstick_click, haptic,
        aim_space_left:  None,
        aim_space_right: None,
        path_left,
        path_right,
        hand_tracker_left:    None,
        hand_tracker_right:   None,
        hand_reference_space: None,
    });
    info!("XR input actions created");
}

fn attach_xr_input(session: Res<OpenXrSession>, mut input: ResMut<OpenXrInput>) {
    session
        .attach_action_sets(&[&input.action_set])
        .expect("attach action sets");

    let sl = session
        .create_action_space(&input.aim_pose, input.path_left)
        .expect("left aim space");
    let sr = session
        .create_action_space(&input.aim_pose, input.path_right)
        .expect("right aim space");

    input.aim_space_left  = Some(sl);
    input.aim_space_right = Some(sr);
    info!("XR action sets attached");

    // --- Hand tracking (XR_EXT_hand_tracking) ---
    // Mirror reference_space.rs: prefer STAGE, fall back to LOCAL_FLOOR.
    // The Space and HandTrackers MUST be created from the same session call so they share
    // the same SessionInner pointer (required by Space::locate_hand_joints's assert).
    let identity = openxr::Posef {
        orientation: openxr::Quaternionf { x: 0.0, y: 0.0, z: 0.0, w: 1.0 },
        position:    openxr::Vector3f    { x: 0.0, y: 0.0, z: 0.0 },
    };
    let ref_space = session
        .create_owned_reference_space(openxr::ReferenceSpaceType::STAGE, identity)
        .or_else(|_| session.create_owned_reference_space(openxr::ReferenceSpaceType::LOCAL_FLOOR, identity));

    match ref_space {
        Ok(space) => {
            let lt = session.create_hand_tracker(openxr::Hand::LEFT);
            let rt = session.create_hand_tracker(openxr::Hand::RIGHT);
            match (lt, rt) {
                (Ok(lt), Ok(rt)) => {
                    input.hand_tracker_left    = Some(lt);
                    input.hand_tracker_right   = Some(rt);
                    input.hand_reference_space = Some(space);
                    info!("XR hand tracking initialized");
                }
                (lt, rt) => {
                    info!("XR hand tracking unavailable — left: {:?}, right: {:?}", lt.err(), rt.err());
                }
            }
        }
        Err(e) => info!("XR hand reference space unavailable: {e:?}"),
    }
}

// ---------------------------------------------------------------------------
// Per-frame poll system
// ---------------------------------------------------------------------------

fn poll_xr_input(
    session:       Res<OpenXrSession>,
    input:         Res<OpenXrInput>,
    frame_state:   Res<OpenXrFrameState>,
    primary_space: Res<OpenXrPrimaryReferenceSpace>,
    mut xr_input:  ResMut<XrInput>,
) {
    let active = ActiveActionSet::new(&input.action_set);
    if let Err(e) = session.sync_actions(&[active]) {
        warn!("sync_actions: {e:?}");
        return;
    }

    let l = input.path_left;
    let r = input.path_right;
    let t = frame_state.0.predicted_display_time;

    // Save previous select to compute edge detection after hand tracking fill.
    let prev_sel_l = xr_input.left.select;
    let prev_sel_r = xr_input.right.select;

    let trig_l  = read_f32(&session, &input.trigger, l);
    let trig_r  = read_f32(&session, &input.trigger, r);
    let grip_l  = read_f32(&session, &input.grip, l);
    let grip_r  = read_f32(&session, &input.grip, r);
    let stick_l = read_vec2(&session, &input.thumbstick, l);
    let stick_r = read_vec2(&session, &input.thumbstick, r);

    let (sel_l, _)      = read_bool_edge(&session, &input.select,          l);
    let (sel_r, _)      = read_bool_edge(&session, &input.select,          r);
    let (menu_l, mjp_l) = read_bool_edge(&session, &input.menu,            l);
    let (menu_r, mjp_r) = read_bool_edge(&session, &input.menu,            r);
    let (tc_l,   tc_jp_l) = read_bool_edge(&session, &input.thumbstick_click, l);
    let (tc_r,   tc_jp_r) = read_bool_edge(&session, &input.thumbstick_click, r);

    let pose_l = locate_aim_pose(input.aim_space_left.as_ref(),  &primary_space.0, t, &session);
    let pose_r = locate_aim_pose(input.aim_space_right.as_ref(), &primary_space.0, t, &session);

    xr_input.left = XrPointerState {
        pose:       pose_l,
        trigger:    trig_l,
        grip:       grip_l,
        thumbstick: stick_l,
        select:     sel_l || trig_l > 0.5,
        // edge fields computed below after hand tracking
        select_just_pressed:           false,
        select_just_released:          false,
        menu:                          menu_l,
        menu_just_pressed:             mjp_l,
        thumbstick_click:              tc_l,
        thumbstick_click_just_pressed: tc_jp_l,
        source: XrInputSource::Controller,
    };
    xr_input.right = XrPointerState {
        pose:       pose_r,
        trigger:    trig_r,
        grip:       grip_r,
        thumbstick: stick_r,
        select:     sel_r || trig_r > 0.5,
        select_just_pressed:           false,
        select_just_released:          false,
        menu:                          menu_r,
        menu_just_pressed:             mjp_r,
        thumbstick_click:              tc_r,
        thumbstick_click_just_pressed: tc_jp_r,
        source: XrInputSource::Controller,
    };

    // Hand tracking fallback — may override pose, trigger, grip, select, source.
    // menu / thumbstick_click remain controller-only (not overwritten here).
    if let Some(ref_space) = &input.hand_reference_space {
        fill_hand_if_untracked(&mut xr_input.left,  ref_space, &input.hand_tracker_left,  t);
        fill_hand_if_untracked(&mut xr_input.right, ref_space, &input.hand_tracker_right, t);
    }

    // Compute select edge detection AFTER hand tracking so it captures both input sources.
    xr_input.left.select_just_pressed   = !prev_sel_l && xr_input.left.select;
    xr_input.left.select_just_released  =  prev_sel_l && !xr_input.left.select;
    xr_input.right.select_just_pressed  = !prev_sel_r && xr_input.right.select;
    xr_input.right.select_just_released =  prev_sel_r && !xr_input.right.select;
}

// ---------------------------------------------------------------------------
// Haptic feedback system
// ---------------------------------------------------------------------------

fn apply_haptic_feedback_system(
    session: Res<OpenXrSession>,
    input:   Res<OpenXrInput>,
    mut req: MessageReader<XrHapticRequest>,
) {
    for r in req.read() {
        let path = match r.hand {
            XrHand::Left  => input.path_left,
            XrHand::Right => input.path_right,
        };
        let duration = if r.duration_secs > 0.0 {
            openxr::Duration::from_nanos((r.duration_secs * 1_000_000_000.0) as i64)
        } else {
            openxr::Duration::MIN_HAPTIC
        };
        if let Err(e) = session.apply_haptic_feedback(
            &input.haptic, path,
            r.amplitude.clamp(0.0, 1.0), duration, r.frequency,
        ) {
            warn!("apply_haptic_feedback ({:?}): {e:?}", r.hand);
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn read_f32(session: &OpenXrSession, action: &openxr::Action<f32>, path: openxr::Path) -> f32 {
    session.action_state_f32(action, path)
        .map(|s| if s.is_active { s.current_state } else { 0.0 })
        .unwrap_or(0.0)
}

fn read_vec2(
    session: &OpenXrSession,
    action:  &openxr::Action<openxr::Vector2f>,
    path:    openxr::Path,
) -> Vec2 {
    session.action_state_vec2f(action, path)
        .map(|s| if s.is_active { Vec2::new(s.current_state.x, s.current_state.y) } else { Vec2::ZERO })
        .unwrap_or(Vec2::ZERO)
}

/// Returns `(current_state, just_pressed)`.
/// `just_pressed` is true only on the frame the button first became active this sync cycle,
/// using OpenXR's `changed_since_last_sync` flag.
fn read_bool_edge(
    session: &OpenXrSession,
    action:  &openxr::Action<bool>,
    path:    openxr::Path,
) -> (bool, bool) {
    session.action_state_bool(action, path)
        .map(|s| {
            if !s.is_active { return (false, false); }
            let pressed     = s.current_state;
            let just_pressed = pressed && s.changed_since_last_sync;
            (pressed, just_pressed)
        })
        .unwrap_or((false, false))
}

// ---------------------------------------------------------------------------
// Hand tracking helpers
// ---------------------------------------------------------------------------

// XR_EXT_hand_tracking joint indices (OpenXR spec — stable)
const JOINT_THUMB_TIP:  usize = 5;
const JOINT_INDEX_PROX: usize = 7;
const JOINT_INDEX_TIP:  usize = 10;

/// If the controller for this hand has no pose, try to derive pointer state from hand joints.
fn fill_hand_if_untracked(
    state:      &mut XrPointerState,
    base_space: &openxr::Space,
    tracker:    &Option<openxr::HandTracker>,
    time:       openxr::Time,
) {
    if state.pose.is_some() { return; }

    let Some(tracker) = tracker else { return };
    let Ok(Some(joints)) = base_space.locate_hand_joints(tracker, time) else { return };

    let thumb_tip  = &joints[JOINT_THUMB_TIP];
    let index_prox = &joints[JOINT_INDEX_PROX];
    let index_tip  = &joints[JOINT_INDEX_TIP];

    use openxr::sys::SpaceLocationFlags;
    let valid = |j: &openxr::HandJointLocation| {
        j.location_flags.contains(SpaceLocationFlags::POSITION_VALID)
    };
    if !valid(thumb_tip) || !valid(index_prox) || !valid(index_tip) { return; }

    let prox  = xr_to_vec3(index_prox.pose.position);
    let tip   = xr_to_vec3(index_tip.pose.position);
    let thumb = xr_to_vec3(thumb_tip.pose.position);

    // Aim ray: align -Z with (index proximal → index tip) direction
    let dir = (tip - prox).normalize_or_zero();
    let rot = if dir.length_squared() > 0.001 {
        Quat::from_rotation_arc(-Vec3::Z, dir)
    } else {
        Quat::IDENTITY
    };
    state.pose = Some(Transform::from_translation(prox).with_rotation(rot));

    // Pinch: index tip ↔ thumb tip distance → trigger (0 = open 8 cm, 1 = pinched 2 cm)
    let dist    = (tip - thumb).length();
    let trigger = 1.0 - ((dist - 0.02) / (0.08 - 0.02)).clamp(0.0, 1.0);
    state.trigger    = trigger;
    state.grip       = trigger;
    state.thumbstick = Vec2::ZERO;
    state.select     = trigger > 0.5;
    // select_just_pressed / select_just_released computed in poll_xr_input after this call
    state.source     = XrInputSource::Hand;
}

#[inline]
fn xr_to_vec3(v: openxr::Vector3f) -> Vec3 {
    Vec3::new(v.x, v.y, v.z)
}

fn locate_aim_pose(
    space:   Option<&openxr::Space>,
    base:    &OpenXrSpace,
    time:    openxr::Time,
    session: &OpenXrSession,
) -> Option<Transform> {
    let space = space?;
    // Borrow the raw handle without transferring ownership — openxr::Space still owns it.
    let space_raw = OpenXrSpace(space.as_raw().into_raw());
    let loc = session.locate_space(&space_raw, base, time).ok()?;
    let flags = loc.location_flags;
    if !flags.contains(openxr::sys::SpaceLocationFlags::POSITION_VALID)
        || !flags.contains(openxr::sys::SpaceLocationFlags::ORIENTATION_VALID)
    {
        return None;
    }
    let p = loc.pose.position;
    let q = loc.pose.orientation;
    Some(
        Transform::from_translation(Vec3::new(p.x, p.y, p.z))
            .with_rotation(Quat::from_xyzw(q.x, q.y, q.z, q.w)),
    )
}
