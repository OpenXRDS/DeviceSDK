use bevy::{
    anti_alias::smaa::{Smaa, SmaaPreset},
    input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll, MouseScrollUnit},
    prelude::*,
    window::{CursorGrabMode, CursorOptions},
};
use xrds::sdk::{
    primitives::{XrdsExtrudedText, XrdsExtrudedTextAlignment},
    world::{lights::XrdsAmbientLight, lights::XrdsDirectionalLight, XrdsCamera},
    TransformParams,
};
use xrds::{Handle, Runtime, RuntimeParameters, XrdsAPI, XrdsApp, XrdsUpdateContext};

// ── FPS camera controller ────────────────────────────────────────────────────

#[derive(Component)]
struct FpsController {
    yaw: f32,
    pitch: f32,
    walk_speed: f32,
    run_speed: f32,
    sensitivity: f32,
    initialized: bool,
}

impl Default for FpsController {
    fn default() -> Self {
        Self {
            yaw: 0.0,
            pitch: 0.0,
            walk_speed: 3.0,
            run_speed: 9.0,
            sensitivity: 1.0 / 180.0,
            initialized: false,
        }
    }
}

fn attach_fps_controller(
    mut commands: Commands,
    camera_q: Query<Entity, Added<Camera3d>>,
) {
    for entity in &camera_q {
        commands
            .entity(entity)
            .insert((FpsController::default(), Smaa { preset: SmaaPreset::High }));
    }
}

fn run_fps_controller(
    time: Res<Time>,
    keys: Res<ButtonInput<KeyCode>>,
    mouse_motion: Res<AccumulatedMouseMotion>,
    scroll: Res<AccumulatedMouseScroll>,
    mut cursor_q: Query<&mut CursorOptions>,
    mouse_buttons: Res<ButtonInput<MouseButton>>,
    mut query: Query<(&mut FpsController, &mut Transform), With<Camera3d>>,
) {
    let cursor_locked = cursor_q
        .iter()
        .any(|c| c.grab_mode == CursorGrabMode::Locked);

    if keys.just_pressed(KeyCode::Escape) {
        for mut cursor in &mut cursor_q {
            cursor.grab_mode = CursorGrabMode::None;
            cursor.visible = true;
        }
        return;
    }
    if mouse_buttons.just_pressed(MouseButton::Left) && !cursor_locked {
        for mut cursor in &mut cursor_q {
            cursor.grab_mode = CursorGrabMode::Locked;
            cursor.visible = false;
        }
        return;
    }

    for (mut ctrl, mut transform) in &mut query {
        if !ctrl.initialized {
            let (yaw, pitch, _) = transform.rotation.to_euler(EulerRot::YXZ);
            ctrl.yaw = yaw;
            ctrl.pitch = pitch;
            ctrl.initialized = true;
        }

        if cursor_locked {
            ctrl.yaw -= mouse_motion.delta.x * ctrl.sensitivity;
            ctrl.pitch = (ctrl.pitch - mouse_motion.delta.y * ctrl.sensitivity)
                .clamp(-1.5, 1.5);
            transform.rotation =
                Quat::from_euler(EulerRot::YXZ, ctrl.yaw, ctrl.pitch, 0.0);
        }

        let speed = if keys.pressed(KeyCode::ShiftLeft) {
            ctrl.run_speed
        } else {
            ctrl.walk_speed
        };
        let dt = time.delta_secs();
        let fwd = transform.forward();
        let right = transform.right();

        let scroll_zoom = match scroll.unit {
            MouseScrollUnit::Line => scroll.delta.y * 0.5,
            MouseScrollUnit::Pixel => scroll.delta.y * 0.01,
        };

        if keys.pressed(KeyCode::KeyW) {
            transform.translation += *fwd * speed * dt;
        }
        if keys.pressed(KeyCode::KeyS) {
            transform.translation -= *fwd * speed * dt;
        }
        if keys.pressed(KeyCode::KeyA) {
            transform.translation -= *right * speed * dt;
        }
        if keys.pressed(KeyCode::KeyD) {
            transform.translation += *right * speed * dt;
        }
        if keys.pressed(KeyCode::KeyE) || scroll_zoom > 0.0 {
            transform.translation.y += speed * dt;
        }
        if keys.pressed(KeyCode::KeyQ) || scroll_zoom < 0.0 {
            transform.translation.y -= speed * dt;
        }
    }
}

