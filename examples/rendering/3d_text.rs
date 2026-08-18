use bevy::{
    anti_alias::smaa::{Smaa, SmaaPreset},
    input::mouse::{AccumulatedMouseMotion, AccumulatedMouseScroll, MouseScrollUnit},
    prelude::*,
    window::{CursorGrabMode, CursorOptions},
};
use xrds::sdk::{
    primitives::{XrdsText, XrdsTextAlignment},
    world::{lights::XrdsAmbientLight, lights::XrdsDirectionalLight, XrdsCamera},
    TextParams,
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
    cameras: Query<Entity, With<Camera3d>>,
    mut windows: Query<&mut CursorOptions>,
) {
    for entity in &cameras {
        commands.entity(entity).insert((
            FpsController::default(),
            Smaa { preset: SmaaPreset::High },
        ));
    }
    // Lock cursor immediately so mouse look is active from first frame.
    for mut opts in &mut windows {
        opts.grab_mode = CursorGrabMode::Locked;
        opts.visible = false;
    }
}

fn run_fps_controller(
    time: Res<Time>,
    key: Res<ButtonInput<KeyCode>>,
    mouse_button: Res<ButtonInput<MouseButton>>,
    mouse: Res<AccumulatedMouseMotion>,
    scroll: Res<AccumulatedMouseScroll>,
    mut windows: Query<(&Window, &mut CursorOptions)>,
    mut query: Query<(&mut Transform, &mut FpsController), With<Camera3d>>,
) {
    let Ok((mut transform, mut ctrl)) = query.single_mut() else {
        return;
    };

    if !ctrl.initialized {
        let (yaw, pitch, _) = transform.rotation.to_euler(EulerRot::YXZ);
        ctrl.yaw = yaw;
        ctrl.pitch = pitch;
        ctrl.initialized = true;
    }

    // Check current cursor state
    let cursor_locked = windows
        .iter()
        .next()
        .map(|(_, opts)| opts.grab_mode == CursorGrabMode::Locked)
        .unwrap_or(false);

    // Escape releases cursor; left-click re-locks it
    if key.just_pressed(KeyCode::Escape) && cursor_locked {
        for (_, mut opts) in &mut windows {
            opts.grab_mode = CursorGrabMode::None;
            opts.visible = true;
        }
        return;
    }
    if mouse_button.just_pressed(MouseButton::Left) && !cursor_locked {
        for (_, mut opts) in &mut windows {
            opts.grab_mode = CursorGrabMode::Locked;
            opts.visible = false;
        }
        return;
    }

    if !cursor_locked {
        return;
    }

    let dt = time.delta_secs();

    // Scroll adjusts speed
    let scroll_delta = match scroll.unit {
        MouseScrollUnit::Line => scroll.delta.y,
        MouseScrollUnit::Pixel => scroll.delta.y / 16.0,
    };
    ctrl.walk_speed = (ctrl.walk_speed + scroll_delta * 0.5).clamp(0.5, 50.0);
    ctrl.run_speed = ctrl.walk_speed * 3.0;

    // Mouse look
    if mouse.delta.x != 0.0 || mouse.delta.y != 0.0 {
        ctrl.yaw -= mouse.delta.x * ctrl.sensitivity;
        ctrl.pitch = (ctrl.pitch - mouse.delta.y * ctrl.sensitivity)
            .clamp(-std::f32::consts::FRAC_PI_2 + 0.01, std::f32::consts::FRAC_PI_2 - 0.01);
        transform.rotation = Quat::from_euler(EulerRot::YXZ, ctrl.yaw, ctrl.pitch, 0.0);
    }

    // Keyboard movement
    let speed = if key.pressed(KeyCode::ShiftLeft) || key.pressed(KeyCode::ShiftRight) {
        ctrl.run_speed
    } else {
        ctrl.walk_speed
    };

    let mut axis = Vec3::ZERO;
    if key.pressed(KeyCode::KeyW) { axis.z += 1.0; }
    if key.pressed(KeyCode::KeyS) { axis.z -= 1.0; }
    if key.pressed(KeyCode::KeyD) { axis.x += 1.0; }
    if key.pressed(KeyCode::KeyA) { axis.x -= 1.0; }
    if key.pressed(KeyCode::KeyE) { axis.y += 1.0; }
    if key.pressed(KeyCode::KeyQ) { axis.y -= 1.0; }

    if axis != Vec3::ZERO {
        let forward = *transform.forward();
        let right = *transform.right();
        let vel = axis.normalize() * speed;
        transform.translation +=
            vel.z * dt * forward + vel.x * dt * right + vel.y * dt * Vec3::Y;
    }
}

