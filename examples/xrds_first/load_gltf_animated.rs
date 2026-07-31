use xrds::sdk::world::{XrdsCamera, XrdsGltfAsset};
use xrds::*;

fn main() {
    Runtime::new(RuntimeParameters {
        app_name: "LoadAnimatedGltf".to_owned(),
        ..Default::default()
    })
    .run_xrds(LoadAnimatedGltfApp {
        model_handle: None,
        animation_started: false,
    })
    .unwrap();
}

pub struct LoadAnimatedGltfApp {
    model_handle: Option<Handle<XrdsGltfAsset>>,
    animation_started: bool,
}

impl XrdsApp for LoadAnimatedGltfApp {
    fn setup(&mut self, api: &mut XrdsAPI<'_>) {
        // Spawn Buster Drone
        self.model_handle = Some(api.spawn(&{
            let mut gltf =
                XrdsGltfAsset::new("models/animated/buster_drone.glb").with_name("BusterDrone");
            gltf.transform.translation = [0.0, -1.0, -4.0];
            gltf
        }));

        // Setup Camera
        let _camera = api.spawn(&{
            XrdsCamera::perspective(50.0)
                .with_name("Camera")
                .near(0.1)
                .far(200.0)
                .order(0)
                .at([0.0, 1.0, 2.0])
                .looking_at([0.0, 0.0, -4.0])
        });
    }

    fn update(&mut self, ctx: &mut XrdsUpdateContext<'_>) {
        if self.animation_started {
            return;
        }

        let Some(handle) = &self.model_handle else {
            return;
        };

        if !matches!(
            ctx.gltf_load_status(handle),
            Some(XrdsGltfLoadStatus::Loaded)
        ) {
            return;
        }

        println!("BusterDrone is fully loaded. Checking animations...");

        let animations = match ctx.gltf_animations(handle) {
            Ok(a) => a,
            Err(e) => {
                println!("Error getting animation info: {:?}", e);
                return;
            }
        };

        if animations.is_empty() {
            println!("Warning: No animations found in model");
            self.animation_started = true;
            return;
        }

        for anim in &animations {
            println!(
                "  Animation {}: {}{}",
                anim.index,
                anim.name.as_deref().unwrap_or("unnamed"),
                anim.duration_secs
                    .map(|d| format!("  ({:.2}s)", d))
                    .unwrap_or_default(),
            );
        }

        // Play the first animation on loop
        let selector = match &animations[0].name {
            Some(name) => XrdsGltfAnimationSelector::Name(name.clone()),
            None => XrdsGltfAnimationSelector::Index(0),
        };

        match ctx.play_gltf_animation(
            handle,
            selector,
            XrdsGltfAnimationPlaybackOptions {
                repeat: XrdsAnimationRepeatMode::Loop,
                ..Default::default()
            },
        ) {
            Ok(()) => println!("Animation started."),
            Err(e) => println!("Failed to start animation: {:?}", e),
        }

        self.animation_started = true;
    }
}