// ── App ──────────────────────────────────────────────────────────────────────

struct ExtrudedTextApp {
    title_handle: Option<Handle<XrdsExtrudedText>>,
    deep_handle: Option<Handle<XrdsExtrudedText>>,
}

impl Default for ExtrudedTextApp {
    fn default() -> Self {
        Self {
            title_handle: None,
            deep_handle: None,
        }
    }
}

impl XrdsApp for ExtrudedTextApp {
    fn configure(&mut self, app: &mut App) {
        app.add_systems(PostStartup, attach_fps_controller);
        app.add_systems(Update, run_fps_controller);
    }

    fn setup(&mut self, api: &mut XrdsAPI<'_>) {
        // Camera
        api.spawn(
            &XrdsCamera::perspective(60.0)
                .with_name("Camera")
                .near(0.1)
                .far(200.0)
                .at([0.0, 1.0, 6.0])
                .looking_at([0.0, 1.0, 0.0]),
        );

        // Lighting
        api.spawn(&{
            let mut a = XrdsAmbientLight::new().with_name("Ambient");
            a.brightness = 300.0;
            a
        });
        api.spawn(&{
            let mut d = XrdsDirectionalLight::new().with_name("Sun");
            d.illuminance = 10_000.0;
            let r = Quat::from_euler(EulerRot::YXZ, -0.5, -1.0, 0.0);
            d.transform.rotation_quat_xyzw = [r.x, r.y, r.z, r.w];
            d
        });

        // Title — moderate extrusion, metallic blue
        self.title_handle = Some(api.spawn(&{
            let mut t = XrdsExtrudedText::new()
                .with_name("Title")
                .with_text("XRDS")
                .with_depth(0.2);
            t.font_size = 96.0;
            t.color = [0.2, 0.5, 1.0, 1.0];
            t.alignment = XrdsExtrudedTextAlignment::Center;
            t.transform.translation = [-1.8, 2.0, 0.0];
            t
        }));

        // Subtitle — shallow extrusion, white
        api.spawn(&{
            let mut t = XrdsExtrudedText::new()
                .with_name("Subtitle")
                .with_text("Extruded Text Demo")
                .with_depth(0.05);
            t.font_size = 36.0;
            t.color = [0.9, 0.9, 0.9, 1.0];
            t.alignment = XrdsExtrudedTextAlignment::Center;
            t.transform.translation = [-2.5, 1.0, 0.0];
            t
        });

        // Deep extruded label — slow Y rotation to reveal depth from sides
        self.deep_handle = Some(api.spawn(&{
            let mut t = XrdsExtrudedText::new()
                .with_name("Deep")
                .with_text("SDK")
                .with_depth(0.8);
            t.font_size = 72.0;
            t.color = [1.0, 0.4, 0.1, 1.0];
            t.alignment = XrdsExtrudedTextAlignment::Center;
            t.transform.translation = [0.5, -0.5, 0.0];
            t
        }));
    }

    fn update(&mut self, ctx: &mut XrdsUpdateContext<'_>) {
        // Slowly rotate the deep label so extrusion depth is visible from the side
        if let Some(h) = &self.deep_handle {
            let angle = ctx.elapsed_secs() * 0.6;
            let r = Quat::from_euler(EulerRot::YXZ, angle, 0.0, 0.0);
            ctx.set_transform(h, {
                let mut t = TransformParams::default();
                t.translation = [0.5, -0.5, 0.0];
                t.rotation_quat_xyzw = [r.x, r.y, r.z, r.w];
                t
            });
        }
    }
}

fn main() {
    Runtime::new(RuntimeParameters {
        app_name: "Extruded Text Demo".to_owned(),
        ..Default::default()
    })
    .run_xrds(ExtrudedTextApp::default())
    .expect("Could not run application");
}