// ── App ─────────────────────────────────────────────────────────────────────

struct TextDemoApp {
    hello_handle: Option<Handle<XrdsText>>,
    counter_handle: Option<Handle<XrdsText>>,
    multiline_handle: Option<Handle<XrdsText>>,
    last_count: u32,
}

impl Default for TextDemoApp {
    fn default() -> Self {
        Self {
            hello_handle: None,
            counter_handle: None,
            multiline_handle: None,
            last_count: u32::MAX,
        }
    }
}

pub fn main() {
    Runtime::new(RuntimeParameters {
        app_name: "3D Text Demo".to_owned(),
        ..Default::default()
    })
    .run_xrds(TextDemoApp::default())
    .expect("Could not run application");
}

impl XrdsApp for TextDemoApp {
    fn configure(&mut self, app: &mut App) {
        app.add_systems(PostStartup, attach_fps_controller);
        app.add_systems(Update, run_fps_controller);
    }

    fn setup(&mut self, api: &mut XrdsAPI<'_>) {
        api.spawn(
            &XrdsCamera::perspective(60.0)
                .with_name("Camera")
                .near(0.1)
                .far(100.0)
                .at([0.0, 1.5, 6.0])
                .looking_at([0.0, 1.0, 0.0]),
        );

        api.spawn(&{
            let mut light = XrdsDirectionalLight::new().with_name("Sun");
            light.illuminance = 10_000.0;
            light.transform.rotation_quat_xyzw = [-0.383, 0.0, 0.0, 0.924];
            light
        });

        api.spawn(&{
            let mut ambient = XrdsAmbientLight::new().with_name("Ambient");
            ambient.brightness = 150.0;
            ambient
        });

        // Large centered title
        let hello = api.spawn(&{
            let mut t = XrdsText::new()
                .with_name("Title")
                .with_text("Hello, 3D World!");
            t.font_size = 48.0;
            t.color = [0.2, 0.9, 1.0, 1.0];
            t.alignment = XrdsTextAlignment::Center;
            t.transform.translation = [0.0, 2.5, 0.0];
            t
        });

        // Live counter — updated every second
        let counter = api.spawn(&{
            let mut t = XrdsText::new()
                .with_name("Counter")
                .with_text("Elapsed: 0s");
            t.font_size = 32.0;
            t.color = [1.0, 1.0, 0.4, 1.0];
            t.alignment = XrdsTextAlignment::Center;
            t.transform.translation = [0.0, 1.5, 0.0];
            t
        });

        // Multi-line left-aligned block
        let multiline = api.spawn(&{
            let mut t = XrdsText::new()
                .with_name("Multiline")
                .with_text("Mouse    — look around\nWASD     — move\nQ / E    — down / up\nShift    — run\nScroll   — adjust speed\nEsc      — release cursor\nLClick   — re-lock cursor");
            t.font_size = 22.0;
            t.color = [0.9, 0.9, 0.9, 1.0];
            t.alignment = XrdsTextAlignment::Left;
            t.transform.translation = [-3.5, 2.0, 0.0];
            t
        });

        self.hello_handle = Some(hello);
        self.counter_handle = Some(counter);
        self.multiline_handle = Some(multiline);
    }

    fn update(&mut self, ctx: &mut XrdsUpdateContext<'_>) {
        // Pulse title color
        let pulse = (ctx.elapsed_secs() * 1.5).sin() * 0.4 + 0.6;
        if let Some(handle) = &self.hello_handle {
            if let Some(mut params) = ctx.text_params(handle) {
                params.color = [pulse * 0.3, 0.9, 1.0, 1.0];
                ctx.set_text_params(handle, params);
            }
        }

        // Update counter once per second
        let count = ctx.elapsed_secs() as u32;
        if count != self.last_count {
            self.last_count = count;
            if let Some(handle) = &self.counter_handle {
                ctx.set_text_params(
                    handle,
                    TextParams {
                        text: format!("Elapsed: {count}s"),
                        font_size: 32.0,
                        color: [1.0, 1.0, 0.4, 1.0],
                        alignment: XrdsTextAlignment::Center,
                    },
                );
            }
        }
    }
}
