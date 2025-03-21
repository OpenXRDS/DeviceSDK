use std::{f32::consts::PI, path::PathBuf};

use rand::{rng, Rng};
use xrds::{core::core::Transform, RuntimeHandler};

#[derive(Clone, clap::Args)]
pub struct GltfOptions {
    /// Enable openxr rendering
    #[arg(long, default_value_t = false)]
    pub enable_xr: bool,
    /// Disable window rendering
    #[arg(long, default_value_t = false)]
    pub disable_window: bool,
    /// Viewer window width
    #[arg(long, default_value_t = 1280)]
    pub width: u32,
    /// Viewer window height
    #[arg(long, default_value_t = 720)]
    pub height: u32,
    /// Make viewer window resizable
    #[arg(long, default_value_t = false)]
    pub resizable: bool,
    /// Path of .gltf or .glb file
    pub path: String,
}

#[derive(Default)]
struct App {
    gltf_path: String,
    objects: Vec<xrds::Object>,
}

impl RuntimeHandler for App {
    fn on_construct(&mut self) -> anyhow::Result<()> {
        Ok(())
    }

    fn on_begin(&mut self, context: xrds::Context) -> anyhow::Result<()> {
        self.objects = context.load_objects_from_gltf(self.gltf_path.as_str())?;

        let world = context.get_current_world();
        let uniform = rand::distr::Uniform::new(0.0f32, 1.0f32)?;
        for _ in 0..10000 {
            let distance = rng().sample(uniform) * 50.0 + 0.5;
            let angle = rng().sample(uniform) * PI * 2.0;
            let tx = distance * angle.cos();
            let tz = distance * angle.sin();
            let s = rng().sample(uniform) + 0.5;
            let ry = rng().sample(uniform) * PI * 2.0;

            let transform = Transform::default()
                .with_translation(glam::vec3(tx, 0.0, tz))
                .with_scale(glam::vec3(s, s, s))
                .with_rotation(glam::Quat::from_rotation_y(ry));
            world.spawn(&self.objects[0], &transform)?;
        }
        Ok(())
    }

    fn on_resumed(&mut self, _context: xrds::Context) -> anyhow::Result<()> {
        Ok(())
    }

    fn on_update(&mut self, _context: xrds::Context) -> anyhow::Result<()> {
        Ok(())
    }

    fn on_suspended(&mut self, _context: xrds::Context) -> anyhow::Result<()> {
        Ok(())
    }

    fn on_end(&mut self, _context: xrds::Context) -> anyhow::Result<()> {
        Ok(())
    }

    fn on_deconstruct(&mut self, _context: xrds::Context) -> anyhow::Result<()> {
        Ok(())
    }
}

pub fn run(options: GltfOptions) -> anyhow::Result<()> {
    let target = if options.enable_xr {
        if options.disable_window {
            xrds::RuntimeTarget::Xr
        } else {
            xrds::RuntimeTarget::XrWithPreview
        }
    } else if options.disable_window {
        anyhow::bail!("Both xr and window disabled. At least one must be enabled")
    } else {
        xrds::RuntimeTarget::Window
    };
    let mut runtime_builder = xrds::Runtime::builder()
        .with_application_name("gltf_viewer")
        .with_target(target);

    match target {
        xrds::RuntimeTarget::Window | xrds::RuntimeTarget::XrWithPreview => {
            runtime_builder = runtime_builder.with_window_options(xrds::RuntimeWindowOptions {
                width: options.width,
                height: options.height,
                resizable: options.resizable,
                title: "gitf_viewer".to_owned(),
                ..Default::default()
            })
        }
        _ => {}
    }
    let runtime = runtime_builder.build()?;

    let path = PathBuf::from(options.path);

    let gltf_file = if path.is_file() {
        if let Some(extension) = path.extension() {
            let extension_str = extension.to_string_lossy();
            if extension_str != "gltf" && extension_str != "glb" {
                anyhow::bail!("Invalid file extension: {}", extension_str);
            }
        } else {
            anyhow::bail!("Invalid file extension");
        }
        path
    } else {
        let gltf_files: Vec<_> = path
            .read_dir()?
            .filter_map(|e| e.ok())
            .filter_map(|e| {
                let path = e.path();
                if let Some(extension) = path.extension() {
                    let extension_str = extension.to_string_lossy();
                    if extension_str == "gltf" || extension_str == "glb" {
                        Some(path)
                    } else {
                        None
                    }
                } else {
                    None
                }
            })
            .collect();
        if gltf_files.is_empty() {
            anyhow::bail!("No gltf file found");
        }

        // viewer support only 1 gltf or glb file so we use first gltf file
        gltf_files[0].clone()
    };

    let app = App {
        gltf_path: gltf_file.to_string_lossy().to_string(),
        ..Default::default()
    };

    runtime.run(app)?;

    Ok(())
}
